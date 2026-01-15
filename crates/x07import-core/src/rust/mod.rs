use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use syn::spanned::Spanned;

use crate::diagnostics::{Diagnostic, DiagnosticCode, Phase};
use crate::x07ir::{X07Expr, X07Func, X07Module, X07Param, X07Stmt, X07Ty};

pub fn import_rust_file(module_id: &str, src_path: &Path, src: &str) -> Result<X07Module> {
    let file: syn::File = syn::parse_file(src).map_err(|e| {
        anyhow::anyhow!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0001ParseError,
                Phase::Parse,
                e.to_string()
            )
        )
        .context("x07import parse failed")
    })?;

    let mut funcs: Vec<syn::ItemFn> = Vec::new();
    for item in file.items {
        match item {
            syn::Item::Fn(f) => funcs.push(f),
            other => {
                let kind = format!("{:?}", other);
                anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0100UnsupportedItem,
                        Phase::Validate,
                        format!("unsupported top-level item: {kind}")
                    )
                );
            }
        }
    }

    let mut local_fns: BTreeSet<String> = BTreeSet::new();
    for f in &funcs {
        local_fns.insert(f.sig.ident.to_string());
    }

    let mut lowered: Vec<X07Func> = Vec::new();
    for f in funcs {
        lowered.push(lower_fn(module_id, &local_fns, f)?);
    }

    Ok(X07Module {
        module_id: module_id.to_string(),
        source_path: Some(src_path.to_string_lossy().to_string()),
        source_sha256: Some(crate::util::sha256_hex(src.as_bytes())),
        funcs: lowered,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarTy {
    I32,
    Bytes,
    BytesView,
    VecU8,
}

#[derive(Debug)]
struct Ctx<'a> {
    module_id: &'a str,
    local_fns: &'a BTreeSet<String>,
    scopes: Vec<BTreeMap<String, ScalarTy>>,
}

impl<'a> Ctx<'a> {
    fn new(module_id: &'a str, local_fns: &'a BTreeSet<String>) -> Self {
        Ctx {
            module_id,
            local_fns,
            scopes: Vec::new(),
        }
    }

    fn enter_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn exit_scope(&mut self) {
        let _ = self.scopes.pop();
    }

    fn declare(&mut self, name: &str, ty: ScalarTy) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup(&self, name: &str) -> Option<ScalarTy> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(*t);
            }
        }
        None
    }
}

fn lower_fn(module_id: &str, local_fns: &BTreeSet<String>, f: syn::ItemFn) -> Result<X07Func> {
    if !f.sig.generics.params.is_empty() {
        anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0110UnsupportedFnSig,
                Phase::Validate,
                "generics are not supported"
            )
        );
    }
    if f.sig.asyncness.is_some() || f.sig.unsafety.is_some() || f.sig.abi.is_some() {
        anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0110UnsupportedFnSig,
                Phase::Validate,
                "async/unsafe/extern are not supported"
            )
        );
    }

    let exported = matches!(f.vis, syn::Visibility::Public(_));
    let local_name = f.sig.ident.to_string();
    let full_name = format!("{module_id}.{local_name}");

    let mut params: Vec<X07Param> = Vec::new();
    let mut ctx = Ctx::new(module_id, local_fns);
    ctx.enter_scope();

    for arg in &f.sig.inputs {
        let syn::FnArg::Typed(pat_ty) = arg else {
            anyhow::bail!(
                "{}",
                Diagnostic::error(
                    DiagnosticCode::X7I0111UnsupportedParamPattern,
                    Phase::Validate,
                    "receiver arguments are not supported"
                )
            );
        };
        let syn::Pat::Ident(pat_ident) = &*pat_ty.pat else {
            anyhow::bail!(
                "{}",
                Diagnostic::error(
                    DiagnosticCode::X7I0111UnsupportedParamPattern,
                    Phase::Validate,
                    "only ident parameters are supported"
                )
            );
        };
        let name = pat_ident.ident.to_string();
        let (ty, scalar_ty) = lower_ty(&pat_ty.ty)?;
        params.push(X07Param {
            name: name.clone(),
            ty,
        });
        ctx.declare(&name, scalar_ty);
    }

    let ret_ty = match &f.sig.output {
        syn::ReturnType::Default => X07Ty::Unit,
        syn::ReturnType::Type(_, ty) => lower_ty(ty)?.0,
    };

    let body = lower_block(&mut ctx, &f.block)?;

    ctx.exit_scope();

    Ok(X07Func {
        name: full_name,
        exported,
        params,
        ret: ret_ty,
        body,
    })
}

fn lower_ty(ty: &syn::Type) -> Result<(X07Ty, ScalarTy)> {
    match ty {
        syn::Type::Path(p) => {
            if p.qself.is_some() {
                anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0120UnsupportedType,
                        Phase::Validate,
                        "qualified self types are not supported"
                    )
                );
            }
            let Some(ident) = p.path.get_ident() else {
                anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0120UnsupportedType,
                        Phase::Validate,
                        "only single-ident types are supported"
                    )
                );
            };
            match ident.to_string().as_str() {
                "i32" => Ok((X07Ty::I32, ScalarTy::I32)),
                "u32" => Ok((X07Ty::U32, ScalarTy::I32)),
                "u8" => Ok((X07Ty::U8, ScalarTy::I32)),
                "usize" => Ok((X07Ty::U32, ScalarTy::I32)),
                "bool" => Ok((X07Ty::Bool, ScalarTy::I32)),
                "Bytes" => Ok((X07Ty::Bytes, ScalarTy::Bytes)),
                "BytesView" => Ok((X07Ty::BytesView, ScalarTy::BytesView)),
                "VecU8" => Ok((X07Ty::VecU8, ScalarTy::VecU8)),
                other => anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0120UnsupportedType,
                        Phase::Validate,
                        format!("unsupported type: {other}")
                    )
                ),
            }
        }
        syn::Type::Reference(r) => {
            if r.mutability.is_some() {
                anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0123UnsupportedMutRef,
                        Phase::Validate,
                        "mutable references are not supported"
                    )
                );
            }
            match &*r.elem {
                syn::Type::Slice(s) => match &*s.elem {
                    syn::Type::Path(p) if p.path.is_ident("u8") => {
                        Ok((X07Ty::Bytes, ScalarTy::Bytes))
                    }
                    _ => anyhow::bail!(
                        "{}",
                        Diagnostic::error(
                            DiagnosticCode::X7I0122UnsupportedRefType,
                            Phase::Validate,
                            "only &[u8] is supported"
                        )
                    ),
                },
                _ => anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0122UnsupportedRefType,
                        Phase::Validate,
                        "only &[u8] is supported"
                    )
                ),
            }
        }
        _ => anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0120UnsupportedType,
                Phase::Validate,
                "unsupported type expression"
            )
        ),
    }
}

fn lower_block(ctx: &mut Ctx<'_>, b: &syn::Block) -> Result<Vec<X07Stmt>> {
    ctx.enter_scope();
    let mut out: Vec<X07Stmt> = Vec::new();
    for stmt in &b.stmts {
        lower_stmt(ctx, stmt, &mut out)?;
    }
    ctx.exit_scope();
    Ok(out)
}

fn lower_stmt(ctx: &mut Ctx<'_>, stmt: &syn::Stmt, out: &mut Vec<X07Stmt>) -> Result<()> {
    match stmt {
        syn::Stmt::Local(local) => {
            let (name, ty_opt): (String, Option<&syn::Type>) = match &local.pat {
                syn::Pat::Ident(pat_ident) => (pat_ident.ident.to_string(), None),
                syn::Pat::Type(pat_ty) => {
                    let syn::Pat::Ident(pat_ident) = &*pat_ty.pat else {
                        anyhow::bail!(
                            "{}",
                            Diagnostic::error(
                                DiagnosticCode::X7I0200UnsupportedLetPattern,
                                Phase::Validate,
                                "only ident let bindings are supported"
                            )
                        );
                    };
                    (pat_ident.ident.to_string(), Some(&*pat_ty.ty))
                }
                _ => {
                    anyhow::bail!(
                        "{}",
                        Diagnostic::error(
                            DiagnosticCode::X7I0200UnsupportedLetPattern,
                            Phase::Validate,
                            "only ident let bindings are supported"
                        )
                    );
                }
            };
            let Some(init) = &local.init else {
                anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0201MissingLetInit,
                        Phase::Validate,
                        "let bindings must have an initializer"
                    )
                );
            };
            let expr = lower_expr(ctx, &init.expr)?;
            // Local types are not represented in emitted X07, but we track a tiny set of
            // scalar categories for deterministic builtin lowering.
            if let Some(ty) = ty_opt {
                let (_x07_ty, scalar) = lower_ty(ty)?;
                ctx.declare(&name, scalar);
            } else {
                ctx.declare(&name, ScalarTy::I32);
            }
            out.push(X07Stmt::Let { name, init: expr });
            Ok(())
        }
        syn::Stmt::Item(_) => anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0210UnsupportedStmtItem,
                Phase::Validate,
                "items inside blocks are not supported"
            )
        ),
        syn::Stmt::Macro(_) => anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0211UnsupportedStmtMacro,
                Phase::Validate,
                "macros are not supported"
            )
        ),
        syn::Stmt::Expr(e, _semi) => {
            let lowered = lower_expr(ctx, e)?;
            out.push(X07Stmt::Expr(lowered));
            Ok(())
        }
    }
}

fn lower_expr(ctx: &mut Ctx<'_>, e: &syn::Expr) -> Result<X07Expr> {
    match e {
        syn::Expr::Lit(l) => lower_lit(l),
        syn::Expr::Path(p) => {
            let Some(ident) = p.path.get_ident() else {
                anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0310UnsupportedPath,
                        Phase::Validate,
                        "only single-ident paths are supported"
                    )
                );
            };
            let name = ident.to_string();
            if ctx.lookup(&name).is_some() {
                return Ok(X07Expr::Ident(name));
            }
            anyhow::bail!(
                "{}",
                Diagnostic::error(
                    DiagnosticCode::X7I0311UnknownName,
                    Phase::Validate,
                    format!("unknown name: {name}")
                )
            );
        }
        syn::Expr::Call(call) => lower_call(ctx, call),
        syn::Expr::Binary(b) => lower_bin(ctx, b),
        syn::Expr::Unary(u) => lower_unary(ctx, u),
        syn::Expr::If(i) => lower_if(ctx, i),
        syn::Expr::Return(r) => {
            let Some(expr) = &r.expr else {
                return Ok(X07Expr::Call {
                    head: "return".to_string(),
                    args: vec![X07Expr::Int(0)],
                });
            };
            Ok(X07Expr::Call {
                head: "return".to_string(),
                args: vec![lower_expr(ctx, expr)?],
            })
        }
        syn::Expr::Assign(a) => {
            let syn::Expr::Path(p) = &*a.left else {
                anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0200UnsupportedLetPattern,
                        Phase::Validate,
                        "only assignments to a single ident are supported"
                    )
                );
            };
            let Some(ident) = p.path.get_ident() else {
                anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0310UnsupportedPath,
                        Phase::Validate,
                        "only assignments to a single ident are supported"
                    )
                );
            };
            let name = ident.to_string();
            if ctx.lookup(&name).is_none() {
                anyhow::bail!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0311UnknownName,
                        Phase::Validate,
                        format!("assignment to unknown name: {name}")
                    )
                );
            }
            Ok(X07Expr::Call {
                head: "set".to_string(),
                args: vec![X07Expr::Ident(name), lower_expr(ctx, &a.right)?],
            })
        }
        syn::Expr::ForLoop(fl) => lower_for(ctx, fl),
        syn::Expr::Block(b) => Ok(expr_from_block(ctx, &b.block)?),
        syn::Expr::Paren(p) => lower_expr(ctx, &p.expr),
        syn::Expr::Group(g) => lower_expr(ctx, &g.expr),
        other => anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0901InternalBug,
                Phase::Lower,
                format!("unsupported expression kind: {:?}", other.span())
            )
        ),
    }
}

fn lower_lit(l: &syn::ExprLit) -> Result<X07Expr> {
    match &l.lit {
        syn::Lit::Int(i) => {
            let val: i64 = i.base10_parse().map_err(|_| {
                anyhow::anyhow!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0301IntOutOfRange,
                        Phase::Validate,
                        "integer literal parse failed"
                    )
                )
            })?;
            let val: i32 = val.try_into().map_err(|_| {
                anyhow::anyhow!(
                    "{}",
                    Diagnostic::error(
                        DiagnosticCode::X7I0301IntOutOfRange,
                        Phase::Validate,
                        "integer literal out of i32 range"
                    )
                )
            })?;
            Ok(X07Expr::Int(val))
        }
        syn::Lit::Bool(b) => Ok(X07Expr::Int(if b.value { 1 } else { 0 })),
        syn::Lit::Byte(b) => Ok(X07Expr::Int(i32::from(b.value()))),
        _ => anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0300UnsupportedLiteral,
                Phase::Validate,
                "unsupported literal type"
            )
        ),
    }
}

fn lower_call(ctx: &mut Ctx<'_>, call: &syn::ExprCall) -> Result<X07Expr> {
    let syn::Expr::Path(p) = &*call.func else {
        anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0320UnsupportedCallee,
                Phase::Validate,
                "call target must be an ident"
            )
        );
    };
    let Some(ident) = p.path.get_ident() else {
        anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0310UnsupportedPath,
                Phase::Validate,
                "call target must be a single ident"
            )
        );
    };
    let name = ident.to_string();

    let head = if let Some(builtin) = builtin_map().get(name.as_str()) {
        builtin.to_string()
    } else if ctx.local_fns.contains(&name) {
        format!("{}.{}", ctx.module_id, name)
    } else {
        anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0311UnknownName,
                Phase::Validate,
                format!("unknown function: {name}")
            )
        );
    };

    let mut args = Vec::with_capacity(call.args.len());
    for a in &call.args {
        args.push(lower_expr(ctx, a)?);
    }
    Ok(X07Expr::Call { head, args })
}

fn builtin_map() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("lt_u", "<u"),
        ("ge_u", ">=u"),
        ("bytes_alloc", "bytes.alloc"),
        ("bytes_len", "bytes.len"),
        ("bytes_get_u8", "bytes.get_u8"),
        ("bytes_set_u8", "bytes.set_u8"),
        ("bytes_slice", "bytes.slice"),
        ("bytes_view", "bytes.view"),
        ("bytes_subview", "bytes.subview"),
        ("view_len", "view.len"),
        ("view_get_u8", "view.get_u8"),
        ("view_slice", "view.slice"),
        ("view_to_bytes", "view.to_bytes"),
        ("vec_u8_with_capacity", "vec_u8.with_capacity"),
        ("vec_u8_len", "vec_u8.len"),
        ("vec_u8_get", "vec_u8.get"),
        ("vec_u8_push", "vec_u8.push"),
        ("vec_u8_reserve_exact", "vec_u8.reserve_exact"),
        ("vec_u8_extend_bytes", "vec_u8.extend_bytes"),
        ("vec_u8_extend_bytes_range", "vec_u8.extend_bytes_range"),
        ("vec_u8_into_bytes", "vec_u8.into_bytes"),
        ("vec_u8_as_view", "vec_u8.as_view"),
        ("codec_read_u32_le", "codec.read_u32_le"),
        ("codec_write_u32_le", "codec.write_u32_le"),
        ("fmt_u32_to_dec", "fmt.u32_to_dec"),
        ("fmt_s32_to_dec", "fmt.s32_to_dec"),
        ("parse_u32_dec", "parse.u32_dec"),
        ("parse_u32_dec_at", "parse.u32_dec_at"),
        ("parse_i32_status_le", "parse.i32_status_le"),
        ("parse_i32_status_le_at", "parse.i32_status_le_at"),
        ("io_read", "io.read"),
        ("bufread_new", "bufread.new"),
        ("bufread_fill", "bufread.fill"),
        ("bufread_consume", "bufread.consume"),
        ("fs_read", "fs.read"),
        ("rr_send", "rr.send"),
        ("kv_get", "kv.get"),
        ("kv_set", "kv.set"),
    ])
}

fn lower_bin(ctx: &mut Ctx<'_>, b: &syn::ExprBinary) -> Result<X07Expr> {
    use syn::BinOp;
    let a = lower_expr(ctx, &b.left)?;
    let c = lower_expr(ctx, &b.right)?;
    let head = match &b.op {
        // Arithmetic
        BinOp::Add(_) => "+",
        BinOp::Sub(_) => "-",
        BinOp::Mul(_) => "*",
        BinOp::Div(_) => "/",
        BinOp::Rem(_) => "%",
        // Bitwise
        BinOp::BitAnd(_) => "&",
        BinOp::BitOr(_) => "|",
        BinOp::BitXor(_) => "^",
        BinOp::Shl(_) => "<<u",
        BinOp::Shr(_) => ">>u",
        // Comparison
        BinOp::Eq(_) => "=",
        BinOp::Lt(_) => "<",
        BinOp::Gt(_) => ">",
        BinOp::Le(_) => "<=",
        BinOp::Ge(_) => ">=",
        // != becomes (= (= a b) 0)
        BinOp::Ne(_) => {
            return Ok(X07Expr::Call {
                head: "=".to_string(),
                args: vec![
                    X07Expr::Call {
                        head: "=".to_string(),
                        args: vec![a, c],
                    },
                    X07Expr::Int(0),
                ],
            });
        }
        // Logical (short-circuit)
        BinOp::Or(_) => {
            return Ok(X07Expr::If {
                cond: Box::new(a),
                then_e: Box::new(X07Expr::Int(1)),
                else_e: Box::new(c),
            })
        }
        BinOp::And(_) => {
            return Ok(X07Expr::If {
                cond: Box::new(a),
                then_e: Box::new(c),
                else_e: Box::new(X07Expr::Int(0)),
            })
        }
        _ => {
            anyhow::bail!(
                "{}",
                Diagnostic::error(
                    DiagnosticCode::X7I0340UnsupportedBinOp,
                    Phase::Validate,
                    "unsupported binary operator"
                )
            );
        }
    };
    Ok(X07Expr::Call {
        head: head.to_string(),
        args: vec![a, c],
    })
}

fn lower_unary(ctx: &mut Ctx<'_>, u: &syn::ExprUnary) -> Result<X07Expr> {
    use syn::UnOp;
    match &u.op {
        UnOp::Neg(_) => {
            let inner = lower_expr(ctx, &u.expr)?;
            if let X07Expr::Int(i) = inner {
                if i == i32::MIN {
                    Ok(X07Expr::Call {
                        head: "-".to_string(),
                        args: vec![X07Expr::Int(0), X07Expr::Int(i)],
                    })
                } else {
                    Ok(X07Expr::Int(-i))
                }
            } else {
                Ok(X07Expr::Call {
                    head: "-".to_string(),
                    args: vec![X07Expr::Int(0), inner],
                })
            }
        }
        // !x  ==>  (= x 0)
        UnOp::Not(_) => Ok(X07Expr::Call {
            head: "=".to_string(),
            args: vec![lower_expr(ctx, &u.expr)?, X07Expr::Int(0)],
        }),
        _ => anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0300UnsupportedLiteral,
                Phase::Validate,
                "unsupported unary operator"
            )
        ),
    }
}

fn lower_if(ctx: &mut Ctx<'_>, i: &syn::ExprIf) -> Result<X07Expr> {
    let cond = lower_expr(ctx, &i.cond)?;
    let then_body = lower_block(ctx, &i.then_branch)?;

    let then_e = stmts_to_expr(&then_body);
    let else_e = if let Some((_else_tok, else_expr)) = &i.else_branch {
        match &**else_expr {
            syn::Expr::Block(b) => stmts_to_expr(&lower_block(ctx, &b.block)?),
            syn::Expr::If(elif) => lower_if(ctx, elif)?,
            other => lower_expr(ctx, other)?,
        }
    } else {
        X07Expr::Int(0)
    };

    Ok(X07Expr::If {
        cond: Box::new(cond),
        then_e: Box::new(then_e),
        else_e: Box::new(else_e),
    })
}

fn lower_for(ctx: &mut Ctx<'_>, fl: &syn::ExprForLoop) -> Result<X07Expr> {
    let var = match &*fl.pat {
        syn::Pat::Ident(pat_ident) => pat_ident.ident.to_string(),
        syn::Pat::Wild(_) => "_".to_string(),
        _ => {
            anyhow::bail!(
                "{}",
                Diagnostic::error(
                    DiagnosticCode::X7I0360UnsupportedForIter,
                    Phase::Validate,
                    "for-loop pattern must be an ident or '_'"
                )
            );
        }
    };

    let syn::Expr::Range(r) = &*fl.expr else {
        anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0360UnsupportedForIter,
                Phase::Validate,
                "for-loop iterator must be a range"
            )
        );
    };
    if r.limits != syn::RangeLimits::HalfOpen(syn::token::DotDot::default()) {
        anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0360UnsupportedForIter,
                Phase::Validate,
                "only half-open ranges (start..end) are supported"
            )
        );
    }
    let Some(end) = &r.end else {
        anyhow::bail!(
            "{}",
            Diagnostic::error(
                DiagnosticCode::X7I0360UnsupportedForIter,
                Phase::Validate,
                "range end must be present"
            )
        );
    };
    let start = match &r.start {
        Some(s) => lower_expr(ctx, s)?,
        None => X07Expr::Int(0),
    };
    let end = lower_expr(ctx, end)?;

    ctx.enter_scope();
    ctx.declare(&var, ScalarTy::I32);
    let body = lower_block(ctx, &fl.body)?;
    ctx.exit_scope();

    Ok(X07Expr::Call {
        head: "for".to_string(),
        args: vec![X07Expr::Ident(var), start, end, stmts_to_expr(&body)],
    })
}

fn expr_from_block(ctx: &mut Ctx<'_>, b: &syn::Block) -> Result<X07Expr> {
    let stmts = lower_block(ctx, b)?;
    Ok(stmts_to_expr(&stmts))
}

fn stmts_to_expr(stmts: &[X07Stmt]) -> X07Expr {
    if stmts.is_empty() {
        return X07Expr::Int(0);
    }
    if stmts.len() == 1 {
        return match &stmts[0] {
            X07Stmt::Expr(e) => e.clone(),
            X07Stmt::Let { name, init } => X07Expr::Call {
                head: "begin".to_string(),
                args: vec![
                    X07Expr::Call {
                        head: "let".to_string(),
                        args: vec![X07Expr::Ident(name.clone()), init.clone()],
                    },
                    X07Expr::Int(0),
                ],
            },
            X07Stmt::Set { name, value } => X07Expr::Call {
                head: "set".to_string(),
                args: vec![X07Expr::Ident(name.clone()), value.clone()],
            },
            X07Stmt::Return(e) => X07Expr::Call {
                head: "return".to_string(),
                args: vec![e.clone()],
            },
            X07Stmt::If {
                cond,
                then_body,
                else_body,
            } => X07Expr::Call {
                head: "if".to_string(),
                args: vec![
                    cond.clone(),
                    stmts_to_expr(then_body),
                    stmts_to_expr(else_body),
                ],
            },
            X07Stmt::ForRange {
                var,
                start,
                end,
                body,
            } => X07Expr::Call {
                head: "for".to_string(),
                args: vec![
                    X07Expr::Ident(var.clone()),
                    start.clone(),
                    end.clone(),
                    stmts_to_expr(body),
                ],
            },
        };
    }

    let mut args: Vec<X07Expr> = Vec::with_capacity(stmts.len());
    for s in stmts {
        match s {
            X07Stmt::Let { name, init } => args.push(X07Expr::Call {
                head: "let".to_string(),
                args: vec![X07Expr::Ident(name.clone()), init.clone()],
            }),
            X07Stmt::Set { name, value } => args.push(X07Expr::Call {
                head: "set".to_string(),
                args: vec![X07Expr::Ident(name.clone()), value.clone()],
            }),
            X07Stmt::Expr(e) => args.push(e.clone()),
            X07Stmt::Return(e) => args.push(X07Expr::Call {
                head: "return".to_string(),
                args: vec![e.clone()],
            }),
            X07Stmt::If {
                cond,
                then_body,
                else_body,
            } => args.push(X07Expr::Call {
                head: "if".to_string(),
                args: vec![
                    cond.clone(),
                    stmts_to_expr(then_body),
                    stmts_to_expr(else_body),
                ],
            }),
            X07Stmt::ForRange {
                var,
                start,
                end,
                body,
            } => args.push(X07Expr::Call {
                head: "for".to_string(),
                args: vec![
                    X07Expr::Ident(var.clone()),
                    start.clone(),
                    end.clone(),
                    stmts_to_expr(body),
                ],
            }),
        }
    }
    X07Expr::Call {
        head: "begin".to_string(),
        args,
    }
}
