use crate::ast::Expr;
use crate::types::Ty;
use crate::x07ast::{AsyncProtocolAst, ContractClauseAst};

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: String,
    pub ty: Ty,
    pub brand: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub requires: Vec<ContractClauseAst>,
    pub ensures: Vec<ContractClauseAst>,
    pub invariant: Vec<ContractClauseAst>,
    pub params: Vec<FunctionParam>,
    pub ret_ty: Ty,
    pub ret_brand: Option<String>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub struct AsyncFunctionDef {
    pub name: String,
    pub requires: Vec<ContractClauseAst>,
    pub ensures: Vec<ContractClauseAst>,
    pub invariant: Vec<ContractClauseAst>,
    pub protocol: Option<AsyncProtocolAst>,
    pub params: Vec<FunctionParam>,
    pub ret_ty: Ty,
    pub ret_brand: Option<String>,
    pub body: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbi {
    C,
}

#[derive(Debug, Clone)]
pub struct ExternFunctionDecl {
    pub name: String,
    pub link_name: String,
    pub abi: ExternAbi,
    pub params: Vec<FunctionParam>,
    pub ret_ty: Ty,
    pub ret_is_void: bool,
}

/// A field of a `defrecord` lowered to its fixed byte layout (RFC 0002).
#[derive(Debug, Clone)]
pub struct RecordField {
    pub name: String,
    pub ty: Ty,
    /// Byte offset of this field within the packed record.
    pub offset: u32,
}

/// A lowered `defrecord`: a nominal product type represented as fixed-layout,
/// brand-tagged `bytes`. The brand id is the record's fully-qualified `name`.
#[derive(Debug, Clone)]
pub struct RecordDef {
    pub name: String,
    pub fields: Vec<RecordField>,
    /// Total packed size in bytes.
    pub size: u32,
}

/// A variant of a lowered `defenum` (RFC 0002).
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    /// Tag byte: the variant's 0-based index in declaration order.
    pub tag: u8,
    /// Payload type for a single-payload variant; `None` for a unit variant.
    pub payload: Option<Ty>,
}

/// A lowered `defenum`: a nominal tagged-union (sum) type represented as
/// brand-tagged `bytes` with layout `[u8 tag][payload?]`. The brand id is the
/// enum's fully-qualified `name`.
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub functions: Vec<FunctionDef>,
    pub async_functions: Vec<AsyncFunctionDef>,
    pub extern_functions: Vec<ExternFunctionDecl>,
    pub records: Vec<RecordDef>,
    pub enums: Vec<EnumDef>,
    pub solve: Expr,
}
