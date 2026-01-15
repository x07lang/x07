use anyhow::Result;
use serde_json::Value;
use x07_contracts::X07AST_SCHEMA_VERSION;

use crate::x07ir::{X07Expr, X07Module, X07Stmt, X07Ty};

pub fn emit_module(m: &X07Module) -> Result<String> {
    let v = emit_module_value(m);
    Ok(serde_json::to_string(&v)? + "\n")
}

fn emit_module_value(m: &X07Module) -> Value {
    let mut root = serde_json::Map::new();
    root.insert(
        "schema_version".to_string(),
        Value::String(X07AST_SCHEMA_VERSION.to_string()),
    );
    root.insert("kind".to_string(), Value::String("module".to_string()));
    root.insert("module_id".to_string(), Value::String(m.module_id.clone()));
    root.insert("imports".to_string(), Value::Array(Vec::new()));

    let exports: Vec<String> = {
        let mut xs: Vec<String> = m
            .funcs
            .iter()
            .filter(|f| f.exported)
            .map(|f| f.name.clone())
            .collect();
        xs.sort();
        xs
    };

    let mut decls: Vec<Value> = Vec::new();
    if !exports.is_empty() {
        let mut exp = serde_json::Map::new();
        exp.insert("kind".to_string(), Value::String("export".to_string()));
        exp.insert(
            "names".to_string(),
            Value::Array(exports.into_iter().map(Value::String).collect()),
        );
        decls.push(Value::Object(exp));
    }

    let mut funcs = m.funcs.clone();
    funcs.sort_by(|a, b| a.name.cmp(&b.name));

    for f in &funcs {
        let mut decl = serde_json::Map::new();
        decl.insert("kind".to_string(), Value::String("defn".to_string()));
        decl.insert("name".to_string(), Value::String(f.name.clone()));
        decl.insert(
            "params".to_string(),
            Value::Array(
                f.params
                    .iter()
                    .map(|p| {
                        let mut pm = serde_json::Map::new();
                        pm.insert("name".to_string(), Value::String(p.name.clone()));
                        pm.insert("ty".to_string(), Value::String(emit_ty(p.ty).to_string()));
                        Value::Object(pm)
                    })
                    .collect(),
            ),
        );
        decl.insert(
            "result".to_string(),
            Value::String(emit_ty(f.ret).to_string()),
        );
        decl.insert("body".to_string(), stmts_to_expr_value(&f.body));
        decls.push(Value::Object(decl));
    }

    root.insert("decls".to_string(), Value::Array(decls));

    let mut meta = serde_json::Map::new();
    meta.insert(
        "generated_by".to_string(),
        Value::String("x07import".to_string()),
    );
    if let Some(src) = &m.source_path {
        meta.insert("source_path".to_string(), Value::String(src.clone()));
    }
    if let Some(sha) = &m.source_sha256 {
        meta.insert("source_sha256".to_string(), Value::String(sha.clone()));
    }
    root.insert("meta".to_string(), Value::Object(meta));

    Value::Object(root)
}

fn stmts_to_expr_value(stmts: &[X07Stmt]) -> Value {
    let exprs: Vec<Value> = stmts.iter().map(stmt_to_expr_value).collect();
    match exprs.len() {
        0 => Value::Number(0.into()),
        1 => exprs.into_iter().next().unwrap(),
        _ => {
            let mut items = Vec::with_capacity(exprs.len() + 1);
            items.push(Value::String("begin".to_string()));
            items.extend(exprs);
            Value::Array(items)
        }
    }
}

fn stmt_to_expr_value(s: &X07Stmt) -> Value {
    match s {
        X07Stmt::Let { name, init } => Value::Array(vec![
            Value::String("let".to_string()),
            Value::String(name.clone()),
            expr_to_value(init),
        ]),
        X07Stmt::Set { name, value } => Value::Array(vec![
            Value::String("set".to_string()),
            Value::String(name.clone()),
            expr_to_value(value),
        ]),
        X07Stmt::Expr(e) => expr_to_value(e),
        X07Stmt::Return(e) => {
            Value::Array(vec![Value::String("return".to_string()), expr_to_value(e)])
        }
        X07Stmt::If {
            cond,
            then_body,
            else_body,
        } => Value::Array(vec![
            Value::String("if".to_string()),
            expr_to_value(cond),
            stmts_to_expr_value(then_body),
            stmts_to_expr_value(else_body),
        ]),
        X07Stmt::ForRange {
            var,
            start,
            end,
            body,
        } => Value::Array(vec![
            Value::String("for".to_string()),
            Value::String(var.clone()),
            expr_to_value(start),
            expr_to_value(end),
            stmts_to_expr_value(body),
        ]),
    }
}

fn expr_to_value(e: &X07Expr) -> Value {
    match e {
        X07Expr::Int(i) => Value::Number((*i).into()),
        X07Expr::Ident(s) => Value::String(s.clone()),
        X07Expr::Call { head, args } => {
            let mut items = Vec::with_capacity(args.len() + 1);
            items.push(Value::String(head.clone()));
            items.extend(args.iter().map(expr_to_value));
            Value::Array(items)
        }
        X07Expr::If {
            cond,
            then_e,
            else_e,
        } => Value::Array(vec![
            Value::String("if".to_string()),
            expr_to_value(cond),
            expr_to_value(then_e),
            expr_to_value(else_e),
        ]),
    }
}

fn emit_ty(ty: X07Ty) -> &'static str {
    match ty {
        X07Ty::Unit => "i32",
        X07Ty::Bool => "i32",
        X07Ty::U8 => "i32",
        X07Ty::I32 => "i32",
        X07Ty::U32 => "i32",
        X07Ty::Bytes => "bytes",
        X07Ty::BytesView => "bytes_view",
        X07Ty::VecU8 => "vec_u8",
    }
}
