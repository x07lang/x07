use std::collections::BTreeSet;

use serde::Serialize;

use crate::diagnostics::Diagnostic;

pub(crate) const MEM_PROVENANCE_SCHEMA_VERSION: &str = "x07.mem.provenance_graph@0.1.0";
const MAX_NODES: usize = 128;
const MAX_EDGES: usize = 256;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Focus {
    pub(crate) ptr: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NodeRole {
    Owner,
    Temporary,
    Borrow,
    Move,
    Use,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Node {
    pub(crate) id: String,
    pub(crate) role: NodeRole,
    pub(crate) ptr: String,
    pub(crate) label: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EdgeKind {
    BorrowedFrom,
    MovedTo,
    UsedAfterMove,
    BorrowConflict,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Edge {
    pub(crate) kind: EdgeKind,
    pub(crate) from: String,
    pub(crate) to: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ViolationKind {
    BorrowFromTemporary,
    UseAfterMove,
    BorrowConflict,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Violation {
    pub(crate) kind: ViolationKind,
    pub(crate) node: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HintKind {
    RepairPattern,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Hint {
    pub(crate) kind: HintKind,
    pub(crate) id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct Truncation {
    pub(crate) max_nodes: u64,
    pub(crate) max_edges: u64,
    pub(crate) dropped_nodes: u64,
    pub(crate) dropped_edges: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(crate) struct MemProvenanceGraph {
    pub(crate) schema_version: &'static str,
    pub(crate) focus: Focus,
    pub(crate) truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) truncation: Option<Truncation>,
    pub(crate) nodes: Vec<Node>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) violation: Violation,
    pub(crate) hints: Vec<Hint>,
}

impl MemProvenanceGraph {
    pub(crate) fn new(focus_ptr: String, violation: Violation) -> Self {
        Self {
            schema_version: MEM_PROVENANCE_SCHEMA_VERSION,
            focus: Focus { ptr: focus_ptr },
            truncated: false,
            truncation: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            violation,
            hints: Vec::new(),
        }
    }
}

pub(crate) fn attach_mem_provenance(diag: &mut Diagnostic, mut graph: MemProvenanceGraph) {
    if !graph.nodes.iter().any(|n| n.id == graph.violation.node) {
        return;
    }

    let before_nodes = graph.nodes.len();
    let before_edges = graph.edges.len();

    let mut pinned: Vec<String> = Vec::new();
    let mut pinned_seen: BTreeSet<String> = BTreeSet::new();
    let mut push_pinned = |id: String| {
        if pinned_seen.insert(id.clone()) {
            pinned.push(id);
        }
    };

    push_pinned(graph.violation.node.clone());
    let focus_ptr = graph.focus.ptr.clone();
    for node in &graph.nodes {
        if node.ptr == focus_ptr {
            push_pinned(node.id.clone());
        }
    }

    let mut keep_ids: Vec<String> = Vec::new();
    let mut keep_seen: BTreeSet<String> = BTreeSet::new();
    for id in pinned {
        if keep_seen.insert(id.clone()) {
            keep_ids.push(id);
        }
    }
    for node in &graph.nodes {
        if keep_seen.insert(node.id.clone()) {
            keep_ids.push(node.id.clone());
        }
    }
    if keep_ids.len() > MAX_NODES {
        keep_ids.truncate(MAX_NODES);
    }

    let keep_set: BTreeSet<String> = keep_ids.iter().cloned().collect();

    let mut new_nodes: Vec<Node> = Vec::with_capacity(keep_ids.len());
    for id in &keep_ids {
        if let Some(node) = graph.nodes.iter().find(|n| &n.id == id) {
            new_nodes.push(node.clone());
        }
    }
    graph.nodes = new_nodes;

    let mut new_edges: Vec<Edge> = Vec::new();
    for edge in &graph.edges {
        if !keep_set.contains(&edge.from) || !keep_set.contains(&edge.to) {
            continue;
        }
        if new_edges.len() >= MAX_EDGES {
            break;
        }
        new_edges.push(edge.clone());
    }
    graph.edges = new_edges;

    let after_nodes = graph.nodes.len();
    let after_edges = graph.edges.len();
    let dropped_nodes = before_nodes.saturating_sub(after_nodes) as u64;
    let dropped_edges = before_edges.saturating_sub(after_edges) as u64;

    if dropped_nodes > 0 || dropped_edges > 0 {
        graph.truncated = true;
        graph.truncation = Some(Truncation {
            max_nodes: MAX_NODES as u64,
            max_edges: MAX_EDGES as u64,
            dropped_nodes,
            dropped_edges,
        });
        diag.notes.push(format!(
            "Provenance graph truncated (max_nodes={}, max_edges={}).",
            MAX_NODES, MAX_EDGES
        ));
    }

    let value = match serde_json::to_value(&graph) {
        Ok(v) => v,
        Err(_) => return,
    };
    diag.data.insert("mem_provenance".to_string(), value);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::diagnostics::{Diagnostic, Severity, Stage};

    use super::{
        attach_mem_provenance, Edge, EdgeKind, Hint, HintKind, MemProvenanceGraph, Node, NodeRole,
        Violation, ViolationKind, MEM_PROVENANCE_SCHEMA_VERSION,
    };

    #[test]
    fn attach_mem_provenance_truncates_and_preserves_violation_node() {
        let mut diag = Diagnostic {
            code: "X07-MOVE-0001".to_string(),
            severity: Severity::Error,
            stage: Stage::Lint,
            message: "x".to_string(),
            loc: None,
            notes: Vec::new(),
            related: Vec::new(),
            data: BTreeMap::new(),
            quickfix: None,
        };

        let violation_node = "n199".to_string();
        let focus_ptr = "/focus".to_string();
        let mut graph = MemProvenanceGraph::new(
            focus_ptr.clone(),
            Violation {
                kind: ViolationKind::UseAfterMove,
                node: violation_node.clone(),
            },
        );

        for i in 0..200 {
            let id = format!("n{i}");
            let ptr = if i == 150 {
                focus_ptr.clone()
            } else {
                "/x".to_string()
            };
            graph.nodes.push(Node {
                id: id.clone(),
                role: NodeRole::Use,
                ptr,
                label: "node".to_string(),
            });
            graph.edges.push(Edge {
                kind: EdgeKind::MovedTo,
                from: id,
                to: violation_node.clone(),
            });
        }
        graph.hints.push(Hint {
            kind: HintKind::RepairPattern,
            id: "clone_before_use".to_string(),
        });

        attach_mem_provenance(&mut diag, graph);

        let mp = diag
            .data
            .get("mem_provenance")
            .expect("mem_provenance inserted");
        let mp_obj = mp.as_object().expect("mem_provenance must be object");

        assert_eq!(
            mp_obj.get("schema_version").and_then(|v| v.as_str()),
            Some(MEM_PROVENANCE_SCHEMA_VERSION)
        );
        assert_eq!(
            mp_obj.get("truncated").and_then(|v| v.as_bool()),
            Some(true)
        );

        let trunc = mp_obj
            .get("truncation")
            .and_then(|v| v.as_object())
            .expect("truncation object");
        assert_eq!(trunc.get("max_nodes").and_then(|v| v.as_u64()), Some(128));
        assert_eq!(trunc.get("max_edges").and_then(|v| v.as_u64()), Some(256));
        assert_eq!(
            trunc.get("dropped_nodes").and_then(|v| v.as_u64()),
            Some(72)
        );
        assert_eq!(
            trunc.get("dropped_edges").and_then(|v| v.as_u64()),
            Some(72)
        );

        let nodes = mp_obj
            .get("nodes")
            .and_then(|v| v.as_array())
            .expect("nodes array");
        assert!(
            nodes
                .iter()
                .any(|n| n.get("id").and_then(|v| v.as_str()) == Some("n199")),
            "violation node must be present"
        );

        let violation = mp_obj
            .get("violation")
            .and_then(|v| v.as_object())
            .expect("violation object");
        assert_eq!(violation.get("node").and_then(|v| v.as_str()), Some("n199"));

        assert!(
            diag.notes
                .iter()
                .any(|n| n.contains("Provenance graph truncated")),
            "expected truncation note"
        );
    }
}
