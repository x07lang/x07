use std::path::PathBuf;

use crate::builtin_modules;
use crate::compile::{CompileErrorKind, CompilerError};
use crate::validate;

#[derive(Debug, Clone)]
pub struct ModuleSource {
    pub module_id: String,
    pub src: String,
    pub path: Option<PathBuf>,
    pub is_builtin: bool,
}

pub fn load_module_source(
    module_id: &str,
    world: x07_worlds::WorldId,
    module_roots: &[PathBuf],
) -> Result<ModuleSource, CompilerError> {
    load_module_source_with_preference(module_id, world, module_roots, false)
}

pub fn load_module_source_with_preference(
    module_id: &str,
    world: x07_worlds::WorldId,
    module_roots: &[PathBuf],
    prefer_module_roots_first: bool,
) -> Result<ModuleSource, CompilerError> {
    if world.is_standalone_only() && module_id.starts_with("std.world.") {
        let (path, src) = read_module_from_roots(module_id, module_roots)?;
        return Ok(ModuleSource {
            module_id: module_id.to_string(),
            src,
            path: Some(path),
            is_builtin: false,
        });
    }

    if prefer_module_roots_first {
        if let Some((path, src)) = read_module_from_roots_if_present(module_id, module_roots)? {
            return Ok(ModuleSource {
                module_id: module_id.to_string(),
                src,
                path: Some(path),
                is_builtin: false,
            });
        }
    } else if let Some(src) = builtin_modules::builtin_module_source(module_id) {
        return Ok(ModuleSource {
            module_id: module_id.to_string(),
            src: src.to_string(),
            path: None,
            is_builtin: true,
        });
    }

    if let Some(src) = builtin_modules::builtin_module_source(module_id) {
        return Ok(ModuleSource {
            module_id: module_id.to_string(),
            src: src.to_string(),
            path: None,
            is_builtin: true,
        });
    }

    let (path, src) = read_module_from_roots(module_id, module_roots)?;
    Ok(ModuleSource {
        module_id: module_id.to_string(),
        src,
        path: Some(path),
        is_builtin: false,
    })
}

pub fn read_module_from_roots(
    module_id: &str,
    module_roots: &[PathBuf],
) -> Result<(PathBuf, String), CompilerError> {
    if let Some(found) = read_module_from_roots_if_present(module_id, module_roots)? {
        return Ok(found);
    }
    if module_roots.is_empty() {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!("unknown module: {module_id:?}"),
        ));
    }
    validate::validate_module_id(module_id)
        .map_err(|message| CompilerError::new(CompileErrorKind::Parse, message))?;

    let mut rel_path_base = PathBuf::new();
    for seg in module_id.split('.') {
        rel_path_base.push(seg);
    }

    let mut json_rel = rel_path_base.clone();
    json_rel.set_extension("x07.json");
    let json_rel_display = json_rel.display().to_string();
    let mut text_rel = rel_path_base.clone();
    text_rel.set_extension("x07t");
    let text_rel_display = text_rel.display().to_string();

    // Builtin operation namespaces that read like importable `std.*` modules but
    // are resolved as builtins (no module file). Importing them is the mistake;
    // the operations work without an `:imports` entry.
    const BUILTIN_NAMESPACES: &[&str] = &["std.brand"];
    if BUILTIN_NAMESPACES.contains(&module_id) {
        return Err(CompilerError::new(
            CompileErrorKind::Parse,
            format!(
                "unknown module: {module_id:?}; {module_id} provides builtins that need no import — remove it from :imports (its operations resolve without a module)"
            ),
        ));
    }

    Err(CompilerError::new(
        CompileErrorKind::Parse,
        format!(
            "unknown module: {module_id:?} (searched: {json_rel_display} or {text_rel_display})"
        ),
    ))
}

fn read_module_from_roots_if_present(
    module_id: &str,
    module_roots: &[PathBuf],
) -> Result<Option<(PathBuf, String)>, CompilerError> {
    if module_roots.is_empty() {
        return Ok(None);
    }

    validate::validate_module_id(module_id)
        .map_err(|message| CompilerError::new(CompileErrorKind::Parse, message))?;

    let mut rel_path_base = PathBuf::new();
    for seg in module_id.split('.') {
        rel_path_base.push(seg);
    }

    let mut json_rel = rel_path_base.clone();
    json_rel.set_extension("x07.json");
    let mut json_hits: Vec<PathBuf> = Vec::new();
    for root in module_roots {
        let path = root.join(&json_rel);
        if path.exists() {
            json_hits.push(path);
        }
    }
    if !json_hits.is_empty() {
        return match json_hits.len() {
            1 => {
                let path = &json_hits[0];
                let src = std::fs::read_to_string(path).map_err(|e| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("read module {module_id:?} at {}: {e}", path.display()),
                    )
                })?;
                Ok(Some((path.clone(), src)))
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("module {module_id:?} is ambiguous across roots: {json_hits:?}"),
            )),
        };
    }

    // Fallback: resolve x07text (`.x07t`) modules by parsing them to canonical
    // x07AST JSON, so a project can be authored in the readable text format with
    // no manual `x07 ast from-text` step. A `.x07.json` of the same name wins.
    let mut text_rel = rel_path_base.clone();
    text_rel.set_extension("x07t");
    let mut text_hits: Vec<PathBuf> = Vec::new();
    for root in module_roots {
        let path = root.join(&text_rel);
        if path.exists() {
            text_hits.push(path);
        }
    }
    if !text_hits.is_empty() {
        return match text_hits.len() {
            1 => {
                let path = &text_hits[0];
                let text = std::fs::read_to_string(path).map_err(|e| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("read module {module_id:?} at {}: {e}", path.display()),
                    )
                })?;
                let value = crate::x07text::from_text(&text).map_err(|e| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        format!(
                            "parse x07text module {module_id:?} at {}: {e}",
                            path.display()
                        ),
                    )
                })?;
                let src = serde_json::to_string(&value).map_err(|e| {
                    CompilerError::new(
                        CompileErrorKind::Parse,
                        format!("serialize x07text module {module_id:?}: {e}"),
                    )
                })?;
                Ok(Some((path.clone(), src)))
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("module {module_id:?} is ambiguous across roots (x07t): {text_hits:?}"),
            )),
        };
    }

    Ok(None)
}

#[cfg(test)]
mod builtin_namespace_hint_tests {
    use super::*;

    #[test]
    fn unknown_builtin_namespace_import_hints_removal() {
        let roots = vec![PathBuf::from("/nonexistent-root")];
        let err = read_module_from_roots("std.brand", &roots)
            .expect_err("std.brand is a builtin namespace, not a module");
        let msg = err.message.to_string();
        assert!(
            msg.contains("need no import") && msg.contains("remove it from :imports"),
            "expected builtin-namespace hint, got: {msg}"
        );
    }

    #[test]
    fn unknown_real_module_keeps_searched_path() {
        let roots = vec![PathBuf::from("/nonexistent-root")];
        let err = read_module_from_roots("acme.widget", &roots)
            .expect_err("acme.widget does not exist under the root");
        let msg = err.message.to_string();
        assert!(
            msg.contains("searched:") && msg.contains("acme/widget.x07.json"),
            "expected searched-path message, got: {msg}"
        );
    }
}
