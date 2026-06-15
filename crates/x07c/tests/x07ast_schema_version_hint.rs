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

#[test]
fn schema_version_newer_than_toolchain_points_at_toolchain_update() {
    // A schema newer than anything this toolchain supports: the toolchain is behind,
    // so the hint must point at updating it, not at upgrading a dependency package.
    let bytes = br#"{"schema_version":"x07.x07ast@9.9.9"}"#;
    let err = parse_x07ast_json(bytes).expect_err("expected schema version mismatch");
    assert!(
        err.message.contains("update the toolchain"),
        "expected toolchain-update hint, got: {:?}",
        err.message
    );
}
