use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use x07_contracts::{X07AST_SCHEMA_VERSION, X07C_REPORT_SCHEMA_VERSION};
use x07c::diagnostics;
use x07c::x07ast;

use crate::pbt;
use crate::util;

#[derive(Debug, Clone)]
pub(crate) struct FixFromPbtOutcome {
    pub repro_path: PathBuf,
    pub tests_manifest_path: PathBuf,
    pub wrapper_module_id: String,
    pub wrapper_module_path: PathBuf,
    pub copied_repro_path: PathBuf,
    pub new_test_id: String,
    pub case_sha256_hex: String,
    pub case_len_bytes: usize,
    pub wrote_anything: bool,
}

#[derive(Debug, Serialize)]
struct X07cToolReport {
    schema_version: &'static str,
    command: &'static str,
    ok: bool,
    #[serde(rename = "in")]
    r#in: String,
    diagnostics_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<diagnostics::Diagnostic>,
    exit_code: u8,
}

#[derive(Debug)]
pub(crate) enum FixFromPbtError {
    ArgsRequiresWrite {
        repro_path: PathBuf,
    },
    ReproRead {
        repro_path: PathBuf,
        err: std::io::Error,
    },
    ReproParse {
        repro_path: PathBuf,
        message: String,
    },
    ReproSchema {
        repro_path: PathBuf,
        message: String,
    },
    Manifest {
        tests_manifest_path: PathBuf,
        message: String,
    },
    TestNotFound {
        tests_manifest_path: PathBuf,
        test_id: String,
    },
    Conflict {
        path: PathBuf,
        message: String,
    },
    #[allow(dead_code)]
    UnsupportedTy {
        repro_path: PathBuf,
        ty: String,
    },
}

impl FixFromPbtError {
    pub(crate) fn exit_code(&self) -> u8 {
        1
    }

    pub(crate) fn diagnostic(&self) -> diagnostics::Diagnostic {
        match self {
            FixFromPbtError::ArgsRequiresWrite { repro_path } => {
                let mut data = BTreeMap::new();
                data.insert(
                    "repro_path".to_string(),
                    Value::String(repro_path.display().to_string()),
                );
                diagnostics::Diagnostic {
                    code: "X07-PBT-FIX-ARGS-0001".to_string(),
                    severity: diagnostics::Severity::Error,
                    stage: diagnostics::Stage::Parse,
                    message: self.to_string(),
                    loc: None,
                    notes: Vec::new(),
                    related: Vec::new(),
                    data,
                    quickfix: None,
                }
            }
            FixFromPbtError::ReproRead { repro_path, err } => {
                let mut data = BTreeMap::new();
                data.insert(
                    "repro_path".to_string(),
                    Value::String(repro_path.display().to_string()),
                );
                data.insert("io_error".to_string(), Value::String(err.to_string()));
                diagnostics::Diagnostic {
                    code: "X07-PBT-REPRO-READ-0001".to_string(),
                    severity: diagnostics::Severity::Error,
                    stage: diagnostics::Stage::Parse,
                    message: self.to_string(),
                    loc: None,
                    notes: Vec::new(),
                    related: Vec::new(),
                    data,
                    quickfix: None,
                }
            }
            FixFromPbtError::ReproParse { repro_path, .. } => {
                let mut data = BTreeMap::new();
                data.insert(
                    "repro_path".to_string(),
                    Value::String(repro_path.display().to_string()),
                );
                diagnostics::Diagnostic {
                    code: "X07-PBT-REPRO-PARSE-0001".to_string(),
                    severity: diagnostics::Severity::Error,
                    stage: diagnostics::Stage::Parse,
                    message: self.to_string(),
                    loc: None,
                    notes: Vec::new(),
                    related: Vec::new(),
                    data,
                    quickfix: None,
                }
            }
            FixFromPbtError::ReproSchema { repro_path, .. } => {
                let mut data = BTreeMap::new();
                data.insert(
                    "repro_path".to_string(),
                    Value::String(repro_path.display().to_string()),
                );
                diagnostics::Diagnostic {
                    code: "X07-PBT-REPRO-SCHEMA-0001".to_string(),
                    severity: diagnostics::Severity::Error,
                    stage: diagnostics::Stage::Parse,
                    message: self.to_string(),
                    loc: None,
                    notes: Vec::new(),
                    related: Vec::new(),
                    data,
                    quickfix: None,
                }
            }
            FixFromPbtError::Manifest {
                tests_manifest_path,
                ..
            } => {
                let mut data = BTreeMap::new();
                data.insert(
                    "tests_manifest_path".to_string(),
                    Value::String(tests_manifest_path.display().to_string()),
                );
                diagnostics::Diagnostic {
                    code: "X07-PBT-FIX-MANIFEST-0001".to_string(),
                    severity: diagnostics::Severity::Error,
                    stage: diagnostics::Stage::Parse,
                    message: self.to_string(),
                    loc: None,
                    notes: Vec::new(),
                    related: Vec::new(),
                    data,
                    quickfix: None,
                }
            }
            FixFromPbtError::TestNotFound {
                tests_manifest_path,
                test_id,
            } => {
                let mut data = BTreeMap::new();
                data.insert(
                    "tests_manifest_path".to_string(),
                    Value::String(tests_manifest_path.display().to_string()),
                );
                data.insert("test_id".to_string(), Value::String(test_id.clone()));
                diagnostics::Diagnostic {
                    code: "X07-PBT-FIX-TEST-NOT-FOUND-0001".to_string(),
                    severity: diagnostics::Severity::Error,
                    stage: diagnostics::Stage::Parse,
                    message: self.to_string(),
                    loc: None,
                    notes: Vec::new(),
                    related: Vec::new(),
                    data,
                    quickfix: None,
                }
            }
            FixFromPbtError::Conflict { path, .. } => {
                let mut data = BTreeMap::new();
                data.insert(
                    "path".to_string(),
                    Value::String(path.display().to_string()),
                );
                diagnostics::Diagnostic {
                    code: "X07-PBT-FIX-CONFLICT-0001".to_string(),
                    severity: diagnostics::Severity::Error,
                    stage: diagnostics::Stage::Rewrite,
                    message: self.to_string(),
                    loc: None,
                    notes: Vec::new(),
                    related: Vec::new(),
                    data,
                    quickfix: None,
                }
            }
            FixFromPbtError::UnsupportedTy { repro_path, ty } => {
                let mut data = BTreeMap::new();
                data.insert(
                    "repro_path".to_string(),
                    Value::String(repro_path.display().to_string()),
                );
                data.insert("ty".to_string(), Value::String(ty.clone()));
                diagnostics::Diagnostic {
                    code: "X07-PBT-FIX-UNSUPPORTED-TY-0001".to_string(),
                    severity: diagnostics::Severity::Error,
                    stage: diagnostics::Stage::Rewrite,
                    message: self.to_string(),
                    loc: None,
                    notes: Vec::new(),
                    related: Vec::new(),
                    data,
                    quickfix: None,
                }
            }
        }
    }
}

impl std::fmt::Display for FixFromPbtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixFromPbtError::ArgsRequiresWrite { repro_path } => {
                write!(
                    f,
                    "x07 fix --from-pbt requires --write (repro {})",
                    repro_path.display()
                )
            }
            FixFromPbtError::ReproRead { repro_path, err } => {
                write!(
                    f,
                    "failed to read repro JSON {}: {err}",
                    repro_path.display()
                )
            }
            FixFromPbtError::ReproParse {
                repro_path,
                message,
            } => {
                write!(
                    f,
                    "failed to parse repro JSON {}: {message}",
                    repro_path.display()
                )
            }
            FixFromPbtError::ReproSchema {
                repro_path,
                message,
            } => {
                write!(f, "invalid repro JSON {}: {message}", repro_path.display())
            }
            FixFromPbtError::Manifest {
                tests_manifest_path,
                message,
            } => {
                write!(
                    f,
                    "tests manifest error {}: {message}",
                    tests_manifest_path.display()
                )
            }
            FixFromPbtError::TestNotFound {
                tests_manifest_path,
                test_id,
            } => {
                write!(
                    f,
                    "repro test.id not found in tests manifest {}: {test_id:?}",
                    tests_manifest_path.display()
                )
            }
            FixFromPbtError::Conflict { path, message } => {
                write!(f, "conflict at {}: {message}", path.display())
            }
            FixFromPbtError::UnsupportedTy { repro_path, ty } => {
                write!(
                    f,
                    "unsupported parameter type {ty:?} for wrapper generation (repro {})",
                    repro_path.display()
                )
            }
        }
    }
}

impl std::error::Error for FixFromPbtError {}

pub(crate) fn cmd_fix_from_pbt(
    repro_path: &Path,
    tests_manifest_path: &Path,
    out_dir: &Path,
    write: bool,
) -> Result<FixFromPbtOutcome> {
    if !write {
        return Err(anyhow::Error::new(FixFromPbtError::ArgsRequiresWrite {
            repro_path: repro_path.to_path_buf(),
        }));
    }

    let repro_bytes = match std::fs::read(repro_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err(anyhow::Error::new(FixFromPbtError::ReproRead {
                repro_path: repro_path.to_path_buf(),
                err,
            }));
        }
    };
    let repro = match pbt::parse_repro_json_detailed(&repro_bytes) {
        Ok(repro) => repro,
        Err(pbt::ReproJsonError::JsonParse(err)) => {
            return Err(anyhow::Error::new(FixFromPbtError::ReproParse {
                repro_path: repro_path.to_path_buf(),
                message: err.to_string(),
            }));
        }
        Err(pbt::ReproJsonError::SchemaVersion { expected, got }) => {
            return Err(anyhow::Error::new(FixFromPbtError::ReproSchema {
                repro_path: repro_path.to_path_buf(),
                message: format!("unsupported schema_version: expected {expected} got {got:?}"),
            }));
        }
        Err(pbt::ReproJsonError::SchemaValidatorBuild(err)) => {
            return Err(anyhow::Error::new(FixFromPbtError::ReproSchema {
                repro_path: repro_path.to_path_buf(),
                message: format!("schema validator build failed: {err}"),
            }));
        }
        Err(pbt::ReproJsonError::SchemaInvalid(err)) => {
            return Err(anyhow::Error::new(FixFromPbtError::ReproSchema {
                repro_path: repro_path.to_path_buf(),
                message: format!("schema validation failed: {err}"),
            }));
        }
        Err(pbt::ReproJsonError::Decode(err)) => {
            return Err(anyhow::Error::new(FixFromPbtError::ReproSchema {
                repro_path: repro_path.to_path_buf(),
                message: format!("decode failed: {err}"),
            }));
        }
    };

    let case_bytes = match pbt::counterexample_case_bytes(&repro) {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err(anyhow::Error::new(FixFromPbtError::ReproParse {
                repro_path: repro_path.to_path_buf(),
                message: err.to_string(),
            }));
        }
    };
    let case_sha256_hex = util::sha256_hex(&case_bytes);
    let case_tag = format!("c{}", &case_sha256_hex[0..12]);
    let case_len_bytes = case_bytes.len();

    let wrapper_module_id = format!("repro.pbt.{case_tag}");
    let wrapper_fn = format!("{wrapper_module_id}.run");
    let wrapper_module_path = out_dir.join(format!("{case_tag}.x07.json"));

    let copied_repro_path = out_dir.join(format!("{case_tag}.repro.json"));
    let new_test_id = format!("pbt_repro/{}/{case_tag}", repro.test.id);

    let manifest_text = std::fs::read_to_string(tests_manifest_path).map_err(|err| {
        anyhow::Error::new(FixFromPbtError::Manifest {
            tests_manifest_path: tests_manifest_path.to_path_buf(),
            message: format!("failed to read: {err}"),
        })
    })?;
    let manifest_doc: Value = serde_json::from_str(&manifest_text).map_err(|err| {
        anyhow::Error::new(FixFromPbtError::Manifest {
            tests_manifest_path: tests_manifest_path.to_path_buf(),
            message: format!("failed to parse JSON: {err}"),
        })
    })?;

    let tests = manifest_doc
        .get("tests")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            anyhow::Error::new(FixFromPbtError::Manifest {
                tests_manifest_path: tests_manifest_path.to_path_buf(),
                message: "missing top-level tests[] array".to_string(),
            })
        })?;

    let orig_test = tests
        .iter()
        .find(|t| t.get("id").and_then(Value::as_str) == Some(repro.test.id.as_str()))
        .ok_or_else(|| {
            anyhow::Error::new(FixFromPbtError::TestNotFound {
                tests_manifest_path: tests_manifest_path.to_path_buf(),
                test_id: repro.test.id.clone(),
            })
        })?;

    let orig_entry = orig_test
        .get("entry")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            anyhow::Error::new(FixFromPbtError::Manifest {
                tests_manifest_path: tests_manifest_path.to_path_buf(),
                message: format!("orig test missing entry for id={:?}", repro.test.id),
            })
        })?;
    if orig_entry != repro.test.entry {
        return Err(anyhow::Error::new(FixFromPbtError::Manifest {
            tests_manifest_path: tests_manifest_path.to_path_buf(),
            message: format!(
                "repro test.entry mismatch for id={:?}: repro={:?} manifest={:?}",
                repro.test.id, repro.test.entry, orig_entry
            ),
        }));
    }

    let orig_world = orig_test
        .get("world")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            anyhow::Error::new(FixFromPbtError::Manifest {
                tests_manifest_path: tests_manifest_path.to_path_buf(),
                message: format!("orig test missing world for id={:?}", repro.test.id),
            })
        })?;
    if orig_world != repro.test.world {
        return Err(anyhow::Error::new(FixFromPbtError::Manifest {
            tests_manifest_path: tests_manifest_path.to_path_buf(),
            message: format!(
                "repro test.world mismatch for id={:?}: repro={:?} manifest={:?}",
                repro.test.id, repro.test.world, orig_world
            ),
        }));
    }

    let fixture_root = orig_test
        .get("fixture_root")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let policy_json = orig_test
        .get("policy_json")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let timeout_ms = orig_test.get("timeout_ms").and_then(Value::as_u64);

    let input_b64 = repro.counterexample.case_bytes_b64.clone();

    let budget_scope = if let Some(v) = orig_test.get("pbt").and_then(|p| p.get("budget_scope")) {
        let raw: pbt::PbtBudgetScopeRaw = serde_json::from_value(v.clone()).map_err(|err| {
            anyhow::Error::new(FixFromPbtError::Manifest {
                tests_manifest_path: tests_manifest_path.to_path_buf(),
                message: format!("invalid pbt.budget_scope for id={:?}: {err}", repro.test.id),
            })
        })?;
        let checked = |field: &'static str, v: Option<u64>| -> Result<i32> {
            let Some(v) = v else {
                return Ok(0);
            };
            pbt::checked_u64_to_i32(field, v).map_err(|err| {
                anyhow::Error::new(FixFromPbtError::Manifest {
                    tests_manifest_path: tests_manifest_path.to_path_buf(),
                    message: format!(
                        "invalid pbt.budget_scope.{field} for id={:?}: {err}",
                        repro.test.id
                    ),
                })
            })
        };
        let scope = pbt::PbtBudgetScope {
            alloc_bytes: checked("alloc_bytes", raw.alloc_bytes)?,
            alloc_calls: checked("alloc_calls", raw.alloc_calls)?,
            realloc_calls: checked("realloc_calls", raw.realloc_calls)?,
            memcpy_bytes: checked("memcpy_bytes", raw.memcpy_bytes)?,
            sched_ticks: checked("sched_ticks", raw.sched_ticks)?,
        };
        Some(scope)
    } else {
        None
    };

    let mut new_test_doc = serde_json::json!({
        "id": new_test_id,
        "world": orig_world,
        "entry": wrapper_fn,
        "returns": "bytes_status_v1",
        "input_b64": input_b64,
        "expect": "pass",
    });
    if let Some(fr) = fixture_root.as_ref() {
        new_test_doc
            .as_object_mut()
            .context("internal error: new test doc must be object")?
            .insert("fixture_root".to_string(), Value::String(fr.to_string()));
    }
    if let Some(pj) = policy_json.as_ref() {
        new_test_doc
            .as_object_mut()
            .context("internal error: new test doc must be object")?
            .insert("policy_json".to_string(), Value::String(pj.to_string()));
    }
    if let Some(ms) = timeout_ms {
        new_test_doc
            .as_object_mut()
            .context("internal error: new test doc must be object")?
            .insert("timeout_ms".to_string(), Value::Number(ms.into()));
    }

    let existing_new = tests
        .iter()
        .find(|t| t.get("id").and_then(Value::as_str) == Some(new_test_id.as_str()));
    if let Some(existing) = existing_new {
        if existing != &new_test_doc {
            return Err(anyhow::Error::new(FixFromPbtError::Conflict {
                path: tests_manifest_path.to_path_buf(),
                message: format!(
                    "tests manifest already contains {new_test_id:?} but entry differs"
                ),
            }));
        }
    }

    let tys = pbt::counterexample_tys(&repro);
    let (imports, begin_expr) =
        pbt::build_case_call_begin_expr(&repro.test.entry, &tys, budget_scope).map_err(|err| {
            anyhow::Error::new(FixFromPbtError::ReproParse {
                repro_path: repro_path.to_path_buf(),
                message: err.to_string(),
            })
        })?;

    let wrapper_module_doc = serde_json::json!({
        "schema_version": X07AST_SCHEMA_VERSION,
        "kind": "module",
        "module_id": wrapper_module_id,
        "imports": imports,
        "decls": [
            { "kind": "export", "names": [wrapper_fn] },
            {
                "kind": "defn",
                "name": wrapper_fn,
                "params": [],
                "result": "bytes",
                "body": begin_expr,
            }
        ]
    });
    let wrapper_bytes = format_x07ast_json(&wrapper_module_doc).context("format wrapper module")?;

    let mut wrote_anything = false;

    wrote_anything |= write_if_missing_or_identical(&wrapper_module_path, &wrapper_bytes)?;

    let copied_repro_bytes =
        pbt::repro_to_pretty_canon_bytes(&repro).context("encode repro JSON")?;
    wrote_anything |= write_if_missing_or_identical(&copied_repro_path, &copied_repro_bytes)?;

    if existing_new.is_none() {
        let patched =
            append_manifest_test_entry_text(&manifest_text, &new_test_doc).map_err(|err| {
                anyhow::Error::new(FixFromPbtError::Manifest {
                    tests_manifest_path: tests_manifest_path.to_path_buf(),
                    message: err.to_string(),
                })
            })?;
        if patched != manifest_text {
            wrote_anything = true;
            util::write_atomic(tests_manifest_path, patched.as_bytes()).map_err(|err| {
                anyhow::Error::new(FixFromPbtError::Manifest {
                    tests_manifest_path: tests_manifest_path.to_path_buf(),
                    message: format!("failed to write: {err}"),
                })
            })?;
        }
    }

    Ok(FixFromPbtOutcome {
        repro_path: repro_path.to_path_buf(),
        tests_manifest_path: tests_manifest_path.to_path_buf(),
        wrapper_module_id,
        wrapper_module_path,
        copied_repro_path,
        new_test_id,
        case_sha256_hex,
        case_len_bytes,
        wrote_anything,
    })
}

pub(crate) fn fix_from_pbt_error_report_bytes(
    repro_path: &Path,
    err: &FixFromPbtError,
) -> Result<Vec<u8>> {
    let report = X07cToolReport {
        schema_version: X07C_REPORT_SCHEMA_VERSION,
        command: "fix",
        ok: false,
        r#in: repro_path.display().to_string(),
        diagnostics_count: 1,
        diagnostics: vec![err.diagnostic()],
        exit_code: err.exit_code(),
    };

    let mut v = serde_json::to_value(&report)?;
    x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_vec_pretty(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

pub(crate) fn fix_from_pbt_report_bytes(outcome: &FixFromPbtOutcome) -> Result<Vec<u8>> {
    let mut data = BTreeMap::new();
    data.insert(
        "repro_path".to_string(),
        Value::String(outcome.repro_path.display().to_string()),
    );
    data.insert(
        "tests_manifest_path".to_string(),
        Value::String(outcome.tests_manifest_path.display().to_string()),
    );
    data.insert(
        "new_test_id".to_string(),
        Value::String(outcome.new_test_id.clone()),
    );
    data.insert(
        "wrapper_module_id".to_string(),
        Value::String(outcome.wrapper_module_id.clone()),
    );
    data.insert(
        "wrapper_module_path".to_string(),
        Value::String(outcome.wrapper_module_path.display().to_string()),
    );
    data.insert(
        "copied_repro_path".to_string(),
        Value::String(outcome.copied_repro_path.display().to_string()),
    );
    data.insert(
        "case_sha256_hex".to_string(),
        Value::String(outcome.case_sha256_hex.clone()),
    );
    data.insert(
        "case_len_bytes".to_string(),
        Value::Number(outcome.case_len_bytes.into()),
    );

    let message = if outcome.wrote_anything {
        "wrote PBT repro regression test".to_string()
    } else {
        "PBT repro regression test already present".to_string()
    };

    let diag = diagnostics::Diagnostic {
        code: "X07-PBT-FIX-INFO-0001".to_string(),
        severity: diagnostics::Severity::Info,
        stage: diagnostics::Stage::Rewrite,
        message,
        loc: None,
        notes: Vec::new(),
        related: Vec::new(),
        data,
        quickfix: None,
    };

    let report = X07cToolReport {
        schema_version: X07C_REPORT_SCHEMA_VERSION,
        command: "fix",
        ok: true,
        r#in: outcome.repro_path.display().to_string(),
        diagnostics_count: 1,
        diagnostics: vec![diag],
        exit_code: 0,
    };

    let mut v = serde_json::to_value(&report)?;
    x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_vec_pretty(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn write_if_missing_or_identical(path: &Path, bytes: &[u8]) -> Result<bool> {
    if path.is_file() {
        let existing = std::fs::read(path).map_err(|err| {
            anyhow::Error::new(FixFromPbtError::Conflict {
                path: path.to_path_buf(),
                message: format!("failed to read existing file: {err}"),
            })
        })?;
        if existing != bytes {
            return Err(anyhow::Error::new(FixFromPbtError::Conflict {
                path: path.to_path_buf(),
                message: "refusing to overwrite non-identical file".to_string(),
            }));
        }
        return Ok(false);
    }

    util::write_atomic(path, bytes).map_err(|err| {
        anyhow::Error::new(FixFromPbtError::Conflict {
            path: path.to_path_buf(),
            message: format!("failed to write file: {err}"),
        })
    })?;
    Ok(true)
}

fn format_x07ast_json(doc: &Value) -> Result<Vec<u8>> {
    let mut file =
        x07ast::parse_x07ast_json(&serde_json::to_vec(doc)?).map_err(|e| anyhow::anyhow!("{e}"))?;
    x07ast::canonicalize_x07ast_file(&mut file);
    let mut v = x07ast::x07ast_file_to_value(&file);
    x07ast::canon_value_jcs(&mut v);
    let mut out = serde_json::to_vec(&v)?;
    if out.last() != Some(&b'\n') {
        out.push(b'\n');
    }
    Ok(out)
}

fn append_manifest_test_entry_text(manifest_text: &str, new_test: &Value) -> Result<String> {
    let (arr_start, arr_end) = find_json_array_span_for_key(manifest_text, "tests")
        .context("find tests[] array in manifest text")?;

    let indent_close = line_indent_at(manifest_text, arr_end);

    let array_body = &manifest_text[arr_start + 1..arr_end];
    let is_empty = array_body.trim().is_empty();

    let indent_obj = if is_empty {
        format!("{indent_close}  ")
    } else {
        guess_array_elem_indent(manifest_text, arr_start, arr_end)
            .unwrap_or_else(|| format!("{indent_close}  "))
    };
    let indent_key = format!("{indent_obj}  ");

    let entry_text = render_test_entry(&indent_obj, &indent_key, new_test)?;

    if is_empty {
        let mut out = String::with_capacity(manifest_text.len() + entry_text.len() + 16);
        out.push_str(&manifest_text[..arr_start + 1]);
        out.push('\n');
        out.push_str(&entry_text);
        out.push('\n');
        out.push_str(&manifest_text[arr_end..]);
        return Ok(ensure_trailing_newline(out));
    }

    let insert_pos = manifest_text[..arr_end]
        .rfind('\n')
        .context("tests[] array must contain newlines")?;

    let mut out = String::with_capacity(manifest_text.len() + entry_text.len() + 16);
    out.push_str(&manifest_text[..insert_pos]);
    out.push_str(",\n");
    out.push_str(&entry_text);
    out.push_str(&manifest_text[insert_pos..]);
    Ok(ensure_trailing_newline(out))
}

fn render_test_entry(indent_obj: &str, indent_key: &str, v: &Value) -> Result<String> {
    let obj = v.as_object().context("new test must be object")?;

    let get_str = |k: &str| -> Result<Option<String>> {
        Ok(obj.get(k).and_then(Value::as_str).map(|s| s.to_string()))
    };
    let get_u64 = |k: &str| -> Option<u64> { obj.get(k).and_then(Value::as_u64) };

    let id = get_str("id")?.context("new test missing id")?;
    let world = get_str("world")?.context("new test missing world")?;
    let entry = get_str("entry")?.context("new test missing entry")?;
    let returns = get_str("returns")?.context("new test missing returns")?;
    let input_b64 = get_str("input_b64")?.context("new test missing input_b64")?;
    let fixture_root = get_str("fixture_root")?;
    let policy_json = get_str("policy_json")?;
    let timeout_ms = get_u64("timeout_ms");
    let expect = get_str("expect")?.context("new test missing expect")?;

    let mut fields: Vec<(String, String)> = vec![
        ("id".to_string(), serde_json::to_string(&id)?),
        ("world".to_string(), serde_json::to_string(&world)?),
        ("entry".to_string(), serde_json::to_string(&entry)?),
        ("returns".to_string(), serde_json::to_string(&returns)?),
        ("input_b64".to_string(), serde_json::to_string(&input_b64)?),
    ];
    if let Some(fr) = fixture_root.as_ref() {
        fields.push(("fixture_root".to_string(), serde_json::to_string(fr)?));
    }
    if let Some(pj) = policy_json.as_ref() {
        fields.push(("policy_json".to_string(), serde_json::to_string(pj)?));
    }
    if let Some(ms) = timeout_ms {
        fields.push(("timeout_ms".to_string(), ms.to_string()));
    }
    fields.push(("expect".to_string(), serde_json::to_string(&expect)?));

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("{indent_obj}{{"));
    for (idx, (k, raw)) in fields.iter().enumerate() {
        let comma = idx + 1 != fields.len();
        let mut line = format!("{indent_key}\"{k}\": {raw}");
        if comma {
            line.push(',');
        }
        lines.push(line);
    }
    lines.push(format!("{indent_obj}}}"));
    Ok(lines.join("\n"))
}

fn ensure_trailing_newline(mut s: String) -> String {
    s = s.replace("\r\n", "\n").replace('\r', "\n");
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

fn line_indent_at(text: &str, idx: usize) -> String {
    let before = &text[..idx.min(text.len())];
    let Some(nl) = before.rfind('\n') else {
        return String::new();
    };
    before[nl + 1..]
        .chars()
        .take_while(|c| c.is_whitespace() && *c != '\n' && *c != '\r')
        .collect()
}

fn guess_array_elem_indent(text: &str, arr_start: usize, arr_end: usize) -> Option<String> {
    let mut i = arr_start + 1;
    while i < arr_end {
        let b = text.as_bytes()[i];
        if !b.is_ascii_whitespace() {
            break;
        }
        i += 1;
    }
    if i >= arr_end {
        return None;
    }
    if text.as_bytes()[i] != b'{' {
        return None;
    }
    let before = &text[..i];
    let nl = before.rfind('\n')?;
    Some(before[nl + 1..].to_string())
}

fn find_json_array_span_for_key(text: &str, key: &str) -> Result<(usize, usize)> {
    let needle = format!("\"{key}\"");
    let bytes = text.as_bytes();
    let mut i: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if b == b'"' {
            if text[i..].starts_with(&needle) {
                let mut j = i + needle.len();
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j >= bytes.len() || bytes[j] != b':' {
                    in_string = true;
                    i += 1;
                    continue;
                }
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j >= bytes.len() || bytes[j] != b'[' {
                    anyhow::bail!("tests key exists but is not an array");
                }

                let start = j;
                let mut depth: i32 = 0;
                let mut k = j;
                let mut in2 = false;
                let mut esc2 = false;
                while k < bytes.len() {
                    let c = bytes[k];
                    if in2 {
                        if esc2 {
                            esc2 = false;
                        } else if c == b'\\' {
                            esc2 = true;
                        } else if c == b'"' {
                            in2 = false;
                        }
                        k += 1;
                        continue;
                    }
                    if c == b'"' {
                        in2 = true;
                        k += 1;
                        continue;
                    }
                    if c == b'[' {
                        depth += 1;
                    } else if c == b']' {
                        depth -= 1;
                        if depth == 0 {
                            return Ok((start, k));
                        }
                    }
                    k += 1;
                }
                anyhow::bail!("unterminated tests[] array");
            }
            in_string = true;
            i += 1;
            continue;
        }

        i += 1;
    }
    anyhow::bail!("key not found: {key:?}");
}
