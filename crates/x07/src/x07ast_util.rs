use anyhow::Result;
use serde_json::Value;

pub(crate) fn canonicalize_x07ast_bytes_to_value(bytes: &[u8]) -> Result<Value> {
    let mut file = x07c::x07ast::parse_x07ast_json(bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    Ok(x07c::x07ast::x07ast_file_to_value(&file))
}
