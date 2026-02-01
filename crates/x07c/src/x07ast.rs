use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use x07_contracts::X07AST_SCHEMA_VERSION;

use crate::ast::Expr;
use crate::program::{AsyncFunctionDef, ExternAbi, ExternFunctionDecl, FunctionDef, FunctionParam};
use crate::types::Ty;
use crate::validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X07AstKind {
    Entry,
    Module,
}

#[derive(Debug, Clone)]
pub struct X07AstFile {
    pub kind: X07AstKind,
    pub module_id: String,
    pub imports: BTreeSet<String>,
    pub exports: BTreeSet<String>,
    pub functions: Vec<FunctionDef>,
    pub async_functions: Vec<AsyncFunctionDef>,
    pub extern_functions: Vec<ExternFunctionDecl>,
    pub solve: Option<Expr>,
    pub meta: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct X07AstError {
    pub message: String,
    pub ptr: String,
}

impl std::fmt::Display for X07AstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.ptr.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{} at {}", self.message, self.ptr)
        }
    }
}

impl std::error::Error for X07AstError {}

pub fn parse_x07ast_json(bytes: &[u8]) -> Result<X07AstFile, X07AstError> {
    if bytes.is_empty() {
        return Err(X07AstError {
            message: "empty input".to_string(),
            ptr: "".to_string(),
        });
    }

    let root: Value = serde_json::from_slice(bytes).map_err(|e| X07AstError {
        message: format!("invalid JSON: {e}"),
        ptr: "".to_string(),
    })?;
    parse_x07ast_value(&root)
}

fn parse_x07ast_value(root: &Value) -> Result<X07AstFile, X07AstError> {
    let obj = root.as_object().ok_or_else(|| X07AstError {
        message: "x07AST root must be a JSON object".to_string(),
        ptr: "".to_string(),
    })?;

    let schema_version = get_required_string(obj, "/schema_version", "schema_version")?;
    if schema_version != X07AST_SCHEMA_VERSION {
        return Err(X07AstError {
            message: format!(
                "unsupported schema_version: expected {X07AST_SCHEMA_VERSION} got {schema_version:?}"
            ),
            ptr: "/schema_version".to_string(),
        });
    }

    let kind_raw = get_required_string(obj, "/kind", "kind")?;
    let kind = match kind_raw.as_str() {
        "entry" => X07AstKind::Entry,
        "module" => X07AstKind::Module,
        _ => {
            return Err(X07AstError {
                message: format!("invalid kind: expected \"entry\" or \"module\" got {kind_raw:?}"),
                ptr: "/kind".to_string(),
            })
        }
    };

    let module_id = get_required_string(obj, "/module_id", "module_id")?;
    validate::validate_module_id(&module_id).map_err(|message| X07AstError {
        message,
        ptr: "/module_id".to_string(),
    })?;

    let imports = parse_string_array(obj, "/imports", "imports")?;
    let mut imports_set: BTreeSet<String> = BTreeSet::new();
    for (idx, m) in imports.into_iter().enumerate() {
        if m == "main" {
            return Err(X07AstError {
                message: "import of reserved module 'main'".to_string(),
                ptr: format!("/imports/{idx}"),
            });
        }
        validate::validate_module_id(&m).map_err(|message| X07AstError {
            message,
            ptr: format!("/imports/{idx}"),
        })?;
        imports_set.insert(m);
    }

    let decls_v = obj.get("decls").ok_or_else(|| X07AstError {
        message: "missing required field: decls".to_string(),
        ptr: "".to_string(),
    })?;
    let decls = decls_v.as_array().ok_or_else(|| X07AstError {
        message: "decls must be an array".to_string(),
        ptr: "/decls".to_string(),
    })?;

    let mut exports: BTreeSet<String> = BTreeSet::new();
    let mut functions: Vec<FunctionDef> = Vec::new();
    let mut async_functions: Vec<AsyncFunctionDef> = Vec::new();
    let mut extern_functions: Vec<ExternFunctionDecl> = Vec::new();
    let mut function_names: BTreeSet<String> = BTreeSet::new();

    for (idx, d) in decls.iter().enumerate() {
        let ptr = format!("/decls/{idx}");
        let dobj = d.as_object().ok_or_else(|| X07AstError {
            message: "decl must be an object".to_string(),
            ptr: ptr.clone(),
        })?;
        let dkind = get_required_string(dobj, &format!("{ptr}/kind"), "kind")?;
        match dkind.as_str() {
            "export" => {
                let names = parse_string_array(dobj, &format!("{ptr}/names"), "names")?;
                if names.is_empty() {
                    return Err(X07AstError {
                        message: "export.names must not be empty".to_string(),
                        ptr: format!("{ptr}/names"),
                    });
                }
                for (nidx, name) in names.into_iter().enumerate() {
                    validate::validate_symbol(&name).map_err(|message| X07AstError {
                        message,
                        ptr: format!("{ptr}/names/{nidx}"),
                    })?;
                    exports.insert(name);
                }
            }
            "defn" => {
                let f = parse_def_like(dobj, &ptr, &module_id, false)?;
                if !function_names.insert(f.name.clone()) {
                    return Err(X07AstError {
                        message: format!("duplicate function name: {:?}", f.name),
                        ptr: format!("{ptr}/name"),
                    });
                }
                functions.push(FunctionDef {
                    name: f.name,
                    params: f.params,
                    ret_ty: f.ret_ty,
                    ret_brand: f.ret_brand,
                    body: f.body,
                });
            }
            "defasync" => {
                let f = parse_def_like(dobj, &ptr, &module_id, true)?;
                if !function_names.insert(f.name.clone()) {
                    return Err(X07AstError {
                        message: format!("duplicate function name: {:?}", f.name),
                        ptr: format!("{ptr}/name"),
                    });
                }
                async_functions.push(AsyncFunctionDef {
                    name: f.name,
                    params: f.params,
                    ret_ty: f.ret_ty,
                    ret_brand: f.ret_brand,
                    body: f.body,
                });
            }
            "extern" => {
                let f = parse_extern(dobj, &ptr, &module_id)?;
                if !function_names.insert(f.name.clone()) {
                    return Err(X07AstError {
                        message: format!("duplicate function name: {:?}", f.name),
                        ptr: format!("{ptr}/name"),
                    });
                }
                extern_functions.push(f);
            }
            _ => {
                return Err(X07AstError {
                    message: format!(
                        "unknown decl kind: expected \"defn\", \"defasync\", \"extern\", or \"export\" got {dkind:?}"
                    ),
                    ptr: format!("{ptr}/kind"),
                })
            }
        }
    }

    let solve = match kind {
        X07AstKind::Entry => {
            let solve_v = obj.get("solve").ok_or_else(|| X07AstError {
                message: "missing required field: solve (required when kind==\"entry\")"
                    .to_string(),
                ptr: "".to_string(),
            })?;
            Some(expr_from_json_sexpr(solve_v, "/solve")?)
        }
        X07AstKind::Module => {
            if obj.contains_key("solve") {
                return Err(X07AstError {
                    message: "module files must not contain a solve expression".to_string(),
                    ptr: "/solve".to_string(),
                });
            }
            None
        }
    };

    let mut meta: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(meta_v) = obj.get("meta") {
        let meta_obj = meta_v.as_object().ok_or_else(|| X07AstError {
            message: "meta must be a JSON object".to_string(),
            ptr: "/meta".to_string(),
        })?;
        for (k, v) in meta_obj {
            meta.insert(k.clone(), v.clone());
        }
    }

    Ok(X07AstFile {
        kind,
        module_id,
        imports: imports_set,
        exports,
        functions,
        async_functions,
        extern_functions,
        solve,
        meta,
    })
}

fn parse_def_like(
    dobj: &serde_json::Map<String, Value>,
    ptr: &str,
    module_id: &str,
    is_async: bool,
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

    let params = parse_params(dobj, ptr, kind_label)?;

    let ret_ty_name = get_required_string(dobj, &format!("{ptr}/result"), "result")?;
    validate::validate_type_name(&ret_ty_name).map_err(|message| X07AstError {
        message,
        ptr: format!("{ptr}/result"),
    })?;
    let ret_ty = Ty::parse_named(&ret_ty_name).ok_or_else(|| X07AstError {
        message: format!(
            "unknown type: {ret_ty_name:?} (expected i32, bytes, bytes_view, vec_u8, option_i32, option_bytes, option_bytes_view, result_i32, result_bytes, result_bytes_view, result_result_bytes, iface, ptr_const_u8, ptr_mut_u8, ptr_const_void, ptr_mut_void, ptr_const_i32, or ptr_mut_i32)"
        ),
        ptr: format!("{ptr}/result"),
    })?;

    let ret_brand = if let Some(v) = dobj.get("result_brand") {
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
    if ret_brand.is_some()
        && !matches!(
            ret_ty,
            Ty::Bytes
                | Ty::OptionBytes
                | Ty::OptionBytesView
                | Ty::ResultBytes
                | Ty::ResultBytesView
        )
    {
        return Err(X07AstError {
            message: format!(
                "E_BRAND_BAD_TY: result_brand is not allowed on result type {ret_ty:?}"
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
        params,
        ret_ty,
        ret_brand,
        body,
    })
}

fn parse_extern(
    dobj: &serde_json::Map<String, Value>,
    ptr: &str,
    module_id: &str,
) -> Result<ExternFunctionDecl, X07AstError> {
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
        if !p.ty.is_ffi_ty() {
            return Err(X07AstError {
                message:
                    "extern param has unsupported type (only i32 and raw pointers are supported)"
                        .to_string(),
                ptr: format!("{ptr}/params/{idx}/ty"),
            });
        }
    }

    let ret_ty_name = get_required_string(dobj, &format!("{ptr}/result"), "result")?;
    validate::validate_type_name(&ret_ty_name).map_err(|message| X07AstError {
        message,
        ptr: format!("{ptr}/result"),
    })?;

    let (ret_is_void, ret_ty) = if ret_ty_name == "void" {
        (true, Ty::I32)
    } else {
        let ty = Ty::parse_named(&ret_ty_name).ok_or_else(|| X07AstError {
            message: format!(
                "unknown type: {ret_ty_name:?} (expected i32, ptr_const_u8, ptr_mut_u8, ptr_const_void, ptr_mut_void, ptr_const_i32, ptr_mut_i32, or void)"
            ),
            ptr: format!("{ptr}/result"),
        })?;
        if !ty.is_ffi_ty() {
            return Err(X07AstError {
                message:
                    "extern result has unsupported type (only i32 and raw pointers are supported)"
                        .to_string(),
                ptr: format!("{ptr}/result"),
            });
        }
        (false, ty)
    };

    Ok(ExternFunctionDecl {
        name,
        link_name,
        abi: ExternAbi::C,
        params,
        ret_ty,
        ret_is_void,
    })
}

fn parse_params(
    dobj: &serde_json::Map<String, Value>,
    ptr: &str,
    kind_label: &str,
) -> Result<Vec<FunctionParam>, X07AstError> {
    let params_v = dobj.get("params").ok_or_else(|| X07AstError {
        message: format!("missing required field: params in {kind_label}"),
        ptr: ptr.to_string(),
    })?;
    let params_a = params_v.as_array().ok_or_else(|| X07AstError {
        message: format!("{kind_label}.params must be an array"),
        ptr: format!("{ptr}/params"),
    })?;
    let mut params: Vec<FunctionParam> = Vec::with_capacity(params_a.len());
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
                message: format!("{kind_label} arg name must not be 'input'"),
                ptr: format!("{pptr}/name"),
            });
        }
        let ty_name = get_required_string(pobj, &format!("{pptr}/ty"), "ty")?;
        validate::validate_type_name(&ty_name).map_err(|message| X07AstError {
            message,
            ptr: format!("{pptr}/ty"),
        })?;
        let ty = Ty::parse_named(&ty_name).ok_or_else(|| X07AstError {
            message: format!(
                "unknown type: {ty_name:?} (expected i32, bytes, bytes_view, vec_u8, option_i32, option_bytes, result_i32, result_bytes, iface, ptr_const_u8, ptr_mut_u8, ptr_const_void, ptr_mut_void, ptr_const_i32, or ptr_mut_i32)"
            ),
            ptr: format!("{pptr}/ty"),
        })?;

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
        if brand.is_some()
            && !matches!(
                ty,
                Ty::Bytes
                    | Ty::BytesView
                    | Ty::OptionBytes
                    | Ty::OptionBytesView
                    | Ty::ResultBytes
                    | Ty::ResultBytesView
            )
        {
            return Err(X07AstError {
                message: format!("E_BRAND_BAD_TY: brand is not allowed on param type {ty:?}"),
                ptr: format!("{pptr}/ty"),
            });
        }

        params.push(FunctionParam {
            name: arg_name,
            ty,
            brand,
        });
    }
    Ok(params)
}

#[derive(Debug, Clone)]
struct ParsedDefLike {
    name: String,
    params: Vec<FunctionParam>,
    ret_ty: Ty,
    ret_brand: Option<String>,
    body: Expr,
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
                if head == "bytes.lit" && idx == 1 {
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
        Value::String(X07AST_SCHEMA_VERSION.to_string()),
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
            &f.params,
            f.ret_ty,
            f.ret_brand.as_deref(),
            &f.body,
        )));
    }
    for f in &file.async_functions {
        out.push(Value::Object(def_decl_value(
            "defasync",
            &f.name,
            &f.params,
            f.ret_ty,
            f.ret_brand.as_deref(),
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

fn def_decl_value(
    kind: &str,
    name: &str,
    params: &[FunctionParam],
    result: Ty,
    result_brand: Option<&str>,
    body: &Expr,
) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("kind".to_string(), Value::String(kind.to_string()));
    m.insert("name".to_string(), Value::String(name.to_string()));
    m.insert(
        "params".to_string(),
        Value::Array(
            params
                .iter()
                .map(|p| {
                    let mut pm = serde_json::Map::new();
                    pm.insert("name".to_string(), Value::String(p.name.clone()));
                    pm.insert(
                        "ty".to_string(),
                        Value::String(ty_to_name(p.ty).to_string()),
                    );
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
        Value::String(ty_to_name(result).to_string()),
    );
    if let Some(brand) = result_brand {
        m.insert("result_brand".to_string(), Value::String(brand.to_string()));
    }
    m.insert("body".to_string(), expr_to_value(body));
    m
}

fn extern_decl_value(f: &ExternFunctionDecl) -> serde_json::Map<String, Value> {
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
                    pm.insert(
                        "ty".to_string(),
                        Value::String(ty_to_name(p.ty).to_string()),
                    );
                    Value::Object(pm)
                })
                .collect(),
        ),
    );
    m.insert(
        "result".to_string(),
        Value::String(if f.ret_is_void {
            "void".to_string()
        } else {
            ty_to_name(f.ret_ty).to_string()
        }),
    );
    m
}

fn ty_to_name(ty: Ty) -> &'static str {
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
