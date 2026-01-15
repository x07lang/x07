use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i32),
    Ident(String),
    List(Vec<Expr>),
}

impl Expr {
    pub fn node_count(&self) -> usize {
        match self {
            Expr::Int(_) | Expr::Ident(_) => 1,
            Expr::List(items) => 1 + items.iter().map(Expr::node_count).sum::<usize>(),
        }
    }

    pub fn max_depth(&self) -> usize {
        match self {
            Expr::Int(_) | Expr::Ident(_) => 1,
            Expr::List(items) => 1 + items.iter().map(Expr::max_depth).max().unwrap_or(0),
        }
    }

    pub fn as_ident(&self) -> Option<&str> {
        match self {
            Expr::Ident(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

pub fn expr_from_json(v: &Value) -> Result<Expr, String> {
    match v {
        Value::Number(n) => {
            let i = n
                .as_i64()
                .ok_or_else(|| format!("number is not an i64: {n}"))?;
            let i32_ = i32::try_from(i).map_err(|_| format!("number out of i32 range: {i}"))?;
            Ok(Expr::Int(i32_))
        }
        Value::String(s) => Ok(Expr::Ident(s.to_string())),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(expr_from_json(item)?);
            }
            Ok(Expr::List(out))
        }
        _ => Err(format!("unsupported JSON value in expr: {v:?}")),
    }
}
