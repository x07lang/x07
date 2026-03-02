use std::collections::BTreeMap;

use serde_json::Value;
use wasm_encoder::{
    CodeSection, ConstExpr, ExportKind, ExportSection, Function, FunctionSection, GlobalSection,
    GlobalType, Instruction, MemorySection, MemoryType, Module, TypeSection, ValType,
};

use crate::ast::Expr;
use crate::compile::{CompileErrorKind, CompileOptions, CompilerError};
use crate::diagnostics::{Diagnostic, Location, Severity, Stage};
use crate::program::{FunctionDef, Program};
use crate::types::Ty;
use crate::wasm_emit::features::WasmFeatureV1;
use crate::wasm_emit::{layout, WasmEmitOptions};

#[derive(Debug, Clone)]
struct FuncSig {
    params: Vec<Ty>,
    ret: Ty,
}

#[derive(Debug, Default)]
struct DataBuilder {
    bytes: Vec<u8>,
    offsets: BTreeMap<Vec<u8>, u32>,
}

impl DataBuilder {
    fn intern(&mut self, data: &[u8]) -> u32 {
        if let Some(&off) = self.offsets.get(data) {
            return off;
        }
        let off = self.bytes.len() as u32;
        self.bytes.extend_from_slice(data);
        self.offsets.insert(data.to_vec(), off);
        off
    }
}

struct ModuleCtx {
    func_indices: BTreeMap<String, u32>,
    func_sigs: BTreeMap<String, FuncSig>,
    features: crate::wasm_emit::features::WasmFeatureSetV1,
    data: DataBuilder,

    // Indices wired by emit_solve_pure_wasm_v1.
    rt_alloc_fn: u32,
    solve_fn: u32,

    heap_base_global: u32,
    heap_ptr_global: u32,
    heap_end_global: u32,
}

#[derive(Debug, Clone)]
struct Binding {
    ty: Ty,
    locals: Vec<u32>,
}

#[derive(Debug)]
struct FuncCode {
    params_flat_len: u32,
    locals: Vec<ValType>,
    body: Vec<Instruction<'static>>,
}

impl FuncCode {
    fn new(params_flat_len: u32) -> Self {
        Self {
            params_flat_len,
            locals: Vec::new(),
            body: Vec::new(),
        }
    }

    fn push(&mut self, instr: Instruction<'static>) {
        self.body.push(instr);
    }

    fn new_i32_local(&mut self) -> u32 {
        let idx = self.params_flat_len + self.locals.len() as u32;
        self.locals.push(ValType::I32);
        idx
    }

    fn new_locals_for_ty(&mut self, ty: Ty) -> Result<Vec<u32>, CompilerError> {
        let n = flat_len_for_ty(ty)?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.new_i32_local());
        }
        Ok(out)
    }
}

struct ExprEmitter<'a> {
    module: &'a mut ModuleCtx,
    f: &'a mut FuncCode,
    env: Vec<BTreeMap<String, Binding>>,
}

impl<'a> ExprEmitter<'a> {
    fn new(module: &'a mut ModuleCtx, f: &'a mut FuncCode) -> Self {
        Self {
            module,
            f,
            env: vec![BTreeMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.env.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.env.pop();
    }

    fn bind(&mut self, name: String, binding: Binding) -> Result<(), CompilerError> {
        let Some(scope) = self.env.last_mut() else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: missing scope".to_string(),
            ));
        };
        if scope.contains_key(&name) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("duplicate let binding in same scope: {name:?}"),
            ));
        }
        scope.insert(name, binding);
        Ok(())
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        for scope in self.env.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    fn require_head_feature(&self, head: &str, ptr: &str) -> Result<(), CompilerError> {
        let Some((kind, feature)) = required_feature_for_head(head) else {
            return Ok(());
        };
        if self.module.features.has(feature) {
            return Ok(());
        }
        Err(wasm_unsupported(kind, head, feature, ptr))
    }

    fn infer_ty(&self, expr: &Expr) -> Result<Ty, CompilerError> {
        match expr {
            Expr::Int { .. } => Ok(Ty::I32),
            Expr::Ident { name, .. } => self.lookup(name).map(|b| b.ty).ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                )
            }),
            Expr::List { items, ptr } => {
                let (head, args) = split_head(items)?;
                self.require_head_feature(head, ptr)?;
                match head {
                    "begin" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "(begin ...) requires at least 1 expression".to_string(),
                            ));
                        }
                        self.infer_ty(&args[args.len() - 1])
                    }
                    "let" | "set" | "set0" => Ok(Ty::I32),
                    "for" => Ok(Ty::I32),
                    "return" => Ok(Ty::Never),
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond> <then> <else>)".to_string(),
                            ));
                        }
                        let t = self.infer_ty(&args[1])?;
                        let e = self.infer_ty(&args[2])?;
                        if t == Ty::Never {
                            return Ok(e);
                        }
                        if e == Ty::Never {
                            return Ok(t);
                        }
                        if t != e {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("if branch type mismatch: then={t:?} else={e:?}"),
                            ));
                        }
                        Ok(t)
                    }

                    // Minimal builtins.
                    "i32.lit" => Ok(Ty::I32),
                    "view.len" => Ok(Ty::I32),
                    "view.get_u8" => Ok(Ty::I32),
                    "view.slice" => Ok(Ty::BytesView),
                    "view.to_bytes" => Ok(Ty::Bytes),
                    "view.eq" => Ok(Ty::I32),
                    "bytes.view" => Ok(Ty::BytesView),
                    "bytes.subview" => Ok(Ty::BytesView),
                    "bytes.len" => Ok(Ty::I32),
                    "bytes.get_u8" => Ok(Ty::I32),
                    "bytes.set_u8" => Ok(Ty::Bytes),
                    "bytes.alloc" => Ok(Ty::Bytes),
                    "bytes.concat" => Ok(Ty::Bytes),
                    "bytes.eq" => Ok(Ty::I32),
                    "bytes.cmp_range" => Ok(Ty::I32),
                    "bytes.slice" => Ok(Ty::Bytes),
                    "bytes.lit" => Ok(Ty::Bytes),
                    "bytes.view_lit" => Ok(Ty::BytesView),
                    "codec.read_u32_le" => Ok(Ty::I32),
                    "codec.write_u32_le" => Ok(Ty::Bytes),
                    "vec_u8.with_capacity" => Ok(Ty::VecU8),
                    "vec_u8.len" => Ok(Ty::I32),
                    "vec_u8.get" => Ok(Ty::I32),
                    "vec_u8.push" => Ok(Ty::VecU8),
                    "vec_u8.reserve_exact" => Ok(Ty::VecU8),
                    "vec_u8.extend_bytes" => Ok(Ty::VecU8),
                    "vec_u8.extend_bytes_range" => Ok(Ty::VecU8),
                    "vec_u8.as_view" => Ok(Ty::BytesView),
                    "vec_u8.into_bytes" => Ok(Ty::Bytes),

                    // Minimal ops.
                    "+" | "-" | "*" | "/" | "%" | "=" | "!=" | "<" | "<u" | "<=u" | ">u"
                    | ">=u" | "<=" | ">" | ">=" => Ok(Ty::I32),
                    "&&" | "||" | "&" | "|" | "^" | "<<u" | ">>u" => Ok(Ty::I32),

                    // User function call.
                    _ => {
                        if let Some(sig) = self.module.func_sigs.get(head) {
                            return Ok(sig.ret);
                        }
                        if head.starts_with("bytes.")
                            || head.starts_with("view.")
                            || head.starts_with("codec.")
                            || head.starts_with("vec_u8.")
                        {
                            return Err(wasm_unsupported(
                                "builtin",
                                head,
                                required_feature_for_head(head)
                                    .map(|(_, f)| f)
                                    .unwrap_or(WasmFeatureV1::CoreFormsV1),
                                ptr,
                            ));
                        }
                        Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("unknown callee: {head:?}"),
                        ))
                    }
                }
            }
        }
    }

    fn emit_expr(&mut self, expr: &Expr) -> Result<Ty, CompilerError> {
        match expr {
            Expr::Int { value, .. } => {
                self.f.push(Instruction::I32Const(*value));
                Ok(Ty::I32)
            }
            Expr::Ident { name, .. } => {
                let binding = self.lookup(name).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    )
                })?;
                let locals = binding.locals.clone();
                let ty = binding.ty;
                for l in locals {
                    self.f.push(Instruction::LocalGet(l));
                }
                Ok(ty)
            }
            Expr::List { items, ptr } => {
                let (head, args) = split_head(items)?;
                self.require_head_feature(head, ptr)?;
                match head {
                    "begin" => self.emit_begin(args),
                    "let" => self.emit_let(args),
                    "set" => self.emit_set(args),
                    "set0" => {
                        let _ = self.emit_set(args)?;
                        Ok(Ty::I32)
                    }
                    "if" => self.emit_if(args),
                    "for" => self.emit_for(args),
                    "return" => self.emit_return(args),

                    // Builtins (minimal set for Phase 7 gates).
                    "i32.lit" => self.emit_i32_lit(args),
                    "view.len" => self.emit_view_len(args),
                    "view.get_u8" => self.emit_view_get_u8(args),
                    "view.slice" => self.emit_view_slice(args),
                    "view.to_bytes" => self.emit_view_to_bytes(args),
                    "view.eq" => self.emit_view_eq(args),
                    "bytes.view" => self.emit_bytes_view(args),
                    "bytes.subview" => self.emit_bytes_subview(args),
                    "bytes.len" => self.emit_bytes_len(args),
                    "bytes.get_u8" => self.emit_bytes_get_u8(args),
                    "bytes.set_u8" => self.emit_bytes_set_u8(args),
                    "bytes.alloc" => self.emit_bytes_alloc(args),
                    "bytes.concat" => self.emit_bytes_concat(args),
                    "bytes.eq" => self.emit_bytes_eq(args),
                    "bytes.cmp_range" => self.emit_bytes_cmp_range(args),
                    "bytes.slice" => self.emit_bytes_slice(args),
                    "bytes.lit" => self.emit_bytes_lit(args),
                    "bytes.view_lit" => self.emit_bytes_view_lit(args),
                    "codec.read_u32_le" => self.emit_codec_read_u32_le(args),
                    "codec.write_u32_le" => self.emit_codec_write_u32_le(args),
                    "vec_u8.with_capacity" => self.emit_vec_u8_with_capacity(args),
                    "vec_u8.len" => self.emit_vec_u8_len(args),
                    "vec_u8.get" => self.emit_vec_u8_get(args),
                    "vec_u8.push" => self.emit_vec_u8_push(args),
                    "vec_u8.reserve_exact" => self.emit_vec_u8_reserve_exact(args),
                    "vec_u8.extend_bytes" => self.emit_vec_u8_extend_bytes(args),
                    "vec_u8.extend_bytes_range" => self.emit_vec_u8_extend_bytes_range(args),
                    "vec_u8.as_view" => self.emit_vec_u8_as_view(args),
                    "vec_u8.into_bytes" => self.emit_vec_u8_into_bytes(args),

                    // Ops.
                    "+" => self.emit_i32_binop(args, Instruction::I32Add),
                    "-" => self.emit_i32_binop(args, Instruction::I32Sub),
                    "*" => self.emit_i32_binop(args, Instruction::I32Mul),
                    "/" => self.emit_i32_binop(args, Instruction::I32DivS),
                    "%" => self.emit_i32_binop(args, Instruction::I32RemS),
                    "&" => self.emit_i32_binop(args, Instruction::I32And),
                    "|" => self.emit_i32_binop(args, Instruction::I32Or),
                    "^" => self.emit_i32_binop(args, Instruction::I32Xor),
                    "=" => self.emit_i32_cmp(args, Instruction::I32Eq),
                    "!=" => self.emit_i32_cmp(args, Instruction::I32Ne),
                    "<" => self.emit_i32_cmp(args, Instruction::I32LtS),
                    "<u" => self.emit_i32_cmp(args, Instruction::I32LtU),
                    "<=u" => self.emit_i32_cmp(args, Instruction::I32LeU),
                    ">=u" => self.emit_i32_cmp(args, Instruction::I32GeU),
                    "<=" => self.emit_i32_cmp(args, Instruction::I32LeS),
                    ">=" => self.emit_i32_cmp(args, Instruction::I32GeS),
                    ">" => self.emit_i32_cmp(args, Instruction::I32GtS),
                    ">u" => self.emit_i32_cmp(args, Instruction::I32GtU),
                    "&&" => self.emit_i32_logic_and(args),
                    "||" => self.emit_i32_logic_or(args),
                    "<<u" => self.emit_i32_binop(args, Instruction::I32Shl),
                    ">>u" => self.emit_i32_binop(args, Instruction::I32ShrU),

                    // Call.
                    _ => {
                        if (head.starts_with("bytes.")
                            || head.starts_with("view.")
                            || head.starts_with("codec.")
                            || head.starts_with("vec_u8."))
                            && !self.module.func_sigs.contains_key(head)
                        {
                            return Err(wasm_unsupported(
                                "builtin",
                                head,
                                required_feature_for_head(head)
                                    .map(|(_, f)| f)
                                    .unwrap_or(WasmFeatureV1::CoreFormsV1),
                                ptr,
                            ));
                        }
                        self.emit_call(head, args)
                    }
                }
            }
        }
    }

    fn emit_begin(&mut self, exprs: &[Expr]) -> Result<Ty, CompilerError> {
        if exprs.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "(begin ...) requires at least 1 expression".to_string(),
            ));
        }
        self.push_scope();
        for e in &exprs[..exprs.len() - 1] {
            let ty = self.emit_expr(e)?;
            self.emit_drop_values(ty)?;
        }
        let ty = self.emit_expr(&exprs[exprs.len() - 1])?;
        self.pop_scope();
        Ok(ty)
    }

    fn emit_let(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "let form: (let <name> <expr>)".to_string(),
            ));
        }
        let name = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "let name must be an identifier".to_string(),
            )
        })?;

        let ty = self.emit_expr(&args[1])?;
        if ty == Ty::Never {
            return Ok(Ty::Never);
        }
        let locals = self.f.new_locals_for_ty(ty)?;
        self.store_stack_to_locals(ty, &locals)?;
        self.bind(
            name.to_string(),
            Binding {
                ty,
                locals: locals.clone(),
            },
        )?;

        self.f.push(Instruction::I32Const(0));
        Ok(Ty::I32)
    }

    fn emit_set(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "set form: (set <name> <expr>)".to_string(),
            ));
        }
        let name = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "set name must be an identifier".to_string(),
            )
        })?;
        let Some(dst) = self.lookup(name) else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("set of unknown variable: {name:?}"),
            ));
        };
        let dst_ty = dst.ty;
        let dst_locals = dst.locals.clone();
        let ty = self.emit_expr(&args[1])?;
        if ty == Ty::Never {
            return Ok(Ty::Never);
        }
        if ty != dst_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("set type mismatch: name={name:?} got={ty:?} want={dst_ty:?}"),
            ));
        }
        self.store_stack_to_locals(ty, &dst_locals)?;
        self.f.push(Instruction::I32Const(0));
        Ok(Ty::I32)
    }

    fn emit_if(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "if form: (if <cond> <then> <else>)".to_string(),
            ));
        }
        let cond_ty = self.emit_expr(&args[0])?;
        if cond_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "if condition must be i32".to_string(),
            ));
        }

        let then_ty = self.infer_ty(&args[1])?;
        let else_ty = self.infer_ty(&args[2])?;
        let out_ty = if then_ty == Ty::Never {
            else_ty
        } else if else_ty == Ty::Never {
            then_ty
        } else if then_ty != else_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("if branch type mismatch: then={then_ty:?} else={else_ty:?}"),
            ));
        } else {
            then_ty
        };

        let out_locals = if out_ty == Ty::Never {
            Vec::new()
        } else {
            self.f.new_locals_for_ty(out_ty)?
        };

        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.push_scope();
        let t = self.emit_expr(&args[1])?;
        if t != Ty::Never {
            if t != out_ty {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("if then branch type mismatch: got={t:?} want={out_ty:?}"),
                ));
            }
            self.store_stack_to_locals(t, &out_locals)?;
        }
        self.pop_scope();
        self.f.push(Instruction::Else);
        self.push_scope();
        let e = self.emit_expr(&args[2])?;
        if e != Ty::Never {
            if e != out_ty {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("if else branch type mismatch: got={e:?} want={out_ty:?}"),
                ));
            }
            self.store_stack_to_locals(e, &out_locals)?;
        }
        self.pop_scope();
        self.f.push(Instruction::End);

        if out_ty == Ty::Never {
            return Ok(Ty::Never);
        }
        self.load_locals_to_stack(&out_locals);
        Ok(out_ty)
    }

    fn emit_for(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 4 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "for form: (for <i> <start> <end> <body>)".to_string(),
            ));
        }
        let var_name = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "for variable must be an identifier".to_string(),
            )
        })?;

        let var_local = if let Some(b) = self.lookup(var_name) {
            if b.ty != Ty::I32 || b.locals.len() != 1 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("for variable must be i32: {var_name:?}"),
                ));
            }
            b.locals[0]
        } else {
            let l = self.f.new_i32_local();
            self.bind(
                var_name.to_string(),
                Binding {
                    ty: Ty::I32,
                    locals: vec![l],
                },
            )?;
            l
        };

        // start -> var
        let start_ty = self.emit_expr(&args[1])?;
        if start_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "for start must be i32".to_string(),
            ));
        }
        self.f.push(Instruction::LocalSet(var_local));

        // end -> local
        let end_local = self.f.new_i32_local();
        let end_ty = self.emit_expr(&args[2])?;
        if end_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "for end must be i32".to_string(),
            ));
        }
        self.f.push(Instruction::LocalSet(end_local));

        self.f
            .push(Instruction::Block(wasm_encoder::BlockType::Empty));
        self.f
            .push(Instruction::Loop(wasm_encoder::BlockType::Empty));

        // break if var >= end
        self.f.push(Instruction::LocalGet(var_local));
        self.f.push(Instruction::LocalGet(end_local));
        self.f.push(Instruction::I32GeU);
        self.f.push(Instruction::BrIf(1));

        self.push_scope();
        let body_ty = self.emit_expr(&args[3])?;
        self.emit_drop_values(body_ty)?;
        self.pop_scope();

        // var += 1
        self.f.push(Instruction::LocalGet(var_local));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(var_local));

        self.f.push(Instruction::Br(0));
        self.f.push(Instruction::End); // loop
        self.f.push(Instruction::End); // block

        self.f.push(Instruction::I32Const(0));
        Ok(Ty::I32)
    }

    fn emit_return(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "return form: (return <expr>)".to_string(),
            ));
        }
        let ty = self.emit_expr(&args[0])?;
        if ty == Ty::Never {
            self.f.push(Instruction::Unreachable);
            return Ok(Ty::Never);
        }
        self.f.push(Instruction::Return);
        Ok(Ty::Never)
    }

    fn emit_i32_lit(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "i32.lit expects 1 arg".to_string(),
            ));
        }
        let ty = self.emit_expr(&args[0])?;
        if ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "i32.lit expects i32".to_string(),
            ));
        }
        Ok(Ty::I32)
    }

    fn emit_view_len(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.len expects 1 arg".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        if v_ty != Ty::BytesView && v_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("view.len expects bytes_view (got {v_ty:?})"),
            ));
        }
        let len_local = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(len_local)); // len
        self.f.push(Instruction::Drop); // ptr
        self.f.push(Instruction::LocalGet(len_local));
        Ok(Ty::I32)
    }

    fn emit_view_get_u8(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.get_u8 expects 2 args".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        if v_ty != Ty::BytesView && v_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("view.get_u8 expects bytes_view (got {v_ty:?})"),
            ));
        }
        let idx_ty = self.emit_expr(&args[1])?;
        if idx_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.get_u8 index must be i32".to_string(),
            ));
        }

        let idx = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        let ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(idx));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(ptr));

        // trap if idx >= len (unsigned)
        self.f.push(Instruction::LocalGet(idx));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32LtU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        self.f.push(Instruction::LocalGet(ptr));
        self.f.push(Instruction::LocalGet(idx));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::I32Load8U(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));
        Ok(Ty::I32)
    }

    fn emit_view_slice(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.slice expects 3 args".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        if v_ty != Ty::BytesView && v_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("view.slice expects bytes_view (got {v_ty:?})"),
            ));
        }
        let start_ty = self.emit_expr(&args[1])?;
        let len_ty = self.emit_expr(&args[2])?;
        if start_ty != Ty::I32 || len_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.slice expects (bytes_view, i32, i32)".to_string(),
            ));
        }

        let slice_len = self.f.new_i32_local();
        let start = self.f.new_i32_local();
        let view_len = self.f.new_i32_local();
        let view_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(slice_len));
        self.f.push(Instruction::LocalSet(start));
        self.f.push(Instruction::LocalSet(view_len));
        self.f.push(Instruction::LocalSet(view_ptr));

        // end = start + slice_len; trap if end > view_len (unsigned)
        let end = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(start));
        self.f.push(Instruction::LocalGet(slice_len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(end));

        self.f.push(Instruction::LocalGet(end));
        self.f.push(Instruction::LocalGet(view_len));
        self.f.push(Instruction::I32LeU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        // ptr = view_ptr + start
        self.f.push(Instruction::LocalGet(view_ptr));
        self.f.push(Instruction::LocalGet(start));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalGet(slice_len));
        Ok(Ty::BytesView)
    }

    fn emit_view_to_bytes(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.to_bytes expects 1 arg".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        if v_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.to_bytes expects bytes_view".to_string(),
            ));
        }

        let len = self.f.new_i32_local();
        let src = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(src));

        let out_ptr = self.f.new_i32_local();

        // if len==0 -> empty bytes
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f
            .push(Instruction::GlobalGet(self.module.heap_base_global));
        self.f.push(Instruction::LocalSet(out_ptr));
        self.f.push(Instruction::Else);

        // alloc(len, 1)
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(out_ptr));

        // memory.copy(out_ptr, src, len)
        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(src));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        self.f.push(Instruction::End);

        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(len));
        Ok(Ty::Bytes)
    }

    fn emit_view_eq(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.eq expects 2 args".to_string(),
            ));
        }
        let a_ty = self.emit_expr(&args[0])?;
        let b_ty = self.emit_expr(&args[1])?;
        if !matches!(a_ty, Ty::BytesView | Ty::Bytes) || !matches!(b_ty, Ty::BytesView | Ty::Bytes)
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("view.eq expects (bytes_view, bytes_view) (got {a_ty:?}, {b_ty:?})"),
            ));
        }

        let b_len = self.f.new_i32_local();
        let b_ptr = self.f.new_i32_local();
        let a_len = self.f.new_i32_local();
        let a_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(b_len));
        self.f.push(Instruction::LocalSet(b_ptr));
        self.f.push(Instruction::LocalSet(a_len));
        self.f.push(Instruction::LocalSet(a_ptr));

        // if a_len != b_len -> return 0
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::LocalGet(b_len));
        self.f.push(Instruction::I32Ne);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(0));
        self.f.push(Instruction::Return);
        self.f.push(Instruction::End);

        // if a_len == 0 -> return 1
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Return);
        self.f.push(Instruction::End);

        let i = self.f.new_i32_local();
        self.f.push(Instruction::I32Const(0));
        self.f.push(Instruction::LocalSet(i));

        self.f
            .push(Instruction::Block(wasm_encoder::BlockType::Empty));
        self.f
            .push(Instruction::Loop(wasm_encoder::BlockType::Empty));

        // if i >= a_len break
        self.f.push(Instruction::LocalGet(i));
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::I32GeU);
        self.f.push(Instruction::BrIf(1));

        // load a[i]
        self.f.push(Instruction::LocalGet(a_ptr));
        self.f.push(Instruction::LocalGet(i));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::I32Load8U(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));

        // load b[i]
        self.f.push(Instruction::LocalGet(b_ptr));
        self.f.push(Instruction::LocalGet(i));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::I32Load8U(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));

        self.f.push(Instruction::I32Ne);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(0));
        self.f.push(Instruction::Return);
        self.f.push(Instruction::End);

        // i += 1
        self.f.push(Instruction::LocalGet(i));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(i));

        self.f.push(Instruction::Br(0));
        self.f.push(Instruction::End); // loop
        self.f.push(Instruction::End); // block

        self.f.push(Instruction::I32Const(1));
        Ok(Ty::I32)
    }

    fn emit_bytes_view(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.view expects 1 arg".to_string(),
            ));
        }
        let b_ty = self.emit_expr(&args[0])?;
        if b_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.view expects bytes".to_string(),
            ));
        }
        Ok(Ty::BytesView)
    }

    fn emit_bytes_subview(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.subview expects 3 args".to_string(),
            ));
        }
        let b_ty = self.emit_expr(&args[0])?;
        if b_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.subview expects bytes".to_string(),
            ));
        }
        let start_ty = self.emit_expr(&args[1])?;
        let len_ty = self.emit_expr(&args[2])?;
        if start_ty != Ty::I32 || len_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.subview expects (bytes, i32, i32)".to_string(),
            ));
        }

        let slice_len = self.f.new_i32_local();
        let start = self.f.new_i32_local();
        let bytes_len = self.f.new_i32_local();
        let bytes_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(slice_len));
        self.f.push(Instruction::LocalSet(start));
        self.f.push(Instruction::LocalSet(bytes_len));
        self.f.push(Instruction::LocalSet(bytes_ptr));

        // end = start + slice_len; trap if end > bytes_len (unsigned)
        let end = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(start));
        self.f.push(Instruction::LocalGet(slice_len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(end));

        self.f.push(Instruction::LocalGet(end));
        self.f.push(Instruction::LocalGet(bytes_len));
        self.f.push(Instruction::I32LeU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        // ptr = bytes_ptr + start
        self.f.push(Instruction::LocalGet(bytes_ptr));
        self.f.push(Instruction::LocalGet(start));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalGet(slice_len));
        Ok(Ty::BytesView)
    }

    fn emit_bytes_len(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        // bytes.len is defined over bytes_view.
        self.emit_view_len(args)
    }

    fn emit_bytes_get_u8(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        // bytes.get_u8 is defined over bytes_view.
        self.emit_view_get_u8(args)
    }

    fn emit_bytes_set_u8(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.set_u8 expects 3 args".to_string(),
            ));
        }
        let b_ty = self.emit_expr(&args[0])?;
        if b_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.set_u8 expects bytes".to_string(),
            ));
        }
        let idx_ty = self.emit_expr(&args[1])?;
        let v_ty = self.emit_expr(&args[2])?;
        if idx_ty != Ty::I32 || v_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
            ));
        }

        let v = self.f.new_i32_local();
        let idx = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        let ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(v));
        self.f.push(Instruction::LocalSet(idx));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(ptr));

        // trap if idx >= len (unsigned)
        self.f.push(Instruction::LocalGet(idx));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32LtU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        // *(ptr+idx) = (v & 0xFF)
        self.f.push(Instruction::LocalGet(ptr));
        self.f.push(Instruction::LocalGet(idx));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalGet(v));
        self.f.push(Instruction::I32Store8(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));

        // return bytes (ptr,len)
        self.f.push(Instruction::LocalGet(ptr));
        self.f.push(Instruction::LocalGet(len));
        Ok(Ty::Bytes)
    }

    fn emit_bytes_alloc(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.alloc expects 1 arg".to_string(),
            ));
        }
        let len_ty = self.emit_expr(&args[0])?;
        if len_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.alloc length must be i32".to_string(),
            ));
        }

        let len = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(len));

        let out_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f
            .push(Instruction::GlobalGet(self.module.heap_base_global));
        self.f.push(Instruction::LocalSet(out_ptr));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(out_ptr));
        self.f.push(Instruction::End);

        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(len));
        Ok(Ty::Bytes)
    }

    fn emit_bytes_concat(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.concat expects 2 args".to_string(),
            ));
        }
        let a_ty = self.emit_expr(&args[0])?;
        let b_ty = self.emit_expr(&args[1])?;
        if a_ty != Ty::Bytes || b_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.concat expects (bytes, bytes)".to_string(),
            ));
        }

        let b_len = self.f.new_i32_local();
        let b_ptr = self.f.new_i32_local();
        let a_len = self.f.new_i32_local();
        let a_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(b_len));
        self.f.push(Instruction::LocalSet(b_ptr));
        self.f.push(Instruction::LocalSet(a_len));
        self.f.push(Instruction::LocalSet(a_ptr));

        let out_len = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::LocalGet(b_len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(out_len));

        let out_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(out_len));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(out_ptr));

        // copy a
        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(a_ptr));
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        // copy b
        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalGet(b_ptr));
        self.f.push(Instruction::LocalGet(b_len));
        self.f.push(Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(out_len));
        Ok(Ty::Bytes)
    }

    fn emit_bytes_eq(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.eq expects 2 args".to_string(),
            ));
        }
        self.emit_view_eq(args)
    }

    fn emit_bytes_cmp_range(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 6 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.cmp_range expects 6 args".to_string(),
            ));
        }
        let a_ty = self.emit_expr(&args[0])?;
        let a_off_ty = self.emit_expr(&args[1])?;
        let a_len_ty = self.emit_expr(&args[2])?;
        let b_ty = self.emit_expr(&args[3])?;
        let b_off_ty = self.emit_expr(&args[4])?;
        let b_len_ty = self.emit_expr(&args[5])?;
        if a_ty != Ty::BytesView
            || b_ty != Ty::BytesView
            || a_off_ty != Ty::I32
            || a_len_ty != Ty::I32
            || b_off_ty != Ty::I32
            || b_len_ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.cmp_range expects (bytes_view, i32, i32, bytes_view, i32, i32)".to_string(),
            ));
        }

        let b_len = self.f.new_i32_local();
        let b_off = self.f.new_i32_local();
        let b_view_len = self.f.new_i32_local();
        let b_ptr = self.f.new_i32_local();
        let a_len = self.f.new_i32_local();
        let a_off = self.f.new_i32_local();
        let a_view_len = self.f.new_i32_local();
        let a_ptr = self.f.new_i32_local();

        self.f.push(Instruction::LocalSet(b_len));
        self.f.push(Instruction::LocalSet(b_off));
        self.f.push(Instruction::LocalSet(b_view_len));
        self.f.push(Instruction::LocalSet(b_ptr));
        self.f.push(Instruction::LocalSet(a_len));
        self.f.push(Instruction::LocalSet(a_off));
        self.f.push(Instruction::LocalSet(a_view_len));
        self.f.push(Instruction::LocalSet(a_ptr));

        // bounds: (a_off + a_len) <= a_view_len and (b_off + b_len) <= b_view_len
        let a_end = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(a_off));
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(a_end));

        self.f.push(Instruction::LocalGet(a_end));
        self.f.push(Instruction::LocalGet(a_view_len));
        self.f.push(Instruction::I32LeU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        let b_end = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(b_off));
        self.f.push(Instruction::LocalGet(b_len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(b_end));

        self.f.push(Instruction::LocalGet(b_end));
        self.f.push(Instruction::LocalGet(b_view_len));
        self.f.push(Instruction::I32LeU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        // min = min(a_len, b_len)
        let min = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::LocalGet(b_len));
        self.f.push(Instruction::I32LtU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::LocalSet(min));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::LocalGet(b_len));
        self.f.push(Instruction::LocalSet(min));
        self.f.push(Instruction::End);

        let i = self.f.new_i32_local();
        self.f.push(Instruction::I32Const(0));
        self.f.push(Instruction::LocalSet(i));

        self.f
            .push(Instruction::Block(wasm_encoder::BlockType::Empty));
        self.f
            .push(Instruction::Loop(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::LocalGet(i));
        self.f.push(Instruction::LocalGet(min));
        self.f.push(Instruction::I32GeU);
        self.f.push(Instruction::BrIf(1));

        // a byte
        self.f.push(Instruction::LocalGet(a_ptr));
        self.f.push(Instruction::LocalGet(a_off));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalGet(i));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::I32Load8U(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));
        let ac = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(ac));

        // b byte
        self.f.push(Instruction::LocalGet(b_ptr));
        self.f.push(Instruction::LocalGet(b_off));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalGet(i));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::I32Load8U(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));
        let bc = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(bc));

        // if ac < bc => return -1
        self.f.push(Instruction::LocalGet(ac));
        self.f.push(Instruction::LocalGet(bc));
        self.f.push(Instruction::I32LtU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(-1));
        self.f.push(Instruction::Return);
        self.f.push(Instruction::End);

        // if bc < ac => return 1
        self.f.push(Instruction::LocalGet(bc));
        self.f.push(Instruction::LocalGet(ac));
        self.f.push(Instruction::I32LtU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Return);
        self.f.push(Instruction::End);

        // i += 1
        self.f.push(Instruction::LocalGet(i));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(i));

        self.f.push(Instruction::Br(0));
        self.f.push(Instruction::End); // loop
        self.f.push(Instruction::End); // block

        // after compare: if a_len < b_len => -1; if a_len > b_len => 1 else 0
        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::LocalGet(b_len));
        self.f.push(Instruction::I32LtU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(-1));
        self.f.push(Instruction::Return);
        self.f.push(Instruction::End);

        self.f.push(Instruction::LocalGet(a_len));
        self.f.push(Instruction::LocalGet(b_len));
        self.f.push(Instruction::I32GtU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Return);
        self.f.push(Instruction::End);

        self.f.push(Instruction::I32Const(0));
        Ok(Ty::I32)
    }

    fn emit_bytes_slice(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.slice expects 3 args".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        let start_ty = self.emit_expr(&args[1])?;
        let len_ty = self.emit_expr(&args[2])?;
        if !matches!(v_ty, Ty::BytesView | Ty::Bytes) || start_ty != Ty::I32 || len_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.slice expects (bytes_view, i32, i32)".to_string(),
            ));
        }

        let slice_len = self.f.new_i32_local();
        let start = self.f.new_i32_local();
        let view_len = self.f.new_i32_local();
        let view_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(slice_len));
        self.f.push(Instruction::LocalSet(start));
        self.f.push(Instruction::LocalSet(view_len));
        self.f.push(Instruction::LocalSet(view_ptr));

        // end = start + slice_len; trap if end > view_len (unsigned)
        let end = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(start));
        self.f.push(Instruction::LocalGet(slice_len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(end));

        self.f.push(Instruction::LocalGet(end));
        self.f.push(Instruction::LocalGet(view_len));
        self.f.push(Instruction::I32LeU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        let out_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(slice_len));
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f
            .push(Instruction::GlobalGet(self.module.heap_base_global));
        self.f.push(Instruction::LocalSet(out_ptr));
        self.f.push(Instruction::Else);

        self.f.push(Instruction::LocalGet(slice_len));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(out_ptr));

        // memory.copy(out_ptr, view_ptr+start, slice_len)
        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(view_ptr));
        self.f.push(Instruction::LocalGet(start));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalGet(slice_len));
        self.f.push(Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        self.f.push(Instruction::End);

        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(slice_len));
        Ok(Ty::Bytes)
    }

    fn emit_bytes_lit(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.lit expects 1 arg".to_string(),
            ));
        }
        let s = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.lit expects a text string".to_string(),
            )
        })?;
        let lit = s.as_bytes();
        let off = self.module.data.intern(lit) as i32;
        let len = i32::try_from(lit.len()).unwrap_or(i32::MAX);

        let len_local = self.f.new_i32_local();
        self.f.push(Instruction::I32Const(len));
        self.f.push(Instruction::LocalSet(len_local));

        let out_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(len_local));
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f
            .push(Instruction::GlobalGet(self.module.heap_base_global));
        self.f.push(Instruction::LocalSet(out_ptr));
        self.f.push(Instruction::Else);

        self.f.push(Instruction::LocalGet(len_local));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(out_ptr));

        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::I32Const(off));
        self.f.push(Instruction::LocalGet(len_local));
        self.f.push(Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        self.f.push(Instruction::End);

        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(len_local));
        Ok(Ty::Bytes)
    }

    fn emit_bytes_view_lit(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.view_lit expects 1 arg".to_string(),
            ));
        }
        let s = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.view_lit expects a text string".to_string(),
            )
        })?;
        let lit = s.as_bytes();
        let off = self.module.data.intern(lit) as i32;
        let len = i32::try_from(lit.len()).unwrap_or(i32::MAX);
        if len == 0 {
            self.f
                .push(Instruction::GlobalGet(self.module.heap_base_global));
            self.f.push(Instruction::I32Const(0));
        } else {
            self.f.push(Instruction::I32Const(off));
            self.f.push(Instruction::I32Const(len));
        }
        Ok(Ty::BytesView)
    }

    fn emit_codec_read_u32_le(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "codec.read_u32_le expects 2 args".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        let off_ty = self.emit_expr(&args[1])?;
        if v_ty != Ty::BytesView || off_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.read_u32_le expects (bytes_view, i32)".to_string(),
            ));
        }

        let off = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        let ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(off));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(ptr));

        // end = off + 4; trap if end > len (unsigned)
        let end = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(off));
        self.f.push(Instruction::I32Const(4));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(end));

        self.f.push(Instruction::LocalGet(end));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32LeU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        self.f.push(Instruction::LocalGet(ptr));
        self.f.push(Instruction::LocalGet(off));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::I32Load(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));
        Ok(Ty::I32)
    }

    fn emit_codec_write_u32_le(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "codec.write_u32_le expects 1 arg".to_string(),
            ));
        }
        let x_ty = self.emit_expr(&args[0])?;
        if x_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.write_u32_le expects i32".to_string(),
            ));
        }

        let x = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(x));

        let out_ptr = self.f.new_i32_local();
        self.f.push(Instruction::I32Const(4));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(out_ptr));

        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::LocalGet(x));
        self.f.push(Instruction::I32Store(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));

        self.f.push(Instruction::LocalGet(out_ptr));
        self.f.push(Instruction::I32Const(4));
        Ok(Ty::Bytes)
    }

    fn emit_vec_u8_with_capacity(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.with_capacity expects 1 arg".to_string(),
            ));
        }
        let cap_ty = self.emit_expr(&args[0])?;
        if cap_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.with_capacity expects i32".to_string(),
            ));
        }

        let cap = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(cap));

        let data = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(cap));
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f
            .push(Instruction::GlobalGet(self.module.heap_base_global));
        self.f.push(Instruction::LocalSet(data));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::LocalGet(cap));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(data));
        self.f.push(Instruction::End);

        // return (data, len=0, cap)
        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::I32Const(0));
        self.f.push(Instruction::LocalGet(cap));
        Ok(Ty::VecU8)
    }

    fn emit_vec_u8_len(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.len expects 1 arg".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        if v_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.len expects vec_u8".to_string(),
            ));
        }
        let cap = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(cap));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::Drop); // data
        self.f.push(Instruction::LocalGet(len));
        Ok(Ty::I32)
    }

    fn emit_vec_u8_get(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.get expects 2 args".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        let idx_ty = self.emit_expr(&args[1])?;
        if v_ty != Ty::VecU8 || idx_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.get expects (vec_u8, i32)".to_string(),
            ));
        }

        let idx = self.f.new_i32_local();
        let cap = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        let data = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(idx));
        self.f.push(Instruction::LocalSet(cap));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(data));

        // trap if idx >= len
        self.f.push(Instruction::LocalGet(idx));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32LtU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(idx));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::I32Load8U(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));
        Ok(Ty::I32)
    }

    fn emit_vec_u8_push(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.push expects 2 args".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        let x_ty = self.emit_expr(&args[1])?;
        if v_ty != Ty::VecU8 || x_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.push expects (vec_u8, i32)".to_string(),
            ));
        }

        let x = self.f.new_i32_local();
        let cap = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        let data = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(x));
        self.f.push(Instruction::LocalSet(cap));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(data));

        // if len == cap, grow
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::LocalGet(cap));
        self.f.push(Instruction::I32Eq);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));

        let new_cap = self.f.new_i32_local();
        // new_cap = (cap == 0) ? 1 : (cap * 2)
        self.f.push(Instruction::LocalGet(cap));
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::LocalSet(new_cap));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::LocalGet(cap));
        self.f.push(Instruction::I32Const(2));
        self.f.push(Instruction::I32Mul);
        self.f.push(Instruction::LocalSet(new_cap));
        self.f.push(Instruction::End);

        let new_data = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(new_cap));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(new_data));

        // copy old data
        self.f.push(Instruction::LocalGet(new_data));
        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        self.f.push(Instruction::LocalGet(new_data));
        self.f.push(Instruction::LocalSet(data));
        self.f.push(Instruction::LocalGet(new_cap));
        self.f.push(Instruction::LocalSet(cap));

        self.f.push(Instruction::End); // if grow

        // store x at data[len]
        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalGet(x));
        self.f.push(Instruction::I32Store8(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));

        // len += 1
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(len));

        // return vec
        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::LocalGet(cap));
        Ok(Ty::VecU8)
    }

    fn emit_vec_u8_reserve_exact(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.reserve_exact expects 2 args".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        let add_ty = self.emit_expr(&args[1])?;
        if v_ty != Ty::VecU8 || add_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.reserve_exact expects (vec_u8, i32)".to_string(),
            ));
        }

        let additional = self.f.new_i32_local();
        let cap = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        let data = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(additional));
        self.f.push(Instruction::LocalSet(cap));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(data));

        // need = len + additional
        let need = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::LocalGet(additional));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(need));

        // if need <= cap: return
        self.f.push(Instruction::LocalGet(need));
        self.f.push(Instruction::LocalGet(cap));
        self.f.push(Instruction::I32LeU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);

        let new_data = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(need));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(new_data));

        self.f.push(Instruction::LocalGet(new_data));
        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        self.f.push(Instruction::LocalGet(new_data));
        self.f.push(Instruction::LocalSet(data));
        self.f.push(Instruction::LocalGet(need));
        self.f.push(Instruction::LocalSet(cap));

        self.f.push(Instruction::End);

        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::LocalGet(cap));
        Ok(Ty::VecU8)
    }

    fn emit_vec_u8_extend_bytes(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.extend_bytes expects 2 args".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        let b_ty = self.emit_expr(&args[1])?;
        if v_ty != Ty::VecU8 || !matches!(b_ty, Ty::BytesView | Ty::Bytes) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "vec_u8.extend_bytes expects (vec_u8, bytes_view) (got {v_ty:?}, {b_ty:?})"
                ),
            ));
        }

        let b_len = self.f.new_i32_local();
        let b_ptr = self.f.new_i32_local();
        let cap = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        let data = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(b_len));
        self.f.push(Instruction::LocalSet(b_ptr));
        self.f.push(Instruction::LocalSet(cap));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(data));

        self.emit_vec_u8_extend_from_locals(data, len, cap, b_ptr, b_len)?;
        Ok(Ty::VecU8)
    }

    fn emit_vec_u8_extend_from_locals(
        &mut self,
        data: u32,
        len: u32,
        cap: u32,
        src_ptr: u32,
        src_len: u32,
    ) -> Result<(), CompilerError> {
        // need = len + src_len
        let need = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::LocalGet(src_len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(need));

        // grow if need > cap
        self.f.push(Instruction::LocalGet(need));
        self.f.push(Instruction::LocalGet(cap));
        self.f.push(Instruction::I32GtU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));

        // new_cap = cap; while new_cap < need: new_cap *= 2
        let new_cap = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(cap));
        self.f.push(Instruction::LocalSet(new_cap));
        self.f.push(Instruction::LocalGet(new_cap));
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::LocalSet(new_cap));
        self.f.push(Instruction::End);

        self.f
            .push(Instruction::Block(wasm_encoder::BlockType::Empty));
        self.f
            .push(Instruction::Loop(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::LocalGet(new_cap));
        self.f.push(Instruction::LocalGet(need));
        self.f.push(Instruction::I32GeU);
        self.f.push(Instruction::BrIf(1));
        self.f.push(Instruction::LocalGet(new_cap));
        self.f.push(Instruction::I32Const(2));
        self.f.push(Instruction::I32Mul);
        self.f.push(Instruction::LocalSet(new_cap));
        self.f.push(Instruction::Br(0));
        self.f.push(Instruction::End);
        self.f.push(Instruction::End);

        let new_data = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(new_cap));
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::Call(self.module.rt_alloc_fn));
        self.f.push(Instruction::LocalSet(new_data));

        self.f.push(Instruction::LocalGet(new_data));
        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        self.f.push(Instruction::LocalGet(new_data));
        self.f.push(Instruction::LocalSet(data));
        self.f.push(Instruction::LocalGet(new_cap));
        self.f.push(Instruction::LocalSet(cap));

        self.f.push(Instruction::End); // if grow

        // copy src bytes at data+len
        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalGet(src_ptr));
        self.f.push(Instruction::LocalGet(src_len));
        self.f.push(Instruction::MemoryCopy {
            src_mem: 0,
            dst_mem: 0,
        });

        self.f.push(Instruction::LocalGet(need));
        self.f.push(Instruction::LocalSet(len));

        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        self.f.push(Instruction::LocalGet(cap));
        Ok(())
    }

    fn emit_vec_u8_extend_bytes_range(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 4 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.extend_bytes_range expects 4 args".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        let b_ty = self.emit_expr(&args[1])?;
        let start_ty = self.emit_expr(&args[2])?;
        let len_ty = self.emit_expr(&args[3])?;
        if v_ty != Ty::VecU8
            || !matches!(b_ty, Ty::BytesView | Ty::Bytes)
            || start_ty != Ty::I32
            || len_ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32, i32) (got {v_ty:?}, {b_ty:?}, {start_ty:?}, {len_ty:?})"
                ),
            ));
        }

        let range_len = self.f.new_i32_local();
        let start = self.f.new_i32_local();
        let b_view_len = self.f.new_i32_local();
        let b_ptr = self.f.new_i32_local();
        let cap = self.f.new_i32_local();
        let v_len = self.f.new_i32_local();
        let data = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(range_len));
        self.f.push(Instruction::LocalSet(start));
        self.f.push(Instruction::LocalSet(b_view_len));
        self.f.push(Instruction::LocalSet(b_ptr));
        self.f.push(Instruction::LocalSet(cap));
        self.f.push(Instruction::LocalSet(v_len));
        self.f.push(Instruction::LocalSet(data));

        // bounds: start + range_len <= b_view_len
        let end = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(start));
        self.f.push(Instruction::LocalGet(range_len));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(end));

        self.f.push(Instruction::LocalGet(end));
        self.f.push(Instruction::LocalGet(b_view_len));
        self.f.push(Instruction::I32LeU);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::Else);
        self.f.push(Instruction::Unreachable);
        self.f.push(Instruction::End);

        // Create a subview (b_ptr+start, range_len) and call extend_bytes.
        let sub_ptr = self.f.new_i32_local();
        self.f.push(Instruction::LocalGet(b_ptr));
        self.f.push(Instruction::LocalGet(start));
        self.f.push(Instruction::I32Add);
        self.f.push(Instruction::LocalSet(sub_ptr));

        self.emit_vec_u8_extend_from_locals(data, v_len, cap, sub_ptr, range_len)?;
        Ok(Ty::VecU8)
    }

    fn emit_vec_u8_as_view(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.as_view expects 1 arg".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        if v_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_view expects vec_u8".to_string(),
            ));
        }
        let cap = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        let data = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(cap));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(data));
        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        Ok(Ty::BytesView)
    }

    fn emit_vec_u8_into_bytes(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.into_bytes expects 1 arg".to_string(),
            ));
        }
        let v_ty = self.emit_expr(&args[0])?;
        if v_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.into_bytes expects vec_u8".to_string(),
            ));
        }
        let cap = self.f.new_i32_local();
        let len = self.f.new_i32_local();
        let data = self.f.new_i32_local();
        self.f.push(Instruction::LocalSet(cap));
        self.f.push(Instruction::LocalSet(len));
        self.f.push(Instruction::LocalSet(data));
        self.f.push(Instruction::LocalGet(data));
        self.f.push(Instruction::LocalGet(len));
        Ok(Ty::Bytes)
    }

    fn emit_i32_binop(
        &mut self,
        args: &[Expr],
        op: Instruction<'static>,
    ) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "binary operator expects 2 args".to_string(),
            ));
        }
        let a_ty = self.emit_expr(&args[0])?;
        let b_ty = self.emit_expr(&args[1])?;
        if a_ty != Ty::I32 || b_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "binary operator expects (i32, i32)".to_string(),
            ));
        }
        self.f.push(op);
        Ok(Ty::I32)
    }

    fn emit_i32_cmp(
        &mut self,
        args: &[Expr],
        op: Instruction<'static>,
    ) -> Result<Ty, CompilerError> {
        self.emit_i32_binop(args, op)
    }

    fn emit_i32_logic_and(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "&& expects 2 args".to_string(),
            ));
        }
        let a_ty = self.emit_expr(&args[0])?;
        if a_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "&& expects (i32, i32)".to_string(),
            ));
        }
        // if a != 0 then evaluate b else 0; normalize to 0/1.
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        self.f.push(Instruction::I32Const(0));
        self.f.push(Instruction::Else);
        let b_ty = self.emit_expr(&args[1])?;
        if b_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "&& expects (i32, i32)".to_string(),
            ));
        }
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::End);
        Ok(Ty::I32)
    }

    fn emit_i32_logic_or(&mut self, args: &[Expr]) -> Result<Ty, CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "|| expects 2 args".to_string(),
            ));
        }
        let a_ty = self.emit_expr(&args[0])?;
        if a_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "|| expects (i32, i32)".to_string(),
            ));
        }
        // if a != 0 then 1 else evaluate b; normalize to 0/1.
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        let b_ty = self.emit_expr(&args[1])?;
        if b_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "|| expects (i32, i32)".to_string(),
            ));
        }
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::I32Eqz);
        self.f.push(Instruction::Else);
        self.f.push(Instruction::I32Const(1));
        self.f.push(Instruction::End);
        Ok(Ty::I32)
    }

    fn emit_call(&mut self, callee: &str, args: &[Expr]) -> Result<Ty, CompilerError> {
        let Some(sig) = self.module.func_sigs.get(callee) else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown callee: {callee:?}"),
            ));
        };
        let want_params = sig.params.clone();
        let ret = sig.ret;
        if args.len() != want_params.len() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!(
                    "wrong arity for call: callee={callee:?} want={} got={}",
                    want_params.len(),
                    args.len()
                ),
            ));
        }
        for (i, (arg, want_ty)) in args.iter().zip(want_params.iter()).enumerate() {
            let got = self.emit_expr(arg)?;
            if got != *want_ty && !(*want_ty == Ty::BytesView && got == Ty::Bytes) {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "call arg type mismatch: callee={callee:?} arg_index={i} got={got:?} want={want_ty:?}"
                    ),
                ));
            }
        }
        let Some(&idx) = self.module.func_indices.get(callee) else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!("internal error: missing function index for {callee:?}"),
            ));
        };
        self.f.push(Instruction::Call(idx));
        Ok(ret)
    }

    fn emit_drop_values(&mut self, ty: Ty) -> Result<(), CompilerError> {
        if ty == Ty::Never {
            return Ok(());
        }
        for _ in 0..flat_len_for_ty(ty)? {
            self.f.push(Instruction::Drop);
        }
        Ok(())
    }

    fn store_stack_to_locals(&mut self, ty: Ty, locals: &[u32]) -> Result<(), CompilerError> {
        let n = flat_len_for_ty(ty)?;
        if locals.len() != n {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: locals arity mismatch".to_string(),
            ));
        }
        for &l in locals.iter().rev() {
            self.f.push(Instruction::LocalSet(l));
        }
        Ok(())
    }

    fn load_locals_to_stack(&mut self, locals: &[u32]) {
        for &l in locals {
            self.f.push(Instruction::LocalGet(l));
        }
    }
}

fn split_head(items: &[Expr]) -> Result<(&str, &[Expr]), CompilerError> {
    let Some((head_expr, args)) = items.split_first() else {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            "empty list expression".to_string(),
        ));
    };
    let head = head_expr.as_ident().ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Parse,
            "head must be an identifier".to_string(),
        )
    })?;
    Ok((head, args))
}

fn required_feature_for_head(head: &str) -> Option<(&'static str, WasmFeatureV1)> {
    match head {
        // Core forms.
        "begin" | "let" | "set" | "set0" | "return" => Some(("form", WasmFeatureV1::CoreFormsV1)),
        "if" | "for" => Some(("form", WasmFeatureV1::ControlFlowV1)),

        // Literals.
        "i32.lit" | "bytes.lit" | "bytes.view_lit" => Some(("builtin", WasmFeatureV1::LiteralsV1)),

        // View builtins.
        "view.to_bytes" => Some(("builtin", WasmFeatureV1::ViewToBytesV1)),
        "view.len" | "view.get_u8" | "view.slice" | "view.eq" => {
            Some(("builtin", WasmFeatureV1::ViewReadV1))
        }

        // Codec builtins.
        "codec.read_u32_le" | "codec.write_u32_le" => {
            Some(("builtin", WasmFeatureV1::CodecU32LeV1))
        }

        // Operators.
        "+" | "-" | "*" | "/" | "%" => Some(("op", WasmFeatureV1::OpsArithV1)),
        "=" | "<u" | "<=u" | ">u" | ">=u" => Some(("op", WasmFeatureV1::OpsCmpV1)),
        "<" | "<=" | ">" | ">=" => Some(("op", WasmFeatureV1::OpsCmpSignedV1)),
        "!=" => Some(("op", WasmFeatureV1::OpsNeqV1)),
        "&" | "|" | "^" => Some(("op", WasmFeatureV1::OpsBitwiseV1)),
        "&&" | "||" => Some(("op", WasmFeatureV1::OpsLogicV1)),
        "<<u" | ">>u" => Some(("op", WasmFeatureV1::OpsShiftV1)),

        // Bytes builtins.
        _ if head.starts_with("bytes.") || head.starts_with("vec_u8.") => {
            Some(("builtin", WasmFeatureV1::BytesBuiltinsV1))
        }
        _ if head.starts_with("view.") => Some(("builtin", WasmFeatureV1::ViewReadV1)),
        _ if head.starts_with("codec.") => Some(("builtin", WasmFeatureV1::CodecU32LeV1)),
        _ => None,
    }
}

fn wasm_unsupported(kind: &str, name: &str, requires: WasmFeatureV1, ptr: &str) -> CompilerError {
    let message = format!(
        "wasm backend unsupported {kind}: {name} (requires feature {})",
        requires.as_str()
    );

    let mut data = BTreeMap::new();
    data.insert("kind".to_string(), Value::String(kind.to_string()));
    data.insert("name".to_string(), Value::String(name.to_string()));
    data.insert(
        "requires_feature".to_string(),
        Value::String(requires.as_str().to_string()),
    );

    let loc = if ptr.trim().is_empty() {
        None
    } else {
        Some(Location::X07Ast {
            ptr: ptr.to_string(),
        })
    };

    let diagnostic = match kind {
        "form" => Diagnostic {
            code: "X07C_WASM_BACKEND_UNSUPPORTED_FORM".to_string(),
            severity: Severity::Error,
            stage: Stage::Codegen,
            message: message.clone(),
            loc: loc.clone(),
            notes: Vec::new(),
            related: Vec::new(),
            data: data.clone(),
            quickfix: None,
        },
        "builtin" => Diagnostic {
            code: "X07C_WASM_BACKEND_UNSUPPORTED_BUILTIN".to_string(),
            severity: Severity::Error,
            stage: Stage::Codegen,
            message: message.clone(),
            loc: loc.clone(),
            notes: Vec::new(),
            related: Vec::new(),
            data: data.clone(),
            quickfix: None,
        },
        "op" => Diagnostic {
            code: "X07C_WASM_BACKEND_UNSUPPORTED_OP".to_string(),
            severity: Severity::Error,
            stage: Stage::Codegen,
            message: message.clone(),
            loc: loc.clone(),
            notes: Vec::new(),
            related: Vec::new(),
            data: data.clone(),
            quickfix: None,
        },
        "type" => Diagnostic {
            code: "X07C_WASM_BACKEND_UNSUPPORTED_TYPE".to_string(),
            severity: Severity::Error,
            stage: Stage::Codegen,
            message: message.clone(),
            loc: loc.clone(),
            notes: Vec::new(),
            related: Vec::new(),
            data: data.clone(),
            quickfix: None,
        },
        "feature" => Diagnostic {
            code: "X07C_WASM_BACKEND_FEATURE_REQUIRED".to_string(),
            severity: Severity::Error,
            stage: Stage::Codegen,
            message: message.clone(),
            loc: loc.clone(),
            notes: Vec::new(),
            related: Vec::new(),
            data: data.clone(),
            quickfix: None,
        },
        _ => Diagnostic {
            code: "X07C_WASM_BACKEND_FEATURE_REQUIRED".to_string(),
            severity: Severity::Error,
            stage: Stage::Codegen,
            message: message.clone(),
            loc: loc.clone(),
            notes: Vec::new(),
            related: Vec::new(),
            data: data.clone(),
            quickfix: None,
        },
    };

    CompilerError::with_diagnostic(CompileErrorKind::Unsupported, message.clone(), diagnostic)
}

fn flat_len_for_ty(ty: Ty) -> Result<usize, CompilerError> {
    match ty {
        Ty::I32 => Ok(1),
        Ty::Bytes | Ty::BytesView => Ok(2),
        Ty::VecU8 => Ok(3),
        Ty::Never => Ok(0),
        other => Err(wasm_unsupported(
            "type",
            &format!("{other:?}"),
            WasmFeatureV1::CoreFormsV1,
            "",
        )),
    }
}

fn encode_locals(locals: &[ValType]) -> Vec<(u32, ValType)> {
    if locals.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(u32, ValType)> = Vec::new();
    let mut cur = locals[0];
    let mut n: u32 = 1;
    for &t in &locals[1..] {
        if t == cur {
            n += 1;
        } else {
            out.push((n, cur));
            cur = t;
            n = 1;
        }
    }
    out.push((n, cur));
    out
}

fn validate_mem_limits(wasm_opts: &WasmEmitOptions) -> Result<(u32, u32), CompilerError> {
    let initial_pages = layout::bytes_to_pages_exact(wasm_opts.mem.initial_memory_bytes)
        .ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                format!(
                    "wasm initial_memory_bytes must be a multiple of {} (got {})",
                    layout::WASM_PAGE_SIZE_BYTES,
                    wasm_opts.mem.initial_memory_bytes
                ),
            )
        })?;

    let max_pages =
        layout::bytes_to_pages_exact(wasm_opts.mem.max_memory_bytes).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                format!(
                    "wasm max_memory_bytes must be a multiple of {} (got {})",
                    layout::WASM_PAGE_SIZE_BYTES,
                    wasm_opts.mem.max_memory_bytes
                ),
            )
        })?;

    if max_pages < initial_pages {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "wasm max_memory_bytes must be >= initial_memory_bytes (initial={} max={})",
                wasm_opts.mem.initial_memory_bytes, wasm_opts.mem.max_memory_bytes
            ),
        ));
    }
    if wasm_opts.mem.no_growable_memory && max_pages != initial_pages {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "wasm-no-growable-memory requires max_memory_bytes == initial_memory_bytes (initial={} max={})",
                wasm_opts.mem.initial_memory_bytes, wasm_opts.mem.max_memory_bytes
            ),
        ));
    }

    Ok((initial_pages, max_pages))
}

pub(super) fn emit_solve_pure_wasm_v1(
    program: &Program,
    options: &CompileOptions,
    wasm_opts: &WasmEmitOptions,
) -> Result<Vec<u8>, CompilerError> {
    let (initial_pages, max_pages) = validate_mem_limits(wasm_opts)?;

    if !options.freestanding {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            "wasm backend currently requires freestanding=true".to_string(),
        ));
    }
    if options.world != x07_worlds::WorldId::SolvePure {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            "wasm backend currently supports only --world solve-pure".to_string(),
        ));
    }
    if !program.async_functions.is_empty() || !program.extern_functions.is_empty() {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            "wasm backend does not yet support async or extern functions".to_string(),
        ));
    }

    // Assign function indices.
    let mut func_indices: BTreeMap<String, u32> = BTreeMap::new();
    let mut func_sigs: BTreeMap<String, FuncSig> = BTreeMap::new();

    let mut next_fn: u32 = 0;
    let rt_alloc_fn = next_fn;
    next_fn += 1;

    for f in &program.functions {
        func_indices.insert(f.name.clone(), next_fn);
        func_sigs.insert(
            f.name.clone(),
            FuncSig {
                params: f.params.iter().map(|p| p.ty).collect(),
                ret: f.ret_ty,
            },
        );
        next_fn += 1;
    }

    let solve_fn = next_fn;
    next_fn += 1;
    let x07_solve_v2_fn = next_fn;

    // Build module context.
    let mut module_ctx = ModuleCtx {
        func_indices,
        func_sigs,
        features: wasm_opts.features.clone(),
        data: DataBuilder::default(),
        rt_alloc_fn,
        solve_fn,
        heap_base_global: 2,
        heap_ptr_global: 3,
        heap_end_global: 4,
    };

    // Compile functions.
    let mut func_bodies: Vec<FuncCode> = Vec::new();

    // 1) rt_alloc(size, align) -> ptr
    {
        let mut f = FuncCode::new(2);
        let size = 0u32;
        let align = 1u32;

        let out_ptr = f.new_i32_local();
        let cur = f.new_i32_local();
        let mask = f.new_i32_local();
        let aligned = f.new_i32_local();
        let new_ptr = f.new_i32_local();

        // if size == 0 => out_ptr = heap_base
        f.push(Instruction::LocalGet(size));
        f.push(Instruction::I32Eqz);
        f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        f.push(Instruction::GlobalGet(module_ctx.heap_base_global));
        f.push(Instruction::LocalSet(out_ptr));
        f.push(Instruction::Else);

        // cur = heap_ptr
        f.push(Instruction::GlobalGet(module_ctx.heap_ptr_global));
        f.push(Instruction::LocalSet(cur));

        // mask = align - 1
        f.push(Instruction::LocalGet(align));
        f.push(Instruction::I32Const(1));
        f.push(Instruction::I32Sub);
        f.push(Instruction::LocalSet(mask));

        // aligned = (cur + mask) & ~mask
        f.push(Instruction::LocalGet(cur));
        f.push(Instruction::LocalGet(mask));
        f.push(Instruction::I32Add);
        f.push(Instruction::LocalGet(mask));
        f.push(Instruction::I32Const(-1));
        f.push(Instruction::I32Xor);
        f.push(Instruction::I32And);
        f.push(Instruction::LocalSet(aligned));

        // new_ptr = aligned + size
        f.push(Instruction::LocalGet(aligned));
        f.push(Instruction::LocalGet(size));
        f.push(Instruction::I32Add);
        f.push(Instruction::LocalSet(new_ptr));

        // trap if new_ptr > heap_end (unsigned)
        f.push(Instruction::LocalGet(new_ptr));
        f.push(Instruction::GlobalGet(module_ctx.heap_end_global));
        f.push(Instruction::I32LeU);
        f.push(Instruction::If(wasm_encoder::BlockType::Empty));
        f.push(Instruction::Else);
        f.push(Instruction::Unreachable);
        f.push(Instruction::End);

        // heap_ptr = new_ptr
        f.push(Instruction::LocalGet(new_ptr));
        f.push(Instruction::GlobalSet(module_ctx.heap_ptr_global));

        // memory.fill(aligned, 0, size)
        f.push(Instruction::LocalGet(aligned));
        f.push(Instruction::I32Const(0));
        f.push(Instruction::LocalGet(size));
        f.push(Instruction::MemoryFill(0));

        f.push(Instruction::LocalGet(aligned));
        f.push(Instruction::LocalSet(out_ptr));

        f.push(Instruction::End);

        f.push(Instruction::LocalGet(out_ptr));
        f.push(Instruction::End);
        func_bodies.push(f);
    }

    // 2) user functions
    for def in &program.functions {
        func_bodies.push(compile_defn(&mut module_ctx, def)?);
    }

    // 3) __x07_solve(input_ptr, input_len) -> bytes
    {
        let mut f = FuncCode::new(2);
        let mut e = ExprEmitter::new(&mut module_ctx, &mut f);
        e.bind(
            "input".to_string(),
            Binding {
                ty: Ty::BytesView,
                locals: vec![0, 1],
            },
        )?;
        let ty = e.emit_expr(&program.solve)?;
        if ty != Ty::Bytes && ty != Ty::Never {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("solve expression must return bytes (got {ty:?})"),
            ));
        }
        f.push(Instruction::End);
        func_bodies.push(f);
    }

    // 4) x07_solve_v2 wrapper
    {
        let mut f = FuncCode::new(5);
        let retptr = 0u32;
        let arena_ptr = 1u32;
        let arena_cap = 2u32;
        let input_ptr = 3u32;
        let input_len = 4u32;

        // heap_base = arena_ptr
        f.push(Instruction::LocalGet(arena_ptr));
        f.push(Instruction::GlobalSet(module_ctx.heap_base_global));
        // heap_ptr = arena_ptr
        f.push(Instruction::LocalGet(arena_ptr));
        f.push(Instruction::GlobalSet(module_ctx.heap_ptr_global));
        // heap_end = arena_ptr + arena_cap
        f.push(Instruction::LocalGet(arena_ptr));
        f.push(Instruction::LocalGet(arena_cap));
        f.push(Instruction::I32Add);
        f.push(Instruction::GlobalSet(module_ctx.heap_end_global));

        // call __x07_solve(input_ptr, input_len) -> (out_ptr, out_len)
        f.push(Instruction::LocalGet(input_ptr));
        f.push(Instruction::LocalGet(input_len));
        f.push(Instruction::Call(module_ctx.solve_fn));

        let out_len = f.new_i32_local();
        let out_ptr = f.new_i32_local();
        f.push(Instruction::LocalSet(out_len));
        f.push(Instruction::LocalSet(out_ptr));

        // *(retptr+0) = out_ptr
        f.push(Instruction::LocalGet(retptr));
        f.push(Instruction::LocalGet(out_ptr));
        f.push(Instruction::I32Store(wasm_encoder::MemArg {
            offset: 0,
            align: 2,
            memory_index: 0,
        }));

        // *(retptr+4) = out_len
        f.push(Instruction::LocalGet(retptr));
        f.push(Instruction::LocalGet(out_len));
        f.push(Instruction::I32Store(wasm_encoder::MemArg {
            offset: 4,
            align: 2,
            memory_index: 0,
        }));

        f.push(Instruction::End);
        func_bodies.push(f);
    }

    // Finalize data layout.
    let mut data_bytes = module_ctx.data.bytes;
    while !(data_bytes.len() as u32).is_multiple_of(16) {
        data_bytes.push(0);
    }
    let data_end = data_bytes.len() as u32;

    // Build wasm sections.
    let mut types = TypeSection::new();
    let mut funcs = FunctionSection::new();
    let mut code = CodeSection::new();

    // type + function section entries in func index order.
    for (idx, func) in func_bodies.iter().enumerate() {
        let idx = idx as u32;
        let (params, results) = match idx {
            0 => (vec![ValType::I32, ValType::I32], vec![ValType::I32]),
            _ => {
                // Infer from func_sigs or wrapper/solve.
                if idx == module_ctx.solve_fn {
                    (
                        vec![ValType::I32, ValType::I32],
                        vec![ValType::I32, ValType::I32],
                    )
                } else if idx == x07_solve_v2_fn {
                    (
                        vec![
                            ValType::I32,
                            ValType::I32,
                            ValType::I32,
                            ValType::I32,
                            ValType::I32,
                        ],
                        vec![],
                    )
                } else {
                    let name = module_ctx
                        .func_indices
                        .iter()
                        .find_map(|(k, v)| if *v == idx { Some(k.as_str()) } else { None })
                        .ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Internal,
                                format!("internal error: missing func name for idx={idx}"),
                            )
                        })?;
                    let sig = module_ctx.func_sigs.get(name).unwrap();
                    let params = flatten_valtypes(&sig.params)?;
                    let results = flatten_valtypes(&[sig.ret])?;
                    (params, results)
                }
            }
        };

        let ty_idx = types.len();
        types.ty().function(params.into_iter(), results.into_iter());
        funcs.function(ty_idx);

        let locals = encode_locals(&func.locals);
        let mut wf = Function::new(locals);
        for instr in &func.body {
            wf.instruction(instr);
        }
        code.function(&wf);
    }

    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: initial_pages as u64,
        maximum: Some(max_pages as u64),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut globals = GlobalSection::new();
    let data_end_global = globals.len();
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: false,
            shared: false,
        },
        &ConstExpr::i32_const(data_end as i32),
    );
    let heap_base_export_global = globals.len();
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: false,
            shared: false,
        },
        &ConstExpr::i32_const(data_end as i32),
    );

    // Mutable heap globals (base/ptr/end).
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        },
        &ConstExpr::i32_const(data_end as i32),
    );
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        },
        &ConstExpr::i32_const(data_end as i32),
    );
    globals.global(
        GlobalType {
            val_type: ValType::I32,
            mutable: true,
            shared: false,
        },
        &ConstExpr::i32_const(data_end as i32),
    );

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("x07_solve_v2", ExportKind::Func, x07_solve_v2_fn);
    exports.export("__data_end", ExportKind::Global, data_end_global);
    exports.export("__heap_base", ExportKind::Global, heap_base_export_global);

    let mut module = Module::new();
    module.section(&types);
    module.section(&funcs);
    module.section(&memories);
    module.section(&globals);
    module.section(&exports);
    module.section(&code);

    if !data_bytes.is_empty() {
        let mut data = wasm_encoder::DataSection::new();
        data.active(0, &ConstExpr::i32_const(0), data_bytes);
        module.section(&data);
    }

    Ok(module.finish())
}

fn flatten_valtypes(tys: &[Ty]) -> Result<Vec<ValType>, CompilerError> {
    let mut out: Vec<ValType> = Vec::new();
    for ty in tys {
        match ty {
            Ty::I32 => out.push(ValType::I32),
            Ty::Bytes | Ty::BytesView => {
                out.push(ValType::I32);
                out.push(ValType::I32);
            }
            Ty::VecU8 => {
                out.push(ValType::I32);
                out.push(ValType::I32);
                out.push(ValType::I32);
            }
            Ty::Never => {}
            other => {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!("wasm backend: unsupported type in signature: {other:?}"),
                ))
            }
        }
    }
    Ok(out)
}

fn compile_defn(module_ctx: &mut ModuleCtx, def: &FunctionDef) -> Result<FuncCode, CompilerError> {
    let params_flat_len =
        flatten_valtypes(&def.params.iter().map(|p| p.ty).collect::<Vec<_>>())?.len() as u32;
    let mut f = FuncCode::new(params_flat_len);
    let mut e = ExprEmitter::new(module_ctx, &mut f);

    // Bind params.
    let mut cur_local: u32 = 0;
    for p in &def.params {
        let locals = match p.ty {
            Ty::I32 => {
                let l = cur_local;
                cur_local += 1;
                vec![l]
            }
            Ty::Bytes | Ty::BytesView => {
                let a = cur_local;
                let b = cur_local + 1;
                cur_local += 2;
                vec![a, b]
            }
            Ty::VecU8 => {
                let a = cur_local;
                let b = cur_local + 1;
                let c = cur_local + 2;
                cur_local += 3;
                vec![a, b, c]
            }
            other => {
                return Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!("wasm backend: unsupported param type: {other:?}"),
                ))
            }
        };
        e.bind(p.name.clone(), Binding { ty: p.ty, locals })?;
    }

    let ty = e.emit_expr(&def.body)?;
    if ty != def.ret_ty && ty != Ty::Never {
        return Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "function return type mismatch: fn={:?} got={ty:?} want={:?}",
                def.name, def.ret_ty
            ),
        ));
    }
    f.push(Instruction::End);
    Ok(f)
}
