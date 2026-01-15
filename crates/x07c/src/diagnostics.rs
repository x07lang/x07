use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use x07_contracts::X07DIAG_SCHEMA_VERSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Parse,
    Lint,
    Rewrite,
    Type,
    Lower,
    Codegen,
    Link,
    Run,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Location {
    X07Ast {
        ptr: String,
    },
    Text {
        span: Span,
        #[serde(skip_serializing_if = "Option::is_none")]
        snippet: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: Position,
    pub end: Position,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub col: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quickfix {
    pub kind: QuickfixKind,
    pub patch: Vec<PatchOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickfixKind {
    JsonPatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum PatchOp {
    #[serde(rename = "add")]
    Add { path: String, value: Value },
    #[serde(rename = "remove")]
    Remove { path: String },
    #[serde(rename = "replace")]
    Replace { path: String, value: Value },
    #[serde(rename = "move")]
    Move { path: String, from: String },
    #[serde(rename = "copy")]
    Copy { path: String, from: String },
    #[serde(rename = "test")]
    Test { path: String, value: Value },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub stage: Stage,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loc: Option<Location>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<Location>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub data: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quickfix: Option<Quickfix>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub schema_version: String,
    pub ok: bool,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, Value>,
}

impl Report {
    pub fn ok() -> Self {
        Self {
            schema_version: X07DIAG_SCHEMA_VERSION.to_string(),
            ok: true,
            diagnostics: Vec::new(),
            meta: BTreeMap::new(),
        }
    }

    pub fn with_diagnostics(mut self, mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort_by(|a, b| {
            let ap = a
                .loc
                .as_ref()
                .and_then(|l| match l {
                    Location::X07Ast { ptr } => Some(ptr.as_str()),
                    Location::Text { .. } => None,
                })
                .unwrap_or("");
            let bp = b
                .loc
                .as_ref()
                .and_then(|l| match l {
                    Location::X07Ast { ptr } => Some(ptr.as_str()),
                    Location::Text { .. } => None,
                })
                .unwrap_or("");
            ap.cmp(bp)
                .then_with(|| a.code.cmp(&b.code))
                .then_with(|| a.message.cmp(&b.message))
        });
        self.ok = diagnostics.iter().all(|d| d.severity != Severity::Error);
        self.diagnostics = diagnostics;
        self
    }
}
