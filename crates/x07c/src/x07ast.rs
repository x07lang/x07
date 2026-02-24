use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;

use serde_json::Value;
use x07_contracts::{X07AST_SCHEMA_VERSIONS_SUPPORTED, X07AST_SCHEMA_VERSION_V0_5_0};

use crate::ast::Expr;
use crate::types::Ty;
use crate::validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X07AstKind {
    Entry,
    Module,
}

#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    pub bound: Option<String>,
}

/// A structured type reference.
///
/// v0.3 only allowed concrete type strings.
/// v0.4 adds list expressions:
///   * ["t", "A"]        => type variable A
///   * ["option", <ty>]   => type application
///
/// This type is designed to be lossless over the JSON representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRef {
    /// Concrete named type (legacy string form).
    Named(String),
    /// Type variable reference ("t" form).
    Var(String),
    /// Type application.
    App { head: String, args: Vec<TypeRef> },
}

impl TypeRef {
    pub fn as_named(&self) -> Option<&str> {
        match self {
            TypeRef::Named(s) => Some(s.as_str()),
            TypeRef::Var(_) | TypeRef::App { .. } => None,
        }
    }

    /// Best-effort lowering to the current monomorphic `Ty` universe.
    ///
    /// This is intentionally conservative: it returns `None` when a type expression cannot
    /// be represented as a concrete `Ty` (e.g., variables, unknown constructors).
    pub fn as_mono_ty(&self) -> Option<Ty> {
        match self {
            TypeRef::Named(s) => Ty::parse_named(s),
            TypeRef::Var(_) => None,
            TypeRef::App { head, args } => {
                if args.len() != 1 {
                    return None;
                }
                let inner = args.first()?;
                match head.as_str() {
                    "option" => match inner.as_mono_ty()? {
                        Ty::I32 => Some(Ty::OptionI32),
                        Ty::Bytes => Some(Ty::OptionBytes),
                        Ty::BytesView => Some(Ty::OptionBytesView),
                        _ => None,
                    },
                    "result" => match inner.as_mono_ty()? {
                        Ty::I32 => Some(Ty::ResultI32),
                        Ty::Bytes => Some(Ty::ResultBytes),
                        Ty::BytesView => Some(Ty::ResultBytesView),
                        Ty::ResultBytes => Some(Ty::ResultResultBytes),
                        _ => None,
                    },
                    _ => None,
                }
            }
        }
    }
}

/// Convert a monomorphic internal type into its canonical x07AST string name.
///
/// NOTE: Some internal-only types are mapped onto their concrete carrier types
/// (e.g., `task_scope_v1` is represented as `i32` in x07AST).
pub fn ty_to_name(ty: Ty) -> &'static str {
    match ty {
        Ty::I32 => "i32",
        Ty::Bytes => "bytes",
        Ty::BytesView => "bytes_view",
        Ty::VecU8 => "vec_u8",
        Ty::OptionI32 => "option_i32",
        Ty::OptionTaskSelectEvtV1 => "option_i32",
        Ty::OptionBytes => "option_bytes",
        Ty::OptionBytesView => "option_bytes_view",
        Ty::ResultI32 => "result_i32",
        Ty::ResultBytes => "result_bytes",
        Ty::ResultBytesView => "result_bytes_view",
        Ty::ResultResultBytes => "result_result_bytes",
        Ty::Iface => "iface",
        Ty::PtrConstU8 => "ptr_const_u8",
        Ty::PtrMutU8 => "ptr_mut_u8",
        Ty::PtrConstVoid => "ptr_const_void",
        Ty::PtrMutVoid => "ptr_mut_void",
        Ty::PtrConstI32 => "ptr_const_i32",
        Ty::PtrMutI32 => "ptr_mut_i32",
        Ty::TaskScopeV1 => "i32",
        Ty::BudgetScopeV1 => "i32",
        Ty::TaskHandleBytesV1
        | Ty::TaskHandleResultBytesV1
        | Ty::TaskSlotV1
        | Ty::TaskSelectEvtV1 => "i32",
        Ty::Never => "never",
    }
}

#[derive(Debug, Clone)]
pub struct AstFunctionParam {
    pub name: String,
    pub ty: TypeRef,
    pub brand: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContractClauseAst {
    pub id: Option<String>,
    pub expr: Expr,
    pub witness: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct AstFunctionDef {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub requires: Vec<ContractClauseAst>,
    pub ensures: Vec<ContractClauseAst>,
    pub invariant: Vec<ContractClauseAst>,
    pub params: Vec<AstFunctionParam>,
    pub result: TypeRef,
    pub result_brand: Option<String>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub struct AstAsyncFunctionDef {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub requires: Vec<ContractClauseAst>,
    pub ensures: Vec<ContractClauseAst>,
    pub invariant: Vec<ContractClauseAst>,
    pub params: Vec<AstFunctionParam>,
    pub result: TypeRef,
    pub result_brand: Option<String>,
    pub body: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternAbi {
    C,
}

#[derive(Debug, Clone)]
pub struct AstExternFunctionDecl {
    pub name: String,
    pub link_name: String,
    pub abi: ExternAbi,
    pub params: Vec<AstFunctionParam>,
    /// `None` means extern C returns `void`.
    pub result: Option<TypeRef>,
}

#[derive(Debug, Clone)]
pub struct X07AstFile {
    pub schema_version: String,
    pub kind: X07AstKind,
    pub module_id: String,
    pub imports: BTreeSet<String>,
    pub exports: BTreeSet<String>,
    pub functions: Vec<AstFunctionDef>,
    pub async_functions: Vec<AstAsyncFunctionDef>,
    pub extern_functions: Vec<AstExternFunctionDecl>,
    pub solve: Option<Expr>,
    pub meta: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct X07AstError {
    pub message: String,
    pub ptr: String,
}

impl std::error::Error for X07AstError {}

impl Display for X07AstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}", self.message, self.ptr)
    }
}

pub fn parse_x07ast_json(bytes: &[u8]) -> Result<X07AstFile, X07AstError> {
    let doc: Value = serde_json::from_slice(bytes).map_err(|e| X07AstError {
        message: e.to_string(),
        ptr: "".to_string(),
    })?;
    parse_x07ast_value(&doc)
}

fn parse_x07ast_value(root: &Value) -> Result<X07AstFile, X07AstError> {
    let root_obj = root.as_object().ok_or_else(|| X07AstError {
        message: "x07ast root must be an object".to_string(),
        ptr: "".to_string(),
    })?;

    let schema_version = get_required_string(root_obj, "/schema_version", "schema_version")?;
    if !X07AST_SCHEMA_VERSIONS_SUPPORTED
        .iter()
        .any(|&v| v == schema_version)
    {
        return Err(X07AstError {
            message: format!(
                "unsupported schema_version: got {schema_version:?} (supported: {}) (hint: if this comes from a dependency package, upgrade to a newer package version)",
                X07AST_SCHEMA_VERSIONS_SUPPORTED.join(", ")
            ),
            ptr: "/schema_version".to_string(),
        });
    }
    let allow_contracts = schema_version == X07AST_SCHEMA_VERSION_V0_5_0;

    let kind = get_required_string(root_obj, "/kind", "kind")?;
    let kind = match kind.trim() {
        "entry" => X07AstKind::Entry,
        "module" => X07AstKind::Module,
        _ => {
            return Err(X07AstError {
                message: format!("invalid kind: expected \"entry\" or \"module\" got {kind:?}"),
                ptr: "/kind".to_string(),
            })
        }
    };

    let module_id = get_required_string(root_obj, "/module_id", "module_id")?;
    validate::validate_module_id(&module_id).map_err(|message| X07AstError {
        message,
        ptr: "/module_id".to_string(),
    })?;

    let imports: BTreeSet<String> = parse_string_array(root_obj, "/imports", "imports")?
        .into_iter()
        .map(|s| {
            validate::validate_module_id(&s).map_err(|message| X07AstError {
                message,
                ptr: "/imports".to_string(),
            })?;
            Ok(s)
        })
        .collect::<Result<BTreeSet<_>, X07AstError>>()?;

    let decls_v = root_obj.get("decls").ok_or_else(|| X07AstError {
        message: "missing required field: decls".to_string(),
        ptr: "".to_string(),
    })?;
    let decls_a = decls_v.as_array().ok_or_else(|| X07AstError {
        message: "decls must be an array".to_string(),
        ptr: "/decls".to_string(),
    })?;

    let mut exports: BTreeSet<String> = BTreeSet::new();
    let mut functions: Vec<AstFunctionDef> = Vec::new();
    let mut async_functions: Vec<AstAsyncFunctionDef> = Vec::new();
    let mut extern_functions: Vec<AstExternFunctionDecl> = Vec::new();

    let mut function_names: BTreeSet<String> = BTreeSet::new();

    for (didx, d) in decls_a.iter().enumerate() {
        let dptr = format!("/decls/{didx}");
        let dobj = d.as_object().ok_or_else(|| X07AstError {
            message: format!("decls[{didx}] must be an object"),
            ptr: dptr.clone(),
        })?;
        let kind = get_required_string(dobj, &format!("{dptr}/kind"), "kind")?;
        match kind.trim() {
            "export" => {
                if !exports.is_empty() {
                    return Err(X07AstError {
                        message: "duplicate export decl".to_string(),
                        ptr: format!("{dptr}/kind"),
                    });
                }
                if kind.trim() != "export" {
                    return Err(X07AstError {
                        message: format!(
                            "invalid export decl kind: expected \"export\" got {kind:?}"
                        ),
                        ptr: format!("{dptr}/kind"),
                    });
                }

                let names = parse_string_array(dobj, &format!("{dptr}/names"), "names")?;
                for (nidx, sym) in names.iter().enumerate() {
                    validate::validate_symbol(sym).map_err(|message| X07AstError {
                        message,
                        ptr: format!("{dptr}/names/{nidx}"),
                    })?;
                    exports.insert(sym.to_string());
                }
            }
            "defn" => {
                let parsed = parse_def_like(dobj, &dptr, &module_id, false, allow_contracts)?;
                if !function_names.insert(parsed.name.clone()) {
                    return Err(X07AstError {
                        message: format!("duplicate function name: {:?}", parsed.name),
                        ptr: format!("{dptr}/name"),
                    });
                }
                functions.push(AstFunctionDef {
                    name: parsed.name,
                    type_params: parsed.type_params,
                    requires: parsed.requires,
                    ensures: parsed.ensures,
                    invariant: parsed.invariant,
                    params: parsed.params,
                    result: parsed.result,
                    result_brand: parsed.result_brand,
                    body: parsed.body,
                });
            }
            "defasync" => {
                let parsed = parse_def_like(dobj, &dptr, &module_id, true, allow_contracts)?;
                if !function_names.insert(parsed.name.clone()) {
                    return Err(X07AstError {
                        message: format!("duplicate function name: {:?}", parsed.name),
                        ptr: format!("{dptr}/name"),
                    });
                }
                async_functions.push(AstAsyncFunctionDef {
                    name: parsed.name,
                    type_params: parsed.type_params,
                    requires: parsed.requires,
                    ensures: parsed.ensures,
                    invariant: parsed.invariant,
                    params: parsed.params,
                    result: parsed.result,
                    result_brand: parsed.result_brand,
                    body: parsed.body,
                });
            }
            "extern" => {
                let ex = parse_extern(dobj, &dptr, &module_id)?;
                if !function_names.insert(ex.name.clone()) {
                    return Err(X07AstError {
                        message: format!("duplicate function name: {:?}", ex.name),
                        ptr: format!("{dptr}/name"),
                    });
                }
                extern_functions.push(ex);
            }
            _ => {
                return Err(X07AstError {
                    message: format!("unsupported decl kind: {kind:?}"),
                    ptr: format!("{dptr}/kind"),
                })
            }
        }
    }

    let solve = if let Some(solve_v) = root_obj.get("solve") {
        Some(expr_from_json_sexpr(solve_v, "/solve")?)
    } else {
        None
    };

    let mut meta: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(meta_v) = root_obj.get("meta") {
        if let Some(obj) = meta_v.as_object() {
            for (k, v) in obj {
                meta.insert(k.clone(), v.clone());
            }
        }
    }

    Ok(X07AstFile {
        schema_version,
        kind,
        module_id,
        imports,
        exports,
        functions,
        async_functions,
        extern_functions,
        solve,
        meta,
    })
}

#[derive(Debug, Clone)]
struct ParsedDefLike {
    name: String,
    type_params: Vec<TypeParam>,
    requires: Vec<ContractClauseAst>,
    ensures: Vec<ContractClauseAst>,
    invariant: Vec<ContractClauseAst>,
    params: Vec<AstFunctionParam>,
    result: TypeRef,
    result_brand: Option<String>,
    body: Expr,
}

fn parse_def_like(
    dobj: &serde_json::Map<String, Value>,
    ptr: &str,
    module_id: &str,
    is_async: bool,
    allow_contracts: bool,
) -> Result<ParsedDefLike, X07AstError> {
    let kind_label = if is_async { "defasync" } else { "defn" };

    let name = get_required_string(dobj, &format!("{ptr}/name"), "name")?;
    validate::validate_symbol(&name).map_err(|message| X07AstError {
        message,
        ptr: format!("{ptr}/name"),
    })?;
    let prefix = format!("{module_id}.");
    if !name.starts_with(&prefix) {
        return Err(X07AstError {
            message: format!("{kind_label} name must start with {prefix:?}, got {name:?}"),
            ptr: format!("{ptr}/name"),
        });
    }

    let type_params = parse_type_params(dobj, ptr)?;

    if !allow_contracts {
        for field in ["requires", "ensures", "invariant"] {
            if dobj.contains_key(field) {
                return Err(X07AstError {
                    message: format!("{field} is only supported in {X07AST_SCHEMA_VERSION_V0_5_0}"),
                    ptr: format!("{ptr}/{field}"),
                });
            }
        }
    }

    let requires = parse_contract_clauses(dobj, ptr, "requires")?;
    let ensures = parse_contract_clauses(dobj, ptr, "ensures")?;
    let invariant = parse_contract_clauses(dobj, ptr, "invariant")?;

    let params = parse_params(dobj, ptr, kind_label)?;

    let result_v = dobj.get("result").ok_or_else(|| X07AstError {
        message: format!("missing required field: result in {kind_label}"),
        ptr: ptr.to_string(),
    })?;
    let result = parse_type_ref(result_v, &format!("{ptr}/result"))?;

    let result_brand = if let Some(v) = dobj.get("result_brand") {
        let s = v.as_str().ok_or_else(|| X07AstError {
            message: "result_brand must be a string".to_string(),
            ptr: format!("{ptr}/result_brand"),
        })?;
        validate::validate_symbol(s).map_err(|message| X07AstError {
            message,
            ptr: format!("{ptr}/result_brand"),
        })?;
        Some(s.to_string())
    } else {
        None
    };

    if result_brand.is_some() && !type_ref_is_bytesish(&result) {
        return Err(X07AstError {
            message: format!(
                "E_BRAND_BAD_TY: result_brand is not allowed on result type {result:?}"
            ),
            ptr: format!("{ptr}/result"),
        });
    }

    let body_v = dobj.get("body").ok_or_else(|| X07AstError {
        message: format!("missing required field: body in {kind_label}"),
        ptr: ptr.to_string(),
    })?;
    let body = expr_from_json_sexpr(body_v, &format!("{ptr}/body"))?;

    Ok(ParsedDefLike {
        name,
        type_params,
        requires,
        ensures,
        invariant,
        params,
        result,
        result_brand,
        body,
    })
}

fn parse_contract_clauses(
    dobj: &serde_json::Map<String, Value>,
    ptr: &str,
    field: &str,
) -> Result<Vec<ContractClauseAst>, X07AstError> {
    let Some(v) = dobj.get(field) else {
        return Ok(Vec::new());
    };

    let arr = v.as_array().ok_or_else(|| X07AstError {
        message: format!("{field} must be an array"),
        ptr: format!("{ptr}/{field}"),
    })?;

    let mut out: Vec<ContractClauseAst> = Vec::with_capacity(arr.len());
    for (idx, item) in arr.iter().enumerate() {
        let cptr = format!("{ptr}/{field}/{idx}");
        let obj = item.as_object().ok_or_else(|| X07AstError {
            message: format!("{field} clause must be an object"),
            ptr: cptr.clone(),
        })?;

        let id = if let Some(v) = obj.get("id") {
            let s = v.as_str().ok_or_else(|| X07AstError {
                message: "id must be a string".to_string(),
                ptr: format!("{cptr}/id"),
            })?;
            Some(s.to_string())
        } else {
            None
        };

        let expr_v = obj.get("expr").ok_or_else(|| X07AstError {
            message: "missing required field: expr".to_string(),
            ptr: cptr.clone(),
        })?;
        let expr = expr_from_json_sexpr(expr_v, &format!("{cptr}/expr"))?;

        let witness = if let Some(w) = obj.get("witness") {
            let w_arr = w.as_array().ok_or_else(|| X07AstError {
                message: "witness must be an array".to_string(),
                ptr: format!("{cptr}/witness"),
            })?;
            let mut w_out: Vec<Expr> = Vec::with_capacity(w_arr.len());
            for (widx, witem) in w_arr.iter().enumerate() {
                let wptr = format!("{cptr}/witness/{widx}");
                w_out.push(expr_from_json_sexpr(witem, &wptr)?);
            }
            w_out
        } else {
            Vec::new()
        };

        out.push(ContractClauseAst { id, expr, witness });
    }

    Ok(out)
}

fn parse_type_params(
    dobj: &serde_json::Map<String, Value>,
    ptr: &str,
) -> Result<Vec<TypeParam>, X07AstError> {
    let Some(v) = dobj.get("type_params") else {
        return Ok(Vec::new());
    };

    let arr = v.as_array().ok_or_else(|| X07AstError {
        message: "type_params must be an array".to_string(),
        ptr: format!("{ptr}/type_params"),
    })?;

    let mut out: Vec<TypeParam> = Vec::with_capacity(arr.len());
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for (idx, item) in arr.iter().enumerate() {
        let iptr = format!("{ptr}/type_params/{idx}");
        let obj = item.as_object().ok_or_else(|| X07AstError {
            message: "type_params item must be an object".to_string(),
            ptr: iptr.clone(),
        })?;

        let name = get_required_string(obj, &format!("{iptr}/name"), "name")?;
        validate::validate_local_name(&name).map_err(|message| X07AstError {
            message,
            ptr: format!("{iptr}/name"),
        })?;

        if !seen.insert(name.clone()) {
            return Err(X07AstError {
                message: format!("duplicate type param name: {name:?}"),
                ptr: format!("{iptr}/name"),
            });
        }

        let bound = if let Some(bv) = obj.get("bound") {
            let s = bv.as_str().ok_or_else(|| X07AstError {
                message: "bound must be a string".to_string(),
                ptr: format!("{iptr}/bound"),
            })?;
            validate::validate_type_name(s).map_err(|message| X07AstError {
                message,
                ptr: format!("{iptr}/bound"),
            })?;
            Some(s.to_string())
        } else {
            None
        };

        out.push(TypeParam { name, bound });
    }

    Ok(out)
}

fn parse_extern(
    dobj: &serde_json::Map<String, Value>,
    ptr: &str,
    module_id: &str,
) -> Result<AstExternFunctionDecl, X07AstError> {
    let name = get_required_string(dobj, &format!("{ptr}/name"), "name")?;
    validate::validate_symbol(&name).map_err(|message| X07AstError {
        message,
        ptr: format!("{ptr}/name"),
    })?;
    let prefix = format!("{module_id}.");
    if !name.starts_with(&prefix) {
        return Err(X07AstError {
            message: format!("extern name must start with {prefix:?}, got {name:?}"),
            ptr: format!("{ptr}/name"),
        });
    }

    let abi = get_required_string(dobj, &format!("{ptr}/abi"), "abi")?;
    if abi.trim() != "C" {
        return Err(X07AstError {
            message: format!("unsupported extern abi: expected \"C\" got {abi:?}"),
            ptr: format!("{ptr}/abi"),
        });
    }

    let link_name = if let Some(v) = dobj.get("link_name") {
        let s = v.as_str().ok_or_else(|| X07AstError {
            message: "link_name must be a string".to_string(),
            ptr: format!("{ptr}/link_name"),
        })?;
        validate::validate_local_name(s).map_err(|message| X07AstError {
            message,
            ptr: format!("{ptr}/link_name"),
        })?;
        s.to_string()
    } else {
        let sym = name
            .rsplit_once('.')
            .map(|(_, s)| s)
            .unwrap_or(name.as_str());
        validate::validate_local_name(sym).map_err(|message| X07AstError {
            message,
            ptr: format!("{ptr}/name"),
        })?;
        sym.to_string()
    };

    let params = parse_params(dobj, ptr, "extern")?;
    for (idx, p) in params.iter().enumerate() {
        let Some(ty) = p.ty.as_mono_ty() else {
            return Err(X07AstError {
                message:
                    "extern param has unsupported type (only i32 and raw pointers are supported)"
                        .to_string(),
                ptr: format!("{ptr}/params/{idx}/ty"),
            });
        };
        if !ty.is_ffi_ty() {
            return Err(X07AstError {
                message:
                    "extern param has unsupported type (only i32 and raw pointers are supported)"
                        .to_string(),
                ptr: format!("{ptr}/params/{idx}/ty"),
            });
        }
    }

    let ret_v = dobj.get("result").ok_or_else(|| X07AstError {
        message: "missing required field: result".to_string(),
        ptr: ptr.to_string(),
    })?;

    // extern "result" is special-cased for "void".
    let result = match ret_v {
        Value::String(s) if s.trim() == "void" => None,
        _ => {
            let tr = parse_type_ref(ret_v, &format!("{ptr}/result"))?;
            let Some(ty) = tr.as_mono_ty() else {
                return Err(X07AstError {
                    message: "extern result has unsupported type (only i32 and raw pointers are supported)".to_string(),
                    ptr: format!("{ptr}/result"),
                });
            };
            if !ty.is_ffi_ty() {
                return Err(X07AstError {
                    message: "extern result has unsupported type (only i32 and raw pointers are supported)".to_string(),
                    ptr: format!("{ptr}/result"),
                });
            }
            Some(tr)
        }
    };

    Ok(AstExternFunctionDecl {
        name,
        link_name,
        abi: ExternAbi::C,
        params,
        result,
    })
}

fn parse_params(
    dobj: &serde_json::Map<String, Value>,
    ptr: &str,
    kind_label: &str,
) -> Result<Vec<AstFunctionParam>, X07AstError> {
    let params_v = dobj.get("params").ok_or_else(|| X07AstError {
        message: format!("missing required field: params in {kind_label}"),
        ptr: ptr.to_string(),
    })?;
    let params_a = params_v.as_array().ok_or_else(|| X07AstError {
        message: format!("{kind_label}.params must be an array"),
        ptr: format!("{ptr}/params"),
    })?;
    let mut params: Vec<AstFunctionParam> = Vec::with_capacity(params_a.len());
    for (pidx, p) in params_a.iter().enumerate() {
        let pptr = format!("{ptr}/params/{pidx}");
        let pobj = p.as_object().ok_or_else(|| X07AstError {
            message: format!("{kind_label}.params[{pidx}] must be an object"),
            ptr: pptr.clone(),
        })?;
        let arg_name = get_required_string(pobj, &format!("{pptr}/name"), "name")?;
        validate::validate_local_name(&arg_name).map_err(|message| X07AstError {
            message,
            ptr: format!("{pptr}/name"),
        })?;
        if arg_name == "input" {
            return Err(X07AstError {
                message: format!(
                    "{kind_label} arg name must not be 'input' (reserved); \
                     rename to 'data', 'payload', 'in_bytes', or 'buf'"
                ),
                ptr: format!("{pptr}/name"),
            });
        }

        let ty_v = pobj.get("ty").ok_or_else(|| X07AstError {
            message: "missing required field: ty".to_string(),
            ptr: pptr.clone(),
        })?;
        let ty = parse_type_ref(ty_v, &format!("{pptr}/ty"))?;

        let brand = if let Some(v) = pobj.get("brand") {
            let s = v.as_str().ok_or_else(|| X07AstError {
                message: "brand must be a string".to_string(),
                ptr: format!("{pptr}/brand"),
            })?;
            validate::validate_symbol(s).map_err(|message| X07AstError {
                message,
                ptr: format!("{pptr}/brand"),
            })?;
            Some(s.to_string())
        } else {
            None
        };

        if brand.is_some() && !type_ref_is_bytesish(&ty) {
            return Err(X07AstError {
                message: format!("E_BRAND_BAD_TY: brand is not allowed on param type {ty:?}"),
                ptr: format!("{pptr}/ty"),
            });
        }

        params.push(AstFunctionParam {
            name: arg_name,
            ty,
            brand,
        });
    }
    Ok(params)
}

fn type_ref_is_bytesish(tr: &TypeRef) -> bool {
    matches!(
        tr.as_mono_ty(),
        Some(
            Ty::Bytes
                | Ty::BytesView
                | Ty::OptionBytes
                | Ty::OptionBytesView
                | Ty::ResultBytes
                | Ty::ResultBytesView
                | Ty::ResultResultBytes
        )
    )
}

fn parse_type_ref(v: &Value, ptr: &str) -> Result<TypeRef, X07AstError> {
    match v {
        Value::String(s) => {
            validate::validate_type_name(s).map_err(|message| X07AstError {
                message,
                ptr: ptr.to_string(),
            })?;
            // Keep the v0.3 behavior: named types must be in the current Ty universe.
            if Ty::parse_named(s).is_none() {
                return Err(X07AstError {
                    message: format!(
                        "unknown type: {s:?} (expected i32, u32, bytes, bytes_view, vec_u8, option_i32, option_bytes, option_bytes_view, result_i32, result_bytes, result_bytes_view, result_result_bytes, iface, ptr_const_u8, ptr_mut_u8, ptr_const_void, ptr_mut_void, ptr_const_i32, or ptr_mut_i32)"
                    ),
                    ptr: ptr.to_string(),
                });
            }
            Ok(TypeRef::Named(s.to_string()))
        }
        Value::Array(items) => {
            if items.is_empty() {
                return Err(X07AstError {
                    message: "type expression list must not be empty".to_string(),
                    ptr: ptr.to_string(),
                });
            }
            let head = items[0].as_str().ok_or_else(|| X07AstError {
                message: "type expression head must be a string".to_string(),
                ptr: format!("{ptr}/0"),
            })?;
            validate::validate_type_name(head).map_err(|message| X07AstError {
                message,
                ptr: format!("{ptr}/0"),
            })?;

            if head == "t" {
                if items.len() != 2 {
                    return Err(X07AstError {
                        message: "type var expression must be [\"t\", <name>]".to_string(),
                        ptr: ptr.to_string(),
                    });
                }
                let name = items[1].as_str().ok_or_else(|| X07AstError {
                    message: "type var name must be a string".to_string(),
                    ptr: format!("{ptr}/1"),
                })?;
                validate::validate_local_name(name).map_err(|message| X07AstError {
                    message,
                    ptr: format!("{ptr}/1"),
                })?;
                return Ok(TypeRef::Var(name.to_string()));
            }

            if items.len() < 2 {
                return Err(X07AstError {
                    message: "type application must have at least 1 argument".to_string(),
                    ptr: ptr.to_string(),
                });
            }

            let mut args: Vec<TypeRef> = Vec::with_capacity(items.len().saturating_sub(1));
            for (idx, item) in items.iter().enumerate().skip(1) {
                args.push(parse_type_ref(item, &format!("{ptr}/{idx}"))?);
            }
            Ok(TypeRef::App {
                head: head.to_string(),
                args,
            })
        }
        _ => Err(X07AstError {
            message: "type must be a string or a list".to_string(),
            ptr: ptr.to_string(),
        }),
    }
}

fn get_required_string(
    obj: &serde_json::Map<String, Value>,
    ptr: &str,
    key: &str,
) -> Result<String, X07AstError> {
    let v = obj.get(key).ok_or_else(|| X07AstError {
        message: format!("missing required field: {key}"),
        ptr: ptr
            .rsplit_once('/')
            .map(|(p, _)| p)
            .unwrap_or("")
            .to_string(),
    })?;
    v.as_str().map(str::to_string).ok_or_else(|| X07AstError {
        message: format!("{key} must be a string"),
        ptr: ptr.to_string(),
    })
}

fn parse_string_array(
    obj: &serde_json::Map<String, Value>,
    ptr: &str,
    key: &str,
) -> Result<Vec<String>, X07AstError> {
    let v = obj.get(key).ok_or_else(|| X07AstError {
        message: format!("missing required field: {key}"),
        ptr: ptr
            .rsplit_once('/')
            .map(|(p, _)| p)
            .unwrap_or("")
            .to_string(),
    })?;
    let arr = v.as_array().ok_or_else(|| X07AstError {
        message: format!("{key} must be an array"),
        ptr: ptr.to_string(),
    })?;
    let mut out = Vec::with_capacity(arr.len());
    for (idx, item) in arr.iter().enumerate() {
        let s = item.as_str().ok_or_else(|| X07AstError {
            message: format!("{key}[{idx}] must be a string"),
            ptr: format!("{ptr}/{idx}"),
        })?;
        out.push(s.to_string());
    }
    Ok(out)
}

fn expr_from_json_sexpr(v: &Value, ptr: &str) -> Result<Expr, X07AstError> {
    match v {
        Value::Number(n) => {
            let i = n.as_i64().ok_or_else(|| X07AstError {
                message: format!("number is not an i64: {n}"),
                ptr: ptr.to_string(),
            })?;
            let i32_ = i32::try_from(i).map_err(|_| X07AstError {
                message: format!("number out of i32 range: {i}"),
                ptr: ptr.to_string(),
            })?;
            Ok(Expr::Int {
                value: i32_,
                ptr: ptr.to_string(),
            })
        }
        Value::String(s) => {
            if s.chars().any(|c| c.is_whitespace()) {
                return Err(X07AstError {
                    message: "atom must not contain whitespace".to_string(),
                    ptr: ptr.to_string(),
                });
            }
            Ok(Expr::Ident {
                name: s.to_string(),
                ptr: ptr.to_string(),
            })
        }
        Value::Array(items) => {
            if items.is_empty() {
                return Err(X07AstError {
                    message: "list expression must not be empty".to_string(),
                    ptr: ptr.to_string(),
                });
            }
            let head = items[0].as_str().ok_or_else(|| X07AstError {
                message: "list head must be an atom string".to_string(),
                ptr: format!("{ptr}/0"),
            })?;
            if head.chars().any(|c| c.is_whitespace()) {
                return Err(X07AstError {
                    message: "list head must not contain whitespace".to_string(),
                    ptr: format!("{ptr}/0"),
                });
            }
            let mut out = Vec::with_capacity(items.len());
            out.push(Expr::Ident {
                name: head.to_string(),
                ptr: format!("{ptr}/0"),
            });
            for (idx, item) in items.iter().enumerate().skip(1) {
                let item_ptr = format!("{ptr}/{idx}");
                if (head == "bytes.lit" || head == "bytes.view_lit") && idx == 1 {
                    if let Value::String(s) = item {
                        out.push(Expr::Ident {
                            name: s.to_string(),
                            ptr: item_ptr,
                        });
                        continue;
                    }
                }
                out.push(expr_from_json_sexpr(item, &item_ptr)?);
            }
            Ok(Expr::List {
                items: out,
                ptr: ptr.to_string(),
            })
        }
        _ => Err(X07AstError {
            message: format!("unsupported JSON value in expr: {v:?}"),
            ptr: ptr.to_string(),
        }),
    }
}

pub fn x07ast_file_to_value(file: &X07AstFile) -> Value {
    let mut m = serde_json::Map::new();
    m.insert(
        "schema_version".to_string(),
        Value::String(file.schema_version.clone()),
    );
    m.insert(
        "kind".to_string(),
        Value::String(match file.kind {
            X07AstKind::Entry => "entry".to_string(),
            X07AstKind::Module => "module".to_string(),
        }),
    );
    m.insert(
        "module_id".to_string(),
        Value::String(file.module_id.clone()),
    );
    m.insert(
        "imports".to_string(),
        Value::Array(file.imports.iter().cloned().map(Value::String).collect()),
    );
    m.insert(
        "decls".to_string(),
        Value::Array(x07ast_decls_to_values(file)),
    );

    if let Some(solve) = &file.solve {
        m.insert("solve".to_string(), expr_to_value(solve));
    }

    if !file.meta.is_empty() {
        let mut meta = serde_json::Map::new();
        for (k, v) in &file.meta {
            meta.insert(k.clone(), v.clone());
        }
        m.insert("meta".to_string(), Value::Object(meta));
    }

    Value::Object(m)
}

pub fn canonicalize_x07ast_file(file: &mut X07AstFile) {
    file.functions.sort_by(|a, b| a.name.cmp(&b.name));
    file.async_functions.sort_by(|a, b| a.name.cmp(&b.name));
    file.extern_functions.sort_by(|a, b| a.name.cmp(&b.name));
    reptr_x07ast_file(file);
}

fn reptr_x07ast_file(file: &mut X07AstFile) {
    if let Some(solve) = &mut file.solve {
        reptr_expr(solve, "/solve");
    }

    let export_slots = if file.kind == X07AstKind::Module && !file.exports.is_empty() {
        1usize
    } else {
        0usize
    };
    let extern_slots = file.extern_functions.len();
    let defn_base = export_slots + extern_slots;

    for (idx, f) in file.functions.iter_mut().enumerate() {
        let decl_idx = defn_base + idx;
        reptr_contract_clauses(&mut f.requires, &format!("/decls/{decl_idx}/requires"));
        reptr_contract_clauses(&mut f.ensures, &format!("/decls/{decl_idx}/ensures"));
        reptr_contract_clauses(&mut f.invariant, &format!("/decls/{decl_idx}/invariant"));
        reptr_expr(&mut f.body, &format!("/decls/{decl_idx}/body"));
    }
    let sync_fns_count = file.functions.len();
    for (idx, f) in file.async_functions.iter_mut().enumerate() {
        let decl_idx = defn_base + sync_fns_count + idx;
        reptr_contract_clauses(&mut f.requires, &format!("/decls/{decl_idx}/requires"));
        reptr_contract_clauses(&mut f.ensures, &format!("/decls/{decl_idx}/ensures"));
        reptr_contract_clauses(&mut f.invariant, &format!("/decls/{decl_idx}/invariant"));
        reptr_expr(&mut f.body, &format!("/decls/{decl_idx}/body"));
    }
}

fn reptr_contract_clauses(clauses: &mut [ContractClauseAst], base_ptr: &str) {
    for (cidx, clause) in clauses.iter_mut().enumerate() {
        reptr_expr(&mut clause.expr, &format!("{base_ptr}/{cidx}/expr"));
        for (widx, w) in clause.witness.iter_mut().enumerate() {
            reptr_expr(w, &format!("{base_ptr}/{cidx}/witness/{widx}"));
        }
    }
}

fn reptr_expr(expr: &mut Expr, ptr: &str) {
    match expr {
        Expr::Int { ptr: p, .. } | Expr::Ident { ptr: p, .. } => {
            *p = ptr.to_string();
        }
        Expr::List { items, ptr: p } => {
            *p = ptr.to_string();
            for (idx, item) in items.iter_mut().enumerate() {
                let item_ptr = format!("{ptr}/{idx}");
                reptr_expr(item, &item_ptr);
            }
        }
    }
}

fn x07ast_decls_to_values(file: &X07AstFile) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    if file.kind == X07AstKind::Module && !file.exports.is_empty() {
        out.push(Value::Object(export_decl_value(&file.exports)));
    }

    for f in &file.extern_functions {
        out.push(Value::Object(extern_decl_value(f)));
    }
    for f in &file.functions {
        out.push(Value::Object(def_decl_value(
            "defn",
            &f.name,
            &f.type_params,
            &f.requires,
            &f.ensures,
            &f.invariant,
            &f.params,
            &f.result,
            f.result_brand.as_deref(),
            &f.body,
        )));
    }
    for f in &file.async_functions {
        out.push(Value::Object(def_decl_value(
            "defasync",
            &f.name,
            &f.type_params,
            &f.requires,
            &f.ensures,
            &f.invariant,
            &f.params,
            &f.result,
            f.result_brand.as_deref(),
            &f.body,
        )));
    }
    out
}

fn export_decl_value(exports: &BTreeSet<String>) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("kind".to_string(), Value::String("export".to_string()));
    m.insert(
        "names".to_string(),
        Value::Array(exports.iter().cloned().map(Value::String).collect()),
    );
    m
}

fn type_param_to_value(tp: &TypeParam) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("name".to_string(), Value::String(tp.name.clone()));
    if let Some(bound) = &tp.bound {
        m.insert("bound".to_string(), Value::String(bound.clone()));
    }
    Value::Object(m)
}

pub fn type_ref_to_value(tr: &TypeRef) -> Value {
    match tr {
        TypeRef::Named(s) => Value::String(s.clone()),
        TypeRef::Var(name) => Value::Array(vec![
            Value::String("t".to_string()),
            Value::String(name.clone()),
        ]),
        TypeRef::App { head, args } => {
            let mut items: Vec<Value> = Vec::with_capacity(args.len() + 1);
            items.push(Value::String(head.clone()));
            for a in args {
                items.push(type_ref_to_value(a));
            }
            Value::Array(items)
        }
    }
}

pub fn type_ref_from_expr(e: &Expr) -> Result<TypeRef, String> {
    match e {
        Expr::Ident { name, .. } => Ok(TypeRef::Named(name.clone())),
        Expr::Int { .. } => Err("type expression must be a string or a list".to_string()),
        Expr::List { items, .. } => {
            if items.is_empty() {
                return Err("type expression list must not be empty".to_string());
            }
            let head = items[0]
                .as_ident()
                .ok_or_else(|| "type expression head must be a string".to_string())?;
            if head == "t" {
                if items.len() != 2 {
                    return Err("type var expression must be [\"t\", <name>]".to_string());
                }
                let name = items[1]
                    .as_ident()
                    .ok_or_else(|| "type var name must be a string".to_string())?;
                return Ok(TypeRef::Var(name.to_string()));
            }
            if items.len() < 2 {
                return Err("type application must have at least 1 argument".to_string());
            }
            let mut args: Vec<TypeRef> = Vec::with_capacity(items.len().saturating_sub(1));
            for item in items.iter().skip(1) {
                args.push(type_ref_from_expr(item)?);
            }
            Ok(TypeRef::App {
                head: head.to_string(),
                args,
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn def_decl_value(
    kind: &str,
    name: &str,
    type_params: &[TypeParam],
    requires: &[ContractClauseAst],
    ensures: &[ContractClauseAst],
    invariant: &[ContractClauseAst],
    params: &[AstFunctionParam],
    result: &TypeRef,
    result_brand: Option<&str>,
    body: &Expr,
) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("kind".to_string(), Value::String(kind.to_string()));
    m.insert("name".to_string(), Value::String(name.to_string()));

    if !type_params.is_empty() {
        m.insert(
            "type_params".to_string(),
            Value::Array(type_params.iter().map(type_param_to_value).collect()),
        );
    }

    if !requires.is_empty() {
        m.insert(
            "requires".to_string(),
            Value::Array(requires.iter().map(contract_clause_to_value).collect()),
        );
    }
    if !ensures.is_empty() {
        m.insert(
            "ensures".to_string(),
            Value::Array(ensures.iter().map(contract_clause_to_value).collect()),
        );
    }
    if !invariant.is_empty() {
        m.insert(
            "invariant".to_string(),
            Value::Array(invariant.iter().map(contract_clause_to_value).collect()),
        );
    }

    m.insert(
        "params".to_string(),
        Value::Array(
            params
                .iter()
                .map(|p| {
                    let mut pm = serde_json::Map::new();
                    pm.insert("name".to_string(), Value::String(p.name.clone()));
                    pm.insert("ty".to_string(), type_ref_to_value(&p.ty));
                    if let Some(brand) = &p.brand {
                        pm.insert("brand".to_string(), Value::String(brand.clone()));
                    }
                    Value::Object(pm)
                })
                .collect(),
        ),
    );
    m.insert("result".to_string(), type_ref_to_value(result));
    if let Some(brand) = result_brand {
        m.insert("result_brand".to_string(), Value::String(brand.to_string()));
    }
    m.insert("body".to_string(), expr_to_value(body));
    m
}

fn contract_clause_to_value(c: &ContractClauseAst) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("expr".to_string(), expr_to_value(&c.expr));
    if let Some(id) = &c.id {
        m.insert("id".to_string(), Value::String(id.clone()));
    }
    if !c.witness.is_empty() {
        m.insert(
            "witness".to_string(),
            Value::Array(c.witness.iter().map(expr_to_value).collect()),
        );
    }
    Value::Object(m)
}

fn extern_decl_value(f: &AstExternFunctionDecl) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("kind".to_string(), Value::String("extern".to_string()));
    m.insert("abi".to_string(), Value::String("C".to_string()));
    m.insert("name".to_string(), Value::String(f.name.clone()));
    m.insert("link_name".to_string(), Value::String(f.link_name.clone()));
    m.insert(
        "params".to_string(),
        Value::Array(
            f.params
                .iter()
                .map(|p| {
                    let mut pm = serde_json::Map::new();
                    pm.insert("name".to_string(), Value::String(p.name.clone()));
                    pm.insert("ty".to_string(), type_ref_to_value(&p.ty));
                    if let Some(brand) = &p.brand {
                        pm.insert("brand".to_string(), Value::String(brand.clone()));
                    }
                    Value::Object(pm)
                })
                .collect(),
        ),
    );
    m.insert(
        "result".to_string(),
        match &f.result {
            None => Value::String("void".to_string()),
            Some(tr) => type_ref_to_value(tr),
        },
    );
    m
}

pub fn expr_to_value(e: &Expr) -> Value {
    match e {
        Expr::Int { value, .. } => Value::Number((*value).into()),
        Expr::Ident { name, .. } => Value::String(name.clone()),
        Expr::List { items, .. } => Value::Array(items.iter().map(expr_to_value).collect()),
    }
}

pub fn canon_value_jcs(v: &mut Value) {
    match v {
        Value::Array(items) => {
            for item in items {
                canon_value_jcs(item);
            }
        }
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            for (_, value) in entries.iter_mut() {
                canon_value_jcs(value);
            }
            entries.sort_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
            for (k, v) in entries {
                map.insert(k, v);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
