use sha2::{Digest, Sha256};

use crate::ast::Expr;
use crate::x07ast::{canon_value_jcs, expr_to_value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractClauseKind {
    Requires,
    Ensures,
    Invariant,
}

impl ContractClauseKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ContractClauseKind::Requires => "requires",
            ContractClauseKind::Ensures => "ensures",
            ContractClauseKind::Invariant => "invariant",
        }
    }
}

pub fn clause_id_or_hash(
    fn_name: &str,
    kind: ContractClauseKind,
    clause_index: usize,
    expr: &Expr,
    explicit_id: Option<&str>,
) -> String {
    let explicit = explicit_id.unwrap_or("").trim();
    if !explicit.is_empty() {
        return explicit.to_string();
    }

    let mut expr_value = expr_to_value(expr);
    canon_value_jcs(&mut expr_value);
    let expr_bytes = serde_json::to_vec(&expr_value).expect("serialize canonical contract expr");

    let mut h = Sha256::new();
    h.update(b"x07.contract.id.v1\0");
    h.update(fn_name.as_bytes());
    h.update([0]);
    h.update(kind.as_str().as_bytes());
    h.update([0]);
    h.update((clause_index as u64).to_le_bytes());
    h.update(expr_bytes);
    let digest = h.finalize();

    let mut out = String::from("c_");
    for b in digest.iter().take(8) {
        out.push_str(&format!("{b:02x}"));
    }
    out
}
