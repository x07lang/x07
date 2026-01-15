use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Phase {
    Parse,
    Validate,
    Lower,
    Emit,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DiagnosticCode {
    X7I0001ParseError,
    X7I0100UnsupportedItem,
    X7I0110UnsupportedFnSig,
    X7I0111UnsupportedParamPattern,
    X7I0120UnsupportedType,
    X7I0122UnsupportedRefType,
    X7I0123UnsupportedMutRef,
    X7I0200UnsupportedLetPattern,
    X7I0201MissingLetInit,
    X7I0210UnsupportedStmtItem,
    X7I0211UnsupportedStmtMacro,
    X7I0300UnsupportedLiteral,
    X7I0301IntOutOfRange,
    X7I0310UnsupportedPath,
    X7I0311UnknownName,
    X7I0320UnsupportedCallee,
    X7I0330UnsupportedMethod,
    X7I0340UnsupportedBinOp,
    X7I0350IfBranchTypeMismatch,
    X7I0360UnsupportedForIter,
    X7I0901InternalBug,
}

impl DiagnosticCode {
    pub fn code_str(self) -> &'static str {
        match self {
            DiagnosticCode::X7I0001ParseError => "X7I0001",
            DiagnosticCode::X7I0100UnsupportedItem => "X7I0100",
            DiagnosticCode::X7I0110UnsupportedFnSig => "X7I0110",
            DiagnosticCode::X7I0111UnsupportedParamPattern => "X7I0111",
            DiagnosticCode::X7I0120UnsupportedType => "X7I0120",
            DiagnosticCode::X7I0122UnsupportedRefType => "X7I0122",
            DiagnosticCode::X7I0123UnsupportedMutRef => "X7I0123",
            DiagnosticCode::X7I0200UnsupportedLetPattern => "X7I0200",
            DiagnosticCode::X7I0201MissingLetInit => "X7I0201",
            DiagnosticCode::X7I0210UnsupportedStmtItem => "X7I0210",
            DiagnosticCode::X7I0211UnsupportedStmtMacro => "X7I0211",
            DiagnosticCode::X7I0300UnsupportedLiteral => "X7I0300",
            DiagnosticCode::X7I0301IntOutOfRange => "X7I0301",
            DiagnosticCode::X7I0310UnsupportedPath => "X7I0310",
            DiagnosticCode::X7I0311UnknownName => "X7I0311",
            DiagnosticCode::X7I0320UnsupportedCallee => "X7I0320",
            DiagnosticCode::X7I0330UnsupportedMethod => "X7I0330",
            DiagnosticCode::X7I0340UnsupportedBinOp => "X7I0340",
            DiagnosticCode::X7I0350IfBranchTypeMismatch => "X7I0350",
            DiagnosticCode::X7I0360UnsupportedForIter => "X7I0360",
            DiagnosticCode::X7I0901InternalBug => "X7I0901",
        }
    }

    pub fn default_message(self) -> &'static str {
        match self {
            DiagnosticCode::X7I0001ParseError => "failed to parse source file",
            DiagnosticCode::X7I0100UnsupportedItem => "unsupported top-level item",
            DiagnosticCode::X7I0110UnsupportedFnSig => "unsupported function signature",
            DiagnosticCode::X7I0111UnsupportedParamPattern => "unsupported parameter pattern",
            DiagnosticCode::X7I0120UnsupportedType => "unsupported type",
            DiagnosticCode::X7I0122UnsupportedRefType => "unsupported reference type",
            DiagnosticCode::X7I0123UnsupportedMutRef => "mutable references are not supported",
            DiagnosticCode::X7I0200UnsupportedLetPattern => "unsupported let pattern",
            DiagnosticCode::X7I0201MissingLetInit => "let binding is missing initializer",
            DiagnosticCode::X7I0210UnsupportedStmtItem => "statement items are not supported",
            DiagnosticCode::X7I0211UnsupportedStmtMacro => "statement macros are not supported",
            DiagnosticCode::X7I0300UnsupportedLiteral => "unsupported literal",
            DiagnosticCode::X7I0301IntOutOfRange => "integer literal out of range",
            DiagnosticCode::X7I0310UnsupportedPath => "unsupported path",
            DiagnosticCode::X7I0311UnknownName => "unknown name",
            DiagnosticCode::X7I0320UnsupportedCallee => "unsupported call target",
            DiagnosticCode::X7I0330UnsupportedMethod => "unsupported method call",
            DiagnosticCode::X7I0340UnsupportedBinOp => "unsupported binary operator",
            DiagnosticCode::X7I0350IfBranchTypeMismatch => "if branches have mismatched types",
            DiagnosticCode::X7I0360UnsupportedForIter => "unsupported for-loop iterator",
            DiagnosticCode::X7I0901InternalBug => "internal x07import bug",
        }
    }

    pub fn default_help(self) -> Option<&'static str> {
        match self {
            DiagnosticCode::X7I0001ParseError => Some(
                "Ensure the file parses as Rust/C and contains only supported items for x07import v1.",
            ),
            DiagnosticCode::X7I0123UnsupportedMutRef => Some("Rewrite the code to avoid &mut."),
            DiagnosticCode::X7I0901InternalBug => Some(
                "This is a bug in x07import. Please report it with the input source file.",
            ),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub phase: Phase,
    pub severity: Severity,
    pub message: String,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: DiagnosticCode, phase: Phase, message: impl Into<String>) -> Self {
        Diagnostic {
            code,
            phase,
            severity: Severity::Error,
            message: message.into(),
            help: code.default_help().map(|s| s.to_string()),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {:?} {:?}: {}",
            self.code.code_str(),
            self.phase,
            self.severity,
            self.message
        )?;
        if let Some(help) = &self.help {
            write!(f, "\n  help: {help}")?;
        }
        Ok(())
    }
}

pub fn render_diagnostics_md() -> String {
    let mut rows: Vec<(String, Phase, Severity, String, String)> = Vec::new();
    for code in all_codes() {
        let code_str = code.code_str().to_string();
        let phase = match code {
            DiagnosticCode::X7I0001ParseError => Phase::Parse,
            DiagnosticCode::X7I0901InternalBug => Phase::Internal,
            _ => Phase::Validate,
        };
        let sev = match code {
            DiagnosticCode::X7I0901InternalBug => Severity::Error,
            _ => Severity::Error,
        };
        rows.push((
            code_str,
            phase,
            sev,
            code.default_message().to_string(),
            code.default_help().unwrap_or("").to_string(),
        ));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    out.push_str("# x07import diagnostics catalog\n\n");
    out.push_str("This document is generated from `crates/x07import-core/src/diagnostics.rs`.\n\n");
    out.push_str("| Code | Phase | Severity | Message | Help |\n");
    out.push_str("| ---- | ----- | -------- | ------- | ---- |\n");
    for (code, phase, sev, msg, help) in rows {
        out.push_str(&format!(
            "| {code} | {phase:?} | {sev:?} | {msg} | {help} |\n"
        ));
    }
    out
}

fn all_codes() -> &'static [DiagnosticCode] {
    &[
        DiagnosticCode::X7I0001ParseError,
        DiagnosticCode::X7I0100UnsupportedItem,
        DiagnosticCode::X7I0110UnsupportedFnSig,
        DiagnosticCode::X7I0111UnsupportedParamPattern,
        DiagnosticCode::X7I0120UnsupportedType,
        DiagnosticCode::X7I0122UnsupportedRefType,
        DiagnosticCode::X7I0123UnsupportedMutRef,
        DiagnosticCode::X7I0200UnsupportedLetPattern,
        DiagnosticCode::X7I0201MissingLetInit,
        DiagnosticCode::X7I0210UnsupportedStmtItem,
        DiagnosticCode::X7I0211UnsupportedStmtMacro,
        DiagnosticCode::X7I0300UnsupportedLiteral,
        DiagnosticCode::X7I0301IntOutOfRange,
        DiagnosticCode::X7I0310UnsupportedPath,
        DiagnosticCode::X7I0311UnknownName,
        DiagnosticCode::X7I0320UnsupportedCallee,
        DiagnosticCode::X7I0330UnsupportedMethod,
        DiagnosticCode::X7I0340UnsupportedBinOp,
        DiagnosticCode::X7I0350IfBranchTypeMismatch,
        DiagnosticCode::X7I0360UnsupportedForIter,
        DiagnosticCode::X7I0901InternalBug,
    ]
}
