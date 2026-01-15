use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::x07ir::{X07Expr, X07Func, X07Param, X07Stmt, X07Ty};

use super::validate::CFunction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarTy {
    I32,
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

pub fn lower_module(module_id: &str, funcs: &[CFunction]) -> Result<Vec<X07Func>> {
    let mut local_fns = BTreeSet::new();
    for f in funcs {
        local_fns.insert(f.name.clone());
    }

    let mut out: Vec<X07Func> = Vec::new();
    for f in funcs {
        out.push(lower_fn(module_id, &local_fns, f)?);
    }
    Ok(out)
}

fn lower_fn(module_id: &str, local_fns: &BTreeSet<String>, f: &CFunction) -> Result<X07Func> {
    let full_name = format!("{module_id}.{}", f.name);
    let mut params: Vec<X07Param> = Vec::new();

    let mut ctx = Ctx::new(module_id, local_fns);
    ctx.enter_scope();
    for p in &f.params {
        let (ty, scalar) = lower_ty_str(&p.ty)?;
        params.push(X07Param {
            name: p.name.clone(),
            ty,
        });
        ctx.declare(&p.name, scalar);
    }

    let ret_ty = lower_ret_ty_str(&f.ret_ty)?;
    let body = lower_compound_stmt(&mut ctx, &f.body)?;
    ctx.exit_scope();

    Ok(X07Func {
        name: full_name,
        exported: true,
        params,
        ret: ret_ty,
        body,
    })
}

fn lower_ty_str(ty: &str) -> Result<(X07Ty, ScalarTy)> {
    let t = ty.trim();
    match t {
        "int" | "signed int" | "int32_t" => Ok((X07Ty::I32, ScalarTy::I32)),
        "unsigned int" | "uint32_t" | "size_t" => Ok((X07Ty::U32, ScalarTy::I32)),
        "uint8_t" | "unsigned char" => Ok((X07Ty::U8, ScalarTy::I32)),
        other => anyhow::bail!("unsupported C type: {other}"),
    }
}

fn lower_ret_ty_str(ty: &str) -> Result<X07Ty> {
    Ok(lower_ty_str(ty)?.0)
}

fn lower_compound_stmt(ctx: &mut Ctx<'_>, node: &Value) -> Result<Vec<X07Stmt>> {
    ctx.enter_scope();
    let mut out: Vec<X07Stmt> = Vec::new();
    if let Some(items) = node.get("inner").and_then(|v| v.as_array()) {
        for item in items {
            lower_stmt(ctx, item, &mut out)?;
        }
    }
    ctx.exit_scope();
    Ok(out)
}

fn lower_stmt(ctx: &mut Ctx<'_>, node: &Value, out: &mut Vec<X07Stmt>) -> Result<()> {
    match node_kind(node) {
        Some("NullStmt") => Ok(()),
        Some("ReturnStmt") => {
            let expr = node
                .get("inner")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .map(|e| lower_expr(ctx, e))
                .transpose()?
                .unwrap_or(X07Expr::Int(0));
            out.push(X07Stmt::Return(expr));
            Ok(())
        }
        Some("DeclStmt") => {
            let Some(decls) = node.get("inner").and_then(|v| v.as_array()) else {
                return Ok(());
            };
            for d in decls {
                if node_kind(d) != Some("VarDecl") {
                    anyhow::bail!(
                        "unsupported decl in DeclStmt: {}",
                        node_kind(d).unwrap_or("?")
                    );
                }
                let name = d
                    .get("name")
                    .and_then(|v| v.as_str())
                    .context("VarDecl missing name")?
                    .to_string();
                let ty = d
                    .get("type")
                    .and_then(|t| t.get("qualType"))
                    .and_then(|v| v.as_str())
                    .context("VarDecl missing type.qualType")?;
                let (_x07_ty, scalar) = lower_ty_str(ty)?;

                let init = d
                    .get("inner")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .context("VarDecl missing initializer")?;
                let init = lower_expr(ctx, init)?;
                ctx.declare(&name, scalar);
                out.push(X07Stmt::Let { name, init });
            }
            Ok(())
        }
        Some("IfStmt") => {
            let Some(inner) = node.get("inner").and_then(|v| v.as_array()) else {
                anyhow::bail!("IfStmt missing inner");
            };
            let Some(cond_node) = inner.first() else {
                anyhow::bail!("IfStmt missing condition");
            };
            let cond = lower_expr(ctx, cond_node)?;

            let then_node = inner.get(1).context("IfStmt missing then")?;
            let then_body = lower_stmt_as_block(ctx, then_node)?;

            let else_body = if let Some(else_node) = inner.get(2) {
                lower_stmt_as_block(ctx, else_node)?
            } else {
                Vec::new()
            };

            out.push(X07Stmt::If {
                cond,
                then_body,
                else_body,
            });
            Ok(())
        }
        Some("CompoundStmt") => {
            out.push(X07Stmt::Expr(expr_from_block(ctx, node)?));
            Ok(())
        }
        Some("BinaryOperator") => {
            if is_assign_op(node, "=") {
                let (lhs, rhs) = binop_children(node)?;
                let lhs = unwrap_implicit(lhs);
                let rhs = unwrap_implicit(rhs);
                let name = declref_name(lhs).context("assignment LHS must be a declref")?;
                if ctx.lookup(name).is_none() {
                    anyhow::bail!("assignment to unknown name: {name}");
                }
                out.push(X07Stmt::Set {
                    name: name.to_string(),
                    value: lower_expr(ctx, rhs)?,
                });
                return Ok(());
            }
            out.push(X07Stmt::Expr(lower_expr(ctx, node)?));
            Ok(())
        }
        _ => {
            // Fallback: expression statement.
            out.push(X07Stmt::Expr(lower_expr(ctx, node)?));
            Ok(())
        }
    }
}

fn lower_stmt_as_block(ctx: &mut Ctx<'_>, node: &Value) -> Result<Vec<X07Stmt>> {
    match node_kind(node) {
        Some("CompoundStmt") => lower_compound_stmt(ctx, node),
        _ => {
            ctx.enter_scope();
            let mut out = Vec::new();
            lower_stmt(ctx, node, &mut out)?;
            ctx.exit_scope();
            Ok(out)
        }
    }
}

fn expr_from_block(ctx: &mut Ctx<'_>, node: &Value) -> Result<X07Expr> {
    let stmts = lower_compound_stmt(ctx, node)?;
    Ok(stmts_to_expr(&stmts))
}

fn lower_expr(ctx: &mut Ctx<'_>, node: &Value) -> Result<X07Expr> {
    let node = unwrap_implicit(node);
    match node_kind(node) {
        Some("IntegerLiteral") => {
            let v = node
                .get("value")
                .and_then(|v| v.as_str())
                .context("IntegerLiteral missing value")?;
            let val: i64 = v
                .parse()
                .with_context(|| format!("parse int literal {v:?}"))?;
            let val: i32 = val
                .try_into()
                .with_context(|| format!("int literal out of range: {v}"))?;
            Ok(X07Expr::Int(val))
        }
        Some("DeclRefExpr") => {
            let name = declref_name(node).context("DeclRefExpr missing referencedDecl.name")?;
            if ctx.lookup(name).is_some() {
                return Ok(X07Expr::Ident(name.to_string()));
            }
            anyhow::bail!("unknown name: {name}");
        }
        Some("BinaryOperator") => {
            if is_assign_op(node, "=") {
                anyhow::bail!("assignment is only supported as a statement");
            }
            let (lhs, rhs) = binop_children(node)?;
            let a = lower_expr(ctx, lhs)?;
            let b = lower_expr(ctx, rhs)?;
            let op = node
                .get("opcode")
                .and_then(|v| v.as_str())
                .context("BinaryOperator missing opcode")?;
            let head = match op {
                "+" => "+",
                "-" => "-",
                "*" => "*",
                "&" => "&",
                "==" => "=",
                "<" => "<",
                "||" => {
                    return Ok(X07Expr::If {
                        cond: Box::new(a),
                        then_e: Box::new(X07Expr::Int(1)),
                        else_e: Box::new(b),
                    })
                }
                "&&" => {
                    return Ok(X07Expr::If {
                        cond: Box::new(a),
                        then_e: Box::new(b),
                        else_e: Box::new(X07Expr::Int(0)),
                    })
                }
                other => anyhow::bail!("unsupported binary operator: {other}"),
            };
            Ok(X07Expr::Call {
                head: head.to_string(),
                args: vec![a, b],
            })
        }
        Some("UnaryOperator") => {
            let op = node
                .get("opcode")
                .and_then(|v| v.as_str())
                .context("UnaryOperator missing opcode")?;
            let inner = node
                .get("inner")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .context("UnaryOperator missing inner expr")?;
            let inner = lower_expr(ctx, inner)?;
            match op {
                "-" => {
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
                "!" => Ok(X07Expr::Call {
                    head: "=".to_string(),
                    args: vec![inner, X07Expr::Int(0)],
                }),
                "+" => Ok(inner),
                other => anyhow::bail!("unsupported unary operator: {other}"),
            }
        }
        Some("CallExpr") => {
            let Some(inner) = node.get("inner").and_then(|v| v.as_array()) else {
                anyhow::bail!("CallExpr missing inner");
            };
            let callee = inner.first().context("CallExpr missing callee")?;
            let callee = unwrap_implicit(callee);
            let Some(name) = declref_name(callee) else {
                anyhow::bail!("CallExpr callee must be a declref");
            };

            let head = if let Some(b) = builtin_map().get(name) {
                b.to_string()
            } else if ctx.local_fns.contains(name) {
                format!("{}.{}", ctx.module_id, name)
            } else {
                anyhow::bail!("unknown function: {name}");
            };

            let mut args: Vec<X07Expr> = Vec::with_capacity(inner.len().saturating_sub(1));
            for a in inner.iter().skip(1) {
                args.push(lower_expr(ctx, a)?);
            }

            Ok(X07Expr::Call { head, args })
        }
        other => anyhow::bail!("unsupported expression kind: {}", other.unwrap_or("?")),
    }
}

fn unwrap_implicit(mut node: &Value) -> &Value {
    while let Some("ImplicitCastExpr") | Some("ParenExpr") | Some("CStyleCastExpr") =
        node_kind(node)
    {
        let Some(inner) = node.get("inner").and_then(|v| v.as_array()) else {
            break;
        };
        let Some(first) = inner.first() else {
            break;
        };
        node = first;
    }
    node
}

fn declref_name(node: &Value) -> Option<&str> {
    node.get("referencedDecl")
        .and_then(|d| d.get("name"))
        .and_then(|v| v.as_str())
}

fn is_assign_op(node: &Value, op: &str) -> bool {
    node.get("opcode").and_then(|v| v.as_str()) == Some(op)
}

fn binop_children(node: &Value) -> Result<(&Value, &Value)> {
    let inner = node
        .get("inner")
        .and_then(|v| v.as_array())
        .context("BinaryOperator missing inner")?;
    let a = inner.first().context("BinaryOperator missing lhs")?;
    let b = inner.get(1).context("BinaryOperator missing rhs")?;
    Ok((a, b))
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

fn builtin_map() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([("lt_u", "<u"), ("ge_u", ">=u")])
}

fn node_kind(node: &Value) -> Option<&str> {
    node.get("kind").and_then(|v| v.as_str())
}
