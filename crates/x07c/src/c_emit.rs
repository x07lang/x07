use std::collections::{BTreeMap, BTreeSet};

use crate::ast::Expr;
use crate::compile::{CompileErrorKind, CompileOptions, CompilerError};
use crate::language;
use crate::native;
use crate::native::NativeBackendReq;
use crate::program::{AsyncFunctionDef, ExternFunctionDecl, FunctionDef, FunctionParam, Program};
use crate::types::Ty;

#[derive(Debug, Clone)]
struct VarRef {
    ty: Ty,
    c_name: String,
    moved: bool,
    borrow_count: u32,
    // For `bytes_view` values that borrow from an owned buffer, this is the C local name of the
    // owner (`bytes_t` or `vec_u8_t`) whose backing allocation must outlive the view.
    borrow_of: Option<String>,
    // Temporaries participate in scope cleanup (drops / borrow releases).
    is_temp: bool,
}

#[derive(Debug, Clone)]
struct AsyncVarRef {
    ty: Ty,
    c_name: String,
    moved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ViewBorrowFrom {
    Runtime,
    Param(usize),
    LocalOwned(String),
}

#[derive(Debug, Default)]
struct ViewBorrowEnv {
    scopes: Vec<BTreeMap<String, ViewBorrowFrom>>,
}

impl ViewBorrowEnv {
    fn new() -> Self {
        Self {
            scopes: vec![BTreeMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind(&mut self, name: String, src: ViewBorrowFrom) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, src);
        }
    }

    fn lookup(&self, name: &str) -> Option<ViewBorrowFrom> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }
}

#[derive(Debug, Default)]
struct ViewBorrowCollector {
    src: Option<ViewBorrowFrom>,
}

impl ViewBorrowCollector {
    fn merge(&mut self, fn_name: &str, new_src: ViewBorrowFrom) -> Result<(), CompilerError> {
        self.src = match self.src.take() {
            None => Some(new_src),
            Some(old_src) => Some(merge_view_borrow_from(fn_name, old_src, new_src)?),
        };
        Ok(())
    }
}

fn merge_view_borrow_from(
    fn_name: &str,
    a: ViewBorrowFrom,
    b: ViewBorrowFrom,
) -> Result<ViewBorrowFrom, CompilerError> {
    if a == b {
        return Ok(a);
    }
    match (a, b) {
        (ViewBorrowFrom::Runtime, other) | (other, ViewBorrowFrom::Runtime) => Ok(other),
        (ViewBorrowFrom::Param(a), ViewBorrowFrom::Param(b)) => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "function {fn_name:?} returns bytes_view that can borrow from multiple params ({a} vs {b})"
            ),
        )),
        (ViewBorrowFrom::LocalOwned(a), ViewBorrowFrom::LocalOwned(b)) => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "function {fn_name:?} returns bytes_view that can borrow from multiple locals ({a:?} vs {b:?})"
            ),
        )),
        (ViewBorrowFrom::Param(a), ViewBorrowFrom::LocalOwned(b))
        | (ViewBorrowFrom::LocalOwned(b), ViewBorrowFrom::Param(a)) => Err(CompilerError::new(
            CompileErrorKind::Typing,
            format!(
                "function {fn_name:?} returns bytes_view that can borrow from param {a} or local {b:?}"
            ),
        )),
    }
}

fn c_escape_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7E => out.push(b as char),
            _ => out.push_str(&format!("\\x{:02x}", b)),
        }
    }
    out
}

fn is_owned_ty(ty: Ty) -> bool {
    matches!(
        ty,
        Ty::Bytes | Ty::VecU8 | Ty::OptionBytes | Ty::ResultBytes
    )
}

fn expr_uses_head(expr: &Expr, head: &str) -> bool {
    match expr {
        Expr::Int(_) | Expr::Ident(_) => false,
        Expr::List(items) => {
            if let Some(Expr::Ident(h)) = items.first() {
                if h == head {
                    return true;
                }
            }
            items.iter().any(|e| expr_uses_head(e, head))
        }
    }
}

fn program_uses_head(program: &Program, head: &str) -> bool {
    if expr_uses_head(&program.solve, head) {
        return true;
    }
    for f in &program.functions {
        if expr_uses_head(&f.body, head) {
            return true;
        }
    }
    for f in &program.async_functions {
        if expr_uses_head(&f.body, head) {
            return true;
        }
    }
    false
}

fn trim_preamble_section(
    src: &str,
    start: &str,
    end: &str,
    label: &str,
) -> Result<String, CompilerError> {
    let (head, rest) = src.split_once(start).ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: runtime preamble missing {label} start marker"),
        )
    })?;
    let (_, tail) = rest.split_once(end).ok_or_else(|| {
        CompilerError::new(
            CompileErrorKind::Internal,
            format!("internal error: runtime preamble missing {label} end marker"),
        )
    })?;
    let mut out = String::with_capacity(head.len() + end.len() + tail.len());
    out.push_str(head);
    out.push_str(end);
    out.push_str(tail);
    Ok(out)
}

pub fn emit_c(expr: &Expr, options: &CompileOptions) -> Result<String, CompilerError> {
    let program = Program {
        functions: Vec::new(),
        async_functions: Vec::new(),
        extern_functions: Vec::new(),
        solve: expr.clone(),
    };
    emit_c_program(&program, options)
}

pub fn emit_c_program(
    program: &Program,
    options: &CompileOptions,
) -> Result<String, CompilerError> {
    emit_c_program_with_native_requires(program, options).map(|(src, _)| src)
}

pub fn emit_c_program_with_native_requires(
    program: &Program,
    options: &CompileOptions,
) -> Result<(String, Vec<NativeBackendReq>), CompilerError> {
    let mut emitter = Emitter::new(program, options.clone());
    emitter.emit_program().map_err(|mut e| {
        if let Some(name) = &emitter.current_fn_name {
            if !e.message.contains("(fn=") {
                e.message = format!("{} (fn={name})", e.message);
            }
        }
        e
    })?;
    let native_requires = emitter.native_requires();
    Ok((emitter.out, native_requires))
}

pub fn check_c_program(program: &Program, options: &CompileOptions) -> Result<(), CompilerError> {
    let mut emitter = Emitter::new(program, options.clone());
    emitter.suppress_output = true;
    emitter.check_program().map_err(|mut e| {
        if let Some(name) = &emitter.current_fn_name {
            if !e.message.contains("(fn=") {
                e.message = format!("{} (fn={name})", e.message);
            }
        }
        e
    })
}

pub fn emit_c_header(options: &CompileOptions) -> Result<String, CompilerError> {
    if options.emit_main {
        return Err(CompilerError::new(
            CompileErrorKind::Unsupported,
            "C header emission requires emit_main=false".to_string(),
        ));
    }
    Ok(RUNTIME_C_HEADER.to_string())
}

struct Emitter<'a> {
    program: &'a Program,
    options: CompileOptions,
    out: String,
    suppress_output: bool,
    indent: usize,
    tmp_counter: u32,
    local_count: usize,
    scopes: Vec<BTreeMap<String, VarRef>>,
    fn_c_names: BTreeMap<String, String>,
    async_fn_new_names: BTreeMap<String, String>,
    extern_functions: BTreeMap<String, ExternFunctionDecl>,
    fn_view_return_arg: BTreeMap<String, Option<usize>>,
    fn_ret_ty: Ty,
    allow_async_ops: bool,
    unsafe_depth: usize,
    current_fn_name: Option<String>,
    native_requires: BTreeMap<String, NativeReqAcc>,
}

#[derive(Debug, Clone)]
struct NativeReqAcc {
    abi_major: u32,
    features: BTreeSet<String>,
}

impl<'a> Emitter<'a> {
    fn new(program: &'a Program, options: CompileOptions) -> Self {
        let mut fn_c_names = BTreeMap::new();
        for f in &program.functions {
            fn_c_names.insert(f.name.clone(), c_user_fn_name(&f.name));
        }
        let mut async_fn_new_names = BTreeMap::new();
        for f in &program.async_functions {
            async_fn_new_names.insert(f.name.clone(), c_async_new_name(&f.name));
        }
        let extern_functions = program
            .extern_functions
            .iter()
            .map(|d| (d.name.clone(), d.clone()))
            .collect::<BTreeMap<_, _>>();
        Self {
            program,
            options,
            out: String::new(),
            suppress_output: false,
            indent: 0,
            tmp_counter: 0,
            local_count: 0,
            scopes: vec![BTreeMap::new()],
            fn_c_names,
            async_fn_new_names,
            extern_functions,
            fn_view_return_arg: BTreeMap::new(),
            fn_ret_ty: Ty::Bytes,
            allow_async_ops: false,
            unsafe_depth: 0,
            current_fn_name: None,
            native_requires: BTreeMap::new(),
        }
    }

    fn require_native_backend(
        &mut self,
        backend_id: &str,
        abi_major: u32,
        feature: &str,
    ) -> Result<(), CompilerError> {
        let mismatch = {
            let entry = self
                .native_requires
                .entry(backend_id.to_string())
                .or_insert_with(|| NativeReqAcc {
                    abi_major,
                    features: BTreeSet::new(),
                });

            if entry.abi_major != abi_major {
                Some(entry.abi_major)
            } else {
                entry.features.insert(feature.to_string());
                None
            }
        };

        if let Some(expected) = mismatch {
            return Err(self.err(
                CompileErrorKind::Internal,
                format!("native backend ABI mismatch for {backend_id}: got {abi_major} expected {expected}"),
            ));
        }

        Ok(())
    }

    fn native_requires(&self) -> Vec<NativeBackendReq> {
        self.native_requires
            .iter()
            .map(|(backend_id, acc)| NativeBackendReq {
                backend_id: backend_id.clone(),
                abi_major: acc.abi_major,
                features: acc.features.iter().cloned().collect(),
            })
            .collect()
    }

    fn push_str(&mut self, s: &str) {
        if self.suppress_output {
            return;
        }
        self.out.push_str(s);
    }

    fn push_char(&mut self, c: char) {
        if self.suppress_output {
            return;
        }
        self.out.push(c);
    }

    fn make_var_ref(&self, ty: Ty, c_name: String, is_temp: bool) -> VarRef {
        VarRef {
            ty,
            c_name,
            moved: false,
            borrow_count: 0,
            borrow_of: None,
            is_temp,
        }
    }

    fn err(&self, kind: CompileErrorKind, message: String) -> CompilerError {
        match &self.current_fn_name {
            Some(name) => CompilerError::new(kind, format!("{message} (fn={name})")),
            None => CompilerError::new(kind, message),
        }
    }

    fn lookup_mut_by_c_name(&mut self, c_name: &str) -> Option<&mut VarRef> {
        for scope in self.scopes.iter_mut().rev() {
            for v in scope.values_mut() {
                if v.c_name == c_name {
                    return Some(v);
                }
            }
        }
        None
    }

    fn inc_borrow_count(&mut self, owner_c_name: &str) -> Result<(), CompilerError> {
        let Some(owner) = self.lookup_mut_by_c_name(owner_c_name) else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("borrow of unknown owner: {:?}", owner_c_name),
            ));
        };
        owner.borrow_count = owner.borrow_count.saturating_add(1);
        Ok(())
    }

    fn dec_borrow_count(&mut self, owner_c_name: &str) -> Result<(), CompilerError> {
        let Some(owner) = self.lookup_mut_by_c_name(owner_c_name) else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("borrow release of unknown owner: {:?}", owner_c_name),
            ));
        };
        if owner.borrow_count == 0 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("borrow underflow for owner: {:?}", owner_c_name),
            ));
        }
        owner.borrow_count -= 1;
        Ok(())
    }

    fn release_temp_view_borrow(&mut self, view: &VarRef) -> Result<(), CompilerError> {
        if view.ty != Ty::BytesView || !view.is_temp {
            return Ok(());
        }
        let Some(owner) = &view.borrow_of else {
            return Ok(());
        };
        self.dec_borrow_count(owner)?;
        if let Some(tmp) = self.lookup_mut_by_c_name(&view.c_name) {
            tmp.borrow_of = None;
        }
        Ok(())
    }

    fn emit_drop_var(&mut self, ty: Ty, c_name: &str) {
        match ty {
            Ty::Bytes => self.line(&format!("rt_bytes_drop(ctx, &{c_name});")),
            Ty::VecU8 => self.line(&format!("rt_vec_u8_drop(ctx, &{c_name});")),
            Ty::OptionBytes => {
                self.line(&format!("if ({c_name}.tag) {{"));
                self.indent += 1;
                self.line(&format!("rt_bytes_drop(ctx, &{c_name}.payload);"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{c_name}.tag = UINT32_C(0);"));
            }
            Ty::ResultBytes => {
                self.line(&format!("if ({c_name}.tag) {{"));
                self.indent += 1;
                self.line(&format!("rt_bytes_drop(ctx, &{c_name}.payload.ok);"));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{c_name}.tag = UINT32_C(0);"));
                self.line(&format!("{c_name}.payload.err = UINT32_C(0);"));
            }
            _ => {}
        }
    }

    fn borrow_of_view_expr(&self, expr: &Expr) -> Result<Option<String>, CompilerError> {
        match expr {
            Expr::Ident(name) => {
                if name == "input" {
                    return Ok(None);
                }
                let Some(v) = self.lookup(name) else {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                if v.ty != Ty::BytesView {
                    return Err(self.err(
                        CompileErrorKind::Typing,
                        format!("expected bytes_view, got {:?} for {name:?}", v.ty),
                    ));
                }
                Ok(v.borrow_of.clone())
            }
            Expr::List(items) => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];
                match head {
                    "begin" | "unsafe" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("({head} ...) requires at least 1 expression"),
                            ));
                        }
                        self.borrow_of_view_expr(&args[args.len() - 1])
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        let t = self.borrow_of_view_expr(&args[1])?;
                        let e = self.borrow_of_view_expr(&args[2])?;
                        match (t, e) {
                            (Some(a), Some(b)) => {
                                if a != b {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "bytes_view must have a single borrow source across branches"
                                            .to_string(),
                                    ));
                                }
                                Ok(Some(a))
                            }
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "bytes.view" | "bytes.subview" => {
                        let Some(owner_name) = args.first().and_then(Expr::as_ident) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("{head} requires an identifier owner"),
                            ));
                        };
                        let Some(owner) = self.lookup(owner_name) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {owner_name:?}"),
                            ));
                        };
                        if owner.ty != Ty::Bytes {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("{head} expects bytes owner"),
                            ));
                        }
                        Ok(Some(owner.c_name.clone()))
                    }
                    "vec_u8.as_view" => {
                        let Some(owner_name) = args.first().and_then(Expr::as_ident) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                "vec_u8.as_view requires an identifier owner".to_string(),
                            ));
                        };
                        let Some(owner) = self.lookup(owner_name) else {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {owner_name:?}"),
                            ));
                        };
                        if owner.ty != Ty::VecU8 {
                            return Err(self.err(
                                CompileErrorKind::Typing,
                                "vec_u8.as_view expects vec_u8 owner".to_string(),
                            ));
                        }
                        Ok(Some(owner.c_name.clone()))
                    }
                    "view.slice" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.slice expects (bytes_view,i32,i32)".to_string(),
                            ));
                        }
                        self.borrow_of_view_expr(&args[0])
                    }
                    // Views that borrow from runtime state (not from a user-owned buffer).
                    "bufread.fill" => Ok(None),
                    _ if self.fn_view_return_arg.contains_key(head) => {
                        let Some(spec) = self.fn_view_return_arg.get(head) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Internal,
                                format!(
                                    "internal error: missing view-return analysis for {head:?}"
                                ),
                            ));
                        };
                        match spec {
                            Some(idx) => {
                                if args.len() <= *idx {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!(
                                            "call {head:?} needs arg {idx} to infer bytes_view borrow source"
                                        ),
                                    ));
                                }
                                self.borrow_of_as_bytes_view(&args[*idx])
                            }
                            None => Ok(None),
                        }
                    }
                    _ => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "cannot infer borrow source for bytes_view expression".to_string(),
                    )),
                }
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Typing,
                "cannot infer borrow source for bytes_view expression".to_string(),
            )),
        }
    }

    fn borrow_of_as_bytes_view(&self, expr: &Expr) -> Result<Option<String>, CompilerError> {
        match expr {
            Expr::Ident(name) => {
                if name == "input" {
                    return Ok(None);
                }
                let Some(v) = self.lookup(name) else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                match v.ty {
                    Ty::BytesView => Ok(v.borrow_of.clone()),
                    Ty::Bytes | Ty::VecU8 => Ok(Some(v.c_name.clone())),
                    other => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("expected bytes/bytes_view/vec_u8, got {other:?}"),
                    )),
                }
            }
            _ => {
                let ty = self.infer_expr_in_new_scope(expr)?;
                match ty {
                    Ty::BytesView => self.borrow_of_view_expr(expr),
                    Ty::Bytes | Ty::VecU8 => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "bytes_view borrow source requires an identifier owner".to_string(),
                    )),
                    other => Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("expected bytes/bytes_view/vec_u8, got {other:?}"),
                    )),
                }
            }
        }
    }

    fn compute_view_return_args(&mut self) -> Result<(), CompilerError> {
        let mut cache: BTreeMap<String, Option<usize>> = BTreeMap::new();
        let mut visiting: BTreeSet<String> = BTreeSet::new();
        for f in &self.program.functions {
            if f.ret_ty != Ty::BytesView {
                continue;
            }
            self.view_return_arg_for_fn(&f.name, &mut cache, &mut visiting)
                .map_err(|e| {
                    CompilerError::new(
                        e.kind,
                        format!("{} (bytes_view return analysis: {:?})", e.message, f.name),
                    )
                })?;
        }
        self.fn_view_return_arg = cache;
        Ok(())
    }

    fn view_return_arg_for_fn(
        &self,
        fn_name: &str,
        cache: &mut BTreeMap<String, Option<usize>>,
        visiting: &mut BTreeSet<String>,
    ) -> Result<Option<usize>, CompilerError> {
        if let Some(spec) = cache.get(fn_name) {
            return Ok(*spec);
        }
        if !visiting.insert(fn_name.to_string()) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("recursive bytes_view return borrow analysis: {fn_name:?}"),
            ));
        }

        let Some(f) = self.program.functions.iter().find(|f| f.name == fn_name) else {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!("internal error: missing function def for {fn_name:?}"),
            ));
        };
        if f.ret_ty != Ty::BytesView {
            visiting.remove(fn_name);
            return Ok(None);
        }

        let mut env = ViewBorrowEnv::new();
        for (idx, p) in f.params.iter().enumerate() {
            match p.ty {
                Ty::BytesView => env.bind(p.name.clone(), ViewBorrowFrom::Param(idx)),
                Ty::Bytes | Ty::VecU8 => {
                    env.bind(p.name.clone(), ViewBorrowFrom::LocalOwned(p.name.clone()))
                }
                _ => {}
            }
        }

        let mut collector = ViewBorrowCollector::default();
        let body_src = self.infer_view_borrow_from_expr(
            fn_name,
            &f.body,
            &mut env,
            cache,
            visiting,
            &mut collector,
        )?;
        if let Some(src) = body_src {
            collector.merge(fn_name, src)?;
        }

        let Some(final_src) = collector.src else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("function {fn_name:?} returns bytes_view but no borrow source could be inferred"),
            ));
        };

        let spec = match final_src {
            ViewBorrowFrom::Runtime => None,
            ViewBorrowFrom::Param(i) => Some(i),
            ViewBorrowFrom::LocalOwned(owner) => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!(
                        "function {fn_name:?} returns bytes_view borrowed from local owned buffer {owner:?}"
                    ),
                ));
            }
        };
        cache.insert(fn_name.to_string(), spec);
        visiting.remove(fn_name);
        Ok(spec)
    }

    fn require_view_borrow_from_expr(
        &self,
        fn_name: &str,
        expr: &Expr,
        env: &mut ViewBorrowEnv,
        cache: &mut BTreeMap<String, Option<usize>>,
        visiting: &mut BTreeSet<String>,
        collector: &mut ViewBorrowCollector,
    ) -> Result<ViewBorrowFrom, CompilerError> {
        if let Some(src) =
            self.infer_view_borrow_from_expr(fn_name, expr, env, cache, visiting, collector)?
        {
            return Ok(src);
        }

        match expr {
            Expr::Ident(name) if name != "input" => {
                Ok(ViewBorrowFrom::LocalOwned(name.to_string()))
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("cannot infer bytes_view borrow source in function {fn_name:?}"),
            )),
        }
    }

    fn infer_view_borrow_from_expr(
        &self,
        fn_name: &str,
        expr: &Expr,
        env: &mut ViewBorrowEnv,
        cache: &mut BTreeMap<String, Option<usize>>,
        visiting: &mut BTreeSet<String>,
        collector: &mut ViewBorrowCollector,
    ) -> Result<Option<ViewBorrowFrom>, CompilerError> {
        match expr {
            Expr::Int(_) => Ok(None),
            Expr::Ident(name) => {
                if name == "input" {
                    return Ok(Some(ViewBorrowFrom::Runtime));
                }
                Ok(env.lookup(name))
            }
            Expr::List(items) => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];

                match head {
                    "begin" | "unsafe" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("({head} ...) requires at least 1 expression"),
                            ));
                        }
                        env.push_scope();
                        for e in &args[..args.len() - 1] {
                            let _ = self.infer_view_borrow_from_expr(
                                fn_name, e, env, cache, visiting, collector,
                            )?;
                        }
                        let out = self.infer_view_borrow_from_expr(
                            fn_name,
                            &args[args.len() - 1],
                            env,
                            cache,
                            visiting,
                            collector,
                        )?;
                        env.pop_scope();
                        Ok(out)
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        env.push_scope();
                        let t = self.infer_view_borrow_from_expr(
                            fn_name, &args[1], env, cache, visiting, collector,
                        )?;
                        env.pop_scope();

                        env.push_scope();
                        let e = self.infer_view_borrow_from_expr(
                            fn_name, &args[2], env, cache, visiting, collector,
                        )?;
                        env.pop_scope();

                        match (t, e) {
                            (Some(a), Some(b)) => Ok(Some(merge_view_borrow_from(fn_name, a, b)?)),
                            (Some(a), None) | (None, Some(a)) => Ok(Some(a)),
                            (None, None) => Ok(None),
                        }
                    }
                    "let" | "set" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} form: ({head} <name> <expr>)"),
                            ));
                        }
                        let name = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} name must be an identifier"),
                            )
                        })?;

                        let src = self.infer_view_borrow_from_expr(
                            fn_name, &args[1], env, cache, visiting, collector,
                        )?;
                        if let Some(src) = &src {
                            env.bind(name.to_string(), src.clone());
                        }
                        Ok(src)
                    }
                    "return" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "return form: (return <expr>)".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name, &args[0], env, cache, visiting, collector,
                        )?;
                        collector.merge(fn_name, src)?;
                        Ok(None)
                    }
                    "view.slice" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.slice expects (bytes_view,i32,i32)".to_string(),
                            ));
                        }
                        let src = self.require_view_borrow_from_expr(
                            fn_name, &args[0], env, cache, visiting, collector,
                        )?;
                        Ok(Some(src))
                    }
                    "bytes.view" | "bytes.subview" | "vec_u8.as_view" => {
                        let Some(owner_name) = args.first().and_then(Expr::as_ident) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} requires an identifier owner"),
                            ));
                        };
                        Ok(Some(ViewBorrowFrom::LocalOwned(owner_name.to_string())))
                    }
                    "bufread.fill" => Ok(Some(ViewBorrowFrom::Runtime)),
                    _ => {
                        let Some(f) = self.program.functions.iter().find(|f| f.name == head) else {
                            return Ok(None);
                        };
                        if f.ret_ty != Ty::BytesView {
                            return Ok(None);
                        }
                        let spec = self.view_return_arg_for_fn(head, cache, visiting)?;
                        match spec {
                            None => Ok(Some(ViewBorrowFrom::Runtime)),
                            Some(idx) => {
                                if args.len() <= idx {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("call {head:?} missing arg {idx}"),
                                    ));
                                }
                                let src = self.require_view_borrow_from_expr(
                                    fn_name, &args[idx], env, cache, visiting, collector,
                                )?;
                                Ok(Some(src))
                            }
                        }
                    }
                }
            }
        }
    }

    fn recompute_borrow_counts(&mut self) -> Result<(), CompilerError> {
        for scope in self.scopes.iter_mut() {
            for v in scope.values_mut() {
                v.borrow_count = 0;
            }
        }

        let mut borrows = Vec::<String>::new();
        for scope in &self.scopes {
            for v in scope.values() {
                if v.ty == Ty::BytesView {
                    if let Some(owner) = &v.borrow_of {
                        borrows.push(owner.clone());
                    }
                }
            }
        }
        for owner in borrows {
            self.inc_borrow_count(&owner)?;
        }

        for scope in &self.scopes {
            for v in scope.values() {
                if is_owned_ty(v.ty) && v.moved && v.borrow_count != 0 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "borrow of moved value".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn live_owned_drop_list(&self, skip_c_name: Option<&str>) -> Vec<(Ty, String)> {
        let mut to_drop = Vec::new();
        for scope in self.scopes.iter().rev() {
            for var in scope.values() {
                if !is_owned_ty(var.ty) {
                    continue;
                }
                if skip_c_name.is_some_and(|skip| skip == var.c_name) {
                    continue;
                }
                to_drop.push((var.ty, var.c_name.clone()));
            }
        }
        to_drop
    }

    fn merge_if_states(
        &self,
        before: &[BTreeMap<String, VarRef>],
        then_state: &[BTreeMap<String, VarRef>],
        else_state: &[BTreeMap<String, VarRef>],
    ) -> Result<Vec<BTreeMap<String, VarRef>>, CompilerError> {
        if before.len() != then_state.len() || before.len() != else_state.len() {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: if scope depth mismatch".to_string(),
            ));
        }

        let mut merged_scopes = Vec::with_capacity(before.len());
        for (i, before_scope) in before.iter().enumerate() {
            let then_scope = &then_state[i];
            let else_scope = &else_state[i];
            let mut merged = BTreeMap::new();
            for (name, pre) in before_scope {
                let Some(t) = then_scope.get(name) else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing var in then branch".to_string(),
                    ));
                };
                let Some(e) = else_scope.get(name) else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: missing var in else branch".to_string(),
                    ));
                };
                if pre.ty != t.ty
                    || pre.ty != e.ty
                    || pre.c_name != t.c_name
                    || pre.c_name != e.c_name
                {
                    return Err(CompilerError::new(
                        CompileErrorKind::Internal,
                        "internal error: if branch var mismatch".to_string(),
                    ));
                }

                let mut v = pre.clone();
                v.moved = t.moved || e.moved;
                v.borrow_count = 0;
                if v.ty == Ty::BytesView {
                    v.borrow_of = match (t.borrow_of.clone(), e.borrow_of.clone()) {
                        (Some(a), Some(b)) => {
                            if a != b {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "bytes_view must have a single borrow source across branches"
                                        .to_string(),
                                ));
                            }
                            Some(a)
                        }
                        (Some(a), None) | (None, Some(a)) => Some(a),
                        (None, None) => None,
                    };
                }

                merged.insert(name.clone(), v);
            }
            merged_scopes.push(merged);
        }
        Ok(merged_scopes)
    }

    fn require_standalone_only(&self, head: &str) -> Result<(), CompilerError> {
        if !self.options.world.is_standalone_only() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{head} is standalone-only; compile with --world run-os or --world run-os-sandboxed"
                ),
            ));
        }
        Ok(())
    }

    fn emit_program(&mut self) -> Result<(), CompilerError> {
        self.compute_view_return_args()?;
        if self.options.freestanding {
            self.push_str("#define X07_FREESTANDING 1\n");
        }
        if self.options.world.is_standalone_only() {
            self.push_str("#define X07_STANDALONE 1\n");
        }
        self.emit_runtime_preamble()?;
        if self.options.world.is_standalone_only() {
            self.push_str(RUNTIME_C_OS);
        }
        self.push_char('\n');

        self.emit_extern_function_prototypes();
        self.emit_async_function_prototypes();
        self.emit_user_function_prototypes();
        self.emit_async_functions()?;
        self.emit_user_functions()?;
        self.emit_solve()?;

        if self.options.emit_main {
            self.push_str(RUNTIME_C_MAIN);
        } else {
            self.push_str(RUNTIME_C_LIB);
        }
        Ok(())
    }

    fn check_program(&mut self) -> Result<(), CompilerError> {
        self.compute_view_return_args()?;
        for f in &self.program.async_functions {
            self.emit_async_function(f)?;
        }
        for f in &self.program.functions {
            self.emit_user_function(f)?;
        }
        self.emit_solve()?;
        Ok(())
    }

    fn emit_runtime_preamble(&mut self) -> Result<(), CompilerError> {
        if self.options.enable_fs || self.options.enable_rr || self.options.enable_kv {
            const FS_START: &str = "\n#if X07_ENABLE_FS\nstatic bytes_t rt_fs_read";
            const SHA256_START: &str = "\nstatic uint32_t rt_sha256_rotr";
            const SHA256_END: &str = "\nstatic void rt_hex_bytes";
            const RR_SEND_REQUEST_START: &str = "\nstatic bytes_t rt_rr_send_request";
            const RR_SEND_REQUEST_END: &str = "\nstatic void rt_rr_index_load";
            const RR_SEND_START: &str =
                "\nstatic uint32_t rt_rr_send(ctx_t* ctx, bytes_view_t req)";
            const RR_SEND_END: &str = "\n#endif\n\nstatic uint32_t rt_kv_u32_le";

            let uses_fs = program_uses_head(self.program, "fs.read")
                || program_uses_head(self.program, "fs.read_async")
                || program_uses_head(self.program, "fs.open_read")
                || program_uses_head(self.program, "fs.list_dir");
            let uses_rr_send_request = program_uses_head(self.program, "rr.send_request");
            let uses_rr_send = program_uses_head(self.program, "rr.send");

            if uses_fs && uses_rr_send_request {
                self.push_str(RUNTIME_C_PREAMBLE);
                return Ok(());
            }

            let mut preamble = RUNTIME_C_PREAMBLE.to_string();

            if !uses_fs {
                preamble = trim_preamble_section(&preamble, FS_START, SHA256_START, "fs runtime")?;
            }
            if !uses_rr_send_request {
                preamble = trim_preamble_section(&preamble, SHA256_START, SHA256_END, "sha256")?;
                preamble = trim_preamble_section(
                    &preamble,
                    RR_SEND_REQUEST_START,
                    RR_SEND_REQUEST_END,
                    "rr.send_request",
                )?;
            }
            if !uses_rr_send {
                preamble = trim_preamble_section(&preamble, RR_SEND_START, RR_SEND_END, "rr.send")?;
            }

            self.push_str(&preamble);
            return Ok(());
        }

        const FIXTURE_START: &str = "\n#if X07_ENABLE_FS\nstatic bytes_t rt_fs_read";
        const FIXTURE_END: &str = "\nstatic uint32_t rt_codec_read_u32_le";

        let (head, rest) = RUNTIME_C_PREAMBLE
            .split_once(FIXTURE_START)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    "internal error: runtime preamble missing fixture start marker".to_string(),
                )
            })?;
        let (_, tail) = rest.split_once(FIXTURE_END).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                "internal error: runtime preamble missing fixture end marker".to_string(),
            )
        })?;

        self.push_str(head);
        self.push_str(RUNTIME_C_PURE_STUBS);
        self.push_str(FIXTURE_END);
        self.push_str(tail);
        Ok(())
    }

    fn emit_extern_function_prototypes(&mut self) {
        for f in &self.program.extern_functions {
            let ret = if f.ret_is_void {
                "void".to_string()
            } else {
                c_ret_ty(f.ret_ty).to_string()
            };
            self.line(&format!(
                "extern {ret} {}({});",
                f.link_name,
                c_extern_param_list(&f.params)
            ));
        }
        if !self.program.extern_functions.is_empty() {
            self.push_char('\n');
        }
    }

    fn emit_async_function_prototypes(&mut self) {
        for f in &self.program.async_functions {
            self.line(&format!(
                "static uint32_t {}(ctx_t* ctx, void* fut, bytes_t* out);",
                c_async_poll_name(&f.name)
            ));
            self.line(&format!(
                "static uint32_t {}(ctx_t* ctx, bytes_view_t input{});",
                c_async_new_name(&f.name),
                c_param_list_value(&f.params)
            ));
        }
        if !self.program.async_functions.is_empty() {
            self.push_char('\n');
        }
    }

    fn emit_user_function_prototypes(&mut self) {
        for f in &self.program.functions {
            self.line(&format!(
                "static {} {}(ctx_t* ctx, bytes_view_t input{});",
                c_ret_ty(f.ret_ty),
                self.fn_c_name(&f.name),
                c_param_list_user(&f.params)
            ));
        }
        if !self.program.functions.is_empty() {
            self.push_char('\n');
        }
    }

    fn emit_user_functions(&mut self) -> Result<(), CompilerError> {
        for f in &self.program.functions {
            self.emit_user_function(f)?;
            self.push_char('\n');
        }
        Ok(())
    }

    fn emit_async_functions(&mut self) -> Result<(), CompilerError> {
        for f in &self.program.async_functions {
            self.emit_async_function(f)?;
            self.push_char('\n');
        }
        Ok(())
    }

    fn emit_async_function(&mut self, f: &AsyncFunctionDef) -> Result<(), CompilerError> {
        self.reset_fn_state();
        self.current_fn_name = Some(f.name.clone());
        self.fn_ret_ty = f.ret_ty;
        self.allow_async_ops = true;
        self.emit_source_line_for_symbol(&f.name);

        if f.ret_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!("defasync {:?} must return bytes", f.name),
            ));
        }

        let fut_type = c_async_fut_type_name(&f.name);
        let poll_name = c_async_poll_name(&f.name);
        let new_name = c_async_new_name(&f.name);
        let drop_name = c_async_drop_name(&f.name);

        let mut fields: Vec<(String, Ty)> = vec![
            ("state".to_string(), Ty::I32),
            ("input".to_string(), Ty::BytesView),
            ("ret".to_string(), Ty::Bytes),
        ];
        for (i, p) in f.params.iter().enumerate() {
            fields.push((format!("p{i}"), p.ty));
        }

        let functions = {
            let mut functions: BTreeMap<String, (Ty, Vec<Ty>)> = BTreeMap::new();
            for fun in &self.program.functions {
                functions.insert(
                    fun.name.clone(),
                    (
                        fun.ret_ty,
                        fun.params.iter().map(|p| p.ty).collect::<Vec<_>>(),
                    ),
                );
            }
            for fun in &self.program.async_functions {
                functions.insert(
                    fun.name.clone(),
                    (Ty::I32, fun.params.iter().map(|p| p.ty).collect::<Vec<_>>()),
                );
            }
            functions
        };

        struct Machine {
            options: CompileOptions,
            functions: BTreeMap<String, (Ty, Vec<Ty>)>,
            extern_functions: BTreeMap<String, ExternFunctionDecl>,
            fn_c_names: BTreeMap<String, String>,
            async_fn_new_names: BTreeMap<String, String>,
            fields: Vec<(String, Ty)>,
            tmp_counter: u32,
            local_count: usize,
            unsafe_depth: usize,
            scopes: Vec<BTreeMap<String, AsyncVarRef>>,
            states: Vec<Vec<String>>,
            ret_state: usize,
            fn_name: String,
        }

        impl Machine {
            fn new_state(&mut self) -> usize {
                let id = self.states.len();
                self.states.push(Vec::new());
                id
            }

            fn line(&mut self, state: usize, s: impl Into<String>) {
                self.states[state].push(s.into());
            }

            fn push_scope(&mut self) {
                self.scopes.push(BTreeMap::new());
            }

            fn pop_scope(&mut self) {
                self.scopes.pop();
            }

            fn bind(&mut self, name: String, var: AsyncVarRef) {
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(name, var);
                }
            }

            fn lookup(&self, name: &str) -> Option<AsyncVarRef> {
                for scope in self.scopes.iter().rev() {
                    if let Some(v) = scope.get(name) {
                        return Some(v.clone());
                    }
                }
                None
            }

            fn lookup_mut(&mut self, name: &str) -> Option<&mut AsyncVarRef> {
                for scope in self.scopes.iter_mut().rev() {
                    if let Some(v) = scope.get_mut(name) {
                        return Some(v);
                    }
                }
                None
            }

            fn alloc_local(&mut self, prefix: &str, ty: Ty) -> Result<AsyncVarRef, CompilerError> {
                if self.local_count >= language::limits::MAX_LOCALS {
                    return Err(CompilerError::new(
                        CompileErrorKind::Budget,
                        format!(
                            "max locals exceeded: {} (fn={})",
                            language::limits::MAX_LOCALS,
                            self.fn_name
                        ),
                    ));
                }
                self.local_count += 1;
                self.tmp_counter += 1;
                let name = format!("{prefix}{}", self.tmp_counter);
                self.fields.push((name.clone(), ty));
                Ok(AsyncVarRef {
                    ty,
                    c_name: format!("f->{name}"),
                    moved: false,
                })
            }

            fn infer_expr(&self, expr: &Expr) -> Result<Ty, CompilerError> {
                let mut infer = InferCtx {
                    options: self.options.clone(),
                    fn_ret_ty: Ty::Bytes,
                    allow_async_ops: true,
                    unsafe_depth: self.unsafe_depth,
                    scopes: self
                        .scopes
                        .iter()
                        .map(|s| {
                            s.iter()
                                .map(|(k, v)| (k.clone(), v.ty))
                                .collect::<BTreeMap<_, _>>()
                        })
                        .collect(),
                    functions: self.functions.clone(),
                    extern_functions: self.extern_functions.clone(),
                };
                infer.infer(expr)
            }

            fn emit_expr_entry(
                &mut self,
                state: usize,
                expr: &Expr,
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                match expr {
                    Expr::Int(_) | Expr::Ident(_) | Expr::List(_) => {}
                }
                match expr {
                    Expr::Int(i) => {
                        self.line(state, "rt_fuel(ctx, 1);");
                        if dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "int literal used where bytes expected".to_string(),
                            ));
                        }
                        let v = *i as u32;
                        self.line(state, format!("{} = UINT32_C({v});", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        Ok(())
                    }
                    Expr::Ident(name) => {
                        self.line(state, "rt_fuel(ctx, 1);");
                        if name == "input" {
                            if dest.ty != Ty::BytesView {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "input is bytes_view".to_string(),
                                ));
                            }
                            self.line(state, format!("{} = f->input;", dest.c_name));
                            self.line(state, format!("goto st_{cont};"));
                            return Ok(());
                        }
                        let Some(v) = self.lookup(name) else {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        };
                        if is_owned_ty(v.ty) && v.moved {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("use after move: {name:?}"),
                            ));
                        }
                        let wrote = match (dest.ty, v.ty) {
                            (Ty::BytesView, Ty::BytesView) => {
                                self.line(state, format!("{} = {};", dest.c_name, v.c_name));
                                true
                            }
                            (Ty::BytesView, Ty::Bytes) => {
                                self.line(
                                    state,
                                    format!("{} = rt_bytes_view(ctx, {});", dest.c_name, v.c_name),
                                );
                                true
                            }
                            (Ty::BytesView, Ty::VecU8) => {
                                self.line(
                                    state,
                                    format!(
                                        "{} = rt_vec_u8_as_view(ctx, {});",
                                        dest.c_name, v.c_name
                                    ),
                                );
                                true
                            }
                            _ => false,
                        };
                        if !wrote {
                            if v.ty != dest.ty {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("type mismatch for identifier {name:?}"),
                                ));
                            }
                            self.line(state, format!("{} = {};", dest.c_name, v.c_name));
                            if is_owned_ty(dest.ty) {
                                if let Some(v) = self.lookup_mut(name) {
                                    v.moved = true;
                                }
                                self.line(state, format!("{} = {};", v.c_name, c_empty(dest.ty)));
                            }
                        }
                        self.line(state, format!("goto st_{cont};"));
                        Ok(())
                    }
                    Expr::List(items) => self.emit_list_entry(state, items, dest, cont),
                }
            }

            fn emit_list_entry(
                &mut self,
                state: usize,
                items: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];

                match head {
                    "unsafe" => return self.emit_unsafe(state, args, dest, cont),
                    "begin" => return self.emit_begin(state, args, dest, cont),
                    "let" => return self.emit_let(state, args, dest, cont),
                    "set" => return self.emit_set(state, args, dest, cont),
                    "if" => return self.emit_if(state, args, dest, cont),
                    "for" => return self.emit_for(state, args, dest, cont),
                    "return" => return self.emit_return(state, args),
                    _ => {}
                }

                if head == "bytes.lit" {
                    if args.len() != 1 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Parse,
                            "bytes.lit expects 1 arg".to_string(),
                        ));
                    }
                    if dest.ty != Ty::Bytes {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            "bytes.lit returns bytes".to_string(),
                        ));
                    }
                    let ident = args[0].as_ident().ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Parse,
                            "bytes.lit expects an identifier".to_string(),
                        )
                    })?;
                    let lit_bytes = ident.as_bytes();
                    self.line(state, "rt_fuel(ctx, 1);");
                    self.tmp_counter += 1;
                    let lit_name = format!("lit_{}", self.tmp_counter);
                    let escaped = c_escape_string(lit_bytes);
                    self.line(
                        state,
                        format!("static const char {lit_name}[] = \"{escaped}\";"),
                    );
                    self.line(
                        state,
                        format!(
                            "{} = rt_bytes_from_literal(ctx, (const uint8_t*){lit_name}, UINT32_C({}));",
                            dest.c_name,
                            lit_bytes.len()
                        ),
                    );
                    self.line(state, format!("goto st_{cont};"));
                    return Ok(());
                }

                if head == "ptr.cast" {
                    return self.emit_ptr_cast_form(state, args, dest, cont);
                }
                if head == "addr_of" {
                    return self.emit_addr_of_form(state, args, dest, cont, false);
                }
                if head == "addr_of_mut" {
                    return self.emit_addr_of_form(state, args, dest, cont, true);
                }

                let call_ty = self.infer_expr(&Expr::List(items.to_vec()))?;
                if call_ty != dest.ty && call_ty != Ty::Never {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("call must evaluate to {:?}, got {:?}", dest.ty, call_ty),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");

                let want_params: Option<Vec<Ty>> = if self.extern_functions.contains_key(head) {
                    let f = self.extern_functions.get(head).cloned().ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Internal,
                            format!("internal error: missing extern decl for {head:?}"),
                        )
                    })?;
                    Some(f.params.iter().map(|p| p.ty).collect())
                } else if self.fn_c_names.contains_key(head)
                    || self.async_fn_new_names.contains_key(head)
                {
                    let (_ret, params) = self.functions.get(head).cloned().ok_or_else(|| {
                        CompilerError::new(
                            CompileErrorKind::Internal,
                            format!("internal error: missing function signature for {head:?}"),
                        )
                    })?;
                    Some(params)
                } else {
                    None
                };
                if let Some(want) = &want_params {
                    if want.len() != args.len() {
                        return Err(CompilerError::new(
                            CompileErrorKind::Parse,
                            format!("call {head:?} expects {} args", want.len()),
                        ));
                    }
                }

                let mut arg_vars: Vec<AsyncVarRef> = Vec::with_capacity(args.len());
                let mut arg_states: Vec<usize> = Vec::with_capacity(args.len());
                for (i, arg_expr) in args.iter().enumerate() {
                    let ty = match &want_params {
                        Some(want) => want[i],
                        None => self.infer_expr(arg_expr)?,
                    };
                    let storage_ty = match ty {
                        Ty::Never => Ty::I32,
                        other => other,
                    };
                    let tmp = self.alloc_local("t_arg_", storage_ty)?;
                    arg_vars.push(tmp);
                    arg_states.push(self.new_state());
                }

                let apply_state = self.new_state();
                if let Some(first) = arg_states.first().copied() {
                    self.line(state, format!("goto st_{first};"));
                } else {
                    self.line(state, format!("goto st_{apply_state};"));
                }

                for i in 0..arg_states.len() {
                    let next = if i + 1 < arg_states.len() {
                        arg_states[i + 1]
                    } else {
                        apply_state
                    };
                    let s = arg_states[i];
                    let expr = &args[i];
                    let tmp = arg_vars[i].clone();
                    self.emit_expr_entry(s, expr, tmp, next)?;
                }

                self.emit_apply_call(apply_state, head, &arg_vars, dest, cont)
            }

            fn emit_ptr_cast_form(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if !self.options.allow_unsafe() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!(
                            "ptr.cast requires unsafe capability; {}",
                            self.options.hint_enable_unsafe()
                        ),
                    ));
                }
                if self.unsafe_depth == 0 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "unsafe-required: ptr.cast".to_string(),
                    ));
                }
                if args.len() != 2 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "ptr.cast form: (ptr.cast <ptr_ty> <ptr>)".to_string(),
                    ));
                }
                self.line(state, "rt_fuel(ctx, 1);");

                let ty_name = args[0].as_ident().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "ptr.cast target type must be an identifier".to_string(),
                    )
                })?;
                let target = Ty::parse_named(ty_name).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("ptr.cast unknown type: {ty_name:?}"),
                    )
                })?;
                if !target.is_ptr_ty() || dest.ty != target {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "ptr.cast target type must match the expression type".to_string(),
                    ));
                }

                let ptr_expr = &args[1];
                let src_ty = self.infer_expr(ptr_expr)?;
                if !src_ty.is_ptr_ty() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "ptr.cast expects a raw pointer".to_string(),
                    ));
                }

                let tmp = self.alloc_local("t_cast_", src_ty)?;
                let expr_state = self.new_state();
                let after = self.new_state();
                self.line(state, format!("goto st_{expr_state};"));
                self.emit_expr_entry(expr_state, ptr_expr, tmp.clone(), after)?;
                self.line(
                    after,
                    format!("{} = ({}){};", dest.c_name, c_ret_ty(target), tmp.c_name),
                );
                self.line(after, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_addr_of_form(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
                is_mut: bool,
            ) -> Result<(), CompilerError> {
                let head = if is_mut { "addr_of_mut" } else { "addr_of" };
                if !self.options.allow_unsafe() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!(
                            "{head} requires unsafe capability; {}",
                            self.options.hint_enable_unsafe()
                        ),
                    ));
                }
                if args.len() != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "addr_of expects 1 arg".to_string(),
                    ));
                }
                let want = if is_mut {
                    Ty::PtrMutVoid
                } else {
                    Ty::PtrConstVoid
                };
                if dest.ty != want {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "addr_of returns a void raw pointer".to_string(),
                    ));
                }
                let name = args[0].as_ident().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "addr_of expects an identifier".to_string(),
                    )
                })?;

                let lvalue = if name == "input" {
                    "f->input".to_string()
                } else {
                    let Some(var) = self.lookup(name) else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("unknown identifier: {name:?}"),
                        ));
                    };
                    var.c_name
                };

                self.line(state, "rt_fuel(ctx, 1);");
                let cty = if is_mut { "void*" } else { "const void*" };
                self.line(state, format!("{} = ({cty})&({lvalue});", dest.c_name));
                self.line(state, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_apply_call(
                &mut self,
                state: usize,
                head: &str,
                args: &[AsyncVarRef],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                match head {
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<u" | ">>u" | "=" | "!="
                    | "<" | "<=" | ">" | ">=" | "<u" | ">=u" | ">u" | "<=u" => {
                        if args.len() != 2 || dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects 2 i32 args"),
                            ));
                        }
                        let a = &args[0];
                        let b = &args[1];
                        if a.ty != Ty::I32 || b.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 args"),
                            ));
                        }
                        match head {
                            "+" => self.line(
                                state,
                                format!("{} = {} + {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "-" => self.line(
                                state,
                                format!("{} = {} - {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "*" => self.line(
                                state,
                                format!("{} = {} * {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "/" => self.line(
                                state,
                                format!(
                                    "{} = ({} == UINT32_C(0)) ? UINT32_C(0) : ({} / {});",
                                    dest.c_name, b.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "%" => self.line(
                                state,
                                format!(
                                    "{} = ({} == UINT32_C(0)) ? {} : ({} % {});",
                                    dest.c_name, b.c_name, a.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "&" => self.line(
                                state,
                                format!("{} = {} & {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "|" => self.line(
                                state,
                                format!("{} = {} | {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "^" => self.line(
                                state,
                                format!("{} = {} ^ {};", dest.c_name, a.c_name, b.c_name),
                            ),
                            "<<u" => self.line(
                                state,
                                format!(
                                    "{} = {} << ({} & UINT32_C(31));",
                                    dest.c_name, a.c_name, b.c_name
                                ),
                            ),
                            ">>u" => self.line(
                                state,
                                format!(
                                    "{} = {} >> ({} & UINT32_C(31));",
                                    dest.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "=" => self.line(
                                state,
                                format!("{} = ({} == {});", dest.c_name, a.c_name, b.c_name),
                            ),
                            "!=" => self.line(
                                state,
                                format!("{} = ({} != {});", dest.c_name, a.c_name, b.c_name),
                            ),
                            "<" => self.line(
                                state,
                                format!(
                                    "{} = (({} ^ UINT32_C(0x80000000)) < ({} ^ UINT32_C(0x80000000)));",
                                    dest.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "<=" => self.line(
                                state,
                                format!(
                                    "{} = (({} ^ UINT32_C(0x80000000)) >= ({} ^ UINT32_C(0x80000000)));",
                                    dest.c_name, b.c_name, a.c_name
                                ),
                            ),
                            ">" => self.line(
                                state,
                                format!(
                                    "{} = (({} ^ UINT32_C(0x80000000)) < ({} ^ UINT32_C(0x80000000)));",
                                    dest.c_name, b.c_name, a.c_name
                                ),
                            ),
                            ">=" => self.line(
                                state,
                                format!(
                                    "{} = (({} ^ UINT32_C(0x80000000)) >= ({} ^ UINT32_C(0x80000000)));",
                                    dest.c_name, a.c_name, b.c_name
                                ),
                            ),
                            "<u" => self.line(
                                state,
                                format!("{} = ({} < {});", dest.c_name, a.c_name, b.c_name),
                            ),
                            ">=u" => self.line(
                                state,
                                format!("{} = ({} >= {});", dest.c_name, a.c_name, b.c_name),
                            ),
                            ">u" => self.line(
                                state,
                                format!("{} = ({} < {});", dest.c_name, b.c_name, a.c_name),
                            ),
                            "<=u" => self.line(
                                state,
                                format!("{} = ({} >= {});", dest.c_name, b.c_name, a.c_name),
                            ),
                            _ => unreachable!(),
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.len" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.len expects bytes_view".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.len;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.get_u8" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.get_u8 expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        let v = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_get_u8(ctx, {}, {});",
                                dest.c_name, v, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.set_u8" => {
                        if args.len() != 3
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_bytes_set_u8(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.add_v1" | "math.f64.sub_v1" | "math.f64.mul_v1"
                    | "math.f64.div_v1" | "math.f64.pow_v1" | "math.f64.atan2_v1"
                    | "math.f64.min_v1" | "math.f64.max_v1" => {
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || (args[1].ty != Ty::Bytes && args[1].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (bytes_view, bytes_view)"),
                            ));
                        }
                        let a = if args[0].ty == Ty::Bytes {
                            args[0].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[0].c_name, args[0].c_name
                            )
                        };
                        let b = if args[1].ty == Ty::Bytes {
                            args[1].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[1].c_name, args[1].c_name
                            )
                        };
                        let c_fn = match head {
                            "math.f64.add_v1" => "ev_math_f64_add_v1",
                            "math.f64.sub_v1" => "ev_math_f64_sub_v1",
                            "math.f64.mul_v1" => "ev_math_f64_mul_v1",
                            "math.f64.div_v1" => "ev_math_f64_div_v1",
                            "math.f64.pow_v1" => "ev_math_f64_pow_v1",
                            "math.f64.atan2_v1" => "ev_math_f64_atan2_v1",
                            "math.f64.min_v1" => "ev_math_f64_min_v1",
                            "math.f64.max_v1" => "ev_math_f64_max_v1",
                            _ => unreachable!(),
                        };
                        self.line(state, format!("{} = {c_fn}({a}, {b});", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.sqrt_v1"
                    | "math.f64.neg_v1"
                    | "math.f64.abs_v1"
                    | "math.f64.sin_v1"
                    | "math.f64.cos_v1"
                    | "math.f64.tan_v1"
                    | "math.f64.exp_v1"
                    | "math.f64.log_v1"
                    | "math.f64.floor_v1"
                    | "math.f64.ceil_v1"
                    | "math.f64.fmt_shortest_v1"
                    | "math.f64.to_bits_u64le_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects bytes_view"),
                            ));
                        }
                        let x = if args[0].ty == Ty::Bytes {
                            args[0].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[0].c_name, args[0].c_name
                            )
                        };
                        let c_fn = match head {
                            "math.f64.sqrt_v1" => "ev_math_f64_sqrt_v1",
                            "math.f64.neg_v1" => "ev_math_f64_neg_v1",
                            "math.f64.abs_v1" => "ev_math_f64_abs_v1",
                            "math.f64.sin_v1" => "ev_math_f64_sin_v1",
                            "math.f64.cos_v1" => "ev_math_f64_cos_v1",
                            "math.f64.tan_v1" => "ev_math_f64_tan_v1",
                            "math.f64.exp_v1" => "ev_math_f64_exp_v1",
                            "math.f64.log_v1" => "ev_math_f64_ln_v1",
                            "math.f64.floor_v1" => "ev_math_f64_floor_v1",
                            "math.f64.ceil_v1" => "ev_math_f64_ceil_v1",
                            "math.f64.fmt_shortest_v1" => "ev_math_f64_fmt_shortest_v1",
                            "math.f64.to_bits_u64le_v1" => "ev_math_f64_to_bits_u64le_v1",
                            _ => unreachable!(),
                        };
                        self.line(state, format!("{} = {c_fn}({x});", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.from_i32_v1" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.from_i32_v1 expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ev_math_f64_from_i32_v1({});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.to_i32_trunc_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::ResultI32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.to_i32_trunc_v1 expects bytes_view".to_string(),
                            ));
                        }
                        let x = if args[0].ty == Ty::Bytes {
                            args[0].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[0].c_name, args[0].c_name
                            )
                        };
                        self.line(
                            state,
                            format!("{} = ev_math_f64_to_i32_trunc_v1({x});", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "math.f64.parse_v1" => {
                        if args.len() != 1
                            || dest.ty != Ty::ResultBytes
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.parse_v1 expects bytes_view".to_string(),
                            ));
                        }
                        let s = if args[0].ty == Ty::Bytes {
                            args[0].c_name.clone()
                        } else {
                            format!(
                                "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
                                args[0].c_name, args[0].c_name
                            )
                        };
                        self.line(
                            state,
                            format!("{} = ev_math_f64_parse_v1({s});", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.alloc" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.alloc expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_bytes_alloc(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.empty" => {
                        if !args.is_empty() || dest.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.empty expects 0 args".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = rt_bytes_empty(ctx);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes1" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes1 expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_bytes_alloc(ctx, UINT32_C(1));", dest.c_name),
                        );
                        self.line(
                            state,
                            format!(
                                "{} = rt_bytes_set_u8(ctx, {}, UINT32_C(0), {});",
                                dest.c_name, dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.lit" => {
                        return Err(CompilerError::new(
                            CompileErrorKind::Unsupported,
                            "bytes.lit is not supported inside async codegen yet".to_string(),
                        ));
                    }
                    "bytes.slice" => {
                        if args.len() != 3
                            || dest.ty != Ty::Bytes
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.slice expects (bytes_view, i32, i32)".to_string(),
                            ));
                        }
                        let v = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_to_bytes(ctx, rt_view_slice(ctx, {}, {}, {}));",
                                dest.c_name, v, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.eq" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || (args[1].ty != Ty::Bytes && args[1].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.eq expects (bytes_view, bytes_view)".to_string(),
                            ));
                        }
                        let a = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        let b = if args[1].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[1].c_name)
                        } else {
                            args[1].c_name.clone()
                        };
                        self.line(
                            state,
                            format!("{} = rt_view_eq(ctx, {}, {});", dest.c_name, a, b),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.cmp_range" => {
                        if args.len() != 6
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                            || (args[3].ty != Ty::Bytes && args[3].ty != Ty::BytesView)
                            || args[4].ty != Ty::I32
                            || args[5].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.cmp_range expects (bytes_view,i32,i32,bytes_view,i32,i32)"
                                    .to_string(),
                            ));
                        }
                        let a = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        let b = if args[3].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[3].c_name)
                        } else {
                            args[3].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_cmp_range(ctx, {}, {}, {}, {}, {}, {});",
                                dest.c_name,
                                a,
                                args[1].c_name,
                                args[2].c_name,
                                b,
                                args[4].c_name,
                                args[5].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.as_ptr" => {
                        if args.len() != 1 || dest.ty != Ty::PtrConstU8 || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.as_ptr expects bytes".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.ptr;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.as_mut_ptr" => {
                        if args.len() != 1 || dest.ty != Ty::PtrMutU8 || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.as_mut_ptr expects bytes".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.ptr;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.view" => {
                        if args.len() != 1 || dest.ty != Ty::BytesView || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.view expects bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_bytes_view(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bytes.subview" => {
                        if args.len() != 3
                            || dest.ty != Ty::BytesView
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.subview expects (bytes,i32,i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_bytes_subview(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.len" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.len expects bytes_view".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.len;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.get_u8" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.get_u8 expects (bytes_view,i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_get_u8(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.slice" => {
                        if args.len() != 3
                            || dest.ty != Ty::BytesView
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.slice expects (bytes_view,i32,i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_slice(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.to_bytes" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.to_bytes expects bytes_view".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_to_bytes(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.as_ptr" => {
                        if args.len() != 1
                            || dest.ty != Ty::PtrConstU8
                            || args[0].ty != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.as_ptr expects bytes_view".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {}.ptr;", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.eq" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.eq expects (bytes_view,bytes_view)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_eq(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "view.cmp_range" => {
                        if args.len() != 6
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                            || args[3].ty != Ty::BytesView
                            || args[4].ty != Ty::I32
                            || args[5].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.cmp_range expects (bytes_view,i32,i32,bytes_view,i32,i32)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_view_cmp_range(ctx, {}, {}, {}, {}, {}, {});",
                                dest.c_name,
                                args[0].c_name,
                                args[1].c_name,
                                args[2].c_name,
                                args[3].c_name,
                                args[4].c_name,
                                args[5].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "await" | "task.join.bytes" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 and returns bytes"),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_task_join_bytes_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "task.spawn" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.spawn expects i32 and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_task_spawn(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "task.yield" => {
                        if !args.is_empty() || dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.yield expects 0 args".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, "rt_task_yield(ctx);");
                        self.line(state, format!("f->state = UINT32_C({cont});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "task.sleep" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.sleep expects i32".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("rt_task_sleep(ctx, {});", args[0].c_name));
                        self.line(state, format!("f->state = UINT32_C({cont});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "task.cancel" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.cancel expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_task_cancel(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "chan.bytes.new" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.new expects i32 cap".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_chan_bytes_new(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "chan.bytes.send" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.send expects (i32, bytes)".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_chan_bytes_send_poll(ctx, {}, {})) {{ {} = UINT32_C(1); {} = {}; goto st_{cont}; }}",
                                args[0].c_name, args[1].c_name, dest.c_name, args[1].c_name, c_empty(Ty::Bytes)
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "chan.bytes.recv" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.recv expects i32".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_chan_bytes_recv_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "chan.bytes.close" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.close expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_chan_bytes_close(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fs.read" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.read is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.read expects bytes_view path".to_string(),
                            ));
                        }
                        let path = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        let done = self.new_state();
                        self.line(
                            state,
                            format!("uint32_t ticks = rt_fs_latency_ticks(ctx, {});", path),
                        );
                        self.line(state, "if (ticks == UINT32_C(0)) {");
                        self.line(
                            state,
                            format!("  {} = rt_fs_read(ctx, {});", dest.c_name, path),
                        );
                        self.line(state, format!("  goto st_{cont};"));
                        self.line(state, "}");
                        self.line(state, "rt_task_sleep(ctx, ticks);");
                        self.line(state, format!("f->state = UINT32_C({done});"));
                        self.line(state, "return UINT32_C(0);");
                        self.line(
                            done,
                            format!("{} = rt_fs_read(ctx, {});", dest.c_name, path),
                        );
                        self.line(done, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fs.list_dir" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.list_dir is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.list_dir expects bytes_view path".to_string(),
                            ));
                        }
                        let path = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!("{} = rt_fs_list_dir(ctx, {});", dest.c_name, path),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fs.read_async" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.read_async is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.read_async expects bytes_view path".to_string(),
                            ));
                        }
                        let path = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        let done = self.new_state();
                        self.line(
                            state,
                            format!("uint32_t ticks = rt_fs_latency_ticks(ctx, {});", path),
                        );
                        self.line(state, "if (ticks == UINT32_C(0)) {");
                        self.line(
                            state,
                            format!("  {} = rt_fs_read(ctx, {});", dest.c_name, path),
                        );
                        self.line(state, format!("  goto st_{cont};"));
                        self.line(state, "}");
                        self.line(state, "rt_task_sleep(ctx, ticks);");
                        self.line(state, format!("f->state = UINT32_C({done});"));
                        self.line(state, "return UINT32_C(0);");
                        self.line(
                            done,
                            format!("{} = rt_fs_read(ctx, {});", dest.c_name, path),
                        );
                        self.line(done, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fs.open_read" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.open_read is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Iface
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.open_read expects bytes_view path".to_string(),
                            ));
                        }
                        let path = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!(
                                "{} = (iface_t){{ .data = rt_fs_open_read(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
                                dest.c_name, path
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.read_file" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.read_file is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.read_file expects bytes path".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_fs_read_file(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.write_file" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.write_file is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.write_file expects (bytes path, bytes data)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_fs_write_file(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.read_all_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.read_all_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.read_all_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_read_all_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.write_all_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.write_all_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 3
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                            || args[2].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.write_all_v1 expects (bytes path, bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_write_all_v1({}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.mkdirs_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.mkdirs_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.mkdirs_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_mkdirs_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.remove_file_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.remove_file_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.remove_file_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_remove_file_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.remove_dir_all_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.remove_dir_all_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.remove_dir_all_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_remove_dir_all_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.rename_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.rename_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 3
                            || dest.ty != Ty::ResultI32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                            || args[2].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.rename_v1 expects (bytes src, bytes dst, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_rename_v1({}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.list_dir_sorted_text_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.list_dir_sorted_text_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.list_dir_sorted_text_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_list_dir_sorted_text_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.walk_glob_sorted_text_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.walk_glob_sorted_text_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 3
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                            || args[2].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.walk_glob_sorted_text_v1 expects (bytes root, bytes glob, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_walk_glob_sorted_text_v1({}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.fs.stat_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.fs.stat_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::ResultBytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stat_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_fs_stat_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.sqlite.open_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.sqlite.open_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_sqlite_open_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.sqlite.query_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.sqlite.query_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_sqlite_query_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.sqlite.exec_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.sqlite.exec_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_sqlite_exec_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.sqlite.close_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.sqlite.close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_sqlite_close_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.pg.open_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.pg.open_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_pg_open_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.pg.query_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.pg.query_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_pg_query_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.pg.exec_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.pg.exec_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_pg_exec_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.pg.close_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.pg.close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_pg_close_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.mysql.open_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.mysql.open_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_mysql_open_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.mysql.query_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.mysql.query_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_mysql_query_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.mysql.exec_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.mysql.exec_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_mysql_exec_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.mysql.close_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.mysql.close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_mysql_close_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.redis.open_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.redis.open_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_redis_open_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.redis.cmd_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.redis.cmd_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.cmd_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_redis_cmd_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.db.redis.close_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.db.redis.close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = x07_ext_db_redis_close_v1({}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.env.get" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.env.get is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.env.get expects bytes key".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_os_env_get(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.now_unix_ms" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.time.now_unix_ms is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if !args.is_empty() || dest.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.now_unix_ms expects 0 args and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_os_time_now_unix_ms(ctx);", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.now_instant_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.time.now_instant_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if !args.is_empty() || dest.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.now_instant_v1 expects 0 args and returns bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_os_time_now_instant_v1(ctx);", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.sleep_ms_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.time.sleep_ms_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.sleep_ms_v1 expects i32 ms and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = (int32_t)rt_os_time_sleep_ms_v1(ctx, (int32_t){});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.local_tzid_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.time.local_tzid_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if !args.is_empty() || dest.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.local_tzid_v1 expects 0 args and returns bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_os_time_local_tzid_v1(ctx);", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.tzdb_is_valid_tzid_v1" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_is_valid_tzid_v1 expects bytes_view tzid and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ev_time_tzdb_is_valid_tzid_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }});",
                                dest.c_name, args[0].c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.tzdb_offset_duration_v1" => {
                        if args.len() != 3
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::BytesView
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_offset_duration_v1 expects (bytes_view tzid, i32 unix_s_lo, i32 unix_s_hi) and returns bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = ev_time_tzdb_offset_duration_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){}, (int32_t){});",
                                dest.c_name, args[0].c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.time.tzdb_snapshot_id_v1" => {
                        if !args.is_empty() || dest.ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_snapshot_id_v1 expects 0 args and returns bytes"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ev_time_tzdb_snapshot_id_v1();", dest.c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.exit" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.exit is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.exit expects i32 code".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("rt_os_process_exit(ctx, (int32_t){});", args[0].c_name),
                        );
                        self.line(state, "__builtin_unreachable();");
                        return Ok(());
                    }
                    "os.process.spawn_capture_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.spawn_capture_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.spawn_capture_v1 expects (bytes req, bytes caps) and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_spawn_capture_v1(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.spawn_piped_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.spawn_piped_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.spawn_piped_v1 expects (bytes req, bytes caps) and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_spawn_piped_v1(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.try_join_capture_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.try_join_capture_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::OptionBytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.try_join_capture_v1 expects i32 and returns option_bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_try_join_capture_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.join_capture_v1" | "std.os.process.join_capture_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.join_capture_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.join_capture_v1 expects i32 and returns bytes"
                                    .to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_os_process_join_capture_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "os.process.stdout_read_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.stdout_read_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdout_read_v1 expects (i32 handle, i32 max) and returns bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_stdout_read_v1(ctx, {}, (int32_t){});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.stderr_read_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.stderr_read_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stderr_read_v1 expects (i32 handle, i32 max) and returns bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_stderr_read_v1(ctx, {}, (int32_t){});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.stdin_write_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.stdin_write_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdin_write_v1 expects (i32 handle, bytes chunk) and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_stdin_write_v1(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.stdin_close_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.stdin_close_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdin_close_v1 expects i32 and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_stdin_close_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.try_wait_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.try_wait_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.try_wait_v1 expects i32 and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_try_wait_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.join_exit_v1" | "std.os.process.join_exit_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.join_exit_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.join_exit_v1 expects i32 and returns i32".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_os_process_join_exit_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "os.process.take_exit_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.take_exit_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.take_exit_v1 expects i32 and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_take_exit_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.kill_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.kill_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.kill_v1 expects (i32 proc_handle, i32 sig) and returns i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_kill_v1(ctx, {}, (int32_t){});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.drop_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.drop_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.drop_v1 expects i32 proc handle and returns i32"
                                    .to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_drop_v1(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.process.run_capture_v1" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.run_capture_v1 is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.run_capture_v1 expects (bytes req, bytes caps) and returns bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_process_run_capture_v1(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "os.net.http_request" => {
                        if !self.options.world.is_standalone_only() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.net.http_request is only available in standalone worlds (run-os, run-os-sandboxed)".to_string(),
                            ));
                        }
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.net.http_request expects bytes req".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_os_net_http_request(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "rr.send_request" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.send_request is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.send_request expects bytes_view".to_string(),
                            ));
                        }
                        let req = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!("{} = rt_rr_send_request(ctx, {});", dest.c_name, req),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "rr.fetch" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.fetch is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.fetch expects bytes_view key".to_string(),
                            ));
                        }
                        let key = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        let done = self.new_state();
                        self.line(
                            state,
                            format!("uint32_t ticks = rt_rr_latency_ticks(ctx, {});", key),
                        );
                        self.line(state, "if (ticks == UINT32_C(0)) {");
                        self.line(
                            state,
                            format!("  {} = rt_rr_fetch_body(ctx, {});", dest.c_name, key),
                        );
                        self.line(state, format!("  goto st_{cont};"));
                        self.line(state, "}");
                        self.line(state, "rt_task_sleep(ctx, ticks);");
                        self.line(state, format!("f->state = UINT32_C({done});"));
                        self.line(state, "return UINT32_C(0);");
                        self.line(
                            done,
                            format!("{} = rt_rr_fetch_body(ctx, {});", dest.c_name, key),
                        );
                        self.line(done, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "rr.send" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.send is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Iface
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.send expects bytes_view req".to_string(),
                            ));
                        }
                        let req = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!(
                                "{} = (iface_t){{ .data = rt_rr_send(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
                                dest.c_name, req
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "kv.get" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get expects bytes_view key".to_string(),
                            ));
                        }
                        let key = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        let done = self.new_state();
                        self.line(
                            state,
                            format!("uint32_t ticks = rt_kv_latency_ticks(ctx, {});", key),
                        );
                        self.line(state, "if (ticks == UINT32_C(0)) {");
                        self.line(
                            state,
                            format!("  {} = rt_kv_get(ctx, {});", dest.c_name, key),
                        );
                        self.line(state, format!("  goto st_{cont};"));
                        self.line(state, "}");
                        self.line(state, "rt_task_sleep(ctx, ticks);");
                        self.line(state, format!("f->state = UINT32_C({done});"));
                        self.line(state, "return UINT32_C(0);");
                        self.line(done, format!("{} = rt_kv_get(ctx, {});", dest.c_name, key));
                        self.line(done, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "kv.get_async" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get_async is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Bytes
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get_async expects bytes_view key".to_string(),
                            ));
                        }
                        let key = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        let done = self.new_state();
                        self.line(
                            state,
                            format!("uint32_t ticks = rt_kv_latency_ticks(ctx, {});", key),
                        );
                        self.line(state, "if (ticks == UINT32_C(0)) {");
                        self.line(
                            state,
                            format!("  {} = rt_kv_get(ctx, {});", dest.c_name, key),
                        );
                        self.line(state, format!("  goto st_{cont};"));
                        self.line(state, "}");
                        self.line(state, "rt_task_sleep(ctx, ticks);");
                        self.line(state, format!("f->state = UINT32_C({done});"));
                        self.line(state, "return UINT32_C(0);");
                        self.line(done, format!("{} = rt_kv_get(ctx, {});", dest.c_name, key));
                        self.line(done, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "kv.get_stream" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get_stream is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1
                            || dest.ty != Ty::Iface
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get_stream expects bytes_view key".to_string(),
                            ));
                        }
                        let key = match args[0].ty {
                            Ty::BytesView => args[0].c_name.clone(),
                            Ty::Bytes => format!("rt_bytes_view(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_as_view(ctx, {})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!(
                                "{} = (iface_t){{ .data = rt_kv_get_stream(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
                                dest.c_name, key
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "kv.set" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.set is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.set expects (bytes,bytes)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_kv_set(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(
                            state,
                            format!("{} = {};", args[0].c_name, c_empty(Ty::Bytes)),
                        );
                        self.line(
                            state,
                            format!("{} = {};", args[1].c_name, c_empty(Ty::Bytes)),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "io.open_read_bytes" => {
                        if args.len() != 1
                            || dest.ty != Ty::Iface
                            || !matches!(args[0].ty, Ty::Bytes | Ty::BytesView | Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "io.open_read_bytes expects bytes".to_string(),
                            ));
                        }
                        let b = match args[0].ty {
                            Ty::Bytes => args[0].c_name.clone(),
                            Ty::BytesView => format!("rt_view_to_bytes(ctx, {})", args[0].c_name),
                            Ty::VecU8 => format!("rt_vec_u8_into_bytes(ctx, &{})", args[0].c_name),
                            _ => unreachable!(),
                        };
                        self.line(
                            state,
                            format!(
                                "{} = (iface_t){{ .data = rt_io_reader_new_bytes(ctx, {}, UINT32_C(0)), .vtable = RT_IFACE_VTABLE_IO_READER }};",
                                dest.c_name, b
                            ),
                        );
                        if args[0].ty == Ty::Bytes {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::Bytes)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "io.read" => {
                        if args.len() != 2
                            || dest.ty != Ty::Bytes
                            || args[0].ty != Ty::Iface
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "io.read expects (iface, i32)".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if ({}.vtable != RT_IFACE_VTABLE_IO_READER) {{ {} = rt_iface_io_read_block(ctx, {}, {}); goto st_{cont}; }}",
                                args[0].c_name,
                                dest.c_name,
                                args[0].c_name,
                                args[1].c_name,
                            ),
                        );
                        self.line(
                            state,
                            format!(
                                "if (rt_io_read_poll(ctx, {}.data, {}, &{})) goto st_{cont};",
                                args[0].c_name, args[1].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "iface.make_v1" => {
                        if args.len() != 2
                            || dest.ty != Ty::Iface
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "iface.make_v1 expects (i32, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = (iface_t){{ .data = {}, .vtable = {} }};",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bufread.new" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Iface
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.new expects (iface, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_bufread_new(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "bufread.fill" => {
                        if args.len() != 1 || dest.ty != Ty::BytesView || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.fill expects i32".to_string(),
                            ));
                        }
                        let resume = state;
                        self.line(
                            state,
                            format!(
                                "if (rt_bufread_fill_poll(ctx, {}, &{})) goto st_{cont};",
                                args[0].c_name, dest.c_name
                            ),
                        );
                        self.line(state, format!("f->state = UINT32_C({resume});"));
                        self.line(state, "return UINT32_C(0);");
                        return Ok(());
                    }
                    "bufread.consume" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.consume expects (i32, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_bufread_consume(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "codec.read_u32_le" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || (args[0].ty != Ty::Bytes && args[0].ty != Ty::BytesView)
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "codec.read_u32_le expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        let b = if args[0].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[0].c_name)
                        } else {
                            args[0].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_codec_read_u32_le(ctx, {}, {});",
                                dest.c_name, b, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "codec.write_u32_le" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "codec.write_u32_le expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_codec_write_u32_le(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fmt.u32_to_dec" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fmt.u32_to_dec expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_fmt_u32_to_dec(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "fmt.s32_to_dec" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fmt.s32_to_dec expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_fmt_s32_to_dec(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "parse.u32_dec" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "parse.u32_dec expects bytes".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_parse_u32_dec(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "parse.u32_dec_at" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::Bytes
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "parse.u32_dec_at expects (bytes,i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_parse_u32_dec_at(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "prng.lcg_next_u32" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "prng.lcg_next_u32 expects i32".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_prng_lcg_next_u32(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.with_capacity" => {
                        if args.len() != 1 || dest.ty != Ty::VecU8 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.with_capacity expects i32 cap".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_vec_u8_new(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.len" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.len expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_vec_u8_len(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.get" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.get expects (vec_u8, idx)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_get(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.set" => {
                        if args.len() != 3
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.set expects (vec_u8, idx, val)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_set(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.push" => {
                        if args.len() != 2
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.push expects (vec_u8, val)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_push(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.reserve_exact" => {
                        if args.len() != 2
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.reserve_exact expects (vec_u8, additional)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_reserve_exact(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.extend_zeroes" => {
                        if args.len() != 2
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_extend_zeroes(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.extend_bytes" => {
                        if args.len() != 2
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || (args[1].ty != Ty::Bytes && args[1].ty != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
                            ));
                        }
                        let b = if args[1].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[1].c_name)
                        } else {
                            args[1].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_extend_bytes(ctx, {}, {});",
                                dest.c_name, args[0].c_name, b
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.extend_bytes_range" => {
                        if args.len() != 4
                            || dest.ty != Ty::VecU8
                            || args[0].ty != Ty::VecU8
                            || (args[1].ty != Ty::Bytes && args[1].ty != Ty::BytesView)
                            || args[2].ty != Ty::I32
                            || args[3].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, off, len)"
                                    .to_string(),
                            ));
                        }
                        let b = if args[1].ty == Ty::Bytes {
                            format!("rt_bytes_view(ctx, {})", args[1].c_name)
                        } else {
                            args[1].c_name.clone()
                        };
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_extend_bytes_range(ctx, {}, {}, {}, {});",
                                dest.c_name, args[0].c_name, b, args[2].c_name, args[3].c_name
                            ),
                        );
                        if dest.c_name != args[0].c_name {
                            self.line(
                                state,
                                format!("{} = {};", args[0].c_name, c_empty(Ty::VecU8)),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.into_bytes" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.into_bytes expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_into_bytes(ctx, &{});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.as_view" => {
                        if args.len() != 1 || dest.ty != Ty::BytesView || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.as_view expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_vec_u8_as_view(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.as_ptr" => {
                        if args.len() != 1 || dest.ty != Ty::PtrConstU8 || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.as_ptr expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ({}).data;", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "vec_u8.as_mut_ptr" => {
                        if args.len() != 1 || dest.ty != Ty::PtrMutU8 || args[0].ty != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.as_mut_ptr expects vec_u8".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = ({}).data;", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.null" => {
                        if !args.is_empty() || dest.ty != Ty::PtrMutVoid {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.null expects 0 args".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = NULL;", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.as_const" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.as_const expects 1 arg".to_string(),
                            ));
                        }
                        let ok = matches!(
                            (args[0].ty, dest.ty),
                            (Ty::PtrMutU8, Ty::PtrConstU8)
                                | (Ty::PtrConstU8, Ty::PtrConstU8)
                                | (Ty::PtrMutVoid, Ty::PtrConstVoid)
                                | (Ty::PtrConstVoid, Ty::PtrConstVoid)
                                | (Ty::PtrMutI32, Ty::PtrConstI32)
                                | (Ty::PtrConstI32, Ty::PtrConstI32)
                        );
                        if !ok {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.as_const expects a raw pointer and returns a const raw pointer"
                                    .to_string(),
                            ));
                        }
                        self.line(state, format!("{} = {};", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.add" | "ptr.sub" => {
                        if args.len() != 2 || args[0].ty != dest.ty || args[1].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (ptr, i32)"),
                            ));
                        }
                        let op = if head == "ptr.add" { "+" } else { "-" };
                        self.line(
                            state,
                            format!(
                                "{} = {} {op} (size_t){};",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.offset" => {
                        if args.len() != 2 || args[0].ty != dest.ty || args[1].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.offset expects (ptr, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = {} + (int32_t){};",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.read_u8" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || !matches!(args[0].ty, Ty::PtrConstU8 | Ty::PtrMutU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.read_u8 expects (ptr_const_u8|ptr_mut_u8)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = (uint32_t)(*{});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.write_u8" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::PtrMutU8
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.write_u8 expects (ptr_mut_u8, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("*{} = (uint8_t){};", args[0].c_name, args[1].c_name),
                        );
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.read_i32" => {
                        if args.len() != 1
                            || dest.ty != Ty::I32
                            || !matches!(args[0].ty, Ty::PtrConstI32 | Ty::PtrMutI32)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.read_i32 expects (ptr_const_i32|ptr_mut_i32)".to_string(),
                            ));
                        }
                        self.line(state, format!("{} = *{};", dest.c_name, args[0].c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "ptr.write_i32" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::PtrMutI32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.write_i32 expects (ptr_mut_i32, i32)".to_string(),
                            ));
                        }
                        self.line(state, format!("*{} = {};", args[0].c_name, args[1].c_name));
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "memcpy" | "memmove" => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::PtrMutVoid
                            || !matches!(args[1].ty, Ty::PtrConstVoid | Ty::PtrMutVoid)
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (ptr_mut_void, ptr_const_void, i32)"),
                            ));
                        }
                        self.line(state, format!("rt_mem_on_memcpy(ctx, {});", args[2].c_name));
                        self.line(
                            state,
                            format!(
                                "(void){head}((void*){}, (const void*){}, (size_t){});",
                                args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "memset" => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::PtrMutVoid
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "memset expects (ptr_mut_void, i32, i32)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "(void)memset((void*){}, (int){}, (size_t){});",
                                args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.new" | "set_u32.new" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.new expects i32 cap".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_map_u32_new(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.len" => {
                        if args.len() != 1 || dest.ty != Ty::I32 || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.len expects i32 handle".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!("{} = rt_map_u32_len(ctx, {});", dest.c_name, args[0].c_name),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.get" => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.get expects (handle,key,default)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_get(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.set" => {
                        if args.len() != 3
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                            || args[2].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.set expects (handle,key,val)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_set(ctx, {}, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name, args[2].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.contains" | "set_u32.contains" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.contains expects (handle,key)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_contains(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.remove" | "set_u32.remove" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.remove expects (handle,key)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_remove(ctx, {}, {});",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "set_u32.add" => {
                        if args.len() != 2
                            || dest.ty != Ty::I32
                            || args[0].ty != Ty::I32
                            || args[1].ty != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "set_u32.add expects (handle,key)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_set(ctx, {}, {}, UINT32_C(1));",
                                dest.c_name, args[0].c_name, args[1].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "set_u32.dump_u32le" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "set_u32.dump_u32le expects (handle)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_set_u32_dump_u32le(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    "map_u32.dump_kv_u32le_u32le" => {
                        if args.len() != 1 || dest.ty != Ty::Bytes || args[0].ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.dump_kv_u32le_u32le expects (handle)".to_string(),
                            ));
                        }
                        self.line(
                            state,
                            format!(
                                "{} = rt_map_u32_dump_kv_u32le_u32le(ctx, {});",
                                dest.c_name, args[0].c_name
                            ),
                        );
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    _ if self.extern_functions.contains_key(head) => {
                        let f = self.extern_functions.get(head).cloned().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Internal,
                                format!("internal error: missing extern decl for {head:?}"),
                            )
                        })?;

                        if args.len() != f.params.len() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("call {head:?} expects {} args", f.params.len()),
                            ));
                        }
                        if dest.ty != f.ret_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("call {head:?} returns {:?}", f.ret_ty),
                            ));
                        }

                        let mut rendered_args = Vec::with_capacity(args.len());
                        for (i, (arg, p)) in args.iter().zip(f.params.iter()).enumerate() {
                            let got = arg.ty;
                            let want = p.ty;
                            let ok = got == want
                                || matches!(
                                    (got, want),
                                    (Ty::PtrMutU8, Ty::PtrConstU8)
                                        | (Ty::PtrMutVoid, Ty::PtrConstVoid)
                                        | (Ty::PtrMutI32, Ty::PtrConstI32)
                                );
                            if !ok {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("call {head:?} arg {i} expects {want:?}"),
                                ));
                            }
                            rendered_args.push(arg.c_name.clone());
                        }
                        let c_args = rendered_args.join(", ");
                        if f.ret_is_void {
                            self.line(state, format!("{}({c_args});", f.link_name));
                            self.line(state, format!("{} = UINT32_C(0);", dest.c_name));
                        } else {
                            self.line(
                                state,
                                format!("{} = {}({c_args});", dest.c_name, f.link_name),
                            );
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    _ if self.fn_c_names.contains_key(head) => {
                        let cfn = self.fn_c_names.get(head).cloned().unwrap_or_default();
                        let rendered = args.iter().map(|a| a.c_name.clone()).collect::<Vec<_>>();
                        let c_args = if rendered.is_empty() {
                            String::new()
                        } else {
                            format!(", {}", rendered.join(", "))
                        };
                        self.line(
                            state,
                            format!("{} = {}(ctx, f->input{});", dest.c_name, cfn, c_args),
                        );
                        for a in args {
                            if is_owned_ty(a.ty) {
                                self.line(state, format!("{} = {};", a.c_name, c_empty(a.ty)));
                            }
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    _ if self.async_fn_new_names.contains_key(head) => {
                        let cfn = self
                            .async_fn_new_names
                            .get(head)
                            .cloned()
                            .unwrap_or_default();
                        let rendered = args.iter().map(|a| a.c_name.clone()).collect::<Vec<_>>();
                        let c_args = if rendered.is_empty() {
                            String::new()
                        } else {
                            format!(", {}", rendered.join(", "))
                        };
                        self.line(
                            state,
                            format!("{} = {}(ctx, f->input{});", dest.c_name, cfn, c_args),
                        );
                        for a in args {
                            if is_owned_ty(a.ty) {
                                self.line(state, format!("{} = {};", a.c_name, c_empty(a.ty)));
                            }
                        }
                        self.line(state, format!("goto st_{cont};"));
                        return Ok(());
                    }
                    _ => {}
                }

                Err(CompilerError::new(
                    CompileErrorKind::Unsupported,
                    format!("unsupported head in async: {head:?}"),
                ))
            }

            fn emit_unsafe(
                &mut self,
                state: usize,
                exprs: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if !self.options.allow_unsafe() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        "unsafe is not allowed in this world".to_string(),
                    ));
                }
                if exprs.is_empty() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "(unsafe ...) requires at least 1 expression".to_string(),
                    ));
                }
                let prev = self.unsafe_depth;
                self.unsafe_depth = self.unsafe_depth.saturating_add(1);
                let res = self.emit_begin(state, exprs, dest, cont);
                self.unsafe_depth = prev;
                res
            }

            fn emit_begin(
                &mut self,
                state: usize,
                exprs: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if exprs.is_empty() {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "(begin ...) requires at least 1 expression".to_string(),
                    ));
                }
                self.line(state, "rt_fuel(ctx, 1);");
                self.push_scope();

                let mut states = Vec::with_capacity(exprs.len());
                for _ in exprs {
                    states.push(self.new_state());
                }

                self.line(state, format!("goto st_{};", states[0]));

                for (i, e) in exprs.iter().enumerate() {
                    let s = states[i];
                    let next = if i + 1 < exprs.len() {
                        states[i + 1]
                    } else {
                        cont
                    };
                    if i + 1 == exprs.len() {
                        self.emit_expr_entry(s, e, dest.clone(), next)?;
                    } else {
                        let ty = self.infer_expr(e)?;
                        let storage_ty = match ty {
                            Ty::I32 | Ty::Never => Ty::I32,
                            Ty::Bytes => Ty::Bytes,
                            Ty::BytesView => Ty::BytesView,
                            Ty::VecU8 => Ty::VecU8,
                            Ty::OptionI32 => Ty::OptionI32,
                            Ty::OptionBytes => Ty::OptionBytes,
                            Ty::ResultI32 => Ty::ResultI32,
                            Ty::ResultBytes => Ty::ResultBytes,
                            Ty::Iface => Ty::Iface,
                            Ty::PtrConstU8 => Ty::PtrConstU8,
                            Ty::PtrMutU8 => Ty::PtrMutU8,
                            Ty::PtrConstVoid => Ty::PtrConstVoid,
                            Ty::PtrMutVoid => Ty::PtrMutVoid,
                            Ty::PtrConstI32 => Ty::PtrConstI32,
                            Ty::PtrMutI32 => Ty::PtrMutI32,
                        };
                        let tmp = self.alloc_local("t_begin_", storage_ty)?;
                        self.emit_expr_entry(s, e, tmp, next)?;
                    }
                }

                self.pop_scope();
                Ok(())
            }

            fn emit_let(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
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
                let expr_ty = self.infer_expr(&args[1])?;
                if expr_ty != dest.ty && expr_ty != Ty::Never {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("let expression must match context type {:?}", dest.ty),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");
                let storage_ty = match expr_ty {
                    Ty::I32 | Ty::Never => Ty::I32,
                    Ty::Bytes => Ty::Bytes,
                    Ty::BytesView => Ty::BytesView,
                    Ty::VecU8 => Ty::VecU8,
                    Ty::OptionI32 => Ty::OptionI32,
                    Ty::OptionBytes => Ty::OptionBytes,
                    Ty::ResultI32 => Ty::ResultI32,
                    Ty::ResultBytes => Ty::ResultBytes,
                    Ty::Iface => Ty::Iface,
                    Ty::PtrConstU8 => Ty::PtrConstU8,
                    Ty::PtrMutU8 => Ty::PtrMutU8,
                    Ty::PtrConstVoid => Ty::PtrConstVoid,
                    Ty::PtrMutVoid => Ty::PtrMutVoid,
                    Ty::PtrConstI32 => Ty::PtrConstI32,
                    Ty::PtrMutI32 => Ty::PtrMutI32,
                };
                let binding = self.alloc_local("v_", storage_ty)?;

                let expr_state = self.new_state();
                let after = self.new_state();
                self.line(state, format!("goto st_{expr_state};"));
                self.emit_expr_entry(expr_state, &args[1], binding.clone(), after)?;

                self.bind(
                    name.to_string(),
                    AsyncVarRef {
                        ty: expr_ty,
                        c_name: binding.c_name.clone(),
                        moved: false,
                    },
                );

                if binding.c_name != dest.c_name && expr_ty != Ty::Never {
                    if is_owned_ty(expr_ty) {
                        self.line(after, format!("{} = {};", dest.c_name, c_empty(expr_ty)));
                    } else {
                        self.line(after, format!("{} = {};", dest.c_name, binding.c_name));
                    }
                }
                self.line(after, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_set(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
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
                let Some(var) = self.lookup(name) else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("set of unknown variable: {name:?}"),
                    ));
                };
                if var.ty != dest.ty && var.ty != Ty::Never {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("set expression must match context type {:?}", dest.ty),
                    ));
                }
                let expr_ty = self.infer_expr(&args[1])?;
                if expr_ty != var.ty && expr_ty != Ty::Never {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("type mismatch in set for variable {name:?}"),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");
                let tmp = self.alloc_local("t_set_", var.ty)?;
                let expr_state = self.new_state();
                let after = self.new_state();
                self.line(state, format!("goto st_{expr_state};"));
                self.emit_expr_entry(expr_state, &args[1], tmp.clone(), after)?;
                if is_owned_ty(var.ty) {
                    match var.ty {
                        Ty::Bytes => {
                            self.line(after, format!("rt_bytes_drop(ctx, &{});", var.c_name))
                        }
                        Ty::VecU8 => {
                            self.line(after, format!("rt_vec_u8_drop(ctx, &{});", var.c_name))
                        }
                        Ty::OptionBytes => {
                            self.line(after, format!("if ({}.tag) {{", var.c_name));
                            self.line(
                                after,
                                format!("  rt_bytes_drop(ctx, &{}.payload);", var.c_name),
                            );
                            self.line(after, "}");
                            self.line(after, format!("{}.tag = UINT32_C(0);", var.c_name));
                        }
                        Ty::ResultBytes => {
                            self.line(after, format!("if ({}.tag) {{", var.c_name));
                            self.line(
                                after,
                                format!("  rt_bytes_drop(ctx, &{}.payload.ok);", var.c_name),
                            );
                            self.line(after, "}");
                            self.line(after, format!("{}.tag = UINT32_C(0);", var.c_name));
                            self.line(after, format!("{}.payload.err = UINT32_C(0);", var.c_name));
                        }
                        _ => {}
                    }
                }
                self.line(after, format!("{} = {};", var.c_name, tmp.c_name));
                if is_owned_ty(var.ty) {
                    self.line(after, format!("{} = {};", tmp.c_name, c_empty(var.ty)));
                }
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = false;
                }
                if var.c_name != dest.c_name && var.ty != Ty::Never {
                    if is_owned_ty(var.ty) {
                        self.line(after, format!("{} = {};", dest.c_name, c_empty(var.ty)));
                    } else {
                        self.line(after, format!("{} = {};", dest.c_name, var.c_name));
                    }
                }
                self.line(after, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_if(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 3 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                    ));
                }
                self.line(state, "rt_fuel(ctx, 1);");
                let cond_tmp = self.alloc_local("t_if_", Ty::I32)?;
                let cond_state = self.new_state();
                let branch_state = self.new_state();
                let then_state = self.new_state();
                let else_state = self.new_state();

                self.line(state, format!("goto st_{cond_state};"));
                self.emit_expr_entry(cond_state, &args[0], cond_tmp.clone(), branch_state)?;

                self.line(
                    branch_state,
                    format!(
                        "if ({} != UINT32_C(0)) goto st_{then_state}; else goto st_{else_state};",
                        cond_tmp.c_name
                    ),
                );

                self.push_scope();
                self.emit_expr_entry(then_state, &args[1], dest.clone(), cont)?;
                self.pop_scope();

                self.push_scope();
                self.emit_expr_entry(else_state, &args[2], dest, cont)?;
                self.pop_scope();

                Ok(())
            }

            fn emit_for(
                &mut self,
                state: usize,
                args: &[Expr],
                dest: AsyncVarRef,
                cont: usize,
            ) -> Result<(), CompilerError> {
                if args.len() != 4 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "for form: (for <var> <start:i32> <end:i32> <body:any>)".to_string(),
                    ));
                }
                if dest.ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "for returns i32".to_string(),
                    ));
                }
                let var_name = args[0].as_ident().ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "for variable must be an identifier".to_string(),
                    )
                })?;

                let (var, var_ty) = match self.lookup(var_name) {
                    Some(v) => (v.c_name, v.ty),
                    None => {
                        let v = self.alloc_local("v_for_", Ty::I32)?;
                        self.bind(var_name.to_string(), v.clone());
                        (v.c_name, Ty::I32)
                    }
                };
                if var_ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("for variable must be i32: {var_name:?}"),
                    ));
                }

                self.line(state, "rt_fuel(ctx, 1);");
                let end_tmp = self.alloc_local("t_for_end_", Ty::I32)?;
                let start_state = self.new_state();
                let end_state = self.new_state();
                let loop_check = self.new_state();
                let body_state = self.new_state();
                let inc_state = self.new_state();
                let done_state = self.new_state();

                self.line(state, format!("goto st_{start_state};"));
                self.emit_expr_entry(
                    start_state,
                    &args[1],
                    AsyncVarRef {
                        ty: Ty::I32,
                        c_name: var.clone(),
                        moved: false,
                    },
                    end_state,
                )?;
                self.emit_expr_entry(end_state, &args[2], end_tmp.clone(), loop_check)?;

                self.line(
                    loop_check,
                    format!("if ({var} >= {}) goto st_{done_state};", end_tmp.c_name),
                );
                self.line(loop_check, format!("goto st_{body_state};"));

                self.push_scope();
                let body_ty = self.infer_expr(&args[3])?;
                let body_storage = match body_ty {
                    Ty::I32 | Ty::Never => Ty::I32,
                    Ty::Bytes => Ty::Bytes,
                    Ty::BytesView => Ty::BytesView,
                    Ty::VecU8 => Ty::VecU8,
                    Ty::OptionI32 => Ty::OptionI32,
                    Ty::OptionBytes => Ty::OptionBytes,
                    Ty::ResultI32 => Ty::ResultI32,
                    Ty::ResultBytes => Ty::ResultBytes,
                    Ty::Iface => Ty::Iface,
                    Ty::PtrConstU8 => Ty::PtrConstU8,
                    Ty::PtrMutU8 => Ty::PtrMutU8,
                    Ty::PtrConstVoid => Ty::PtrConstVoid,
                    Ty::PtrMutVoid => Ty::PtrMutVoid,
                    Ty::PtrConstI32 => Ty::PtrConstI32,
                    Ty::PtrMutI32 => Ty::PtrMutI32,
                };
                let body_tmp = self.alloc_local("t_for_body_", body_storage)?;
                self.emit_expr_entry(body_state, &args[3], body_tmp, inc_state)?;
                self.pop_scope();

                self.line(inc_state, format!("{var} = {var} + UINT32_C(1);"));
                self.line(inc_state, format!("goto st_{loop_check};"));

                self.line(done_state, format!("{} = UINT32_C(0);", dest.c_name));
                self.line(done_state, format!("goto st_{cont};"));
                Ok(())
            }

            fn emit_return(&mut self, state: usize, args: &[Expr]) -> Result<(), CompilerError> {
                if args.len() != 1 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Parse,
                        "return form: (return <expr>)".to_string(),
                    ));
                }
                self.line(state, "rt_fuel(ctx, 1);");
                let expr_state = self.new_state();
                self.line(state, format!("goto st_{expr_state};"));
                self.emit_expr_entry(
                    expr_state,
                    &args[0],
                    AsyncVarRef {
                        ty: Ty::Bytes,
                        c_name: "f->ret".to_string(),
                        moved: false,
                    },
                    self.ret_state,
                )?;
                Ok(())
            }
        }

        let mut machine = Machine {
            options: self.options.clone(),
            functions,
            extern_functions: self.extern_functions.clone(),
            fn_c_names: self.fn_c_names.clone(),
            async_fn_new_names: self.async_fn_new_names.clone(),
            fields,
            tmp_counter: 0,
            local_count: 0,
            unsafe_depth: 0,
            scopes: vec![BTreeMap::new()],
            states: Vec::new(),
            ret_state: 0,
            fn_name: f.name.clone(),
        };

        for (i, p) in f.params.iter().enumerate() {
            machine.bind(
                p.name.clone(),
                AsyncVarRef {
                    ty: p.ty,
                    c_name: format!("f->p{i}"),
                    moved: false,
                },
            );
        }

        let start = machine.new_state();
        let ret_state = machine.new_state();
        machine.ret_state = ret_state;

        machine.emit_expr_entry(
            start,
            &f.body,
            AsyncVarRef {
                ty: Ty::Bytes,
                c_name: "f->ret".to_string(),
                moved: false,
            },
            ret_state,
        )?;

        machine.line(ret_state, "*out = f->ret;");
        machine.line(ret_state, "f->ret = rt_bytes_empty(ctx);");
        machine.line(ret_state, "return UINT32_C(1);");

        self.line("typedef struct {");
        self.indent += 1;
        for (name, ty) in &machine.fields {
            self.line(&format!("{} {};", c_ret_ty(*ty), name));
        }
        self.indent -= 1;
        self.line(&format!("}} {fut_type};"));
        self.push_char('\n');

        self.line(&format!(
            "static uint32_t {poll_name}(ctx_t* ctx, void* fut, bytes_t* out) {{"
        ));
        self.indent += 1;
        self.line(&format!("{fut_type}* f = ({fut_type}*)fut;"));
        self.line("switch (f->state) {");
        self.indent += 1;
        for i in 0..machine.states.len() {
            self.line(&format!("case UINT32_C({i}): goto st_{i};"));
        }
        self.line("default: rt_trap(\"bad async state\");");
        self.indent -= 1;
        self.line("}");
        for (i, lines) in machine.states.iter().enumerate() {
            self.line(&format!("st_{i}: ;"));
            for l in lines {
                self.line(l);
            }
        }
        self.indent -= 1;
        self.line("}");
        self.push_char('\n');

        self.line(&format!(
            "static void {drop_name}(ctx_t* ctx, void* fut) {{"
        ));
        self.indent += 1;
        self.line("if (!fut) return;");
        self.line(&format!("{fut_type}* f = ({fut_type}*)fut;"));
        for (name, ty) in &machine.fields {
            if !is_owned_ty(*ty) {
                continue;
            }
            let field = format!("f->{name}");
            match *ty {
                Ty::Bytes => self.line(&format!("rt_bytes_drop(ctx, &{field});")),
                Ty::VecU8 => self.line(&format!("rt_vec_u8_drop(ctx, &{field});")),
                Ty::OptionBytes => {
                    self.line(&format!("if ({field}.tag) {{"));
                    self.indent += 1;
                    self.line(&format!("rt_bytes_drop(ctx, &{field}.payload);"));
                    self.indent -= 1;
                    self.line("}");
                    self.line(&format!("{field}.tag = UINT32_C(0);"));
                }
                Ty::ResultBytes => {
                    self.line(&format!("if ({field}.tag) {{"));
                    self.indent += 1;
                    self.line(&format!("rt_bytes_drop(ctx, &{field}.payload.ok);"));
                    self.indent -= 1;
                    self.line("}");
                    self.line(&format!("{field}.tag = UINT32_C(0);"));
                    self.line(&format!("{field}.payload.err = UINT32_C(0);"));
                }
                _ => {}
            }
        }
        self.line(&format!(
            "rt_free(ctx, f, (uint32_t)sizeof({fut_type}), (uint32_t)_Alignof({fut_type}));"
        ));
        self.indent -= 1;
        self.line("}");
        self.push_char('\n');

        self.line(&format!(
            "static uint32_t {new_name}(ctx_t* ctx, bytes_view_t input{}) {{",
            c_param_list_value(&f.params)
        ));
        self.indent += 1;
        self.line(&format!(
            "{fut_type}* f = ({fut_type}*)rt_alloc(ctx, (uint32_t)sizeof({fut_type}), (uint32_t)_Alignof({fut_type}));"
        ));
        self.line(&format!("memset(f, 0, sizeof({fut_type}));"));
        self.line("f->state = UINT32_C(0);");
        self.line("f->input = input;");
        for (i, _p) in f.params.iter().enumerate() {
            self.line(&format!("f->p{i} = p{i};"));
        }
        self.line(&format!(
            "return rt_task_create(ctx, {poll_name}, {drop_name}, f);"
        ));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    fn emit_user_function(&mut self, f: &FunctionDef) -> Result<(), CompilerError> {
        self.reset_fn_state();
        self.current_fn_name = Some(f.name.clone());
        self.fn_ret_ty = f.ret_ty;
        self.allow_async_ops = false;
        self.emit_source_line_for_symbol(&f.name);

        if f.ret_ty != Ty::I32
            && f.ret_ty != Ty::Bytes
            && f.ret_ty != Ty::BytesView
            && f.ret_ty != Ty::VecU8
            && f.ret_ty != Ty::OptionI32
            && f.ret_ty != Ty::OptionBytes
            && f.ret_ty != Ty::ResultI32
            && f.ret_ty != Ty::ResultBytes
            && f.ret_ty != Ty::Iface
        {
            return Err(CompilerError::new(
                CompileErrorKind::Internal,
                format!("invalid function return type: {:?}", f.ret_ty),
            ));
        }

        self.line(&format!(
            "static {} {}(ctx_t* ctx, bytes_view_t input{}) {{",
            c_ret_ty(f.ret_ty),
            self.fn_c_name(&f.name),
            c_param_list_user(&f.params)
        ));
        self.indent += 1;

        for (i, p) in f.params.iter().enumerate() {
            self.bind(
                p.name.clone(),
                self.make_var_ref(p.ty, format!("p{i}"), false),
            );
        }

        let result_ty = self.infer_expr_in_new_scope(&f.body)?;
        if result_ty != f.ret_ty && result_ty != Ty::Never {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!(
                    "function {:?} must evaluate to {:?} (or return), got {:?}",
                    f.name, f.ret_ty, result_ty
                ),
            ));
        }

        self.line(&format!(
            "{} out = {};",
            c_ret_ty(f.ret_ty),
            c_zero(f.ret_ty)
        ));
        self.emit_expr_to(&f.body, f.ret_ty, "out")?;
        for (ty, c_name) in self.live_owned_drop_list(None) {
            self.emit_drop_var(ty, &c_name);
        }
        self.line("return out;");

        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    fn emit_solve(&mut self) -> Result<(), CompilerError> {
        self.reset_fn_state();
        self.current_fn_name = Some("solve".to_string());
        self.fn_ret_ty = Ty::Bytes;
        self.allow_async_ops = true;
        self.emit_source_line_for_module("main");

        let result_ty = self.infer_expr_in_new_scope(&self.program.solve)?;
        if result_ty != Ty::Bytes && result_ty != Ty::Never {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("solve expression must evaluate to bytes (or return), got {result_ty:?}"),
            ));
        }

        self.push_str("static bytes_t solve(ctx_t* ctx, bytes_view_t input) {\n");
        self.indent += 1;
        self.line("bytes_t out = rt_bytes_empty(ctx);");
        self.emit_expr_to(&self.program.solve, Ty::Bytes, "out")?;
        for (ty, c_name) in self.live_owned_drop_list(None) {
            self.emit_drop_var(ty, &c_name);
        }
        self.line("return out;");
        self.indent -= 1;
        self.push_str("}\n\n");
        Ok(())
    }

    fn reset_fn_state(&mut self) {
        self.indent = 0;
        self.tmp_counter = 0;
        self.local_count = 0;
        self.scopes.clear();
        self.scopes.push(BTreeMap::new());
        self.allow_async_ops = false;
        self.unsafe_depth = 0;
        self.current_fn_name = None;
    }

    fn emit_source_line_for_symbol(&mut self, sym: &str) {
        let module_id = sym.rsplit_once('.').map(|(m, _)| m).unwrap_or(sym);
        self.emit_source_line_for_module(module_id);
    }

    fn emit_source_line_for_module(&mut self, module_id: &str) {
        let file = module_id.replace('.', "/") + ".x07.json";
        self.line(&format!("#line 1 \"{}\"", c_escape_c_string(&file)));
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) -> Result<(), CompilerError> {
        let Some(mut scope) = self.scopes.pop() else {
            return Ok(());
        };

        // Release borrows from views in this scope first so owned values can be dropped safely.
        let mut release = Vec::<String>::new();
        for v in scope.values() {
            if v.ty == Ty::BytesView {
                if let Some(owner) = &v.borrow_of {
                    release.push(owner.clone());
                }
            }
        }
        for owner in release {
            let mut found = false;
            for v in scope.values_mut() {
                if v.c_name == owner {
                    v.borrow_count = v.borrow_count.saturating_sub(1);
                    found = true;
                    break;
                }
            }
            if !found {
                self.dec_borrow_count(&owner)?;
            }
        }

        // Drop owned values in this scope. Moves clear the source to an empty value, so dropping
        // moved-from locals is safe and required to avoid leaks across control-flow merges.
        for v in scope.values() {
            if !is_owned_ty(v.ty) {
                continue;
            }
            if v.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("drop while borrowed: {:?}", v.c_name),
                ));
            }
            self.emit_drop_var(v.ty, &v.c_name);
        }

        Ok(())
    }

    fn bind(&mut self, name: String, var: VarRef) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, var);
        }
    }

    fn lookup(&self, name: &str) -> Option<&VarRef> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut VarRef> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                return scope.get_mut(name);
            }
        }
        None
    }

    fn alloc_local(&mut self, prefix: &str) -> Result<String, CompilerError> {
        if self.local_count >= language::limits::MAX_LOCALS {
            let msg = match &self.current_fn_name {
                Some(name) => format!(
                    "max locals exceeded: {} (fn={})",
                    language::limits::MAX_LOCALS,
                    name
                ),
                None => format!("max locals exceeded: {}", language::limits::MAX_LOCALS),
            };
            return Err(CompilerError::new(CompileErrorKind::Budget, msg));
        }
        self.local_count += 1;
        self.tmp_counter += 1;
        Ok(format!("{prefix}{}", self.tmp_counter))
    }

    fn decl_local(&mut self, ty: Ty, name: &str) {
        match ty {
            Ty::I32 => self.line(&format!("uint32_t {name} = UINT32_C(0);")),
            Ty::Bytes => self.line(&format!("bytes_t {name} = rt_bytes_empty(ctx);")),
            Ty::BytesView => self.line(&format!("bytes_view_t {name} = rt_view_empty(ctx);")),
            Ty::VecU8 => self.line(&format!("vec_u8_t {name} = (vec_u8_t){{0}};")),
            Ty::OptionI32 => self.line(&format!("option_i32_t {name} = (option_i32_t){{0}};")),
            Ty::OptionBytes => {
                self.line(&format!("option_bytes_t {name} = (option_bytes_t){{0}};"))
            }
            Ty::ResultI32 => self.line(&format!("result_i32_t {name} = (result_i32_t){{0}};")),
            Ty::ResultBytes => {
                self.line(&format!("result_bytes_t {name} = (result_bytes_t){{0}};"))
            }
            Ty::Iface => self.line(&format!("iface_t {name} = (iface_t){{0}};")),
            Ty::PtrConstU8 => self.line(&format!("const uint8_t* {name} = NULL;")),
            Ty::PtrMutU8 => self.line(&format!("uint8_t* {name} = NULL;")),
            Ty::PtrConstVoid => self.line(&format!("const void* {name} = NULL;")),
            Ty::PtrMutVoid => self.line(&format!("void* {name} = NULL;")),
            Ty::PtrConstI32 => self.line(&format!("const uint32_t* {name} = NULL;")),
            Ty::PtrMutI32 => self.line(&format!("uint32_t* {name} = NULL;")),
            Ty::Never => self.line(&format!("uint32_t {name} = UINT32_C(0);")),
        }
    }

    fn line(&mut self, s: &str) {
        if self.suppress_output {
            return;
        }
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn open_block(&mut self) {
        self.line("{");
        self.indent += 1;
    }

    fn close_block(&mut self) {
        self.indent = self.indent.saturating_sub(1);
        self.line("}");
    }

    fn emit_expr(&mut self, expr: &Expr) -> Result<VarRef, CompilerError> {
        let ty = self.infer_expr_in_new_scope(expr)?;
        let (storage_ty, name) = match ty {
            Ty::I32 => (Ty::I32, self.alloc_local("t_i32_")?),
            Ty::Bytes => (Ty::Bytes, self.alloc_local("t_bytes_")?),
            Ty::BytesView => (Ty::BytesView, self.alloc_local("t_view_")?),
            Ty::VecU8 => (Ty::VecU8, self.alloc_local("t_vec_u8_")?),
            Ty::OptionI32 => (Ty::OptionI32, self.alloc_local("t_opt_i32_")?),
            Ty::OptionBytes => (Ty::OptionBytes, self.alloc_local("t_opt_bytes_")?),
            Ty::ResultI32 => (Ty::ResultI32, self.alloc_local("t_res_i32_")?),
            Ty::ResultBytes => (Ty::ResultBytes, self.alloc_local("t_res_bytes_")?),
            Ty::Iface => (Ty::Iface, self.alloc_local("t_iface_")?),
            Ty::PtrConstU8 => (Ty::PtrConstU8, self.alloc_local("t_ptr_")?),
            Ty::PtrMutU8 => (Ty::PtrMutU8, self.alloc_local("t_ptr_")?),
            Ty::PtrConstVoid => (Ty::PtrConstVoid, self.alloc_local("t_ptr_")?),
            Ty::PtrMutVoid => (Ty::PtrMutVoid, self.alloc_local("t_ptr_")?),
            Ty::PtrConstI32 => (Ty::PtrConstI32, self.alloc_local("t_ptr_")?),
            Ty::PtrMutI32 => (Ty::PtrMutI32, self.alloc_local("t_ptr_")?),
            Ty::Never => (Ty::I32, self.alloc_local("t_never_")?),
        };
        self.decl_local(storage_ty, &name);
        self.emit_expr_to(expr, storage_ty, &name)?;

        let mut v = self.make_var_ref(ty, name.clone(), true);
        if ty == Ty::BytesView {
            let borrow_of = self.borrow_of_view_expr(expr)?;
            if let Some(owner) = &borrow_of {
                self.inc_borrow_count(owner)?;
            }
            v.borrow_of = borrow_of;
        }

        if is_owned_ty(ty) || ty == Ty::BytesView {
            self.bind(format!("#tmp:{name}"), v.clone());
        }

        Ok(v)
    }

    fn emit_expr_as_bytes_view(&mut self, expr: &Expr) -> Result<VarRef, CompilerError> {
        let ty = self.infer_expr_in_new_scope(expr)?;
        match ty {
            Ty::BytesView => self.emit_expr(expr),
            Ty::Bytes => match expr {
                Expr::Ident(name) if name != "input" => {
                    let Some(owner) = self.lookup(name).cloned() else {
                        return Err(self.err(
                            CompileErrorKind::Typing,
                            format!("unknown identifier: {name:?}"),
                        ));
                    };
                    if owner.moved {
                        return Err(self.err(
                            CompileErrorKind::Typing,
                            format!("use after move: {name:?}"),
                        ));
                    }
                    if owner.ty != Ty::Bytes {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("type mismatch for identifier {name:?}"),
                        ));
                    }

                    let tmp = self.alloc_local("t_view_")?;
                    self.decl_local(Ty::BytesView, &tmp);
                    self.line(&format!("{tmp} = rt_bytes_view(ctx, {});", owner.c_name));
                    self.inc_borrow_count(&owner.c_name)?;
                    let mut view = self.make_var_ref(Ty::BytesView, tmp.clone(), true);
                    view.borrow_of = Some(owner.c_name);
                    self.bind(format!("#tmp:{tmp}"), view.clone());
                    Ok(view)
                }
                _ => {
                    let owner = self.emit_expr(expr)?;
                    if owner.ty != Ty::Bytes {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("expected bytes, got {:?}", owner.ty),
                        ));
                    }
                    let tmp = self.alloc_local("t_view_")?;
                    self.decl_local(Ty::BytesView, &tmp);
                    self.line(&format!("{tmp} = rt_bytes_view(ctx, {});", owner.c_name));
                    self.inc_borrow_count(&owner.c_name)?;
                    let mut view = self.make_var_ref(Ty::BytesView, tmp.clone(), true);
                    view.borrow_of = Some(owner.c_name);
                    self.bind(format!("#tmp:{tmp}"), view.clone());
                    Ok(view)
                }
            },
            Ty::VecU8 => match expr {
                Expr::Ident(name) => {
                    let Some(owner) = self.lookup(name).cloned() else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("unknown identifier: {name:?}"),
                        ));
                    };
                    if owner.moved {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("use after move: {name:?}"),
                        ));
                    }
                    if owner.ty != Ty::VecU8 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("type mismatch for identifier {name:?}"),
                        ));
                    }

                    let tmp = self.alloc_local("t_view_")?;
                    self.decl_local(Ty::BytesView, &tmp);
                    self.line(&format!(
                        "{tmp} = rt_vec_u8_as_view(ctx, {});",
                        owner.c_name
                    ));
                    self.inc_borrow_count(&owner.c_name)?;
                    let mut view = self.make_var_ref(Ty::BytesView, tmp.clone(), true);
                    view.borrow_of = Some(owner.c_name);
                    self.bind(format!("#tmp:{tmp}"), view.clone());
                    Ok(view)
                }
                _ => {
                    let owner = self.emit_expr(expr)?;
                    if owner.ty != Ty::VecU8 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("expected vec_u8, got {:?}", owner.ty),
                        ));
                    }
                    let tmp = self.alloc_local("t_view_")?;
                    self.decl_local(Ty::BytesView, &tmp);
                    self.line(&format!(
                        "{tmp} = rt_vec_u8_as_view(ctx, {});",
                        owner.c_name
                    ));
                    self.inc_borrow_count(&owner.c_name)?;
                    let mut view = self.make_var_ref(Ty::BytesView, tmp.clone(), true);
                    view.borrow_of = Some(owner.c_name);
                    self.bind(format!("#tmp:{tmp}"), view.clone());
                    Ok(view)
                }
            },
            other => Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("expected bytes/bytes_view/vec_u8, got {other:?}"),
            )),
        }
    }

    fn emit_stmt(&mut self, expr: &Expr) -> Result<(), CompilerError> {
        match expr {
            Expr::List(items) => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];

                match head {
                    "begin" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "(begin ...) requires at least 1 expression".to_string(),
                            ));
                        }
                        self.push_scope();
                        self.open_block();
                        for e in args {
                            self.emit_stmt(e)?;
                        }
                        self.pop_scope()?;
                        self.close_block();
                        Ok(())
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }

                        let cond = self.emit_expr(&args[0])?;
                        if cond.ty != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "if condition must be i32".to_string(),
                            ));
                        }

                        let then_ty = self.infer_expr_in_new_scope(&args[1])?;
                        let else_ty = self.infer_expr_in_new_scope(&args[2])?;
                        let scopes_before = self.scopes.clone();

                        self.line(&format!("if ({} != UINT32_C(0)) {{", cond.c_name));
                        self.indent += 1;
                        self.push_scope();
                        self.emit_stmt(&args[1])?;
                        self.pop_scope()?;
                        let scopes_then = self.scopes.clone();
                        self.indent -= 1;
                        self.line("} else {");
                        self.indent += 1;
                        self.scopes = scopes_before.clone();
                        self.push_scope();
                        self.emit_stmt(&args[2])?;
                        self.pop_scope()?;
                        let scopes_else = self.scopes.clone();
                        self.indent -= 1;
                        self.line("}");

                        if then_ty == Ty::Never && else_ty == Ty::Never {
                            self.scopes = scopes_before;
                        } else if then_ty == Ty::Never {
                            self.scopes = scopes_else;
                        } else if else_ty == Ty::Never {
                            self.scopes = scopes_then;
                        } else {
                            self.scopes =
                                self.merge_if_states(&scopes_before, &scopes_then, &scopes_else)?;
                            self.recompute_borrow_counts()?;
                        }
                        Ok(())
                    }
                    "let" => self.emit_let_stmt(args),
                    "set" => self.emit_set_stmt(args),
                    "for" => {
                        let tmp = self.alloc_local("t_i32_")?;
                        self.decl_local(Ty::I32, &tmp);
                        self.emit_for_to(args, Ty::I32, &tmp)
                    }
                    "return" => self.emit_return(args),
                    _ => {
                        let _ = self.emit_expr(expr)?;
                        Ok(())
                    }
                }
            }
            _ => {
                let _ = self.emit_expr(expr)?;
                Ok(())
            }
        }
    }

    fn emit_let_stmt(&mut self, args: &[Expr]) -> Result<(), CompilerError> {
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

        if self.scopes.last().and_then(|s| s.get(name)).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("duplicate let binding in same scope: {name:?}"),
            ));
        }

        let expr_ty = self.infer_expr_in_new_scope(&args[1])?;
        let c_name = self.alloc_local("v_")?;
        self.decl_local(expr_ty, &c_name);

        let mut var = self.make_var_ref(expr_ty, c_name.clone(), false);
        if is_owned_ty(expr_ty) {
            match &args[1] {
                Expr::Ident(src_name) if src_name != "input" => {
                    let Some(src) = self.lookup(src_name).cloned() else {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("unknown identifier: {src_name:?}"),
                        ));
                    };
                    if src.ty != expr_ty {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("type mismatch in move: {src_name:?}"),
                        ));
                    }
                    if src.moved {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("use after move: {src_name:?}"),
                        ));
                    }
                    if src.borrow_count != 0 {
                        return Err(CompilerError::new(
                            CompileErrorKind::Typing,
                            format!("move while borrowed: {src_name:?}"),
                        ));
                    }
                    self.line(&format!("{c_name} = {};", src.c_name));
                    self.line(&format!("{} = {};", src.c_name, c_empty(expr_ty)));
                    if let Some(src_mut) = self.lookup_mut(src_name) {
                        src_mut.moved = true;
                    }
                }
                _ => {
                    self.emit_expr_to(&args[1], expr_ty, &c_name)?;
                }
            }
        } else {
            self.emit_expr_to(&args[1], expr_ty, &c_name)?;
        }

        if expr_ty == Ty::BytesView {
            let borrow_of = self.borrow_of_view_expr(&args[1])?;
            if let Some(owner) = &borrow_of {
                self.inc_borrow_count(owner)?;
            }
            var.borrow_of = borrow_of;
        }

        self.bind(name.to_string(), var);
        Ok(())
    }

    fn emit_set_stmt(&mut self, args: &[Expr]) -> Result<(), CompilerError> {
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
        let Some(dst) = self.lookup(name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("set of unknown variable: {name:?}"),
            ));
        };

        if is_owned_ty(dst.ty) {
            if dst.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("set while borrowed: {name:?}"),
                ));
            }

            let tmp = self.alloc_local("t_set_")?;
            self.decl_local(dst.ty, &tmp);
            self.emit_expr_to(&args[1], dst.ty, &tmp)?;

            // Drop the previous value (unless it was moved-out during RHS evaluation).
            let moved = self.lookup(name).map(|v| v.moved).unwrap_or(false);
            if !moved {
                self.emit_drop_var(dst.ty, &dst.c_name);
            }
            self.line(&format!("{} = {};", dst.c_name, tmp));
            self.line(&format!("{tmp} = {};", c_empty(dst.ty)));

            if let Some(v) = self.lookup_mut(name) {
                v.moved = false;
            }
        } else if dst.ty == Ty::BytesView {
            let tmp = self.alloc_local("t_view_")?;
            self.decl_local(Ty::BytesView, &tmp);
            self.emit_expr_to(&args[1], Ty::BytesView, &tmp)?;
            let new_borrow_of = self.borrow_of_view_expr(&args[1])?;

            let old_borrow_of = dst.borrow_of.clone();
            if let Some(owner) = &old_borrow_of {
                self.dec_borrow_count(owner)?;
            }
            if let Some(owner) = &new_borrow_of {
                self.inc_borrow_count(owner)?;
            }
            if let Some(v) = self.lookup_mut(name) {
                v.borrow_of = new_borrow_of;
            }

            self.line(&format!("{} = {tmp};", dst.c_name));
        } else {
            self.emit_expr_to(&args[1], dst.ty, &dst.c_name)?;
        }
        Ok(())
    }

    fn emit_expr_to(&mut self, expr: &Expr, dest_ty: Ty, dest: &str) -> Result<(), CompilerError> {
        self.line("rt_fuel(ctx, 1);");
        match expr {
            Expr::Int(i) => {
                if dest_ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "int literal used where bytes expected".to_string(),
                    ));
                }
                let v = *i as u32;
                self.line(&format!("{dest} = UINT32_C({v});"));
                Ok(())
            }
            Expr::Ident(name) => self.emit_ident_to(name, dest_ty, dest),
            Expr::List(items) => self.emit_list_to(items, dest_ty, dest),
        }
    }

    fn emit_ident_to(&mut self, name: &str, dest_ty: Ty, dest: &str) -> Result<(), CompilerError> {
        if name == "input" {
            if dest_ty != Ty::BytesView {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "input is bytes_view".to_string(),
                ));
            }
            self.line(&format!("{dest} = input;"));
            return Ok(());
        }

        let Some(var) = self.lookup(name).cloned() else {
            return Err(self.err(
                CompileErrorKind::Typing,
                format!("unknown identifier: {name:?}"),
            ));
        };
        if var.moved {
            return Err(self.err(
                CompileErrorKind::Typing,
                format!("use after move: {name:?}"),
            ));
        }
        if var.ty != dest_ty {
            return Err(self.err(
                CompileErrorKind::Typing,
                format!("type mismatch for identifier {name:?}"),
            ));
        }
        if is_owned_ty(dest_ty) {
            if var.borrow_count != 0 {
                return Err(self.err(
                    CompileErrorKind::Typing,
                    format!("move while borrowed: {name:?}"),
                ));
            }
            self.line(&format!("{dest} = {};", var.c_name));
            self.line(&format!("{} = {};", var.c_name, c_empty(dest_ty)));
            if let Some(v) = self.lookup_mut(name) {
                v.moved = true;
            }
        } else {
            self.line(&format!("{dest} = {};", var.c_name));
        }
        Ok(())
    }

    fn emit_list_to(
        &mut self,
        items: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "list head must be an identifier".to_string(),
            )
        })?;
        let args = &items[1..];

        match head {
            "unsafe" => self.emit_unsafe_to(args, dest_ty, dest),
            "begin" => self.emit_begin_to(args, dest_ty, dest),
            "let" => self.emit_let_to(args, dest_ty, dest),
            "set" => self.emit_set_to(args, dest_ty, dest),
            "if" => self.emit_if_to(args, dest_ty, dest),
            "for" => self.emit_for_to(args, dest_ty, dest),
            "return" => self.emit_return(args),

            "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<u" | ">>u" | "=" | "!=" | "<"
            | "<=" | ">" | ">=" | "<u" | ">=u" | ">u" | "<=u" => {
                self.emit_binop_to(head, args, dest_ty, dest)
            }

            "bytes.len" => self.emit_bytes_len_to(args, dest_ty, dest),
            "bytes.get_u8" => self.emit_bytes_get_u8_to(args, dest_ty, dest),
            "bytes.set_u8" => self.emit_bytes_set_u8_to(args, dest_ty, dest),
            "bytes.alloc" => self.emit_bytes_alloc_to(args, dest_ty, dest),
            "bytes.empty" => self.emit_bytes_empty_to(args, dest_ty, dest),
            "bytes1" => self.emit_bytes1_to(args, dest_ty, dest),
            "bytes.lit" => self.emit_bytes_lit_to(args, dest_ty, dest),
            "bytes.slice" => self.emit_bytes_slice_to(args, dest_ty, dest),
            "bytes.copy" => self.emit_bytes_copy_to(args, dest_ty, dest),
            "bytes.concat" => self.emit_bytes_concat_to(args, dest_ty, dest),
            "bytes.eq" => self.emit_bytes_eq_to(args, dest_ty, dest),
            "bytes.cmp_range" => self.emit_bytes_cmp_range_to(args, dest_ty, dest),
            "bytes.as_ptr" => self.emit_bytes_as_ptr_to(args, dest_ty, dest),
            "bytes.as_mut_ptr" => self.emit_bytes_as_mut_ptr_to(args, dest_ty, dest),

            "math.f64.add_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_add_v1", args, dest_ty, dest)
            }
            "math.f64.sub_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_sub_v1", args, dest_ty, dest)
            }
            "math.f64.mul_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_mul_v1", args, dest_ty, dest)
            }
            "math.f64.div_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_div_v1", args, dest_ty, dest)
            }
            "math.f64.pow_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_pow_v1", args, dest_ty, dest)
            }
            "math.f64.atan2_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_atan2_v1", args, dest_ty, dest)
            }
            "math.f64.min_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_min_v1", args, dest_ty, dest)
            }
            "math.f64.max_v1" => {
                self.emit_math_f64_binop_to(head, "ev_math_f64_max_v1", args, dest_ty, dest)
            }

            "math.f64.sqrt_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_sqrt_v1", args, dest_ty, dest)
            }
            "math.f64.neg_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_neg_v1", args, dest_ty, dest)
            }
            "math.f64.abs_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_abs_v1", args, dest_ty, dest)
            }
            "math.f64.sin_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_sin_v1", args, dest_ty, dest)
            }
            "math.f64.cos_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_cos_v1", args, dest_ty, dest)
            }
            "math.f64.tan_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_tan_v1", args, dest_ty, dest)
            }
            "math.f64.exp_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_exp_v1", args, dest_ty, dest)
            }
            "math.f64.log_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_ln_v1", args, dest_ty, dest)
            }
            "math.f64.floor_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_floor_v1", args, dest_ty, dest)
            }
            "math.f64.ceil_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_ceil_v1", args, dest_ty, dest)
            }
            "math.f64.fmt_shortest_v1" => {
                self.emit_math_f64_unop_to(head, "ev_math_f64_fmt_shortest_v1", args, dest_ty, dest)
            }
            "math.f64.parse_v1" => self.emit_math_f64_parse_to(args, dest_ty, dest),
            "math.f64.from_i32_v1" => self.emit_math_f64_from_i32_to(args, dest_ty, dest),
            "math.f64.to_i32_trunc_v1" => self.emit_math_f64_to_i32_trunc_to(args, dest_ty, dest),
            "math.f64.to_bits_u64le_v1" => self.emit_math_f64_unop_to(
                head,
                "ev_math_f64_to_bits_u64le_v1",
                args,
                dest_ty,
                dest,
            ),

            "regex.compile_opts_v1" => self.emit_regex_compile_opts_v1_to(args, dest_ty, dest),
            "regex.exec_from_v1" => self.emit_regex_exec_from_v1_to(args, dest_ty, dest),
            "regex.exec_caps_from_v1" => self.emit_regex_exec_caps_from_v1_to(args, dest_ty, dest),
            "regex.find_all_x7sl_v1" => self.emit_regex_find_all_x7sl_v1_to(args, dest_ty, dest),
            "regex.split_v1" => self.emit_regex_split_v1_to(args, dest_ty, dest),
            "regex.replace_all_v1" => self.emit_regex_replace_all_v1_to(args, dest_ty, dest),

            "bytes.view" => self.emit_bytes_view_to(args, dest_ty, dest),
            "bytes.subview" => self.emit_bytes_subview_to(args, dest_ty, dest),
            "view.len" => self.emit_view_len_to(args, dest_ty, dest),
            "view.get_u8" => self.emit_view_get_u8_to(args, dest_ty, dest),
            "view.slice" => self.emit_view_slice_to(args, dest_ty, dest),
            "view.to_bytes" => self.emit_view_to_bytes_to(args, dest_ty, dest),
            "view.as_ptr" => self.emit_view_as_ptr_to(args, dest_ty, dest),
            "view.eq" => self.emit_view_eq_to(args, dest_ty, dest),
            "view.cmp_range" => self.emit_view_cmp_range_to(args, dest_ty, dest),

            "vec_u8.as_ptr" => self.emit_vec_u8_as_ptr_to(args, dest_ty, dest),
            "vec_u8.as_mut_ptr" => self.emit_vec_u8_as_mut_ptr_to(args, dest_ty, dest),

            "ptr.null" => self.emit_ptr_null_to(args, dest_ty, dest),
            "ptr.as_const" => self.emit_ptr_as_const_to(args, dest_ty, dest),
            "ptr.cast" => self.emit_ptr_cast_to(args, dest_ty, dest),

            "addr_of" => self.emit_addr_of_to(args, dest_ty, dest),
            "addr_of_mut" => self.emit_addr_of_mut_to(args, dest_ty, dest),

            "ptr.add" => self.emit_ptr_add_to(args, dest_ty, dest),
            "ptr.sub" => self.emit_ptr_sub_to(args, dest_ty, dest),
            "ptr.offset" => self.emit_ptr_offset_to(args, dest_ty, dest),
            "ptr.read_u8" => self.emit_ptr_read_u8_to(args, dest_ty, dest),
            "ptr.write_u8" => self.emit_ptr_write_u8_to(args, dest_ty, dest),
            "ptr.read_i32" => self.emit_ptr_read_i32_to(args, dest_ty, dest),
            "ptr.write_i32" => self.emit_ptr_write_i32_to(args, dest_ty, dest),

            "memcpy" => self.emit_memcpy_to(args, dest_ty, dest),
            "memmove" => self.emit_memmove_to(args, dest_ty, dest),
            "memset" => self.emit_memset_to(args, dest_ty, dest),

            "await" => self.emit_task_await_to(args, dest_ty, dest),
            "task.spawn" => self.emit_task_spawn_to(args, dest_ty, dest),
            "task.is_finished" => self.emit_task_is_finished_to(args, dest_ty, dest),
            "task.try_join.bytes" => self.emit_task_try_join_bytes_to(args, dest_ty, dest),
            "task.join.bytes" => self.emit_task_join_bytes_to(args, dest_ty, dest),
            "task.yield" => self.emit_task_yield_to(args, dest_ty, dest),
            "task.sleep" => self.emit_task_sleep_to(args, dest_ty, dest),
            "task.cancel" => self.emit_task_cancel_to(args, dest_ty, dest),

            "chan.bytes.new" => self.emit_chan_bytes_new_to(args, dest_ty, dest),
            "chan.bytes.try_send" => self.emit_chan_bytes_try_send_to(args, dest_ty, dest),
            "chan.bytes.send" => self.emit_chan_bytes_send_to(args, dest_ty, dest),
            "chan.bytes.try_recv" => self.emit_chan_bytes_try_recv_to(args, dest_ty, dest),
            "chan.bytes.recv" => self.emit_chan_bytes_recv_to(args, dest_ty, dest),
            "chan.bytes.close" => self.emit_chan_bytes_close_to(args, dest_ty, dest),

            "fs.read" => self.emit_fs_read_to(args, dest_ty, dest),
            "fs.read_async" => self.emit_fs_read_async_to(args, dest_ty, dest),
            "fs.open_read" => self.emit_fs_open_read_to(args, dest_ty, dest),
            "fs.list_dir" => self.emit_fs_list_dir_to(args, dest_ty, dest),

            "os.fs.read_file" => self.emit_os_fs_read_file_to(args, dest_ty, dest),
            "os.fs.write_file" => self.emit_os_fs_write_file_to(args, dest_ty, dest),
            "os.fs.read_all_v1" => self.emit_os_fs_read_all_v1_to(args, dest_ty, dest),
            "os.fs.write_all_v1" => self.emit_os_fs_write_all_v1_to(args, dest_ty, dest),
            "os.fs.mkdirs_v1" => self.emit_os_fs_mkdirs_v1_to(args, dest_ty, dest),
            "os.fs.remove_file_v1" => self.emit_os_fs_remove_file_v1_to(args, dest_ty, dest),
            "os.fs.remove_dir_all_v1" => self.emit_os_fs_remove_dir_all_v1_to(args, dest_ty, dest),
            "os.fs.rename_v1" => self.emit_os_fs_rename_v1_to(args, dest_ty, dest),
            "os.fs.list_dir_sorted_text_v1" => {
                self.emit_os_fs_list_dir_sorted_text_v1_to(args, dest_ty, dest)
            }
            "os.fs.walk_glob_sorted_text_v1" => {
                self.emit_os_fs_walk_glob_sorted_text_v1_to(args, dest_ty, dest)
            }
            "os.fs.stat_v1" => self.emit_os_fs_stat_v1_to(args, dest_ty, dest),
            "os.db.sqlite.open_v1" => self.emit_os_db_sqlite_open_v1_to(args, dest_ty, dest),
            "os.db.sqlite.query_v1" => self.emit_os_db_sqlite_query_v1_to(args, dest_ty, dest),
            "os.db.sqlite.exec_v1" => self.emit_os_db_sqlite_exec_v1_to(args, dest_ty, dest),
            "os.db.sqlite.close_v1" => self.emit_os_db_sqlite_close_v1_to(args, dest_ty, dest),
            "os.db.pg.open_v1" => self.emit_os_db_pg_open_v1_to(args, dest_ty, dest),
            "os.db.pg.query_v1" => self.emit_os_db_pg_query_v1_to(args, dest_ty, dest),
            "os.db.pg.exec_v1" => self.emit_os_db_pg_exec_v1_to(args, dest_ty, dest),
            "os.db.pg.close_v1" => self.emit_os_db_pg_close_v1_to(args, dest_ty, dest),
            "os.db.mysql.open_v1" => self.emit_os_db_mysql_open_v1_to(args, dest_ty, dest),
            "os.db.mysql.query_v1" => self.emit_os_db_mysql_query_v1_to(args, dest_ty, dest),
            "os.db.mysql.exec_v1" => self.emit_os_db_mysql_exec_v1_to(args, dest_ty, dest),
            "os.db.mysql.close_v1" => self.emit_os_db_mysql_close_v1_to(args, dest_ty, dest),
            "os.db.redis.open_v1" => self.emit_os_db_redis_open_v1_to(args, dest_ty, dest),
            "os.db.redis.cmd_v1" => self.emit_os_db_redis_cmd_v1_to(args, dest_ty, dest),
            "os.db.redis.close_v1" => self.emit_os_db_redis_close_v1_to(args, dest_ty, dest),
            "os.env.get" => self.emit_os_env_get_to(args, dest_ty, dest),
            "os.time.now_unix_ms" => self.emit_os_time_now_unix_ms_to(args, dest_ty, dest),
            "os.time.now_instant_v1" => self.emit_os_time_now_instant_v1_to(args, dest_ty, dest),
            "os.time.sleep_ms_v1" => self.emit_os_time_sleep_ms_v1_to(args, dest_ty, dest),
            "os.time.local_tzid_v1" => self.emit_os_time_local_tzid_v1_to(args, dest_ty, dest),
            "os.time.tzdb_is_valid_tzid_v1" => {
                self.emit_os_time_tzdb_is_valid_tzid_v1_to(args, dest_ty, dest)
            }
            "os.time.tzdb_offset_duration_v1" => {
                self.emit_os_time_tzdb_offset_duration_v1_to(args, dest_ty, dest)
            }
            "os.time.tzdb_snapshot_id_v1" => {
                self.emit_os_time_tzdb_snapshot_id_v1_to(args, dest_ty, dest)
            }
            "os.process.exit" => self.emit_os_process_exit_to(args, dest_ty, dest),
            "os.process.spawn_capture_v1" => {
                self.emit_os_process_spawn_capture_v1_to(args, dest_ty, dest)
            }
            "os.process.spawn_piped_v1" => {
                self.emit_os_process_spawn_piped_v1_to(args, dest_ty, dest)
            }
            "os.process.try_join_capture_v1" => {
                self.emit_os_process_try_join_capture_v1_to(args, dest_ty, dest)
            }
            "os.process.join_capture_v1" | "std.os.process.join_capture_v1" => {
                self.emit_os_process_join_capture_v1_to(args, dest_ty, dest)
            }
            "os.process.stdout_read_v1" => {
                self.emit_os_process_stdout_read_v1_to(args, dest_ty, dest)
            }
            "os.process.stderr_read_v1" => {
                self.emit_os_process_stderr_read_v1_to(args, dest_ty, dest)
            }
            "os.process.stdin_write_v1" => {
                self.emit_os_process_stdin_write_v1_to(args, dest_ty, dest)
            }
            "os.process.stdin_close_v1" => {
                self.emit_os_process_stdin_close_v1_to(args, dest_ty, dest)
            }
            "os.process.try_wait_v1" => self.emit_os_process_try_wait_v1_to(args, dest_ty, dest),
            "os.process.join_exit_v1" | "std.os.process.join_exit_v1" => {
                self.emit_os_process_join_exit_v1_to(args, dest_ty, dest)
            }
            "os.process.take_exit_v1" => self.emit_os_process_take_exit_v1_to(args, dest_ty, dest),
            "os.process.kill_v1" => self.emit_os_process_kill_v1_to(args, dest_ty, dest),
            "os.process.drop_v1" => self.emit_os_process_drop_v1_to(args, dest_ty, dest),
            "os.process.run_capture_v1" => {
                self.emit_os_process_run_capture_v1_to(args, dest_ty, dest)
            }
            "os.net.http_request" => self.emit_os_net_http_request_to(args, dest_ty, dest),

            "rr.send_request" => self.emit_rr_send_request_to(args, dest_ty, dest),
            "rr.fetch" => self.emit_rr_fetch_to(args, dest_ty, dest),
            "rr.send" => self.emit_rr_send_to(args, dest_ty, dest),
            "kv.get" => self.emit_kv_get_to(args, dest_ty, dest),
            "kv.get_async" => self.emit_kv_get_async_to(args, dest_ty, dest),
            "kv.get_stream" => self.emit_kv_get_stream_to(args, dest_ty, dest),
            "kv.set" => self.emit_kv_set_to(args, dest_ty, dest),

            "io.open_read_bytes" => self.emit_io_open_read_bytes_to(args, dest_ty, dest),
            "io.read" => self.emit_io_read_to(args, dest_ty, dest),
            "iface.make_v1" => self.emit_iface_make_v1_to(args, dest_ty, dest),
            "bufread.new" => self.emit_bufread_new_to(args, dest_ty, dest),
            "bufread.fill" => self.emit_bufread_fill_to(args, dest_ty, dest),
            "bufread.consume" => self.emit_bufread_consume_to(args, dest_ty, dest),

            "codec.read_u32_le" => self.emit_codec_read_u32_le_to(args, dest_ty, dest),
            "codec.write_u32_le" => self.emit_codec_write_u32_le_to(args, dest_ty, dest),
            "fmt.u32_to_dec" => self.emit_fmt_u32_to_dec_to(args, dest_ty, dest),
            "fmt.s32_to_dec" => self.emit_fmt_s32_to_dec_to(args, dest_ty, dest),
            "parse.u32_dec" => self.emit_parse_u32_dec_to(args, dest_ty, dest),
            "parse.u32_dec_at" => self.emit_parse_u32_dec_at_to(args, dest_ty, dest),
            "prng.lcg_next_u32" => self.emit_prng_lcg_next_u32_to(args, dest_ty, dest),

            "vec_u8.with_capacity" => self.emit_vec_u8_new_to(args, dest_ty, dest),
            "vec_u8.len" => self.emit_vec_u8_len_to(args, dest_ty, dest),
            "vec_u8.get" => self.emit_vec_u8_get_to(args, dest_ty, dest),
            "vec_u8.set" => self.emit_vec_u8_set_to(args, dest_ty, dest),
            "vec_u8.push" => self.emit_vec_u8_push_to(args, dest_ty, dest),
            "vec_u8.reserve_exact" => self.emit_vec_u8_reserve_exact_to(args, dest_ty, dest),
            "vec_u8.extend_zeroes" => self.emit_vec_u8_extend_zeroes_to(args, dest_ty, dest),
            "vec_u8.extend_bytes" => self.emit_vec_u8_extend_bytes_to(args, dest_ty, dest),
            "vec_u8.extend_bytes_range" => {
                self.emit_vec_u8_extend_bytes_range_to(args, dest_ty, dest)
            }
            "vec_u8.into_bytes" => self.emit_vec_u8_into_bytes_to(args, dest_ty, dest),
            "vec_u8.as_view" => self.emit_vec_u8_as_view_to(args, dest_ty, dest),

            "option_i32.none" => self.emit_option_i32_none_to(args, dest_ty, dest),
            "option_i32.some" => self.emit_option_i32_some_to(args, dest_ty, dest),
            "option_i32.is_some" => self.emit_option_i32_is_some_to(args, dest_ty, dest),
            "option_i32.unwrap_or" => self.emit_option_i32_unwrap_or_to(args, dest_ty, dest),

            "option_bytes.none" => self.emit_option_bytes_none_to(args, dest_ty, dest),
            "option_bytes.some" => self.emit_option_bytes_some_to(args, dest_ty, dest),
            "option_bytes.is_some" => self.emit_option_bytes_is_some_to(args, dest_ty, dest),
            "option_bytes.unwrap_or" => self.emit_option_bytes_unwrap_or_to(args, dest_ty, dest),

            "result_i32.ok" => self.emit_result_i32_ok_to(args, dest_ty, dest),
            "result_i32.err" => self.emit_result_i32_err_to(args, dest_ty, dest),
            "result_i32.is_ok" => self.emit_result_i32_is_ok_to(args, dest_ty, dest),
            "result_i32.err_code" => self.emit_result_i32_err_code_to(args, dest_ty, dest),
            "result_i32.unwrap_or" => self.emit_result_i32_unwrap_or_to(args, dest_ty, dest),

            "result_bytes.ok" => self.emit_result_bytes_ok_to(args, dest_ty, dest),
            "result_bytes.err" => self.emit_result_bytes_err_to(args, dest_ty, dest),
            "result_bytes.is_ok" => self.emit_result_bytes_is_ok_to(args, dest_ty, dest),
            "result_bytes.err_code" => self.emit_result_bytes_err_code_to(args, dest_ty, dest),
            "result_bytes.unwrap_or" => self.emit_result_bytes_unwrap_or_to(args, dest_ty, dest),

            "try" => self.emit_try_to(args, dest_ty, dest),

            "map_u32.new" | "set_u32.new" => self.emit_map_u32_new_to(args, dest_ty, dest),
            "map_u32.len" => self.emit_map_u32_len_to(args, dest_ty, dest),
            "map_u32.get" => self.emit_map_u32_get_to(args, dest_ty, dest),
            "map_u32.set" => self.emit_map_u32_set_to(args, dest_ty, dest),
            "map_u32.contains" | "set_u32.contains" => {
                self.emit_map_u32_contains_to(args, dest_ty, dest)
            }
            "map_u32.remove" | "set_u32.remove" => self.emit_map_u32_remove_to(args, dest_ty, dest),
            "set_u32.add" => self.emit_set_u32_add_to(args, dest_ty, dest),
            "set_u32.dump_u32le" => self.emit_set_u32_dump_u32le_to(args, dest_ty, dest),
            "map_u32.dump_kv_u32le_u32le" => {
                self.emit_map_u32_dump_kv_u32le_u32le_to(args, dest_ty, dest)
            }

            _ => {
                if self.fn_c_names.contains_key(head) {
                    self.emit_user_call_to(head, args, dest_ty, dest)
                } else if self.async_fn_new_names.contains_key(head) {
                    self.emit_async_call_to(head, args, dest_ty, dest)
                } else if self.extern_functions.contains_key(head) {
                    self.emit_extern_call_to(head, args, dest_ty, dest)
                } else {
                    Err(CompilerError::new(
                        CompileErrorKind::Unsupported,
                        format!("unsupported head: {head:?}"),
                    ))
                }
            }
        }
    }

    fn emit_begin_to(
        &mut self,
        exprs: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if exprs.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "(begin ...) requires at least 1 expression".to_string(),
            ));
        }

        self.push_scope();
        self.open_block();
        for e in &exprs[..exprs.len() - 1] {
            self.emit_stmt(e)?;
        }
        self.emit_expr_to(&exprs[exprs.len() - 1], dest_ty, dest)?;
        self.pop_scope()?;
        self.close_block();
        Ok(())
    }

    fn emit_unsafe_to(
        &mut self,
        exprs: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.allow_unsafe() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "unsafe is not allowed in this world".to_string(),
            ));
        }
        let prev = self.unsafe_depth;
        self.unsafe_depth = self.unsafe_depth.saturating_add(1);
        let res = self.emit_begin_to(exprs, dest_ty, dest);
        self.unsafe_depth = prev;
        res
    }

    fn emit_let_to(&mut self, args: &[Expr], dest_ty: Ty, dest: &str) -> Result<(), CompilerError> {
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

        if is_owned_ty(dest_ty) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "let expression cannot produce an owned value; use (begin (let ...) <var>)"
                    .to_string(),
            ));
        }

        if self.scopes.last().and_then(|s| s.get(name)).is_some() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("duplicate let binding in same scope: {name:?}"),
            ));
        }

        let expr_ty = self.infer_expr_in_new_scope(&args[1])?;
        if expr_ty != dest_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("let expression must match context type {dest_ty:?}"),
            ));
        }

        let c_name = self.alloc_local("v_")?;
        self.decl_local(expr_ty, &c_name);
        self.emit_expr_to(&args[1], expr_ty, &c_name)?;
        let mut var = self.make_var_ref(expr_ty, c_name.clone(), false);
        if expr_ty == Ty::BytesView {
            let borrow_of = self.borrow_of_view_expr(&args[1])?;
            if let Some(owner) = &borrow_of {
                self.inc_borrow_count(owner)?;
            }
            var.borrow_of = borrow_of;
        }
        self.bind(name.to_string(), var);
        self.line(&format!("{dest} = {c_name};"));
        Ok(())
    }

    fn emit_set_to(&mut self, args: &[Expr], dest_ty: Ty, dest: &str) -> Result<(), CompilerError> {
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
        let Some(var) = self.lookup(name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("set of unknown variable: {name:?}"),
            ));
        };
        if var.ty != dest_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("set expression must match context type {dest_ty:?}"),
            ));
        }

        if is_owned_ty(dest_ty) {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "set expression cannot produce an owned value; use (begin (set ...) <var>)"
                    .to_string(),
            ));
        }

        self.emit_set_stmt(args)?;
        self.line(&format!("{dest} = {};", var.c_name));
        Ok(())
    }

    fn emit_if_to(&mut self, args: &[Expr], dest_ty: Ty, dest: &str) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
            ));
        }

        let cond = self.emit_expr(&args[0])?;
        if cond.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "if condition must be i32".to_string(),
            ));
        }

        let then_ty = self.infer_expr_in_new_scope(&args[1])?;
        let else_ty = self.infer_expr_in_new_scope(&args[2])?;
        let ok = match (then_ty, else_ty) {
            (Ty::Never, Ty::Never) => true,
            (Ty::Never, t) => t == dest_ty,
            (t, Ty::Never) => t == dest_ty,
            (t, e) => t == dest_ty && e == dest_ty,
        };
        if !ok {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("if branches must match context type {dest_ty:?}"),
            ));
        }

        let scopes_before = self.scopes.clone();

        self.line(&format!("if ({} != UINT32_C(0)) {{", cond.c_name));
        self.indent += 1;
        self.push_scope();
        self.emit_expr_to(&args[1], dest_ty, dest)?;
        self.pop_scope()?;
        let scopes_then = self.scopes.clone();
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.scopes = scopes_before.clone();
        self.push_scope();
        self.emit_expr_to(&args[2], dest_ty, dest)?;
        self.pop_scope()?;
        let scopes_else = self.scopes.clone();
        self.indent -= 1;
        self.line("}");

        if then_ty == Ty::Never && else_ty == Ty::Never {
            self.scopes = scopes_before;
        } else if then_ty == Ty::Never {
            self.scopes = scopes_else;
        } else if else_ty == Ty::Never {
            self.scopes = scopes_then;
        } else {
            self.scopes = self.merge_if_states(&scopes_before, &scopes_then, &scopes_else)?;
            self.recompute_borrow_counts()?;
        }
        Ok(())
    }

    fn emit_for_to(&mut self, args: &[Expr], dest_ty: Ty, dest: &str) -> Result<(), CompilerError> {
        if args.len() != 4 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "for form: (for <i> <start:i32> <end:i32> <body:any>)".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "for expression returns i32".to_string(),
            ));
        }

        let var_name = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "for variable must be an identifier".to_string(),
            )
        })?;

        let (var_c_name, var_ty) = match self.lookup(var_name) {
            Some(v) => (v.c_name.clone(), v.ty),
            None => {
                let c_name = self.alloc_local("v_")?;
                self.decl_local(Ty::I32, &c_name);
                self.bind(
                    var_name.to_string(),
                    self.make_var_ref(Ty::I32, c_name.clone(), false),
                );
                (c_name, Ty::I32)
            }
        };
        if var_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("for variable must be i32: {var_name:?}"),
            ));
        }

        // start (assigned to var)
        self.emit_expr_to(&args[1], Ty::I32, &var_c_name)?;

        // end (evaluated after start)
        let end_local = self.alloc_local("t_i32_")?;
        self.decl_local(Ty::I32, &end_local);
        self.emit_expr_to(&args[2], Ty::I32, &end_local)?;

        self.line("for (;;) {");
        self.indent += 1;
        self.line(&format!("if ({var_c_name} >= {end_local}) break;"));

        self.push_scope();
        self.open_block();
        self.emit_stmt(&args[3])?;
        self.pop_scope()?;
        self.close_block();

        self.line(&format!("{var_c_name} = {var_c_name} + UINT32_C(1);"));
        self.indent -= 1;
        self.line("}");

        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    fn emit_return(&mut self, args: &[Expr]) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "return form: (return <expr>)".to_string(),
            ));
        }
        let scopes_snapshot = self.scopes.clone();
        let v = self.emit_expr(&args[0])?;
        if v.ty != self.fn_ret_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("return expression must evaluate to {:?}", self.fn_ret_ty),
            ));
        }

        for (ty, c_name) in self.live_owned_drop_list(Some(&v.c_name)) {
            self.emit_drop_var(ty, &c_name);
        }
        self.line(&format!("return {};", v.c_name));
        // `return` terminates control flow. Moves/sets performed while evaluating the return
        // expression must not affect the remaining compilation state.
        self.scopes = scopes_snapshot;
        Ok(())
    }

    fn emit_binop_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns i32"),
            ));
        }
        if (head == "<u" || head == ">=u") && matches!(args.get(1), Some(Expr::Int(0))) {
            let msg = match head {
                "<u" => {
                    "semantic error: `(<u x 0)` is always false (unsigned comparison). \
Use a signed comparison like `(< x 0)` when checking for negatives, or guard before decrementing."
                }
                ">=u" => {
                    "semantic error: `(>=u x 0)` is always true (unsigned comparison). \
Use a signed comparison like `(>= x 0)` when checking for negatives, or remove the check."
                }
                _ => unreachable!(),
            };
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                msg.to_string(),
            ));
        }
        let a = self.emit_expr(&args[0])?;
        let b = self.emit_expr(&args[1])?;
        if a.ty != Ty::I32 || b.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects i32 args"),
            ));
        }
        match head {
            "+" => self.line(&format!("{dest} = {} + {};", a.c_name, b.c_name)),
            "-" => self.line(&format!("{dest} = {} - {};", a.c_name, b.c_name)),
            "*" => self.line(&format!("{dest} = {} * {};", a.c_name, b.c_name)),
            "/" => self.line(&format!(
                "{dest} = ({} == UINT32_C(0)) ? UINT32_C(0) : ({} / {});",
                b.c_name, a.c_name, b.c_name
            )),
            "%" => self.line(&format!(
                "{dest} = ({} == UINT32_C(0)) ? {} : ({} % {});",
                b.c_name, a.c_name, a.c_name, b.c_name
            )),
            "&" => self.line(&format!("{dest} = {} & {};", a.c_name, b.c_name)),
            "|" => self.line(&format!("{dest} = {} | {};", a.c_name, b.c_name)),
            "^" => self.line(&format!("{dest} = {} ^ {};", a.c_name, b.c_name)),
            "<<u" => self.line(&format!(
                "{dest} = {} << ({} & UINT32_C(31));",
                a.c_name, b.c_name
            )),
            ">>u" => self.line(&format!(
                "{dest} = {} >> ({} & UINT32_C(31));",
                a.c_name, b.c_name
            )),
            "=" => self.line(&format!("{dest} = ({} == {});", a.c_name, b.c_name)),
            "!=" => self.line(&format!("{dest} = ({} != {});", a.c_name, b.c_name)),
            "<" => self.line(&format!(
                "{dest} = (({} ^ UINT32_C(0x80000000)) < ({} ^ UINT32_C(0x80000000)));",
                a.c_name, b.c_name
            )),
            "<=" => self.line(&format!(
                "{dest} = (({} ^ UINT32_C(0x80000000)) >= ({} ^ UINT32_C(0x80000000)));",
                b.c_name, a.c_name
            )),
            ">" => self.line(&format!(
                "{dest} = (({} ^ UINT32_C(0x80000000)) < ({} ^ UINT32_C(0x80000000)));",
                b.c_name, a.c_name
            )),
            ">=" => self.line(&format!(
                "{dest} = (({} ^ UINT32_C(0x80000000)) >= ({} ^ UINT32_C(0x80000000)));",
                a.c_name, b.c_name
            )),
            "<u" => self.line(&format!("{dest} = ({} < {});", a.c_name, b.c_name)),
            ">=u" => self.line(&format!("{dest} = ({} >= {});", a.c_name, b.c_name)),
            ">u" => self.line(&format!("{dest} = ({} < {});", b.c_name, a.c_name)),
            "<=u" => self.line(&format!("{dest} = ({} >= {});", b.c_name, a.c_name)),
            _ => unreachable!(),
        }
        Ok(())
    }

    fn emit_user_call_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let f = self
            .program
            .functions
            .iter()
            .find(|f| f.name == head)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    format!("internal error: missing function def for {head:?}"),
                )
            })?;

        if args.len() != f.params.len() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("call {:?} expects {} args", head, f.params.len()),
            ));
        }
        if dest_ty != f.ret_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("call {:?} returns {:?}", head, f.ret_ty),
            ));
        }

        let mut rendered_args = Vec::with_capacity(args.len());
        let mut arg_vals = Vec::with_capacity(args.len());
        for (i, (arg_expr, param)) in args.iter().zip(f.params.iter()).enumerate() {
            let v = match param.ty {
                Ty::BytesView => self.emit_expr_as_bytes_view(arg_expr)?,
                _ => self.emit_expr(arg_expr)?,
            };
            if v.ty != param.ty {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("call {:?} arg {} expects {:?}", head, i, param.ty),
                ));
            }
            rendered_args.push(v.c_name.clone());
            arg_vals.push(v);
        }
        let c_args = if rendered_args.is_empty() {
            String::new()
        } else {
            format!(", {}", rendered_args.join(", "))
        };
        self.line(&format!(
            "{dest} = {}(ctx, input{c_args});",
            self.fn_c_name(head)
        ));
        for v in arg_vals {
            if is_owned_ty(v.ty) {
                self.line(&format!("{} = {};", v.c_name, c_empty(v.ty)));
            }
            self.release_temp_view_borrow(&v)?;
        }
        Ok(())
    }

    fn emit_extern_call_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let f = self.extern_functions.get(head).cloned().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Internal,
                format!("internal error: missing extern decl for {head:?}"),
            )
        })?;

        if args.len() != f.params.len() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("call {:?} expects {} args", head, f.params.len()),
            ));
        }
        if dest_ty != f.ret_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("call {:?} returns {:?}", head, f.ret_ty),
            ));
        }

        let mut rendered_args = Vec::with_capacity(args.len());
        for (i, (arg_expr, param)) in args.iter().zip(f.params.iter()).enumerate() {
            let v = self.emit_expr(arg_expr)?;
            let want = param.ty;
            let ok = v.ty == want
                || matches!(
                    (v.ty, want),
                    (Ty::PtrMutU8, Ty::PtrConstU8)
                        | (Ty::PtrMutVoid, Ty::PtrConstVoid)
                        | (Ty::PtrMutI32, Ty::PtrConstI32)
                );
            if !ok {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("call {:?} arg {} expects {:?}", head, i, param.ty),
                ));
            }
            rendered_args.push(v.c_name);
        }
        let c_args = rendered_args.join(", ");

        if f.ret_is_void {
            self.line(&format!("{}({c_args});", f.link_name));
            self.line(&format!("{dest} = UINT32_C(0);"));
        } else {
            self.line(&format!("{dest} = {}({c_args});", f.link_name));
        }
        Ok(())
    }

    fn emit_async_call_to(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        let f = self
            .program
            .async_functions
            .iter()
            .find(|f| f.name == head)
            .ok_or_else(|| {
                CompilerError::new(
                    CompileErrorKind::Internal,
                    format!("internal error: missing async function def for {head:?}"),
                )
            })?;

        if args.len() != f.params.len() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("call {:?} expects {} args", head, f.params.len()),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("async call {:?} returns i32 task handle", head),
            ));
        }

        let mut rendered_args = Vec::with_capacity(args.len());
        let mut arg_vals = Vec::with_capacity(args.len());
        for (i, (arg_expr, param)) in args.iter().zip(f.params.iter()).enumerate() {
            let v = self.emit_expr(arg_expr)?;
            if v.ty != param.ty {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("call {:?} arg {} expects {:?}", head, i, param.ty),
                ));
            }
            rendered_args.push(v.c_name.clone());
            arg_vals.push(v);
        }
        let c_args = if rendered_args.is_empty() {
            String::new()
        } else {
            format!(", {}", rendered_args.join(", "))
        };
        self.line(&format!(
            "{dest} = {}(ctx, input{c_args});",
            self.async_fn_new_c_name(head)
        ));
        for v in arg_vals {
            if is_owned_ty(v.ty) {
                self.line(&format!("{} = {};", v.c_name, c_empty(v.ty)));
            }
        }
        Ok(())
    }

    fn fn_c_name(&self, name: &str) -> &str {
        self.fn_c_names
            .get(name)
            .map(|s| s.as_str())
            .unwrap_or("__x07_missing_fn")
    }

    fn async_fn_new_c_name(&self, name: &str) -> &str {
        self.async_fn_new_names
            .get(name)
            .map(|s| s.as_str())
            .unwrap_or("__x07_missing_async_fn")
    }

    fn emit_bytes_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.len returns i32".to_string(),
            ));
        }
        let v = self.emit_expr_as_bytes_view(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.len expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.len;", v.c_name));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    fn emit_bytes_get_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.get_u8 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.get_u8 returns i32".to_string(),
            ));
        }
        let v = self.emit_expr_as_bytes_view(&args[0])?;
        let i = self.emit_expr(&args[1])?;
        if v.ty != Ty::BytesView || i.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.get_u8 expects (bytes_view, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_get_u8(ctx, {}, {});",
            v.c_name, i.c_name
        ));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    fn emit_bytes_set_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.set_u8 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.set_u8 returns bytes".to_string(),
            ));
        }
        if let Expr::Ident(name) = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("bytes.set_u8 while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::Bytes {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
                ));
            }
            let i = self.emit_expr(&args[1])?;
            let v = self.emit_expr(&args[2])?;
            if i.ty != Ty::I32 || v.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_bytes_set_u8(ctx, {}, {}, {});",
                var_c_name, var_c_name, i.c_name, v.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::Bytes)));
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                }
            }
            return Ok(());
        }

        let b = self.emit_expr(&args[0])?;
        let i = self.emit_expr(&args[1])?;
        let v = self.emit_expr(&args[2])?;
        if b.ty != Ty::Bytes || i.ty != Ty::I32 || v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bytes_set_u8(ctx, {}, {}, {});",
            b.c_name, i.c_name, v.c_name
        ));
        if dest != b.c_name.as_str() {
            self.line(&format!("{} = {};", b.c_name, c_empty(Ty::Bytes)));
        }
        Ok(())
    }

    fn emit_bytes_alloc_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.alloc expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.alloc returns bytes".to_string(),
            ));
        }
        let len = self.emit_expr(&args[0])?;
        if len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.alloc length must be i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_bytes_alloc(ctx, {});", len.c_name));
        Ok(())
    }

    fn emit_bytes_empty_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.empty expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.empty returns bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_bytes_empty(ctx);"));
        Ok(())
    }

    fn emit_bytes1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes1 returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes1 expects i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_bytes_alloc(ctx, UINT32_C(1));"));
        self.line(&format!(
            "{dest} = rt_bytes_set_u8(ctx, {dest}, UINT32_C(0), {});",
            x.c_name
        ));
        Ok(())
    }

    fn emit_bytes_lit_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.lit expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.lit returns bytes".to_string(),
            ));
        }
        let ident = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.lit expects an identifier".to_string(),
            )
        })?;
        let lit_bytes = ident.as_bytes();

        self.tmp_counter += 1;
        let lit_name = format!("lit_{}", self.tmp_counter);
        let escaped = c_escape_string(lit_bytes);
        self.line(&format!("static const char {lit_name}[] = \"{escaped}\";"));
        self.line(&format!(
            "{dest} = rt_bytes_from_literal(ctx, (const uint8_t*){lit_name}, UINT32_C({}));",
            lit_bytes.len()
        ));
        Ok(())
    }

    fn emit_bytes_slice_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.slice expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.slice returns bytes".to_string(),
            ));
        }
        let v = self.emit_expr_as_bytes_view(&args[0])?;
        let start = self.emit_expr(&args[1])?;
        let len = self.emit_expr(&args[2])?;
        if v.ty != Ty::BytesView || start.ty != Ty::I32 || len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.slice expects (bytes_view, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_to_bytes(ctx, rt_view_slice(ctx, {}, {}, {}));",
            v.c_name, start.c_name, len.c_name
        ));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    fn emit_bytes_copy_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.copy expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.copy returns bytes".to_string(),
            ));
        }
        let src = self.emit_expr(&args[0])?;
        let dstb = self.emit_expr(&args[1])?;
        if src.ty != Ty::Bytes || dstb.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.copy expects (bytes, bytes)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bytes_copy(ctx, {}, {});",
            src.c_name, dstb.c_name
        ));
        if dest != dstb.c_name.as_str() {
            self.line(&format!("{} = rt_bytes_empty(ctx);", dstb.c_name));
        }
        Ok(())
    }

    fn emit_bytes_concat_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.concat expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.concat returns bytes".to_string(),
            ));
        }
        let a = self.emit_expr(&args[0])?;
        let b = self.emit_expr(&args[1])?;
        if a.ty != Ty::Bytes || b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.concat expects (bytes, bytes)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bytes_concat(ctx, {}, {});",
            a.c_name, b.c_name
        ));
        Ok(())
    }

    fn emit_bytes_eq_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.eq expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.eq returns i32".to_string(),
            ));
        }
        let a = self.emit_expr_as_bytes_view(&args[0])?;
        let b = self.emit_expr_as_bytes_view(&args[1])?;
        if a.ty != Ty::BytesView || b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.eq expects (bytes_view, bytes_view)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_eq(ctx, {}, {});",
            a.c_name, b.c_name
        ));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    fn emit_bytes_cmp_range_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 6 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.cmp_range expects 6 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.cmp_range returns i32".to_string(),
            ));
        }
        let a = self.emit_expr_as_bytes_view(&args[0])?;
        let a_off = self.emit_expr(&args[1])?;
        let a_len = self.emit_expr(&args[2])?;
        let b = self.emit_expr_as_bytes_view(&args[3])?;
        let b_off = self.emit_expr(&args[4])?;
        let b_len = self.emit_expr(&args[5])?;
        if a.ty != Ty::BytesView
            || a_off.ty != Ty::I32
            || a_len.ty != Ty::I32
            || b.ty != Ty::BytesView
            || b_off.ty != Ty::I32
            || b_len.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.cmp_range expects (bytes_view, i32, i32, bytes_view, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_cmp_range(ctx, {}, {}, {}, {}, {}, {});",
            a.c_name, a_off.c_name, a_len.c_name, b.c_name, b_off.c_name, b_len.c_name
        ));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    fn emit_bytes_as_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.as_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrConstU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.as_ptr returns ptr_const_u8".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.as_ptr expects bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.ptr;", b.c_name));
        Ok(())
    }

    fn emit_bytes_as_mut_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.as_mut_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrMutU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.as_mut_ptr returns ptr_mut_u8".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.as_mut_ptr expects bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.ptr;", b.c_name));
        Ok(())
    }

    fn emit_math_f64_binop_to(
        &mut self,
        head: &str,
        c_fn: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(native::BACKEND_ID_MATH, native::ABI_MAJOR_V1, head)?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns bytes"),
            ));
        }
        let a = self.emit_expr_as_bytes_view(&args[0])?;
        let b = self.emit_expr_as_bytes_view(&args[1])?;
        if a.ty != Ty::BytesView || b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects (bytes_view, bytes_view)"),
            ));
        }
        let a_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            a.c_name, a.c_name
        );
        let b_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            b.c_name, b.c_name
        );
        self.line(&format!("{dest} = {c_fn}({a_expr}, {b_expr});"));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    fn emit_math_f64_unop_to(
        &mut self,
        head: &str,
        c_fn: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(native::BACKEND_ID_MATH, native::ABI_MAJOR_V1, head)?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 1 arg"),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} returns bytes"),
            ));
        }
        let x = self.emit_expr_as_bytes_view(&args[0])?;
        if x.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects bytes_view"),
            ));
        }
        let x_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            x.c_name, x.c_name
        );
        self.line(&format!("{dest} = {c_fn}({x_expr});"));
        self.release_temp_view_borrow(&x)?;
        Ok(())
    }

    fn emit_math_f64_parse_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_MATH,
            native::ABI_MAJOR_V1,
            "math.f64.parse_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "math.f64.parse_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.parse_v1 returns result_bytes".to_string(),
            ));
        }
        let s = self.emit_expr_as_bytes_view(&args[0])?;
        if s.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.parse_v1 expects bytes_view".to_string(),
            ));
        }
        let s_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            s.c_name, s.c_name
        );
        self.line(&format!("{dest} = ev_math_f64_parse_v1({s_expr});"));
        self.release_temp_view_borrow(&s)?;
        Ok(())
    }

    fn emit_math_f64_from_i32_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_MATH,
            native::ABI_MAJOR_V1,
            "math.f64.from_i32_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "math.f64.from_i32_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.from_i32_v1 returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.from_i32_v1 expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ev_math_f64_from_i32_v1((int32_t){});",
            x.c_name
        ));
        Ok(())
    }

    fn emit_math_f64_to_i32_trunc_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_MATH,
            native::ABI_MAJOR_V1,
            "math.f64.to_i32_trunc_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "math.f64.to_i32_trunc_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.to_i32_trunc_v1 returns result_i32".to_string(),
            ));
        }
        let x = self.emit_expr_as_bytes_view(&args[0])?;
        if x.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "math.f64.to_i32_trunc_v1 expects bytes_view".to_string(),
            ));
        }
        let x_expr = format!(
            "(bytes_t){{ .ptr = {}.ptr, .len = {}.len }}",
            x.c_name, x.c_name
        );
        self.line(&format!("{dest} = ev_math_f64_to_i32_trunc_v1({x_expr});"));
        self.release_temp_view_borrow(&x)?;
        Ok(())
    }

    fn emit_bytes_view_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.view expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.view returns bytes_view".to_string(),
            ));
        }
        let Some(b_name) = args[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.view requires an identifier owner (bind the bytes to a variable first)"
                    .to_string(),
            ));
        };
        let Some(b) = self.lookup(b_name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown identifier: {b_name:?}"),
            ));
        };
        if b.moved {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("use after move: {b_name:?}"),
            ));
        }
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.view expects bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_bytes_view(ctx, {});", b.c_name));
        Ok(())
    }

    fn emit_bytes_subview_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bytes.subview expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.subview returns bytes_view".to_string(),
            ));
        }
        let Some(b_name) = args[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.subview requires an identifier owner (bind the bytes to a variable first)"
                    .to_string(),
            ));
        };
        let Some(b) = self.lookup(b_name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown identifier: {b_name:?}"),
            ));
        };
        if b.moved {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("use after move: {b_name:?}"),
            ));
        }
        let start = self.emit_expr(&args[1])?;
        let len = self.emit_expr(&args[2])?;
        if b.ty != Ty::Bytes || start.ty != Ty::I32 || len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bytes.subview expects (bytes, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bytes_subview(ctx, {}, {}, {});",
            b.c_name, start.c_name, len.c_name
        ));
        Ok(())
    }

    fn emit_view_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.len returns i32".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.len expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.len;", v.c_name));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    fn emit_view_get_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.get_u8 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.get_u8 returns i32".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        let i = self.emit_expr(&args[1])?;
        if v.ty != Ty::BytesView || i.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.get_u8 expects (bytes_view, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_get_u8(ctx, {}, {});",
            v.c_name, i.c_name
        ));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    fn emit_view_slice_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.slice expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.slice returns bytes_view".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        let start = self.emit_expr(&args[1])?;
        let len = self.emit_expr(&args[2])?;
        if v.ty != Ty::BytesView || start.ty != Ty::I32 || len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.slice expects (bytes_view, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_slice(ctx, {}, {}, {});",
            v.c_name, start.c_name, len.c_name
        ));
        Ok(())
    }

    fn emit_view_to_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.to_bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.to_bytes returns bytes".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.to_bytes expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_view_to_bytes(ctx, {});", v.c_name));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    fn emit_view_as_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.as_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrConstU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.as_ptr returns ptr_const_u8".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.as_ptr expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = {}.ptr;", v.c_name));
        self.release_temp_view_borrow(&v)?;
        Ok(())
    }

    fn emit_view_eq_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.eq expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.eq returns i32".to_string(),
            ));
        }
        let a = self.emit_expr(&args[0])?;
        let b = self.emit_expr(&args[1])?;
        if a.ty != Ty::BytesView || b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.eq expects (bytes_view, bytes_view)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_eq(ctx, {}, {});",
            a.c_name, b.c_name
        ));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    fn emit_view_cmp_range_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 6 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "view.cmp_range expects 6 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.cmp_range returns i32".to_string(),
            ));
        }
        let a = self.emit_expr(&args[0])?;
        let a_off = self.emit_expr(&args[1])?;
        let a_len = self.emit_expr(&args[2])?;
        let b = self.emit_expr(&args[3])?;
        let b_off = self.emit_expr(&args[4])?;
        let b_len = self.emit_expr(&args[5])?;
        if a.ty != Ty::BytesView
            || a_off.ty != Ty::I32
            || a_len.ty != Ty::I32
            || b.ty != Ty::BytesView
            || b_off.ty != Ty::I32
            || b_len.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "view.cmp_range expects (bytes_view, i32, i32, bytes_view, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_view_cmp_range(ctx, {}, {}, {}, {}, {}, {});",
            a.c_name, a_off.c_name, a_len.c_name, b.c_name, b_off.c_name, b_len.c_name
        ));
        self.release_temp_view_borrow(&a)?;
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    fn emit_task_await_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "await expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "await returns bytes".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "await expects i32 task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_join_bytes_block(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    fn emit_task_spawn_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.spawn expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.spawn returns i32".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.spawn expects i32 task handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_task_spawn(ctx, {});", tid.c_name));
        Ok(())
    }

    fn emit_task_is_finished_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.is_finished expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.is_finished returns i32".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.is_finished expects i32 task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_is_finished(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    fn emit_task_try_join_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.try_join.bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.try_join.bytes returns result_bytes".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.try_join.bytes expects i32 task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_try_join_bytes(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    fn emit_task_join_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.join.bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.join.bytes returns bytes".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.join.bytes expects i32 task handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_join_bytes_block(ctx, {});",
            tid.c_name
        ));
        Ok(())
    }

    fn emit_task_yield_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.yield expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.yield returns i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_task_yield_block(ctx);"));
        Ok(())
    }

    fn emit_task_sleep_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.sleep expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.sleep returns i32".to_string(),
            ));
        }
        let ticks = self.emit_expr(&args[0])?;
        if ticks.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.sleep expects i32 ticks".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_task_sleep_block(ctx, {});",
            ticks.c_name
        ));
        Ok(())
    }

    fn emit_task_cancel_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "task.cancel expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.cancel returns i32".to_string(),
            ));
        }
        let tid = self.emit_expr(&args[0])?;
        if tid.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "task.cancel expects i32 task handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_task_cancel(ctx, {});", tid.c_name));
        Ok(())
    }

    fn emit_chan_bytes_new_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.new expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.new returns i32".to_string(),
            ));
        }
        let cap = self.emit_expr(&args[0])?;
        if cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.new expects i32 cap".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_chan_bytes_new(ctx, {});", cap.c_name));
        Ok(())
    }

    fn emit_chan_bytes_try_send_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.try_send expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.try_send returns i32".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        let msg = self.emit_expr_as_bytes_view(&args[1])?;
        if chan.ty != Ty::I32 || msg.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.try_send expects (i32, bytes_view)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_try_send_view(ctx, {}, {});",
            chan.c_name, msg.c_name
        ));
        self.release_temp_view_borrow(&msg)?;
        Ok(())
    }

    fn emit_chan_bytes_send_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.send expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.send returns i32".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        let msg = self.emit_expr(&args[1])?;
        if chan.ty != Ty::I32 || msg.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.send expects (i32, bytes)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_send_block(ctx, {}, {});",
            chan.c_name, msg.c_name
        ));
        self.line(&format!("{} = {};", msg.c_name, c_empty(Ty::Bytes)));
        Ok(())
    }

    fn emit_chan_bytes_try_recv_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.try_recv expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.try_recv returns result_bytes".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        if chan.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.try_recv expects i32 chan handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_try_recv(ctx, {});",
            chan.c_name
        ));
        Ok(())
    }

    fn emit_chan_bytes_recv_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.recv expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.recv returns bytes".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        if chan.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.recv expects i32 chan handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_recv_block(ctx, {});",
            chan.c_name
        ));
        Ok(())
    }

    fn emit_chan_bytes_close_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "chan.bytes.close expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.close returns i32".to_string(),
            ));
        }
        let chan = self.emit_expr(&args[0])?;
        if chan.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "chan.bytes.close expects i32 chan handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_chan_bytes_close(ctx, {});",
            chan.c_name
        ));
        Ok(())
    }

    fn emit_fs_read_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_fs {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "fs.read is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "fs.read expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.read returns bytes".to_string(),
            ));
        }
        let path = self.emit_expr_as_bytes_view(&args[0])?;
        if path.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.read expects bytes_view path".to_string(),
            ));
        }
        // Phase G2: if a fixture latency index is present, enforce it even for fs.read so
        // benchmark suites can require meaningful concurrency.
        self.line(&format!(
            "{dest} = rt_fs_read_async_block(ctx, {});",
            path.c_name
        ));
        self.release_temp_view_borrow(&path)?;
        Ok(())
    }

    fn emit_fs_read_async_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_fs {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "fs.read_async is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "fs.read_async expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.read_async returns bytes".to_string(),
            ));
        }
        let path = self.emit_expr_as_bytes_view(&args[0])?;
        if path.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.read_async expects bytes_view path".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_fs_read_async_block(ctx, {});",
            path.c_name
        ));
        self.release_temp_view_borrow(&path)?;
        Ok(())
    }

    fn emit_fs_open_read_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_fs {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "fs.open_read is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "fs.open_read expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Iface {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.open_read returns iface".to_string(),
            ));
        }
        let path = self.emit_expr_as_bytes_view(&args[0])?;
        if path.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.open_read expects bytes_view path".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (iface_t){{ .data = rt_fs_open_read(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
            path.c_name
        ));
        self.release_temp_view_borrow(&path)?;
        Ok(())
    }

    fn emit_fs_list_dir_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_fs {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "fs.list_dir is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "fs.list_dir expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.list_dir returns bytes".to_string(),
            ));
        }
        let path = self.emit_expr_as_bytes_view(&args[0])?;
        if path.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fs.list_dir expects bytes_view path".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_fs_list_dir(ctx, {});", path.c_name));
        self.release_temp_view_borrow(&path)?;
        Ok(())
    }

    fn emit_os_fs_read_file_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.read_file")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.read_file expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.read_file returns bytes".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        if path.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.read_file expects bytes path".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_fs_read_file(ctx, {});",
            path.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_write_file_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.write_file")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.write_file expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.write_file returns i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let data = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || data.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.write_file expects (bytes path, bytes data)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_fs_write_file(ctx, {}, {});",
            path.c_name, data.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_read_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.read_all_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.read_all_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.read_all_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.read_all_v1 returns result_bytes".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.read_all_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_read_all_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_write_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.write_all_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.write_all_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.write_all_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.write_all_v1 returns result_i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let data = self.emit_expr(&args[1])?;
        let caps = self.emit_expr(&args[2])?;
        if path.ty != Ty::Bytes || data.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.write_all_v1 expects (bytes path, bytes data, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_write_all_v1({}, {}, {});",
            path.c_name, data.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_mkdirs_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.mkdirs_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.mkdirs_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.mkdirs_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.mkdirs_v1 returns result_i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.mkdirs_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_mkdirs_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_remove_file_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.remove_file_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.remove_file_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.remove_file_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.remove_file_v1 returns result_i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.remove_file_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_remove_file_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_remove_dir_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.remove_dir_all_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.remove_dir_all_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.remove_dir_all_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.remove_dir_all_v1 returns result_i32".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.remove_dir_all_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_remove_dir_all_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_rename_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.rename_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.rename_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.rename_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.rename_v1 returns result_i32".to_string(),
            ));
        }
        let src = self.emit_expr(&args[0])?;
        let dst = self.emit_expr(&args[1])?;
        let caps = self.emit_expr(&args[2])?;
        if src.ty != Ty::Bytes || dst.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.rename_v1 expects (bytes src, bytes dst, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_rename_v1({}, {}, {});",
            src.c_name, dst.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_list_dir_sorted_text_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.list_dir_sorted_text_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.list_dir_sorted_text_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.list_dir_sorted_text_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.list_dir_sorted_text_v1 returns result_bytes".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.list_dir_sorted_text_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_list_dir_sorted_text_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_walk_glob_sorted_text_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.walk_glob_sorted_text_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.walk_glob_sorted_text_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.walk_glob_sorted_text_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.walk_glob_sorted_text_v1 returns result_bytes".to_string(),
            ));
        }
        let root = self.emit_expr(&args[0])?;
        let glob = self.emit_expr(&args[1])?;
        let caps = self.emit_expr(&args[2])?;
        if root.ty != Ty::Bytes || glob.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.walk_glob_sorted_text_v1 expects (bytes root, bytes glob, bytes caps)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_walk_glob_sorted_text_v1({}, {}, {});",
            root.c_name, glob.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_fs_stat_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.fs.stat_v1")?;
        self.require_native_backend(
            native::BACKEND_ID_EXT_FS,
            native::ABI_MAJOR_V1,
            "os.fs.stat_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.fs.stat_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stat_v1 returns result_bytes".to_string(),
            ));
        }
        let path = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if path.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.fs.stat_v1 expects (bytes path, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_fs_stat_v1({}, {});",
            path.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_db_call_bytes_v1_to(
        &mut self,
        builtin: &str,
        c_fn: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only(builtin)?;
        let backend_id = if builtin.starts_with("os.db.sqlite.") {
            Some(native::BACKEND_ID_EXT_DB_SQLITE)
        } else if builtin.starts_with("os.db.pg.") {
            Some(native::BACKEND_ID_EXT_DB_PG)
        } else if builtin.starts_with("os.db.mysql.") {
            Some(native::BACKEND_ID_EXT_DB_MYSQL)
        } else if builtin.starts_with("os.db.redis.") {
            Some(native::BACKEND_ID_EXT_DB_REDIS)
        } else {
            None
        };
        if let Some(backend_id) = backend_id {
            self.require_native_backend(backend_id, native::ABI_MAJOR_V1, builtin)?;
        }
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{builtin} expects 2 args"),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{builtin} returns bytes"),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if req.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{builtin} expects (bytes req, bytes caps)"),
            ));
        }
        self.line(&format!(
            "{dest} = {c_fn}({}, {});",
            req.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_db_sqlite_open_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.sqlite.open_v1",
            "x07_ext_db_sqlite_open_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_sqlite_query_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.sqlite.query_v1",
            "x07_ext_db_sqlite_query_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_sqlite_exec_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.sqlite.exec_v1",
            "x07_ext_db_sqlite_exec_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_sqlite_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.sqlite.close_v1",
            "x07_ext_db_sqlite_close_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_pg_open_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.pg.open_v1",
            "x07_ext_db_pg_open_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_pg_query_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.pg.query_v1",
            "x07_ext_db_pg_query_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_pg_exec_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.pg.exec_v1",
            "x07_ext_db_pg_exec_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_pg_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.pg.close_v1",
            "x07_ext_db_pg_close_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_mysql_open_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.mysql.open_v1",
            "x07_ext_db_mysql_open_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_mysql_query_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.mysql.query_v1",
            "x07_ext_db_mysql_query_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_mysql_exec_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.mysql.exec_v1",
            "x07_ext_db_mysql_exec_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_mysql_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.mysql.close_v1",
            "x07_ext_db_mysql_close_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_redis_open_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.redis.open_v1",
            "x07_ext_db_redis_open_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_redis_cmd_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.redis.cmd_v1",
            "x07_ext_db_redis_cmd_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_db_redis_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_os_db_call_bytes_v1_to(
            "os.db.redis.close_v1",
            "x07_ext_db_redis_close_v1",
            args,
            dest_ty,
            dest,
        )
    }

    fn emit_os_env_get_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.env.get")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.env.get expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.env.get returns bytes".to_string(),
            ));
        }
        let key = self.emit_expr(&args[0])?;
        if key.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.env.get expects bytes key".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_os_env_get(ctx, {});", key.c_name));
        Ok(())
    }

    fn emit_os_time_now_unix_ms_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.time.now_unix_ms")?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.now_unix_ms expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.now_unix_ms returns i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_os_time_now_unix_ms(ctx);"));
        Ok(())
    }

    fn emit_os_time_now_instant_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.time.now_instant_v1")?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.now_instant_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.now_instant_v1 returns bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_os_time_now_instant_v1(ctx);"));
        Ok(())
    }

    fn emit_os_time_sleep_ms_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.time.sleep_ms_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.sleep_ms_v1 expects 1 arg".to_string(),
            ));
        }
        let ms = self.emit_expr(&args[0])?;
        if ms.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.sleep_ms_v1 expects i32 ms".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.sleep_ms_v1 returns i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (int32_t)rt_os_time_sleep_ms_v1(ctx, (int32_t){});",
            ms.c_name
        ));
        Ok(())
    }

    fn emit_os_time_local_tzid_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.time.local_tzid_v1")?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.local_tzid_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.local_tzid_v1 returns bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_os_time_local_tzid_v1(ctx);"));
        Ok(())
    }

    fn emit_os_time_tzdb_is_valid_tzid_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_TIME,
            native::ABI_MAJOR_V1,
            "os.time.tzdb_is_valid_tzid_v1",
        )?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.tzdb_is_valid_tzid_v1 expects 1 arg".to_string(),
            ));
        }
        let tzid = self.emit_expr(&args[0])?;
        if tzid.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_is_valid_tzid_v1 expects bytes_view tzid".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_is_valid_tzid_v1 returns i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ev_time_tzdb_is_valid_tzid_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }});",
            tzid.c_name, tzid.c_name
        ));
        Ok(())
    }

    fn emit_regex_compile_opts_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.compile_opts_v1",
        )?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.compile_opts_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.compile_opts_v1 returns bytes".to_string(),
            ));
        }
        let pat = self.emit_expr(&args[0])?;
        let opts = self.emit_expr(&args[1])?;
        if pat.ty != Ty::BytesView || opts.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.compile_opts_v1 expects (bytes_view pat, i32 opts)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_compile_opts_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            pat.c_name, pat.c_name, opts.c_name
        ));
        Ok(())
    }

    fn emit_regex_exec_from_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.exec_from_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.exec_from_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.exec_from_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let start = self.emit_expr(&args[2])?;
        if compiled.ty != Ty::BytesView || text.ty != Ty::BytesView || start.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.exec_from_v1 expects (bytes_view compiled, bytes_view text, i32 start)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_exec_from_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, start.c_name
        ));
        Ok(())
    }

    fn emit_regex_exec_caps_from_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.exec_caps_from_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.exec_caps_from_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.exec_caps_from_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let start = self.emit_expr(&args[2])?;
        if compiled.ty != Ty::BytesView || text.ty != Ty::BytesView || start.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.exec_caps_from_v1 expects (bytes_view compiled, bytes_view text, i32 start)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_exec_caps_from_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, start.c_name
        ));
        Ok(())
    }

    fn emit_regex_find_all_x7sl_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.find_all_x7sl_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.find_all_x7sl_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.find_all_x7sl_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let max_matches = self.emit_expr(&args[2])?;
        if compiled.ty != Ty::BytesView || text.ty != Ty::BytesView || max_matches.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.find_all_x7sl_v1 expects (bytes_view compiled, bytes_view text, i32 max_matches)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_find_all_x7sl_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, max_matches.c_name
        ));
        Ok(())
    }

    fn emit_regex_split_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.split_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.split_v1 expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.split_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let max_parts = self.emit_expr(&args[2])?;
        if compiled.ty != Ty::BytesView || text.ty != Ty::BytesView || max_parts.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.split_v1 expects (bytes_view compiled, bytes_view text, i32 max_parts)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_split_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, max_parts.c_name
        ));
        Ok(())
    }

    fn emit_regex_replace_all_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_EXT_REGEX,
            native::ABI_MAJOR_V1,
            "regex.replace_all_v1",
        )?;
        if args.len() != 4 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "regex.replace_all_v1 expects 4 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.replace_all_v1 returns bytes".to_string(),
            ));
        }
        let compiled = self.emit_expr(&args[0])?;
        let text = self.emit_expr(&args[1])?;
        let repl = self.emit_expr(&args[2])?;
        let cap_limit = self.emit_expr(&args[3])?;
        if compiled.ty != Ty::BytesView
            || text.ty != Ty::BytesView
            || repl.ty != Ty::BytesView
            || cap_limit.ty != Ty::I32
        {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "regex.replace_all_v1 expects (bytes_view compiled, bytes_view text, bytes_view repl, i32 cap_limit)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = x07_ext_regex_replace_all_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){});",
            compiled.c_name, compiled.c_name, text.c_name, text.c_name, repl.c_name, repl.c_name, cap_limit.c_name
        ));
        Ok(())
    }

    fn emit_os_time_tzdb_offset_duration_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_TIME,
            native::ABI_MAJOR_V1,
            "os.time.tzdb_offset_duration_v1",
        )?;
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.tzdb_offset_duration_v1 expects 3 args".to_string(),
            ));
        }
        let tzid = self.emit_expr(&args[0])?;
        if tzid.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_offset_duration_v1 expects bytes_view tzid".to_string(),
            ));
        }
        let unix_s_lo = self.emit_expr(&args[1])?;
        let unix_s_hi = self.emit_expr(&args[2])?;
        if unix_s_lo.ty != Ty::I32 || unix_s_hi.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_offset_duration_v1 expects i32 unix_s_lo/unix_s_hi".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_offset_duration_v1 returns bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ev_time_tzdb_offset_duration_v1((bytes_t){{ .ptr = {}.ptr, .len = {}.len }}, (int32_t){}, (int32_t){});",
            tzid.c_name, tzid.c_name, unix_s_lo.c_name, unix_s_hi.c_name
        ));
        Ok(())
    }

    fn emit_os_time_tzdb_snapshot_id_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_native_backend(
            native::BACKEND_ID_TIME,
            native::ABI_MAJOR_V1,
            "os.time.tzdb_snapshot_id_v1",
        )?;
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.time.tzdb_snapshot_id_v1 expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.time.tzdb_snapshot_id_v1 returns bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = ev_time_tzdb_snapshot_id_v1();"));
        Ok(())
    }

    fn emit_os_process_exit_to(
        &mut self,
        args: &[Expr],
        _dest_ty: Ty,
        _dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.exit")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.exit expects 1 arg".to_string(),
            ));
        }
        let code = self.emit_expr(&args[0])?;
        if code.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.exit expects i32 exit code".to_string(),
            ));
        }
        self.line(&format!(
            "rt_os_process_exit(ctx, (int32_t){});",
            code.c_name
        ));
        self.line("__builtin_unreachable();");
        Ok(())
    }

    fn emit_os_process_spawn_capture_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.spawn_capture_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.spawn_capture_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.spawn_capture_v1 returns i32".to_string(),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if req.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.spawn_capture_v1 expects (bytes req, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_spawn_capture_v1(ctx, {}, {});",
            req.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_process_spawn_piped_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.spawn_piped_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.spawn_piped_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.spawn_piped_v1 returns i32".to_string(),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if req.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.spawn_piped_v1 expects (bytes req, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_spawn_piped_v1(ctx, {}, {});",
            req.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_process_try_join_capture_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.try_join_capture_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.try_join_capture_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::OptionBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.try_join_capture_v1 returns option_bytes".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.try_join_capture_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_try_join_capture_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    fn emit_os_process_stdout_read_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.stdout_read_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.stdout_read_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdout_read_v1 returns bytes".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        let max = self.emit_expr(&args[1])?;
        if handle.ty != Ty::I32 || max.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdout_read_v1 expects (i32 handle, i32 max)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_stdout_read_v1(ctx, {}, (int32_t){});",
            handle.c_name, max.c_name
        ));
        Ok(())
    }

    fn emit_os_process_stderr_read_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.stderr_read_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.stderr_read_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stderr_read_v1 returns bytes".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        let max = self.emit_expr(&args[1])?;
        if handle.ty != Ty::I32 || max.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stderr_read_v1 expects (i32 handle, i32 max)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_stderr_read_v1(ctx, {}, (int32_t){});",
            handle.c_name, max.c_name
        ));
        Ok(())
    }

    fn emit_os_process_stdin_write_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.stdin_write_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.stdin_write_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdin_write_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        let chunk = self.emit_expr(&args[1])?;
        if handle.ty != Ty::I32 || chunk.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdin_write_v1 expects (i32 handle, bytes chunk)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_stdin_write_v1(ctx, {}, {});",
            handle.c_name, chunk.c_name
        ));
        Ok(())
    }

    fn emit_os_process_stdin_close_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.stdin_close_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.stdin_close_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdin_close_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.stdin_close_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_stdin_close_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    fn emit_os_process_try_wait_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.try_wait_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.try_wait_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.try_wait_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.try_wait_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_try_wait_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    fn emit_os_process_join_exit_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.join_exit_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.join_exit_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.join_exit_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.join_exit_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_join_exit_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    fn emit_os_process_take_exit_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.take_exit_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.take_exit_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.take_exit_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.take_exit_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_take_exit_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    fn emit_os_process_join_capture_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.join_capture_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.join_capture_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.join_capture_v1 returns bytes".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.join_capture_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_join_capture_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    fn emit_os_process_kill_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.kill_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.kill_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.kill_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        let sig = self.emit_expr(&args[1])?;
        if handle.ty != Ty::I32 || sig.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.kill_v1 expects (i32 proc_handle, i32 sig)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_kill_v1(ctx, {}, (int32_t){});",
            handle.c_name, sig.c_name
        ));
        Ok(())
    }

    fn emit_os_process_drop_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.drop_v1")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.drop_v1 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.drop_v1 returns i32".to_string(),
            ));
        }
        let handle = self.emit_expr(&args[0])?;
        if handle.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.drop_v1 expects i32 proc handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_drop_v1(ctx, {});",
            handle.c_name
        ));
        Ok(())
    }

    fn emit_os_process_run_capture_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.process.run_capture_v1")?;
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.process.run_capture_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.run_capture_v1 returns bytes".to_string(),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        let caps = self.emit_expr(&args[1])?;
        if req.ty != Ty::Bytes || caps.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.process.run_capture_v1 expects (bytes req, bytes caps)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_process_run_capture_v1(ctx, {}, {});",
            req.c_name, caps.c_name
        ));
        Ok(())
    }

    fn emit_os_net_http_request_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.require_standalone_only("os.net.http_request")?;
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "os.net.http_request expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.net.http_request returns bytes".to_string(),
            ));
        }
        let req = self.emit_expr(&args[0])?;
        if req.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "os.net.http_request expects bytes req".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_os_net_http_request(ctx, {});",
            req.c_name
        ));
        Ok(())
    }

    fn emit_rr_send_request_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_rr {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "rr.send_request is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "rr.send_request expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.send_request returns bytes".to_string(),
            ));
        }
        let req = self.emit_expr_as_bytes_view(&args[0])?;
        if req.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.send_request expects bytes_view".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_rr_send_request(ctx, {});",
            req.c_name
        ));
        self.release_temp_view_borrow(&req)?;
        Ok(())
    }

    fn emit_rr_fetch_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_rr {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "rr.fetch is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "rr.fetch expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.fetch returns bytes".to_string(),
            ));
        }
        let key = self.emit_expr_as_bytes_view(&args[0])?;
        if key.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.fetch expects bytes_view key".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_rr_fetch_block(ctx, {});", key.c_name));
        self.release_temp_view_borrow(&key)?;
        Ok(())
    }

    fn emit_rr_send_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_rr {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "rr.send is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "rr.send expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Iface {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.send returns iface".to_string(),
            ));
        }
        let req = self.emit_expr_as_bytes_view(&args[0])?;
        if req.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "rr.send expects bytes_view req".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (iface_t){{ .data = rt_rr_send(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
            req.c_name
        ));
        self.release_temp_view_borrow(&req)?;
        Ok(())
    }

    fn emit_kv_get_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_kv {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "kv.get is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "kv.get expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get returns bytes".to_string(),
            ));
        }
        let key = self.emit_expr_as_bytes_view(&args[0])?;
        if key.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get expects bytes_view key".to_string(),
            ));
        }
        // Phase G2: if a fixture latency index is present, enforce it even for kv.get so
        // benchmark suites can require meaningful concurrency.
        self.line(&format!(
            "{dest} = rt_kv_get_async_block(ctx, {});",
            key.c_name
        ));
        self.release_temp_view_borrow(&key)?;
        Ok(())
    }

    fn emit_kv_get_async_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_kv {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "kv.get_async is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "kv.get_async expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get_async returns bytes".to_string(),
            ));
        }
        let key = self.emit_expr_as_bytes_view(&args[0])?;
        if key.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get_async expects bytes_view key".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_kv_get_async_block(ctx, {});",
            key.c_name
        ));
        self.release_temp_view_borrow(&key)?;
        Ok(())
    }

    fn emit_kv_get_stream_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_kv {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "kv.get_stream is disabled in this world".to_string(),
            ));
        }
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "kv.get_stream expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Iface {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get_stream returns iface".to_string(),
            ));
        }
        let key = self.emit_expr_as_bytes_view(&args[0])?;
        if key.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.get_stream expects bytes_view key".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (iface_t){{ .data = rt_kv_get_stream(ctx, {}), .vtable = RT_IFACE_VTABLE_IO_READER }};",
            key.c_name
        ));
        self.release_temp_view_borrow(&key)?;
        Ok(())
    }

    fn emit_kv_set_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !self.options.enable_kv {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                "kv.set is disabled in this world".to_string(),
            ));
        }
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "kv.set expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.set returns i32".to_string(),
            ));
        }
        let key = self.emit_expr(&args[0])?;
        let val = self.emit_expr(&args[1])?;
        if key.ty != Ty::Bytes || val.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "kv.set expects (bytes, bytes)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_kv_set(ctx, {}, {});",
            key.c_name, val.c_name
        ));
        // kv.set takes ownership of key/val.
        self.line(&format!("{} = {};", key.c_name, c_empty(Ty::Bytes)));
        self.line(&format!("{} = {};", val.c_name, c_empty(Ty::Bytes)));
        Ok(())
    }

    fn emit_io_read_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "io.read expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "io.read returns bytes".to_string(),
            ));
        }
        let reader = self.emit_expr(&args[0])?;
        let max = self.emit_expr(&args[1])?;
        if reader.ty != Ty::Iface || max.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "io.read expects (iface, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_iface_io_read_block(ctx, {}, {});",
            reader.c_name, max.c_name
        ));
        Ok(())
    }

    fn emit_iface_make_v1_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "iface.make_v1 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Iface {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "iface.make_v1 returns iface".to_string(),
            ));
        }
        let data = self.emit_expr(&args[0])?;
        let vtable = self.emit_expr(&args[1])?;
        if data.ty != Ty::I32 || vtable.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "iface.make_v1 expects (i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (iface_t){{ .data = {}, .vtable = {} }};",
            data.c_name, vtable.c_name
        ));
        Ok(())
    }

    fn emit_io_open_read_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "io.open_read_bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Iface {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "io.open_read_bytes returns iface".to_string(),
            ));
        }

        let b = self.emit_expr(&args[0])?;
        let b_expr = match b.ty {
            Ty::Bytes => b.c_name.clone(),
            Ty::BytesView => format!("rt_view_to_bytes(ctx, {})", b.c_name),
            Ty::VecU8 => format!("rt_vec_u8_into_bytes(ctx, &{})", b.c_name),
            _ => {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "io.open_read_bytes expects bytes".to_string(),
                ))
            }
        };

        self.line(&format!(
            "{dest} = (iface_t){{ .data = rt_io_reader_new_bytes(ctx, {b_expr}, UINT32_C(0)), .vtable = RT_IFACE_VTABLE_IO_READER }};",
        ));

        if b.ty == Ty::Bytes {
            self.line(&format!("{} = {};", b.c_name, c_empty(Ty::Bytes)));
        }

        Ok(())
    }

    fn emit_bufread_new_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bufread.new expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.new returns i32".to_string(),
            ));
        }
        let reader = self.emit_expr(&args[0])?;
        let cap = self.emit_expr(&args[1])?;
        if reader.ty != Ty::Iface || cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.new expects (iface, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bufread_new(ctx, {}, {});",
            reader.c_name, cap.c_name
        ));
        Ok(())
    }

    fn emit_bufread_fill_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bufread.fill expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.fill returns bytes_view".to_string(),
            ));
        }
        let br = self.emit_expr(&args[0])?;
        if br.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.fill expects i32 bufread handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bufread_fill_block(ctx, {});",
            br.c_name
        ));
        Ok(())
    }

    fn emit_bufread_consume_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "bufread.consume expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.consume returns i32".to_string(),
            ));
        }
        let br = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if br.ty != Ty::I32 || n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "bufread.consume expects (i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_bufread_consume(ctx, {}, {});",
            br.c_name, n.c_name
        ));
        Ok(())
    }

    fn emit_codec_read_u32_le_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "codec.read_u32_le expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.read_u32_le returns i32".to_string(),
            ));
        }
        let b = self.emit_expr_as_bytes_view(&args[0])?;
        let off = self.emit_expr(&args[1])?;
        if b.ty != Ty::BytesView || off.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.read_u32_le expects (bytes_view, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_codec_read_u32_le(ctx, {}, {});",
            b.c_name, off.c_name
        ));
        self.release_temp_view_borrow(&b)?;
        Ok(())
    }

    fn emit_codec_write_u32_le_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "codec.write_u32_le expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.write_u32_le returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "codec.write_u32_le expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_codec_write_u32_le(ctx, {});",
            x.c_name
        ));
        Ok(())
    }

    fn emit_fmt_u32_to_dec_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "fmt.u32_to_dec expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fmt.u32_to_dec returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fmt.u32_to_dec expects i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_fmt_u32_to_dec(ctx, {});", x.c_name));
        Ok(())
    }

    fn emit_fmt_s32_to_dec_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "fmt.s32_to_dec expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fmt.s32_to_dec returns bytes".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "fmt.s32_to_dec expects i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_fmt_s32_to_dec(ctx, {});", x.c_name));
        Ok(())
    }

    fn emit_parse_u32_dec_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "parse.u32_dec expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "parse.u32_dec returns i32".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "parse.u32_dec expects bytes_view".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_parse_u32_dec(ctx, {});", b.c_name));
        Ok(())
    }

    fn emit_parse_u32_dec_at_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "parse.u32_dec_at expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "parse.u32_dec_at returns i32".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        let off = self.emit_expr(&args[1])?;
        if b.ty != Ty::BytesView || off.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "parse.u32_dec_at expects (bytes_view, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_parse_u32_dec_at(ctx, {}, {});",
            b.c_name, off.c_name
        ));
        Ok(())
    }

    fn emit_prng_lcg_next_u32_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "prng.lcg_next_u32 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "prng.lcg_next_u32 returns i32".to_string(),
            ));
        }
        let x = self.emit_expr(&args[0])?;
        if x.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "prng.lcg_next_u32 expects i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_prng_lcg_next_u32({});", x.c_name));
        Ok(())
    }

    fn emit_vec_u8_new_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.with_capacity expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.with_capacity returns vec_u8".to_string(),
            ));
        }
        let cap = self.emit_expr(&args[0])?;
        if cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.with_capacity cap must be i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_u8_new(ctx, {});", cap.c_name));
        Ok(())
    }

    fn emit_vec_u8_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.len returns i32".to_string(),
            ));
        }
        if let Some(name) = args[0].as_ident() {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.len expects vec_u8".to_string(),
                ));
            }
            self.line(&format!("{dest} = rt_vec_u8_len(ctx, {});", var.c_name));
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.len expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_u8_len(ctx, {});", h.c_name));
        Ok(())
    }

    fn emit_vec_u8_get_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.get expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.get returns i32".to_string(),
            ));
        }
        if let Some(name) = args[0].as_ident() {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            let idx = self.emit_expr(&args[1])?;
            if var.ty != Ty::VecU8 || idx.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.get expects (vec_u8, i32 index)".to_string(),
                ));
            }
            self.line(&format!(
                "{dest} = rt_vec_u8_get(ctx, {}, {});",
                var.c_name, idx.c_name
            ));
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let idx = self.emit_expr(&args[1])?;
        if h.ty != Ty::VecU8 || idx.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.get expects (vec_u8, i32 index)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_get(ctx, {}, {});",
            h.c_name, idx.c_name
        ));
        Ok(())
    }

    fn emit_vec_u8_set_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.set expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.set returns vec_u8".to_string(),
            ));
        }

        if let Expr::Ident(name) = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.set while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.set expects (vec_u8, i32 index, i32 value)".to_string(),
                ));
            }
            let idx = self.emit_expr(&args[1])?;
            let val = self.emit_expr(&args[2])?;
            if idx.ty != Ty::I32 || val.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.set expects (vec_u8, i32 index, i32 value)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_set(ctx, {}, {}, {});",
                var_c_name, var_c_name, idx.c_name, val.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                }
            }
            return Ok(());
        }

        let v = self.emit_expr(&args[0])?;
        let idx = self.emit_expr(&args[1])?;
        let val = self.emit_expr(&args[2])?;
        if v.ty != Ty::VecU8 || idx.ty != Ty::I32 || val.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.set expects (vec_u8, i32 index, i32 value)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_set(ctx, {}, {}, {});",
            v.c_name, idx.c_name, val.c_name
        ));
        if dest != v.c_name.as_str() {
            self.line(&format!("{} = {};", v.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    fn emit_vec_u8_push_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.push expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.push returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident(name) = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.push while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.push expects (vec_u8, i32 value)".to_string(),
                ));
            }
            let v = self.emit_expr(&args[1])?;
            if v.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.push expects (vec_u8, i32 value)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_push(ctx, {}, {});",
                var_c_name, var_c_name, v.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                }
            }
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let v = self.emit_expr(&args[1])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.push expects (vec_u8, i32 value)".to_string(),
            ));
        }
        if v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.push expects (vec_u8, i32 value)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_push(ctx, {}, {});",
            h.c_name, v.c_name
        ));
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    fn emit_vec_u8_reserve_exact_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.reserve_exact expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.reserve_exact returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident(name) = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.reserve_exact while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
                ));
            }
            let n = self.emit_expr(&args[1])?;
            if n.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_reserve_exact(ctx, {}, {});",
                var_c_name, var_c_name, n.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                }
            }
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
            ));
        }
        if n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_reserve_exact(ctx, {}, {});",
            h.c_name, n.c_name
        ));
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    fn emit_vec_u8_extend_zeroes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.extend_zeroes expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_zeroes returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident(name) = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.extend_zeroes while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
                ));
            }
            let n = self.emit_expr(&args[1])?;
            if n.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_extend_zeroes(ctx, {}, {});",
                var_c_name, var_c_name, n.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                }
            }
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if h.ty != Ty::VecU8 || n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_extend_zeroes(ctx, {}, {});",
            h.c_name, n.c_name
        ));
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    fn emit_vec_u8_extend_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.extend_bytes expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident(name) = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.extend_bytes while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
                ));
            }
            let b = self.emit_expr_as_bytes_view(&args[1])?;
            if b.ty != Ty::BytesView {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_extend_bytes(ctx, {}, {});",
                var_c_name, var_c_name, b.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                }
            }
            self.release_temp_view_borrow(&b)?;
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let b = self.emit_expr_as_bytes_view(&args[1])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
            ));
        }
        if b.ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_extend_bytes(ctx, {}, {});",
            h.c_name, b.c_name
        ));
        self.release_temp_view_borrow(&b)?;
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    fn emit_vec_u8_extend_bytes_range_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 4 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.extend_bytes_range expects 4 args".to_string(),
            ));
        }
        if dest_ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes_range returns vec_u8".to_string(),
            ));
        }
        if let Expr::Ident(name) = &args[0] {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            if var.borrow_count != 0 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("vec_u8.extend_bytes_range while borrowed: {name:?}"),
                ));
            }
            if var.ty != Ty::VecU8 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)"
                        .to_string(),
                ));
            }
            let b = self.emit_expr_as_bytes_view(&args[1])?;
            let start = self.emit_expr(&args[2])?;
            let len = self.emit_expr(&args[3])?;
            if b.ty != Ty::BytesView || start.ty != Ty::I32 || len.ty != Ty::I32 {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)"
                        .to_string(),
                ));
            }
            let var_c_name = var.c_name;
            self.line(&format!(
                "{} = rt_vec_u8_extend_bytes_range(ctx, {}, {}, {}, {});",
                var_c_name, var_c_name, b.c_name, start.c_name, len.c_name
            ));
            if dest != var_c_name.as_str() {
                self.line(&format!("{dest} = {var_c_name};"));
                self.line(&format!("{var_c_name} = {};", c_empty(Ty::VecU8)));
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                }
            }
            self.release_temp_view_borrow(&b)?;
            return Ok(());
        }

        let h = self.emit_expr(&args[0])?;
        let b = self.emit_expr_as_bytes_view(&args[1])?;
        let start = self.emit_expr(&args[2])?;
        let len = self.emit_expr(&args[3])?;
        if b.ty != Ty::BytesView || start.ty != Ty::I32 || len.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)"
                    .to_string(),
            ));
        }
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)"
                    .to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_vec_u8_extend_bytes_range(ctx, {}, {}, {}, {});",
            h.c_name, b.c_name, start.c_name, len.c_name
        ));
        self.release_temp_view_borrow(&b)?;
        if dest != h.c_name.as_str() {
            self.line(&format!("{} = {};", h.c_name, c_empty(Ty::VecU8)));
        }
        Ok(())
    }

    fn emit_vec_u8_into_bytes_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.into_bytes expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.into_bytes returns bytes".to_string(),
            ));
        }
        match &args[0] {
            Expr::Ident(name) => {
                let Some(var) = self.lookup(name).cloned() else {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    ));
                };
                if var.moved {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("use after move: {name:?}"),
                    ));
                }
                if var.borrow_count != 0 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("vec_u8.into_bytes while borrowed: {name:?}"),
                    ));
                }
                if var.ty != Ty::VecU8 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "vec_u8.into_bytes expects vec_u8".to_string(),
                    ));
                }
                self.line(&format!(
                    "{dest} = rt_vec_u8_into_bytes(ctx, &{});",
                    var.c_name
                ));
                if let Some(v) = self.lookup_mut(name) {
                    v.moved = true;
                }
                Ok(())
            }
            _ => {
                let h = self.emit_expr(&args[0])?;
                if h.ty != Ty::VecU8 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "vec_u8.into_bytes expects vec_u8".to_string(),
                    ));
                }
                self.line(&format!(
                    "{dest} = rt_vec_u8_into_bytes(ctx, &{});",
                    h.c_name
                ));
                Ok(())
            }
        }
    }

    fn emit_vec_u8_as_view_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.as_view expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::BytesView {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_view returns bytes_view".to_string(),
            ));
        }
        let Some(h_name) = args[0].as_ident() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_view requires an identifier owner (bind the vec_u8 to a variable first)"
                    .to_string(),
            ));
        };
        let Some(h) = self.lookup(h_name).cloned() else {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unknown identifier: {h_name:?}"),
            ));
        };
        if h.moved {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("use after move: {h_name:?}"),
            ));
        }
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_view expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_vec_u8_as_view(ctx, {});", h.c_name));
        Ok(())
    }

    fn emit_vec_u8_as_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.as_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrConstU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_ptr returns ptr_const_u8".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_ptr expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}).data;", h.c_name));
        Ok(())
    }

    fn emit_vec_u8_as_mut_ptr_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "vec_u8.as_mut_ptr expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::PtrMutU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_mut_ptr returns ptr_mut_u8".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::VecU8 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "vec_u8.as_mut_ptr expects vec_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}).data;", h.c_name));
        Ok(())
    }

    fn emit_ptr_null_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.null expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::PtrMutVoid {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.null returns ptr_mut_void".to_string(),
            ));
        }
        self.line(&format!("{dest} = NULL;"));
        Ok(())
    }

    fn emit_ptr_as_const_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.as_const expects 1 arg".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let ok = matches!(
            (p.ty, dest_ty),
            (Ty::PtrMutU8, Ty::PtrConstU8)
                | (Ty::PtrConstU8, Ty::PtrConstU8)
                | (Ty::PtrMutVoid, Ty::PtrConstVoid)
                | (Ty::PtrConstVoid, Ty::PtrConstVoid)
                | (Ty::PtrMutI32, Ty::PtrConstI32)
                | (Ty::PtrConstI32, Ty::PtrConstI32)
        );
        if !ok {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.as_const expects a raw pointer and returns a const raw pointer".to_string(),
            ));
        }
        self.line(&format!("{dest} = {};", p.c_name));
        Ok(())
    }

    fn emit_ptr_cast_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.cast form: (ptr.cast <ptr_ty> <ptr>)".to_string(),
            ));
        }
        let ty_name = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.cast target type must be an identifier".to_string(),
            )
        })?;
        let target = Ty::parse_named(ty_name).ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Typing,
                format!("ptr.cast unknown type: {ty_name:?}"),
            )
        })?;
        if target != dest_ty || !target.is_ptr_ty() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.cast target type must match the expression type".to_string(),
            ));
        }
        let p = self.emit_expr(&args[1])?;
        if !p.ty.is_ptr_ty() {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.cast expects a raw pointer".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}){};", c_ret_ty(target), p.c_name));
        Ok(())
    }

    fn emit_addr_of_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_addr_of_common(args, dest_ty, dest, false)
    }

    fn emit_addr_of_mut_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_addr_of_common(args, dest_ty, dest, true)
    }

    fn emit_addr_of_common(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
        is_mut: bool,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "addr_of expects 1 arg".to_string(),
            ));
        }
        let want = if is_mut {
            Ty::PtrMutVoid
        } else {
            Ty::PtrConstVoid
        };
        if dest_ty != want {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "addr_of returns a void raw pointer".to_string(),
            ));
        }
        let name = args[0].as_ident().ok_or_else(|| {
            CompilerError::new(
                CompileErrorKind::Parse,
                "addr_of expects an identifier".to_string(),
            )
        })?;
        let lvalue = if name == "input" {
            "input".to_string()
        } else {
            let Some(var) = self.lookup(name).cloned() else {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("unknown identifier: {name:?}"),
                ));
            };
            if var.moved {
                return Err(CompilerError::new(
                    CompileErrorKind::Typing,
                    format!("use after move: {name:?}"),
                ));
            }
            var.c_name
        };
        let cty = if is_mut { "void*" } else { "const void*" };
        self.line(&format!("{dest} = ({cty})&({lvalue});"));
        Ok(())
    }

    fn emit_ptr_add_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_ptr_addsub_common("ptr.add", args, dest_ty, dest, false)
    }

    fn emit_ptr_sub_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_ptr_addsub_common("ptr.sub", args, dest_ty, dest, true)
    }

    fn emit_ptr_addsub_common(
        &mut self,
        head: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
        is_sub: bool,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{head} expects 2 args"),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if p.ty != dest_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} pointer type mismatch"),
            ));
        }
        if n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{head} expects i32 offset"),
            ));
        }
        let op = if is_sub { "-" } else { "+" };
        self.line(&format!("{dest} = {} {op} (size_t){};", p.c_name, n.c_name));
        Ok(())
    }

    fn emit_ptr_offset_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.offset expects 2 args".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let n = self.emit_expr(&args[1])?;
        if p.ty != dest_ty {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.offset pointer type mismatch".to_string(),
            ));
        }
        if n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.offset expects i32 offset".to_string(),
            ));
        }
        self.line(&format!("{dest} = {} + (int32_t){};", p.c_name, n.c_name));
        Ok(())
    }

    fn emit_ptr_read_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.read_u8 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.read_u8 returns i32".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        if !matches!(p.ty, Ty::PtrConstU8 | Ty::PtrMutU8) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.read_u8 expects ptr_const_u8 or ptr_mut_u8".to_string(),
            ));
        }
        self.line(&format!("{dest} = (uint32_t)(*{});", p.c_name));
        Ok(())
    }

    fn emit_ptr_write_u8_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.write_u8 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.write_u8 returns i32".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let v = self.emit_expr(&args[1])?;
        if p.ty != Ty::PtrMutU8 || v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.write_u8 expects (ptr_mut_u8, i32)".to_string(),
            ));
        }
        self.line(&format!("*{} = (uint8_t){};", p.c_name, v.c_name));
        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    fn emit_ptr_read_i32_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.read_i32 expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.read_i32 returns i32".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        if !matches!(p.ty, Ty::PtrConstI32 | Ty::PtrMutI32) {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.read_i32 expects ptr_const_i32 or ptr_mut_i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = *{};", p.c_name));
        Ok(())
    }

    fn emit_ptr_write_i32_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "ptr.write_i32 expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.write_i32 returns i32".to_string(),
            ));
        }
        let p = self.emit_expr(&args[0])?;
        let v = self.emit_expr(&args[1])?;
        if p.ty != Ty::PtrMutI32 || v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "ptr.write_i32 expects (ptr_mut_i32, i32)".to_string(),
            ));
        }
        self.line(&format!("*{} = {};", p.c_name, v.c_name));
        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    fn emit_memcpy_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_memcpy_common("memcpy", args, dest_ty, dest)
    }

    fn emit_memmove_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        self.emit_memcpy_common("memmove", args, dest_ty, dest)
    }

    fn emit_memcpy_common(
        &mut self,
        name: &str,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("{name} expects 3 args"),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{name} returns i32"),
            ));
        }
        let dst = self.emit_expr(&args[0])?;
        let src = self.emit_expr(&args[1])?;
        let n = self.emit_expr(&args[2])?;
        if dst.ty != Ty::PtrMutVoid {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{name} expects ptr_mut_void dest"),
            ));
        }
        if !matches!(src.ty, Ty::PtrConstVoid | Ty::PtrMutVoid) || n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("{name} expects (ptr_const_void src, i32 len)"),
            ));
        }
        self.line(&format!("rt_mem_on_memcpy(ctx, {});", n.c_name));
        self.line(&format!(
            "(void){name}((void*){}, (const void*){}, (size_t){});",
            dst.c_name, src.c_name, n.c_name
        ));
        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    fn emit_memset_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "memset expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "memset returns i32".to_string(),
            ));
        }
        let dst = self.emit_expr(&args[0])?;
        let val = self.emit_expr(&args[1])?;
        let n = self.emit_expr(&args[2])?;
        if dst.ty != Ty::PtrMutVoid || val.ty != Ty::I32 || n.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "memset expects (ptr_mut_void, i32, i32)".to_string(),
            ));
        }
        self.line(&format!(
            "(void)memset((void*){}, (int){}, (size_t){});",
            dst.c_name, val.c_name, n.c_name
        ));
        self.line(&format!("{dest} = UINT32_C(0);"));
        Ok(())
    }

    fn emit_option_i32_none_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_i32.none expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::OptionI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.none returns option_i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_i32_t){{ .tag = UINT32_C(0), .payload = UINT32_C(0) }};"
        ));
        Ok(())
    }

    fn emit_option_i32_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_i32.some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::OptionI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.some returns option_i32".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.some expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_i32_t){{ .tag = UINT32_C(1), .payload = {} }};",
            v.c_name
        ));
        Ok(())
    }

    fn emit_option_i32_is_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_i32.is_some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.is_some returns i32".to_string(),
            ));
        }
        let opt = self.emit_expr(&args[0])?;
        if opt.ty != Ty::OptionI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.is_some expects option_i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", opt.c_name));
        Ok(())
    }

    fn emit_option_i32_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_i32.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.unwrap_or returns i32".to_string(),
            ));
        }
        let opt = self.emit_expr(&args[0])?;
        let default = self.emit_expr(&args[1])?;
        if opt.ty != Ty::OptionI32 || default.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_i32.unwrap_or expects (option_i32, i32 default)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(1)) ? {}.payload : {};",
            opt.c_name, opt.c_name, default.c_name
        ));
        Ok(())
    }

    fn emit_option_bytes_none_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if !args.is_empty() {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes.none expects 0 args".to_string(),
            ));
        }
        if dest_ty != Ty::OptionBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.none returns option_bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_bytes_t){{ .tag = UINT32_C(0), .payload = rt_bytes_empty(ctx) }};"
        ));
        Ok(())
    }

    fn emit_option_bytes_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes.some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::OptionBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.some returns option_bytes".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.some expects bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (option_bytes_t){{ .tag = UINT32_C(1), .payload = {} }};",
            b.c_name
        ));
        self.line(&format!("{} = rt_bytes_empty(ctx);", b.c_name));
        Ok(())
    }

    fn emit_option_bytes_is_some_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes.is_some expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.is_some returns i32".to_string(),
            ));
        }
        let opt = self.emit_expr(&args[0])?;
        if opt.ty != Ty::OptionBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.is_some expects option_bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", opt.c_name));
        Ok(())
    }

    fn emit_option_bytes_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "option_bytes.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.unwrap_or returns bytes".to_string(),
            ));
        }
        let opt = self.emit_expr(&args[0])?;
        let default = self.emit_expr(&args[1])?;
        if opt.ty != Ty::OptionBytes || default.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "option_bytes.unwrap_or expects (option_bytes, bytes default)".to_string(),
            ));
        }
        self.line(&format!("if ({}.tag == UINT32_C(1)) {{", opt.c_name));
        self.indent += 1;
        self.line(&format!("{dest} = {}.payload;", opt.c_name));
        self.line(&format!("{}.payload = rt_bytes_empty(ctx);", opt.c_name));
        self.line(&format!("{}.tag = UINT32_C(0);", opt.c_name));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!("{dest} = {};", default.c_name));
        self.line(&format!("{} = rt_bytes_empty(ctx);", default.c_name));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    fn emit_result_i32_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.ok returns result_i32".to_string(),
            ));
        }
        let v = self.emit_expr(&args[0])?;
        if v.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.ok expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_i32_t){{ .tag = UINT32_C(1), .payload.ok = {} }};",
            v.c_name
        ));
        Ok(())
    }

    fn emit_result_i32_err_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.err expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.err returns result_i32".to_string(),
            ));
        }
        let code = self.emit_expr(&args[0])?;
        if code.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.err expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_i32_t){{ .tag = UINT32_C(0), .payload.err = {} }};",
            code.c_name
        ));
        Ok(())
    }

    fn emit_result_i32_is_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.is_ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.is_ok returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.is_ok expects result_i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", res.c_name));
        Ok(())
    }

    fn emit_result_i32_err_code_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.err_code expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.err_code returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultI32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.err_code expects result_i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
            res.c_name, res.c_name
        ));
        Ok(())
    }

    fn emit_result_i32_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_i32.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.unwrap_or returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        let default = self.emit_expr(&args[1])?;
        if res.ty != Ty::ResultI32 || default.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_i32.unwrap_or expects (result_i32, i32 default)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(1)) ? {}.payload.ok : {};",
            res.c_name, res.c_name, default.c_name
        ));
        Ok(())
    }

    fn emit_result_bytes_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.ok returns result_bytes".to_string(),
            ));
        }
        let b = self.emit_expr(&args[0])?;
        if b.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.ok expects bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_bytes_t){{ .tag = UINT32_C(1), .payload.ok = {} }};",
            b.c_name
        ));
        self.line(&format!("{} = rt_bytes_empty(ctx);", b.c_name));
        Ok(())
    }

    fn emit_result_bytes_err_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.err expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.err returns result_bytes".to_string(),
            ));
        }
        let code = self.emit_expr(&args[0])?;
        if code.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.err expects i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = (result_bytes_t){{ .tag = UINT32_C(0), .payload.err = {} }};",
            code.c_name
        ));
        Ok(())
    }

    fn emit_result_bytes_is_ok_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.is_ok expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.is_ok returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.is_ok expects result_bytes".to_string(),
            ));
        }
        self.line(&format!("{dest} = ({}.tag == UINT32_C(1));", res.c_name));
        Ok(())
    }

    fn emit_result_bytes_err_code_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.err_code expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.err_code returns i32".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        if res.ty != Ty::ResultBytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.err_code expects result_bytes".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = ({}.tag == UINT32_C(0)) ? {}.payload.err : UINT32_C(0);",
            res.c_name, res.c_name
        ));
        Ok(())
    }

    fn emit_result_bytes_unwrap_or_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "result_bytes.unwrap_or expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.unwrap_or returns bytes".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        let default = self.emit_expr(&args[1])?;
        if res.ty != Ty::ResultBytes || default.ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "result_bytes.unwrap_or expects (result_bytes, bytes default)".to_string(),
            ));
        }
        self.line(&format!("if ({}.tag == UINT32_C(1)) {{", res.c_name));
        self.indent += 1;
        self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
        self.line(&format!("{}.payload.ok = rt_bytes_empty(ctx);", res.c_name));
        self.line(&format!("{}.tag = UINT32_C(0);", res.c_name));
        self.indent -= 1;
        self.line("} else {");
        self.indent += 1;
        self.line(&format!("{dest} = {};", default.c_name));
        self.line(&format!("{} = rt_bytes_empty(ctx);", default.c_name));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    fn emit_try_to(&mut self, args: &[Expr], dest_ty: Ty, dest: &str) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "try expects 1 arg".to_string(),
            ));
        }
        let res = self.emit_expr(&args[0])?;
        match res.ty {
            Ty::ResultI32 => {
                if self.fn_ret_ty != Ty::ResultI32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_i32) requires function return type result_i32".to_string(),
                    ));
                }
                if dest_ty != Ty::I32 {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_i32) returns i32".to_string(),
                    ));
                }
                self.line(&format!("if ({}.tag == UINT32_C(0)) {{", res.c_name));
                self.indent += 1;
                for (ty, c_name) in self.live_owned_drop_list(Some(&res.c_name)) {
                    self.emit_drop_var(ty, &c_name);
                }
                self.line(&format!("return {};", res.c_name));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
                Ok(())
            }
            Ty::ResultBytes => {
                if self.fn_ret_ty != Ty::ResultBytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_bytes) requires function return type result_bytes".to_string(),
                    ));
                }
                if dest_ty != Ty::Bytes {
                    return Err(CompilerError::new(
                        CompileErrorKind::Typing,
                        "try(result_bytes) returns bytes".to_string(),
                    ));
                }
                self.line(&format!("if ({}.tag == UINT32_C(0)) {{", res.c_name));
                self.indent += 1;
                for (ty, c_name) in self.live_owned_drop_list(Some(&res.c_name)) {
                    self.emit_drop_var(ty, &c_name);
                }
                self.line(&format!("return {};", res.c_name));
                self.indent -= 1;
                self.line("}");
                self.line(&format!("{dest} = {}.payload.ok;", res.c_name));
                self.line(&format!("{}.payload.ok = rt_bytes_empty(ctx);", res.c_name));
                self.line(&format!("{}.tag = UINT32_C(0);", res.c_name));
                Ok(())
            }
            other => Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("try expects result_i32 or result_bytes, got {other:?}"),
            )),
        }
    }

    fn emit_map_u32_new_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.new expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.new returns i32 handle".to_string(),
            ));
        }
        let cap = self.emit_expr(&args[0])?;
        if cap.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.new cap must be i32".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_map_u32_new(ctx, {});", cap.c_name));
        Ok(())
    }

    fn emit_map_u32_len_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.len expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.len returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.len expects i32 handle".to_string(),
            ));
        }
        self.line(&format!("{dest} = rt_map_u32_len(ctx, {});", h.c_name));
        Ok(())
    }

    fn emit_map_u32_get_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.get expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.get returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        let default = self.emit_expr(&args[2])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 || default.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.get expects (handle, key, default) all i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_get(ctx, {}, {}, {});",
            h.c_name, key.c_name, default.c_name
        ));
        Ok(())
    }

    fn emit_map_u32_set_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 3 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.set expects 3 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.set returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        let val = self.emit_expr(&args[2])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 || val.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.set expects (handle, key, val) all i32".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_set(ctx, {}, {}, {});",
            h.c_name, key.c_name, val.c_name
        ));
        Ok(())
    }

    fn emit_map_u32_contains_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.contains expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.contains returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.contains expects (handle, key)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_contains(ctx, {}, {});",
            h.c_name, key.c_name
        ));
        Ok(())
    }

    fn emit_map_u32_remove_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.remove expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.remove returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.remove expects (handle, key)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_remove(ctx, {}, {});",
            h.c_name, key.c_name
        ));
        Ok(())
    }

    fn emit_set_u32_add_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 2 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "set_u32.add expects 2 args".to_string(),
            ));
        }
        if dest_ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "set_u32.add returns i32".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        let key = self.emit_expr(&args[1])?;
        if h.ty != Ty::I32 || key.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "set_u32.add expects (handle, key)".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_set(ctx, {}, {}, UINT32_C(1));",
            h.c_name, key.c_name
        ));
        Ok(())
    }

    fn emit_set_u32_dump_u32le_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "set_u32.dump_u32le expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "set_u32.dump_u32le returns bytes".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "set_u32.dump_u32le expects i32 handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_set_u32_dump_u32le(ctx, {});",
            h.c_name
        ));
        Ok(())
    }

    fn emit_map_u32_dump_kv_u32le_u32le_to(
        &mut self,
        args: &[Expr],
        dest_ty: Ty,
        dest: &str,
    ) -> Result<(), CompilerError> {
        if args.len() != 1 {
            return Err(CompilerError::new(
                CompileErrorKind::Parse,
                "map_u32.dump_kv_u32le_u32le expects 1 arg".to_string(),
            ));
        }
        if dest_ty != Ty::Bytes {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.dump_kv_u32le_u32le returns bytes".to_string(),
            ));
        }
        let h = self.emit_expr(&args[0])?;
        if h.ty != Ty::I32 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                "map_u32.dump_kv_u32le_u32le expects i32 handle".to_string(),
            ));
        }
        self.line(&format!(
            "{dest} = rt_map_u32_dump_kv_u32le_u32le(ctx, {});",
            h.c_name
        ));
        Ok(())
    }

    fn infer_expr_in_new_scope(&self, expr: &Expr) -> Result<Ty, CompilerError> {
        let mut functions: BTreeMap<String, (Ty, Vec<Ty>)> = BTreeMap::new();
        for f in &self.program.functions {
            functions.insert(
                f.name.clone(),
                (f.ret_ty, f.params.iter().map(|p| p.ty).collect::<Vec<_>>()),
            );
        }
        for f in &self.program.async_functions {
            functions.insert(
                f.name.clone(),
                (Ty::I32, f.params.iter().map(|p| p.ty).collect::<Vec<_>>()),
            );
        }

        let mut infer = InferCtx {
            options: self.options.clone(),
            fn_ret_ty: self.fn_ret_ty,
            allow_async_ops: self.allow_async_ops,
            unsafe_depth: self.unsafe_depth,
            scopes: self
                .scopes
                .iter()
                .map(|s| {
                    s.iter()
                        .map(|(k, v)| (k.clone(), v.ty))
                        .collect::<BTreeMap<_, _>>()
                })
                .collect(),
            functions,
            extern_functions: self.extern_functions.clone(),
        };
        infer.infer(expr)
    }
}

fn c_escape_c_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}

struct InferCtx {
    options: CompileOptions,
    fn_ret_ty: Ty,
    allow_async_ops: bool,
    unsafe_depth: usize,
    scopes: Vec<BTreeMap<String, Ty>>,
    functions: BTreeMap<String, (Ty, Vec<Ty>)>,
    extern_functions: BTreeMap<String, ExternFunctionDecl>,
}

impl InferCtx {
    fn require_standalone_only(&self, head: &str) -> Result<(), CompilerError> {
        if !self.options.world.is_standalone_only() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{head} is standalone-only; compile with --world run-os or --world run-os-sandboxed"
                ),
            ));
        }
        Ok(())
    }

    fn require_unsafe_world(&self, head: &str) -> Result<(), CompilerError> {
        if !self.options.allow_unsafe() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{head} requires unsafe capability; {}",
                    self.options.hint_enable_unsafe()
                ),
            ));
        }
        Ok(())
    }

    fn require_ffi_world(&self, head: &str) -> Result<(), CompilerError> {
        if !self.options.allow_ffi() {
            return Err(CompilerError::new(
                CompileErrorKind::Unsupported,
                format!(
                    "{head} requires ffi capability; {}",
                    self.options.hint_enable_ffi()
                ),
            ));
        }
        Ok(())
    }

    fn require_unsafe_block(&self, head: &str) -> Result<(), CompilerError> {
        if self.unsafe_depth == 0 {
            return Err(CompilerError::new(
                CompileErrorKind::Typing,
                format!("unsafe-required: {head}"),
            ));
        }
        Ok(())
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn bind(&mut self, name: String, ty: Ty) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn lookup(&self, name: &str) -> Option<Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(*v);
            }
        }
        None
    }

    fn infer(&mut self, expr: &Expr) -> Result<Ty, CompilerError> {
        match expr {
            Expr::Int(_) => Ok(Ty::I32),
            Expr::Ident(name) => {
                if name == "input" {
                    return Ok(Ty::BytesView);
                }
                self.lookup(name).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Typing,
                        format!("unknown identifier: {name:?}"),
                    )
                })
            }
            Expr::List(items) => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];

                match head {
                    "begin" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "(begin ...) requires at least 1 expression".to_string(),
                            ));
                        }
                        self.push_scope();
                        for e in &args[..args.len() - 1] {
                            self.infer_stmt(e)?;
                        }
                        let ty = self.infer(&args[args.len() - 1])?;
                        self.pop_scope();
                        Ok(ty)
                    }
                    "unsafe" => {
                        self.require_unsafe_world("unsafe")?;
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "(unsafe ...) requires at least 1 expression".to_string(),
                            ));
                        }
                        self.unsafe_depth = self.unsafe_depth.saturating_add(1);
                        self.push_scope();
                        for e in &args[..args.len() - 1] {
                            self.infer_stmt(e)?;
                        }
                        let ty = self.infer(&args[args.len() - 1])?;
                        self.pop_scope();
                        self.unsafe_depth = self.unsafe_depth.saturating_sub(1);
                        Ok(ty)
                    }
                    "let" => {
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
                        let ty = self.infer(&args[1])?;
                        self.bind(name.to_string(), ty);
                        Ok(ty)
                    }
                    "set" => {
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
                        let prev = self.lookup(name).ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("set of unknown variable: {name:?}"),
                            )
                        })?;
                        let ty = self.infer(&args[1])?;
                        if ty != prev {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("type mismatch in set for variable {name:?}"),
                            ));
                        }
                        Ok(ty)
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "if condition must be i32".to_string(),
                            ));
                        }
                        self.push_scope();
                        let then_ty = self.infer(&args[1])?;
                        self.pop_scope();

                        self.push_scope();
                        let else_ty = self.infer(&args[2])?;
                        self.pop_scope();

                        if then_ty != else_ty && then_ty != Ty::Never && else_ty != Ty::Never {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!(
                                    "if branches must have same type (then={then_ty:?}, else={else_ty:?})"
                                ),
                            ));
                        }
                        Ok(if then_ty == Ty::Never {
                            else_ty
                        } else {
                            then_ty
                        })
                    }
                    "for" => {
                        if args.len() != 4 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "for form: (for <i> <start:i32> <end:i32> <body:any>)".to_string(),
                            ));
                        }
                        let var = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "for variable must be an identifier".to_string(),
                            )
                        })?;
                        match self.lookup(var) {
                            Some(Ty::I32) => {}
                            Some(_) => {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("for variable must be i32: {var:?}"),
                                ));
                            }
                            None => {
                                self.bind(var.to_string(), Ty::I32);
                            }
                        }
                        if self.infer(&args[1])? != Ty::I32 || self.infer(&args[2])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "for bounds must be i32".to_string(),
                            ));
                        }
                        self.push_scope();
                        self.infer_stmt(&args[3])?;
                        self.pop_scope();
                        Ok(Ty::I32)
                    }
                    "return" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "return form: (return <expr>)".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != self.fn_ret_ty {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("return expression must evaluate to {:?}", self.fn_ret_ty),
                            ));
                        }
                        Ok(Ty::Never)
                    }
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<u" | ">>u" | "=" | "!="
                    | "<" | "<=" | ">" | ">=" | "<u" | ">=u" | ">u" | "<=u" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 args"),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "bytes.as_ptr" | "bytes.as_mut_ptr" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects bytes"),
                            ));
                        }
                        Ok(if head == "bytes.as_ptr" {
                            Ty::PtrConstU8
                        } else {
                            Ty::PtrMutU8
                        })
                    }
                    "view.as_ptr" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.as_ptr expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.as_ptr expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::PtrConstU8)
                    }
                    "vec_u8.as_ptr" | "vec_u8.as_mut_ptr" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects vec_u8"),
                            ));
                        }
                        Ok(if head == "vec_u8.as_ptr" {
                            Ty::PtrConstU8
                        } else {
                            Ty::PtrMutU8
                        })
                    }
                    "ptr.null" => {
                        self.require_unsafe_world(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.null expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::PtrMutVoid)
                    }
                    "ptr.as_const" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.as_const expects 1 arg".to_string(),
                            ));
                        }
                        let ty = self.infer(&args[0])?;
                        Ok(match ty {
                            Ty::PtrMutU8 => Ty::PtrConstU8,
                            Ty::PtrMutVoid => Ty::PtrConstVoid,
                            Ty::PtrMutI32 => Ty::PtrConstI32,
                            Ty::PtrConstU8 | Ty::PtrConstVoid | Ty::PtrConstI32 => ty,
                            _ => {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    "ptr.as_const expects a raw pointer".to_string(),
                                ));
                            }
                        })
                    }
                    "ptr.cast" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.cast form: (ptr.cast <ptr_ty> <ptr>)".to_string(),
                            ));
                        }
                        let ty_name = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.cast target type must be an identifier".to_string(),
                            )
                        })?;
                        let target = Ty::parse_named(ty_name).ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("ptr.cast unknown type: {ty_name:?}"),
                            )
                        })?;
                        if !target.is_ptr_ty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("ptr.cast target must be a pointer type, got {target:?}"),
                            ));
                        }
                        let src = self.infer(&args[1])?;
                        if !src.is_ptr_ty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("ptr.cast expects a pointer, got {src:?}"),
                            ));
                        }
                        Ok(target)
                    }
                    "addr_of" | "addr_of_mut" => {
                        self.require_unsafe_world(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        let name = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects an identifier"),
                            )
                        })?;
                        if name != "input" && self.lookup(name).is_none() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("unknown identifier: {name:?}"),
                            ));
                        }
                        Ok(if head == "addr_of" {
                            Ty::PtrConstVoid
                        } else {
                            Ty::PtrMutVoid
                        })
                    }
                    "ptr.add" | "ptr.sub" | "ptr.offset" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        let ptr_ty = self.infer(&args[0])?;
                        if !matches!(
                            ptr_ty,
                            Ty::PtrConstU8 | Ty::PtrMutU8 | Ty::PtrConstI32 | Ty::PtrMutI32
                        ) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects a non-void raw pointer"),
                            ));
                        }
                        if self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 offset"),
                            ));
                        }
                        Ok(ptr_ty)
                    }
                    "ptr.read_u8" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.read_u8 expects 1 arg".to_string(),
                            ));
                        }
                        let ptr_ty = self.infer(&args[0])?;
                        if !matches!(ptr_ty, Ty::PtrConstU8 | Ty::PtrMutU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.read_u8 expects ptr_const_u8 or ptr_mut_u8".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "ptr.write_u8" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.write_u8 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::PtrMutU8 || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.write_u8 expects (ptr_mut_u8, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "ptr.read_i32" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.read_i32 expects 1 arg".to_string(),
                            ));
                        }
                        let ptr_ty = self.infer(&args[0])?;
                        if !matches!(ptr_ty, Ty::PtrConstI32 | Ty::PtrMutI32) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.read_i32 expects ptr_const_i32 or ptr_mut_i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "ptr.write_i32" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "ptr.write_i32 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::PtrMutI32
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "ptr.write_i32 expects (ptr_mut_i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "memcpy" | "memmove" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 3 args"),
                            ));
                        }
                        let dest_ptr = self.infer(&args[0])?;
                        let src_ptr = self.infer(&args[1])?;
                        if dest_ptr != Ty::PtrMutVoid {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects dest ptr_mut_void"),
                            ));
                        }
                        if src_ptr != Ty::PtrConstVoid && src_ptr != Ty::PtrMutVoid {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects src ptr_const_void"),
                            ));
                        }
                        if self.infer(&args[2])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 len"),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "memset" => {
                        self.require_unsafe_world(head)?;
                        self.require_unsafe_block(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "memset expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::PtrMutVoid
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "memset expects (ptr_mut_void, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "bytes.len" | "bytes.get_u8" | "bytes.eq" | "bytes.cmp_range" => {
                        let (want_args, want_ty) = match head {
                            "bytes.len" => (1, Ty::I32),
                            "bytes.get_u8" => (2, Ty::I32),
                            "bytes.eq" => (2, Ty::I32),
                            "bytes.cmp_range" => (6, Ty::I32),
                            _ => unreachable!(),
                        };
                        if args.len() != want_args {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects {want_args} args"),
                            ));
                        }
                        match head {
                            "bytes.len" => {
                                let b = self.infer(&args[0])?;
                                if b != Ty::Bytes && b != Ty::BytesView {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("{head} expects bytes_view"),
                                    ));
                                }
                                Ok(want_ty)
                            }
                            "bytes.get_u8" => {
                                let b = self.infer(&args[0])?;
                                if (b != Ty::Bytes && b != Ty::BytesView)
                                    || self.infer(&args[1])? != Ty::I32
                                {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "bytes.get_u8 expects (bytes_view, i32)".to_string(),
                                    ));
                                }
                                Ok(want_ty)
                            }
                            "bytes.eq" => {
                                let a = self.infer(&args[0])?;
                                let b = self.infer(&args[1])?;
                                if (a != Ty::Bytes && a != Ty::BytesView)
                                    || (b != Ty::Bytes && b != Ty::BytesView)
                                {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("{head} expects (bytes_view, bytes_view)"),
                                    ));
                                }
                                Ok(want_ty)
                            }
                            "bytes.cmp_range" => {
                                let a = self.infer(&args[0])?;
                                let b = self.infer(&args[3])?;
                                if (a != Ty::Bytes && a != Ty::BytesView)
                                    || self.infer(&args[1])? != Ty::I32
                                    || self.infer(&args[2])? != Ty::I32
                                    || (b != Ty::Bytes && b != Ty::BytesView)
                                    || self.infer(&args[4])? != Ty::I32
                                    || self.infer(&args[5])? != Ty::I32
                                {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "bytes.cmp_range expects (bytes_view, i32, i32, bytes_view, i32, i32)"
                                            .to_string(),
                                    ));
                                }
                                Ok(want_ty)
                            }
                            _ => unreachable!(),
                        }
                    }
                    "bytes.set_u8" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.set_u8 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.set_u8 expects (bytes, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "math.f64.add_v1" | "math.f64.sub_v1" | "math.f64.mul_v1"
                    | "math.f64.div_v1" | "math.f64.pow_v1" | "math.f64.atan2_v1"
                    | "math.f64.min_v1" | "math.f64.max_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        let a = self.infer(&args[0])?;
                        let b = self.infer(&args[1])?;
                        if (a != Ty::Bytes && a != Ty::BytesView)
                            || (b != Ty::Bytes && b != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (bytes_view, bytes_view)"),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "math.f64.sqrt_v1"
                    | "math.f64.neg_v1"
                    | "math.f64.abs_v1"
                    | "math.f64.sin_v1"
                    | "math.f64.cos_v1"
                    | "math.f64.tan_v1"
                    | "math.f64.exp_v1"
                    | "math.f64.log_v1"
                    | "math.f64.floor_v1"
                    | "math.f64.ceil_v1"
                    | "math.f64.fmt_shortest_v1"
                    | "math.f64.to_bits_u64le_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        let x = self.infer(&args[0])?;
                        if x != Ty::Bytes && x != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects bytes_view"),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "math.f64.parse_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "math.f64.parse_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let s = self.infer(&args[0])?;
                        if s != Ty::Bytes && s != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.parse_v1 expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes)
                    }
                    "math.f64.from_i32_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "math.f64.from_i32_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.from_i32_v1 expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "math.f64.to_i32_trunc_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "math.f64.to_i32_trunc_v1 expects 1 arg".to_string(),
                            ));
                        }
                        let x = self.infer(&args[0])?;
                        if x != Ty::Bytes && x != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "math.f64.to_i32_trunc_v1 expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32)
                    }
                    "bytes.alloc" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.alloc expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.alloc length must be i32".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "bytes.empty" => {
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.empty expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "bytes1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes1 expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "bytes.lit" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.lit expects 1 arg".to_string(),
                            ));
                        }
                        if args[0].as_ident().is_none() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.lit expects an identifier".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "bytes.copy" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.copy expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.copy expects (bytes, bytes)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "bytes.concat" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.concat expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.concat expects (bytes, bytes)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "bytes.slice" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.slice expects 3 args".to_string(),
                            ));
                        }
                        let b = self.infer(&args[0])?;
                        if (b != Ty::Bytes && b != Ty::BytesView)
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.slice expects (bytes_view, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "bytes.view" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.view expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.view expects bytes".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView)
                    }
                    "bytes.subview" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bytes.subview expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bytes.subview expects (bytes, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView)
                    }
                    "view.len" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.len expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.len expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "view.get_u8" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.get_u8 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.get_u8 expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "view.slice" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.slice expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.slice expects (bytes_view, i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView)
                    }
                    "view.to_bytes" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.to_bytes expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.to_bytes expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "view.eq" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.eq expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.eq expects (bytes_view, bytes_view)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "view.cmp_range" => {
                        if args.len() != 6 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "view.cmp_range expects 6 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                            || self.infer(&args[3])? != Ty::BytesView
                            || self.infer(&args[4])? != Ty::I32
                            || self.infer(&args[5])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "view.cmp_range expects (bytes_view, i32, i32, bytes_view, i32, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "fs.read" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.read is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "fs.read expects 1 arg".to_string(),
                            ));
                        }
                        let path_ty = self.infer(&args[0])?;
                        if !matches!(path_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.read expects bytes_view path".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "fs.read_async" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.read_async is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "fs.read_async expects 1 arg".to_string(),
                            ));
                        }
                        let path_ty = self.infer(&args[0])?;
                        if !matches!(path_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.read_async expects bytes_view path".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "fs.open_read" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.open_read is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "fs.open_read expects 1 arg".to_string(),
                            ));
                        }
                        let path_ty = self.infer(&args[0])?;
                        if !matches!(path_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.open_read expects bytes_view path".to_string(),
                            ));
                        }
                        Ok(Ty::Iface)
                    }
                    "io.open_read_bytes" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "io.open_read_bytes expects 1 arg".to_string(),
                            ));
                        }
                        let b_ty = self.infer(&args[0])?;
                        if !matches!(b_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "io.open_read_bytes expects bytes".to_string(),
                            ));
                        }
                        Ok(Ty::Iface)
                    }
                    "fs.list_dir" => {
                        if !self.options.enable_fs {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "fs.list_dir is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "fs.list_dir expects 1 arg".to_string(),
                            ));
                        }
                        let path_ty = self.infer(&args[0])?;
                        if !matches!(path_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "fs.list_dir expects bytes_view path".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.fs.read_file" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.read_file expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.read_file expects bytes path".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.fs.write_file" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.write_file expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.write_file expects (bytes path, bytes data)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.fs.read_all_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.read_all_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.read_all_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes)
                    }
                    "os.fs.write_all_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.write_all_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::Bytes
                            || self.infer(&args[2])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.write_all_v1 expects (bytes path, bytes data, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32)
                    }
                    "os.fs.mkdirs_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.mkdirs_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.mkdirs_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32)
                    }
                    "os.fs.remove_file_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.remove_file_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.remove_file_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32)
                    }
                    "os.fs.remove_dir_all_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.remove_dir_all_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.remove_dir_all_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32)
                    }
                    "os.fs.rename_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.rename_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::Bytes
                            || self.infer(&args[2])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.rename_v1 expects (bytes src, bytes dst, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32)
                    }
                    "os.fs.list_dir_sorted_text_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.list_dir_sorted_text_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.list_dir_sorted_text_v1 expects (bytes path, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes)
                    }
                    "os.fs.walk_glob_sorted_text_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.walk_glob_sorted_text_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes
                            || self.infer(&args[1])? != Ty::Bytes
                            || self.infer(&args[2])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.walk_glob_sorted_text_v1 expects (bytes root, bytes glob, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes)
                    }
                    "os.fs.stat_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.fs.stat_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.fs.stat_v1 expects (bytes path, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes)
                    }
                    "os.db.sqlite.open_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.sqlite.open_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.sqlite.query_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.sqlite.query_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.sqlite.exec_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.sqlite.exec_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.sqlite.close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.sqlite.close_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.sqlite.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.pg.open_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.pg.open_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.pg.query_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.pg.query_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.pg.exec_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.pg.exec_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.pg.close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.pg.close_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.pg.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.mysql.open_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.mysql.open_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.mysql.query_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.mysql.query_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.query_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.mysql.exec_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.mysql.exec_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.exec_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.mysql.close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.mysql.close_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.mysql.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.redis.open_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.redis.open_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.open_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.redis.cmd_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.redis.cmd_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.cmd_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.db.redis.close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.db.redis.close_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.db.redis.close_v1 expects (bytes req, bytes caps)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.env.get" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.env.get expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.env.get expects bytes key".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.time.now_unix_ms" => {
                        self.require_standalone_only(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.now_unix_ms expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.time.now_instant_v1" => {
                        self.require_standalone_only(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.now_instant_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.time.sleep_ms_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.sleep_ms_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.sleep_ms_v1 expects i32 ms".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.time.local_tzid_v1" => {
                        self.require_standalone_only(head)?;
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.local_tzid_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.time.tzdb_is_valid_tzid_v1" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.tzdb_is_valid_tzid_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_is_valid_tzid_v1 expects bytes_view tzid".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.time.tzdb_offset_duration_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.tzdb_offset_duration_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.time.tzdb_offset_duration_v1 expects (bytes_view tzid, i32 unix_s_lo, i32 unix_s_hi)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.time.tzdb_snapshot_id_v1" => {
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.time.tzdb_snapshot_id_v1 expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.process.exit" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.exit expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.exit expects i32 code".to_string(),
                            ));
                        }
                        Ok(Ty::Never)
                    }
                    "os.process.spawn_capture_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.spawn_capture_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.spawn_capture_v1 expects (bytes req, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.process.spawn_piped_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.spawn_piped_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.spawn_piped_v1 expects (bytes req, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.process.try_join_capture_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.try_join_capture_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.try_join_capture_v1 expects i32 proc handle"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::OptionBytes)
                    }
                    "os.process.join_capture_v1" | "std.os.process.join_capture_v1" => {
                        self.require_standalone_only(head)?;
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.join_capture_v1 is only allowed in solve or defasync"
                                    .to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.join_capture_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.join_capture_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.process.stdout_read_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.stdout_read_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdout_read_v1 expects (i32 handle, i32 max)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.process.stderr_read_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.stderr_read_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stderr_read_v1 expects (i32 handle, i32 max)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.process.stdin_write_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.stdin_write_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdin_write_v1 expects (i32 handle, bytes chunk)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.process.stdin_close_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.stdin_close_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.stdin_close_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.process.try_wait_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.try_wait_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.try_wait_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.process.join_exit_v1" | "std.os.process.join_exit_v1" => {
                        self.require_standalone_only(head)?;
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "os.process.join_exit_v1 is only allowed in solve or defasync"
                                    .to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.join_exit_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.join_exit_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.process.take_exit_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.take_exit_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.take_exit_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.process.kill_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.kill_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.kill_v1 expects (i32 proc_handle, i32 sig)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.process.drop_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.drop_v1 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.drop_v1 expects i32 proc handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "os.process.run_capture_v1" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.process.run_capture_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.process.run_capture_v1 expects (bytes req, bytes caps)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "os.net.http_request" => {
                        self.require_standalone_only(head)?;
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "os.net.http_request expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "os.net.http_request expects bytes req".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "rr.send_request" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.send_request is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.send_request expects 1 arg".to_string(),
                            ));
                        }
                        let req_ty = self.infer(&args[0])?;
                        if !matches!(req_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.send_request expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "rr.fetch" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.fetch is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.fetch expects 1 arg".to_string(),
                            ));
                        }
                        let key_ty = self.infer(&args[0])?;
                        if !matches!(key_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.fetch expects bytes_view key".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "rr.send" => {
                        if !self.options.enable_rr {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "rr.send is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "rr.send expects 1 arg".to_string(),
                            ));
                        }
                        let req_ty = self.infer(&args[0])?;
                        if !matches!(req_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "rr.send expects bytes_view req".to_string(),
                            ));
                        }
                        Ok(Ty::Iface)
                    }
                    "kv.get" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "kv.get expects 1 arg".to_string(),
                            ));
                        }
                        let key_ty = self.infer(&args[0])?;
                        if !matches!(key_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get expects bytes_view key".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "kv.get_async" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get_async is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "kv.get_async expects 1 arg".to_string(),
                            ));
                        }
                        let key_ty = self.infer(&args[0])?;
                        if !matches!(key_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get_async expects bytes_view key".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "kv.get_stream" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.get_stream is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "kv.get_stream expects 1 arg".to_string(),
                            ));
                        }
                        let key_ty = self.infer(&args[0])?;
                        if !matches!(key_ty, Ty::Bytes | Ty::BytesView | Ty::VecU8) {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.get_stream expects bytes_view key".to_string(),
                            ));
                        }
                        Ok(Ty::Iface)
                    }
                    "kv.set" => {
                        if !self.options.enable_kv {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "kv.set is disabled in this world".to_string(),
                            ));
                        }
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "kv.set expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "kv.set expects (bytes, bytes)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "io.read" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "io.read expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Iface || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "io.read expects (iface, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "iface.make_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "iface.make_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "iface.make_v1 expects (i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Iface)
                    }
                    "bufread.new" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bufread.new expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Iface || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.new expects (iface, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "bufread.fill" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bufread.fill expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.fill expects i32 bufread handle".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView)
                    }
                    "bufread.consume" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "bufread.consume expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "bufread.consume expects (i32, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "await" | "task.spawn" => {
                        if head == "await" && !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "await is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 task handle"),
                            ));
                        }
                        match head {
                            "await" => Ok(Ty::Bytes),
                            "task.spawn" => Ok(Ty::I32),
                            _ => unreachable!(),
                        }
                    }
                    "task.is_finished" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.is_finished expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.is_finished expects i32 task handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "task.try_join.bytes" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.try_join.bytes expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.try_join.bytes expects i32 task handle".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes)
                    }
                    "task.join.bytes" | "task.cancel" => {
                        if head == "task.join.bytes" && !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "task.join.bytes is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32 task handle"),
                            ));
                        }
                        match head {
                            "task.join.bytes" => Ok(Ty::Bytes),
                            "task.cancel" => Ok(Ty::I32),
                            _ => unreachable!(),
                        }
                    }
                    "task.yield" => {
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "task.yield is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.yield expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "task.sleep" => {
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "task.sleep is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "task.sleep expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "task.sleep expects i32 ticks".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "chan.bytes.new" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.new expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.new expects i32 cap".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "chan.bytes.send" => {
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "chan.bytes.send is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.send expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.send expects (i32, bytes)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "chan.bytes.try_send" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.try_send expects 2 args".to_string(),
                            ));
                        }
                        let payload = self.infer(&args[1])?;
                        if self.infer(&args[0])? != Ty::I32
                            || (payload != Ty::Bytes
                                && payload != Ty::BytesView
                                && payload != Ty::VecU8)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.try_send expects (i32, bytes_view)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "chan.bytes.recv" => {
                        if !self.allow_async_ops {
                            return Err(CompilerError::new(
                                CompileErrorKind::Unsupported,
                                "chan.bytes.recv is only allowed in solve or defasync".to_string(),
                            ));
                        }
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.recv expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.recv expects i32 chan handle".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "chan.bytes.try_recv" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.try_recv expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.try_recv expects i32 chan handle".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes)
                    }
                    "chan.bytes.close" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "chan.bytes.close expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "chan.bytes.close expects i32 chan handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "codec.read_u32_le" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "codec.read_u32_le expects 2 args".to_string(),
                            ));
                        }
                        let b = self.infer(&args[0])?;
                        if (b != Ty::Bytes && b != Ty::BytesView)
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "codec.read_u32_le expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "codec.write_u32_le" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "codec.write_u32_le expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "codec.write_u32_le expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "fmt.u32_to_dec" | "fmt.s32_to_dec" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32"),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "parse.u32_dec" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "parse.u32_dec expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "parse.u32_dec expects bytes_view".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "parse.u32_dec_at" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "parse.u32_dec_at expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "parse.u32_dec_at expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "prng.lcg_next_u32" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "prng.lcg_next_u32 expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "prng.lcg_next_u32 expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "regex.compile_opts_v1" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.compile_opts_v1 expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.compile_opts_v1 expects (bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "regex.exec_from_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.exec_from_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.exec_from_v1 expects (bytes_view, bytes_view, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "regex.exec_caps_from_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.exec_caps_from_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.exec_caps_from_v1 expects (bytes_view, bytes_view, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "regex.find_all_x7sl_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.find_all_x7sl_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.find_all_x7sl_v1 expects (bytes_view, bytes_view, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "regex.split_v1" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.split_v1 expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.split_v1 expects (bytes_view, bytes_view, i32)".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "regex.replace_all_v1" => {
                        if args.len() != 4 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "regex.replace_all_v1 expects 4 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::BytesView
                            || self.infer(&args[1])? != Ty::BytesView
                            || self.infer(&args[2])? != Ty::BytesView
                            || self.infer(&args[3])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "regex.replace_all_v1 expects (bytes_view, bytes_view, bytes_view, i32)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "vec_u8.with_capacity" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.with_capacity expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.with_capacity expects i32 cap".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8)
                    }
                    "map_u32.new" | "set_u32.new" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 1 arg"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects i32"),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "vec_u8.len" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.len expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.len expects vec_u8".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "map_u32.len" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_u32.len expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.len expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "vec_u8.reserve_exact" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.reserve_exact expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.reserve_exact expects (vec_u8, i32 additional)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8)
                    }
                    "vec_u8.extend_zeroes" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.extend_zeroes expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_zeroes expects (vec_u8, i32 n)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8)
                    }
                    "vec_u8.get" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.get expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.get expects (vec_u8, i32 index)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "vec_u8.set" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.set expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.set expects (vec_u8, i32 index, i32 value)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8)
                    }
                    "vec_u8.push" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.push expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.push expects (vec_u8, i32 value)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8)
                    }
                    "vec_u8.extend_bytes" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.extend_bytes expects 2 args".to_string(),
                            ));
                        }
                        let b = self.infer(&args[1])?;
                        if self.infer(&args[0])? != Ty::VecU8
                            || (b != Ty::Bytes && b != Ty::BytesView)
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_bytes expects (vec_u8, bytes_view)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8)
                    }
                    "vec_u8.extend_bytes_range" => {
                        if args.len() != 4 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.extend_bytes_range expects 4 args".to_string(),
                            ));
                        }
                        let b = self.infer(&args[1])?;
                        if self.infer(&args[0])? != Ty::VecU8
                            || (b != Ty::Bytes && b != Ty::BytesView)
                            || self.infer(&args[2])? != Ty::I32
                            || self.infer(&args[3])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.extend_bytes_range expects (vec_u8, bytes_view, i32 start, i32 len)".to_string(),
                            ));
                        }
                        Ok(Ty::VecU8)
                    }
                    "vec_u8.into_bytes" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.into_bytes expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.into_bytes expects vec_u8".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "vec_u8.as_view" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "vec_u8.as_view expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::VecU8 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "vec_u8.as_view expects vec_u8".to_string(),
                            ));
                        }
                        Ok(Ty::BytesView)
                    }
                    "option_i32.none" => {
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_i32.none expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::OptionI32)
                    }
                    "option_i32.some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_i32.some expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_i32.some expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::OptionI32)
                    }
                    "option_i32.is_some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_i32.is_some expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::OptionI32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_i32.is_some expects option_i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "option_i32.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_i32.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::OptionI32
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_i32.unwrap_or expects (option_i32, i32 default)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "option_bytes.none" => {
                        if !args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes.none expects 0 args".to_string(),
                            ));
                        }
                        Ok(Ty::OptionBytes)
                    }
                    "option_bytes.some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes.some expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_bytes.some expects bytes".to_string(),
                            ));
                        }
                        Ok(Ty::OptionBytes)
                    }
                    "option_bytes.is_some" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes.is_some expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::OptionBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_bytes.is_some expects option_bytes".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "option_bytes.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "option_bytes.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::OptionBytes
                            || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "option_bytes.unwrap_or expects (option_bytes, bytes default)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "result_i32.ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.ok expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.ok expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32)
                    }
                    "result_i32.err" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.err expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.err expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::ResultI32)
                    }
                    "result_i32.is_ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.is_ok expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultI32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.is_ok expects result_i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "result_i32.err_code" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.err_code expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultI32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.err_code expects result_i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "result_i32.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_i32.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultI32
                            || self.infer(&args[1])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_i32.unwrap_or expects (result_i32, i32 default)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "result_bytes.ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.ok expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::Bytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.ok expects bytes".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes)
                    }
                    "result_bytes.err" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.err expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.err expects i32".to_string(),
                            ));
                        }
                        Ok(Ty::ResultBytes)
                    }
                    "result_bytes.is_ok" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.is_ok expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.is_ok expects result_bytes".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "result_bytes.err_code" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.err_code expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultBytes {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.err_code expects result_bytes".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "result_bytes.unwrap_or" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "result_bytes.unwrap_or expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::ResultBytes
                            || self.infer(&args[1])? != Ty::Bytes
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "result_bytes.unwrap_or expects (result_bytes, bytes default)"
                                    .to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "try" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "try expects 1 arg".to_string(),
                            ));
                        }
                        match self.infer(&args[0])? {
                            Ty::ResultI32 => {
                                if self.fn_ret_ty != Ty::ResultI32 {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "try(result_i32) requires function return type result_i32"
                                            .to_string(),
                                    ));
                                }
                                Ok(Ty::I32)
                            }
                            Ty::ResultBytes => {
                                if self.fn_ret_ty != Ty::ResultBytes {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        "try(result_bytes) requires function return type result_bytes"
                                            .to_string(),
                                    ));
                                }
                                Ok(Ty::Bytes)
                            }
                            other => Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("try expects result_i32 or result_bytes, got {other:?}"),
                            )),
                        }
                    }
                    "map_u32.get" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_u32.get expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.get expects (handle, key, default) all i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "map_u32.set" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_u32.set expects 3 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32
                            || self.infer(&args[1])? != Ty::I32
                            || self.infer(&args[2])? != Ty::I32
                        {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.set expects (handle, key, val) all i32".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "map_u32.contains" | "set_u32.contains" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (handle, key)"),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "map_u32.remove" | "set_u32.remove" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                format!("{head} expects 2 args"),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                format!("{head} expects (handle, key)"),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "set_u32.add" => {
                        if args.len() != 2 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "set_u32.add expects 2 args".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 || self.infer(&args[1])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "set_u32.add expects (handle, key)".to_string(),
                            ));
                        }
                        Ok(Ty::I32)
                    }
                    "set_u32.dump_u32le" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "set_u32.dump_u32le expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "set_u32.dump_u32le expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    "map_u32.dump_kv_u32le_u32le" => {
                        if args.len() != 1 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "map_u32.dump_kv_u32le_u32le expects 1 arg".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "map_u32.dump_kv_u32le_u32le expects i32 handle".to_string(),
                            ));
                        }
                        Ok(Ty::Bytes)
                    }
                    _ => {
                        if let Some(f) = self.extern_functions.get(head).cloned() {
                            self.require_ffi_world(head)?;
                            self.require_unsafe_world(head)?;
                            self.require_unsafe_block(head)?;

                            if args.len() != f.params.len() {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Parse,
                                    format!("call {head:?} expects {} args", f.params.len()),
                                ));
                            }
                            for (i, (arg, p)) in args.iter().zip(f.params.iter()).enumerate() {
                                let got = self.infer(arg)?;
                                let want = p.ty;
                                let ok = got == want
                                    || matches!(
                                        (got, want),
                                        (Ty::Bytes, Ty::BytesView)
                                            | (Ty::VecU8, Ty::BytesView)
                                            | (Ty::PtrMutU8, Ty::PtrConstU8)
                                            | (Ty::PtrMutVoid, Ty::PtrConstVoid)
                                            | (Ty::PtrMutI32, Ty::PtrConstI32)
                                    );
                                if !ok {
                                    return Err(CompilerError::new(
                                        CompileErrorKind::Typing,
                                        format!("call {head:?} arg {i} expects {want:?}"),
                                    ));
                                }
                            }
                            Ok(f.ret_ty)
                        } else {
                            match self.functions.get(head).cloned() {
                                Some((ret_ty, params)) => {
                                    if args.len() != params.len() {
                                        return Err(CompilerError::new(
                                            CompileErrorKind::Parse,
                                            format!("call {head:?} expects {} args", params.len()),
                                        ));
                                    }
                                    for (i, (arg, want_ty)) in
                                        args.iter().zip(params.iter()).enumerate()
                                    {
                                        let got = self.infer(arg)?;
                                        let ok = got == *want_ty
                                            || matches!(
                                                (got, *want_ty),
                                                (Ty::Bytes, Ty::BytesView)
                                                    | (Ty::VecU8, Ty::BytesView)
                                            );
                                        if !ok {
                                            return Err(CompilerError::new(
                                                CompileErrorKind::Typing,
                                                format!(
                                                    "call {head:?} arg {i} expects {want_ty:?}"
                                                ),
                                            ));
                                        }
                                    }
                                    Ok(ret_ty)
                                }
                                None => Err(CompilerError::new(
                                    CompileErrorKind::Unsupported,
                                    format!("unsupported head: {head:?}"),
                                )),
                            }
                        }
                    }
                }
            }
        }
    }

    fn infer_stmt(&mut self, expr: &Expr) -> Result<Ty, CompilerError> {
        let ty = match expr {
            Expr::List(items) => {
                let head = items.first().and_then(Expr::as_ident).ok_or_else(|| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        "list head must be an identifier".to_string(),
                    )
                })?;
                let args = &items[1..];
                match head {
                    "begin" => {
                        if args.is_empty() {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "(begin ...) requires at least 1 expression".to_string(),
                            ));
                        }
                        self.push_scope();
                        let mut result = Ty::I32;
                        for e in args {
                            if self.infer_stmt(e)? == Ty::Never {
                                result = Ty::Never;
                            }
                        }
                        self.pop_scope();
                        result
                    }
                    "if" => {
                        if args.len() != 3 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "if form: (if <cond:i32> <then:any> <else:any>)".to_string(),
                            ));
                        }
                        if self.infer(&args[0])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "if condition must be i32".to_string(),
                            ));
                        }
                        self.push_scope();
                        let then_ty = self.infer_stmt(&args[1])?;
                        self.pop_scope();

                        self.push_scope();
                        let else_ty = self.infer_stmt(&args[2])?;
                        self.pop_scope();

                        if then_ty == Ty::Never && else_ty == Ty::Never {
                            Ty::Never
                        } else {
                            Ty::I32
                        }
                    }
                    "for" => {
                        if args.len() != 4 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Parse,
                                "for form: (for <i> <start:i32> <end:i32> <body:any>)".to_string(),
                            ));
                        }
                        let var = args[0].as_ident().ok_or_else(|| {
                            CompilerError::new(
                                CompileErrorKind::Parse,
                                "for variable must be an identifier".to_string(),
                            )
                        })?;
                        match self.lookup(var) {
                            Some(Ty::I32) => {}
                            Some(_) => {
                                return Err(CompilerError::new(
                                    CompileErrorKind::Typing,
                                    format!("for variable must be i32: {var:?}"),
                                ));
                            }
                            None => {
                                self.bind(var.to_string(), Ty::I32);
                            }
                        }
                        if self.infer(&args[1])? != Ty::I32 || self.infer(&args[2])? != Ty::I32 {
                            return Err(CompilerError::new(
                                CompileErrorKind::Typing,
                                "for bounds must be i32".to_string(),
                            ));
                        }
                        self.push_scope();
                        let _ = self.infer_stmt(&args[3])?;
                        self.pop_scope();
                        Ty::I32
                    }
                    _ => self.infer(expr)?,
                }
            }
            _ => self.infer(expr)?,
        };

        Ok(if ty == Ty::Never { Ty::Never } else { Ty::I32 })
    }
}

fn c_user_fn_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    out.push_str("user_");
    for ch in name.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => out.push(ch),
            '.' => out.push('_'),
            _ => out.push('_'),
        }
    }
    out
}

fn c_async_new_name(name: &str) -> String {
    let base = c_user_fn_name(name);
    let suffix = base.strip_prefix("user_").unwrap_or(base.as_str());
    format!("async_new_{suffix}")
}

fn c_async_poll_name(name: &str) -> String {
    let base = c_user_fn_name(name);
    let suffix = base.strip_prefix("user_").unwrap_or(base.as_str());
    format!("async_poll_{suffix}")
}

fn c_async_drop_name(name: &str) -> String {
    let base = c_user_fn_name(name);
    let suffix = base.strip_prefix("user_").unwrap_or(base.as_str());
    format!("async_drop_{suffix}")
}

fn c_async_fut_type_name(name: &str) -> String {
    let base = c_user_fn_name(name);
    let suffix = base.strip_prefix("user_").unwrap_or(base.as_str());
    format!("async_fut_{suffix}_t")
}

fn c_ret_ty(ty: Ty) -> &'static str {
    match ty {
        Ty::I32 | Ty::Never => "uint32_t",
        Ty::Bytes => "bytes_t",
        Ty::BytesView => "bytes_view_t",
        Ty::VecU8 => "vec_u8_t",
        Ty::OptionI32 => "option_i32_t",
        Ty::OptionBytes => "option_bytes_t",
        Ty::ResultI32 => "result_i32_t",
        Ty::ResultBytes => "result_bytes_t",
        Ty::Iface => "iface_t",
        Ty::PtrConstU8 => "const uint8_t*",
        Ty::PtrMutU8 => "uint8_t*",
        Ty::PtrConstVoid => "const void*",
        Ty::PtrMutVoid => "void*",
        Ty::PtrConstI32 => "const uint32_t*",
        Ty::PtrMutI32 => "uint32_t*",
    }
}

fn c_zero(ty: Ty) -> &'static str {
    match ty {
        Ty::I32 | Ty::Never => "UINT32_C(0)",
        Ty::Bytes => "rt_bytes_empty(ctx)",
        Ty::BytesView => "rt_view_empty(ctx)",
        Ty::VecU8 => "(vec_u8_t){0}",
        Ty::OptionI32 => "(option_i32_t){0}",
        Ty::OptionBytes => "(option_bytes_t){0}",
        Ty::ResultI32 => "(result_i32_t){0}",
        Ty::ResultBytes => "(result_bytes_t){0}",
        Ty::Iface => "(iface_t){0}",
        Ty::PtrConstU8
        | Ty::PtrMutU8
        | Ty::PtrConstVoid
        | Ty::PtrMutVoid
        | Ty::PtrConstI32
        | Ty::PtrMutI32 => "NULL",
    }
}

fn c_empty(ty: Ty) -> &'static str {
    c_zero(ty)
}

fn c_param_list_value(params: &[FunctionParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, p) in params.iter().enumerate() {
        out.push_str(", ");
        out.push_str(match p.ty {
            Ty::I32 | Ty::Never => "uint32_t",
            Ty::Bytes => "bytes_t",
            Ty::BytesView => "bytes_view_t",
            Ty::VecU8 => "vec_u8_t",
            Ty::OptionI32 => "option_i32_t",
            Ty::OptionBytes => "option_bytes_t",
            Ty::ResultI32 => "result_i32_t",
            Ty::ResultBytes => "result_bytes_t",
            Ty::Iface => "iface_t",
            Ty::PtrConstU8 => "const uint8_t*",
            Ty::PtrMutU8 => "uint8_t*",
            Ty::PtrConstVoid => "const void*",
            Ty::PtrMutVoid => "void*",
            Ty::PtrConstI32 => "const uint32_t*",
            Ty::PtrMutI32 => "uint32_t*",
        });
        out.push(' ');
        out.push_str(&format!("p{i}"));
    }
    out
}

fn c_param_list_user(params: &[FunctionParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, p) in params.iter().enumerate() {
        out.push_str(", ");
        out.push_str(match p.ty {
            Ty::I32 | Ty::Never => "uint32_t",
            Ty::Bytes => "bytes_t",
            Ty::BytesView => "bytes_view_t",
            Ty::VecU8 => "vec_u8_t",
            Ty::OptionI32 => "option_i32_t",
            Ty::OptionBytes => "option_bytes_t",
            Ty::ResultI32 => "result_i32_t",
            Ty::ResultBytes => "result_bytes_t",
            Ty::Iface => "iface_t",
            Ty::PtrConstU8 => "const uint8_t*",
            Ty::PtrMutU8 => "uint8_t*",
            Ty::PtrConstVoid => "const void*",
            Ty::PtrMutVoid => "void*",
            Ty::PtrConstI32 => "const uint32_t*",
            Ty::PtrMutI32 => "uint32_t*",
        });
        out.push(' ');
        out.push_str(&format!("p{i}"));
    }
    out
}

fn c_extern_param_list(params: &[FunctionParam]) -> String {
    if params.is_empty() {
        return "void".to_string();
    }
    let mut out = String::new();
    for (idx, p) in params.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(c_ret_ty(p.ty));
        out.push(' ');
        out.push_str(&p.name);
    }
    out
}

const RUNTIME_C_PREAMBLE: &str = r#"
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif

#ifndef X07_FREESTANDING
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

#ifndef _WIN32
#include <poll.h>
#include <spawn.h>
#include <sys/mman.h>
#include <sys/wait.h>
#endif

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <io.h>
#include <windows.h>
#ifndef SIGKILL
#define SIGKILL 9
#endif
#endif

#ifndef _WIN32
#ifndef MAP_ANON
#define MAP_ANON MAP_ANONYMOUS
#endif
#endif
#else
#include <stddef.h>
#include <stdint.h>

void* memcpy(void* dst, const void* src, size_t n);
void* memmove(void* dst, const void* src, size_t n);
void* memset(void* dst, int c, size_t n);
int memcmp(const void* a, const void* b, size_t n);
#endif

#ifndef X07_MEM_CAP
#define X07_MEM_CAP (64u * 1024u * 1024u)
#endif

#ifndef X07_FUEL_INIT
#define X07_FUEL_INIT 50000000ULL
#endif

#ifndef X07_ENABLE_FS
#define X07_ENABLE_FS 0
#endif

#ifndef X07_ENABLE_RR
#define X07_ENABLE_RR 0
#endif

#ifndef X07_ENABLE_KV
#define X07_ENABLE_KV 0
#endif

#define X07_ENABLE_STREAMING_FILE_IO (X07_ENABLE_FS || X07_ENABLE_RR || X07_ENABLE_KV)

#ifdef X07_FREESTANDING
#if X07_ENABLE_FS || X07_ENABLE_RR || X07_ENABLE_KV
#error "X07_FREESTANDING requires X07_ENABLE_FS/RR/KV=0"
#endif
#endif

#ifdef X07_DEBUG_BORROW
#ifndef X07_DBG_ALLOC_CAP
#define X07_DBG_ALLOC_CAP 65536u
#endif
#ifndef X07_DBG_BORROW_CAP
#define X07_DBG_BORROW_CAP 65536u
#endif
#endif

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} bytes_t;

typedef struct {
  uint8_t* ptr;
  uint32_t len;
#ifdef X07_DEBUG_BORROW
  uint64_t aid;
  uint64_t bid;
  uint32_t off_bytes;
#endif
} bytes_view_t;

typedef struct {
  uint32_t tag;
  uint32_t payload;
} option_i32_t;

typedef struct {
  uint32_t tag;
  bytes_t payload;
} option_bytes_t;

typedef struct {
  uint32_t tag;
  union {
    uint32_t ok;
    uint32_t err;
  } payload;
} result_i32_t;

typedef struct {
  uint32_t tag;
  union {
    bytes_t ok;
    uint32_t err;
  } payload;
} result_bytes_t;

typedef struct {
  uint32_t data;
  uint32_t vtable;
} iface_t;

static __attribute__((noreturn)) void rt_trap(const char* msg);

#define RT_IFACE_VTABLE_IO_READER UINT32_C(1)
#define RT_IFACE_VTABLE_EXT_IO_READER_MIN UINT32_C(2)
#define RT_IFACE_VTABLE_EXT_IO_READER_MAX UINT32_C(64)

typedef uint32_t (*rt_ext_io_reader_read_fn_t)(uint32_t data, uint8_t* dst, uint32_t cap);
typedef void (*rt_ext_io_reader_drop_fn_t)(uint32_t data);

typedef struct {
  rt_ext_io_reader_read_fn_t read;
  rt_ext_io_reader_drop_fn_t drop;
} rt_ext_io_reader_vtable_t;

static rt_ext_io_reader_vtable_t rt_ext_io_reader_vtables[
  RT_IFACE_VTABLE_EXT_IO_READER_MAX - RT_IFACE_VTABLE_EXT_IO_READER_MIN + 1
];
static uint32_t rt_ext_io_reader_vtables_len = 0;

// External packages register IO reader vtables at runtime to enable `iface` streaming
// through `io.read` / `bufread.*` without adding new builtins.
uint32_t x07_rt_register_io_reader_vtable_v1(
  rt_ext_io_reader_read_fn_t read,
  rt_ext_io_reader_drop_fn_t drop
) {
  if (!read) return 0;

  for (uint32_t i = 0; i < rt_ext_io_reader_vtables_len; i++) {
    rt_ext_io_reader_vtable_t* vt = &rt_ext_io_reader_vtables[i];
    if (vt->read == read && vt->drop == drop) {
      return RT_IFACE_VTABLE_EXT_IO_READER_MIN + i;
    }
  }

  uint32_t cap = (uint32_t)(sizeof(rt_ext_io_reader_vtables) / sizeof(rt_ext_io_reader_vtables[0]));
  if (rt_ext_io_reader_vtables_len >= cap) return 0;

  rt_ext_io_reader_vtable_t* vt = &rt_ext_io_reader_vtables[rt_ext_io_reader_vtables_len];
  vt->read = read;
  vt->drop = drop;

  uint32_t id = RT_IFACE_VTABLE_EXT_IO_READER_MIN + rt_ext_io_reader_vtables_len;
  rt_ext_io_reader_vtables_len += 1;
  return id;
}

static uint32_t rt_ext_io_reader_read_into(uint32_t vtable, uint32_t data, uint8_t* dst, uint32_t cap) {
  if (vtable < RT_IFACE_VTABLE_EXT_IO_READER_MIN || vtable > RT_IFACE_VTABLE_EXT_IO_READER_MAX) {
    rt_trap("ext io reader invalid vtable");
  }
  uint32_t idx = vtable - RT_IFACE_VTABLE_EXT_IO_READER_MIN;
  if (idx >= rt_ext_io_reader_vtables_len) {
    rt_trap("ext io reader unregistered vtable");
  }
  rt_ext_io_reader_vtable_t* vt = &rt_ext_io_reader_vtables[idx];
  if (!vt->read) {
    rt_trap("ext io reader missing read fn");
  }
  uint32_t got = vt->read(data, dst, cap);
  if (got > cap) {
    rt_trap("ext io reader returned too many bytes");
  }
  return got;
}

typedef struct {
  bytes_t key;
  bytes_t val;
} kv_entry_t;

typedef struct {
  bytes_t path;
  uint32_t ticks;
} fs_latency_entry_t;

typedef struct {
  bytes_t key;
  uint32_t latency_ticks;
  bytes_t body_file;
} rr_index_entry_t;

typedef struct {
  bytes_t key;
  uint32_t ticks;
} kv_latency_entry_t;

typedef struct {
  uint8_t* mem;
  uint32_t cap;
  uint32_t free_head;
} heap_t;

// Heap allocator is fixed-capacity and deterministic.
// It uses a singly-linked free list of blocks (sorted by address) with coalescing.
#define RT_HEAP_ALIGN UINT32_C(16)
#define RT_HEAP_MAGIC_FREE UINT32_C(0x45564f46) // "X07F"
#define RT_HEAP_MAGIC_USED UINT32_C(0x45564f55) // "X07U"
#define RT_HEAP_NULL_OFF UINT32_MAX

typedef struct {
  uint32_t size;     // total block size including header, multiple of RT_HEAP_ALIGN
  uint32_t next_off; // free: free-list next (offset from heap base), used: allocation epoch id
  uint32_t magic;    // RT_HEAP_MAGIC_FREE / RT_HEAP_MAGIC_USED
  uint32_t req_size; // requested payload size in bytes
} heap_hdr_t;

static uint32_t rt_heap_align_u32(uint32_t x) {
  return (x + (RT_HEAP_ALIGN - 1u)) & ~(RT_HEAP_ALIGN - 1u);
}

typedef struct {
  uint64_t alloc_calls;
  uint64_t realloc_calls;
  uint64_t free_calls;

  uint64_t bytes_alloc_total;
  uint64_t bytes_freed_total;

  uint64_t live_bytes;
  uint64_t peak_live_bytes;

  uint64_t live_allocs;
  uint64_t peak_live_allocs;

  uint64_t memcpy_bytes;
} mem_stats_t;

typedef struct {
  void* ctx;
  void* (*alloc)(void* ctx, uint32_t size, uint32_t align);
  void* (*realloc)(void* ctx, void* ptr, uint32_t old_size, uint32_t new_size, uint32_t align);
  void (*free)(void* ctx, void* ptr, uint32_t size, uint32_t align);
} allocator_v1_t;

__attribute__((weak)) allocator_v1_t x07_custom_allocator(void) {
  return (allocator_v1_t){0};
}

#ifdef X07_DEBUG_BORROW
typedef struct {
  uint8_t* base_ptr;
  uint32_t size_bytes;
  uint32_t alive;
  uint64_t borrow_id;
} dbg_alloc_rec_t;

typedef struct {
  uint64_t alloc_id;
  uint32_t off_bytes;
  uint32_t len_bytes;
  uint32_t active;
} dbg_borrow_rec_t;
#endif

typedef struct {
  uint64_t tasks_spawned;
  uint64_t spawn_calls;
  uint64_t join_calls;
  uint64_t yield_calls;
  uint64_t sleep_calls;
  uint64_t chan_send_calls;
  uint64_t chan_recv_calls;
  uint64_t ctx_switches;
  uint64_t wake_events;
  uint64_t blocked_waits;
  uint64_t virtual_time_end;
  uint64_t sched_trace_hash;
} sched_stats_t;

typedef struct rt_task_s rt_task_t;
typedef struct rt_timer_ev_s rt_timer_ev_t;
typedef struct rt_chan_bytes_s rt_chan_bytes_t;
typedef struct rt_io_reader_s rt_io_reader_t;
typedef struct rt_bufread_s rt_bufread_t;
typedef struct rt_os_proc_s rt_os_proc_t;

typedef struct {
  uint64_t fuel_init;
  uint64_t fuel;
  heap_t heap;
  allocator_v1_t allocator;
  uint32_t allocator_is_custom;

  // Total heap usage counters (all allocations, including input/runtime metadata).
  uint64_t heap_live_bytes;
  uint64_t heap_peak_live_bytes;
  uint64_t heap_live_allocs;
  uint64_t heap_peak_live_allocs;

  // Epoch id for mem_stats tracking (0 means "not started yet").
  uint32_t mem_epoch_id;
  mem_stats_t mem_stats;
#ifdef X07_DEBUG_BORROW
  dbg_alloc_rec_t* dbg_allocs;
  uint32_t dbg_allocs_len;
  uint32_t dbg_allocs_cap;

  dbg_borrow_rec_t* dbg_borrows;
  uint32_t dbg_borrows_len;
  uint32_t dbg_borrows_cap;

  uint64_t dbg_borrow_violations;
#endif
  uint64_t fs_read_file_calls;
  uint64_t fs_list_dir_calls;
  uint64_t rr_send_calls;
  uint64_t rr_request_calls;
  uint8_t rr_last_request_sha256[32];
  uint64_t kv_get_calls;
  uint64_t kv_set_calls;

  // Phase G2 fixture-backed latency indices (loaded lazily).
  uint32_t fs_latency_loaded;
  uint32_t fs_latency_default_ticks;
  fs_latency_entry_t* fs_latency_entries;
  uint32_t fs_latency_len;
  bytes_t fs_latency_blob;

  uint32_t rr_index_loaded;
  uint32_t rr_index_default_latency_ticks;
  rr_index_entry_t* rr_index_entries;
  uint32_t rr_index_len;
  bytes_t rr_index_blob;

  uint32_t kv_latency_loaded;
  uint32_t kv_latency_default_ticks;
  kv_latency_entry_t* kv_latency_entries;
  uint32_t kv_latency_len;
  bytes_t kv_latency_blob;

  kv_entry_t* kv_items;
  uint32_t kv_len;
  uint32_t kv_cap;

  void** map_u32_items;
  uint32_t map_u32_len;
  uint32_t map_u32_cap;

  // Phase G2 scheduler + concurrency (deterministic, single-thread cooperative).
  uint32_t sched_current_task;
  uint64_t sched_now_ticks;
  uint64_t sched_seq;
  sched_stats_t sched_stats;

  rt_task_t* sched_tasks;
  uint32_t sched_tasks_len;
  uint32_t sched_tasks_cap;

  uint32_t sched_ready_head;
  uint32_t sched_ready_tail;

  rt_timer_ev_t* sched_timers;
  uint32_t sched_timers_len;
  uint32_t sched_timers_cap;

  rt_chan_bytes_t* sched_chans;
  uint32_t sched_chans_len;
  uint32_t sched_chans_cap;

  // Phase G2 streaming I/O (deterministic, fixture-backed).
  rt_io_reader_t* io_readers;
  uint32_t io_readers_len;
  uint32_t io_readers_cap;

  rt_bufread_t* bufreads;
  uint32_t bufreads_len;
  uint32_t bufreads_cap;

  // Standalone OS process table (run-os*, non-deterministic).
  rt_os_proc_t* os_procs;
  uint32_t os_procs_len;
  uint32_t os_procs_cap;
  uint32_t os_procs_live;
  uint32_t os_procs_spawned;
} ctx_t;

// Global ctx pointer for native extension backends that need to allocate bytes via the runtime.
static ctx_t* rt_ext_ctx = NULL;

// Native math backend entrypoints (linked from deps/x07/libx07_math.*).
bytes_t ev_math_f64_add_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_sub_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_mul_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_div_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_neg_v1(bytes_t x);
bytes_t ev_math_f64_abs_v1(bytes_t x);
bytes_t ev_math_f64_min_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_max_v1(bytes_t a, bytes_t b);
bytes_t ev_math_f64_sqrt_v1(bytes_t x);
bytes_t ev_math_f64_sin_v1(bytes_t x);
bytes_t ev_math_f64_cos_v1(bytes_t x);
bytes_t ev_math_f64_tan_v1(bytes_t x);
bytes_t ev_math_f64_exp_v1(bytes_t x);
bytes_t ev_math_f64_ln_v1(bytes_t x);
bytes_t ev_math_f64_pow_v1(bytes_t x, bytes_t y);
bytes_t ev_math_f64_atan2_v1(bytes_t y, bytes_t x);
bytes_t ev_math_f64_floor_v1(bytes_t x);
bytes_t ev_math_f64_ceil_v1(bytes_t x);
bytes_t ev_math_f64_fmt_shortest_v1(bytes_t x);
result_bytes_t ev_math_f64_parse_v1(bytes_t s);
bytes_t ev_math_f64_from_i32_v1(int32_t x);
result_i32_t ev_math_f64_to_i32_trunc_v1(bytes_t x);
bytes_t ev_math_f64_to_bits_u64le_v1(bytes_t x);

// Native time backend entrypoints (linked from deps/x07/libx07_time.*).
uint32_t ev_time_tzdb_is_valid_tzid_v1(bytes_t tzid);
bytes_t ev_time_tzdb_offset_duration_v1(bytes_t tzid, int32_t unix_s_lo, int32_t unix_s_hi);
bytes_t ev_time_tzdb_snapshot_id_v1(void);

// Native ext-fs backend entrypoints (linked from deps/x07/libx07_ext_fs.*).
result_bytes_t x07_ext_fs_read_all_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_write_all_v1(bytes_t path, bytes_t data, bytes_t caps);
result_i32_t x07_ext_fs_mkdirs_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_remove_file_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_remove_dir_all_v1(bytes_t path, bytes_t caps);
result_i32_t x07_ext_fs_rename_v1(bytes_t src, bytes_t dst, bytes_t caps);
result_bytes_t x07_ext_fs_list_dir_sorted_text_v1(bytes_t path, bytes_t caps);
result_bytes_t x07_ext_fs_walk_glob_sorted_text_v1(bytes_t root, bytes_t glob, bytes_t caps);
result_bytes_t x07_ext_fs_stat_v1(bytes_t path, bytes_t caps);

// Native ext-db-sqlite backend entrypoints (linked from deps/x07/libx07_ext_db_sqlite.*).
bytes_t x07_ext_db_sqlite_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_sqlite_query_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_sqlite_exec_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_sqlite_close_v1(bytes_t req, bytes_t caps);

// Native ext-db-pg backend entrypoints (linked from deps/x07/libx07_ext_db_pg.*).
bytes_t x07_ext_db_pg_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_pg_query_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_pg_exec_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_pg_close_v1(bytes_t req, bytes_t caps);

// Native ext-db-mysql backend entrypoints (linked from deps/x07/libx07_ext_db_mysql.*).
bytes_t x07_ext_db_mysql_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_mysql_query_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_mysql_exec_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_mysql_close_v1(bytes_t req, bytes_t caps);

// Native ext-db-redis backend entrypoints (linked from deps/x07/libx07_ext_db_redis.*).
bytes_t x07_ext_db_redis_open_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_redis_cmd_v1(bytes_t req, bytes_t caps);
bytes_t x07_ext_db_redis_close_v1(bytes_t req, bytes_t caps);

// Native ext-regex backend entrypoints (linked from deps/x07/libx07_ext_regex.*).
bytes_t x07_ext_regex_compile_opts_v1(bytes_t pat, int32_t opts_u32);
bytes_t x07_ext_regex_exec_from_v1(bytes_t compiled, bytes_t text, int32_t start_i32);
bytes_t x07_ext_regex_exec_caps_from_v1(bytes_t compiled, bytes_t text, int32_t start_i32);
bytes_t x07_ext_regex_find_all_x7sl_v1(bytes_t compiled, bytes_t text, int32_t max_matches_i32);
bytes_t x07_ext_regex_split_v1(bytes_t compiled, bytes_t text, int32_t max_parts_i32);
bytes_t x07_ext_regex_replace_all_v1(bytes_t compiled, bytes_t text, bytes_t repl, int32_t cap_limit_i32);

#ifdef X07_STANDALONE
static uint32_t rt_os_process_poll_all(ctx_t* ctx, int poll_timeout_ms);
static void rt_os_process_cleanup(ctx_t* ctx);
#else
static uint32_t rt_os_process_poll_all(ctx_t* ctx, int poll_timeout_ms) {
  (void)ctx;
  (void)poll_timeout_ms;
  return UINT32_C(0);
}
static void rt_os_process_cleanup(ctx_t* ctx) {
  (void)ctx;
}
#endif

static __attribute__((noreturn)) void rt_trap(const char* msg) {

#ifndef X07_FREESTANDING
  if (msg) {
    (void)write(STDERR_FILENO, msg, strlen(msg));
    (void)write(STDERR_FILENO, "\n", 1);
  }
#else
  (void)msg;
#endif
  __builtin_trap();
}

static void rt_fuel(ctx_t* ctx, uint64_t amount) {
  if (ctx->fuel < amount) rt_trap("fuel exhausted");
  ctx->fuel -= amount;
}

static uint32_t rt_align_u32(uint32_t x, uint32_t align) {
  return (x + (align - 1u)) & ~(align - 1u);
}

static uint16_t rt_read_u16_le(const uint8_t* p) {
  return (uint16_t)p[0] | ((uint16_t)p[1] << 8);
}

static uint32_t rt_read_u32_le(const uint8_t* p) {
  return (uint32_t)p[0]
       | ((uint32_t)p[1] << 8)
       | ((uint32_t)p[2] << 16)
       | ((uint32_t)p[3] << 24);
}

static void rt_write_u32_le(uint8_t* p, uint32_t x) {
  p[0] = (uint8_t)(x & UINT32_C(0xFF));
  p[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  p[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  p[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
}

static void rt_heap_init(ctx_t* ctx) {
  if (!ctx->heap.mem) rt_trap("heap mem is NULL");
  uint32_t cap = ctx->heap.cap;
  cap &= ~(RT_HEAP_ALIGN - 1u);
  if (cap < (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) rt_trap("heap too small");
  ctx->heap.cap = cap;

  ctx->heap.free_head = 0;
  heap_hdr_t* h = (heap_hdr_t*)(ctx->heap.mem);
  h->size = cap;
  h->next_off = RT_HEAP_NULL_OFF;
  h->magic = RT_HEAP_MAGIC_FREE;
  h->req_size = 0;
}

static heap_hdr_t* rt_heap_hdr_at(ctx_t* ctx, uint32_t off) {
  if (off == RT_HEAP_NULL_OFF) return (heap_hdr_t*)NULL;
  if (off > ctx->heap.cap || ctx->heap.cap - off < (uint32_t)sizeof(heap_hdr_t)) rt_trap("heap corrupt");
  return (heap_hdr_t*)(ctx->heap.mem + off);
}

static uint32_t rt_heap_off_of(ctx_t* ctx, heap_hdr_t* h) {
  uintptr_t base = (uintptr_t)(ctx->heap.mem);
  uintptr_t p = (uintptr_t)h;
  if (p < base) rt_trap("heap ptr underflow");
  uintptr_t off = p - base;
  if (off > (uintptr_t)UINT32_MAX) rt_trap("heap ptr overflow");
  if ((uint32_t)off > ctx->heap.cap) rt_trap("heap ptr oob");
  return (uint32_t)off;
}

static uint32_t rt_heap_is_pow2_u32(uint32_t x) {
  return x != 0 && (x & (x - 1u)) == 0;
}

static void* rt_heap_alloc(ctx_t* ctx, uint32_t size, uint32_t align) {
  if (size == 0) return (void*)ctx->heap.mem;
  if (align == 0) rt_trap("alloc align=0");
  if (!rt_heap_is_pow2_u32(align)) rt_trap("alloc align not pow2");
  if (align > RT_HEAP_ALIGN) rt_trap("alloc align too large");

  uint32_t payload = rt_heap_align_u32(size);
  uint32_t need = (uint32_t)sizeof(heap_hdr_t) + payload;
  need = rt_heap_align_u32(need);
  if (need < (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) {
    need = (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN;
  }
  if (need > ctx->heap.cap) return NULL;

  uint32_t prev_off = RT_HEAP_NULL_OFF;
  uint32_t off = ctx->heap.free_head;
  while (off != RT_HEAP_NULL_OFF) {
    heap_hdr_t* h = rt_heap_hdr_at(ctx, off);
    if (h->magic != RT_HEAP_MAGIC_FREE) rt_trap("heap free list corrupt");
    if (h->size >= need) {
      uint32_t next_off = h->next_off;

      // Remove from free list.
      if (prev_off == RT_HEAP_NULL_OFF) {
        ctx->heap.free_head = next_off;
      } else {
        heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
        prev->next_off = next_off;
      }

      uint32_t remaining = h->size - need;
      if (remaining >= (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) {
        uint32_t rem_off = off + need;
        heap_hdr_t* rem = rt_heap_hdr_at(ctx, rem_off);
        rem->size = remaining;
        rem->next_off = next_off;
        rem->magic = RT_HEAP_MAGIC_FREE;
        rem->req_size = 0;

        if (prev_off == RT_HEAP_NULL_OFF) {
          ctx->heap.free_head = rem_off;
        } else {
          heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
          prev->next_off = rem_off;
        }
        h->size = need;
      } else {
        // Don't split; keep whole block.
        need = h->size;
      }

      h->next_off = ctx->mem_epoch_id;
      h->magic = RT_HEAP_MAGIC_USED;
      h->req_size = size;

      void* ptr = (void*)(ctx->heap.mem + off + (uint32_t)sizeof(heap_hdr_t));
      memset(ptr, 0, payload);
      return ptr;
    }
    prev_off = off;
    off = h->next_off;
  }
  return NULL;
}

static void rt_heap_free(ctx_t* ctx, void* ptr) {
  if (!ptr) return;
  if ((uint8_t*)ptr == ctx->heap.mem) return;
  uint8_t* p = (uint8_t*)ptr;
  if (p < ctx->heap.mem + (uint32_t)sizeof(heap_hdr_t)) rt_trap("free oob");
  heap_hdr_t* h = (heap_hdr_t*)(p - (uint32_t)sizeof(heap_hdr_t));
  uint32_t off = rt_heap_off_of(ctx, h);
  if (h->magic != RT_HEAP_MAGIC_USED) rt_trap("double free or corrupt heap");
  uint32_t size = h->size;
  if (size < (uint32_t)sizeof(heap_hdr_t) + RT_HEAP_ALIGN) rt_trap("free corrupt size");
  if ((size & (RT_HEAP_ALIGN - 1u)) != 0) rt_trap("free corrupt size");
  if (off > ctx->heap.cap || ctx->heap.cap - off < size) rt_trap("free oob");

  // Insert into free list (sorted by address).
  uint32_t prev_off = RT_HEAP_NULL_OFF;
  uint32_t cur_off = ctx->heap.free_head;
  while (cur_off != RT_HEAP_NULL_OFF && cur_off < off) {
    heap_hdr_t* cur = rt_heap_hdr_at(ctx, cur_off);
    prev_off = cur_off;
    cur_off = cur->next_off;
  }

  h->magic = RT_HEAP_MAGIC_FREE;
  h->next_off = cur_off;
  h->req_size = 0;

  if (prev_off == RT_HEAP_NULL_OFF) {
    ctx->heap.free_head = off;
  } else {
    heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
    prev->next_off = off;
  }

  // Coalesce with next.
  if (cur_off != RT_HEAP_NULL_OFF) {
    heap_hdr_t* next = rt_heap_hdr_at(ctx, cur_off);
    if (off + h->size == cur_off) {
      h->size += next->size;
      h->next_off = next->next_off;
    }
  }

  // Coalesce with prev.
  if (prev_off != RT_HEAP_NULL_OFF) {
    heap_hdr_t* prev = rt_heap_hdr_at(ctx, prev_off);
    if (prev_off + prev->size == off) {
      prev->size += h->size;
      prev->next_off = h->next_off;
    }
  }
}

static void rt_mem_epoch_reset(ctx_t* ctx) {
  ctx->mem_epoch_id += 1;
  if (ctx->mem_epoch_id == 0) ctx->mem_epoch_id = 1;

  ctx->mem_stats.alloc_calls = 0;
  ctx->mem_stats.realloc_calls = 0;
  ctx->mem_stats.free_calls = 0;
  ctx->mem_stats.bytes_alloc_total = 0;
  ctx->mem_stats.bytes_freed_total = 0;
  ctx->mem_stats.live_bytes = 0;
  ctx->mem_stats.peak_live_bytes = 0;
  ctx->mem_stats.live_allocs = 0;
  ctx->mem_stats.peak_live_allocs = 0;
  ctx->mem_stats.memcpy_bytes = 0;
}

static void rt_mem_on_alloc(ctx_t* ctx, uint32_t size, uint32_t is_realloc) {
  ctx->heap_live_bytes += (uint64_t)size;
  ctx->heap_live_allocs += 1;
  if (ctx->heap_live_bytes > ctx->heap_peak_live_bytes) {
    ctx->heap_peak_live_bytes = ctx->heap_live_bytes;
  }
  if (ctx->heap_live_allocs > ctx->heap_peak_live_allocs) {
    ctx->heap_peak_live_allocs = ctx->heap_live_allocs;
  }

  if (ctx->mem_epoch_id == 0) return;

  if (is_realloc) {
    ctx->mem_stats.realloc_calls += 1;
  } else {
    ctx->mem_stats.alloc_calls += 1;
  }
  ctx->mem_stats.bytes_alloc_total += (uint64_t)size;
  ctx->mem_stats.live_bytes += (uint64_t)size;
  ctx->mem_stats.live_allocs += 1;
  if (ctx->mem_stats.live_bytes > ctx->mem_stats.peak_live_bytes) {
    ctx->mem_stats.peak_live_bytes = ctx->mem_stats.live_bytes;
  }
  if (ctx->mem_stats.live_allocs > ctx->mem_stats.peak_live_allocs) {
    ctx->mem_stats.peak_live_allocs = ctx->mem_stats.live_allocs;
  }
}

static void rt_mem_on_free(ctx_t* ctx, uint32_t size, uint32_t is_epoch, uint32_t strict) {
  if (ctx->heap_live_bytes < (uint64_t)size) rt_trap("mem free underflow");
  if (ctx->heap_live_allocs == 0) rt_trap("mem free underflow");
  ctx->heap_live_bytes -= (uint64_t)size;
  ctx->heap_live_allocs -= 1;

  if (ctx->mem_epoch_id == 0) return;
  if (!is_epoch) return;

  ctx->mem_stats.free_calls += 1;
  ctx->mem_stats.bytes_freed_total += (uint64_t)size;
  if (ctx->mem_stats.live_bytes < (uint64_t)size || ctx->mem_stats.live_allocs == 0) {
    if (strict) rt_trap("mem epoch underflow");
    return;
  }
  ctx->mem_stats.live_bytes -= (uint64_t)size;
  ctx->mem_stats.live_allocs -= 1;
}

static void rt_mem_on_memcpy(ctx_t* ctx, uint32_t size) {
  if (ctx->mem_epoch_id == 0) return;
  ctx->mem_stats.memcpy_bytes += (uint64_t)size;
}

static void* rt_alloc_raw(ctx_t* ctx, uint32_t size, uint32_t align) {
  void* ptr = rt_heap_alloc(ctx, size, align);
  if (!ptr && size) rt_trap("out of memory");
  return ptr;
}

static void* rt_default_alloc(void* alloc_ctx, uint32_t size, uint32_t align) {
  return rt_alloc_raw((ctx_t*)alloc_ctx, size, align);
}

static void* rt_default_realloc(
    void* alloc_ctx,
    void* ptr,
    uint32_t old_size,
    uint32_t new_size,
    uint32_t align
) {
  (void)ptr;
  (void)old_size;
  return rt_alloc_raw((ctx_t*)alloc_ctx, new_size, align);
}

static void rt_default_free(void* alloc_ctx, void* ptr, uint32_t size, uint32_t align) {
  (void)size;
  (void)align;
  rt_heap_free((ctx_t*)alloc_ctx, ptr);
}

static void rt_allocator_init(ctx_t* ctx) {
  ctx->allocator.ctx = ctx;
  ctx->allocator.alloc = rt_default_alloc;
  ctx->allocator.realloc = rt_default_realloc;
  ctx->allocator.free = rt_default_free;
  ctx->allocator_is_custom = 0;

  allocator_v1_t custom = x07_custom_allocator();
  if (custom.alloc || custom.realloc || custom.free) {
    if (!custom.alloc || !custom.realloc || !custom.free) rt_trap("custom allocator missing hooks");
    ctx->allocator = custom;
    ctx->allocator_is_custom = 1;
  }
}

static void* rt_alloc(ctx_t* ctx, uint32_t size, uint32_t align) {
  if (size == 0) return (void*)ctx->heap.mem;
  if (!ctx->allocator.alloc) rt_trap("allocator.alloc missing");
  void* ptr = ctx->allocator.alloc(ctx->allocator.ctx, size, align);
  if (!ptr && size) rt_trap("allocator.alloc failed");
  rt_mem_on_alloc(ctx, size, 0);
  return ptr;
}

static void* rt_alloc_realloc(
    ctx_t* ctx,
    void* old_ptr,
    uint32_t old_size,
    uint32_t new_size,
    uint32_t align
) {
  if (new_size == 0) return (void*)ctx->heap.mem;
  if (!ctx->allocator.realloc) rt_trap("allocator.realloc missing");
  void* new_ptr =
      ctx->allocator.realloc(ctx->allocator.ctx, old_ptr, old_size, new_size, align);
  if (!new_ptr && new_size) rt_trap("allocator.realloc failed");
  rt_mem_on_alloc(ctx, new_size, old_size != 0);
  return new_ptr;
}

static void rt_free(ctx_t* ctx, void* ptr, uint32_t size, uint32_t align) {
  if (!ptr) return;
  if (size == 0) return;
  uint32_t is_epoch = 1;
  uint32_t strict = ctx->allocator_is_custom ? 0 : 1;
  if (!ctx->allocator_is_custom && (uint8_t*)ptr != ctx->heap.mem) {
    uint8_t* p = (uint8_t*)ptr;
    if (p < ctx->heap.mem + (uint32_t)sizeof(heap_hdr_t)) rt_trap("free oob");
    heap_hdr_t* h = (heap_hdr_t*)(p - (uint32_t)sizeof(heap_hdr_t));
    if (h->magic != RT_HEAP_MAGIC_USED) rt_trap("free corrupt heap");
    is_epoch = (h->next_off == ctx->mem_epoch_id);
  }
  if (!ctx->allocator.free) rt_trap("allocator.free missing");
  ctx->allocator.free(ctx->allocator.ctx, ptr, size, align);
  rt_mem_on_free(ctx, size, is_epoch, strict);
}

static void rt_mem_free_all(ctx_t* ctx) {
  // Bulk reset used at process end.
  ctx->mem_stats.free_calls += 1;
  ctx->mem_stats.bytes_freed_total += ctx->mem_stats.live_bytes;
  ctx->mem_stats.live_bytes = 0;
  ctx->mem_stats.live_allocs = 0;

  ctx->heap_live_bytes = 0;
  ctx->heap_live_allocs = 0;
  rt_heap_init(ctx);
}

#ifdef X07_DEBUG_BORROW
static void rt_dbg_init(ctx_t* ctx) {
  ctx->dbg_allocs_len = 0;
  ctx->dbg_allocs_cap = X07_DBG_ALLOC_CAP;
  ctx->dbg_allocs =
      (dbg_alloc_rec_t*)rt_alloc(
          ctx,
          ctx->dbg_allocs_cap * (uint32_t)sizeof(dbg_alloc_rec_t),
          (uint32_t)_Alignof(dbg_alloc_rec_t)
      );

  ctx->dbg_borrows_len = 0;
  ctx->dbg_borrows_cap = X07_DBG_BORROW_CAP;
  ctx->dbg_borrows =
      (dbg_borrow_rec_t*)rt_alloc(
          ctx,
          ctx->dbg_borrows_cap * (uint32_t)sizeof(dbg_borrow_rec_t),
          (uint32_t)_Alignof(dbg_borrow_rec_t)
      );

  ctx->dbg_borrow_violations = 0;
}

static uint64_t rt_dbg_borrow_acquire(
    ctx_t* ctx,
    uint64_t alloc_id,
    uint32_t off_bytes,
    uint32_t len_bytes
);

static uint64_t rt_dbg_alloc_register(ctx_t* ctx, uint8_t* base_ptr, uint32_t size_bytes) {
  if (size_bytes == 0) return 0;
  if (!ctx->dbg_allocs || ctx->dbg_allocs_cap == 0) return 0;
  if (ctx->dbg_allocs_len >= ctx->dbg_allocs_cap) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint32_t idx = ctx->dbg_allocs_len++;
  dbg_alloc_rec_t* rec = &ctx->dbg_allocs[idx];
  rec->base_ptr = base_ptr;
  rec->size_bytes = size_bytes;
  rec->alive = 1;
  rec->borrow_id = rt_dbg_borrow_acquire(ctx, (uint64_t)idx + 1, 0, size_bytes);
  return (uint64_t)idx + 1;
}

static void rt_dbg_alloc_kill(ctx_t* ctx, uint64_t alloc_id) {
  if (alloc_id == 0) return;
  uint64_t idx = alloc_id - 1;
  if (!ctx->dbg_allocs || idx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return;
  }
  ctx->dbg_allocs[idx].alive = 0;
}

static uint64_t rt_dbg_alloc_find(
    ctx_t* ctx,
    uint8_t* ptr,
    uint32_t len_bytes,
    uint32_t* out_off_bytes
) {
  if (out_off_bytes) *out_off_bytes = 0;
  if (len_bytes == 0) return 0;
  if (!ctx->dbg_allocs) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uintptr_t p = (uintptr_t)ptr;
  // Search newest-to-oldest so re-used pointers resolve to the latest allocation record.
  for (uint32_t i = ctx->dbg_allocs_len; i > 0; i--) {
    uint32_t idx = i - 1;
    dbg_alloc_rec_t* rec = &ctx->dbg_allocs[idx];
    uintptr_t base = (uintptr_t)rec->base_ptr;
    uintptr_t end = base + (uintptr_t)rec->size_bytes;
    if (end < base) continue;
    if (p >= base && p < end) {
      uint32_t off = (uint32_t)(p - base);
      if (off > rec->size_bytes || rec->size_bytes - off < len_bytes) {
        ctx->dbg_borrow_violations += 1;
        return 0;
      }
      if (out_off_bytes) *out_off_bytes = off;
      return (uint64_t)idx + 1;
    }
  }
  ctx->dbg_borrow_violations += 1;
  return 0;
}

static uint64_t rt_dbg_alloc_borrow_id(ctx_t* ctx, uint64_t alloc_id) {
  if (alloc_id == 0) return 0;
  uint64_t idx = alloc_id - 1;
  if (!ctx->dbg_allocs || idx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t bid = ctx->dbg_allocs[idx].borrow_id;
  if (bid == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  return bid;
}

static uint64_t rt_dbg_borrow_acquire(
    ctx_t* ctx,
    uint64_t alloc_id,
    uint32_t off_bytes,
    uint32_t len_bytes
) {
  if (len_bytes == 0) return 0;
  if (alloc_id == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t aidx = alloc_id - 1;
  if (!ctx->dbg_allocs || aidx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  dbg_alloc_rec_t* a = &ctx->dbg_allocs[aidx];
  if (!a->alive) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (off_bytes > a->size_bytes || a->size_bytes - off_bytes < len_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (!ctx->dbg_borrows || ctx->dbg_borrows_cap == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (ctx->dbg_borrows_len >= ctx->dbg_borrows_cap) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint32_t idx = ctx->dbg_borrows_len++;
  dbg_borrow_rec_t* b = &ctx->dbg_borrows[idx];
  b->alloc_id = alloc_id;
  b->off_bytes = off_bytes;
  b->len_bytes = len_bytes;
  b->active = 1;
  return (uint64_t)idx + 1;
}

static void rt_dbg_borrow_release(ctx_t* ctx, uint64_t borrow_id) {
  if (borrow_id == 0) return;
  uint64_t idx = borrow_id - 1;
  if (!ctx->dbg_borrows || idx >= (uint64_t)ctx->dbg_borrows_len) {
    ctx->dbg_borrow_violations += 1;
    return;
  }
  ctx->dbg_borrows[idx].active = 0;
}

static uint32_t rt_dbg_borrow_check(
    ctx_t* ctx,
    uint64_t borrow_id,
    uint32_t off_bytes,
    uint32_t len_bytes
) {
  if (len_bytes == 0) return 1;
  if (borrow_id == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t idx = borrow_id - 1;
  if (!ctx->dbg_borrows || idx >= (uint64_t)ctx->dbg_borrows_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  dbg_borrow_rec_t* b = &ctx->dbg_borrows[idx];
  if (!b->active) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t alloc_id = b->alloc_id;
  if (alloc_id == 0) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint64_t aidx = alloc_id - 1;
  if (!ctx->dbg_allocs || aidx >= (uint64_t)ctx->dbg_allocs_len) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  dbg_alloc_rec_t* a = &ctx->dbg_allocs[aidx];
  if (!a->alive) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (off_bytes < b->off_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  uint32_t rel = off_bytes - b->off_bytes;
  if (rel > b->len_bytes || b->len_bytes - rel < len_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  if (off_bytes > a->size_bytes || a->size_bytes - off_bytes < len_bytes) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  return 1;
}
#endif

static inline bytes_t rt_bytes_empty(ctx_t* ctx) {
  bytes_t out;
  out.ptr = ctx->heap.mem;
  out.len = UINT32_C(0);
  return out;
}

static inline bytes_view_t rt_view_empty(ctx_t* ctx) {
  bytes_view_t out;
  out.ptr = ctx->heap.mem;
  out.len = UINT32_C(0);
#ifdef X07_DEBUG_BORROW
  out.aid = 0;
  out.bid = 0;
  out.off_bytes = 0;
#endif
  return out;
}

static bytes_t rt_bytes_alloc(ctx_t* ctx, uint32_t len) {
  if (len == 0) return rt_bytes_empty(ctx);
  bytes_t out;
  out.len = len;
  out.ptr = (uint8_t*)rt_alloc(ctx, len, 1);
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_alloc_register(ctx, out.ptr, len);
#endif
  return out;
}

// Native extension hook: allocate bytes using the currently-running ctx allocator.
bytes_t ev_bytes_alloc(uint32_t len) {
  if (!rt_ext_ctx) rt_trap(NULL);
  return rt_bytes_alloc(rt_ext_ctx, len);
}

// Native extension hook: trap without returning.
__attribute__((noreturn)) void ev_trap(int32_t code) {
  (void)code;
  rt_trap(NULL);
}

static bytes_t rt_bytes_from_literal(ctx_t* ctx, const uint8_t* ptr, uint32_t len) {
  bytes_t out = rt_bytes_alloc(ctx, len);
  if (len != 0) {
    memcpy(out.ptr, ptr, len);
    rt_mem_on_memcpy(ctx, len);
  }
  return out;
}

static bytes_t rt_bytes_clone(ctx_t* ctx, bytes_t src) {
  bytes_t out = rt_bytes_alloc(ctx, src.len);
  if (src.len != 0) {
    memcpy(out.ptr, src.ptr, src.len);
    rt_mem_on_memcpy(ctx, src.len);
  }
  return out;
}

static void rt_bytes_drop(ctx_t* ctx, bytes_t* b) {
  if (!b) return;
  if (b->len == 0) {
    b->ptr = ctx->heap.mem;
    return;
  }
#ifdef X07_DEBUG_BORROW
  uint64_t aid = rt_dbg_alloc_find(ctx, b->ptr, b->len, NULL);
  rt_dbg_alloc_kill(ctx, aid);
#endif
  uint32_t size = b->len;
  // Heap allocator stores the requested size in the allocation header; use it for exact accounting.
  if (ctx->allocator.free == rt_default_free) {
    heap_hdr_t* h = (heap_hdr_t*)(b->ptr - (uint32_t)sizeof(heap_hdr_t));
    if (h->magic != RT_HEAP_MAGIC_USED) rt_trap("bytes.drop corrupt header");
    if (h->req_size == 0) rt_trap("bytes.drop corrupt header");
    size = h->req_size;
  }
  rt_free(ctx, b->ptr, size, 1);
  b->ptr = ctx->heap.mem;
  b->len = UINT32_C(0);
}

static bytes_t rt_view_to_bytes(ctx_t* ctx, bytes_view_t v) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, v.bid, v.off_bytes, v.len);
#endif
  bytes_t out = rt_bytes_alloc(ctx, v.len);
  if (v.len != 0) {
    memcpy(out.ptr, v.ptr, v.len);
    rt_mem_on_memcpy(ctx, v.len);
  }
  return out;
}

#ifdef X07_DEBUG_BORROW
static uint32_t rt_dbg_bytes_check(ctx_t* ctx, bytes_t b) {
  if (b.len == 0) return 1;
  uint64_t aid = rt_dbg_alloc_find(ctx, b.ptr, b.len, NULL);
  if (aid == 0) return 0;
  dbg_alloc_rec_t* a = &ctx->dbg_allocs[aid - 1];
  if (!a->alive) {
    ctx->dbg_borrow_violations += 1;
    return 0;
  }
  return 1;
}
#endif

static uint32_t rt_bytes_get_u8(ctx_t* ctx, bytes_t b, uint32_t idx) {
  if (idx >= b.len) rt_trap("bytes.get_u8 oob");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, b)) return 0;
#else
  (void)ctx;
#endif
  return (uint32_t)b.ptr[idx];
}

static bytes_t rt_bytes_set_u8(ctx_t* ctx, bytes_t b, uint32_t idx, uint32_t v) {
  if (idx >= b.len) rt_trap("bytes.set_u8 oob");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, b)) return b;
#else
  (void)ctx;
#endif
  b.ptr[idx] = (uint8_t)(v & UINT32_C(0xFF));
  return b;
}

static bytes_t rt_bytes_copy(ctx_t* ctx, bytes_t src, bytes_t dst) {
  if (dst.len < src.len) rt_trap("bytes.copy dst too small");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, src)) return dst;
  if (!rt_dbg_bytes_check(ctx, dst)) return dst;
#endif
  if (src.len != 0) {
    memcpy(dst.ptr, src.ptr, src.len);
    rt_mem_on_memcpy(ctx, src.len);
  }
  return dst;
}

static bytes_t rt_bytes_slice(ctx_t* ctx, bytes_t b, uint32_t start, uint32_t len) {
  if (start > b.len) rt_trap("bytes.slice oob");
  if (len > b.len - start) rt_trap("bytes.slice oob");

#ifdef X07_DEBUG_BORROW
  // Bulk ops bypass per-byte checks; validate once up front.
  uint32_t ok = rt_dbg_bytes_check(ctx, b);
  if (!ok && b.len != 0) {
    return rt_bytes_alloc(ctx, len);
  }
#endif

  bytes_t out = rt_bytes_alloc(ctx, len);
  if (len != 0) {
    memcpy(out.ptr, b.ptr + start, len);
    rt_mem_on_memcpy(ctx, len);
  }
  return out;
}

static bytes_t rt_bytes_concat(ctx_t* ctx, bytes_t a, bytes_t b) {
  uint32_t la = a.len;
  uint32_t lb = b.len;
  if (UINT32_MAX - la < lb) rt_trap("bytes.concat overflow");
#ifdef X07_DEBUG_BORROW
  uint32_t ok_a = rt_dbg_bytes_check(ctx, a);
  uint32_t ok_b = rt_dbg_bytes_check(ctx, b);
  if ((!ok_a && la != 0) || (!ok_b && lb != 0)) {
    return rt_bytes_alloc(ctx, la + lb);
  }
#endif

  bytes_t out = rt_bytes_alloc(ctx, la + lb);
  if (la != 0) {
    memcpy(out.ptr, a.ptr, la);
    rt_mem_on_memcpy(ctx, la);
  }
  if (lb != 0) {
    memcpy(out.ptr + la, b.ptr, lb);
    rt_mem_on_memcpy(ctx, lb);
  }
  return out;
}

static uint32_t rt_bytes_eq(ctx_t* ctx, bytes_t a, bytes_t b) {
  if (a.len != b.len) return UINT32_C(0);
  if (a.len == 0) return UINT32_C(1);
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, a)) return UINT32_C(0);
  if (!rt_dbg_bytes_check(ctx, b)) return UINT32_C(0);
#else
  (void)ctx;
#endif
  return (memcmp(a.ptr, b.ptr, a.len) == 0) ? UINT32_C(1) : UINT32_C(0);
}

static uint32_t rt_bytes_cmp_range(
    ctx_t* ctx,
    bytes_t a,
    uint32_t a_off,
    uint32_t a_len,
    bytes_t b,
    uint32_t b_off,
    uint32_t b_len
) {
  if (a_off > a.len || a.len - a_off < a_len) rt_trap("bytes.cmp_range oob");
  if (b_off > b.len || b.len - b_off < b_len) rt_trap("bytes.cmp_range oob");

#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_bytes_check(ctx, a)) return UINT32_C(0);
  if (!rt_dbg_bytes_check(ctx, b)) return UINT32_C(0);
#else
  (void)ctx;
#endif

  uint32_t m = (a_len < b_len) ? a_len : b_len;
  if (m) {
    int cmp = memcmp(a.ptr + a_off, b.ptr + b_off, m);
    if (cmp < 0) return UINT32_MAX;
    if (cmp > 0) return UINT32_C(1);
  }
  if (a_len < b_len) return UINT32_MAX;
  if (a_len > b_len) return UINT32_C(1);
  return UINT32_C(0);
}

static bytes_view_t rt_bytes_view(ctx_t* ctx, bytes_t b) {
  bytes_view_t out;
  out.len = b.len;
#ifdef X07_DEBUG_BORROW
  if (b.len == 0) return rt_view_empty(ctx);
  uint32_t off = 0;
  uint64_t aid = rt_dbg_alloc_find(ctx, b.ptr, b.len, &off);
  out.ptr = b.ptr;
  out.aid = aid;
  out.off_bytes = off;
  out.bid = rt_dbg_alloc_borrow_id(ctx, aid);
#else
  out.ptr = (b.len == 0) ? ctx->heap.mem : b.ptr;
#endif
  return out;
}

static bytes_view_t rt_bytes_subview(ctx_t* ctx, bytes_t b, uint32_t start, uint32_t len) {
  if (start > b.len) rt_trap("bytes.subview oob");
  if (len > b.len - start) rt_trap("bytes.subview oob");
  bytes_view_t out;
  out.len = len;
#ifdef X07_DEBUG_BORROW
  if (len == 0) {
    return rt_view_empty(ctx);
  }
  uint32_t base_off = 0;
  uint64_t aid = rt_dbg_alloc_find(ctx, b.ptr, b.len, &base_off);
  uint32_t off = base_off + start;
  out.ptr = b.ptr + start;
  out.aid = aid;
  out.off_bytes = off;
  out.bid = rt_dbg_alloc_borrow_id(ctx, aid);
#else
  out.ptr = (len == 0) ? ctx->heap.mem : (b.ptr + start);
#endif
  return out;
}

static uint32_t rt_view_get_u8(ctx_t* ctx, bytes_view_t v, uint32_t idx) {
  if (idx >= v.len) rt_trap("view.get_u8 oob");
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_borrow_check(ctx, v.bid, v.off_bytes + idx, 1)) return 0;
#else
  (void)ctx;
#endif
  return (uint32_t)v.ptr[idx];
}

static bytes_view_t rt_view_slice(ctx_t* ctx, bytes_view_t v, uint32_t start, uint32_t len) {
  if (start > v.len) rt_trap("view.slice oob");
  if (len > v.len - start) rt_trap("view.slice oob");
  bytes_view_t out;
  out.ptr = v.ptr + start;
  out.len = len;
#ifdef X07_DEBUG_BORROW
  out.aid = v.aid;
  out.bid = v.bid;
  out.off_bytes = v.off_bytes + start;
#else
  (void)ctx;
#endif
  return out;
}

static uint32_t rt_view_eq(ctx_t* ctx, bytes_view_t a, bytes_view_t b) {
  if (a.len != b.len) return UINT32_C(0);
  if (a.len == 0) return UINT32_C(1);
#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_borrow_check(ctx, a.bid, a.off_bytes, a.len)) return UINT32_C(0);
  if (!rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b.len)) return UINT32_C(0);
#else
  (void)ctx;
#endif
  return (memcmp(a.ptr, b.ptr, a.len) == 0) ? UINT32_C(1) : UINT32_C(0);
}

static uint32_t rt_view_cmp_range(
    ctx_t* ctx,
    bytes_view_t a,
    uint32_t a_off,
    uint32_t a_len,
    bytes_view_t b,
    uint32_t b_off,
    uint32_t b_len
) {
  if (a_off > a.len || a.len - a_off < a_len) rt_trap("view.cmp_range oob");
  if (b_off > b.len || b.len - b_off < b_len) rt_trap("view.cmp_range oob");

#ifdef X07_DEBUG_BORROW
  if (!rt_dbg_borrow_check(ctx, a.bid, a.off_bytes + a_off, a_len)) return UINT32_C(0);
  if (!rt_dbg_borrow_check(ctx, b.bid, b.off_bytes + b_off, b_len)) return UINT32_C(0);
#else
  (void)ctx;
#endif

  uint32_t m = (a_len < b_len) ? a_len : b_len;
  if (m) {
    int cmp = memcmp(a.ptr + a_off, b.ptr + b_off, m);
    if (cmp < 0) return UINT32_MAX;
    if (cmp > 0) return UINT32_C(1);
  }
  if (a_len < b_len) return UINT32_MAX;
  if (a_len > b_len) return UINT32_C(1);
  return UINT32_C(0);
}

// Phase G2: deterministic single-thread cooperative scheduler + channels.

#define RT_WAIT_NONE UINT32_C(0)
#define RT_WAIT_JOIN UINT32_C(1)
#define RT_WAIT_SLEEP UINT32_C(2)
#define RT_WAIT_CHAN_SEND UINT32_C(3)
#define RT_WAIT_CHAN_RECV UINT32_C(4)
#define RT_WAIT_OS_PROC_JOIN UINT32_C(5)
#define RT_WAIT_OS_PROC_EXIT UINT32_C(6)

#define RT_TRACE_SWITCH UINT64_C(1)
#define RT_TRACE_BLOCK UINT64_C(2)
#define RT_TRACE_WAKE UINT64_C(3)
#define RT_TRACE_COMPLETE UINT64_C(4)

struct rt_task_s {
  uint32_t alive;
  uint32_t done;
  uint32_t canceled;

  uint32_t in_ready;
  uint32_t ready_next;

  uint32_t wait_kind;
  uint32_t wait_id;
  uint32_t wait_next;

  uint32_t join_wait_head;
  uint32_t join_wait_tail;

  uint32_t (*poll)(ctx_t* ctx, void* fut, bytes_t* out);
  void (*drop)(ctx_t* ctx, void* fut);
  void* fut;
  bytes_t out;
  uint32_t out_taken;
};

struct rt_timer_ev_s {
  uint64_t wake_time;
  uint64_t seq;
  uint32_t task_id;
};

struct rt_chan_bytes_s {
  uint32_t alive;
  uint32_t closed;
  uint32_t cap;

  bytes_t* buf;
  uint32_t head;
  uint32_t tail;
  uint32_t len;

  uint32_t send_wait_head;
  uint32_t send_wait_tail;
  uint32_t recv_wait_head;
  uint32_t recv_wait_tail;
};

// Phase G2: deterministic streaming I/O reader handles + BufRead-like buffering.

#define RT_IO_READER_KIND_FILE UINT32_C(1)
#define RT_IO_READER_KIND_BYTES UINT32_C(2)

struct rt_io_reader_s {
  uint32_t alive;
  uint32_t kind;
  uint32_t eof;
  uint32_t pending_ticks;

#if X07_ENABLE_STREAMING_FILE_IO
  FILE* f;
#endif

  bytes_t bytes;
  uint32_t pos;
};

struct rt_bufread_s {
  uint32_t alive;
  iface_t reader;
  uint32_t eof;

  bytes_t buf;
  uint32_t start;
  uint32_t end;
};

static void rt_sched_trace_init(ctx_t* ctx) {
  if (ctx->sched_stats.sched_trace_hash == 0) {
    ctx->sched_stats.sched_trace_hash = UINT64_C(1469598103934665603);
  }
}

static void rt_sched_trace_u64(ctx_t* ctx, uint64_t x) {
  rt_sched_trace_init(ctx);
  ctx->sched_stats.sched_trace_hash ^= x;
  ctx->sched_stats.sched_trace_hash *= UINT64_C(1099511628211);
}

static void rt_sched_trace_event(ctx_t* ctx, uint64_t tag, uint64_t a, uint64_t b) {
  rt_sched_trace_u64(ctx, tag);
  rt_sched_trace_u64(ctx, a);
  rt_sched_trace_u64(ctx, b);
}

static rt_task_t* rt_task_ptr(ctx_t* ctx, uint32_t task_id) {
  if (task_id == 0 || task_id > ctx->sched_tasks_len) rt_trap("task invalid handle");
  rt_task_t* t = &ctx->sched_tasks[task_id - 1];
  if (!t->alive) rt_trap("task invalid handle");
  return t;
}

static rt_chan_bytes_t* rt_chan_bytes_ptr(ctx_t* ctx, uint32_t chan_id) {
  if (chan_id == 0 || chan_id > ctx->sched_chans_len) rt_trap("chan invalid handle");
  rt_chan_bytes_t* c = &ctx->sched_chans[chan_id - 1];
  if (!c->alive) rt_trap("chan invalid handle");
  return c;
}

static rt_io_reader_t* rt_io_reader_ptr(ctx_t* ctx, uint32_t reader_id) {
  if (reader_id == 0 || reader_id > ctx->io_readers_len) rt_trap("io.reader invalid handle");
  rt_io_reader_t* r = &ctx->io_readers[reader_id - 1];
  if (!r->alive) rt_trap("io.reader invalid handle");
  return r;
}

static rt_bufread_t* rt_bufread_ptr(ctx_t* ctx, uint32_t br_id) {
  if (br_id == 0 || br_id > ctx->bufreads_len) rt_trap("bufread invalid handle");
  rt_bufread_t* br = &ctx->bufreads[br_id - 1];
  if (!br->alive) rt_trap("bufread invalid handle");
  return br;
}

static void rt_sched_tasks_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_tasks_cap) return;
  rt_task_t* old_items = ctx->sched_tasks;
  uint32_t old_cap = ctx->sched_tasks_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_task_t);
  uint32_t new_cap = ctx->sched_tasks_cap ? ctx->sched_tasks_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_task_t* items = (rt_task_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_task_t),
    (uint32_t)_Alignof(rt_task_t)
  );
  if (old_items && ctx->sched_tasks_len) {
    uint32_t bytes = ctx->sched_tasks_len * (uint32_t)sizeof(rt_task_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_task_t));
  }
  ctx->sched_tasks = items;
  ctx->sched_tasks_cap = new_cap;
}

static void rt_sched_chans_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_chans_cap) return;
  rt_chan_bytes_t* old_items = ctx->sched_chans;
  uint32_t old_cap = ctx->sched_chans_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_chan_bytes_t);
  uint32_t new_cap = ctx->sched_chans_cap ? ctx->sched_chans_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_chan_bytes_t* items = (rt_chan_bytes_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_chan_bytes_t),
    (uint32_t)_Alignof(rt_chan_bytes_t)
  );
  if (old_items && ctx->sched_chans_len) {
    uint32_t bytes = ctx->sched_chans_len * (uint32_t)sizeof(rt_chan_bytes_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_chan_bytes_t));
  }
  ctx->sched_chans = items;
  ctx->sched_chans_cap = new_cap;
}

static void rt_sched_timers_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->sched_timers_cap) return;
  rt_timer_ev_t* old_items = ctx->sched_timers;
  uint32_t old_cap = ctx->sched_timers_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_timer_ev_t);
  uint32_t new_cap = ctx->sched_timers_cap ? ctx->sched_timers_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_timer_ev_t* items = (rt_timer_ev_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_timer_ev_t),
    (uint32_t)_Alignof(rt_timer_ev_t)
  );
  if (old_items && ctx->sched_timers_len) {
    uint32_t bytes = ctx->sched_timers_len * (uint32_t)sizeof(rt_timer_ev_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_timer_ev_t));
  }
  ctx->sched_timers = items;
  ctx->sched_timers_cap = new_cap;
}

static void rt_io_readers_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->io_readers_cap) return;
  rt_io_reader_t* old_items = ctx->io_readers;
  uint32_t old_cap = ctx->io_readers_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_io_reader_t);
  uint32_t new_cap = ctx->io_readers_cap ? ctx->io_readers_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_io_reader_t* items = (rt_io_reader_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_io_reader_t),
    (uint32_t)_Alignof(rt_io_reader_t)
  );
  if (old_items && ctx->io_readers_len) {
    uint32_t bytes = ctx->io_readers_len * (uint32_t)sizeof(rt_io_reader_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_io_reader_t));
  }
  ctx->io_readers = items;
  ctx->io_readers_cap = new_cap;
}

static void rt_bufreads_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->bufreads_cap) return;
  rt_bufread_t* old_items = ctx->bufreads;
  uint32_t old_cap = ctx->bufreads_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_bufread_t);
  uint32_t new_cap = ctx->bufreads_cap ? ctx->bufreads_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_bufread_t* items = (rt_bufread_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_bufread_t),
    (uint32_t)_Alignof(rt_bufread_t)
  );
  if (old_items && ctx->bufreads_len) {
    uint32_t bytes = ctx->bufreads_len * (uint32_t)sizeof(rt_bufread_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_bufread_t));
  }
  ctx->bufreads = items;
  ctx->bufreads_cap = new_cap;
}

static void rt_ready_push(ctx_t* ctx, uint32_t task_id) {
  if (task_id == 0) return;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) return;
  if (t->in_ready) return;
  t->in_ready = 1;
  t->ready_next = 0;
  if (ctx->sched_ready_tail == 0) {
    ctx->sched_ready_head = task_id;
    ctx->sched_ready_tail = task_id;
    return;
  }
  rt_task_t* tail = rt_task_ptr(ctx, ctx->sched_ready_tail);
  tail->ready_next = task_id;
  ctx->sched_ready_tail = task_id;
}

static uint32_t rt_ready_pop(ctx_t* ctx) {
  for (;;) {
    uint32_t task_id = ctx->sched_ready_head;
    if (task_id == 0) return 0;
    rt_task_t* t = rt_task_ptr(ctx, task_id);
    ctx->sched_ready_head = t->ready_next;
    if (ctx->sched_ready_head == 0) ctx->sched_ready_tail = 0;
    t->ready_next = 0;
    t->in_ready = 0;
    if (t->done) continue;
    return task_id;
  }
}

static void rt_wait_list_push(ctx_t* ctx, uint32_t* head, uint32_t* tail, uint32_t task_id) {
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  t->wait_next = 0;
  if (*tail == 0) {
    *head = task_id;
    *tail = task_id;
    return;
  }
  rt_task_t* last = rt_task_ptr(ctx, *tail);
  last->wait_next = task_id;
  *tail = task_id;
}

static uint32_t rt_wait_list_pop(ctx_t* ctx, uint32_t* head, uint32_t* tail) {
  for (;;) {
    uint32_t task_id = *head;
    if (task_id == 0) return 0;
    rt_task_t* t = rt_task_ptr(ctx, task_id);
    *head = t->wait_next;
    if (*head == 0) *tail = 0;
    t->wait_next = 0;
    if (t->done) continue;
    return task_id;
  }
}

static void rt_sched_wake(ctx_t* ctx, uint32_t task_id, uint32_t reason_kind, uint32_t reason_id) {
  if (task_id == 0) return;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) return;
  t->wait_kind = RT_WAIT_NONE;
  t->wait_id = 0;
  rt_ready_push(ctx, task_id);
  ctx->sched_stats.wake_events += 1;
  rt_sched_trace_event(
    ctx,
    RT_TRACE_WAKE,
    (uint64_t)task_id,
    ((uint64_t)reason_kind << 32) | (uint64_t)reason_id
  );
}

static uint32_t rt_timer_less(rt_timer_ev_t a, rt_timer_ev_t b) {
  if (a.wake_time < b.wake_time) return 1;
  if (a.wake_time > b.wake_time) return 0;
  return (a.seq < b.seq) ? 1 : 0;
}

static void rt_timer_push(ctx_t* ctx, uint64_t wake_time, uint32_t task_id) {
  rt_sched_timers_ensure_cap(ctx, ctx->sched_timers_len + 1);
  uint32_t i = ctx->sched_timers_len++;
  rt_timer_ev_t ev;
  ev.wake_time = wake_time;
  ev.seq = ctx->sched_seq++;
  ev.task_id = task_id;

  while (i > 0) {
    uint32_t p = (i - 1) / 2;
    rt_timer_ev_t parent = ctx->sched_timers[p];
    if (!rt_timer_less(ev, parent)) break;
    ctx->sched_timers[i] = parent;
    i = p;
  }
  ctx->sched_timers[i] = ev;
}

static uint32_t rt_timer_pop(ctx_t* ctx, rt_timer_ev_t* out) {
  if (ctx->sched_timers_len == 0) return 0;
  if (out) *out = ctx->sched_timers[0];
  ctx->sched_timers_len -= 1;
  if (ctx->sched_timers_len == 0) return 1;

  rt_timer_ev_t ev = ctx->sched_timers[ctx->sched_timers_len];
  uint32_t i = 0;
  for (;;) {
    uint32_t l = i * 2 + 1;
    uint32_t r = l + 1;
    if (l >= ctx->sched_timers_len) break;
    uint32_t m = l;
    if (r < ctx->sched_timers_len && rt_timer_less(ctx->sched_timers[r], ctx->sched_timers[l])) {
      m = r;
    }
    if (!rt_timer_less(ctx->sched_timers[m], ev)) break;
    ctx->sched_timers[i] = ctx->sched_timers[m];
    i = m;
  }
  ctx->sched_timers[i] = ev;
  return 1;
}

static uint64_t rt_timer_peek_wake(ctx_t* ctx) {
  if (ctx->sched_timers_len == 0) return UINT64_MAX;
  return ctx->sched_timers[0].wake_time;
}

static uint32_t rt_sched_step(ctx_t* ctx) {
  uint32_t task_id = rt_ready_pop(ctx);
  if (task_id != 0) {
    rt_task_t* t = rt_task_ptr(ctx, task_id);
    if (t->done) return UINT32_C(1);

    ctx->sched_stats.ctx_switches += 1;
    rt_sched_trace_event(ctx, RT_TRACE_SWITCH, (uint64_t)task_id, ctx->sched_now_ticks);

    uint32_t prev = ctx->sched_current_task;
    ctx->sched_current_task = task_id;

    bytes_t out = rt_bytes_empty(ctx);
    uint32_t done = t->poll(ctx, t->fut, &out);

    ctx->sched_current_task = prev;

    if (done) {
      t->done = 1;
      t->out = out;
      t->out_taken = 0;
      if (t->drop && t->fut) {
        t->drop(ctx, t->fut);
      }
      t->drop = NULL;
      t->fut = NULL;
      rt_sched_trace_event(ctx, RT_TRACE_COMPLETE, (uint64_t)task_id, ctx->sched_now_ticks);

      uint32_t w = t->join_wait_head;
      uint32_t wt = t->join_wait_tail;
      (void)wt;
      t->join_wait_head = 0;
      t->join_wait_tail = 0;
      while (w != 0) {
        rt_task_t* waiter = rt_task_ptr(ctx, w);
        uint32_t next = waiter->wait_next;
        waiter->wait_next = 0;
        rt_sched_wake(ctx, w, RT_WAIT_JOIN, task_id);
        w = next;
      }
      return UINT32_C(1);
    }

    if (!t->in_ready && t->wait_kind == RT_WAIT_NONE) {
      rt_trap("task pending without block");
    }
    (void)rt_os_process_poll_all(ctx, 0);
    return UINT32_C(1);
  }

  rt_timer_ev_t ev;
  while (rt_timer_pop(ctx, &ev)) {
    if (ev.task_id == 0) continue;
    rt_task_t* t = rt_task_ptr(ctx, ev.task_id);
    if (t->done) continue;
    if (ev.wake_time > ctx->sched_now_ticks) ctx->sched_now_ticks = ev.wake_time;
    ctx->sched_stats.virtual_time_end = ctx->sched_now_ticks;
    rt_sched_wake(ctx, ev.task_id, RT_WAIT_SLEEP, 0);
    return UINT32_C(1);
  }

  if (rt_os_process_poll_all(ctx, 50)) return UINT32_C(1);
  return UINT32_C(0);
}

static __attribute__((noreturn)) void rt_sched_deadlock(void) {
  rt_trap("scheduler deadlock");
}

static uint32_t rt_task_create(
    ctx_t* ctx,
    uint32_t (*poll)(ctx_t* ctx, void* fut, bytes_t* out),
    void (*drop)(ctx_t* ctx, void* fut),
    void* fut
) {
  rt_sched_tasks_ensure_cap(ctx, ctx->sched_tasks_len + 1);
  uint32_t task_id = ctx->sched_tasks_len + 1;
  rt_task_t* t = &ctx->sched_tasks[task_id - 1];
  memset(t, 0, sizeof(*t));
  t->alive = 1;
  t->poll = poll;
  t->drop = drop;
  t->fut = fut;
  t->out = rt_bytes_empty(ctx);
  t->out_taken = 0;
  ctx->sched_tasks_len += 1;

  ctx->sched_stats.tasks_spawned += 1;
  rt_ready_push(ctx, task_id);
  return task_id;
}

static uint32_t rt_task_spawn(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.spawn_calls += 1;
  (void)rt_task_ptr(ctx, task_id);
  return task_id;
}

static uint32_t rt_task_cancel(ctx_t* ctx, uint32_t task_id) {
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) return UINT32_C(0);
  t->canceled = 1;
  t->done = 1;
  if (t->drop && t->fut) {
    t->drop(ctx, t->fut);
  }
  t->drop = NULL;
  t->fut = NULL;
  rt_bytes_drop(ctx, &t->out);
  t->out = rt_bytes_empty(ctx);
  t->out_taken = 0;
  rt_sched_trace_event(ctx, RT_TRACE_COMPLETE, (uint64_t)task_id, ctx->sched_now_ticks);

  uint32_t w = t->join_wait_head;
  t->join_wait_head = 0;
  t->join_wait_tail = 0;
  while (w != 0) {
    rt_task_t* waiter = rt_task_ptr(ctx, w);
    uint32_t next = waiter->wait_next;
    waiter->wait_next = 0;
    rt_sched_wake(ctx, w, RT_WAIT_JOIN, task_id);
    w = next;
  }
  return UINT32_C(1);
}

static uint32_t rt_task_join_bytes_poll(ctx_t* ctx, uint32_t task_id, bytes_t* out) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (t->done) {
    if (t->out_taken) rt_trap("join already taken");
    t->out_taken = 1;
    if (t->canceled) {
      if (out) *out = rt_bytes_empty(ctx);
      return UINT32_C(1);
    }
    if (out) {
      *out = t->out;
    } else {
      rt_bytes_drop(ctx, &t->out);
    }
    t->out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("join.poll from main");
  if (cur == task_id) rt_trap("join self");

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_JOIN && me->wait_id == task_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("join while already waiting");

  me->wait_kind = RT_WAIT_JOIN;
  me->wait_id = task_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_JOIN << 32) | task_id);
  rt_wait_list_push(ctx, &t->join_wait_head, &t->join_wait_tail, cur);
  return UINT32_C(0);
}

static bytes_t rt_task_join_bytes_block(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  while (!t->done) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    t = rt_task_ptr(ctx, task_id);
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) return rt_bytes_empty(ctx);
  bytes_t out = t->out;
  t->out = rt_bytes_empty(ctx);
  return out;
}

static uint32_t rt_task_is_finished(ctx_t* ctx, uint32_t task_id) {
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  return t->done ? UINT32_C(1) : UINT32_C(0);
}

static result_bytes_t rt_task_try_join_bytes(ctx_t* ctx, uint32_t task_id) {
  ctx->sched_stats.join_calls += 1;
  rt_task_t* t = rt_task_ptr(ctx, task_id);
  if (!t->done) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(1) };
  }
  if (t->out_taken) rt_trap("join already taken");
  t->out_taken = 1;
  if (t->canceled) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  }
  bytes_t out = t->out;
  t->out = rt_bytes_empty(ctx);
  return (result_bytes_t){ .tag = UINT32_C(1), .payload.ok = out };
}

static void rt_task_yield(ctx_t* ctx) {
  ctx->sched_stats.yield_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("yield from main");
  rt_task_t* me = rt_task_ptr(ctx, cur);
  me->wait_kind = RT_WAIT_NONE;
  me->wait_id = 0;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_NONE << 32) | RT_WAIT_NONE);
  rt_ready_push(ctx, cur);
}

static uint32_t rt_task_yield_block(ctx_t* ctx) {
  ctx->sched_stats.yield_calls += 1;
  (void)rt_sched_step(ctx);
  return UINT32_C(0);
}

static void rt_task_sleep(ctx_t* ctx, uint32_t ticks) {
  ctx->sched_stats.sleep_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("sleep from main");
  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (ticks == 0) {
    rt_ready_push(ctx, cur);
    return;
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("sleep while already waiting");
  me->wait_kind = RT_WAIT_SLEEP;
  me->wait_id = 0;
  ctx->sched_stats.blocked_waits += 1;
  uint64_t wake_time = ctx->sched_now_ticks + (uint64_t)ticks;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_SLEEP << 32) | (uint64_t)ticks);
  rt_timer_push(ctx, wake_time, cur);
}

static uint32_t rt_task_sleep_block(ctx_t* ctx, uint32_t ticks) {
  ctx->sched_stats.sleep_calls += 1;
  if (ticks == 0) return UINT32_C(0);
  uint64_t target = ctx->sched_now_ticks + (uint64_t)ticks;
  while (ctx->sched_now_ticks < target) {
    if (ctx->sched_ready_head != 0) {
      if (!rt_sched_step(ctx)) break;
      continue;
    }
    uint64_t next = rt_timer_peek_wake(ctx);
    if (next == UINT64_MAX || next > target) {
      ctx->sched_now_ticks = target;
      ctx->sched_stats.virtual_time_end = ctx->sched_now_ticks;
      break;
    }
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
  }
  return UINT32_C(0);
}

static uint32_t rt_chan_bytes_new(ctx_t* ctx, uint32_t cap) {
  if (cap == 0) rt_trap("chan cap=0");
  rt_sched_chans_ensure_cap(ctx, ctx->sched_chans_len + 1);
  uint32_t chan_id = ctx->sched_chans_len + 1;
  rt_chan_bytes_t* c = &ctx->sched_chans[chan_id - 1];
  memset(c, 0, sizeof(*c));
  c->alive = 1;
  c->closed = 0;
  c->cap = cap;
  c->buf = (bytes_t*)rt_alloc(
    ctx,
    cap * (uint32_t)sizeof(bytes_t),
    (uint32_t)_Alignof(bytes_t)
  );
  c->head = 0;
  c->tail = 0;
  c->len = 0;
  ctx->sched_chans_len += 1;
  return chan_id;
}

static uint32_t rt_chan_bytes_send_poll(ctx_t* ctx, uint32_t chan_id, bytes_t msg) {
  ctx->sched_stats.chan_send_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("chan.send.poll from main");

  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) rt_trap("chan.send closed");

  if (c->len < c->cap) {
    c->buf[c->tail] = msg;
    c->tail = (c->tail + 1) % c->cap;
    c->len += 1;

    uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
    if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
    return UINT32_C(1);
  }

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_CHAN_SEND && me->wait_id == chan_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("chan.send while already waiting");
  me->wait_kind = RT_WAIT_CHAN_SEND;
  me->wait_id = chan_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_CHAN_SEND << 32) | chan_id);
  rt_wait_list_push(ctx, &c->send_wait_head, &c->send_wait_tail, cur);
  return UINT32_C(0);
}

static uint32_t rt_chan_bytes_recv_poll(ctx_t* ctx, uint32_t chan_id, bytes_t* out) {
  ctx->sched_stats.chan_recv_calls += 1;
  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("chan.recv.poll from main");

  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->len != 0) {
    bytes_t msg = c->buf[c->head];
    c->head = (c->head + 1) % c->cap;
    c->len -= 1;
    if (out) *out = msg;

    uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
    if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
    return UINT32_C(1);
  }

  if (c->closed) {
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  rt_task_t* me = rt_task_ptr(ctx, cur);
  if (me->wait_kind == RT_WAIT_CHAN_RECV && me->wait_id == chan_id) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("chan.recv while already waiting");
  me->wait_kind = RT_WAIT_CHAN_RECV;
  me->wait_id = chan_id;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_CHAN_RECV << 32) | chan_id);
  rt_wait_list_push(ctx, &c->recv_wait_head, &c->recv_wait_tail, cur);
  return UINT32_C(0);
}

static uint32_t rt_chan_bytes_send_block(ctx_t* ctx, uint32_t chan_id, bytes_t msg) {
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) rt_trap("chan.send closed");
  while (c->len >= c->cap) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    c = rt_chan_bytes_ptr(ctx, chan_id);
  }
  c->buf[c->tail] = msg;
  c->tail = (c->tail + 1) % c->cap;
  c->len += 1;
  uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
  if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
  return UINT32_C(1);
}

static uint32_t rt_chan_bytes_try_send_view(ctx_t* ctx, uint32_t chan_id, bytes_view_t msg) {
  ctx->sched_stats.chan_send_calls += 1;
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) return UINT32_C(2);
  if (c->len >= c->cap) return UINT32_C(0);

  c->buf[c->tail] = rt_view_to_bytes(ctx, msg);
  c->tail = (c->tail + 1) % c->cap;
  c->len += 1;
  uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
  if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
  return UINT32_C(1);
}

static result_bytes_t rt_chan_bytes_try_recv(ctx_t* ctx, uint32_t chan_id) {
  ctx->sched_stats.chan_recv_calls += 1;
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);

  if (c->len != 0) {
    bytes_t msg = c->buf[c->head];
    c->head = (c->head + 1) % c->cap;
    c->len -= 1;
    uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
    if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
    return (result_bytes_t){ .tag = UINT32_C(1), .payload.ok = msg };
  }

  if (c->closed) {
    return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(2) };
  }
  return (result_bytes_t){ .tag = UINT32_C(0), .payload.err = UINT32_C(1) };
}

static bytes_t rt_chan_bytes_recv_block(ctx_t* ctx, uint32_t chan_id) {
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  while (c->len == 0 && !c->closed) {
    if (!rt_sched_step(ctx)) rt_sched_deadlock();
    c = rt_chan_bytes_ptr(ctx, chan_id);
  }
  if (c->len == 0) return rt_bytes_empty(ctx);
  bytes_t msg = c->buf[c->head];
  c->head = (c->head + 1) % c->cap;
  c->len -= 1;
  uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
  if (w != 0) rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
  return msg;
}

static uint32_t rt_chan_bytes_close(ctx_t* ctx, uint32_t chan_id) {
  rt_chan_bytes_t* c = rt_chan_bytes_ptr(ctx, chan_id);
  if (c->closed) return UINT32_C(0);
  c->closed = 1;
  for (;;) {
    uint32_t w = rt_wait_list_pop(ctx, &c->recv_wait_head, &c->recv_wait_tail);
    if (w == 0) break;
    rt_sched_wake(ctx, w, RT_WAIT_CHAN_RECV, chan_id);
  }
  for (;;) {
    uint32_t w = rt_wait_list_pop(ctx, &c->send_wait_head, &c->send_wait_tail);
    if (w == 0) break;
    rt_sched_wake(ctx, w, RT_WAIT_CHAN_SEND, chan_id);
  }
  return UINT32_C(1);
}

#if X07_ENABLE_STREAMING_FILE_IO
static uint32_t rt_io_reader_new_file(ctx_t* ctx, FILE* f, uint32_t pending_ticks) {
  if (!f) rt_trap("io.reader null file");
  rt_io_readers_ensure_cap(ctx, ctx->io_readers_len + 1);
  uint32_t reader_id = ctx->io_readers_len + 1;
  rt_io_reader_t* r = &ctx->io_readers[reader_id - 1];
  memset(r, 0, sizeof(*r));
  r->alive = 1;
  r->kind = RT_IO_READER_KIND_FILE;
  r->eof = 0;
  r->pending_ticks = pending_ticks;
  r->f = f;
  r->bytes = rt_bytes_empty(ctx);
  r->pos = 0;
  ctx->io_readers_len += 1;
  return reader_id;
}
#else
static uint32_t rt_io_reader_new_file(ctx_t* ctx, void* f, uint32_t pending_ticks) {
  (void)ctx;
  (void)f;
  (void)pending_ticks;
  rt_trap("io.reader file disabled");
}
#endif

static uint32_t rt_io_reader_new_bytes(ctx_t* ctx, bytes_t b, uint32_t pending_ticks) {
  rt_io_readers_ensure_cap(ctx, ctx->io_readers_len + 1);
  uint32_t reader_id = ctx->io_readers_len + 1;
  rt_io_reader_t* r = &ctx->io_readers[reader_id - 1];
  memset(r, 0, sizeof(*r));
  r->alive = 1;
  r->kind = RT_IO_READER_KIND_BYTES;
  r->eof = 0;
  r->pending_ticks = pending_ticks;
#if X07_ENABLE_STREAMING_FILE_IO
  r->f = NULL;
#endif
  r->bytes = b;
  r->pos = 0;
  ctx->io_readers_len += 1;
  return reader_id;
}

static uint32_t rt_io_read_poll(ctx_t* ctx, uint32_t reader_id, uint32_t max, bytes_t* out) {
  rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader_id);
  if (max == 0 || r->eof) {
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  if (r->pending_ticks != 0) {
    uint32_t ticks = r->pending_ticks;
    r->pending_ticks = 0;
    rt_task_sleep(ctx, ticks);
    return UINT32_C(0);
  }

  if (r->kind == RT_IO_READER_KIND_BYTES) {
    bytes_t b = r->bytes;
    uint32_t pos = r->pos;
    if (pos > b.len) pos = b.len;
    uint32_t rem = b.len - pos;
    uint32_t n = (rem < max) ? rem : max;
    if (n) {
      r->pos = pos + n;
      bytes_t slice = (bytes_t){b.ptr + pos, n};
      if (out) *out = rt_bytes_clone(ctx, slice);
      return UINT32_C(1);
    }
    r->eof = 1;
    rt_bytes_drop(ctx, &r->bytes);
    r->bytes = rt_bytes_empty(ctx);
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

#if X07_ENABLE_STREAMING_FILE_IO
  if (r->kind != RT_IO_READER_KIND_FILE) rt_trap("io.read bad reader kind");
  if (!r->f) {
    r->eof = 1;
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  int c = fgetc(r->f);
  if (c == EOF) {
    fclose(r->f);
    r->f = NULL;
    r->eof = 1;
    if (out) *out = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  bytes_t chunk = rt_bytes_alloc(ctx, max);
  chunk.ptr[0] = (uint8_t)c;
  uint32_t got = 1;
  if (max > 1) {
    size_t n = fread(chunk.ptr + 1, 1, (size_t)(max - 1), r->f);
    if (n > (size_t)(UINT32_MAX - 1)) rt_trap("io.read too large");
    got += (uint32_t)n;
  }
  chunk.len = got;
  if (out) *out = chunk;
  return UINT32_C(1);
#else
  rt_trap("io.read bad reader kind");
#endif
}

static bytes_t rt_io_read_block(ctx_t* ctx, uint32_t reader_id, uint32_t max) {
  rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader_id);
  if (max == 0 || r->eof) return rt_bytes_empty(ctx);

  if (r->pending_ticks != 0) {
    uint32_t ticks = r->pending_ticks;
    r->pending_ticks = 0;
    rt_task_sleep_block(ctx, ticks);
  }

  if (r->kind == RT_IO_READER_KIND_BYTES) {
    bytes_t b = r->bytes;
    uint32_t pos = r->pos;
    if (pos > b.len) pos = b.len;
    uint32_t rem = b.len - pos;
    uint32_t n = (rem < max) ? rem : max;
    if (n) {
      r->pos = pos + n;
      bytes_t slice = (bytes_t){b.ptr + pos, n};
      return rt_bytes_clone(ctx, slice);
    }
    r->eof = 1;
    rt_bytes_drop(ctx, &r->bytes);
    r->bytes = rt_bytes_empty(ctx);
    return rt_bytes_empty(ctx);
  }

#if X07_ENABLE_STREAMING_FILE_IO
  if (r->kind != RT_IO_READER_KIND_FILE) rt_trap("io.read bad reader kind");
  if (!r->f) {
    r->eof = 1;
    return rt_bytes_empty(ctx);
  }

  int c = fgetc(r->f);
  if (c == EOF) {
    fclose(r->f);
    r->f = NULL;
    r->eof = 1;
    return rt_bytes_empty(ctx);
  }

  bytes_t chunk = rt_bytes_alloc(ctx, max);
  chunk.ptr[0] = (uint8_t)c;
  uint32_t got = 1;
  if (max > 1) {
    size_t n = fread(chunk.ptr + 1, 1, (size_t)(max - 1), r->f);
    if (n > (size_t)(UINT32_MAX - 1)) rt_trap("io.read too large");
    got += (uint32_t)n;
  }
  chunk.len = got;
  return chunk;
#else
  rt_trap("io.read bad reader kind");
#endif
}

static bytes_t rt_iface_io_read_block(ctx_t* ctx, iface_t reader, uint32_t max) {
  if (max == 0) return rt_bytes_empty(ctx);
  if (reader.vtable == RT_IFACE_VTABLE_IO_READER) {
    return rt_io_read_block(ctx, reader.data, max);
  }
  if (reader.vtable < RT_IFACE_VTABLE_EXT_IO_READER_MIN || reader.vtable > RT_IFACE_VTABLE_EXT_IO_READER_MAX) {
    rt_trap("io.read bad iface vtable");
  }

  bytes_t chunk = rt_bytes_alloc(ctx, max);
  uint32_t got = rt_ext_io_reader_read_into(reader.vtable, reader.data, chunk.ptr, max);
  if (got == 0) {
    rt_bytes_drop(ctx, &chunk);
    return rt_bytes_empty(ctx);
  }
  chunk.len = got;
  return chunk;
}

static uint32_t rt_bufread_new(ctx_t* ctx, iface_t reader, uint32_t cap) {
  if (cap == 0) rt_trap("bufread cap=0");
  if (reader.vtable == RT_IFACE_VTABLE_IO_READER) {
    (void)rt_io_reader_ptr(ctx, reader.data);
  } else if (reader.vtable >= RT_IFACE_VTABLE_EXT_IO_READER_MIN && reader.vtable <= RT_IFACE_VTABLE_EXT_IO_READER_MAX) {
    // External IO reader: validated lazily on first read.
  } else {
    rt_trap("bufread.new bad iface vtable");
  }

  rt_bufreads_ensure_cap(ctx, ctx->bufreads_len + 1);
  uint32_t br_id = ctx->bufreads_len + 1;
  rt_bufread_t* br = &ctx->bufreads[br_id - 1];
  memset(br, 0, sizeof(*br));
  br->alive = 1;
  br->reader = reader;
  br->eof = 0;
  br->buf = rt_bytes_alloc(ctx, cap);
  br->start = 0;
  br->end = 0;
  ctx->bufreads_len += 1;
  return br_id;
}

static uint32_t rt_bufread_fill_poll(ctx_t* ctx, uint32_t br_id, bytes_view_t* out) {
  rt_bufread_t* br = rt_bufread_ptr(ctx, br_id);
  if (br->start > br->end) rt_trap("bufread corrupt");
  uint32_t avail = br->end - br->start;
  if (avail != 0) {
    if (out) *out = rt_bytes_subview(ctx, br->buf, br->start, avail);
    return UINT32_C(1);
  }
  if (br->eof) {
    if (out) *out = rt_view_empty(ctx);
    return UINT32_C(1);
  }

  iface_t reader = br->reader;
  if (reader.vtable != RT_IFACE_VTABLE_IO_READER) {
    uint32_t cap = br->buf.len;
    uint32_t got = rt_ext_io_reader_read_into(reader.vtable, reader.data, br->buf.ptr, cap);
    br->start = 0;
    br->end = got;
    if (got == 0) {
      br->eof = 1;
      if (out) *out = rt_view_empty(ctx);
      return UINT32_C(1);
    }
    if (out) *out = rt_bytes_subview(ctx, br->buf, 0, got);
    return UINT32_C(1);
  }

  rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
  if (r->eof) {
    br->eof = 1;
    if (out) *out = rt_view_empty(ctx);
    return UINT32_C(1);
  }
  if (r->pending_ticks != 0) {
    uint32_t ticks = r->pending_ticks;
    r->pending_ticks = 0;
    rt_task_sleep(ctx, ticks);
    return UINT32_C(0);
  }

  uint32_t cap = br->buf.len;
  uint32_t got = 0;
  if (r->kind == RT_IO_READER_KIND_BYTES) {
    bytes_t b = r->bytes;
    uint32_t pos = r->pos;
    if (pos > b.len) pos = b.len;
    uint32_t rem = b.len - pos;
    got = (rem < cap) ? rem : cap;
    if (got) {
      memcpy(br->buf.ptr, b.ptr + pos, got);
      rt_mem_on_memcpy(ctx, got);
      r->pos = pos + got;
    } else {
      r->eof = 1;
    }
  }
#if X07_ENABLE_STREAMING_FILE_IO
  else if (r->kind == RT_IO_READER_KIND_FILE) {
    if (!r->f) {
      r->eof = 1;
    } else {
      int c = fgetc(r->f);
      if (c == EOF) {
        fclose(r->f);
        r->f = NULL;
        r->eof = 1;
      } else {
        br->buf.ptr[0] = (uint8_t)c;
        got = 1;
        if (cap > 1) {
          size_t n = fread(br->buf.ptr + 1, 1, (size_t)(cap - 1), r->f);
          if (n > (size_t)(UINT32_MAX - 1)) rt_trap("bufread.fill too large");
          got += (uint32_t)n;
        }
      }
    }
  }
#endif
  else {
    rt_trap("bufread bad reader kind");
  }

  br->start = 0;
  br->end = got;
  if (got == 0) {
    br->eof = 1;
    if (out) *out = rt_view_empty(ctx);
    return UINT32_C(1);
  }

  if (out) *out = rt_bytes_subview(ctx, br->buf, 0, got);
  return UINT32_C(1);
}

static bytes_view_t rt_bufread_fill_block(ctx_t* ctx, uint32_t br_id) {
  rt_bufread_t* br = rt_bufread_ptr(ctx, br_id);
  for (;;) {
    if (br->start > br->end) rt_trap("bufread corrupt");
    uint32_t avail = br->end - br->start;
    if (avail != 0) return rt_bytes_subview(ctx, br->buf, br->start, avail);
    if (br->eof) return rt_view_empty(ctx);

    iface_t reader = br->reader;
    if (reader.vtable != RT_IFACE_VTABLE_IO_READER) {
      uint32_t cap = br->buf.len;
      uint32_t got = rt_ext_io_reader_read_into(reader.vtable, reader.data, br->buf.ptr, cap);
      br->start = 0;
      br->end = got;
      if (got == 0) {
        br->eof = 1;
        return rt_view_empty(ctx);
      }
      return rt_bytes_subview(ctx, br->buf, 0, got);
    }

    rt_io_reader_t* r = rt_io_reader_ptr(ctx, reader.data);
    if (r->eof) {
      br->eof = 1;
      return rt_view_empty(ctx);
    }
    if (r->pending_ticks != 0) {
      uint32_t ticks = r->pending_ticks;
      r->pending_ticks = 0;
      rt_task_sleep_block(ctx, ticks);
      continue;
    }

    uint32_t cap = br->buf.len;
    uint32_t got = 0;
    if (r->kind == RT_IO_READER_KIND_BYTES) {
      bytes_t b = r->bytes;
      uint32_t pos = r->pos;
      if (pos > b.len) pos = b.len;
      uint32_t rem = b.len - pos;
      got = (rem < cap) ? rem : cap;
      if (got) {
        memcpy(br->buf.ptr, b.ptr + pos, got);
        rt_mem_on_memcpy(ctx, got);
        r->pos = pos + got;
      } else {
        r->eof = 1;
      }
    }
#if X07_ENABLE_STREAMING_FILE_IO
    else if (r->kind == RT_IO_READER_KIND_FILE) {
      if (!r->f) {
        r->eof = 1;
      } else {
        int c = fgetc(r->f);
        if (c == EOF) {
          fclose(r->f);
          r->f = NULL;
          r->eof = 1;
        } else {
          br->buf.ptr[0] = (uint8_t)c;
          got = 1;
          if (cap > 1) {
            size_t n = fread(br->buf.ptr + 1, 1, (size_t)(cap - 1), r->f);
            if (n > (size_t)(UINT32_MAX - 1)) rt_trap("bufread.fill too large");
            got += (uint32_t)n;
          }
        }
      }
    }
#endif
    else {
      rt_trap("bufread bad reader kind");
    }

    br->start = 0;
    br->end = got;
    if (got == 0) {
      br->eof = 1;
      return rt_view_empty(ctx);
    }

    return rt_bytes_subview(ctx, br->buf, 0, got);
  }
}

static uint32_t rt_bufread_consume(ctx_t* ctx, uint32_t br_id, uint32_t n) {
  rt_bufread_t* br = rt_bufread_ptr(ctx, br_id);
  if (br->start > br->end) rt_trap("bufread corrupt");
  uint32_t avail = br->end - br->start;
  if (n > avail) rt_trap("bufread.consume oob");
  br->start += n;
  if (br->start == br->end) {
    br->start = 0;
    br->end = 0;
  }
  return UINT32_C(0);
}

static uint32_t rt_fs_is_safe_rel_path(bytes_view_t path) {
  if (path.len == 0) return UINT32_C(0);
  if (path.ptr[0] == (uint8_t)'/') return UINT32_C(0);

  uint32_t seg_start = 0;
  for (uint32_t i = 0; i <= path.len; i++) {
    uint8_t b = (i == path.len) ? (uint8_t)'/' : path.ptr[i];
    if (i < path.len) {
      if (b == 0 || b == (uint8_t)'\\') return UINT32_C(0);
    }
    if (b == (uint8_t)'/') {
      uint32_t seg_len = i - seg_start;
      if (seg_len == 0) return UINT32_C(0);
      if (seg_len == 1 && path.ptr[seg_start] == (uint8_t)'.') return UINT32_C(0);
      if (seg_len == 2
          && path.ptr[seg_start] == (uint8_t)'.'
          && path.ptr[seg_start + 1] == (uint8_t)'.') return UINT32_C(0);
      if (seg_len >= 5 && memcmp(path.ptr + seg_start, ".x07_", 5) == 0) return UINT32_C(0);
      seg_start = i + 1;
    }
  }
  return UINT32_C(1);
}

#if X07_ENABLE_FS
static bytes_t rt_fs_read(ctx_t* ctx, bytes_view_t path) {
  if (!X07_ENABLE_FS) rt_trap("fs disabled");
  if (!rt_fs_is_safe_rel_path(path)) rt_trap("fs.read unsafe path");
  ctx->fs_read_file_calls += 1;

  char* p = (char*)rt_alloc(ctx, path.len + 1, 1);
  memcpy(p, path.ptr, path.len);
  rt_mem_on_memcpy(ctx, path.len);
  p[path.len] = 0;

  FILE* f = fopen(p, "rb");
  if (!f) {
    rt_free(ctx, p, path.len + 1, 1);
    rt_trap("fs.read open failed");
  }
  rt_free(ctx, p, path.len + 1, 1);

  if (fseek(f, 0, SEEK_END) != 0) rt_trap("fs.read seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("fs.read tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("fs.read file too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("fs.read seek failed");

  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)end);
  if (out.len != 0) {
    size_t n = fread(out.ptr, 1, out.len, f);
    if (n != out.len) rt_trap("fs.read short read");
  }
  fclose(f);
  return out;
}

static int rt_fs_name_cmp(const void* a, const void* b) {
  const char* const* pa = (const char* const*)a;
  const char* const* pb = (const char* const*)b;
  return strcmp(*pa, *pb);
}

static bytes_t rt_fs_list_dir(ctx_t* ctx, bytes_view_t path) {
  if (!X07_ENABLE_FS) rt_trap("fs disabled");
  if (!rt_fs_is_safe_rel_path(path)) rt_trap("fs.list_dir unsafe path");
  ctx->fs_list_dir_calls += 1;

  char* p = (char*)rt_alloc(ctx, path.len + 1, 1);
  memcpy(p, path.ptr, path.len);
  rt_mem_on_memcpy(ctx, path.len);
  p[path.len] = 0;

  DIR* dir = opendir(p);
  if (!dir) {
    rt_free(ctx, p, path.len + 1, 1);
    rt_trap("fs.list_dir open failed");
  }

  uint32_t count = 0;
  for (;;) {
    struct dirent* ent = readdir(dir);
    if (!ent) break;
    const char* name = ent->d_name;
    if (!name) continue;
    if (name[0] == '.' && name[1] == 0) continue;
    if (name[0] == '.' && name[1] == '.' && name[2] == 0) continue;
    if (strncmp(name, ".x07_", 5) == 0) continue;
    count += 1;
  }
  closedir(dir);

  if (count == 0) {
    rt_free(ctx, p, path.len + 1, 1);
    bytes_t out;
    out.len = 0;
    out.ptr = (uint8_t*)rt_alloc(ctx, 0, 1);
    return out;
  }

  uint32_t names_cap = count;
  char** names = (char**)rt_alloc(ctx, count * (uint32_t)sizeof(char*), 8);

  dir = opendir(p);
  if (!dir) {
    rt_free(ctx, names, names_cap * (uint32_t)sizeof(char*), 8);
    rt_free(ctx, p, path.len + 1, 1);
    rt_trap("fs.list_dir open failed");
  }

  uint32_t idx = 0;
  for (;;) {
    struct dirent* ent = readdir(dir);
    if (!ent) break;
    const char* name = ent->d_name;
    if (!name) continue;
    if (name[0] == '.' && name[1] == 0) continue;
    if (name[0] == '.' && name[1] == '.' && name[2] == 0) continue;
    if (strncmp(name, ".x07_", 5) == 0) continue;

    size_t len = strlen(name);
    if (len > (size_t)UINT32_MAX) rt_trap("fs.list_dir name too long");
    char* copy = (char*)rt_alloc(ctx, (uint32_t)len + 1, 1);
    memcpy(copy, name, len + 1);
    rt_mem_on_memcpy(ctx, (uint32_t)len + 1);
    if (idx < count) names[idx] = copy;
    idx += 1;
  }
  closedir(dir);

  if (idx < count) count = idx;
  qsort(names, count, sizeof(char*), rt_fs_name_cmp);

  uint64_t out_len_u64 = 0;
  for (uint32_t i = 0; i < count; i++) {
    size_t len = strlen(names[i]);
    if (len > (size_t)UINT32_MAX) rt_trap("fs.list_dir name too long");
    out_len_u64 += (uint64_t)len + 1;
    if (out_len_u64 > (uint64_t)UINT32_MAX) rt_trap("fs.list_dir output too large");
  }

  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)out_len_u64);
  uint32_t off = 0;
  for (uint32_t i = 0; i < count; i++) {
    uint32_t len = (uint32_t)strlen(names[i]);
    if (len) {
      memcpy(out.ptr + off, names[i], len);
      rt_mem_on_memcpy(ctx, len);
      off += len;
    }
    out.ptr[off] = (uint8_t)'\n';
    off += 1;
  }

  for (uint32_t i = 0; i < count; i++) {
    size_t len = strlen(names[i]);
    if (len > (size_t)UINT32_MAX) rt_trap("fs.list_dir name too long");
    rt_free(ctx, names[i], (uint32_t)len + 1, 1);
  }
  rt_free(ctx, names, names_cap * (uint32_t)sizeof(char*), 8);
  rt_free(ctx, p, path.len + 1, 1);
  return out;
}

static void rt_fs_latency_load(ctx_t* ctx) {
  if (ctx->fs_latency_loaded) return;
  ctx->fs_latency_loaded = 1;
  ctx->fs_latency_default_ticks = 0;
  ctx->fs_latency_entries = NULL;
  ctx->fs_latency_len = 0;
  ctx->fs_latency_blob = rt_bytes_empty(ctx);

  FILE* f = fopen(".x07_fs/latency.evfslat", "rb");
  if (!f) return;
  if (fseek(f, 0, SEEK_END) != 0) rt_trap("fs latency seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("fs latency tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("fs latency too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("fs latency seek failed");

  bytes_t blob = rt_bytes_alloc(ctx, (uint32_t)end);
  if (blob.len != 0) {
    size_t got = fread(blob.ptr, 1, blob.len, f);
    if (got != blob.len) rt_trap("fs latency short read");
  }
  fclose(f);

  if (blob.len < 16) rt_trap("fs latency too short");
  if (memcmp(blob.ptr, "X7FL", 4) != 0) rt_trap("fs latency bad magic");
  uint16_t ver = rt_read_u16_le(blob.ptr + 4);
  if (ver != 1) rt_trap("fs latency bad version");

  uint32_t default_ticks = rt_read_u32_le(blob.ptr + 8);
  uint32_t count = rt_read_u32_le(blob.ptr + 12);

  fs_latency_entry_t* entries = NULL;
  if (count != 0) {
    entries = (fs_latency_entry_t*)rt_alloc(
      ctx,
      count * (uint32_t)sizeof(fs_latency_entry_t),
      (uint32_t)_Alignof(fs_latency_entry_t)
    );
  }

  uint32_t off = 16;
  for (uint32_t i = 0; i < count; i++) {
    if (off > blob.len || blob.len - off < 4) rt_trap("fs latency truncated path_len");
    uint32_t plen = rt_read_u32_le(blob.ptr + off);
    off += 4;
    if (off > blob.len || blob.len - off < plen) rt_trap("fs latency truncated path");
    entries[i].path = (bytes_t){blob.ptr + off, plen};
    off += plen;
    if (off > blob.len || blob.len - off < 4) rt_trap("fs latency truncated ticks");
    entries[i].ticks = rt_read_u32_le(blob.ptr + off);
    off += 4;
  }
  if (off != blob.len) rt_trap("fs latency trailing bytes");

  ctx->fs_latency_default_ticks = default_ticks;
  ctx->fs_latency_entries = entries;
  ctx->fs_latency_len = count;
  ctx->fs_latency_blob = blob;
}

static uint32_t rt_fs_latency_ticks(ctx_t* ctx, bytes_view_t path) {
  (void)ctx;
  rt_fs_latency_load(ctx);
  for (uint32_t i = 0; i < ctx->fs_latency_len; i++) {
    bytes_t p = ctx->fs_latency_entries[i].path;
    if (p.len != path.len) continue;
    if (p.len == 0) return ctx->fs_latency_entries[i].ticks;
    if (memcmp(p.ptr, path.ptr, p.len) == 0) return ctx->fs_latency_entries[i].ticks;
  }
  return ctx->fs_latency_default_ticks;
}

static uint32_t rt_fs_open_read(ctx_t* ctx, bytes_view_t path) {
  if (!X07_ENABLE_FS) rt_trap("fs disabled");
  if (!rt_fs_is_safe_rel_path(path)) rt_trap("fs.open_read unsafe path");
  ctx->fs_read_file_calls += 1;

  char* p = (char*)rt_alloc(ctx, path.len + 1, 1);
  memcpy(p, path.ptr, path.len);
  rt_mem_on_memcpy(ctx, path.len);
  p[path.len] = 0;

  FILE* f = fopen(p, "rb");
  if (!f) {
    rt_free(ctx, p, path.len + 1, 1);
    rt_trap("fs.open_read open failed");
  }
  rt_free(ctx, p, path.len + 1, 1);

  uint32_t ticks = rt_fs_latency_ticks(ctx, path);
  return rt_io_reader_new_file(ctx, f, ticks);
}

static bytes_t rt_fs_read_async_block(ctx_t* ctx, bytes_view_t path) {
  uint32_t ticks = rt_fs_latency_ticks(ctx, path);
  if (ticks != 0) {
    rt_task_sleep_block(ctx, ticks);
  }
  return rt_fs_read(ctx, path);
}
#endif

static uint32_t rt_sha256_rotr(uint32_t x, uint32_t n) {
  return (x >> n) | (x << (32u - n));
}

static uint32_t rt_sha256_load_u32_be(const uint8_t* p) {
  return ((uint32_t)p[0] << 24)
       | ((uint32_t)p[1] << 16)
       | ((uint32_t)p[2] << 8)
       | (uint32_t)p[3];
}

static void rt_sha256_store_u32_be(uint8_t* out, uint32_t x) {
  out[0] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
  out[1] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  out[2] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  out[3] = (uint8_t)(x & UINT32_C(0xFF));
}

static void rt_sha256_compress(uint32_t state[8], const uint8_t block[64]) {
  static const uint32_t K[64] = {
    UINT32_C(0x428a2f98), UINT32_C(0x71374491), UINT32_C(0xb5c0fbcf), UINT32_C(0xe9b5dba5),
    UINT32_C(0x3956c25b), UINT32_C(0x59f111f1), UINT32_C(0x923f82a4), UINT32_C(0xab1c5ed5),
    UINT32_C(0xd807aa98), UINT32_C(0x12835b01), UINT32_C(0x243185be), UINT32_C(0x550c7dc3),
    UINT32_C(0x72be5d74), UINT32_C(0x80deb1fe), UINT32_C(0x9bdc06a7), UINT32_C(0xc19bf174),
    UINT32_C(0xe49b69c1), UINT32_C(0xefbe4786), UINT32_C(0x0fc19dc6), UINT32_C(0x240ca1cc),
    UINT32_C(0x2de92c6f), UINT32_C(0x4a7484aa), UINT32_C(0x5cb0a9dc), UINT32_C(0x76f988da),
    UINT32_C(0x983e5152), UINT32_C(0xa831c66d), UINT32_C(0xb00327c8), UINT32_C(0xbf597fc7),
    UINT32_C(0xc6e00bf3), UINT32_C(0xd5a79147), UINT32_C(0x06ca6351), UINT32_C(0x14292967),
    UINT32_C(0x27b70a85), UINT32_C(0x2e1b2138), UINT32_C(0x4d2c6dfc), UINT32_C(0x53380d13),
    UINT32_C(0x650a7354), UINT32_C(0x766a0abb), UINT32_C(0x81c2c92e), UINT32_C(0x92722c85),
    UINT32_C(0xa2bfe8a1), UINT32_C(0xa81a664b), UINT32_C(0xc24b8b70), UINT32_C(0xc76c51a3),
    UINT32_C(0xd192e819), UINT32_C(0xd6990624), UINT32_C(0xf40e3585), UINT32_C(0x106aa070),
    UINT32_C(0x19a4c116), UINT32_C(0x1e376c08), UINT32_C(0x2748774c), UINT32_C(0x34b0bcb5),
    UINT32_C(0x391c0cb3), UINT32_C(0x4ed8aa4a), UINT32_C(0x5b9cca4f), UINT32_C(0x682e6ff3),
    UINT32_C(0x748f82ee), UINT32_C(0x78a5636f), UINT32_C(0x84c87814), UINT32_C(0x8cc70208),
    UINT32_C(0x90befffa), UINT32_C(0xa4506ceb), UINT32_C(0xbef9a3f7), UINT32_C(0xc67178f2),
  };

  uint32_t w[64];
  for (uint32_t i = 0; i < 16; i++) {
    w[i] = rt_sha256_load_u32_be(block + (i * 4));
  }
  for (uint32_t i = 16; i < 64; i++) {
    uint32_t s0 = rt_sha256_rotr(w[i - 15], 7) ^ rt_sha256_rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
    uint32_t s1 = rt_sha256_rotr(w[i - 2], 17) ^ rt_sha256_rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
    w[i] = w[i - 16] + s0 + w[i - 7] + s1;
  }

  uint32_t a = state[0];
  uint32_t b = state[1];
  uint32_t c = state[2];
  uint32_t d = state[3];
  uint32_t e = state[4];
  uint32_t f = state[5];
  uint32_t g = state[6];
  uint32_t h = state[7];

  for (uint32_t i = 0; i < 64; i++) {
    uint32_t S1 = rt_sha256_rotr(e, 6) ^ rt_sha256_rotr(e, 11) ^ rt_sha256_rotr(e, 25);
    uint32_t ch = (e & f) ^ ((~e) & g);
    uint32_t temp1 = h + S1 + ch + K[i] + w[i];
    uint32_t S0 = rt_sha256_rotr(a, 2) ^ rt_sha256_rotr(a, 13) ^ rt_sha256_rotr(a, 22);
    uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
    uint32_t temp2 = S0 + maj;

    h = g;
    g = f;
    f = e;
    e = d + temp1;
    d = c;
    c = b;
    b = a;
    a = temp1 + temp2;
  }

  state[0] += a;
  state[1] += b;
  state[2] += c;
  state[3] += d;
  state[4] += e;
  state[5] += f;
  state[6] += g;
  state[7] += h;
}

static void rt_sha256(const uint8_t* data, uint32_t len, uint8_t out[32]) {
  uint32_t state[8] = {
    UINT32_C(0x6a09e667),
    UINT32_C(0xbb67ae85),
    UINT32_C(0x3c6ef372),
    UINT32_C(0xa54ff53a),
    UINT32_C(0x510e527f),
    UINT32_C(0x9b05688c),
    UINT32_C(0x1f83d9ab),
    UINT32_C(0x5be0cd19),
  };

  uint32_t off = 0;
  while (len - off >= 64) {
    rt_sha256_compress(state, data + off);
    off += 64;
  }

  uint64_t bit_len = (uint64_t)len * UINT64_C(8);
  uint8_t block[128];
  memset(block, 0, sizeof(block));
  uint32_t rem = len - off;
  if (rem) memcpy(block, data + off, rem);
  block[rem] = UINT8_C(0x80);

  if (rem < 56) {
    for (uint32_t i = 0; i < 8; i++) {
      block[56 + i] = (uint8_t)((bit_len >> (56 - (i * 8))) & UINT64_C(0xFF));
    }
    rt_sha256_compress(state, block);
  } else {
    for (uint32_t i = 0; i < 8; i++) {
      block[120 + i] = (uint8_t)((bit_len >> (56 - (i * 8))) & UINT64_C(0xFF));
    }
    rt_sha256_compress(state, block);
    rt_sha256_compress(state, block + 64);
  }

  for (uint32_t i = 0; i < 8; i++) {
    rt_sha256_store_u32_be(out + (i * 4), state[i]);
  }
}

static void rt_hex_bytes(const uint8_t* bytes, uint32_t len, char* out) {
  static const char LUT[16] = "0123456789abcdef";
  for (uint32_t i = 0; i < len; i++) {
    uint8_t b = bytes[i];
    out[i * 2 + 0] = LUT[b >> 4];
    out[i * 2 + 1] = LUT[b & 0x0F];
  }
  out[len * 2] = 0;
}

#if X07_ENABLE_RR
static bytes_t rt_rr_send_request(ctx_t* ctx, bytes_view_t req) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");
  ctx->rr_send_calls += 1;
  ctx->rr_request_calls += 1;

#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, req.bid, req.off_bytes, req.len);
#endif

  uint8_t digest[32];
  rt_sha256(req.ptr, req.len, digest);
  memcpy(ctx->rr_last_request_sha256, digest, 32);

  char hex[65];
  rt_hex_bytes(digest, 32, hex);

  char path[96];
  int n = snprintf(path, sizeof(path), ".x07_rr/responses/%s.bin", hex);
  if (n < 0 || (size_t)n >= sizeof(path)) rt_trap("rr response path too long");

  FILE* f = fopen(path, "rb");
  if (!f) {
    bytes_t miss = rt_bytes_alloc(ctx, 3);
    miss.ptr[0] = UINT8_C(1);
    miss.ptr[1] = UINT8_C(1);
    miss.ptr[2] = UINT8_C(0);
    return miss;
  }

  if (fseek(f, 0, SEEK_END) != 0) rt_trap("rr.send_request seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("rr.send_request tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("rr.send_request response too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("rr.send_request seek failed");

  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)end);
  if (out.len != 0) {
    size_t got = fread(out.ptr, 1, out.len, f);
    if (got != out.len) rt_trap("rr.send_request short read");
  }
  fclose(f);
  return out;
}

static void rt_rr_index_load(ctx_t* ctx) {
  if (ctx->rr_index_loaded) return;
  ctx->rr_index_loaded = 1;
  ctx->rr_index_default_latency_ticks = 0;
  ctx->rr_index_entries = NULL;
  ctx->rr_index_len = 0;
  ctx->rr_index_blob = rt_bytes_empty(ctx);

  FILE* f = fopen(".x07_rr/index.evrr", "rb");
  if (!f) rt_trap("rr index open failed");
  if (fseek(f, 0, SEEK_END) != 0) rt_trap("rr index seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("rr index tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("rr index too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("rr index seek failed");

  bytes_t blob = rt_bytes_alloc(ctx, (uint32_t)end);
  if (blob.len != 0) {
    size_t got = fread(blob.ptr, 1, blob.len, f);
    if (got != blob.len) rt_trap("rr index short read");
  }
  fclose(f);

  if (blob.len < 16) rt_trap("rr index too short");
  if (memcmp(blob.ptr, "X7RR", 4) != 0) rt_trap("rr index bad magic");
  uint16_t ver = rt_read_u16_le(blob.ptr + 4);
  if (ver != 1) rt_trap("rr index bad version");

  uint32_t default_ticks = rt_read_u32_le(blob.ptr + 8);
  uint32_t count = rt_read_u32_le(blob.ptr + 12);

  rr_index_entry_t* entries = NULL;
  if (count != 0) {
    entries = (rr_index_entry_t*)rt_alloc(
      ctx,
      count * (uint32_t)sizeof(rr_index_entry_t),
      (uint32_t)_Alignof(rr_index_entry_t)
    );
  }

  uint32_t off = 16;
  for (uint32_t i = 0; i < count; i++) {
    if (off > blob.len || blob.len - off < 4) rt_trap("rr index truncated key_len");
    uint32_t klen = rt_read_u32_le(blob.ptr + off);
    off += 4;
    if (off > blob.len || blob.len - off < klen) rt_trap("rr index truncated key");
    entries[i].key = (bytes_t){blob.ptr + off, klen};
    off += klen;

    if (off > blob.len || blob.len - off < 4) rt_trap("rr index truncated ticks");
    entries[i].latency_ticks = rt_read_u32_le(blob.ptr + off);
    off += 4;

    if (off > blob.len || blob.len - off < 4) rt_trap("rr index truncated body_len");
    uint32_t blen = rt_read_u32_le(blob.ptr + off);
    off += 4;
    if (off > blob.len || blob.len - off < blen) rt_trap("rr index truncated body");
    entries[i].body_file = (bytes_t){blob.ptr + off, blen};
    off += blen;
  }
  if (off != blob.len) rt_trap("rr index trailing bytes");

  ctx->rr_index_default_latency_ticks = default_ticks;
  ctx->rr_index_entries = entries;
  ctx->rr_index_len = count;
  ctx->rr_index_blob = blob;
}

static rr_index_entry_t* rt_rr_index_find(ctx_t* ctx, bytes_view_t key) {
  rt_rr_index_load(ctx);
#ifdef X07_DEBUG_BORROW
  if (key.len != 0 && !rt_dbg_borrow_check(ctx, key.bid, key.off_bytes, key.len)) return NULL;
#endif
  for (uint32_t i = 0; i < ctx->rr_index_len; i++) {
    rr_index_entry_t* e = &ctx->rr_index_entries[i];
    if (e->key.len != key.len) continue;
    if (e->key.len == 0) return e;
    if (memcmp(e->key.ptr, key.ptr, e->key.len) == 0) return e;
  }
  return NULL;
}

static uint32_t rt_rr_latency_ticks(ctx_t* ctx, bytes_view_t key) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");
  rr_index_entry_t* e = rt_rr_index_find(ctx, key);
  if (!e) return ctx->rr_index_default_latency_ticks;
  return e->latency_ticks;
}

static bytes_t rt_rr_fetch_body(ctx_t* ctx, bytes_view_t key) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");
  ctx->rr_request_calls += 1;

  rr_index_entry_t* e = rt_rr_index_find(ctx, key);
  if (!e) return rt_bytes_empty(ctx);
  bytes_t body = e->body_file;
  if (body.len == 0) return rt_bytes_empty(ctx);
  if (!rt_fs_is_safe_rel_path(rt_bytes_view(ctx, body))) rt_trap("rr.fetch unsafe body path");
  if (memchr(body.ptr, 0, body.len) != NULL) rt_trap("rr.fetch body path contains nul");

  const uint32_t prefix_len = 8; // ".x07_rr/"
  uint32_t total = prefix_len + body.len;
  char* p = (char*)rt_alloc(ctx, total + 1, 1);
  memcpy(p, ".x07_rr/", prefix_len);
  rt_mem_on_memcpy(ctx, prefix_len);
  memcpy(p + prefix_len, body.ptr, body.len);
  rt_mem_on_memcpy(ctx, body.len);
  p[total] = 0;

  FILE* f = fopen(p, "rb");
  if (!f) {
    rt_free(ctx, p, total + 1, 1);
    rt_trap("rr.fetch open failed");
  }
  rt_free(ctx, p, total + 1, 1);
  if (fseek(f, 0, SEEK_END) != 0) rt_trap("rr.fetch seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("rr.fetch tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("rr.fetch body too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("rr.fetch seek failed");

  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)end);
  if (out.len != 0) {
    size_t got = fread(out.ptr, 1, out.len, f);
    if (got != out.len) rt_trap("rr.fetch short read");
  }
  fclose(f);
  return out;
}

static bytes_t rt_rr_fetch_block(ctx_t* ctx, bytes_view_t key) {
  uint32_t ticks = rt_rr_latency_ticks(ctx, key);
  if (ticks != 0) {
    rt_task_sleep_block(ctx, ticks);
  }
  return rt_rr_fetch_body(ctx, key);
}

static uint32_t rt_rr_send(ctx_t* ctx, bytes_view_t req) {
  if (!X07_ENABLE_RR) rt_trap("rr disabled");
  ctx->rr_request_calls += 1;

  rr_index_entry_t* e = rt_rr_index_find(ctx, req);
  uint32_t ticks = e ? e->latency_ticks : ctx->rr_index_default_latency_ticks;
  if (!e || e->body_file.len == 0) {
    return rt_io_reader_new_bytes(ctx, rt_bytes_empty(ctx), ticks);
  }

  bytes_t body = e->body_file;
  if (!rt_fs_is_safe_rel_path(rt_bytes_view(ctx, body))) rt_trap("rr.send unsafe body path");
  if (memchr(body.ptr, 0, body.len) != NULL) rt_trap("rr.send body path contains nul");

  const uint32_t prefix_len = 8; // ".x07_rr/"
  uint32_t total = prefix_len + body.len;
  char* p = (char*)rt_alloc(ctx, total + 1, 1);
  memcpy(p, ".x07_rr/", prefix_len);
  rt_mem_on_memcpy(ctx, prefix_len);
  memcpy(p + prefix_len, body.ptr, body.len);
  rt_mem_on_memcpy(ctx, body.len);
  p[total] = 0;

  FILE* f = fopen(p, "rb");
  if (!f) {
    rt_free(ctx, p, total + 1, 1);
    rt_trap("rr.send open failed");
  }
  rt_free(ctx, p, total + 1, 1);
  return rt_io_reader_new_file(ctx, f, ticks);
}
#endif

static uint32_t rt_kv_u32_le(const uint8_t* p) {
  return (uint32_t)p[0]
       | ((uint32_t)p[1] << 8)
       | ((uint32_t)p[2] << 16)
       | ((uint32_t)p[3] << 24);
}

#if X07_ENABLE_KV
static void rt_kv_ensure_cap(ctx_t* ctx, uint32_t need) {
  if (need <= ctx->kv_cap) return;
  kv_entry_t* old_items = ctx->kv_items;
  uint32_t old_cap = ctx->kv_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(kv_entry_t);
  uint32_t new_cap = ctx->kv_cap ? ctx->kv_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  kv_entry_t* items = (kv_entry_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(kv_entry_t),
    (uint32_t)_Alignof(kv_entry_t)
  );
  if (old_items && ctx->kv_len) {
    uint32_t bytes = ctx->kv_len * (uint32_t)sizeof(kv_entry_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(kv_entry_t));
  }
  ctx->kv_items = items;
  ctx->kv_cap = new_cap;
}

static uint32_t rt_kv_find(ctx_t* ctx, bytes_view_t key) {
#ifdef X07_DEBUG_BORROW
  if (key.len != 0 && !rt_dbg_borrow_check(ctx, key.bid, key.off_bytes, key.len)) {
    return UINT32_MAX;
  }
#endif
  for (uint32_t i = 0; i < ctx->kv_len; i++) {
    bytes_t k = ctx->kv_items[i].key;
    if (k.len != key.len) continue;
    if (k.len == 0) return i;
    if (memcmp(k.ptr, key.ptr, k.len) == 0) return i;
  }
  return UINT32_MAX;
}

static void rt_kv_init(ctx_t* ctx) {
  if (!X07_ENABLE_KV) return;

  FILE* f = fopen(".x07_kv/seed.evkv", "rb");
  if (!f) rt_trap("kv seed open failed");
  if (fseek(f, 0, SEEK_END) != 0) rt_trap("kv seed seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("kv seed tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("kv seed too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("kv seed seek failed");

  bytes_t seed = rt_bytes_alloc(ctx, (uint32_t)end);
  if (seed.len != 0) {
    size_t got = fread(seed.ptr, 1, seed.len, f);
    if (got != seed.len) rt_trap("kv seed short read");
  }
  fclose(f);

  if (seed.len < 10) rt_trap("kv seed too short");
  if (memcmp(seed.ptr, "X7KV", 4) != 0) rt_trap("kv seed bad magic");
  uint32_t ver = (uint32_t)seed.ptr[4] | ((uint32_t)seed.ptr[5] << 8);
  if (ver != 1) rt_trap("kv seed bad version");

  uint32_t count = rt_kv_u32_le(seed.ptr + 6);
  ctx->kv_items = NULL;
  ctx->kv_len = 0;
  ctx->kv_cap = 0;
  if (count != 0) {
    ctx->kv_items = (kv_entry_t*)rt_alloc(
      ctx,
      count * (uint32_t)sizeof(kv_entry_t),
      (uint32_t)_Alignof(kv_entry_t)
    );
    ctx->kv_cap = count;
  }

  uint32_t off = 10;
  for (uint32_t i = 0; i < count; i++) {
    if (off > seed.len || seed.len - off < 4) rt_trap("kv seed truncated klen");
    uint32_t klen = rt_kv_u32_le(seed.ptr + off);
    off += 4;
    if (off > seed.len || seed.len - off < klen) rt_trap("kv seed truncated key");
    bytes_t key = rt_bytes_alloc(ctx, klen);
    if (klen) {
      memcpy(key.ptr, seed.ptr + off, klen);
      rt_mem_on_memcpy(ctx, klen);
    }
    off += klen;

    if (off > seed.len || seed.len - off < 4) rt_trap("kv seed truncated vlen");
    uint32_t vlen = rt_kv_u32_le(seed.ptr + off);
    off += 4;
    if (off > seed.len || seed.len - off < vlen) rt_trap("kv seed truncated value");
    bytes_t val = rt_bytes_alloc(ctx, vlen);
    if (vlen) {
      memcpy(val.ptr, seed.ptr + off, vlen);
      rt_mem_on_memcpy(ctx, vlen);
    }
    off += vlen;

    ctx->kv_items[ctx->kv_len++] = (kv_entry_t){key, val};
  }
  if (off != seed.len) rt_trap("kv seed trailing bytes");
  rt_bytes_drop(ctx, &seed);
}

static void rt_kv_latency_load(ctx_t* ctx) {
  if (ctx->kv_latency_loaded) return;
  ctx->kv_latency_loaded = 1;
  ctx->kv_latency_default_ticks = 0;
  ctx->kv_latency_entries = NULL;
  ctx->kv_latency_len = 0;
  ctx->kv_latency_blob = rt_bytes_empty(ctx);

  FILE* f = fopen(".x07_kv/latency.evkvlat", "rb");
  if (!f) return;
  if (fseek(f, 0, SEEK_END) != 0) rt_trap("kv latency seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("kv latency tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("kv latency too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("kv latency seek failed");

  bytes_t blob = rt_bytes_alloc(ctx, (uint32_t)end);
  if (blob.len != 0) {
    size_t got = fread(blob.ptr, 1, blob.len, f);
    if (got != blob.len) rt_trap("kv latency short read");
  }
  fclose(f);

  if (blob.len < 16) rt_trap("kv latency too short");
  if (memcmp(blob.ptr, "X7KL", 4) != 0) rt_trap("kv latency bad magic");
  uint16_t ver = rt_read_u16_le(blob.ptr + 4);
  if (ver != 1) rt_trap("kv latency bad version");

  uint32_t default_ticks = rt_read_u32_le(blob.ptr + 8);
  uint32_t count = rt_read_u32_le(blob.ptr + 12);

  kv_latency_entry_t* entries = NULL;
  if (count != 0) {
    entries = (kv_latency_entry_t*)rt_alloc(
      ctx,
      count * (uint32_t)sizeof(kv_latency_entry_t),
      (uint32_t)_Alignof(kv_latency_entry_t)
    );
  }

  uint32_t off = 16;
  for (uint32_t i = 0; i < count; i++) {
    if (off > blob.len || blob.len - off < 4) rt_trap("kv latency truncated key_len");
    uint32_t klen = rt_read_u32_le(blob.ptr + off);
    off += 4;
    if (off > blob.len || blob.len - off < klen) rt_trap("kv latency truncated key");
    entries[i].key = (bytes_t){blob.ptr + off, klen};
    off += klen;
    if (off > blob.len || blob.len - off < 4) rt_trap("kv latency truncated ticks");
    entries[i].ticks = rt_read_u32_le(blob.ptr + off);
    off += 4;
  }
  if (off != blob.len) rt_trap("kv latency trailing bytes");

  ctx->kv_latency_default_ticks = default_ticks;
  ctx->kv_latency_entries = entries;
  ctx->kv_latency_len = count;
  ctx->kv_latency_blob = blob;
}

static uint32_t rt_kv_latency_ticks(ctx_t* ctx, bytes_view_t key) {
  if (!X07_ENABLE_KV) rt_trap("kv disabled");
  rt_kv_latency_load(ctx);
#ifdef X07_DEBUG_BORROW
  if (key.len != 0 && !rt_dbg_borrow_check(ctx, key.bid, key.off_bytes, key.len)) {
    return ctx->kv_latency_default_ticks;
  }
#endif
  for (uint32_t i = 0; i < ctx->kv_latency_len; i++) {
    bytes_t k = ctx->kv_latency_entries[i].key;
    if (k.len != key.len) continue;
    if (k.len == 0) return ctx->kv_latency_entries[i].ticks;
    if (memcmp(k.ptr, key.ptr, k.len) == 0) return ctx->kv_latency_entries[i].ticks;
  }
  return ctx->kv_latency_default_ticks;
}

static bytes_t rt_kv_get(ctx_t* ctx, bytes_view_t key) {
  if (!X07_ENABLE_KV) rt_trap("kv disabled");
  ctx->kv_get_calls += 1;
  uint32_t idx = rt_kv_find(ctx, key);
  if (idx == UINT32_MAX) return rt_bytes_empty(ctx);
  return rt_bytes_clone(ctx, ctx->kv_items[idx].val);
}

static bytes_t rt_kv_get_async_block(ctx_t* ctx, bytes_view_t key) {
  uint32_t ticks = rt_kv_latency_ticks(ctx, key);
  if (ticks != 0) {
    rt_task_sleep_block(ctx, ticks);
  }
  return rt_kv_get(ctx, key);
}

static uint32_t rt_kv_get_stream(ctx_t* ctx, bytes_view_t key) {
  if (!X07_ENABLE_KV) rt_trap("kv disabled");
  ctx->kv_get_calls += 1;
  uint32_t idx = rt_kv_find(ctx, key);
  bytes_t val =
      (idx == UINT32_MAX) ? rt_bytes_empty(ctx) : rt_bytes_clone(ctx, ctx->kv_items[idx].val);
  uint32_t ticks = rt_kv_latency_ticks(ctx, key);
  return rt_io_reader_new_bytes(ctx, val, ticks);
}

static uint32_t rt_kv_set(ctx_t* ctx, bytes_t key, bytes_t val) {
  if (!X07_ENABLE_KV) rt_trap("kv disabled");
  ctx->kv_set_calls += 1;

  uint32_t idx = rt_kv_find(ctx, rt_bytes_view(ctx, key));
  if (idx != UINT32_MAX) {
    rt_bytes_drop(ctx, &key);
    rt_bytes_drop(ctx, &ctx->kv_items[idx].val);
    ctx->kv_items[idx].val = val;
    return UINT32_C(0);
  }

  rt_kv_ensure_cap(ctx, ctx->kv_len + 1);
  ctx->kv_items[ctx->kv_len++] = (kv_entry_t){key, val};
  return UINT32_C(1);
}
#else
static void rt_kv_init(ctx_t* ctx) {
  (void)ctx;
}
#endif

static uint32_t rt_codec_read_u32_le(ctx_t* ctx, bytes_view_t buf, uint32_t offset) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, buf.bid, buf.off_bytes, buf.len);
#else
  (void)ctx;
#endif
  if (offset > buf.len || buf.len - offset < 4) rt_trap("codec.read_u32_le oob");
  return (uint32_t)buf.ptr[offset]
       | ((uint32_t)buf.ptr[offset + 1] << 8)
       | ((uint32_t)buf.ptr[offset + 2] << 16)
       | ((uint32_t)buf.ptr[offset + 3] << 24);
}

static bytes_t rt_codec_write_u32_le(ctx_t* ctx, uint32_t x) {
  bytes_t out = rt_bytes_alloc(ctx, 4);
  out.ptr[0] = (uint8_t)(x & UINT32_C(0xFF));
  out.ptr[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  out.ptr[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  out.ptr[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
  return out;
}

static bytes_t rt_fmt_u32_to_dec(ctx_t* ctx, uint32_t x) {
  uint8_t scratch[16];
  uint32_t n = 0;
  if (x == 0) {
    bytes_t out = rt_bytes_alloc(ctx, 1);
    out.ptr[0] = (uint8_t)'0';
    return out;
  }
  while (x > 0) {
    uint32_t digit = x % 10;
    x /= 10;
    scratch[n++] = (uint8_t)('0' + digit);
  }
  bytes_t out = rt_bytes_alloc(ctx, n);
  for (uint32_t i = 0; i < n; i++) {
    out.ptr[i] = scratch[n - 1 - i];
  }
  return out;
}

static bytes_t rt_fmt_s32_to_dec(ctx_t* ctx, uint32_t x) {
  if ((x & UINT32_C(0x80000000)) == 0) {
    return rt_fmt_u32_to_dec(ctx, x);
  }
  uint32_t mag = (~x) + UINT32_C(1);
  bytes_t digits = rt_fmt_u32_to_dec(ctx, mag);
  bytes_t out = rt_bytes_alloc(ctx, digits.len + 1);
  out.ptr[0] = (uint8_t)'-';
  memcpy(out.ptr + 1, digits.ptr, digits.len);
  rt_mem_on_memcpy(ctx, digits.len);
  return out;
}

static uint32_t rt_parse_u32_dec_slice(ctx_t* ctx, uint8_t* ptr, uint32_t len) {
  if (len == 0) rt_trap("parse.u32_dec empty");
  uint32_t acc = 0;
  for (uint32_t i = 0; i < len; i++) {
    uint8_t b = ptr[i];
    if (b < (uint8_t)'0' || b > (uint8_t)'9') rt_trap("parse.u32_dec non-digit");
    uint32_t digit = (uint32_t)(b - (uint8_t)'0');
    if (acc > (UINT32_MAX - digit) / 10) rt_trap("parse.u32_dec overflow");
    acc = acc * 10 + digit;
  }
  return acc;
}

static uint32_t rt_parse_u32_dec(ctx_t* ctx, bytes_view_t buf) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, buf.bid, buf.off_bytes, buf.len);
#endif
  return rt_parse_u32_dec_slice(ctx, buf.ptr, buf.len);
}

static uint32_t rt_parse_u32_dec_at(ctx_t* ctx, bytes_view_t buf, uint32_t offset) {
  if (offset > buf.len) rt_trap("parse.u32_dec_at oob");
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, buf.bid, buf.off_bytes + offset, buf.len - offset);
#endif
  return rt_parse_u32_dec_slice(ctx, buf.ptr + offset, buf.len - offset);
}

static uint32_t rt_prng_lcg_next_u32(uint32_t state) {
  return state * UINT32_C(1103515245) + UINT32_C(12345);
}

typedef struct {
  uint8_t* data;
  uint32_t len;
  uint32_t cap;
#ifdef X07_DEBUG_BORROW
  uint64_t dbg_aid;
#endif
} vec_u8_t;

static vec_u8_t rt_vec_u8_new(ctx_t* ctx, uint32_t cap) {
  vec_u8_t v;
  v.len = 0;
  v.cap = cap;
  v.data = (cap == 0) ? ctx->heap.mem : (uint8_t*)rt_alloc(ctx, cap, 1);
#ifdef X07_DEBUG_BORROW
  v.dbg_aid = (cap == 0) ? 0 : rt_dbg_alloc_register(ctx, v.data, cap);
#endif
  return v;
}

static void rt_vec_u8_drop(ctx_t* ctx, vec_u8_t* v) {
  if (!v) return;
  if (v->cap == 0) {
    v->data = ctx->heap.mem;
    v->len = 0;
    return;
  }
#ifdef X07_DEBUG_BORROW
  rt_dbg_alloc_kill(ctx, v->dbg_aid);
  v->dbg_aid = 0;
#endif
  rt_free(ctx, v->data, v->cap, 1);
  v->data = ctx->heap.mem;
  v->len = 0;
  v->cap = 0;
}

static uint32_t rt_vec_u8_len(ctx_t* ctx, vec_u8_t v) {
  (void)ctx;
  return v.len;
}

static uint32_t rt_vec_u8_get(ctx_t* ctx, vec_u8_t v, uint32_t idx) {
  (void)ctx;
  if (idx >= v.len) rt_trap("vec_u8.get oob");
  return (uint32_t)v.data[idx];
}

static vec_u8_t rt_vec_u8_set(ctx_t* ctx, vec_u8_t v, uint32_t idx, uint32_t val) {
  (void)ctx;
  if (idx >= v.len) rt_trap("vec_u8.set oob");
  v.data[idx] = (uint8_t)(val & UINT32_C(0xFF));
  return v;
}

static vec_u8_t rt_vec_u8_push(ctx_t* ctx, vec_u8_t v, uint32_t val) {
  if (v.len == v.cap) {
    uint8_t* old_data = v.cap ? v.data : NULL;
    uint32_t old_cap = v.cap;
    uint32_t new_cap = v.cap ? (v.cap * 2) : 1;
    uint8_t* data = (uint8_t*)rt_alloc_realloc(
        ctx,
        old_data,
        old_cap,
        new_cap,
        1
    );
    if (v.data && v.len) {
      memcpy(data, v.data, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
#ifdef X07_DEBUG_BORROW
    rt_dbg_alloc_kill(ctx, v.dbg_aid);
    v.dbg_aid = rt_dbg_alloc_register(ctx, data, new_cap);
#endif
    if (old_data && old_cap) {
      rt_free(ctx, old_data, old_cap, 1);
    }
    v.data = data;
    v.cap = new_cap;
  }
  v.data[v.len++] = (uint8_t)(val & UINT32_C(0xFF));
  return v;
}

static vec_u8_t rt_vec_u8_reserve_exact(ctx_t* ctx, vec_u8_t v, uint32_t additional) {
  if (additional > UINT32_MAX - v.len) rt_trap("vec_u8.reserve_exact overflow");
  uint32_t need = v.len + additional;
  if (need <= v.cap) return v;

  uint8_t* old_data = v.cap ? v.data : NULL;
  uint32_t old_cap = v.cap;
  uint8_t* data = (uint8_t*)rt_alloc_realloc(
      ctx,
      old_data,
      old_cap,
      need,
      1
  );
  if (v.data && v.len) {
    memcpy(data, v.data, v.len);
    rt_mem_on_memcpy(ctx, v.len);
  }
#ifdef X07_DEBUG_BORROW
  rt_dbg_alloc_kill(ctx, v.dbg_aid);
  v.dbg_aid = rt_dbg_alloc_register(ctx, data, need);
#endif
  if (old_data && old_cap) {
    rt_free(ctx, old_data, old_cap, 1);
  }
  v.data = data;
  v.cap = need;
  return v;
}

static vec_u8_t rt_vec_u8_extend_zeroes(ctx_t* ctx, vec_u8_t v, uint32_t n) {
  if (n > UINT32_MAX - v.len) rt_trap("vec_u8.extend_zeroes overflow");
  uint32_t need = v.len + n;
  if (need > v.cap) {
    uint8_t* old_data = v.cap ? v.data : NULL;
    uint32_t old_cap = v.cap;
    uint32_t new_cap = v.cap ? v.cap : 1;
    while (new_cap < need) {
      if (new_cap > UINT32_MAX / 2) {
        new_cap = need;
        break;
      }
      new_cap *= 2;
    }

    uint8_t* data = (uint8_t*)rt_alloc_realloc(
        ctx,
        old_data,
        old_cap,
        new_cap,
        1
    );
    if (v.data && v.len) {
      memcpy(data, v.data, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
#ifdef X07_DEBUG_BORROW
    rt_dbg_alloc_kill(ctx, v.dbg_aid);
    v.dbg_aid = rt_dbg_alloc_register(ctx, data, new_cap);
#endif
    if (old_data && old_cap) {
      rt_free(ctx, old_data, old_cap, 1);
    }
    v.data = data;
    v.cap = new_cap;
  }

  if (n) {
    memset(v.data + v.len, 0, n);
  }
  v.len += n;
  return v;
}

static vec_u8_t rt_vec_u8_extend_bytes(ctx_t* ctx, vec_u8_t v, bytes_view_t b) {
#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_borrow_check(ctx, b.bid, b.off_bytes, b.len);
#endif
  if (b.len > UINT32_MAX - v.len) rt_trap("vec_u8.extend_bytes overflow");
  uint32_t need = v.len + b.len;
  if (need > v.cap) {
    uint8_t* old_data = v.cap ? v.data : NULL;
    uint32_t old_cap = v.cap;
    uint32_t new_cap = v.cap ? v.cap : 1;
    while (new_cap < need) {
      if (new_cap > UINT32_MAX / 2) {
        new_cap = need;
        break;
      }
      new_cap *= 2;
    }

    uint8_t* data = (uint8_t*)rt_alloc_realloc(
        ctx,
        old_data,
        old_cap,
        new_cap,
        1
    );
    if (v.data && v.len) {
      memcpy(data, v.data, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
#ifdef X07_DEBUG_BORROW
    rt_dbg_alloc_kill(ctx, v.dbg_aid);
    v.dbg_aid = rt_dbg_alloc_register(ctx, data, new_cap);
#endif
    if (old_data && old_cap) {
      rt_free(ctx, old_data, old_cap, 1);
    }
    v.data = data;
    v.cap = new_cap;
  }

  if (b.len) {
    memcpy(v.data + v.len, b.ptr, b.len);
    rt_mem_on_memcpy(ctx, b.len);
  }
  v.len += b.len;
  return v;
}

static vec_u8_t rt_vec_u8_extend_bytes_range(
    ctx_t* ctx,
    vec_u8_t v,
    bytes_view_t b,
    uint32_t start,
    uint32_t len
) {
  if (start > b.len || b.len - start < len) rt_trap("vec_u8.extend_bytes_range oob");
  bytes_view_t sub;
  sub.ptr = b.ptr + start;
  sub.len = len;
#ifdef X07_DEBUG_BORROW
  sub.aid = b.aid;
  sub.bid = b.bid;
  if (UINT32_MAX - b.off_bytes < start) rt_trap("vec_u8.extend_bytes_range off overflow");
  sub.off_bytes = b.off_bytes + start;
#endif
  return rt_vec_u8_extend_bytes(ctx, v, sub);
}

static bytes_view_t rt_vec_u8_as_view(ctx_t* ctx, vec_u8_t v) {
  bytes_view_t out;
  out.len = v.len;
#ifdef X07_DEBUG_BORROW
  if (out.len == 0) {
    out.ptr = ctx->heap.mem;
    out.aid = 0;
    out.bid = 0;
    out.off_bytes = 0;
    return out;
  }
  out.ptr = v.data;
  out.aid = v.dbg_aid;
  out.off_bytes = 0;
  out.bid = rt_dbg_alloc_borrow_id(ctx, out.aid);
#else
  out.ptr = (out.len == 0) ? ctx->heap.mem : v.data;
#endif
  return out;
}

static bytes_t rt_vec_u8_into_bytes(ctx_t* ctx, vec_u8_t* v) {
  if (!v) return rt_bytes_empty(ctx);
  if (v->len == 0) {
    rt_vec_u8_drop(ctx, v);
    return rt_bytes_empty(ctx);
  }

  bytes_t out;
  out.ptr = v->data;
  out.len = v->len;

  v->data = ctx->heap.mem;
  v->len = 0;
  v->cap = 0;
#ifdef X07_DEBUG_BORROW
  v->dbg_aid = 0;
#endif
  return out;
}

typedef struct {
  uint32_t cap;
  uint32_t len;
  uint32_t* keys;
  uint32_t* vals;
} map_u32_t;

static map_u32_t* rt_map_u32_ptr(ctx_t* ctx, uint32_t handle) {
  if (handle == 0 || handle > ctx->map_u32_len) rt_trap("map_u32 invalid handle");
  map_u32_t* m = (map_u32_t*)ctx->map_u32_items[handle - 1];
  if (!m) rt_trap("map_u32 invalid handle");
  return m;
}

static uint32_t rt_is_pow2_u32(uint32_t x) {
  return (x != 0) && ((x & (x - 1)) == 0);
}

static uint32_t rt_map_u32_new(ctx_t* ctx, uint32_t cap) {
  if (!rt_is_pow2_u32(cap)) rt_trap("map_u32.new cap must be power of two");
  if (ctx->map_u32_len == ctx->map_u32_cap) {
    void** old_items = ctx->map_u32_items;
    uint32_t old_cap = ctx->map_u32_cap;
    uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(void*);
    uint32_t new_cap = ctx->map_u32_cap ? (ctx->map_u32_cap * 2) : 8;
    void** items = (void**)rt_alloc_realloc(
      ctx,
      old_items,
      old_bytes_total,
      new_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
    if (old_items && ctx->map_u32_len) {
      uint32_t bytes = ctx->map_u32_len * (uint32_t)sizeof(void*);
      memcpy(items, old_items, bytes);
      rt_mem_on_memcpy(ctx, bytes);
    }
    if (old_items && old_bytes_total) {
      rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(void*));
    }
    ctx->map_u32_items = items;
    ctx->map_u32_cap = new_cap;
  }
  map_u32_t* m = (map_u32_t*)rt_alloc(ctx, (uint32_t)sizeof(map_u32_t), (uint32_t)_Alignof(map_u32_t));
  m->cap = cap;
  m->len = 0;
  m->keys = (uint32_t*)rt_alloc(ctx, cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  m->vals = (uint32_t*)rt_alloc(ctx, cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  memset(m->keys, 0xFF, cap * (uint32_t)sizeof(uint32_t));
  ctx->map_u32_items[ctx->map_u32_len++] = m;
  return ctx->map_u32_len;
}

static uint32_t rt_map_u32_len(ctx_t* ctx, uint32_t handle) {
  return rt_map_u32_ptr(ctx, handle)->len;
}

static uint32_t rt_map_u32_hash(uint32_t key) {
  return key * UINT32_C(2654435769);
}

static uint32_t rt_map_u32_get(ctx_t* ctx, uint32_t handle, uint32_t key, uint32_t default_) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.get key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) return m->vals[idx];
    if (slot == UINT32_C(0xFFFFFFFF)) return default_;
    idx = (idx + 1) & mask;
    if (idx == start) return default_;
  }
}

static uint32_t rt_map_u32_set(ctx_t* ctx, uint32_t handle, uint32_t key, uint32_t val) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.set key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) {
      m->vals[idx] = val;
      return UINT32_C(0);
    }
    if (slot == UINT32_C(0xFFFFFFFF)) {
      m->keys[idx] = key;
      m->vals[idx] = val;
      m->len += 1;
      return UINT32_C(1);
    }
    idx = (idx + 1) & mask;
    if (idx == start) rt_trap("map_u32 full");
  }
}

static uint32_t rt_map_u32_contains(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.contains key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) return UINT32_C(1);
    if (slot == UINT32_C(0xFFFFFFFF)) return UINT32_C(0);
    idx = (idx + 1) & mask;
    if (idx == start) return UINT32_C(0);
  }
}

static uint32_t rt_map_u32_remove(ctx_t* ctx, uint32_t handle, uint32_t key) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (key == UINT32_C(0xFFFFFFFF)) rt_trap("map_u32.remove key=-1 reserved");
  uint32_t mask = m->cap - 1;
  uint32_t idx = rt_map_u32_hash(key) & mask;
  uint32_t start = idx;
  for (;;) {
    uint32_t slot = m->keys[idx];
    if (slot == key) break;
    if (slot == UINT32_C(0xFFFFFFFF)) return UINT32_C(0);
    idx = (idx + 1) & mask;
    if (idx == start) return UINT32_C(0);
  }

  m->keys[idx] = UINT32_C(0xFFFFFFFF);
  m->vals[idx] = 0;
  m->len -= 1;

  uint32_t j = (idx + 1) & mask;
  while (m->keys[j] != UINT32_C(0xFFFFFFFF)) {
    uint32_t k = m->keys[j];
    uint32_t v = m->vals[j];
    m->keys[j] = UINT32_C(0xFFFFFFFF);
    m->vals[j] = 0;
    m->len -= 1;
    (void)rt_map_u32_set(ctx, handle, k, v);
    j = (j + 1) & mask;
  }

  return UINT32_C(1);
}

static inline void rt_store_u32_le(uint8_t* out, uint32_t x) {
  out[0] = (uint8_t)(x & UINT32_C(0xFF));
  out[1] = (uint8_t)((x >> 8) & UINT32_C(0xFF));
  out[2] = (uint8_t)((x >> 16) & UINT32_C(0xFF));
  out[3] = (uint8_t)((x >> 24) & UINT32_C(0xFF));
}

static bytes_t rt_set_u32_dump_u32le(ctx_t* ctx, uint32_t handle) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (m->len == 0) return rt_bytes_empty(ctx);
  if (m->len > UINT32_MAX / UINT32_C(4)) rt_trap("set_u32.dump_u32le overflow");
  bytes_t out = rt_bytes_alloc(ctx, m->len * UINT32_C(4));
  uint32_t off = 0;
  for (uint32_t i = 0; i < m->cap; i++) {
    uint32_t k = m->keys[i];
    if (k == UINT32_C(0xFFFFFFFF)) continue;
    if (off > out.len - UINT32_C(4)) rt_trap("set_u32.dump_u32le len mismatch");
    rt_store_u32_le(out.ptr + off, k);
    off += UINT32_C(4);
  }
  if (off != out.len) rt_trap("set_u32.dump_u32le len mismatch");
  return out;
}

static bytes_t rt_map_u32_dump_kv_u32le_u32le(ctx_t* ctx, uint32_t handle) {
  map_u32_t* m = rt_map_u32_ptr(ctx, handle);
  if (m->len == 0) return rt_bytes_empty(ctx);
  if (m->len > UINT32_MAX / UINT32_C(8)) rt_trap("map_u32.dump_kv_u32le_u32le overflow");
  bytes_t out = rt_bytes_alloc(ctx, m->len * UINT32_C(8));
  uint32_t off = 0;
  for (uint32_t i = 0; i < m->cap; i++) {
    uint32_t k = m->keys[i];
    if (k == UINT32_C(0xFFFFFFFF)) continue;
    if (off > out.len - UINT32_C(8)) rt_trap("map_u32.dump_kv_u32le_u32le len mismatch");
    rt_store_u32_le(out.ptr + off, k);
    rt_store_u32_le(out.ptr + off + UINT32_C(4), m->vals[i]);
    off += UINT32_C(8);
  }
  if (off != out.len) rt_trap("map_u32.dump_kv_u32le_u32le len mismatch");
  return out;
}

static void rt_ctx_cleanup(ctx_t* ctx) {
#ifdef X07_DEBUG_BORROW
  if (ctx->dbg_borrows && ctx->dbg_borrows_cap) {
    rt_free(
      ctx,
      ctx->dbg_borrows,
      ctx->dbg_borrows_cap * (uint32_t)sizeof(dbg_borrow_rec_t),
      (uint32_t)_Alignof(dbg_borrow_rec_t)
    );
  }
  if (ctx->dbg_allocs && ctx->dbg_allocs_cap) {
    rt_free(
      ctx,
      ctx->dbg_allocs,
      ctx->dbg_allocs_cap * (uint32_t)sizeof(dbg_alloc_rec_t),
      (uint32_t)_Alignof(dbg_alloc_rec_t)
    );
  }
  ctx->dbg_borrows = NULL;
  ctx->dbg_borrows_len = 0;
  ctx->dbg_borrows_cap = 0;
  ctx->dbg_allocs = NULL;
  ctx->dbg_allocs_len = 0;
  ctx->dbg_allocs_cap = 0;
#endif

  rt_os_process_cleanup(ctx);

  for (uint32_t i = 0; i < ctx->bufreads_len; i++) {
    rt_bufread_t* br = &ctx->bufreads[i];
    if (!br->alive) continue;
    rt_bytes_drop(ctx, &br->buf);
    br->buf = rt_bytes_empty(ctx);
    br->alive = 0;
  }
  if (ctx->bufreads && ctx->bufreads_cap) {
    rt_free(
      ctx,
      ctx->bufreads,
      ctx->bufreads_cap * (uint32_t)sizeof(rt_bufread_t),
      (uint32_t)_Alignof(rt_bufread_t)
    );
  }
  ctx->bufreads = NULL;
  ctx->bufreads_len = 0;
  ctx->bufreads_cap = 0;

  for (uint32_t i = 0; i < ctx->io_readers_len; i++) {
    rt_io_reader_t* r = &ctx->io_readers[i];
    if (!r->alive) continue;
#if X07_ENABLE_STREAMING_FILE_IO
    if (r->kind == RT_IO_READER_KIND_FILE && r->f) {
      fclose(r->f);
      r->f = NULL;
    }
#endif
    if (r->kind == RT_IO_READER_KIND_BYTES) {
      rt_bytes_drop(ctx, &r->bytes);
      r->bytes = rt_bytes_empty(ctx);
    }
    r->alive = 0;
  }
  if (ctx->io_readers && ctx->io_readers_cap) {
    rt_free(
      ctx,
      ctx->io_readers,
      ctx->io_readers_cap * (uint32_t)sizeof(rt_io_reader_t),
      (uint32_t)_Alignof(rt_io_reader_t)
    );
  }
  ctx->io_readers = NULL;
  ctx->io_readers_len = 0;
  ctx->io_readers_cap = 0;

  for (uint32_t i = 0; i < ctx->sched_chans_len; i++) {
    rt_chan_bytes_t* c = &ctx->sched_chans[i];
    if (!c->alive) continue;
    for (uint32_t j = 0; j < c->len; j++) {
      uint32_t idx = (c->head + j) % c->cap;
      rt_bytes_drop(ctx, &c->buf[idx]);
    }
    if (c->buf && c->cap) {
      rt_free(ctx, c->buf, c->cap * (uint32_t)sizeof(bytes_t), (uint32_t)_Alignof(bytes_t));
    }
    c->buf = NULL;
    c->alive = 0;
  }
  if (ctx->sched_chans && ctx->sched_chans_cap) {
    rt_free(
      ctx,
      ctx->sched_chans,
      ctx->sched_chans_cap * (uint32_t)sizeof(rt_chan_bytes_t),
      (uint32_t)_Alignof(rt_chan_bytes_t)
    );
  }
  ctx->sched_chans = NULL;
  ctx->sched_chans_len = 0;
  ctx->sched_chans_cap = 0;

  if (ctx->sched_timers && ctx->sched_timers_cap) {
    rt_free(
      ctx,
      ctx->sched_timers,
      ctx->sched_timers_cap * (uint32_t)sizeof(rt_timer_ev_t),
      (uint32_t)_Alignof(rt_timer_ev_t)
    );
  }
  ctx->sched_timers = NULL;
  ctx->sched_timers_len = 0;
  ctx->sched_timers_cap = 0;

  for (uint32_t i = 0; i < ctx->sched_tasks_len; i++) {
    rt_task_t* t = &ctx->sched_tasks[i];
    if (!t->alive) continue;
    if (!t->done) t->canceled = 1;
    t->done = 1;
    if (t->drop && t->fut) {
      t->drop(ctx, t->fut);
    }
    t->drop = NULL;
    t->fut = NULL;
    rt_bytes_drop(ctx, &t->out);
    t->out = rt_bytes_empty(ctx);
    t->out_taken = 0;
    t->alive = 0;
  }
  if (ctx->sched_tasks && ctx->sched_tasks_cap) {
    rt_free(
      ctx,
      ctx->sched_tasks,
      ctx->sched_tasks_cap * (uint32_t)sizeof(rt_task_t),
      (uint32_t)_Alignof(rt_task_t)
    );
  }
  ctx->sched_tasks = NULL;
  ctx->sched_tasks_len = 0;
  ctx->sched_tasks_cap = 0;
  ctx->sched_ready_head = 0;
  ctx->sched_ready_tail = 0;

  if (ctx->fs_latency_entries && ctx->fs_latency_len) {
    rt_free(
      ctx,
      ctx->fs_latency_entries,
      ctx->fs_latency_len * (uint32_t)sizeof(fs_latency_entry_t),
      (uint32_t)_Alignof(fs_latency_entry_t)
    );
  }
  ctx->fs_latency_entries = NULL;
  ctx->fs_latency_len = 0;
  rt_bytes_drop(ctx, &ctx->fs_latency_blob);
  ctx->fs_latency_blob = rt_bytes_empty(ctx);

  if (ctx->rr_index_entries && ctx->rr_index_len) {
    rt_free(
      ctx,
      ctx->rr_index_entries,
      ctx->rr_index_len * (uint32_t)sizeof(rr_index_entry_t),
      (uint32_t)_Alignof(rr_index_entry_t)
    );
  }
  ctx->rr_index_entries = NULL;
  ctx->rr_index_len = 0;
  rt_bytes_drop(ctx, &ctx->rr_index_blob);
  ctx->rr_index_blob = rt_bytes_empty(ctx);

  if (ctx->kv_latency_entries && ctx->kv_latency_len) {
    rt_free(
      ctx,
      ctx->kv_latency_entries,
      ctx->kv_latency_len * (uint32_t)sizeof(kv_latency_entry_t),
      (uint32_t)_Alignof(kv_latency_entry_t)
    );
  }
  ctx->kv_latency_entries = NULL;
  ctx->kv_latency_len = 0;
  rt_bytes_drop(ctx, &ctx->kv_latency_blob);
  ctx->kv_latency_blob = rt_bytes_empty(ctx);

#if X07_ENABLE_KV
  for (uint32_t i = 0; i < ctx->kv_len; i++) {
    rt_bytes_drop(ctx, &ctx->kv_items[i].key);
    rt_bytes_drop(ctx, &ctx->kv_items[i].val);
  }
  if (ctx->kv_items && ctx->kv_cap) {
    rt_free(
      ctx,
      ctx->kv_items,
      ctx->kv_cap * (uint32_t)sizeof(kv_entry_t),
      (uint32_t)_Alignof(kv_entry_t)
    );
  }
#endif
  ctx->kv_items = NULL;
  ctx->kv_len = 0;
  ctx->kv_cap = 0;

  for (uint32_t i = 0; i < ctx->map_u32_len; i++) {
    map_u32_t* m = (map_u32_t*)ctx->map_u32_items[i];
    if (!m) continue;
    if (m->keys && m->cap) {
      rt_free(ctx, m->keys, m->cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
    }
    if (m->vals && m->cap) {
      rt_free(ctx, m->vals, m->cap * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
    }
    rt_free(ctx, m, (uint32_t)sizeof(map_u32_t), (uint32_t)_Alignof(map_u32_t));
  }
  if (ctx->map_u32_items && ctx->map_u32_cap) {
    rt_free(
      ctx,
      ctx->map_u32_items,
      ctx->map_u32_cap * (uint32_t)sizeof(void*),
      (uint32_t)_Alignof(void*)
    );
  }
  ctx->map_u32_items = NULL;
  ctx->map_u32_len = 0;
  ctx->map_u32_cap = 0;
}
"#;

const RUNTIME_C_PURE_STUBS: &str = r#"
static void rt_kv_init(ctx_t* ctx) {
  (void)ctx;
}

static void rt_hex_bytes(const uint8_t* bytes, uint32_t len, char* out) {
  static const char LUT[16] = "0123456789abcdef";
  for (uint32_t i = 0; i < len; i++) {
    uint8_t b = bytes[i];
    out[i * 2 + 0] = LUT[b >> 4];
    out[i * 2 + 1] = LUT[b & 0x0F];
  }
  out[len * 2] = 0;
}
"#;

const RUNTIME_C_OS: &str = r#"
// Standalone OS runtime helpers (Phase H3).
//
// These helpers are only compiled into binaries produced for standalone-only
// worlds (run-os, run-os-sandboxed).

static uint32_t rt_os_policy_inited = 0;
static uint32_t rt_os_sandboxed = 0;

static uint32_t rt_os_fs_enabled = 1;
static uint32_t rt_os_net_enabled = 1;
static uint32_t rt_os_env_enabled = 1;
static uint32_t rt_os_time_enabled = 1;
static uint32_t rt_os_proc_enabled = 1;

static uint32_t rt_os_deny_hidden = 0;
static const char* rt_os_fs_read_roots = NULL;
static const char* rt_os_fs_write_roots = NULL;

static const char* rt_os_env_allow_keys = NULL;
static const char* rt_os_env_deny_keys = NULL;

static uint32_t rt_os_time_allow_wall_clock = 1;
static uint32_t rt_os_time_allow_monotonic = 1;
static uint32_t rt_os_time_allow_sleep = 1;
static uint32_t rt_os_time_max_sleep_ms = 0;
static uint32_t rt_os_time_allow_local_tzid = 1;
static uint32_t rt_os_proc_allow_exit = 1;
static uint32_t rt_os_proc_allow_spawn = 1;
static uint32_t rt_os_proc_allow_exec = 1;
static const char* rt_os_proc_allow_execs = NULL;
static const char* rt_os_proc_allow_exec_prefixes = NULL;
static const char* rt_os_proc_allow_args_regex_lite = NULL;
static const char* rt_os_proc_allow_env_keys = NULL;
static uint32_t rt_os_proc_max_live = 0;
static uint32_t rt_os_proc_max_spawns = 0;
static uint32_t rt_os_proc_max_exe_bytes = 4096;
static uint32_t rt_os_proc_max_args = 64;
static uint32_t rt_os_proc_max_arg_bytes = 4096;
static uint32_t rt_os_proc_max_env = 64;
static uint32_t rt_os_proc_max_env_key_bytes = 256;
static uint32_t rt_os_proc_max_env_val_bytes = 4096;
static uint32_t rt_os_proc_max_runtime_ms = 0;
static uint32_t rt_os_proc_max_stdout_bytes = 0;
static uint32_t rt_os_proc_max_stderr_bytes = 0;
static uint32_t rt_os_proc_max_total_bytes = 0;
static uint32_t rt_os_proc_max_stdin_bytes = 0;
static uint32_t rt_os_proc_kill_on_drop = 1;
static uint32_t rt_os_proc_kill_tree = 1;
static uint32_t rt_os_proc_allow_cwd = 0;
static const char* rt_os_proc_allow_cwd_roots = NULL;

static uint32_t rt_os_net_allow_tcp = 1;
static uint32_t rt_os_net_allow_dns = 1;
static const char* rt_os_net_allow_hosts = NULL;

static uint32_t rt_os_env_u32(const char* key, uint32_t def) {
  const char* raw = getenv(key);
  if (!raw || !*raw) return def;
  char* end = NULL;
  errno = 0;
  unsigned long v = strtoul(raw, &end, 10);
  if (errno != 0) return def;
  if (end == raw) return def;
  return (uint32_t)v;
}

static void rt_os_policy_init(ctx_t* ctx) {
  (void)ctx;
  if (rt_os_policy_inited) return;

  const char* w = getenv("X07_WORLD");
  const char* sb = getenv("X07_OS_SANDBOXED");
  if ((w && strcmp(w, "run-os-sandboxed") == 0) || (sb && sb[0] == '1')) {
    rt_os_sandboxed = 1;
  }

  if (rt_os_sandboxed) {
    rt_os_fs_enabled = rt_os_env_u32("X07_OS_FS", 0);
    rt_os_net_enabled = rt_os_env_u32("X07_OS_NET", 0);
    rt_os_env_enabled = rt_os_env_u32("X07_OS_ENV", 0);
    rt_os_time_enabled = rt_os_env_u32("X07_OS_TIME", 0);
    rt_os_proc_enabled = rt_os_env_u32("X07_OS_PROC", 0);

    rt_os_deny_hidden = rt_os_env_u32("X07_OS_DENY_HIDDEN", 1);
    rt_os_fs_read_roots = getenv("X07_OS_FS_READ_ROOTS");
    rt_os_fs_write_roots = getenv("X07_OS_FS_WRITE_ROOTS");

    rt_os_env_allow_keys = getenv("X07_OS_ENV_ALLOW_KEYS");
    rt_os_env_deny_keys = getenv("X07_OS_ENV_DENY_KEYS");

    rt_os_time_allow_wall_clock = rt_os_env_u32("X07_OS_TIME_ALLOW_WALL_CLOCK", 0);
    rt_os_time_allow_monotonic = rt_os_env_u32("X07_OS_TIME_ALLOW_MONOTONIC", 0);
    rt_os_time_allow_sleep = rt_os_env_u32("X07_OS_TIME_ALLOW_SLEEP", 0);
    rt_os_time_max_sleep_ms = rt_os_env_u32("X07_OS_TIME_MAX_SLEEP_MS", 0);
    rt_os_time_allow_local_tzid = rt_os_env_u32("X07_OS_TIME_ALLOW_LOCAL_TZID", 0);
    rt_os_proc_allow_exit = rt_os_env_u32("X07_OS_PROC_ALLOW_EXIT", 0);
    rt_os_proc_allow_spawn = rt_os_env_u32("X07_OS_PROC_ALLOW_SPAWN", 0);
    rt_os_proc_allow_exec = rt_os_env_u32("X07_OS_PROC_ALLOW_EXEC", 0);
    rt_os_proc_allow_execs = getenv("X07_OS_PROC_ALLOW_EXECS");
    rt_os_proc_allow_exec_prefixes = getenv("X07_OS_PROC_ALLOW_EXEC_PREFIXES");
    rt_os_proc_allow_args_regex_lite = getenv("X07_OS_PROC_ALLOW_ARGS_REGEX_LITE");
    rt_os_proc_allow_env_keys = getenv("X07_OS_PROC_ALLOW_ENV_KEYS");
    rt_os_proc_max_live = rt_os_env_u32("X07_OS_PROC_MAX_LIVE", 0);
    rt_os_proc_max_spawns = rt_os_env_u32("X07_OS_PROC_MAX_SPAWNS", 0);
    rt_os_proc_max_exe_bytes = rt_os_env_u32("X07_OS_PROC_MAX_EXE_BYTES", 4096);
    rt_os_proc_max_args = rt_os_env_u32("X07_OS_PROC_MAX_ARGS", 64);
    rt_os_proc_max_arg_bytes = rt_os_env_u32("X07_OS_PROC_MAX_ARG_BYTES", 4096);
    rt_os_proc_max_env = rt_os_env_u32("X07_OS_PROC_MAX_ENV", 64);
    rt_os_proc_max_env_key_bytes = rt_os_env_u32("X07_OS_PROC_MAX_ENV_KEY_BYTES", 256);
    rt_os_proc_max_env_val_bytes = rt_os_env_u32("X07_OS_PROC_MAX_ENV_VAL_BYTES", 4096);
    rt_os_proc_max_runtime_ms = rt_os_env_u32("X07_OS_PROC_MAX_RUNTIME_MS", 0);
    rt_os_proc_max_stdout_bytes = rt_os_env_u32("X07_OS_PROC_MAX_STDOUT_BYTES", 0);
    rt_os_proc_max_stderr_bytes = rt_os_env_u32("X07_OS_PROC_MAX_STDERR_BYTES", 0);
    rt_os_proc_max_total_bytes = rt_os_env_u32("X07_OS_PROC_MAX_TOTAL_BYTES", 0);
    rt_os_proc_max_stdin_bytes = rt_os_env_u32("X07_OS_PROC_MAX_STDIN_BYTES", 0);
    rt_os_proc_kill_on_drop = rt_os_env_u32("X07_OS_PROC_KILL_ON_DROP", 1);
    rt_os_proc_kill_tree = rt_os_env_u32("X07_OS_PROC_KILL_TREE", 1);
    rt_os_proc_allow_cwd = rt_os_env_u32("X07_OS_PROC_ALLOW_CWD", 0);
    rt_os_proc_allow_cwd_roots = getenv("X07_OS_PROC_ALLOW_CWD_ROOTS");

    rt_os_net_allow_tcp = rt_os_env_u32("X07_OS_NET_ALLOW_TCP", 0);
    rt_os_net_allow_dns = rt_os_env_u32("X07_OS_NET_ALLOW_DNS", 0);
    rt_os_net_allow_hosts = getenv("X07_OS_NET_ALLOW_HOSTS");
  } else {
    rt_os_fs_enabled = 1;
    rt_os_net_enabled = 1;
    rt_os_env_enabled = 1;
    rt_os_time_enabled = 1;
    rt_os_proc_enabled = 1;
    rt_os_deny_hidden = 0;
    rt_os_time_allow_wall_clock = 1;
    rt_os_time_allow_monotonic = 1;
    rt_os_time_allow_sleep = 1;
    rt_os_time_max_sleep_ms = 0;
    rt_os_time_allow_local_tzid = 1;
    rt_os_proc_allow_exit = 1;
    rt_os_proc_allow_spawn = 1;
    rt_os_proc_allow_exec = 1;
    rt_os_proc_allow_execs = NULL;
    rt_os_proc_allow_exec_prefixes = NULL;
    rt_os_proc_allow_args_regex_lite = NULL;
    rt_os_proc_allow_env_keys = NULL;
    rt_os_proc_max_live = 0;
    rt_os_proc_max_spawns = 0;
    rt_os_proc_max_exe_bytes = 0;
    rt_os_proc_max_args = 0;
    rt_os_proc_max_arg_bytes = 0;
    rt_os_proc_max_env = 0;
    rt_os_proc_max_env_key_bytes = 0;
    rt_os_proc_max_env_val_bytes = 0;
    rt_os_proc_max_runtime_ms = 0;
    rt_os_proc_max_stdout_bytes = 0;
    rt_os_proc_max_stderr_bytes = 0;
    rt_os_proc_max_total_bytes = 0;
    rt_os_proc_max_stdin_bytes = 0;
    rt_os_proc_kill_on_drop = 1;
    rt_os_proc_kill_tree = 1;
    rt_os_proc_allow_cwd = 1;
    rt_os_proc_allow_cwd_roots = NULL;
    rt_os_net_allow_tcp = 1;
    rt_os_net_allow_dns = 1;
  }

  rt_os_policy_inited = 1;
}

static void rt_os_require(ctx_t* ctx, uint32_t ok, const char* msg) {
  if (!rt_os_sandboxed) return;
  if (!ok) rt_trap(msg);
  (void)ctx;
}

static uint32_t rt_os_split_next(const char** cursor, const char** out_start, size_t* out_len) {
  const char* p = *cursor;
  if (!p) return UINT32_C(0);

  for (;;) {
    while (*p == ';' || *p == ' ' || *p == '\t' || *p == '\n' || *p == '\r') p++;
    if (*p == 0) {
      *cursor = p;
      return UINT32_C(0);
    }

    const char* start = p;
    while (*p && *p != ';') p++;
    const char* end = p;
    while (end > start
           && (end[-1] == ' ' || end[-1] == '\t' || end[-1] == '\n' || end[-1] == '\r')) {
      end--;
    }

    *cursor = (*p == ';') ? p + 1 : p;

    if (end == start) {
      p = *cursor;
      continue;
    }

    *out_start = start;
    *out_len = (size_t)(end - start);
    return UINT32_C(1);
  }
}

static uint32_t rt_os_list_contains(const char* list, bytes_t key) {
  if (!list || !*list) return UINT32_C(0);
  const char* cur = list;
  for (;;) {
    const char* s = NULL;
    size_t n = 0;
    if (!rt_os_split_next(&cur, &s, &n)) return UINT32_C(0);
    if (n > (size_t)UINT32_MAX) continue;
    if ((uint32_t)n != key.len) continue;
    if (memcmp(s, key.ptr, n) == 0) return UINT32_C(1);
  }
}

static uint32_t rt_os_list_contains_prefix(const char* list, bytes_t path) {
  if (!list || !*list) return UINT32_C(0);
  const char* cur = list;
  for (;;) {
    const char* s = NULL;
    size_t n = 0;
    if (!rt_os_split_next(&cur, &s, &n)) return UINT32_C(0);
    if (n > (size_t)UINT32_MAX) continue;
    if ((uint32_t)n > path.len) continue;
    if (memcmp(s, path.ptr, n) == 0) return UINT32_C(1);
  }
}

static uint32_t rt_os_regex_lite_match_full(
    const char* pat,
    size_t pat_len,
    bytes_t text
) {
  uint8_t tok_kind[256];
  uint8_t tok_lit[256];
  uint8_t tok_star[256];
  uint32_t k = 0;

  size_t i = 0;
  while (i < pat_len) {
    if (k >= UINT32_C(256)) rt_trap("os.process allow_args_regex_lite pattern too long");

    uint8_t c = (uint8_t)pat[i];
    if (c == (uint8_t)'*') rt_trap("os.process allow_args_regex_lite invalid pattern");

    uint8_t kind = 0;
    uint8_t lit = c;
    if (c == (uint8_t)'\\') {
      if (i + 1 >= pat_len) rt_trap("os.process allow_args_regex_lite invalid pattern");
      lit = (uint8_t)pat[i + 1];
      kind = 0;
      i += 2;
    } else if (c == (uint8_t)'.') {
      kind = 1;
      lit = 0;
      i += 1;
    } else {
      kind = 0;
      lit = c;
      i += 1;
    }

    uint8_t star = 0;
    if (i < pat_len && (uint8_t)pat[i] == (uint8_t)'*') {
      star = 1;
      i += 1;
    }

    tok_kind[k] = kind;
    tok_lit[k] = lit;
    tok_star[k] = star;
    k += 1;
  }

  uint8_t states[257];
  uint8_t next[257];
  for (uint32_t j = 0; j <= k; j++) {
    states[j] = 0;
    next[j] = 0;
  }

  states[0] = 1;
  for (uint32_t j = 0; j < k; j++) {
    if (tok_star[j] && states[j]) states[j + 1] = 1;
  }

  for (uint32_t t = 0; t < text.len; t++) {
    uint8_t ch = text.ptr[t];
    for (uint32_t j = 0; j <= k; j++) next[j] = 0;

    for (uint32_t j = 0; j < k; j++) {
      if (!states[j]) continue;

      uint8_t match = 0;
      if (tok_kind[j] == 1) {
        match = 1;
      } else if (tok_lit[j] == ch) {
        match = 1;
      }

      if (!match) continue;

      if (tok_star[j]) {
        next[j] = 1;
      } else {
        next[j + 1] = 1;
      }
    }

    for (uint32_t j = 0; j < k; j++) {
      if (tok_star[j] && next[j]) next[j + 1] = 1;
    }

    for (uint32_t j = 0; j <= k; j++) states[j] = next[j];
  }

  for (uint32_t j = 0; j < k; j++) {
    if (tok_star[j] && states[j]) states[j + 1] = 1;
  }

  return states[k] ? UINT32_C(1) : UINT32_C(0);
}

static uint32_t rt_os_proc_args_allowed(bytes_t arg) {
  if (!rt_os_proc_allow_args_regex_lite || !*rt_os_proc_allow_args_regex_lite) return UINT32_C(1);
  const char* cur = rt_os_proc_allow_args_regex_lite;
  for (;;) {
    const char* s = NULL;
    size_t n = 0;
    if (!rt_os_split_next(&cur, &s, &n)) return UINT32_C(0);
    if (rt_os_regex_lite_match_full(s, n, arg)) return UINT32_C(1);
  }
}

static char* rt_os_bytes_to_cstr(ctx_t* ctx, bytes_t b, const char* what) {
  for (uint32_t i = 0; i < b.len; i++) {
    if (b.ptr[i] == 0) rt_trap(what);
  }
  char* out = (char*)rt_alloc(ctx, b.len + 1, 1);
  if (b.len != 0) {
    memcpy(out, b.ptr, b.len);
    rt_mem_on_memcpy(ctx, b.len);
  }
  out[b.len] = 0;
  return out;
}

static uint32_t rt_os_path_has_hidden_segment(bytes_t path) {
  if (path.len == 0) return UINT32_C(0);
  uint32_t seg_start = 0;
  for (uint32_t i = 0; i <= path.len; i++) {
    uint8_t b = (i == path.len) ? (uint8_t)'/' : path.ptr[i];
    if (b == (uint8_t)'/') {
      if (i > seg_start && path.ptr[seg_start] == (uint8_t)'.') return UINT32_C(1);
      seg_start = i + 1;
    }
  }
  return UINT32_C(0);
}

static char* rt_os_join_root_and_rel(ctx_t* ctx, const char* root, size_t root_len, bytes_t rel) {
  if (root_len == 0) rt_trap("os.fs root empty");
  if (root_len > (size_t)UINT32_MAX) rt_trap("os.fs root too long");

  uint32_t need_slash = 0;
  char last = root[root_len - 1];
  if (last != '/' && last != '\\') need_slash = 1;

  uint64_t total = (uint64_t)root_len + (uint64_t)need_slash + (uint64_t)rel.len + UINT64_C(1);
  if (total > (uint64_t)UINT32_MAX) rt_trap("os.fs path too long");

  char* out = (char*)rt_alloc(ctx, (uint32_t)total, 1);
  memcpy(out, root, root_len);
  rt_mem_on_memcpy(ctx, (uint32_t)root_len);
  uint32_t off = (uint32_t)root_len;
  if (need_slash) {
    out[off] = '/';
    off += 1;
  }
  if (rel.len != 0) {
    memcpy(out + off, rel.ptr, rel.len);
    rt_mem_on_memcpy(ctx, rel.len);
  }
  out[off + rel.len] = 0;
  return out;
}

static bytes_t rt_os_strip_root_prefix(bytes_t path, const char* root, size_t root_len) {
  // If the caller already included the root prefix in `path` (common for project-relative paths like
  // `out/file.txt`), avoid producing `root/root/...` when we join roots.
  size_t trimmed = root_len;
  while (trimmed > 0 && (root[trimmed - 1] == '/' || root[trimmed - 1] == '\\')) trimmed--;
  if (trimmed == 0) return path;
  if ((uint32_t)trimmed >= path.len) return path;
  if (memcmp(path.ptr, root, trimmed) != 0) return path;
  if (path.ptr[trimmed] != (uint8_t)'/') return path;

  bytes_t out = path;
  out.ptr = path.ptr + trimmed + 1;
  out.len = path.len - (uint32_t)(trimmed + 1);
  return out;
}

static bytes_t rt_os_fs_read_file(ctx_t* ctx, bytes_t path) {
  rt_os_policy_init(ctx);

  FILE* f = NULL;
  char* p = NULL;

  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_fs_enabled, "os.fs disabled by policy");
    bytes_view_t path_view = rt_bytes_view(ctx, path);
    if (!rt_fs_is_safe_rel_path(path_view)) rt_trap("os.fs.read_file unsafe path");
    if (rt_os_deny_hidden && rt_os_path_has_hidden_segment(path)) {
      rt_trap("os.fs.read_file hidden path denied");
    }

    const char* roots = rt_os_fs_read_roots;
    const char* cur = roots;
    const char* root = NULL;
    size_t root_len = 0;
    while (rt_os_split_next(&cur, &root, &root_len)) {
      bytes_t rel = rt_os_strip_root_prefix(path, root, root_len);
      p = rt_os_join_root_and_rel(ctx, root, root_len, rel);
      f = fopen(p, "rb");
      if (f) break;
    }
    if (!f) rt_trap("os.fs.read_file open failed");
  } else {
    p = rt_os_bytes_to_cstr(ctx, path, "os.fs.read_file path contains NUL");
    f = fopen(p, "rb");
    if (!f) rt_trap("os.fs.read_file open failed");
  }

  if (fseek(f, 0, SEEK_END) != 0) rt_trap("os.fs.read_file seek failed");
  long end = ftell(f);
  if (end < 0) rt_trap("os.fs.read_file tell failed");
  if ((uint64_t)end > (uint64_t)UINT32_MAX) rt_trap("os.fs.read_file file too large");
  if (fseek(f, 0, SEEK_SET) != 0) rt_trap("os.fs.read_file seek failed");

  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)end);
  if (out.len != 0) {
    size_t n = fread(out.ptr, 1, out.len, f);
    if (n != out.len) rt_trap("os.fs.read_file short read");
  }
  fclose(f);
  return out;
}

static uint32_t rt_os_fs_write_file(ctx_t* ctx, bytes_t path, bytes_t data) {
  rt_os_policy_init(ctx);

  FILE* f = NULL;
  char* p = NULL;
  int last_errno = 0;

  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_fs_enabled, "os.fs disabled by policy");
    bytes_view_t path_view = rt_bytes_view(ctx, path);
    if (!rt_fs_is_safe_rel_path(path_view)) rt_trap("os.fs.write_file unsafe path");
    if (rt_os_deny_hidden && rt_os_path_has_hidden_segment(path)) {
      rt_trap("os.fs.write_file hidden path denied");
    }

    const char* roots = rt_os_fs_write_roots;
    const char* cur = roots;
    const char* root = NULL;
    size_t root_len = 0;
    while (rt_os_split_next(&cur, &root, &root_len)) {
      bytes_t rel = rt_os_strip_root_prefix(path, root, root_len);
      p = rt_os_join_root_and_rel(ctx, root, root_len, rel);
      f = fopen(p, "wb");
      if (f) break;
      last_errno = errno;
    }
    if (!f) return (uint32_t)(last_errno ? last_errno : EACCES);
  } else {
    p = rt_os_bytes_to_cstr(ctx, path, "os.fs.write_file path contains NUL");
    f = fopen(p, "wb");
    if (!f) {
      last_errno = errno;
      return (uint32_t)(last_errno ? last_errno : 1);
    }
  }

  if (data.len != 0) {
    size_t n = fwrite(data.ptr, 1, data.len, f);
    if (n != data.len) {
      last_errno = errno;
      fclose(f);
      return (uint32_t)(last_errno ? last_errno : 1);
    }
  }
  if (fclose(f) != 0) {
    last_errno = errno;
    return (uint32_t)(last_errno ? last_errno : 1);
  }
  return UINT32_C(0);
}

static bytes_t rt_os_env_get(ctx_t* ctx, bytes_t key) {
  rt_os_policy_init(ctx);

  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_env_enabled, "os.env disabled by policy");
    if (rt_os_list_contains(rt_os_env_deny_keys, key)) rt_trap("os.env.get key denied");
    if (!rt_os_list_contains(rt_os_env_allow_keys, key)) rt_trap("os.env.get key not allowed");
  }

  char* k = rt_os_bytes_to_cstr(ctx, key, "os.env.get key contains NUL");
  const char* v = getenv(k);
  if (!v) return rt_bytes_empty(ctx);
  size_t n = strlen(v);
  if (n > (size_t)UINT32_MAX) rt_trap("os.env.get value too large");
  bytes_t out = rt_bytes_alloc(ctx, (uint32_t)n);
  if (out.len != 0) {
    memcpy(out.ptr, v, n);
    rt_mem_on_memcpy(ctx, out.len);
  }
  return out;
}

static uint32_t rt_os_time_now_unix_ms(ctx_t* ctx) {
  rt_os_policy_init(ctx);
  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_time_enabled, "os.time disabled by policy");
    rt_os_require(ctx, rt_os_time_allow_wall_clock, "os.time.now_unix_ms disabled by policy");
  }

  struct timespec ts;
  if (timespec_get(&ts, TIME_UTC) != TIME_UTC) rt_trap("os.time.now_unix_ms failed");
  uint64_t ms = (uint64_t)ts.tv_sec * UINT64_C(1000) + (uint64_t)(ts.tv_nsec / 1000000L);
  return (uint32_t)ms;
}

#define RT_OS_TIME_CODE_POLICY_DENIED UINT32_C(300)
#define RT_OS_TIME_CODE_INTERNAL UINT32_C(301)

#define RT_OS_TIME_TZID_CODE_POLICY_DENIED UINT32_C(1)
#define RT_OS_TIME_TZID_CODE_INTERNAL UINT32_C(2)

static bytes_t rt_os_time_duration_err(ctx_t* ctx, uint32_t code) {
  bytes_t out = rt_bytes_alloc(ctx, UINT32_C(9));
  out.ptr[0] = 0;
  rt_write_u32_le(out.ptr + 1, code);
  rt_write_u32_le(out.ptr + 5, UINT32_C(0));
  return out;
}

static bytes_t rt_os_time_local_tzid_err(ctx_t* ctx, uint32_t code) {
  bytes_t out = rt_bytes_alloc(ctx, UINT32_C(5));
  out.ptr[0] = 0;
  rt_write_u32_le(out.ptr + 1, code);
  return out;
}

static bytes_t rt_os_time_now_instant_v1(ctx_t* ctx) {
  rt_os_policy_init(ctx);
  if (rt_os_sandboxed) {
    if (!rt_os_time_enabled) return rt_os_time_duration_err(ctx, RT_OS_TIME_CODE_POLICY_DENIED);
    if (!rt_os_time_allow_wall_clock) return rt_os_time_duration_err(ctx, RT_OS_TIME_CODE_POLICY_DENIED);
  }

  struct timespec ts;
  if (timespec_get(&ts, TIME_UTC) != TIME_UTC) {
    return rt_os_time_duration_err(ctx, RT_OS_TIME_CODE_INTERNAL);
  }

  int64_t unix_s = (int64_t)ts.tv_sec;
  uint64_t unix_s_bits = (uint64_t)unix_s;

  if (ts.tv_nsec < 0 || ts.tv_nsec >= 1000000000L) {
    return rt_os_time_duration_err(ctx, RT_OS_TIME_CODE_INTERNAL);
  }
  uint32_t nanos = (uint32_t)ts.tv_nsec;

  bytes_t out = rt_bytes_alloc(ctx, UINT32_C(14));
  out.ptr[0] = 1;
  out.ptr[1] = 1;
  rt_write_u32_le(out.ptr + 2, (uint32_t)(unix_s_bits & UINT64_C(0xFFFFFFFF)));
  rt_write_u32_le(out.ptr + 6, (uint32_t)((unix_s_bits >> 32) & UINT64_C(0xFFFFFFFF)));
  rt_write_u32_le(out.ptr + 10, nanos);
  return out;
}

static uint32_t rt_os_time_sleep_ms_v1(ctx_t* ctx, int32_t ms) {
  rt_os_policy_init(ctx);
  if (ms < 0) return UINT32_C(0);

  if (rt_os_sandboxed) {
    if (!rt_os_time_enabled) return UINT32_C(0);
    if (!rt_os_time_allow_sleep) return UINT32_C(0);
    if ((uint32_t)ms > rt_os_time_max_sleep_ms) return UINT32_C(0);
  }

  uint32_t ms_u = (uint32_t)ms;
#if defined(_WIN32)
  Sleep(ms_u);
  return UINT32_C(1);
#else
  struct timespec req;
  req.tv_sec = (time_t)(ms_u / UINT32_C(1000));
  req.tv_nsec = (long)((ms_u % UINT32_C(1000)) * UINT32_C(1000000));

  for (;;) {
    if (nanosleep(&req, &req) == 0) return UINT32_C(1);
    if (errno == EINTR) continue;
    return UINT32_C(0);
  }
#endif
}

static bytes_t rt_os_time_local_tzid_v1(ctx_t* ctx) {
  rt_os_policy_init(ctx);
  if (rt_os_sandboxed) {
    if (!rt_os_time_enabled) return rt_os_time_local_tzid_err(ctx, RT_OS_TIME_TZID_CODE_POLICY_DENIED);
    if (!rt_os_time_allow_local_tzid) return rt_os_time_local_tzid_err(ctx, RT_OS_TIME_TZID_CODE_POLICY_DENIED);
  }

  // v1: if the platform tzid can't be determined portably, return empty.
  bytes_t out = rt_bytes_alloc(ctx, UINT32_C(6));
  out.ptr[0] = 1;
  out.ptr[1] = 1;
  rt_write_u32_le(out.ptr + 2, UINT32_C(0));
  return out;
}

static uint64_t rt_os_now_ms(void) {
  struct timespec ts;
  if (timespec_get(&ts, TIME_UTC) != TIME_UTC) rt_trap("os.process.run_capture_v1 clock failed");
  return (uint64_t)ts.tv_sec * UINT64_C(1000) + (uint64_t)(ts.tv_nsec / 1000000L);
}

static void rt_os_close_fd(int fd) {
  if (fd < 0) return;
  for (;;) {
    if (close(fd) == 0) return;
    if (errno == EINTR) continue;
    return;
  }
}

static int rt_os_set_nonblock(int fd) {
#ifdef _WIN32
  (void)fd; return 0;
#else
  int flags = fcntl(fd, F_GETFL, 0);
  if (flags < 0) return -1;
  if (fcntl(fd, F_SETFL, flags | O_NONBLOCK) < 0) return -1;
  return 0;
#endif
}

static int rt_os_set_cloexec(int fd) {
#ifdef _WIN32
  (void)fd; return 0;
#else
  int flags = fcntl(fd, F_GETFD, 0);
  if (flags < 0) return -1;
  if (fcntl(fd, F_SETFD, flags | FD_CLOEXEC) < 0) return -1;
  return 0;
#endif
}

#define RT_OS_PROC_CODE_POLICY_DENIED UINT32_C(1)
#define RT_OS_PROC_CODE_INVALID_REQUEST UINT32_C(2)
#define RT_OS_PROC_CODE_SPAWN_FAILED UINT32_C(3)
#define RT_OS_PROC_CODE_TIMEOUT UINT32_C(4)
#define RT_OS_PROC_CODE_OUTPUT_LIMIT UINT32_C(5)

#define RT_OS_PROC_STATE_EMPTY UINT32_C(0)
#define RT_OS_PROC_STATE_RUNNING UINT32_C(1)
#define RT_OS_PROC_STATE_DONE UINT32_C(2)

#define RT_OS_PROC_MODE_CAPTURE UINT32_C(1)
#define RT_OS_PROC_MODE_PIPED UINT32_C(2)

#define RT_OS_PROC_POLL_KIND_STDIN UINT32_C(1)
#define RT_OS_PROC_POLL_KIND_STDOUT UINT32_C(2)
#define RT_OS_PROC_POLL_KIND_STDERR UINT32_C(3)

struct rt_os_proc_s {
  uint32_t state;
  uint32_t mode;
  uint16_t gen;
  uint16_t _pad;

#if defined(_WIN32)
  HANDLE win_proc;
  HANDLE win_job;
  HANDLE win_stdin;
  HANDLE win_stdout;
  HANDLE win_stderr;
  HANDLE win_thread_stdin;
  HANDLE win_thread_stdout;
  HANDLE win_thread_stderr;
  HANDLE win_stdin_event;
  volatile LONG win_lock;
#else
  pid_t pid;
  pid_t pgid;
  int stdin_fd;
  int stdout_fd;
  int stderr_fd;
#endif

  uint32_t stdin_closed;
  uint32_t stdout_closed;
  uint32_t stderr_closed;

  uint32_t exited;
  int status;

  uint32_t fail_code;
  uint32_t kill_sent;

  uint32_t max_stdout;
  uint32_t max_stderr;
  uint32_t max_total;
  uint32_t timeout_ms;
  uint64_t start_ms;

  bytes_t stdin_buf;
  uint32_t stdin_off;

  bytes_t stdout_buf;
  uint32_t stdout_off;
  uint32_t stdout_len;

  bytes_t stderr_buf;
  uint32_t stderr_off;
  uint32_t stderr_len;

  bytes_t result;
  uint32_t result_taken;
  uint32_t exit_taken;

  uint32_t join_wait_head;
  uint32_t join_wait_tail;

  uint32_t exit_wait_head;
  uint32_t exit_wait_tail;
};

static void rt_os_procs_ensure_cap(ctx_t* ctx, uint32_t need) {
#if defined(_WIN32)
  if (need <= ctx->os_procs_cap) return;
  if (ctx->os_procs_cap != 0) rt_trap("os.process out of proc slots");
  uint32_t new_cap = UINT32_C(1024);
  if (new_cap < need) new_cap = need;
  rt_os_proc_t* items = (rt_os_proc_t*)rt_alloc(
    ctx,
    new_cap * (uint32_t)sizeof(rt_os_proc_t),
    (uint32_t)_Alignof(rt_os_proc_t)
  );
  memset(items, 0, new_cap * (uint32_t)sizeof(rt_os_proc_t));
  ctx->os_procs = items;
  ctx->os_procs_cap = new_cap;
  return;
#else
  if (need <= ctx->os_procs_cap) return;
  rt_os_proc_t* old_items = ctx->os_procs;
  uint32_t old_cap = ctx->os_procs_cap;
  uint32_t old_bytes_total = old_cap * (uint32_t)sizeof(rt_os_proc_t);
  uint32_t new_cap = ctx->os_procs_cap ? ctx->os_procs_cap : 8;
  while (new_cap < need) {
    if (new_cap > UINT32_MAX / 2) {
      new_cap = need;
      break;
    }
    new_cap *= 2;
  }
  rt_os_proc_t* items = (rt_os_proc_t*)rt_alloc_realloc(
    ctx,
    old_items,
    old_bytes_total,
    new_cap * (uint32_t)sizeof(rt_os_proc_t),
    (uint32_t)_Alignof(rt_os_proc_t)
  );
  if (old_items && ctx->os_procs_len) {
    uint32_t bytes = ctx->os_procs_len * (uint32_t)sizeof(rt_os_proc_t);
    memcpy(items, old_items, bytes);
    rt_mem_on_memcpy(ctx, bytes);
  }
  if (old_items && old_bytes_total) {
    rt_free(ctx, old_items, old_bytes_total, (uint32_t)_Alignof(rt_os_proc_t));
  }
  ctx->os_procs = items;
  ctx->os_procs_cap = new_cap;
#endif
}

static uint16_t rt_os_proc_next_gen(uint16_t gen) {
  gen = (uint16_t)(gen + 1);
  if (gen == 0) gen = 1;
  return gen;
}

static uint32_t rt_os_proc_handle_u32(uint32_t idx, uint16_t gen) {
  uint32_t idx16 = idx + 1;
  if (idx16 == 0 || idx16 > UINT32_C(0xFFFF)) rt_trap("os.process handle overflow");
  if (gen == 0) rt_trap("os.process handle overflow");
  return ((uint32_t)gen << 16) | idx16;
}

static int32_t rt_os_proc_handle_i32(uint32_t idx, uint16_t gen) {
  return (int32_t)rt_os_proc_handle_u32(idx, gen);
}

static uint32_t rt_os_proc_handle_is_sentinel(int32_t handle, uint32_t* out_code) {
  uint32_t h = (uint32_t)handle;
  if ((h & UINT32_C(0xFFFF)) != 0) return UINT32_C(0);
  uint32_t code = h >> 16;
  if (code == 0) return UINT32_C(0);
  if (out_code) *out_code = code;
  return UINT32_C(1);
}

static void rt_os_proc_init_entry(ctx_t* ctx, rt_os_proc_t* p, uint16_t gen) {
  memset(p, 0, sizeof(*p));
  p->state = RT_OS_PROC_STATE_EMPTY;
  p->gen = gen ? gen : 1;
#if !defined(_WIN32)
  p->pid = (pid_t)-1;
  p->pgid = (pid_t)-1;
  p->stdin_fd = -1;
  p->stdout_fd = -1;
  p->stderr_fd = -1;
#endif
  p->stdin_buf = rt_bytes_empty(ctx);
  p->stdout_buf = rt_bytes_empty(ctx);
  p->stderr_buf = rt_bytes_empty(ctx);
  p->result = rt_bytes_empty(ctx);
}

#if defined(_WIN32)
static void rt_os_proc_lock(rt_os_proc_t* p) {
  for (;;) {
    if (InterlockedCompareExchange(&p->win_lock, 1, 0) == 0) return;
    Sleep(0);
  }
}

static void rt_os_proc_unlock(rt_os_proc_t* p) {
  InterlockedExchange(&p->win_lock, 0);
}
#endif

static uint32_t rt_os_proc_find_free_slot(ctx_t* ctx) {
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    if (ctx->os_procs[i].state == RT_OS_PROC_STATE_EMPTY) return i;
  }
  return UINT32_MAX;
}

static uint32_t rt_os_proc_alloc_slot(ctx_t* ctx, uint32_t allow_extend) {
  uint32_t idx = rt_os_proc_find_free_slot(ctx);
  if (idx != UINT32_MAX) {
    rt_os_proc_t* p = &ctx->os_procs[idx];
    uint16_t gen = p->gen ? p->gen : 1;
    rt_os_proc_init_entry(ctx, p, gen);
    return idx;
  }
  if (!allow_extend) rt_trap("os.process out of proc slots");
  idx = ctx->os_procs_len;
  rt_os_procs_ensure_cap(ctx, idx + 1);
  ctx->os_procs_len += 1;
  rt_os_proc_init_entry(ctx, &ctx->os_procs[idx], 1);
  return idx;
}

static rt_os_proc_t* rt_os_proc_from_handle(ctx_t* ctx, int32_t handle, uint32_t* out_idx) {
  uint32_t h = (uint32_t)handle;
  uint32_t idx16 = h & UINT32_C(0xFFFF);
  uint16_t gen = (uint16_t)(h >> 16);
  if (idx16 == 0 || gen == 0) rt_trap("os.process invalid handle");
  uint32_t idx = idx16 - 1;
  if (idx >= ctx->os_procs_len) rt_trap("os.process invalid handle");
  rt_os_proc_t* p = &ctx->os_procs[idx];
  if (p->state == RT_OS_PROC_STATE_EMPTY) rt_trap("os.process invalid handle");
  if (p->gen != gen) rt_trap("os.process invalid handle");
  if (out_idx) *out_idx = idx;
  return p;
}

static void rt_os_proc_close_fds(rt_os_proc_t* p) {
#if defined(_WIN32)
  rt_os_proc_lock(p);
  if (p->win_stdin) {
    (void)CloseHandle(p->win_stdin);
    p->win_stdin = NULL;
  }
  if (p->win_stdout) {
    (void)CloseHandle(p->win_stdout);
    p->win_stdout = NULL;
  }
  if (p->win_stderr) {
    (void)CloseHandle(p->win_stderr);
    p->win_stderr = NULL;
  }
  if (p->win_stdin_event) {
    (void)SetEvent(p->win_stdin_event);
  }
  p->stdin_closed = 1;
  p->stdout_closed = 1;
  p->stderr_closed = 1;
  rt_os_proc_unlock(p);
#else
  rt_os_close_fd(p->stdin_fd);
  rt_os_close_fd(p->stdout_fd);
  rt_os_close_fd(p->stderr_fd);
  p->stdin_fd = -1;
  p->stdout_fd = -1;
  p->stderr_fd = -1;
  p->stdin_closed = 1;
  p->stdout_closed = 1;
  p->stderr_closed = 1;
#endif
}

static void rt_os_proc_drop_buffers(ctx_t* ctx, rt_os_proc_t* p) {
  rt_bytes_drop(ctx, &p->stdin_buf);
  rt_bytes_drop(ctx, &p->stdout_buf);
  rt_bytes_drop(ctx, &p->stderr_buf);
  p->stdin_buf = rt_bytes_empty(ctx);
  p->stdout_buf = rt_bytes_empty(ctx);
  p->stderr_buf = rt_bytes_empty(ctx);
  p->stdin_off = 0;
  p->stdout_off = 0;
  p->stdout_len = 0;
  p->stderr_off = 0;
  p->stderr_len = 0;
}

static void rt_os_proc_drop_result(ctx_t* ctx, rt_os_proc_t* p) {
  rt_bytes_drop(ctx, &p->result);
  p->result = rt_bytes_empty(ctx);
  p->result_taken = 0;
}

static void rt_os_wake_wait_list(
    ctx_t* ctx,
    uint32_t* head,
    uint32_t* tail,
    uint32_t wait_kind,
    uint32_t reason_id
) {
  uint32_t w = *head;
  uint32_t wt = *tail;
  (void)wt;
  *head = 0;
  *tail = 0;
  while (w != 0) {
    rt_task_t* waiter = rt_task_ptr(ctx, w);
    uint32_t next = waiter->wait_next;
    waiter->wait_next = 0;
    rt_sched_wake(ctx, w, wait_kind, reason_id);
    w = next;
  }
}

static void rt_os_proc_wake_waiters(ctx_t* ctx, rt_os_proc_t* p, uint32_t reason_id) {
  rt_os_wake_wait_list(
    ctx,
    &p->join_wait_head,
    &p->join_wait_tail,
    RT_WAIT_OS_PROC_JOIN,
    reason_id
  );
}

static void rt_os_proc_wake_exit_waiters(ctx_t* ctx, rt_os_proc_t* p, uint32_t reason_id) {
  rt_os_wake_wait_list(
    ctx,
    &p->exit_wait_head,
    &p->exit_wait_tail,
    RT_WAIT_OS_PROC_EXIT,
    reason_id
  );
}

static bytes_t rt_os_proc_make_err(ctx_t* ctx, uint32_t code) {
  bytes_t out = rt_bytes_alloc(ctx, 9);
  out.ptr[0] = 0;
  rt_write_u32_le(out.ptr + 1, code);
  rt_write_u32_le(out.ptr + 5, 0);
  return out;
}

static uint32_t rt_os_proc_exit_code_from_status(int status) {
#if defined(_WIN32)
  return (uint32_t)status;
#else
  uint32_t exit_code = UINT32_C(1);
  if (WIFEXITED(status)) {
    exit_code = (uint32_t)WEXITSTATUS(status);
  } else if (WIFSIGNALED(status)) {
    exit_code = UINT32_C(128) + (uint32_t)WTERMSIG(status);
  }
  return exit_code;
#endif
}

static bytes_t rt_os_proc_build_ok_doc(
    ctx_t* ctx,
    uint32_t exit_code,
    uint32_t flags,
    bytes_t stdout_buf,
    uint32_t stdout_len,
    bytes_t stderr_buf,
    uint32_t stderr_len
) {
  if (stdout_len > UINT32_MAX - UINT32_C(18) || stderr_len > UINT32_MAX - UINT32_C(18) - stdout_len) {
    return rt_os_proc_make_err(ctx, RT_OS_PROC_CODE_OUTPUT_LIMIT);
  }

  uint32_t out_len = UINT32_C(18) + stdout_len + stderr_len;
  bytes_t out = rt_bytes_alloc(ctx, out_len);
  uint8_t* p = out.ptr;
  p[0] = 1;                 // ok tag
  p[1] = 1;                 // ProcRespV1 ver
  rt_write_u32_le(p + 2, exit_code);
  rt_write_u32_le(p + 6, flags);
  rt_write_u32_le(p + 10, stdout_len);
  if (stdout_len != 0) {
    memcpy(p + 14, stdout_buf.ptr, stdout_len);
    rt_mem_on_memcpy(ctx, stdout_len);
  }
  rt_write_u32_le(p + 14 + stdout_len, stderr_len);
  if (stderr_len != 0) {
    memcpy(p + 18 + stdout_len, stderr_buf.ptr, stderr_len);
    rt_mem_on_memcpy(ctx, stderr_len);
  }
  return out;
}

static void rt_os_proc_try_wait(rt_os_proc_t* p) {
#if defined(_WIN32)
  if (!p->win_proc) return;
  if (p->exited) return;
  DWORD r = WaitForSingleObject(p->win_proc, 0);
  if (r == WAIT_OBJECT_0) {
    DWORD code = 0;
    if (!GetExitCodeProcess(p->win_proc, &code)) code = 0;
    p->exited = 1;
    p->status = (int)code;
  }
#else
  if (p->pid == (pid_t)-1) return;
  if (p->exited) return;
  for (;;) {
    pid_t r = waitpid(p->pid, &p->status, WNOHANG);
    if (r == p->pid) {
      p->exited = 1;
      return;
    }
    if (r == 0) {
      return;
    }
    if (r < 0) {
      if (errno == EINTR) continue;
      p->exited = 1;
      p->status = 0;
      return;
    }
  }
#endif
}

#if !defined(_WIN32)
static pid_t rt_os_proc_kill_target(rt_os_proc_t* p) {
  if (p->pid == (pid_t)-1) return (pid_t)-1;
  if (rt_os_proc_kill_tree && p->pgid > (pid_t)1) {
    return (pid_t)(-p->pgid);
  }
  return p->pid;
}
#endif

static void rt_os_proc_send_kill(rt_os_proc_t* p, int32_t sig) {
#if defined(_WIN32)
  (void)sig;
  if (!p->win_proc) return;
  if (p->kill_sent) return;
  if (rt_os_proc_kill_tree && p->win_job) {
    (void)TerminateJobObject(p->win_job, 1);
  } else {
    (void)TerminateProcess(p->win_proc, 1);
  }
  p->kill_sent = 1;
#else
  if (p->pid == (pid_t)-1) return;
  if (p->kill_sent) return;
  pid_t target = rt_os_proc_kill_target(p);
  if (target == (pid_t)-1 || target == (pid_t)0) return;
  (void)kill(target, sig);
  p->kill_sent = 1;
#endif
}

static void rt_os_proc_mark_done(ctx_t* ctx, rt_os_proc_t* p, uint32_t idx, bytes_t result) {
  if (p->state == RT_OS_PROC_STATE_RUNNING) {
    if (ctx->os_procs_live == 0) rt_trap("os.process live underflow");
    ctx->os_procs_live -= 1;
  }

  rt_os_proc_drop_result(ctx, p);
  p->state = RT_OS_PROC_STATE_DONE;
  p->result = result;
  p->result_taken = 0;

  uint32_t reason_id = rt_os_proc_handle_u32(idx, p->gen);
  rt_os_proc_wake_waiters(ctx, p, reason_id);
}

static void rt_os_proc_finish_ok(ctx_t* ctx, rt_os_proc_t* p, uint32_t idx) {
  uint32_t exit_code = rt_os_proc_exit_code_from_status(p->status);
  bytes_t doc = rt_os_proc_build_ok_doc(
    ctx,
    exit_code,
    UINT32_C(0),
    p->stdout_buf,
    p->stdout_len,
    p->stderr_buf,
    p->stderr_len
  );

  rt_os_proc_drop_buffers(ctx, p);
  rt_os_proc_mark_done(ctx, p, idx, doc);
}

static void rt_os_proc_finish_err(ctx_t* ctx, rt_os_proc_t* p, uint32_t idx, uint32_t code) {
  rt_os_proc_drop_buffers(ctx, p);
  rt_os_proc_mark_done(ctx, p, idx, rt_os_proc_make_err(ctx, code));
}

static void rt_os_proc_finish_piped(ctx_t* ctx, rt_os_proc_t* p) {
  if (p->state == RT_OS_PROC_STATE_RUNNING) {
    if (ctx->os_procs_live == 0) rt_trap("os.process live underflow");
    ctx->os_procs_live -= 1;
  }
  rt_os_proc_drop_result(ctx, p);
  p->state = RT_OS_PROC_STATE_DONE;
  p->result_taken = 0;
  rt_bytes_drop(ctx, &p->stdin_buf);
  p->stdin_buf = rt_bytes_empty(ctx);
  p->stdin_off = 0;
}

static void rt_os_proc_kill_and_reap(rt_os_proc_t* p) {
#if defined(_WIN32)
  if (!p->win_proc) return;
  if (rt_os_proc_kill_tree && p->win_job) {
    (void)TerminateJobObject(p->win_job, 1);
  } else {
    (void)TerminateProcess(p->win_proc, 1);
  }
  (void)WaitForSingleObject(p->win_proc, INFINITE);
  DWORD code = 0;
  if (!GetExitCodeProcess(p->win_proc, &code)) code = 0;
  p->exited = 1;
  p->status = (int)code;
#else
  if (p->pid == (pid_t)-1) return;
  pid_t target = rt_os_proc_kill_target(p);
  if (target == (pid_t)-1 || target == (pid_t)0) return;
  (void)kill(target, SIGKILL);
  for (;;) {
    int status = 0;
    pid_t r = waitpid(p->pid, &status, 0);
    if (r == p->pid) break;
    if (r < 0 && errno == EINTR) continue;
    break;
  }
#endif
}

static void rt_os_spawn_free_argv_env(
    ctx_t* ctx,
    char** argv,
    uint32_t* argv_sizes,
    uint32_t argv_count,
    char** envp,
    uint32_t* env_sizes,
    uint32_t env_count
) {
  if (argv) {
    for (uint32_t i = 0; i < argv_count; i++) {
      if (argv[i]) rt_free(ctx, argv[i], argv_sizes ? argv_sizes[i] : 0, 1);
    }
    rt_free(ctx, argv, (argv_count + 1) * (uint32_t)sizeof(char*), (uint32_t)_Alignof(char*));
  }
  if (argv_sizes) {
    rt_free(ctx, argv_sizes, argv_count * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  }

  if (envp) {
    for (uint32_t i = 0; i < env_count; i++) {
      if (envp[i]) rt_free(ctx, envp[i], env_sizes ? env_sizes[i] : 0, 1);
    }
    rt_free(ctx, envp, (env_count + 1) * (uint32_t)sizeof(char*), (uint32_t)_Alignof(char*));
  }
  if (env_sizes) {
    rt_free(ctx, env_sizes, env_count * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));
  }
}

#if defined(_WIN32)
static void rt_os_win_close_handle(HANDLE* h) {
  if (!h || !*h) return;
  (void)CloseHandle(*h);
  *h = NULL;
}

static uint32_t rt_os_win_wcs_len(const wchar_t* s) {
  uint32_t n = 0;
  while (s && s[n]) n += 1;
  return n;
}

static wchar_t* rt_os_win_utf8_to_wide_alloc(
    ctx_t* ctx,
    const char* s,
    uint32_t s_len,
    const char* what,
    uint32_t* out_bytes
) {
  if (s_len > (uint32_t)INT32_MAX) rt_trap(what);
  int wlen = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, s, (int)s_len, NULL, 0);
  if (wlen <= 0) rt_trap(what);
  uint64_t bytes64 = ((uint64_t)(wlen + 1)) * (uint64_t)sizeof(wchar_t);
  if (bytes64 > (uint64_t)UINT32_MAX) rt_trap(what);
  uint32_t bytes = (uint32_t)bytes64;
  wchar_t* out = (wchar_t*)rt_alloc(ctx, bytes, (uint32_t)_Alignof(wchar_t));
  int wlen2 = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, s, (int)s_len, out, wlen);
  if (wlen2 != wlen) rt_trap(what);
  out[wlen] = 0;
  if (out_bytes) *out_bytes = bytes;
  return out;
}

static uint32_t rt_os_win_arg_needs_quotes(const wchar_t* s) {
  if (!s || !*s) return 1;
  for (uint32_t i = 0; s[i]; i++) {
    wchar_t c = s[i];
    if (c == L' ' || c == L'\t' || c == L'"') return 1;
  }
  return 0;
}

static void rt_os_win_cmdline_append_wchar(wchar_t* out, uint32_t cap, uint32_t* at, wchar_t c) {
  if (!out || !at) rt_trap("os.process windows cmdline internal error");
  if (*at >= cap) rt_trap("os.process windows cmdline overflow");
  out[*at] = c;
  *at += 1;
}

static void rt_os_win_cmdline_append_str(wchar_t* out, uint32_t cap, uint32_t* at, const wchar_t* s) {
  uint32_t n = rt_os_win_wcs_len(s);
  for (uint32_t i = 0; i < n; i++) {
    rt_os_win_cmdline_append_wchar(out, cap, at, s[i]);
  }
}

static void rt_os_win_cmdline_append_arg(wchar_t* out, uint32_t cap, uint32_t* at, const wchar_t* arg) {
  uint32_t needs = rt_os_win_arg_needs_quotes(arg);
  if (!needs) {
    rt_os_win_cmdline_append_str(out, cap, at, arg);
    return;
  }

  rt_os_win_cmdline_append_wchar(out, cap, at, L'"');

  uint32_t i = 0;
  while (arg && arg[i]) {
    uint32_t bs = 0;
    while (arg[i] == L'\\') {
      bs += 1;
      i += 1;
    }

    if (arg[i] == 0) {
      for (uint32_t j = 0; j < bs * 2; j++) rt_os_win_cmdline_append_wchar(out, cap, at, L'\\');
      break;
    }

    if (arg[i] == L'"') {
      for (uint32_t j = 0; j < bs * 2 + 1; j++) rt_os_win_cmdline_append_wchar(out, cap, at, L'\\');
      rt_os_win_cmdline_append_wchar(out, cap, at, L'"');
      i += 1;
      continue;
    }

    for (uint32_t j = 0; j < bs; j++) rt_os_win_cmdline_append_wchar(out, cap, at, L'\\');
    rt_os_win_cmdline_append_wchar(out, cap, at, arg[i]);
    i += 1;
  }

  rt_os_win_cmdline_append_wchar(out, cap, at, L'"');
}

static wchar_t* rt_os_win_build_cmdline(
    ctx_t* ctx,
    char** argv,
    uint32_t* argv_sizes,
    uint32_t argv_count,
    uint32_t* out_bytes
) {
  uint64_t cap64 = 16;
  for (uint32_t i = 0; i < argv_count; i++) {
    uint32_t n = argv_sizes ? argv_sizes[i] : 0;
    if (n > 0) n -= 1;
    cap64 += (uint64_t)(n * 2) + 4;
  }
  if (cap64 > (uint64_t)UINT32_MAX) rt_trap("os.process windows cmdline too long");
  uint32_t cap = (uint32_t)cap64;

  uint64_t bytes64 = ((uint64_t)(cap + 1)) * (uint64_t)sizeof(wchar_t);
  if (bytes64 > (uint64_t)UINT32_MAX) rt_trap("os.process windows cmdline too long");
  uint32_t bytes = (uint32_t)bytes64;

  wchar_t* out = (wchar_t*)rt_alloc(ctx, bytes, (uint32_t)_Alignof(wchar_t));
  uint32_t at = 0;

  for (uint32_t i = 0; i < argv_count; i++) {
    if (i != 0) rt_os_win_cmdline_append_wchar(out, cap, &at, L' ');
    uint32_t n = argv_sizes ? argv_sizes[i] : 0;
    if (n > 0) n -= 1;
    uint32_t wbytes = 0;
    wchar_t* argw = rt_os_win_utf8_to_wide_alloc(
      ctx,
      argv[i],
      n,
      "os.process argv must be valid utf8 on windows",
      &wbytes
    );
    rt_os_win_cmdline_append_arg(out, cap, &at, argw);
    rt_free(ctx, argw, wbytes, (uint32_t)_Alignof(wchar_t));
  }

  if (at > cap) rt_trap("os.process windows cmdline overflow");
  out[at] = 0;
  if (out_bytes) *out_bytes = bytes;
  return out;
}

static wchar_t* rt_os_win_build_env_block(
    ctx_t* ctx,
    char** envp,
    uint32_t* env_sizes,
    uint32_t env_count,
    uint32_t* out_bytes
) {
  if (env_count == 0) {
    uint32_t bytes = (uint32_t)(2u * (uint32_t)sizeof(wchar_t));
    wchar_t* block = (wchar_t*)rt_alloc(ctx, bytes, (uint32_t)_Alignof(wchar_t));
    block[0] = 0;
    block[1] = 0;
    if (out_bytes) *out_bytes = bytes;
    return block;
  }

  uint64_t cap64 = 8;
  for (uint32_t i = 0; i < env_count; i++) {
    uint32_t n = env_sizes ? env_sizes[i] : 0;
    if (n > 0) n -= 1;
    cap64 += (uint64_t)(n * 2) + 2;
  }
  if (cap64 > (uint64_t)UINT32_MAX) rt_trap("os.process windows env block too long");
  uint32_t cap = (uint32_t)cap64;

  uint64_t bytes64 = ((uint64_t)(cap + 1)) * (uint64_t)sizeof(wchar_t);
  if (bytes64 > (uint64_t)UINT32_MAX) rt_trap("os.process windows env block too long");
  uint32_t bytes = (uint32_t)bytes64;

  wchar_t* block = (wchar_t*)rt_alloc(ctx, bytes, (uint32_t)_Alignof(wchar_t));
  uint32_t at = 0;

  for (uint32_t i = 0; i < env_count; i++) {
    uint32_t n = env_sizes ? env_sizes[i] : 0;
    if (n > 0) n -= 1;
    uint32_t wbytes = 0;
    wchar_t* w = rt_os_win_utf8_to_wide_alloc(
      ctx,
      envp[i],
      n,
      "os.process env must be valid utf8 on windows",
      &wbytes
    );
    uint32_t wlen = rt_os_win_wcs_len(w);
    if (at + wlen + 2 > cap) rt_trap("os.process windows env block overflow");
    for (uint32_t j = 0; j < wlen; j++) block[at++] = w[j];
    block[at++] = 0;
    rt_free(ctx, w, wbytes, (uint32_t)_Alignof(wchar_t));
  }

  block[at++] = 0;
  if (out_bytes) *out_bytes = bytes;
  return block;
}

static DWORD WINAPI rt_os_win_stdout_thread(LPVOID arg) {
  rt_os_proc_t* p = (rt_os_proc_t*)arg;
  uint8_t buf[4096];

  for (;;) {
    HANDLE h = NULL;
    rt_os_proc_lock(p);
    h = p->win_stdout;
    rt_os_proc_unlock(p);
    if (!h) break;

    DWORD got = 0;
    BOOL ok = ReadFile(h, buf, (DWORD)sizeof(buf), &got, NULL);
    if (!ok || got == 0) break;

    rt_os_proc_lock(p);
    if (p->stdout_closed || p->fail_code != 0) {
      rt_os_proc_unlock(p);
      break;
    }

    uint32_t total_len = p->stdout_len + p->stderr_len;
    uint32_t total_rem = (total_len < p->max_total) ? (p->max_total - total_len) : 0;
    uint32_t rem_total = (p->stdout_len < p->max_stdout) ? (p->max_stdout - p->stdout_len) : 0;
    if (rem_total > total_rem) rem_total = total_rem;
    if (rem_total == 0) {
      p->fail_code = RT_OS_PROC_CODE_OUTPUT_LIMIT;
      rt_os_proc_unlock(p);
      break;
    }

    if (p->stdout_off != 0 && p->stdout_off + p->stdout_len == p->max_stdout) {
      memmove(p->stdout_buf.ptr, p->stdout_buf.ptr + p->stdout_off, p->stdout_len);
      p->stdout_off = 0;
    }

    uint32_t cont_rem = p->max_stdout - (p->stdout_off + p->stdout_len);
    uint32_t rem = rem_total;
    if (rem > cont_rem) rem = cont_rem;

    uint32_t n = (uint32_t)got;
    if (n > rem) n = rem;
    if (n != 0) {
      memcpy(p->stdout_buf.ptr + p->stdout_off + p->stdout_len, buf, n);
      p->stdout_len += n;
    }
    if ((uint32_t)got > n) {
      p->fail_code = RT_OS_PROC_CODE_OUTPUT_LIMIT;
    }
    rt_os_proc_unlock(p);
  }

  rt_os_proc_lock(p);
  rt_os_win_close_handle(&p->win_stdout);
  p->stdout_closed = 1;
  rt_os_proc_unlock(p);
  return 0;
}

static DWORD WINAPI rt_os_win_stderr_thread(LPVOID arg) {
  rt_os_proc_t* p = (rt_os_proc_t*)arg;
  uint8_t buf[4096];

  for (;;) {
    HANDLE h = NULL;
    rt_os_proc_lock(p);
    h = p->win_stderr;
    rt_os_proc_unlock(p);
    if (!h) break;

    DWORD got = 0;
    BOOL ok = ReadFile(h, buf, (DWORD)sizeof(buf), &got, NULL);
    if (!ok || got == 0) break;

    rt_os_proc_lock(p);
    if (p->stderr_closed || p->fail_code != 0) {
      rt_os_proc_unlock(p);
      break;
    }

    uint32_t total_len = p->stdout_len + p->stderr_len;
    uint32_t total_rem = (total_len < p->max_total) ? (p->max_total - total_len) : 0;
    uint32_t rem_total = (p->stderr_len < p->max_stderr) ? (p->max_stderr - p->stderr_len) : 0;
    if (rem_total > total_rem) rem_total = total_rem;
    if (rem_total == 0) {
      p->fail_code = RT_OS_PROC_CODE_OUTPUT_LIMIT;
      rt_os_proc_unlock(p);
      break;
    }

    if (p->stderr_off != 0 && p->stderr_off + p->stderr_len == p->max_stderr) {
      memmove(p->stderr_buf.ptr, p->stderr_buf.ptr + p->stderr_off, p->stderr_len);
      p->stderr_off = 0;
    }

    uint32_t cont_rem = p->max_stderr - (p->stderr_off + p->stderr_len);
    uint32_t rem = rem_total;
    if (rem > cont_rem) rem = cont_rem;

    uint32_t n = (uint32_t)got;
    if (n > rem) n = rem;
    if (n != 0) {
      memcpy(p->stderr_buf.ptr + p->stderr_off + p->stderr_len, buf, n);
      p->stderr_len += n;
    }
    if ((uint32_t)got > n) {
      p->fail_code = RT_OS_PROC_CODE_OUTPUT_LIMIT;
    }
    rt_os_proc_unlock(p);
  }

  rt_os_proc_lock(p);
  rt_os_win_close_handle(&p->win_stderr);
  p->stderr_closed = 1;
  rt_os_proc_unlock(p);
  return 0;
}

static DWORD WINAPI rt_os_win_stdin_thread(LPVOID arg) {
  rt_os_proc_t* p = (rt_os_proc_t*)arg;
  uint8_t buf[4096];

  for (;;) {
    HANDLE ev = NULL;
    rt_os_proc_lock(p);
    ev = p->win_stdin_event;
    rt_os_proc_unlock(p);
    if (!ev) break;

    DWORD wr = WaitForSingleObject(ev, INFINITE);
    if (wr != WAIT_OBJECT_0) return 0;

    for (;;) {
      HANDLE h = NULL;
      uint32_t n = 0;

      rt_os_proc_lock(p);
      if (p->stdin_closed) {
        rt_os_proc_unlock(p);
        return 0;
      }
      h = p->win_stdin;
      if (!h) {
        p->stdin_closed = 1;
        rt_os_proc_unlock(p);
        return 0;
      }

      uint32_t pending = 0;
      if (p->stdin_off < p->stdin_buf.len) pending = p->stdin_buf.len - p->stdin_off;
      if (pending == 0) {
        if (p->mode == RT_OS_PROC_MODE_CAPTURE) {
          rt_os_win_close_handle(&p->win_stdin);
          p->stdin_closed = 1;
          rt_os_proc_unlock(p);
          return 0;
        }
        rt_os_proc_unlock(p);
        break;
      }

      uint32_t off0 = p->stdin_off;
      n = pending;
      if (n > (uint32_t)sizeof(buf)) n = (uint32_t)sizeof(buf);
      memcpy(buf, p->stdin_buf.ptr + off0, n);
      rt_os_proc_unlock(p);

      DWORD wrote = 0;
      BOOL ok = WriteFile(h, buf, (DWORD)n, &wrote, NULL);
      if (!ok) {
        rt_os_proc_lock(p);
        rt_os_win_close_handle(&p->win_stdin);
        p->stdin_closed = 1;
        rt_os_proc_unlock(p);
        return 0;
      }
      if (wrote == 0) {
        rt_os_proc_lock(p);
        rt_os_win_close_handle(&p->win_stdin);
        p->stdin_closed = 1;
        rt_os_proc_unlock(p);
        return 0;
      }

      rt_os_proc_lock(p);
      if (!p->stdin_closed) {
        uint64_t next64 = (uint64_t)p->stdin_off + (uint64_t)wrote;
        p->stdin_off = (next64 > (uint64_t)p->stdin_buf.len) ? p->stdin_buf.len : (uint32_t)next64;
      }
      rt_os_proc_unlock(p);

      if ((uint32_t)wrote < n) {
        break;
      }
    }
  }

  return 0;
}

static void rt_os_win_proc_join_threads(rt_os_proc_t* p) {
  HANDLE th[3];
  th[0] = p->win_thread_stdout;
  th[1] = p->win_thread_stderr;
  th[2] = p->win_thread_stdin;
  for (uint32_t i = 0; i < 3; i++) {
    if (!th[i]) continue;
    (void)WaitForSingleObject(th[i], INFINITE);
    rt_os_win_close_handle(&th[i]);
  }
  p->win_thread_stdout = NULL;
  p->win_thread_stderr = NULL;
  p->win_thread_stdin = NULL;
}
#endif

static int32_t rt_os_process_spawn_impl(ctx_t* ctx, bytes_t req, bytes_t caps, uint32_t mode) {
  rt_os_policy_init(ctx);

  if (rt_os_sandboxed && rt_os_proc_max_spawns != 0 && ctx->os_procs_spawned >= rt_os_proc_max_spawns) {
    return (int32_t)((uint32_t)RT_OS_PROC_CODE_POLICY_DENIED << 16);
  }

  uint32_t idx = rt_os_proc_alloc_slot(ctx, UINT32_C(1));
  rt_os_proc_t* proc = &ctx->os_procs[idx];
  uint16_t gen = proc->gen;
  proc->mode = mode;

  uint32_t err = 0;

  uint32_t max_stdout = 0;
  uint32_t max_stderr = 0;
  uint32_t timeout_ms = 0;
  uint32_t max_total = 0;

  uint32_t off = 0;
  uint32_t argv_count = 0;
  uint32_t env_count = 0;

  bytes_t argv0 = rt_bytes_empty(ctx);
  bytes_t stdin_view = rt_bytes_empty(ctx);
  bytes_t cwd_view = rt_bytes_empty(ctx);
  char* cwd_cstr = NULL;
  uint32_t cwd_cstr_size = 0;

  char** argv = NULL;
  uint32_t* argv_sizes = NULL;
  char** envp = NULL;
  uint32_t* env_sizes = NULL;

#if defined(_WIN32)
  HANDLE win_stdin_read = NULL;
  HANDLE win_stdin_write = NULL;
  HANDLE win_stdout_read = NULL;
  HANDLE win_stdout_write = NULL;
  HANDLE win_stderr_read = NULL;
  HANDLE win_stderr_write = NULL;

  PROCESS_INFORMATION win_pi = (PROCESS_INFORMATION){0};

  HANDLE win_job = NULL;
  HANDLE win_stdin_event = NULL;

  wchar_t* win_exe = NULL;
  uint32_t win_exe_bytes = 0;
  wchar_t* win_cmdline = NULL;
  uint32_t win_cmdline_bytes = 0;
  wchar_t* win_env_block = NULL;
  uint32_t win_env_block_bytes = 0;
  wchar_t* win_cwd = NULL;
  uint32_t win_cwd_bytes = 0;
#else
  int stdin_pipe[2] = {-1, -1};
  int stdout_pipe[2] = {-1, -1};
  int stderr_pipe[2] = {-1, -1};
  int exec_pipe[2] = {-1, -1};

  int stdin_fd = -1;
  int stdout_fd = -1;
  int stderr_fd = -1;

  pid_t pid = (pid_t)-1;
  pid_t pgid = (pid_t)-1;
  int status = 0;
#endif

  bytes_t stdin_copy = rt_bytes_empty(ctx);
  bytes_t stdout_buf = rt_bytes_empty(ctx);
  bytes_t stderr_buf = rt_bytes_empty(ctx);

  uint32_t cwd_len = 0;
  uint32_t stdin_len = 0;

  if (rt_os_sandboxed) {
    if (!rt_os_proc_enabled || !rt_os_proc_allow_spawn) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }

    if (rt_os_proc_max_spawns != 0) {
      if (ctx->os_procs_spawned >= rt_os_proc_max_spawns) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
      ctx->os_procs_spawned += 1;
    }

    if (rt_os_proc_max_live != 0 && ctx->os_procs_live >= rt_os_proc_max_live) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
  }

  // Parse caps (ProcCapsV1).
  if (caps.len < UINT32_C(17) || caps.ptr[0] != UINT32_C(1)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  max_stdout = rt_read_u32_le(caps.ptr + 1);
  max_stderr = rt_read_u32_le(caps.ptr + 5);
  timeout_ms = rt_read_u32_le(caps.ptr + 9);
  max_total = rt_read_u32_le(caps.ptr + 13);
  if (max_total == 0) {
    uint64_t sum = (uint64_t)max_stdout + (uint64_t)max_stderr;
    max_total = (sum > (uint64_t)UINT32_MAX) ? UINT32_MAX : (uint32_t)sum;
  }
  if (rt_os_sandboxed && rt_os_proc_max_runtime_ms != 0) {
    if (timeout_ms == 0) {
      timeout_ms = rt_os_proc_max_runtime_ms;
    } else if (timeout_ms > rt_os_proc_max_runtime_ms) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
  }
  if (rt_os_sandboxed) {
    if (rt_os_proc_max_stdout_bytes != 0 && max_stdout > rt_os_proc_max_stdout_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (rt_os_proc_max_stderr_bytes != 0 && max_stderr > rt_os_proc_max_stderr_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (rt_os_proc_max_total_bytes != 0 && max_total > rt_os_proc_max_total_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
  }

  // Parse request (ProcReqV1).
  if (req.len < UINT32_C(6) || req.ptr[0] != UINT32_C(1)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }

  // v1 currently requires flags=0 (no env inheritance/clear toggles yet).
  if (req.ptr[1] != UINT32_C(0)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }

  off = UINT32_C(2);
  argv_count = rt_read_u32_le(req.ptr + off);
  off += 4;
  if (argv_count == 0 || argv_count > UINT32_C(128)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  if (rt_os_sandboxed && rt_os_proc_max_args != 0 && argv_count > rt_os_proc_max_args) {
    err = RT_OS_PROC_CODE_POLICY_DENIED;
    goto cleanup;
  }

  argv = (char**)rt_alloc(
      ctx,
      (argv_count + 1) * (uint32_t)sizeof(char*),
      (uint32_t)_Alignof(char*)
  );
  argv_sizes = (uint32_t*)rt_alloc(
      ctx,
      argv_count * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
  );
  for (uint32_t i = 0; i < argv_count; i++) argv[i] = NULL;

  for (uint32_t i = 0; i < argv_count; i++) {
    if (off > req.len || req.len - off < UINT32_C(4)) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    uint32_t n = rt_read_u32_le(req.ptr + off);
    off += 4;
    if (rt_os_sandboxed && rt_os_proc_max_arg_bytes != 0 && n > rt_os_proc_max_arg_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (off > req.len || req.len - off < n) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    bytes_t b;
    b.ptr = req.ptr + off;
    b.len = n;
    if (rt_os_sandboxed && i != 0) {
      if (!rt_os_proc_args_allowed(b)) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
    }
    if (i == 0) argv0 = b;
    off += n;
    argv[i] = rt_os_bytes_to_cstr(ctx, b, "os.process.spawn_capture_v1 argv contains NUL");
    argv_sizes[i] = n + 1;
  }
  argv[argv_count] = NULL;

  if (argv0.len == 0) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  if (rt_os_sandboxed && rt_os_proc_max_exe_bytes != 0 && argv0.len > rt_os_proc_max_exe_bytes) {
    err = RT_OS_PROC_CODE_POLICY_DENIED;
    goto cleanup;
  }

  if (rt_os_sandboxed) {
    uint32_t allow = UINT32_C(0);
    if (rt_os_list_contains(rt_os_proc_allow_execs, argv0)) allow = UINT32_C(1);
    if (!allow && rt_os_list_contains_prefix(rt_os_proc_allow_exec_prefixes, argv0)) {
      allow = UINT32_C(1);
    }
    if (!allow) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
  }

  // env
  if (off > req.len || req.len - off < UINT32_C(4)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  env_count = rt_read_u32_le(req.ptr + off);
  off += 4;
  if (env_count > UINT32_C(256)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  if (rt_os_sandboxed && rt_os_proc_max_env != 0 && env_count > rt_os_proc_max_env) {
    err = RT_OS_PROC_CODE_POLICY_DENIED;
    goto cleanup;
  }

  envp = (char**)rt_alloc(
      ctx,
      (env_count + 1) * (uint32_t)sizeof(char*),
      (uint32_t)_Alignof(char*)
  );
  env_sizes = (uint32_t*)rt_alloc(
      ctx,
      env_count * (uint32_t)sizeof(uint32_t),
      (uint32_t)_Alignof(uint32_t)
  );
  for (uint32_t i = 0; i < env_count; i++) envp[i] = NULL;

  for (uint32_t i = 0; i < env_count; i++) {
    if (off > req.len || req.len - off < UINT32_C(4)) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    uint32_t klen = rt_read_u32_le(req.ptr + off);
    off += 4;
    if (rt_os_sandboxed && rt_os_proc_max_env_key_bytes != 0 && klen > rt_os_proc_max_env_key_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (off > req.len || req.len - off < klen) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    bytes_t k;
    k.ptr = req.ptr + off;
    k.len = klen;
    off += klen;

    if (off > req.len || req.len - off < UINT32_C(4)) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    uint32_t vlen = rt_read_u32_le(req.ptr + off);
    off += 4;
    if (rt_os_sandboxed && rt_os_proc_max_env_val_bytes != 0 && vlen > rt_os_proc_max_env_val_bytes) {
      err = RT_OS_PROC_CODE_POLICY_DENIED;
      goto cleanup;
    }
    if (off > req.len || req.len - off < vlen) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    bytes_t v;
    v.ptr = req.ptr + off;
    v.len = vlen;
    off += vlen;

    if (k.len == 0) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    if (rt_os_sandboxed) {
      if (!rt_os_list_contains(rt_os_proc_allow_env_keys, k)) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
    }

    // key=value\0
    uint64_t need64 = (uint64_t)k.len + UINT64_C(1) + (uint64_t)v.len + UINT64_C(1);
    if (need64 > (uint64_t)UINT32_MAX) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    uint32_t need = (uint32_t)need64;

    char* kv = (char*)rt_alloc(ctx, need, 1);
    // Validate no NULs.
    for (uint32_t j = 0; j < k.len; j++) {
      if (k.ptr[j] == 0) rt_trap("os.process.spawn_capture_v1 env key contains NUL");
    }
    for (uint32_t j = 0; j < v.len; j++) {
      if (v.ptr[j] == 0) rt_trap("os.process.spawn_capture_v1 env val contains NUL");
    }

    if (k.len != 0) {
      memcpy(kv, k.ptr, k.len);
      rt_mem_on_memcpy(ctx, k.len);
    }
    kv[k.len] = '=';
    if (v.len != 0) {
      memcpy(kv + k.len + 1, v.ptr, v.len);
      rt_mem_on_memcpy(ctx, v.len);
    }
    kv[k.len + 1 + v.len] = 0;

    envp[i] = kv;
    env_sizes[i] = need;
  }
  envp[env_count] = NULL;

  // cwd override (v1 currently requires cwd_len=0).
  if (off > req.len || req.len - off < UINT32_C(4)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  cwd_len = rt_read_u32_le(req.ptr + off);
  off += 4;
  if (cwd_len != 0) {
    if (off > req.len || req.len - off < cwd_len) {
      err = RT_OS_PROC_CODE_INVALID_REQUEST;
      goto cleanup;
    }
    cwd_view.ptr = req.ptr + off;
    cwd_view.len = cwd_len;
    off += cwd_len;
    for (uint32_t j = 0; j < cwd_view.len; j++) {
      if (cwd_view.ptr[j] == 0) rt_trap("os.process.spawn_capture_v1 cwd contains NUL");
    }
    if (rt_os_sandboxed) {
      if (!rt_os_proc_allow_cwd) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
      if (!rt_os_proc_allow_cwd_roots || !*rt_os_proc_allow_cwd_roots) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
      bytes_view_t path_view = rt_bytes_view(ctx, cwd_view);
      if (!rt_fs_is_safe_rel_path(path_view)) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
      if (rt_os_deny_hidden && rt_os_path_has_hidden_segment(cwd_view)) {
        err = RT_OS_PROC_CODE_POLICY_DENIED;
        goto cleanup;
      }
    } else {
      cwd_cstr = rt_os_bytes_to_cstr(ctx, cwd_view, "os.process.spawn_capture_v1 cwd contains NUL");
      cwd_cstr_size = cwd_view.len + 1;
    }
  }

  // stdin bytes
  if (off > req.len || req.len - off < UINT32_C(4)) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  stdin_len = rt_read_u32_le(req.ptr + off);
  off += 4;
  if (rt_os_sandboxed && rt_os_proc_max_stdin_bytes != 0 && stdin_len > rt_os_proc_max_stdin_bytes) {
    err = RT_OS_PROC_CODE_POLICY_DENIED;
    goto cleanup;
  }
  if (off > req.len || req.len - off < stdin_len) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }
  stdin_view.ptr = req.ptr + off;
  stdin_view.len = stdin_len;
  off += stdin_len;
  if (off != req.len) {
    err = RT_OS_PROC_CODE_INVALID_REQUEST;
    goto cleanup;
  }

  stdin_copy = rt_bytes_alloc(ctx, stdin_len);
  if (stdin_len != 0) {
    memcpy(stdin_copy.ptr, stdin_view.ptr, stdin_len);
    rt_mem_on_memcpy(ctx, stdin_len);
  }

#if defined(_WIN32)
  SECURITY_ATTRIBUTES sa = (SECURITY_ATTRIBUTES){0};
  sa.nLength = (DWORD)sizeof(sa);
  sa.lpSecurityDescriptor = NULL;
  sa.bInheritHandle = TRUE;

  if (!CreatePipe(&win_stdin_read, &win_stdin_write, &sa, 0)) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  if (!SetHandleInformation(win_stdin_write, HANDLE_FLAG_INHERIT, 0)) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  if (!CreatePipe(&win_stdout_read, &win_stdout_write, &sa, 0)) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  if (!SetHandleInformation(win_stdout_read, HANDLE_FLAG_INHERIT, 0)) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  if (!CreatePipe(&win_stderr_read, &win_stderr_write, &sa, 0)) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  if (!SetHandleInformation(win_stderr_read, HANDLE_FLAG_INHERIT, 0)) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  if (rt_os_proc_kill_tree) {
    win_job = CreateJobObjectW(NULL, NULL);
    if (!win_job) {
      err = RT_OS_PROC_CODE_SPAWN_FAILED;
      goto cleanup;
    }
    (void)SetHandleInformation(win_job, HANDLE_FLAG_INHERIT, 0);

    JOBOBJECT_EXTENDED_LIMIT_INFORMATION jeli = (JOBOBJECT_EXTENDED_LIMIT_INFORMATION){0};
    jeli.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!SetInformationJobObject(
            win_job,
            JobObjectExtendedLimitInformation,
            &jeli,
            (DWORD)sizeof(jeli))) {
      err = RT_OS_PROC_CODE_SPAWN_FAILED;
      goto cleanup;
    }
  }

  uint32_t argv0_len = argv_sizes ? argv_sizes[0] : 0;
  if (argv0_len > 0) argv0_len -= 1;
  win_exe = rt_os_win_utf8_to_wide_alloc(
    ctx,
    argv[0],
    argv0_len,
    "os.process argv must be valid utf8 on windows",
    &win_exe_bytes
  );
  for (uint32_t i = 0; win_exe[i]; i++) {
    if (win_exe[i] == L'/') win_exe[i] = L'\\';
  }

  win_cmdline = rt_os_win_build_cmdline(ctx, argv, argv_sizes, argv_count, &win_cmdline_bytes);
  win_env_block = rt_os_win_build_env_block(ctx, envp, env_sizes, env_count, &win_env_block_bytes);

  if (cwd_len != 0) {
    if (rt_os_sandboxed) {
      uint32_t ok = 0;
      const char* cur = rt_os_proc_allow_cwd_roots;
      const char* root = NULL;
      size_t root_len = 0;
      while (rt_os_split_next(&cur, &root, &root_len)) {
        char* p = rt_os_join_root_and_rel(ctx, root, root_len, cwd_view);
        uint32_t pn = (uint32_t)strlen(p);
        uint32_t wbytes = 0;
        wchar_t* w = rt_os_win_utf8_to_wide_alloc(
          ctx,
          p,
          pn,
          "os.process cwd must be valid utf8 on windows",
          &wbytes
        );
        for (uint32_t j = 0; w[j]; j++) {
          if (w[j] == L'/') w[j] = L'\\';
        }
        DWORD attrs = GetFileAttributesW(w);
        if (attrs != INVALID_FILE_ATTRIBUTES && (attrs & FILE_ATTRIBUTE_DIRECTORY)) {
          ok = 1;
          win_cwd = w;
          win_cwd_bytes = wbytes;
          rt_free(ctx, p, pn + 1, 1);
          break;
        }
        rt_free(ctx, p, pn + 1, 1);
        rt_free(ctx, w, wbytes, (uint32_t)_Alignof(wchar_t));
      }
      if (!ok) {
        err = RT_OS_PROC_CODE_SPAWN_FAILED;
        goto cleanup;
      }
    } else {
      uint32_t n = cwd_cstr_size ? (cwd_cstr_size - 1) : 0;
      win_cwd = rt_os_win_utf8_to_wide_alloc(
        ctx,
        cwd_cstr,
        n,
        "os.process cwd must be valid utf8 on windows",
        &win_cwd_bytes
      );
      for (uint32_t j = 0; win_cwd[j]; j++) {
        if (win_cwd[j] == L'/') win_cwd[j] = L'\\';
      }
    }
  }

  uint32_t need_stdin_thread = (mode == RT_OS_PROC_MODE_PIPED || stdin_len != 0) ? UINT32_C(1) : UINT32_C(0);
  if (need_stdin_thread) {
    win_stdin_event = CreateEventW(NULL, FALSE, FALSE, NULL);
    if (!win_stdin_event) {
      err = RT_OS_PROC_CODE_SPAWN_FAILED;
      goto cleanup;
    }
    (void)SetHandleInformation(win_stdin_event, HANDLE_FLAG_INHERIT, 0);
  }

  DWORD flags = CREATE_UNICODE_ENVIRONMENT;
  if (rt_os_proc_kill_tree) flags |= CREATE_SUSPENDED;

  STARTUPINFOW si = (STARTUPINFOW){0};
  si.cb = (DWORD)sizeof(si);
  si.dwFlags = STARTF_USESTDHANDLES;
  si.hStdInput = win_stdin_read;
  si.hStdOutput = win_stdout_write;
  si.hStdError = win_stderr_write;

  if (!CreateProcessW(
        win_exe,
        win_cmdline,
        NULL,
        NULL,
        TRUE,
        flags,
        (LPVOID)win_env_block,
        win_cwd,
        &si,
        &win_pi)) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  if (win_job) {
    if (!AssignProcessToJobObject(win_job, win_pi.hProcess)) {
      err = RT_OS_PROC_CODE_SPAWN_FAILED;
      goto cleanup;
    }
  }

  if (flags & CREATE_SUSPENDED) {
    DWORD r = ResumeThread(win_pi.hThread);
    if (r == (DWORD)-1) {
      err = RT_OS_PROC_CODE_SPAWN_FAILED;
      goto cleanup;
    }
  }

  rt_os_win_close_handle(&win_pi.hThread);
  rt_os_win_close_handle(&win_stdin_read);
  rt_os_win_close_handle(&win_stdout_write);
  rt_os_win_close_handle(&win_stderr_write);

  stdout_buf = rt_bytes_alloc(ctx, max_stdout);
  stderr_buf = rt_bytes_alloc(ctx, max_stderr);

  proc->mode = mode;
  proc->win_proc = win_pi.hProcess;
  win_pi.hProcess = NULL;
  proc->win_job = win_job;
  win_job = NULL;
  proc->win_stdin = win_stdin_write;
  win_stdin_write = NULL;
  proc->win_stdout = win_stdout_read;
  win_stdout_read = NULL;
  proc->win_stderr = win_stderr_read;
  win_stderr_read = NULL;

  proc->stdin_closed = 0;
  proc->stdout_closed = 0;
  proc->stderr_closed = 0;

  proc->exited = 0;
  proc->status = 0;

  proc->fail_code = 0;
  proc->kill_sent = 0;

  proc->max_stdout = max_stdout;
  proc->max_stderr = max_stderr;
  proc->max_total = max_total;
  proc->timeout_ms = timeout_ms;
  proc->start_ms = rt_os_now_ms();

  proc->stdin_buf = stdin_copy;
  proc->stdin_off = 0;
  proc->stdout_buf = stdout_buf;
  proc->stdout_len = 0;
  proc->stderr_buf = stderr_buf;
  proc->stderr_len = 0;

  stdin_copy = rt_bytes_empty(ctx);
  stdout_buf = rt_bytes_empty(ctx);
  stderr_buf = rt_bytes_empty(ctx);

  proc->win_thread_stdout = CreateThread(NULL, 0, rt_os_win_stdout_thread, proc, 0, NULL);
  if (!proc->win_thread_stdout) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  proc->win_thread_stderr = CreateThread(NULL, 0, rt_os_win_stderr_thread, proc, 0, NULL);
  if (!proc->win_thread_stderr) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  if (mode == RT_OS_PROC_MODE_CAPTURE && proc->stdin_buf.len == 0) {
    rt_os_win_close_handle(&proc->win_stdin);
    proc->stdin_closed = 1;
  } else {
    proc->win_stdin_event = win_stdin_event;
    win_stdin_event = NULL;
    proc->win_thread_stdin = CreateThread(NULL, 0, rt_os_win_stdin_thread, proc, 0, NULL);
    if (!proc->win_thread_stdin) {
      err = RT_OS_PROC_CODE_SPAWN_FAILED;
      goto cleanup;
    }
    if (proc->win_stdin_event) (void)SetEvent(proc->win_stdin_event);
  }

  proc->state = RT_OS_PROC_STATE_RUNNING;

  rt_os_spawn_free_argv_env(ctx, argv, argv_sizes, argv_count, envp, env_sizes, env_count);
  argv = NULL;
  argv_sizes = NULL;
  envp = NULL;
  env_sizes = NULL;
  if (cwd_cstr) {
    rt_free(ctx, cwd_cstr, cwd_cstr_size, 1);
    cwd_cstr = NULL;
    cwd_cstr_size = 0;
  }
  if (win_exe) {
    rt_free(ctx, win_exe, win_exe_bytes, (uint32_t)_Alignof(wchar_t));
    win_exe = NULL;
    win_exe_bytes = 0;
  }
  if (win_cmdline) {
    rt_free(ctx, win_cmdline, win_cmdline_bytes, (uint32_t)_Alignof(wchar_t));
    win_cmdline = NULL;
    win_cmdline_bytes = 0;
  }
  if (win_env_block) {
    rt_free(ctx, win_env_block, win_env_block_bytes, (uint32_t)_Alignof(wchar_t));
    win_env_block = NULL;
    win_env_block_bytes = 0;
  }
  if (win_cwd) {
    rt_free(ctx, win_cwd, win_cwd_bytes, (uint32_t)_Alignof(wchar_t));
    win_cwd = NULL;
    win_cwd_bytes = 0;
  }

  ctx->os_procs_live += 1;
  return rt_os_proc_handle_i32(idx, gen);
#else
  if (pipe(stdin_pipe) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  if (pipe(stdout_pipe) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  if (pipe(stderr_pipe) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  if (pipe(exec_pipe) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  if (rt_os_set_cloexec(stdin_pipe[0]) != 0
      || rt_os_set_cloexec(stdin_pipe[1]) != 0
      || rt_os_set_cloexec(stdout_pipe[0]) != 0
      || rt_os_set_cloexec(stdout_pipe[1]) != 0
      || rt_os_set_cloexec(stderr_pipe[0]) != 0
      || rt_os_set_cloexec(stderr_pipe[1]) != 0
      || rt_os_set_cloexec(exec_pipe[0]) != 0
      || rt_os_set_cloexec(exec_pipe[1]) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  pid = fork();
  if (pid < 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }
  if (pid == 0) {
    // Child.
    rt_os_close_fd(exec_pipe[0]);
    exec_pipe[0] = -1;

    if (dup2(stdin_pipe[0], STDIN_FILENO) < 0
        || dup2(stdout_pipe[1], STDOUT_FILENO) < 0
        || dup2(stderr_pipe[1], STDERR_FILENO) < 0) {
      int e = errno ? errno : 1;
      (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
      _exit(127);
    }

    rt_os_close_fd(stdin_pipe[0]);
    rt_os_close_fd(stdin_pipe[1]);
    rt_os_close_fd(stdout_pipe[0]);
    rt_os_close_fd(stdout_pipe[1]);
    rt_os_close_fd(stderr_pipe[0]);
    rt_os_close_fd(stderr_pipe[1]);
    stdin_pipe[0] = stdin_pipe[1] = -1;
    stdout_pipe[0] = stdout_pipe[1] = -1;
    stderr_pipe[0] = stderr_pipe[1] = -1;

    if (rt_os_proc_kill_tree) {
      if (setpgid(0, 0) != 0) {
        int e = errno ? errno : 1;
        (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
        _exit(127);
      }
    }

    if (cwd_len != 0) {
      if (rt_os_sandboxed) {
        uint32_t ok = 0;
        const char* cur = rt_os_proc_allow_cwd_roots;
        const char* root = NULL;
        size_t root_len = 0;
        while (rt_os_split_next(&cur, &root, &root_len)) {
          char* p = rt_os_join_root_and_rel(ctx, root, root_len, cwd_view);
          if (chdir(p) == 0) {
            ok = 1;
            break;
          }
        }
        if (!ok) {
          int e = errno ? errno : 1;
          (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
          _exit(127);
        }
      } else {
        if (chdir(cwd_cstr) != 0) {
          int e = errno ? errno : 1;
          (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
          _exit(127);
        }
      }
    }

    execve(argv[0], argv, envp);
    int e = errno ? errno : 1;
    (void)write(exec_pipe[1], &e, (uint32_t)sizeof(e));
    _exit(127);
  }

  // Parent.
  if (rt_os_proc_kill_tree) pgid = pid;

  rt_os_close_fd(stdin_pipe[0]);
  stdin_pipe[0] = -1;
  rt_os_close_fd(stdout_pipe[1]);
  stdout_pipe[1] = -1;
  rt_os_close_fd(stderr_pipe[1]);
  stderr_pipe[1] = -1;
  rt_os_close_fd(exec_pipe[1]);
  exec_pipe[1] = -1;

  int child_errno = 0;
  ssize_t er = read(exec_pipe[0], &child_errno, (uint32_t)sizeof(child_errno));
  rt_os_close_fd(exec_pipe[0]);
  exec_pipe[0] = -1;
  if (er > 0) {
    (void)waitpid(pid, &status, 0);
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  stdin_fd = stdin_pipe[1];
  stdout_fd = stdout_pipe[0];
  stderr_fd = stderr_pipe[0];

  if (rt_os_set_nonblock(stdin_fd) != 0
      || rt_os_set_nonblock(stdout_fd) != 0
      || rt_os_set_nonblock(stderr_fd) != 0) {
    err = RT_OS_PROC_CODE_SPAWN_FAILED;
    goto cleanup;
  }

  stdout_buf = rt_bytes_alloc(ctx, max_stdout);
  stderr_buf = rt_bytes_alloc(ctx, max_stderr);

  proc->state = RT_OS_PROC_STATE_RUNNING;
  proc->mode = mode;
  proc->pid = pid;
  proc->pgid = pgid;
  proc->stdin_fd = stdin_fd;
  proc->stdout_fd = stdout_fd;
  proc->stderr_fd = stderr_fd;

  proc->stdin_closed = 0;
  proc->stdout_closed = 0;
  proc->stderr_closed = 0;

  proc->exited = 0;
  proc->status = 0;

  proc->fail_code = 0;
  proc->kill_sent = 0;

  proc->max_stdout = max_stdout;
  proc->max_stderr = max_stderr;
  proc->max_total = max_total;
  proc->timeout_ms = timeout_ms;
  proc->start_ms = rt_os_now_ms();

  proc->stdin_buf = stdin_copy;
  proc->stdin_off = 0;
  proc->stdout_buf = stdout_buf;
  proc->stdout_len = 0;
  proc->stderr_buf = stderr_buf;
  proc->stderr_len = 0;

  stdin_copy = rt_bytes_empty(ctx);
  stdout_buf = rt_bytes_empty(ctx);
  stderr_buf = rt_bytes_empty(ctx);
  rt_os_spawn_free_argv_env(ctx, argv, argv_sizes, argv_count, envp, env_sizes, env_count);
  argv = NULL;
  argv_sizes = NULL;
  envp = NULL;
  env_sizes = NULL;
  if (cwd_cstr) {
    rt_free(ctx, cwd_cstr, cwd_cstr_size, 1);
    cwd_cstr = NULL;
    cwd_cstr_size = 0;
  }
  stdin_fd = -1;
  stdout_fd = -1;
  stderr_fd = -1;

  if (mode == RT_OS_PROC_MODE_CAPTURE && proc->stdin_buf.len == 0) {
    rt_os_close_fd(proc->stdin_fd);
    proc->stdin_fd = -1;
    proc->stdin_closed = 1;
  }

  ctx->os_procs_live += 1;
  return rt_os_proc_handle_i32(idx, gen);

#endif

cleanup:
#if defined(_WIN32)
  if (win_pi.hProcess) {
    if (rt_os_proc_kill_tree && win_job) (void)TerminateJobObject(win_job, 1);
    (void)TerminateProcess(win_pi.hProcess, 1);
    (void)WaitForSingleObject(win_pi.hProcess, INFINITE);
  }

  rt_os_proc_kill_and_reap(proc);
  rt_os_proc_close_fds(proc);
  rt_os_win_proc_join_threads(proc);
  rt_os_proc_drop_buffers(ctx, proc);
  rt_os_proc_drop_result(ctx, proc);
  rt_os_win_close_handle(&proc->win_proc);
  rt_os_win_close_handle(&proc->win_job);
  rt_os_win_close_handle(&proc->win_stdin_event);

  rt_os_win_close_handle(&win_pi.hThread);
  rt_os_win_close_handle(&win_pi.hProcess);
  rt_os_win_close_handle(&win_stdin_read);
  rt_os_win_close_handle(&win_stdin_write);
  rt_os_win_close_handle(&win_stdout_read);
  rt_os_win_close_handle(&win_stdout_write);
  rt_os_win_close_handle(&win_stderr_read);
  rt_os_win_close_handle(&win_stderr_write);
  rt_os_win_close_handle(&win_stdin_event);
  rt_os_win_close_handle(&win_job);

  if (win_exe) rt_free(ctx, win_exe, win_exe_bytes, (uint32_t)_Alignof(wchar_t));
  if (win_cmdline) rt_free(ctx, win_cmdline, win_cmdline_bytes, (uint32_t)_Alignof(wchar_t));
  if (win_env_block) rt_free(ctx, win_env_block, win_env_block_bytes, (uint32_t)_Alignof(wchar_t));
  if (win_cwd) rt_free(ctx, win_cwd, win_cwd_bytes, (uint32_t)_Alignof(wchar_t));
#else
  rt_os_close_fd(stdin_fd);
  rt_os_close_fd(stdout_fd);
  rt_os_close_fd(stderr_fd);
  rt_os_close_fd(stdin_pipe[0]);
  rt_os_close_fd(stdin_pipe[1]);
  rt_os_close_fd(stdout_pipe[0]);
  rt_os_close_fd(stdout_pipe[1]);
  rt_os_close_fd(stderr_pipe[0]);
  rt_os_close_fd(stderr_pipe[1]);
  rt_os_close_fd(exec_pipe[0]);
  rt_os_close_fd(exec_pipe[1]);

  if (pid != (pid_t)-1 && err != 0) {
    (void)kill(pid, SIGKILL);
    (void)waitpid(pid, &status, 0);
  }
#endif

  rt_os_spawn_free_argv_env(ctx, argv, argv_sizes, argv_count, envp, env_sizes, env_count);
  if (cwd_cstr) rt_free(ctx, cwd_cstr, cwd_cstr_size, 1);

  rt_bytes_drop(ctx, &stdin_copy);
  rt_bytes_drop(ctx, &stdout_buf);
  rt_bytes_drop(ctx, &stderr_buf);

  if (err == 0) err = RT_OS_PROC_CODE_SPAWN_FAILED;
  rt_os_proc_mark_done(ctx, proc, idx, rt_os_proc_make_err(ctx, err));
  return rt_os_proc_handle_i32(idx, gen);
}

static int32_t rt_os_process_spawn_capture_v1(ctx_t* ctx, bytes_t req, bytes_t caps) {
  return rt_os_process_spawn_impl(ctx, req, caps, RT_OS_PROC_MODE_CAPTURE);
}

static int32_t rt_os_process_spawn_piped_v1(ctx_t* ctx, bytes_t req, bytes_t caps) {
  return rt_os_process_spawn_impl(ctx, req, caps, RT_OS_PROC_MODE_PIPED);
}

static option_bytes_t rt_os_process_try_join_capture_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    return (option_bytes_t){
      .tag = UINT32_C(1),
      .payload = rt_os_proc_make_err(ctx, sentinel_code),
    };
  }
  (void)rt_os_process_poll_all(ctx, 0);

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_CAPTURE) {
    rt_trap("os.process.try_join_capture_v1 invalid proc mode");
  }

  if (p->state != RT_OS_PROC_STATE_DONE) {
    return (option_bytes_t){ .tag = UINT32_C(0), .payload = rt_bytes_empty(ctx) };
  }
  if (p->result_taken) rt_trap("os.process join already taken");
  p->result_taken = 1;
  bytes_t out = p->result;
  p->result = rt_bytes_empty(ctx);
  return (option_bytes_t){ .tag = UINT32_C(1), .payload = out };
}

static uint32_t rt_os_process_join_capture_poll(ctx_t* ctx, int32_t handle, bytes_t* out) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    bytes_t doc = rt_os_proc_make_err(ctx, sentinel_code);
    if (out) {
      *out = doc;
    } else {
      rt_bytes_drop(ctx, &doc);
    }
    return UINT32_C(1);
  }

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_CAPTURE) {
    rt_trap("os.process.join_capture_v1 invalid proc mode");
  }

  if (p->state == RT_OS_PROC_STATE_DONE) {
    if (p->result_taken) rt_trap("os.process join already taken");
    p->result_taken = 1;
    if (out) {
      *out = p->result;
    } else {
      rt_bytes_drop(ctx, &p->result);
    }
    p->result = rt_bytes_empty(ctx);
    return UINT32_C(1);
  }

  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("os.process.join.poll from main");

  rt_task_t* me = rt_task_ptr(ctx, cur);
  uint32_t hid = (uint32_t)handle;
  if (me->wait_kind == RT_WAIT_OS_PROC_JOIN && me->wait_id == hid) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("os.process.join while already waiting");

  me->wait_kind = RT_WAIT_OS_PROC_JOIN;
  me->wait_id = hid;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_OS_PROC_JOIN << 32) | (uint64_t)hid);
  rt_wait_list_push(ctx, &p->join_wait_head, &p->join_wait_tail, cur);
  return UINT32_C(0);
}

static bytes_t rt_os_process_join_capture_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    return rt_os_proc_make_err(ctx, sentinel_code);
  }

  for (;;) {
    uint32_t idx = 0;
    rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
    (void)idx;

    if (p->mode != RT_OS_PROC_MODE_CAPTURE) {
      rt_trap("os.process.join_capture_v1 invalid proc mode");
    }

    if (p->state == RT_OS_PROC_STATE_DONE) {
      if (p->result_taken) rt_trap("os.process join already taken");
      p->result_taken = 1;
      bytes_t out = p->result;
      p->result = rt_bytes_empty(ctx);
      return out;
    }

    if (!rt_sched_step(ctx)) rt_sched_deadlock();
  }
}

static bytes_t rt_os_process_stdout_read_v1(ctx_t* ctx, int32_t handle, int32_t max) {
  rt_os_policy_init(ctx);
  if (max <= 0) return rt_bytes_empty(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return rt_bytes_empty(ctx);

  (void)rt_os_process_poll_all(ctx, 0);

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_PIPED) rt_trap("os.process.stdout_read_v1 invalid proc mode");


#if defined(_WIN32)
  rt_os_proc_lock(p);
#endif

  uint32_t avail = p->stdout_len;
  if (avail == 0) {
#if defined(_WIN32)
    rt_os_proc_unlock(p);
#endif
    return rt_bytes_empty(ctx);
  }

  uint32_t want = (uint32_t)max;
  uint32_t n = (avail < want) ? avail : want;
  bytes_t out = rt_bytes_alloc(ctx, n);
  if (n != 0) {
    memcpy(out.ptr, p->stdout_buf.ptr + p->stdout_off, n);
    rt_mem_on_memcpy(ctx, n);
  }
  p->stdout_off += n;
  p->stdout_len -= n;
  if (p->stdout_len == 0) p->stdout_off = 0;

#if defined(_WIN32)
  rt_os_proc_unlock(p);
#endif
  return out;
}

static bytes_t rt_os_process_stderr_read_v1(ctx_t* ctx, int32_t handle, int32_t max) {
  rt_os_policy_init(ctx);
  if (max <= 0) return rt_bytes_empty(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return rt_bytes_empty(ctx);

  (void)rt_os_process_poll_all(ctx, 0);

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_PIPED) rt_trap("os.process.stderr_read_v1 invalid proc mode");

#if defined(_WIN32)
  rt_os_proc_lock(p);
#endif

  uint32_t avail = p->stderr_len;
  if (avail == 0) {
#if defined(_WIN32)
    rt_os_proc_unlock(p);
#endif
    return rt_bytes_empty(ctx);
  }

  uint32_t want = (uint32_t)max;
  uint32_t n = (avail < want) ? avail : want;
  bytes_t out = rt_bytes_alloc(ctx, n);
  if (n != 0) {
    memcpy(out.ptr, p->stderr_buf.ptr + p->stderr_off, n);
    rt_mem_on_memcpy(ctx, n);
  }
  p->stderr_off += n;
  p->stderr_len -= n;
  if (p->stderr_len == 0) p->stderr_off = 0;

#if defined(_WIN32)
  rt_os_proc_unlock(p);
#endif
  return out;
}

static void rt_os_proc_stdin_append(ctx_t* ctx, rt_os_proc_t* p, bytes_t chunk) {
  if (chunk.len == 0) return;

  uint32_t pending = 0;
  if (p->stdin_off < p->stdin_buf.len) {
    pending = p->stdin_buf.len - p->stdin_off;
  }

  uint64_t total64 = (uint64_t)pending + (uint64_t)chunk.len;
  if (total64 > (uint64_t)UINT32_MAX) rt_trap("os.process.stdin_write_v1 pending overflow");
  uint32_t total = (uint32_t)total64;

  bytes_t b = rt_bytes_alloc(ctx, total);
  if (pending != 0) {
    memcpy(b.ptr, p->stdin_buf.ptr + p->stdin_off, pending);
    rt_mem_on_memcpy(ctx, pending);
  }
  memcpy(b.ptr + pending, chunk.ptr, chunk.len);
  rt_mem_on_memcpy(ctx, chunk.len);

  rt_bytes_drop(ctx, &p->stdin_buf);
  p->stdin_buf = b;
  p->stdin_off = 0;
}

static int32_t rt_os_process_stdin_write_v1(ctx_t* ctx, int32_t handle, bytes_t chunk) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 0;

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_PIPED) rt_trap("os.process.stdin_write_v1 invalid proc mode");
  if (p->state != RT_OS_PROC_STATE_RUNNING) return 0;

  if (rt_os_sandboxed && rt_os_proc_max_stdin_bytes != 0) {
    uint64_t total64 = 0;
#if defined(_WIN32)
    rt_os_proc_lock(p);
    if (p->stdin_closed || !p->win_stdin) {
      rt_os_proc_unlock(p);
      return 0;
    }
    uint32_t pending = 0;
    if (p->stdin_off < p->stdin_buf.len) pending = p->stdin_buf.len - p->stdin_off;
    total64 = (uint64_t)pending + (uint64_t)chunk.len;
    rt_os_proc_unlock(p);
#else
    (void)rt_os_process_poll_all(ctx, 0);
    if (p->stdin_fd < 0 || p->stdin_closed) return 0;

    uint32_t pending = 0;
    if (p->stdin_off < p->stdin_buf.len) pending = p->stdin_buf.len - p->stdin_off;
#endif
    total64 = (uint64_t)pending + (uint64_t)chunk.len;
    if (total64 > (uint64_t)rt_os_proc_max_stdin_bytes) {
      rt_trap("os.process.stdin_write_v1 pending exceeds policy max_stdin_bytes");
    }
  }

#if defined(_WIN32)
  rt_os_proc_lock(p);
  uint32_t is_closed = p->stdin_closed || !p->win_stdin;
  rt_os_proc_unlock(p);
  if (is_closed) return 0;

  rt_os_proc_lock(p);
  rt_os_proc_stdin_append(ctx, p, chunk);
  HANDLE ev = p->win_stdin_event;
  rt_os_proc_unlock(p);
  if (ev) (void)SetEvent(ev);
  return 1;
#else
  (void)rt_os_process_poll_all(ctx, 0);
  if (p->stdin_fd < 0 || p->stdin_closed) return 0;

  rt_os_proc_stdin_append(ctx, p, chunk);
  (void)rt_os_process_poll_all(ctx, 0);
  return (p->stdin_fd < 0 || p->stdin_closed) ? 0 : 1;
#endif
}

static int32_t rt_os_process_stdin_close_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 0;

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->mode != RT_OS_PROC_MODE_PIPED) rt_trap("os.process.stdin_close_v1 invalid proc mode");
#if defined(_WIN32)
  rt_os_proc_lock(p);
  if (!p->win_stdin || p->stdin_closed) {
    rt_os_proc_unlock(p);
    return 0;
  }

  rt_os_win_close_handle(&p->win_stdin);
  p->stdin_closed = 1;
  bytes_t old = p->stdin_buf;
  p->stdin_buf = rt_bytes_empty(ctx);
  p->stdin_off = 0;
  HANDLE ev = p->win_stdin_event;
  rt_os_proc_unlock(p);

  if (ev) (void)SetEvent(ev);
  rt_bytes_drop(ctx, &old);
  return 1;
#else
  if (p->stdin_fd < 0 || p->stdin_closed) return 0;

  rt_os_close_fd(p->stdin_fd);
  p->stdin_fd = -1;
  p->stdin_closed = 1;
  rt_bytes_drop(ctx, &p->stdin_buf);
  p->stdin_buf = rt_bytes_empty(ctx);
  p->stdin_off = 0;
  return 1;
#endif
}

static int32_t rt_os_process_try_wait_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 1;
  (void)rt_os_process_poll_all(ctx, 0);

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;
  return (p->state != RT_OS_PROC_STATE_RUNNING || p->exited) ? 1 : 0;
}

static uint32_t rt_os_process_join_exit_poll(ctx_t* ctx, int32_t handle, int32_t* out) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    if (out) *out = 1;
    return UINT32_C(1);
  }

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->state != RT_OS_PROC_STATE_RUNNING || p->exited) {
    if (out) *out = 1;
    return UINT32_C(1);
  }

  uint32_t cur = ctx->sched_current_task;
  if (cur == 0) rt_trap("os.process.join_exit.poll from main");

  rt_task_t* me = rt_task_ptr(ctx, cur);
  uint32_t hid = (uint32_t)handle;
  if (me->wait_kind == RT_WAIT_OS_PROC_EXIT && me->wait_id == hid) {
    return UINT32_C(0);
  }
  if (me->wait_kind != RT_WAIT_NONE) rt_trap("os.process.join_exit while already waiting");

  me->wait_kind = RT_WAIT_OS_PROC_EXIT;
  me->wait_id = hid;
  ctx->sched_stats.blocked_waits += 1;
  rt_sched_trace_event(ctx, RT_TRACE_BLOCK, (uint64_t)cur, ((uint64_t)RT_WAIT_OS_PROC_EXIT << 32) | (uint64_t)hid);
  rt_wait_list_push(ctx, &p->exit_wait_head, &p->exit_wait_tail, cur);
  return UINT32_C(0);
}

static int32_t rt_os_process_join_exit_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) return 1;

  for (;;) {
    uint32_t idx = 0;
    rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
    (void)idx;

    if (p->state != RT_OS_PROC_STATE_RUNNING || p->exited) return 1;

    if (!rt_sched_step(ctx)) rt_sched_deadlock();
  }
}

static int32_t rt_os_process_take_exit_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  uint32_t sentinel_code = 0;
  if (rt_os_proc_handle_is_sentinel(handle, &sentinel_code)) {
    return (int32_t)(0 - (int32_t)sentinel_code);
  }

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->exit_taken) rt_trap("os.process.take_exit_v1 already taken");

  if (p->fail_code != 0) {
    p->exit_taken = 1;
    return (int32_t)(0 - (int32_t)p->fail_code);
  }

  if (p->state != RT_OS_PROC_STATE_RUNNING && p->result.len != 0 && p->result.ptr[0] == 0) {
    uint32_t code = (p->result.len >= 5) ? rt_read_u32_le(p->result.ptr + 1) : UINT32_C(0);
    if (code == 0) rt_trap("os.process.take_exit_v1 invalid error doc");
    p->exit_taken = 1;
    return (int32_t)(0 - (int32_t)code);
  }

  if (!p->exited) rt_trap("os.process.take_exit_v1 before exit");
  p->exit_taken = 1;
  return (int32_t)rt_os_proc_exit_code_from_status(p->status);
}

static int32_t rt_os_process_kill_v1(ctx_t* ctx, int32_t handle, int32_t sig) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 0;

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);
  (void)idx;

  if (p->state != RT_OS_PROC_STATE_RUNNING) return 0;
#if defined(_WIN32)
  rt_os_proc_send_kill(p, sig);
  return 1;
#else
  if (p->pid == (pid_t)-1) return 0;
  pid_t target = rt_os_proc_kill_target(p);
  if (target == (pid_t)-1 || target == (pid_t)0) return 0;
  if (kill(target, (int)sig) != 0) return 0;
  return 1;
#endif
}

static int32_t rt_os_process_drop_v1(ctx_t* ctx, int32_t handle) {
  rt_os_policy_init(ctx);
  if (rt_os_proc_handle_is_sentinel(handle, NULL)) return 0;

  uint32_t idx = 0;
  rt_os_proc_t* p = rt_os_proc_from_handle(ctx, handle, &idx);

  if (p->join_wait_head != 0 || p->exit_wait_head != 0) rt_trap("os.process.drop_v1 while tasks waiting");

  uint32_t was_running = (p->state == RT_OS_PROC_STATE_RUNNING) ? 1 : 0;
  if (was_running) {
    if (rt_os_sandboxed && !rt_os_proc_kill_on_drop) return 0;
    if (ctx->os_procs_live == 0) rt_trap("os.process live underflow");
    ctx->os_procs_live -= 1;
  }

#if defined(_WIN32)
  rt_os_proc_close_fds(p);
  rt_os_win_proc_join_threads(p);
  rt_os_proc_drop_buffers(ctx, p);
  rt_os_proc_drop_result(ctx, p);
  if (was_running) {
    rt_os_proc_kill_and_reap(p);
  }
  rt_os_win_close_handle(&p->win_proc);
  rt_os_win_close_handle(&p->win_job);
  rt_os_win_close_handle(&p->win_stdin_event);
#else
  if (was_running) {
    rt_os_proc_close_fds(p);
    rt_os_proc_drop_buffers(ctx, p);
    rt_os_proc_drop_result(ctx, p);
    rt_os_proc_kill_and_reap(p);
  } else {
    rt_os_proc_close_fds(p);
    rt_os_proc_drop_buffers(ctx, p);
    rt_os_proc_drop_result(ctx, p);
  }
#endif

  uint16_t next_gen = rt_os_proc_next_gen(p->gen);
  rt_os_proc_init_entry(ctx, p, next_gen);
  return 1;
}

static uint32_t rt_os_process_poll_all(ctx_t* ctx, int poll_timeout_ms) {
  rt_os_policy_init(ctx);
  uint32_t had_live = ctx->os_procs_live ? UINT32_C(1) : UINT32_C(0);
  if (!had_live) {
#if defined(_WIN32)
    if (poll_timeout_ms > 0) Sleep((DWORD)poll_timeout_ms);
#else
    if (poll_timeout_ms > 0) (void)poll(NULL, 0, poll_timeout_ms);
#endif
    return UINT32_C(0);
  }

  if (poll_timeout_ms < 0) poll_timeout_ms = 0;

#if defined(_WIN32)
  if (poll_timeout_ms > 0) Sleep((DWORD)poll_timeout_ms);

  // Update exit status and enforce timeouts.
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state != RT_OS_PROC_STATE_RUNNING) continue;
    uint32_t was_exited = p->exited;
    rt_os_proc_try_wait(p);
    if (!was_exited && p->exited) {
      uint32_t reason_id = rt_os_proc_handle_u32(i, p->gen);
      rt_os_proc_wake_exit_waiters(ctx, p, reason_id);
    }

    if (p->fail_code == 0 && p->timeout_ms != 0) {
      uint64_t now_ms = rt_os_now_ms();
      if (now_ms >= p->start_ms && now_ms - p->start_ms > (uint64_t)p->timeout_ms) {
        p->fail_code = RT_OS_PROC_CODE_TIMEOUT;
        rt_os_proc_send_kill(p, SIGKILL);
        rt_os_proc_close_fds(p);
        rt_os_proc_drop_buffers(ctx, p);
      }
    }
  }

  // Finalize completed processes.
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state != RT_OS_PROC_STATE_RUNNING) continue;

    uint32_t was_exited = p->exited;
    rt_os_proc_try_wait(p);
    if (!was_exited && p->exited) {
      uint32_t reason_id = rt_os_proc_handle_u32(i, p->gen);
      rt_os_proc_wake_exit_waiters(ctx, p, reason_id);
    }

    if (p->fail_code != 0) {
      rt_os_proc_send_kill(p, SIGKILL);
      if (p->exited) {
        rt_os_proc_close_fds(p);
        rt_os_proc_finish_err(ctx, p, i, p->fail_code);
      }
      continue;
    }

    if (p->exited && p->stdin_closed && p->stdout_closed && p->stderr_closed) {
      rt_os_proc_close_fds(p);
      if (p->mode == RT_OS_PROC_MODE_CAPTURE) {
        rt_os_proc_finish_ok(ctx, p, i);
      } else {
        rt_os_proc_finish_piped(ctx, p);
      }
    }
  }

  return UINT32_C(1);
#else
  // Update exit status and enforce timeouts.
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state != RT_OS_PROC_STATE_RUNNING) continue;
    uint32_t was_exited = p->exited;
    rt_os_proc_try_wait(p);
    if (!was_exited && p->exited) {
      uint32_t reason_id = rt_os_proc_handle_u32(i, p->gen);
      rt_os_proc_wake_exit_waiters(ctx, p, reason_id);
    }

    if (p->fail_code == 0 && p->timeout_ms != 0) {
      uint64_t now_ms = rt_os_now_ms();
      if (now_ms >= p->start_ms && now_ms - p->start_ms > (uint64_t)p->timeout_ms) {
        p->fail_code = RT_OS_PROC_CODE_TIMEOUT;
        rt_os_proc_send_kill(p, SIGKILL);
        rt_os_proc_close_fds(p);
        rt_os_proc_drop_buffers(ctx, p);
      }
    }
  }

  // Build poll set.
  uint32_t nfds = 0;
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state != RT_OS_PROC_STATE_RUNNING) continue;
    if (p->stdin_fd >= 0 && !p->stdin_closed && p->stdin_off < p->stdin_buf.len) nfds += 1;
    if (p->stdout_fd >= 0 && !p->stdout_closed) nfds += 1;
    if (p->stderr_fd >= 0 && !p->stderr_closed) nfds += 1;
  }

  struct pollfd* fds = NULL;
  uint32_t* tags = NULL;
  if (nfds != 0) {
    fds = (struct pollfd*)rt_alloc(ctx, nfds * (uint32_t)sizeof(struct pollfd), (uint32_t)_Alignof(struct pollfd));
    tags = (uint32_t*)rt_alloc(ctx, nfds * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));

    uint32_t at = 0;
    for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
      rt_os_proc_t* p = &ctx->os_procs[i];
      if (p->state != RT_OS_PROC_STATE_RUNNING) continue;
      if (p->stdin_fd >= 0 && !p->stdin_closed && p->stdin_off < p->stdin_buf.len) {
        fds[at].fd = p->stdin_fd;
        fds[at].events = POLLOUT;
        fds[at].revents = 0;
        tags[at] = (i << 2) | RT_OS_PROC_POLL_KIND_STDIN;
        at += 1;
      }
      if (p->stdout_fd >= 0 && !p->stdout_closed) {
        fds[at].fd = p->stdout_fd;
        fds[at].events = POLLIN;
        fds[at].revents = 0;
        tags[at] = (i << 2) | RT_OS_PROC_POLL_KIND_STDOUT;
        at += 1;
      }
      if (p->stderr_fd >= 0 && !p->stderr_closed) {
        fds[at].fd = p->stderr_fd;
        fds[at].events = POLLIN;
        fds[at].revents = 0;
        tags[at] = (i << 2) | RT_OS_PROC_POLL_KIND_STDERR;
        at += 1;
      }
    }

    int pr = poll(fds, (nfds_t)nfds, poll_timeout_ms);
    if (pr < 0) {
      if (errno != EINTR) rt_trap("os.process.poll_all poll failed");
    }

    for (uint32_t i = 0; i < nfds; i++) {
      short re = fds[i].revents;
      if (re == 0) continue;

      uint32_t tag = tags[i];
      uint32_t pidx = tag >> 2;
      uint32_t kind = tag & 3;
      if (pidx >= ctx->os_procs_len) continue;
      rt_os_proc_t* p = &ctx->os_procs[pidx];
      if (p->state != RT_OS_PROC_STATE_RUNNING) continue;
      if (p->fail_code != 0) continue;

      if (kind == RT_OS_PROC_POLL_KIND_STDIN) {
        if (p->stdin_fd < 0 || p->stdin_closed) continue;
        if (p->stdin_off >= p->stdin_buf.len) {
          rt_bytes_drop(ctx, &p->stdin_buf);
          p->stdin_buf = rt_bytes_empty(ctx);
          p->stdin_off = 0;
          if (p->mode == RT_OS_PROC_MODE_CAPTURE) {
            rt_os_close_fd(p->stdin_fd);
            p->stdin_fd = -1;
            p->stdin_closed = 1;
          }
          continue;
        }

        ssize_t n = write(p->stdin_fd, p->stdin_buf.ptr + p->stdin_off, p->stdin_buf.len - p->stdin_off);
        if (n > 0) {
          p->stdin_off += (uint32_t)n;
          if (p->stdin_off >= p->stdin_buf.len) {
            rt_bytes_drop(ctx, &p->stdin_buf);
            p->stdin_buf = rt_bytes_empty(ctx);
            p->stdin_off = 0;
            if (p->mode == RT_OS_PROC_MODE_CAPTURE) {
              rt_os_close_fd(p->stdin_fd);
              p->stdin_fd = -1;
              p->stdin_closed = 1;
            }
          }
          continue;
        }
        if (n < 0) {
          if (errno == EINTR || errno == EAGAIN) continue;
          if (errno == EPIPE) {
            rt_os_close_fd(p->stdin_fd);
            p->stdin_fd = -1;
            p->stdin_closed = 1;
            rt_bytes_drop(ctx, &p->stdin_buf);
            p->stdin_buf = rt_bytes_empty(ctx);
            continue;
          }
          p->fail_code = RT_OS_PROC_CODE_SPAWN_FAILED;
          rt_os_proc_send_kill(p, SIGKILL);
          rt_os_proc_close_fds(p);
          rt_os_proc_drop_buffers(ctx, p);
          continue;
        }
      }

      if (kind == RT_OS_PROC_POLL_KIND_STDOUT) {
        if (p->stdout_fd < 0 || p->stdout_closed) continue;

        uint32_t total_len = p->stdout_len + p->stderr_len;
        uint32_t total_rem = (total_len < p->max_total) ? (p->max_total - total_len) : 0;
        uint32_t rem_total = (p->stdout_len < p->max_stdout) ? (p->max_stdout - p->stdout_len) : 0;
        if (rem_total > total_rem) rem_total = total_rem;
        if (rem_total == 0) {
          p->fail_code = RT_OS_PROC_CODE_OUTPUT_LIMIT;
          rt_os_proc_send_kill(p, SIGKILL);
          rt_os_proc_close_fds(p);
          rt_os_proc_drop_buffers(ctx, p);
          continue;
        }

        if (p->stdout_off != 0 && p->stdout_off + p->stdout_len == p->max_stdout) {
          memmove(p->stdout_buf.ptr, p->stdout_buf.ptr + p->stdout_off, p->stdout_len);
          rt_mem_on_memcpy(ctx, p->stdout_len);
          p->stdout_off = 0;
        }
        uint32_t cont_rem = p->max_stdout - (p->stdout_off + p->stdout_len);
        uint32_t rem = rem_total;
        if (rem > cont_rem) rem = cont_rem;

        ssize_t n = read(p->stdout_fd, p->stdout_buf.ptr + p->stdout_off + p->stdout_len, rem);
        if (n > 0) {
          p->stdout_len += (uint32_t)n;
          continue;
        }
        if (n == 0) {
          rt_os_close_fd(p->stdout_fd);
          p->stdout_fd = -1;
          p->stdout_closed = 1;
          continue;
        }
        if (errno == EINTR || errno == EAGAIN) continue;
        p->fail_code = RT_OS_PROC_CODE_SPAWN_FAILED;
        rt_os_proc_send_kill(p, SIGKILL);
        rt_os_proc_close_fds(p);
        rt_os_proc_drop_buffers(ctx, p);
        continue;
      }

      if (kind == RT_OS_PROC_POLL_KIND_STDERR) {
        if (p->stderr_fd < 0 || p->stderr_closed) continue;

        uint32_t total_len = p->stdout_len + p->stderr_len;
        uint32_t total_rem = (total_len < p->max_total) ? (p->max_total - total_len) : 0;
        uint32_t rem_total = (p->stderr_len < p->max_stderr) ? (p->max_stderr - p->stderr_len) : 0;
        if (rem_total > total_rem) rem_total = total_rem;
        if (rem_total == 0) {
          p->fail_code = RT_OS_PROC_CODE_OUTPUT_LIMIT;
          rt_os_proc_send_kill(p, SIGKILL);
          rt_os_proc_close_fds(p);
          rt_os_proc_drop_buffers(ctx, p);
          continue;
        }

        if (p->stderr_off != 0 && p->stderr_off + p->stderr_len == p->max_stderr) {
          memmove(p->stderr_buf.ptr, p->stderr_buf.ptr + p->stderr_off, p->stderr_len);
          rt_mem_on_memcpy(ctx, p->stderr_len);
          p->stderr_off = 0;
        }
        uint32_t cont_rem = p->max_stderr - (p->stderr_off + p->stderr_len);
        uint32_t rem = rem_total;
        if (rem > cont_rem) rem = cont_rem;

        ssize_t n = read(p->stderr_fd, p->stderr_buf.ptr + p->stderr_off + p->stderr_len, rem);
        if (n > 0) {
          p->stderr_len += (uint32_t)n;
          continue;
        }
        if (n == 0) {
          rt_os_close_fd(p->stderr_fd);
          p->stderr_fd = -1;
          p->stderr_closed = 1;
          continue;
        }
        if (errno == EINTR || errno == EAGAIN) continue;
        p->fail_code = RT_OS_PROC_CODE_SPAWN_FAILED;
        rt_os_proc_send_kill(p, SIGKILL);
        rt_os_proc_close_fds(p);
        rt_os_proc_drop_buffers(ctx, p);
        continue;
      }
    }
  } else {
    (void)poll(NULL, 0, poll_timeout_ms);
  }

  // Finalize completed processes.
  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state != RT_OS_PROC_STATE_RUNNING) continue;

    uint32_t was_exited = p->exited;
    rt_os_proc_try_wait(p);
    if (!was_exited && p->exited) {
      uint32_t reason_id = rt_os_proc_handle_u32(i, p->gen);
      rt_os_proc_wake_exit_waiters(ctx, p, reason_id);
    }

    if (p->fail_code != 0) {
      rt_os_proc_send_kill(p, SIGKILL);
      if (p->exited) {
        rt_os_proc_close_fds(p);
        rt_os_proc_finish_err(ctx, p, i, p->fail_code);
      }
      continue;
    }

    if (p->exited && p->stdin_closed && p->stdout_closed && p->stderr_closed) {
      rt_os_proc_close_fds(p);
      if (p->mode == RT_OS_PROC_MODE_CAPTURE) {
        rt_os_proc_finish_ok(ctx, p, i);
      } else {
        rt_os_proc_finish_piped(ctx, p);
      }
    }
  }

  if (fds) rt_free(ctx, fds, nfds * (uint32_t)sizeof(struct pollfd), (uint32_t)_Alignof(struct pollfd));
  if (tags) rt_free(ctx, tags, nfds * (uint32_t)sizeof(uint32_t), (uint32_t)_Alignof(uint32_t));

  return UINT32_C(1);
#endif
}

static void rt_os_process_cleanup(ctx_t* ctx) {
  rt_os_policy_init(ctx);
  if (!ctx->os_procs) {
    ctx->os_procs_len = 0;
    ctx->os_procs_cap = 0;
    ctx->os_procs_live = 0;
    ctx->os_procs_spawned = 0;
    return;
  }

  for (uint32_t i = 0; i < ctx->os_procs_len; i++) {
    rt_os_proc_t* p = &ctx->os_procs[i];
    if (p->state == RT_OS_PROC_STATE_EMPTY) continue;

#if defined(_WIN32)
    uint32_t was_running = (p->state == RT_OS_PROC_STATE_RUNNING) ? 1 : 0;
    rt_os_proc_close_fds(p);
    rt_os_win_proc_join_threads(p);
    rt_os_proc_drop_buffers(ctx, p);
    rt_os_proc_drop_result(ctx, p);
    if (was_running) {
      rt_os_proc_kill_and_reap(p);
    }
    rt_os_win_close_handle(&p->win_proc);
    rt_os_win_close_handle(&p->win_job);
    rt_os_win_close_handle(&p->win_stdin_event);
#else
    if (p->state == RT_OS_PROC_STATE_RUNNING) {
      rt_os_proc_close_fds(p);
      rt_os_proc_drop_buffers(ctx, p);
      rt_os_proc_drop_result(ctx, p);
      rt_os_proc_kill_and_reap(p);
    } else {
      rt_os_proc_close_fds(p);
      rt_os_proc_drop_buffers(ctx, p);
      rt_os_proc_drop_result(ctx, p);
    }
#endif

    uint16_t next_gen = rt_os_proc_next_gen(p->gen);
    rt_os_proc_init_entry(ctx, p, next_gen);
  }

  if (ctx->os_procs_cap) {
    rt_free(
      ctx,
      ctx->os_procs,
      ctx->os_procs_cap * (uint32_t)sizeof(rt_os_proc_t),
      (uint32_t)_Alignof(rt_os_proc_t)
    );
  }
  ctx->os_procs = NULL;
  ctx->os_procs_len = 0;
  ctx->os_procs_cap = 0;
  ctx->os_procs_live = 0;
  ctx->os_procs_spawned = 0;
}

static bytes_t rt_os_process_run_capture_v1(ctx_t* ctx, bytes_t req, bytes_t caps) {
  int32_t h = rt_os_process_spawn_capture_v1(ctx, req, caps);
  bytes_t out = rt_os_process_join_capture_v1(ctx, h);
  (void)rt_os_process_drop_v1(ctx, h);
  return out;
}

static __attribute__((noreturn)) void rt_os_process_exit(ctx_t* ctx, int32_t code) {
  rt_os_policy_init(ctx);
  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_proc_enabled, "os.process disabled by policy");
    rt_os_require(ctx, rt_os_proc_allow_exit, "os.process.exit disabled by policy");
  }
  (void)ctx;
  exit((int)code);
}

static bytes_t rt_os_net_http_request(ctx_t* ctx, bytes_t req) {
  rt_os_policy_init(ctx);
  (void)req;
  if (rt_os_sandboxed) {
    rt_os_require(ctx, rt_os_net_enabled, "os.net disabled by policy");
    rt_os_require(ctx, rt_os_net_allow_tcp, "os.net.http_request disabled by policy");
    rt_os_require(ctx, rt_os_net_allow_dns, "os.net.http_request disabled by policy");
    rt_os_require(ctx, rt_os_net_allow_hosts && *rt_os_net_allow_hosts, "os.net.allow_hosts required");
  }
  rt_trap("os.net.http_request not implemented");
}
"#;

const RUNTIME_C_MAIN: &str = r#"
static int rt_read_exact(int fd, uint8_t* dst, uint32_t len) {
  uint32_t off = 0;
  while (off < len) {
    ssize_t n = read(fd, dst + off, len - off);
    if (n == 0) return -1;
    if (n < 0) {
      if (errno == EINTR) continue;
      return -1;
    }
    off += (uint32_t)n;
  }
  return 0;
}

static int rt_write_exact(int fd, const uint8_t* src, uint32_t len) {
  uint32_t off = 0;
  while (off < len) {
    ssize_t n = write(fd, src + off, len - off);
    if (n < 0) {
      if (errno == EINTR) continue;
      return -1;
    }
    off += (uint32_t)n;
  }
  return 0;
}

int main(void) {
#ifdef _WIN32
  _setmode(0, _O_BINARY); _setmode(1, _O_BINARY);
#endif
  const uint32_t mem_cap = (uint32_t)(X07_MEM_CAP);
  int mem_is_mmap = 0;
  uint8_t* mem = NULL;
#ifndef _WIN32
  mem = (uint8_t*)mmap(
    NULL,
    (size_t)mem_cap,
    PROT_READ | PROT_WRITE,
    MAP_PRIVATE | MAP_ANON,
    -1,
    0
  );
  if (mem != (uint8_t*)MAP_FAILED) {
    mem_is_mmap = 1;
  } else {
    mem = (uint8_t*)calloc(1, (size_t)mem_cap);
    if (!mem) rt_trap("calloc failed");
  }
#else
  mem = (uint8_t*)calloc(1, (size_t)mem_cap);
  if (!mem) rt_trap("calloc failed");
#endif

  ctx_t ctx;
  memset(&ctx, 0, sizeof(ctx));
  ctx.fuel_init = (uint64_t)(X07_FUEL_INIT);
  ctx.fuel = ctx.fuel_init;
  ctx.heap.mem = mem;
  ctx.heap.cap = mem_cap;
  rt_heap_init(&ctx);
  rt_allocator_init(&ctx);
  rt_ext_ctx = &ctx;

#ifdef X07_DEBUG_BORROW
  rt_dbg_init(&ctx);
#endif

  rt_kv_init(&ctx);

  uint8_t len_buf[4];
  if (rt_read_exact(STDIN_FILENO, len_buf, 4) != 0) return 2;
  uint32_t in_len = (uint32_t)len_buf[0]
                  | ((uint32_t)len_buf[1] << 8)
                  | ((uint32_t)len_buf[2] << 16)
                  | ((uint32_t)len_buf[3] << 24);

  bytes_t input_bytes = rt_bytes_alloc(&ctx, in_len);
  if (in_len && rt_read_exact(STDIN_FILENO, input_bytes.ptr, in_len) != 0) return 2;

  rt_mem_epoch_reset(&ctx);

  bytes_view_t input = rt_bytes_view(&ctx, input_bytes);
  bytes_t out = solve(&ctx, input);
  rt_ext_ctx = NULL;

#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_bytes_check(&ctx, out);
#endif

  uint32_t out_len = out.len;
  uint8_t out_len_buf[4] = {
    (uint8_t)(out_len & UINT32_C(0xFF)),
    (uint8_t)((out_len >> 8) & UINT32_C(0xFF)),
    (uint8_t)((out_len >> 16) & UINT32_C(0xFF)),
    (uint8_t)((out_len >> 24) & UINT32_C(0xFF)),
  };
  if (rt_write_exact(STDOUT_FILENO, out_len_buf, 4) != 0) return 2;
  if (out_len && rt_write_exact(STDOUT_FILENO, out.ptr, out_len) != 0) return 2;

  rt_bytes_drop(&ctx, &out);
  rt_bytes_drop(&ctx, &input_bytes);
  rt_ctx_cleanup(&ctx);

  uint32_t heap_used = (ctx.heap_peak_live_bytes > (uint64_t)UINT32_MAX)
    ? UINT32_MAX
    : (uint32_t)ctx.heap_peak_live_bytes;
  uint64_t fuel_used = ctx.fuel_init - ctx.fuel;

  char rr_last_sha[65];
  rt_hex_bytes(ctx.rr_last_request_sha256, 32, rr_last_sha);
  ctx.sched_stats.virtual_time_end = ctx.sched_now_ticks;
  rt_sched_trace_init(&ctx);
  char sched_trace_hash_str[19];
  (void)snprintf(
    sched_trace_hash_str,
    sizeof(sched_trace_hash_str),
    "0x%016" PRIx64,
    ctx.sched_stats.sched_trace_hash
  );

#ifdef X07_DEBUG_BORROW
  fprintf(
    stderr,
    "{\"fuel_used\":%" PRIu64 ",\"heap_used\":%u,\"fs_read_file_calls\":%" PRIu64 ",\"fs_list_dir_calls\":%" PRIu64 ","
    "\"rr_send_calls\":%" PRIu64 ",\"rr_request_calls\":%" PRIu64 ",\"rr_last_request_sha256\":\"%s\","
    "\"kv_get_calls\":%" PRIu64 ",\"kv_set_calls\":%" PRIu64 ","
    "\"sched_stats\":{"
    "\"tasks_spawned\":%" PRIu64 ",\"spawn_calls\":%" PRIu64 ",\"join_calls\":%" PRIu64 ","
    "\"yield_calls\":%" PRIu64 ",\"sleep_calls\":%" PRIu64 ","
    "\"chan_send_calls\":%" PRIu64 ",\"chan_recv_calls\":%" PRIu64 ","
    "\"ctx_switches\":%" PRIu64 ",\"wake_events\":%" PRIu64 ",\"blocked_waits\":%" PRIu64 ","
    "\"virtual_time_end\":%" PRIu64 ",\"sched_trace_hash\":\"%s\"},"
    "\"mem_stats\":{"
    "\"alloc_calls\":%" PRIu64 ",\"realloc_calls\":%" PRIu64 ",\"free_calls\":%" PRIu64 ","
    "\"bytes_alloc_total\":%" PRIu64 ",\"bytes_freed_total\":%" PRIu64 ","
    "\"live_bytes\":%" PRIu64 ",\"peak_live_bytes\":%" PRIu64 ","
    "\"live_allocs\":%" PRIu64 ",\"peak_live_allocs\":%" PRIu64 ","
    "\"memcpy_bytes\":%" PRIu64 "},"
    "\"debug_stats\":{"
    "\"borrow_violations\":%" PRIu64 "}}\n",
    fuel_used,
    heap_used,
    ctx.fs_read_file_calls,
    ctx.fs_list_dir_calls,
    ctx.rr_send_calls,
    ctx.rr_request_calls,
    rr_last_sha,
    ctx.kv_get_calls,
    ctx.kv_set_calls,
    ctx.sched_stats.tasks_spawned,
    ctx.sched_stats.spawn_calls,
    ctx.sched_stats.join_calls,
    ctx.sched_stats.yield_calls,
    ctx.sched_stats.sleep_calls,
    ctx.sched_stats.chan_send_calls,
    ctx.sched_stats.chan_recv_calls,
    ctx.sched_stats.ctx_switches,
    ctx.sched_stats.wake_events,
    ctx.sched_stats.blocked_waits,
    ctx.sched_stats.virtual_time_end,
    sched_trace_hash_str,
    ctx.mem_stats.alloc_calls,
    ctx.mem_stats.realloc_calls,
    ctx.mem_stats.free_calls,
    ctx.mem_stats.bytes_alloc_total,
    ctx.mem_stats.bytes_freed_total,
    ctx.mem_stats.live_bytes,
    ctx.mem_stats.peak_live_bytes,
    ctx.mem_stats.live_allocs,
    ctx.mem_stats.peak_live_allocs,
    ctx.mem_stats.memcpy_bytes,
    ctx.dbg_borrow_violations
  );
#else
  fprintf(
    stderr,
    "{\"fuel_used\":%" PRIu64 ",\"heap_used\":%u,\"fs_read_file_calls\":%" PRIu64 ",\"fs_list_dir_calls\":%" PRIu64 ","
    "\"rr_send_calls\":%" PRIu64 ",\"rr_request_calls\":%" PRIu64 ",\"rr_last_request_sha256\":\"%s\","
    "\"kv_get_calls\":%" PRIu64 ",\"kv_set_calls\":%" PRIu64 ","
    "\"sched_stats\":{"
    "\"tasks_spawned\":%" PRIu64 ",\"spawn_calls\":%" PRIu64 ",\"join_calls\":%" PRIu64 ","
    "\"yield_calls\":%" PRIu64 ",\"sleep_calls\":%" PRIu64 ","
    "\"chan_send_calls\":%" PRIu64 ",\"chan_recv_calls\":%" PRIu64 ","
    "\"ctx_switches\":%" PRIu64 ",\"wake_events\":%" PRIu64 ",\"blocked_waits\":%" PRIu64 ","
    "\"virtual_time_end\":%" PRIu64 ",\"sched_trace_hash\":\"%s\"},"
    "\"mem_stats\":{"
    "\"alloc_calls\":%" PRIu64 ",\"realloc_calls\":%" PRIu64 ",\"free_calls\":%" PRIu64 ","
    "\"bytes_alloc_total\":%" PRIu64 ",\"bytes_freed_total\":%" PRIu64 ","
    "\"live_bytes\":%" PRIu64 ",\"peak_live_bytes\":%" PRIu64 ","
    "\"live_allocs\":%" PRIu64 ",\"peak_live_allocs\":%" PRIu64 ","
    "\"memcpy_bytes\":%" PRIu64 "}}\n",
    fuel_used,
    heap_used,
    ctx.fs_read_file_calls,
    ctx.fs_list_dir_calls,
    ctx.rr_send_calls,
    ctx.rr_request_calls,
    rr_last_sha,
    ctx.kv_get_calls,
    ctx.kv_set_calls,
    ctx.sched_stats.tasks_spawned,
    ctx.sched_stats.spawn_calls,
    ctx.sched_stats.join_calls,
    ctx.sched_stats.yield_calls,
    ctx.sched_stats.sleep_calls,
    ctx.sched_stats.chan_send_calls,
    ctx.sched_stats.chan_recv_calls,
    ctx.sched_stats.ctx_switches,
    ctx.sched_stats.wake_events,
    ctx.sched_stats.blocked_waits,
    ctx.sched_stats.virtual_time_end,
    sched_trace_hash_str,
    ctx.mem_stats.alloc_calls,
    ctx.mem_stats.realloc_calls,
    ctx.mem_stats.free_calls,
    ctx.mem_stats.bytes_alloc_total,
    ctx.mem_stats.bytes_freed_total,
    ctx.mem_stats.live_bytes,
    ctx.mem_stats.peak_live_bytes,
    ctx.mem_stats.live_allocs,
    ctx.mem_stats.peak_live_allocs,
    ctx.mem_stats.memcpy_bytes
  );
#endif
  fflush(stderr);
#ifndef _WIN32
  if (mem_is_mmap) {
    (void)munmap(mem, (size_t)mem_cap);
  } else {
    free(mem);
  }
#else
  free(mem);
#endif
  return 0;
}
"#;

const RUNTIME_C_HEADER: &str = r#"
#ifndef X07_PKG_H
#define X07_PKG_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
  uint8_t* ptr;
  uint32_t len;
} bytes_t;

typedef struct {
  void* ctx;
  void* (*alloc)(void* ctx, uint32_t size, uint32_t align);
  void* (*realloc)(void* ctx, void* ptr, uint32_t old_size, uint32_t new_size, uint32_t align);
  void (*free)(void* ctx, void* ptr, uint32_t size, uint32_t align);
} allocator_v1_t;

allocator_v1_t x07_custom_allocator(void);

bytes_t x07_solve_v2(
    uint8_t* arena_mem,
    uint32_t arena_cap,
    const uint8_t* input_ptr,
    uint32_t input_len
);

#ifdef __cplusplus
} // extern "C"
#endif

#endif // X07_PKG_H
"#;

const RUNTIME_C_LIB: &str = r#"
static uint8_t rt_dummy_heap_mem[1];

bytes_t x07_solve_v2(
    uint8_t* arena_mem,
    uint32_t arena_cap,
    const uint8_t* input_ptr,
    uint32_t input_len
) {
  if (!arena_mem) {
    if (arena_cap != 0) rt_trap("arena_mem is NULL");
    arena_mem = rt_dummy_heap_mem;
  }

  ctx_t ctx;
  memset(&ctx, 0, sizeof(ctx));
  ctx.fuel_init = (uint64_t)(X07_FUEL_INIT);
  ctx.fuel = ctx.fuel_init;
  ctx.heap.mem = arena_mem;
  ctx.heap.cap = arena_cap;
  rt_heap_init(&ctx);
  rt_allocator_init(&ctx);
  rt_ext_ctx = &ctx;

#ifdef X07_DEBUG_BORROW
  rt_dbg_init(&ctx);
#endif

  rt_kv_init(&ctx);

  bytes_t input_bytes = rt_bytes_alloc(&ctx, input_len);
  if (input_len) {
    memcpy(input_bytes.ptr, input_ptr, input_len);
    rt_mem_on_memcpy(&ctx, input_len);
  }

  rt_mem_epoch_reset(&ctx);

  bytes_view_t input = rt_bytes_view(&ctx, input_bytes);
  bytes_t out = solve(&ctx, input);
  rt_ext_ctx = NULL;

#ifdef X07_DEBUG_BORROW
  (void)rt_dbg_bytes_check(&ctx, out);
#endif

  rt_bytes_drop(&ctx, &input_bytes);

  return out;
}
"#;
