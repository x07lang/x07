use serde_json::Value;

use crate::diagnostics::PatchOp;

#[derive(Debug, Clone)]
pub struct PatchError {
    pub message: String,
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PatchError {}

pub fn apply_patch(doc: &mut Value, ops: &[PatchOp]) -> Result<(), PatchError> {
    for op in ops {
        apply_op(doc, op)?;
    }
    Ok(())
}

fn apply_op(doc: &mut Value, op: &PatchOp) -> Result<(), PatchError> {
    match op {
        PatchOp::Add { path, value } => add(doc, path, value.clone()),
        PatchOp::Remove { path } => remove(doc, path),
        PatchOp::Replace { path, value } => replace(doc, path, value.clone()),
        PatchOp::Move { path, from } => {
            let v = get(doc, from)?.clone();
            remove(doc, from)?;
            add(doc, path, v)
        }
        PatchOp::Copy { path, from } => {
            let v = get(doc, from)?.clone();
            add(doc, path, v)
        }
        PatchOp::Test { path, value } => {
            let cur = get(doc, path)?;
            if cur != value {
                return Err(PatchError {
                    message: format!("test failed at {path:?}: expected {value:?} got {cur:?}"),
                });
            }
            Ok(())
        }
    }
}

fn decode_pointer(ptr: &str) -> Result<Vec<String>, PatchError> {
    if ptr.is_empty() {
        return Ok(Vec::new());
    }
    if !ptr.starts_with('/') {
        return Err(PatchError {
            message: format!("invalid JSON Pointer (must start with '/' or be empty): {ptr:?}"),
        });
    }
    let mut out = Vec::new();
    for raw in ptr.split('/').skip(1) {
        let token = raw.replace("~1", "/").replace("~0", "~");
        out.push(token);
    }
    Ok(out)
}

fn get<'a>(doc: &'a Value, ptr: &str) -> Result<&'a Value, PatchError> {
    let toks = decode_pointer(ptr)?;
    let mut cur = doc;
    for tok in toks {
        match cur {
            Value::Object(m) => {
                cur = m.get(&tok).ok_or_else(|| PatchError {
                    message: format!("path not found: {ptr:?}"),
                })?;
            }
            Value::Array(a) => {
                let idx = tok.parse::<usize>().map_err(|_| PatchError {
                    message: format!("expected array index in pointer {ptr:?}, got {tok:?}"),
                })?;
                cur = a.get(idx).ok_or_else(|| PatchError {
                    message: format!("array index out of bounds in pointer {ptr:?}: {idx}"),
                })?;
            }
            _ => {
                return Err(PatchError {
                    message: format!("cannot traverse into non-container at {ptr:?}"),
                })
            }
        }
    }
    Ok(cur)
}

fn get_parent_mut<'a>(
    doc: &'a mut Value,
    ptr: &str,
) -> Result<(&'a mut Value, String), PatchError> {
    let toks = decode_pointer(ptr)?;
    let (parent_toks, last) = toks
        .split_last()
        .map(|(last, rest)| (rest, last))
        .ok_or_else(|| PatchError {
            message: "JSON Pointer must not be empty for this operation".to_string(),
        })?;

    let mut cur = doc;
    for tok in parent_toks {
        match cur {
            Value::Object(m) => {
                cur = m.get_mut(tok).ok_or_else(|| PatchError {
                    message: format!("path not found: {ptr:?}"),
                })?;
            }
            Value::Array(a) => {
                let idx = tok.parse::<usize>().map_err(|_| PatchError {
                    message: format!("expected array index in pointer {ptr:?}, got {tok:?}"),
                })?;
                cur = a.get_mut(idx).ok_or_else(|| PatchError {
                    message: format!("array index out of bounds in pointer {ptr:?}: {idx}"),
                })?;
            }
            _ => {
                return Err(PatchError {
                    message: format!("cannot traverse into non-container at {ptr:?}"),
                })
            }
        }
    }

    Ok((cur, last.clone()))
}

fn add(doc: &mut Value, ptr: &str, value: Value) -> Result<(), PatchError> {
    if ptr.is_empty() {
        *doc = value;
        return Ok(());
    }
    let (parent, last) = get_parent_mut(doc, ptr)?;
    match parent {
        Value::Object(m) => {
            m.insert(last, value);
            Ok(())
        }
        Value::Array(a) => {
            if last == "-" {
                a.push(value);
                return Ok(());
            }
            let idx = last.parse::<usize>().map_err(|_| PatchError {
                message: format!("expected array index in pointer {ptr:?}, got {last:?}"),
            })?;
            if idx > a.len() {
                return Err(PatchError {
                    message: format!("array index out of bounds for add at {ptr:?}: {idx}"),
                });
            }
            a.insert(idx, value);
            Ok(())
        }
        _ => Err(PatchError {
            message: format!("add parent is not a container at {ptr:?}"),
        }),
    }
}

fn replace(doc: &mut Value, ptr: &str, value: Value) -> Result<(), PatchError> {
    if ptr.is_empty() {
        *doc = value;
        return Ok(());
    }
    let (parent, last) = get_parent_mut(doc, ptr)?;
    match parent {
        Value::Object(m) => {
            if !m.contains_key(&last) {
                return Err(PatchError {
                    message: format!("replace path not found: {ptr:?}"),
                });
            }
            m.insert(last, value);
            Ok(())
        }
        Value::Array(a) => {
            let idx = last.parse::<usize>().map_err(|_| PatchError {
                message: format!("expected array index in pointer {ptr:?}, got {last:?}"),
            })?;
            if idx >= a.len() {
                return Err(PatchError {
                    message: format!("array index out of bounds for replace at {ptr:?}: {idx}"),
                });
            }
            a[idx] = value;
            Ok(())
        }
        _ => Err(PatchError {
            message: format!("replace parent is not a container at {ptr:?}"),
        }),
    }
}

fn remove(doc: &mut Value, ptr: &str) -> Result<(), PatchError> {
    if ptr.is_empty() {
        return Err(PatchError {
            message: "cannot remove the document root".to_string(),
        });
    }
    let (parent, last) = get_parent_mut(doc, ptr)?;
    match parent {
        Value::Object(m) => {
            if m.remove(&last).is_none() {
                return Err(PatchError {
                    message: format!("remove path not found: {ptr:?}"),
                });
            }
            Ok(())
        }
        Value::Array(a) => {
            let idx = last.parse::<usize>().map_err(|_| PatchError {
                message: format!("expected array index in pointer {ptr:?}, got {last:?}"),
            })?;
            if idx >= a.len() {
                return Err(PatchError {
                    message: format!("array index out of bounds for remove at {ptr:?}: {idx}"),
                });
            }
            a.remove(idx);
            Ok(())
        }
        _ => Err(PatchError {
            message: format!("remove parent is not a container at {ptr:?}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use x07_contracts::X07AST_SCHEMA_VERSION;

    use super::{apply_patch, PatchOp};

    #[test]
    fn applies_add_replace_remove_object_and_array() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let mut doc = json!({"a": {"b": [1, 2]}});
        let ops = vec![
            PatchOp::Add {
                path: "/a/c".to_string(),
                value: json!(3),
            },
            PatchOp::Replace {
                path: "/a/b/0".to_string(),
                value: json!(9),
            },
            PatchOp::Remove {
                path: "/a/b/1".to_string(),
            },
        ];

        apply_patch(&mut doc, &ops).expect("apply patch");
        assert_eq!(doc, json!({"a": {"b": [9], "c": 3}}));
    }

    #[test]
    fn pointer_unescaping_and_dash_append() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let mut doc = json!({"a/b": {"~": 1}, "arr": [1]});
        let ops = vec![
            PatchOp::Replace {
                path: "/a~1b/~0".to_string(),
                value: json!(7),
            },
            PatchOp::Add {
                path: "/arr/-".to_string(),
                value: json!(2),
            },
        ];

        apply_patch(&mut doc, &ops).expect("apply patch");
        assert_eq!(doc, json!({"a/b": {"~": 7}, "arr": [1, 2]}));
    }

    #[test]
    fn move_and_copy_semantics_on_arrays() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let mut doc = json!({"a": ["A", "B"]});
        apply_patch(
            &mut doc,
            &[PatchOp::Move {
                path: "/a/1".to_string(),
                from: "/a/0".to_string(),
            }],
        )
        .expect("apply move patch");
        assert_eq!(doc, json!({"a": ["B", "A"]}));

        let mut doc = json!({"a": [1]});
        apply_patch(
            &mut doc,
            &[PatchOp::Copy {
                path: "/a/1".to_string(),
                from: "/a/0".to_string(),
            }],
        )
        .expect("apply copy patch");
        assert_eq!(doc, json!({"a": [1, 1]}));
    }

    #[test]
    fn test_op_and_invalid_pointer_errors() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let mut doc = json!({"a": 1});
        let err = apply_patch(
            &mut doc,
            &[PatchOp::Test {
                path: "/a".to_string(),
                value: json!(2),
            }],
        )
        .expect_err("test op must fail");
        assert!(
            err.message.contains("test failed"),
            "unexpected error: {err}"
        );

        let mut doc = json!({"a": 1});
        let err = apply_patch(
            &mut doc,
            &[PatchOp::Remove {
                path: "a".to_string(),
            }],
        )
        .expect_err("invalid JSON Pointer must fail");
        assert!(
            err.message.contains("invalid JSON Pointer"),
            "unexpected error: {err}"
        );

        let mut doc = Value::Null;
        let err = apply_patch(
            &mut doc,
            &[PatchOp::Remove {
                path: "".to_string(),
            }],
        )
        .expect_err("removing root must fail");
        assert_eq!(err.message, "cannot remove the document root");
    }

    #[test]
    fn x07ast_roundtrip_after_patch() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let mut doc = json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": [],
            "decls": [],
            "solve": ["bytes.alloc", 0],
        });
        apply_patch(
            &mut doc,
            &[PatchOp::Replace {
                path: "/solve".to_string(),
                value: json!(["bytes.alloc", 1]),
            }],
        )
        .expect("apply patch");

        let bytes = serde_json::to_vec(&doc).expect("encode x07AST json");
        let _ = crate::x07ast::parse_x07ast_json(&bytes).expect("parse x07AST after patch");
    }
}
