mod lower;
mod parse;
mod validate;

use std::path::Path;

use anyhow::Result;

use crate::x07ir::X07Module;

pub fn import_c_file(module_id: &str, src_path: &Path) -> Result<X07Module> {
    let bytes = std::fs::read(src_path)?;
    let src = std::str::from_utf8(&bytes)?;

    validate::validate_source_text(src_path, src)?;

    let abs = src_path.canonicalize()?;
    let tu = parse::parse_translation_unit(&abs)?;
    let fns = validate::extract_functions(src_path, src, &tu)?;

    Ok(X07Module {
        module_id: module_id.to_string(),
        source_path: Some(src_path.to_string_lossy().to_string()),
        source_sha256: Some(crate::util::sha256_hex(bytes.as_slice())),
        funcs: lower::lower_module(module_id, &fns)?,
    })
}
