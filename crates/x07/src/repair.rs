use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use serde::Serialize;
use serde_json::Value;
use x07_worlds::WorldId;

use x07c::{diagnostics, lint, world_config, x07ast};

fn patch_op_path(op: &diagnostics::PatchOp) -> &str {
    match op {
        diagnostics::PatchOp::Add { path, .. } => path,
        diagnostics::PatchOp::Remove { path } => path,
        diagnostics::PatchOp::Replace { path, .. } => path,
        diagnostics::PatchOp::Move { path, .. } => path,
        diagnostics::PatchOp::Copy { path, .. } => path,
        diagnostics::PatchOp::Test { path, .. } => path,
    }
}

fn json_ptr_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a_it = a.trim_start_matches('/').split('/');
    let mut b_it = b.trim_start_matches('/').split('/');
    loop {
        match (a_it.next(), b_it.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(ax), Some(bx)) => {
                let ax_is_idx = !ax.is_empty() && ax.as_bytes().iter().all(|c| c.is_ascii_digit());
                let bx_is_idx = !bx.is_empty() && bx.as_bytes().iter().all(|c| c.is_ascii_digit());

                let ord = if ax_is_idx && bx_is_idx {
                    let ai = ax.parse::<u64>().unwrap_or(u64::MAX);
                    let bi = bx.parse::<u64>().unwrap_or(u64::MAX);
                    ai.cmp(&bi)
                } else {
                    ax.cmp(bx)
                };

                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[clap(rename_all = "kebab_case")]
#[serde(rename_all = "kebab-case")]
pub enum RepairMode {
    Off,
    Memory,
    Write,
}

#[derive(Debug, Clone, Args)]
pub struct RepairArgs {
    /// Auto-repair: format, lint, apply quickfixes (repeatable).
    #[arg(long, value_enum, default_value_t = RepairMode::Write)]
    pub repair: RepairMode,

    /// Maximum auto-repair iterations.
    #[arg(long, value_name = "N", default_value_t = 3)]
    pub repair_max_iters: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairSummary {
    pub mode: RepairMode,
    pub iterations: u32,
    pub applied_ops_count: usize,
    pub last_lint_ok: bool,
}

#[derive(Debug)]
pub struct RepairResult {
    pub summary: RepairSummary,
    pub formatted: String,
    pub final_report: diagnostics::Report,
}

pub fn repair_x07ast_file_doc(
    doc: &mut Value,
    world: WorldId,
    max_iters: u32,
    mode: RepairMode,
) -> Result<RepairResult> {
    let max_iters = max_iters.max(1);
    let lint_options = world_config::lint_options_for_world(world);

    let mut applied_ops_count: usize = 0;
    let mut iterations: u32 = 0;

    for _pass in 0..max_iters {
        iterations = iterations.saturating_add(1);

        let doc_bytes = serde_json::to_vec(doc)?;
        let mut file = x07ast::parse_x07ast_json(&doc_bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
        x07ast::canonicalize_x07ast_file(&mut file);
        let report = lint::lint_file(&file, lint_options);

        if report.ok {
            break;
        }

        let mut any_applied = false;
        let mut tests: Vec<(usize, diagnostics::PatchOp)> = Vec::new();
        let mut replaces: Vec<(usize, diagnostics::PatchOp)> = Vec::new();
        let mut removes: Vec<(usize, diagnostics::PatchOp)> = Vec::new();
        let mut adds: Vec<(usize, diagnostics::PatchOp)> = Vec::new();
        let mut others: Vec<(usize, diagnostics::PatchOp)> = Vec::new();

        let mut op_idx: usize = 0;
        for d in &report.diagnostics {
            let Some(q) = &d.quickfix else { continue };
            if q.kind != diagnostics::QuickfixKind::JsonPatch {
                continue;
            }
            any_applied = true;
            applied_ops_count = applied_ops_count.saturating_add(q.patch.len());
            for op in &q.patch {
                match op {
                    diagnostics::PatchOp::Test { .. } => tests.push((op_idx, op.clone())),
                    diagnostics::PatchOp::Replace { .. } => replaces.push((op_idx, op.clone())),
                    diagnostics::PatchOp::Remove { .. } => removes.push((op_idx, op.clone())),
                    diagnostics::PatchOp::Add { .. } => adds.push((op_idx, op.clone())),
                    diagnostics::PatchOp::Move { .. } | diagnostics::PatchOp::Copy { .. } => {
                        others.push((op_idx, op.clone()));
                    }
                }
                op_idx = op_idx.saturating_add(1);
            }
        }
        if !any_applied {
            break;
        }

        tests.sort_by(|(ia, a), (ib, b)| {
            ia.cmp(ib)
                .then_with(|| patch_op_path(a).cmp(patch_op_path(b)))
        });
        replaces.sort_by(|(ia, a), (ib, b)| {
            json_ptr_cmp(patch_op_path(b), patch_op_path(a))
                .then_with(|| ia.cmp(ib))
                .then_with(|| patch_op_path(a).cmp(patch_op_path(b)))
        });
        removes.sort_by(|(ia, a), (ib, b)| {
            json_ptr_cmp(patch_op_path(b), patch_op_path(a))
                .then_with(|| ib.cmp(ia))
                .then_with(|| patch_op_path(a).cmp(patch_op_path(b)))
        });
        adds.sort_by(|(ia, a), (ib, b)| {
            json_ptr_cmp(patch_op_path(b), patch_op_path(a))
                .then_with(|| ib.cmp(ia))
                .then_with(|| patch_op_path(a).cmp(patch_op_path(b)))
        });
        others.sort_by(|(ia, a), (ib, b)| {
            ia.cmp(ib)
                .then_with(|| patch_op_path(a).cmp(patch_op_path(b)))
        });

        let mut patch: Vec<diagnostics::PatchOp> = Vec::new();
        patch.extend(tests.into_iter().map(|(_, op)| op));
        patch.extend(replaces.into_iter().map(|(_, op)| op));
        patch.extend(others.into_iter().map(|(_, op)| op));
        patch.extend(removes.into_iter().map(|(_, op)| op));
        patch.extend(adds.into_iter().map(|(_, op)| op));

        x07c::json_patch::apply_patch(doc, &patch)
            .map_err(|e| anyhow::anyhow!("apply patch failed: {e}"))?;
    }

    let doc_bytes = serde_json::to_vec(doc)?;
    let mut file = x07ast::parse_x07ast_json(&doc_bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
    x07ast::canonicalize_x07ast_file(&mut file);
    let final_report = lint::lint_file(&file, lint_options);

    let mut v = x07ast::x07ast_file_to_value(&file);
    x07ast::canon_value_jcs(&mut v);
    let formatted = serde_json::to_string(&v)? + "\n";

    Ok(RepairResult {
        summary: RepairSummary {
            mode,
            iterations,
            applied_ops_count,
            last_lint_ok: final_report.ok,
        },
        formatted,
        final_report,
    })
}

pub fn maybe_repair_x07ast_file(
    path: &Path,
    world: WorldId,
    args: &RepairArgs,
) -> Result<Option<RepairResult>> {
    if args.repair == RepairMode::Off {
        return Ok(None);
    }

    let bytes = std::fs::read(path).with_context(|| format!("read x07AST: {}", path.display()))?;
    let mut doc: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("parse x07AST JSON: {}", path.display()))?;

    let result = repair_x07ast_file_doc(&mut doc, world, args.repair_max_iters, args.repair)
        .context("repair")?;

    if args.repair == RepairMode::Write && bytes != result.formatted.as_bytes() {
        std::fs::write(path, result.formatted.as_bytes())
            .with_context(|| format!("write repaired x07AST: {}", path.display()))?;
    }

    Ok(Some(result))
}
