use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum X07Ty {
    Unit,
    Bool,
    U8,
    I32,
    U32,
    Bytes,
    BytesView,
    VecU8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X07Param {
    pub name: String,
    pub ty: X07Ty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X07Func {
    /// Fully-qualified name (e.g. `std.text.ascii.normalize_lines`).
    pub name: String,
    pub exported: bool,
    pub params: Vec<X07Param>,
    pub ret: X07Ty,
    pub body: Vec<X07Stmt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X07Module {
    pub module_id: String,
    pub source_path: Option<String>,
    pub source_sha256: Option<String>,
    pub funcs: Vec<X07Func>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum X07Expr {
    Int(i32),
    Ident(String),
    Call {
        head: String,
        args: Vec<X07Expr>,
    },
    If {
        cond: Box<X07Expr>,
        then_e: Box<X07Expr>,
        else_e: Box<X07Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum X07Stmt {
    Let {
        name: String,
        init: X07Expr,
    },
    Set {
        name: String,
        value: X07Expr,
    },
    Expr(X07Expr),
    Return(X07Expr),
    If {
        cond: X07Expr,
        then_body: Vec<X07Stmt>,
        else_body: Vec<X07Stmt>,
    },
    ForRange {
        var: String,
        start: X07Expr,
        end: X07Expr,
        body: Vec<X07Stmt>,
    },
}
