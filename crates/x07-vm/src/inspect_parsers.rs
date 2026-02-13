use std::collections::BTreeMap;

use serde_json::Value;

use crate::{X07_LABEL_SCHEMA_KEY, X07_LABEL_SCHEMA_VALUE};

pub type Labels = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedContainer {
    pub id: String,
    pub labels: Labels,
    pub status: Option<String>,
    pub primary_ipv4_cidr: Option<String>,
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
}

impl ParseError {
    fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<serde_json::Error> for ParseError {
    fn from(e: serde_json::Error) -> Self {
        ParseError::new(format!("invalid JSON: {e}"))
    }
}

#[derive(Clone, Copy)]
struct Seg(&'static [&'static str]);

const fn seg(keys: &'static [&'static str]) -> Seg {
    Seg(keys)
}

fn get_path<'a>(root: &'a Value, path: &[Seg]) -> Option<&'a Value> {
    let mut cur = root;
    for Seg(keys) in path {
        let obj = cur.as_object()?;
        let mut next: Option<&Value> = None;
        for k in *keys {
            if let Some(v) = obj.get(*k) {
                next = Some(v);
                break;
            }
        }
        cur = next?;
    }
    Some(cur)
}

fn scalar_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn parse_labels_object(v: &Value) -> Result<Labels, ParseError> {
    let obj = v
        .as_object()
        .ok_or_else(|| ParseError::new("labels field exists but is not a JSON object"))?;

    let mut out: Labels = BTreeMap::new();
    for (k, vv) in obj {
        let sv = scalar_to_string(vv).ok_or_else(|| {
            ParseError::new(format!(
                "label value for key {k:?} must be scalar (string/number/bool)"
            ))
        })?;
        out.insert(k.clone(), sv);
    }
    Ok(out)
}

pub fn is_owned_by_x07(labels: &Labels) -> bool {
    matches!(
        labels.get(X07_LABEL_SCHEMA_KEY).map(|s| s.as_str()),
        Some(X07_LABEL_SCHEMA_VALUE)
    )
}

pub fn parse_apple_container_json_owned(input: &str) -> Result<Vec<OwnedContainer>, ParseError> {
    let root: Value = serde_json::from_str(input)?;

    let entries: Vec<Value> = match root {
        Value::Array(a) => a,
        Value::Object(_) => vec![root],
        other => {
            return Err(ParseError::new(format!(
                "apple: expected array/object, got {}",
                json_type_name(&other)
            )))
        }
    };

    let mut out: Vec<OwnedContainer> = Vec::new();
    for e in entries {
        let id_val = get_path(
            &e,
            &[seg(&["configuration", "Configuration"]), seg(&["id", "ID"])],
        )
        .or_else(|| get_path(&e, &[seg(&["id", "ID"])]));
        let Some(id_val) = id_val else {
            continue;
        };
        let Some(id) = scalar_to_string(id_val) else {
            continue;
        };

        let status = get_path(&e, &[seg(&["status", "Status", "state", "State"])])
            .and_then(scalar_to_string);

        let primary_ipv4_cidr = get_path(&e, &[seg(&["networks", "Networks"])])
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_object())
            .and_then(|o| {
                o.get("address")
                    .or_else(|| o.get("ipv4Address"))
                    .and_then(scalar_to_string)
            });

        let labels_val = get_path(
            &e,
            &[
                seg(&["configuration", "Configuration"]),
                seg(&["labels", "Labels"]),
            ],
        )
        .or_else(|| get_path(&e, &[seg(&["labels", "Labels"])]));

        let labels: Labels = match labels_val {
            Some(Value::Object(_)) => parse_labels_object(labels_val.unwrap())?,
            Some(other) => {
                return Err(ParseError::new(format!(
                    "apple: labels exists but is {}/not object",
                    json_type_name(other)
                )))
            }
            None => Labels::new(),
        };

        if !is_owned_by_x07(&labels) {
            continue;
        }

        out.push(OwnedContainer {
            id,
            labels,
            status,
            primary_ipv4_cidr,
        });
    }
    Ok(out)
}

pub fn parse_ctr_container_info_json_owned(
    input: &str,
) -> Result<Option<OwnedContainer>, ParseError> {
    let root: Value = serde_json::from_str(input)?;
    let id_val = get_path(&root, &[seg(&["id", "ID"])]);
    let Some(id_val) = id_val else {
        return Ok(None);
    };
    let Some(id) = scalar_to_string(id_val) else {
        return Ok(None);
    };

    let labels_val = get_path(&root, &[seg(&["labels", "Labels"])]).or_else(|| {
        get_path(
            &root,
            &[seg(&["spec", "Spec"]), seg(&["annotations", "Annotations"])],
        )
    });

    let labels: Labels = match labels_val {
        Some(Value::Object(_)) => parse_labels_object(labels_val.unwrap())?,
        Some(other) => {
            return Err(ParseError::new(format!(
                "ctr: labels/annotations exists but is {}/not object",
                json_type_name(other)
            )))
        }
        None => Labels::new(),
    };

    if !is_owned_by_x07(&labels) {
        return Ok(None);
    }

    Ok(Some(OwnedContainer {
        id,
        labels,
        status: None,
        primary_ipv4_cidr: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_owned_happy_path() {
        let input = r#"
        [
          {
            "status": "running",
            "networks": [ { "address": "192.168.64.3/24" } ],
            "configuration": {
              "id": "abc",
              "labels": { "io.x07.schema": "1", "io.x07.job_id": "J1" }
            }
          },
          {
            "status": "running",
            "configuration": { "id": "not-owned", "labels": { "other": "x" } }
          }
        ]
        "#;

        let owned = parse_apple_container_json_owned(input).unwrap();
        assert_eq!(owned.len(), 1);
        assert_eq!(owned[0].id, "abc");
        assert_eq!(owned[0].status.as_deref(), Some("running"));
        assert_eq!(
            owned[0].primary_ipv4_cidr.as_deref(),
            Some("192.168.64.3/24")
        );
    }

    #[test]
    fn apple_network_ipv4address_synonym() {
        let input = r#"
        [
          {
            "configuration": {
              "id": "abc",
              "labels": { "io.x07.schema": "1" }
            },
            "networks": [ { "ipv4Address": "192.168.65.10/24" } ]
          }
        ]
        "#;

        let owned = parse_apple_container_json_owned(input).unwrap();
        assert_eq!(owned.len(), 1);
        assert_eq!(
            owned[0].primary_ipv4_cidr.as_deref(),
            Some("192.168.65.10/24")
        );
    }

    #[test]
    fn ctr_owned_from_labels() {
        let input = r#"
        {
          "id": "c1",
          "labels": { "io.x07.schema": "1", "io.x07.job_id": "J2" }
        }
        "#;

        let owned = parse_ctr_container_info_json_owned(input).unwrap().unwrap();
        assert_eq!(owned.id, "c1");
        assert_eq!(
            owned.labels.get("io.x07.job_id").map(|s| s.as_str()),
            Some("J2")
        );
    }

    #[test]
    fn ctr_owned_from_spec_annotations_fallback() {
        let input = r#"
        {
          "ID": "c2",
          "Spec": {
            "Annotations": { "io.x07.schema": "1" }
          }
        }
        "#;

        let owned = parse_ctr_container_info_json_owned(input).unwrap().unwrap();
        assert_eq!(owned.id, "c2");
        assert!(is_owned_by_x07(&owned.labels));
    }

    #[test]
    fn ctr_not_owned() {
        let input = r#"{ "id": "c3", "labels": { "foo": "bar" } }"#;
        assert!(parse_ctr_container_info_json_owned(input)
            .unwrap()
            .is_none());
    }
}
