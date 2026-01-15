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
