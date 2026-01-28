use serde_json::Value;

#[derive(Debug, Clone)]
pub enum Expr {
    Int { value: i32, ptr: String },
    Ident { name: String, ptr: String },
    List { items: Vec<Expr>, ptr: String },
}

impl Expr {
    pub fn ptr(&self) -> &str {
        match self {
            Expr::Int { ptr, .. } | Expr::Ident { ptr, .. } | Expr::List { ptr, .. } => ptr,
        }
    }

    pub fn node_count(&self) -> usize {
        match self {
            Expr::Int { .. } | Expr::Ident { .. } => 1,
            Expr::List { items, .. } => 1 + items.iter().map(Expr::node_count).sum::<usize>(),
        }
    }

    pub fn max_depth(&self) -> usize {
        match self {
            Expr::Int { .. } | Expr::Ident { .. } => 1,
            Expr::List { items, .. } => 1 + items.iter().map(Expr::max_depth).max().unwrap_or(0),
        }
    }

    pub fn as_ident(&self) -> Option<&str> {
        match self {
            Expr::Ident { name, .. } => Some(name.as_str()),
            _ => None,
        }
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Expr::Int { value: a, .. }, Expr::Int { value: b, .. }) => a == b,
            (Expr::Ident { name: a, .. }, Expr::Ident { name: b, .. }) => a == b,
            (Expr::List { items: a, .. }, Expr::List { items: b, .. }) => a == b,
            _ => false,
        }
    }
}

impl Eq for Expr {}

pub fn expr_from_json(v: &Value) -> Result<Expr, String> {
    match v {
        Value::Number(n) => {
            let i = n
                .as_i64()
                .ok_or_else(|| format!("number is not an i64: {n}"))?;
            let i32_ = i32::try_from(i).map_err(|_| format!("number out of i32 range: {i}"))?;
            Ok(Expr::Int {
                value: i32_,
                ptr: String::new(),
            })
        }
        Value::String(s) => Ok(Expr::Ident {
            name: s.to_string(),
            ptr: String::new(),
        }),
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(expr_from_json(item)?);
            }
            Ok(Expr::List {
                items: out,
                ptr: String::new(),
            })
        }
        _ => Err(format!("unsupported JSON value in expr: {v:?}")),
    }
}
