use crate::ast::Expr;
use crate::types::Ty;
use crate::x07ast::ContractClauseAst;

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

#[derive(Debug, Clone)]
pub struct Program {
    pub functions: Vec<FunctionDef>,
    pub async_functions: Vec<AsyncFunctionDef>,
    pub extern_functions: Vec<ExternFunctionDecl>,
    pub solve: Expr,
}
