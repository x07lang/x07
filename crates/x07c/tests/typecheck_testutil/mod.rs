use serde_json::Value;

pub fn file_from_json(doc: &Value) -> x07c::x07ast::X07AstFile {
    let bytes = serde_json::to_vec(doc).expect("encode x07AST json");
    let mut file = x07c::x07ast::parse_x07ast_json(&bytes).expect("parse x07AST");
    x07c::x07ast::canonicalize_x07ast_file(&mut file);
    file
}
