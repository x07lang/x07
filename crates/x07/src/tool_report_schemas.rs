use std::ffi::OsStr;

pub(crate) fn tool_report_schema_bytes(scope: Option<&OsStr>) -> Option<&'static [u8]> {
    match scope.and_then(|s| s.to_str()) {
        None => Some(include_bytes!(
            "../../../spec/x07-tool-root.report.schema.json"
        )),
        Some("agent") => Some(include_bytes!(
            "../../../spec/x07-tool-agent.report.schema.json"
        )),
        Some("agent.context") => Some(include_bytes!(
            "../../../spec/x07-tool-agent-context.report.schema.json"
        )),
        Some("arch") => Some(include_bytes!(
            "../../../spec/x07-tool-arch.report.schema.json"
        )),
        Some("arch.check") => Some(include_bytes!(
            "../../../spec/x07-tool-arch-check.report.schema.json"
        )),
        Some("assets") => Some(include_bytes!(
            "../../../spec/x07-tool-assets.report.schema.json"
        )),
        Some("assets.embed-dir") => Some(include_bytes!(
            "../../../spec/x07-tool-assets-embed-dir.report.schema.json"
        )),
        Some("ast") => Some(include_bytes!(
            "../../../spec/x07-tool-ast.report.schema.json"
        )),
        Some("ast.apply-patch") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-apply-patch.report.schema.json"
        )),
        Some("ast.canon") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-canon.report.schema.json"
        )),
        Some("ast.edit") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-edit.report.schema.json"
        )),
        Some("ast.edit.apply-quickfix") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-edit-apply-quickfix.report.schema.json"
        )),
        Some("ast.edit.insert-stmts") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-edit-insert-stmts.report.schema.json"
        )),
        Some("ast.get") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-get.report.schema.json"
        )),
        Some("ast.grammar") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-grammar.report.schema.json"
        )),
        Some("ast.init") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-init.report.schema.json"
        )),
        Some("ast.schema") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-schema.report.schema.json"
        )),
        Some("ast.slice") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-slice.report.schema.json"
        )),
        Some("ast.validate") => Some(include_bytes!(
            "../../../spec/x07-tool-ast-validate.report.schema.json"
        )),
        Some("bench") => Some(include_bytes!(
            "../../../spec/x07-tool-bench.report.schema.json"
        )),
        Some("bench.eval") => Some(include_bytes!(
            "../../../spec/x07-tool-bench-eval.report.schema.json"
        )),
        Some("bench.list") => Some(include_bytes!(
            "../../../spec/x07-tool-bench-list.report.schema.json"
        )),
        Some("bench.validate") => Some(include_bytes!(
            "../../../spec/x07-tool-bench-validate.report.schema.json"
        )),
        Some("build") => Some(include_bytes!(
            "../../../spec/x07-tool-build.report.schema.json"
        )),
        Some("bundle") => Some(include_bytes!(
            "../../../spec/x07-tool-bundle.report.schema.json"
        )),
        Some("check") => Some(include_bytes!(
            "../../../spec/x07-tool-check.report.schema.json"
        )),
        Some("cli") => Some(include_bytes!(
            "../../../spec/x07-tool-cli.report.schema.json"
        )),
        Some("cli.spec") => Some(include_bytes!(
            "../../../spec/x07-tool-cli-spec.report.schema.json"
        )),
        Some("cli.spec.check") => Some(include_bytes!(
            "../../../spec/x07-tool-cli-spec-check.report.schema.json"
        )),
        Some("cli.spec.compile") => Some(include_bytes!(
            "../../../spec/x07-tool-cli-spec-compile.report.schema.json"
        )),
        Some("cli.spec.fmt") => Some(include_bytes!(
            "../../../spec/x07-tool-cli-spec-fmt.report.schema.json"
        )),
        Some("diag") => Some(include_bytes!(
            "../../../spec/x07-tool-diag.report.schema.json"
        )),
        Some("diag.catalog") => Some(include_bytes!(
            "../../../spec/x07-tool-diag-catalog.report.schema.json"
        )),
        Some("diag.check") => Some(include_bytes!(
            "../../../spec/x07-tool-diag-check.report.schema.json"
        )),
        Some("diag.coverage") => Some(include_bytes!(
            "../../../spec/x07-tool-diag-coverage.report.schema.json"
        )),
        Some("diag.explain") => Some(include_bytes!(
            "../../../spec/x07-tool-diag-explain.report.schema.json"
        )),
        Some("diag.init-catalog") => Some(include_bytes!(
            "../../../spec/x07-tool-diag-init-catalog.report.schema.json"
        )),
        Some("diag.sarif") => Some(include_bytes!(
            "../../../spec/x07-tool-diag-sarif.report.schema.json"
        )),
        Some("doctor") => Some(include_bytes!(
            "../../../spec/x07-tool-doctor.report.schema.json"
        )),
        Some("fix") => Some(include_bytes!(
            "../../../spec/x07-tool-fix.report.schema.json"
        )),
        Some("fmt") => Some(include_bytes!(
            "../../../spec/x07-tool-fmt.report.schema.json"
        )),
        Some("guide") => Some(include_bytes!(
            "../../../spec/x07-tool-guide.report.schema.json"
        )),
        Some("init") => Some(include_bytes!(
            "../../../spec/x07-tool-init.report.schema.json"
        )),
        Some("lint") => Some(include_bytes!(
            "../../../spec/x07-tool-lint.report.schema.json"
        )),
        Some("mcp") => Some(include_bytes!(
            "../../../spec/x07-tool-mcp.report.schema.json"
        )),
        Some("patch") => Some(include_bytes!(
            "../../../spec/x07-tool-patch.report.schema.json"
        )),
        Some("patch.apply") => Some(include_bytes!(
            "../../../spec/x07-tool-patch-apply.report.schema.json"
        )),
        Some("pkg") => Some(include_bytes!(
            "../../../spec/x07-tool-pkg.report.schema.json"
        )),
        Some("pkg.add") => Some(include_bytes!(
            "../../../spec/x07-tool-pkg-add.report.schema.json"
        )),
        Some("pkg.lock") => Some(include_bytes!(
            "../../../spec/x07-tool-pkg-lock.report.schema.json"
        )),
        Some("pkg.login") => Some(include_bytes!(
            "../../../spec/x07-tool-pkg-login.report.schema.json"
        )),
        Some("pkg.pack") => Some(include_bytes!(
            "../../../spec/x07-tool-pkg-pack.report.schema.json"
        )),
        Some("pkg.provides") => Some(include_bytes!(
            "../../../spec/x07-tool-pkg-provides.report.schema.json"
        )),
        Some("pkg.publish") => Some(include_bytes!(
            "../../../spec/x07-tool-pkg-publish.report.schema.json"
        )),
        Some("pkg.remove") => Some(include_bytes!(
            "../../../spec/x07-tool-pkg-remove.report.schema.json"
        )),
        Some("pkg.versions") => Some(include_bytes!(
            "../../../spec/x07-tool-pkg-versions.report.schema.json"
        )),
        Some("policy") => Some(include_bytes!(
            "../../../spec/x07-tool-policy.report.schema.json"
        )),
        Some("policy.init") => Some(include_bytes!(
            "../../../spec/x07-tool-policy-init.report.schema.json"
        )),
        Some("review") => Some(include_bytes!(
            "../../../spec/x07-tool-review.report.schema.json"
        )),
        Some("review.diff") => Some(include_bytes!(
            "../../../spec/x07-tool-review-diff.report.schema.json"
        )),
        Some("rr") => Some(include_bytes!(
            "../../../spec/x07-tool-rr.report.schema.json"
        )),
        Some("rr.record") => Some(include_bytes!(
            "../../../spec/x07-tool-rr-record.report.schema.json"
        )),
        Some("run") => Some(include_bytes!(
            "../../../spec/x07-tool-run.report.schema.json"
        )),
        Some("schema") => Some(include_bytes!(
            "../../../spec/x07-tool-schema.report.schema.json"
        )),
        Some("schema.derive") => Some(include_bytes!(
            "../../../spec/x07-tool-schema-derive.report.schema.json"
        )),
        Some("sm") => Some(include_bytes!(
            "../../../spec/x07-tool-sm.report.schema.json"
        )),
        Some("sm.check") => Some(include_bytes!(
            "../../../spec/x07-tool-sm-check.report.schema.json"
        )),
        Some("sm.gen") => Some(include_bytes!(
            "../../../spec/x07-tool-sm-gen.report.schema.json"
        )),
        Some("test") => Some(include_bytes!(
            "../../../spec/x07-tool-test.report.schema.json"
        )),
        Some("trust") => Some(include_bytes!(
            "../../../spec/x07-tool-trust.report.schema.json"
        )),
        Some("trust.report") => Some(include_bytes!(
            "../../../spec/x07-tool-trust-report.report.schema.json"
        )),
        Some("verify") => Some(include_bytes!(
            "../../../spec/x07-tool-verify.report.schema.json"
        )),
        Some("wasm") => Some(include_bytes!(
            "../../../spec/x07-tool-wasm.report.schema.json"
        )),
        _ => None,
    }
}
