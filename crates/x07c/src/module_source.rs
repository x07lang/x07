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
    if world.is_standalone_only() && module_id.starts_with("std.world.") {
        let (path, src) = read_module_from_roots(module_id, module_roots)?;
        return Ok(ModuleSource {
            module_id: module_id.to_string(),
            src,
            path: Some(path),
            is_builtin: false,
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
                Ok((path.clone(), src))
            }
            _ => Err(CompilerError::new(
                CompileErrorKind::Parse,
                format!("module {module_id:?} is ambiguous across roots: {json_hits:?}"),
            )),
        };
    }
    Err(CompilerError::new(
        CompileErrorKind::Parse,
        format!("unknown module: {module_id:?} (searched: {json_rel_display})"),
    ))
}
