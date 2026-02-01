use x07c::x07ast::parse_x07ast_json;

#[test]
fn schema_version_mismatch_includes_hint() {
    let bytes = br#"{"schema_version":"x07.x07ast@0.2.0"}"#;
    let err = parse_x07ast_json(bytes).expect_err("expected schema version mismatch");
    assert!(
        err.message.contains("hint:"),
        "expected hint in error message, got: {:?}",
        err.message
    );
}
