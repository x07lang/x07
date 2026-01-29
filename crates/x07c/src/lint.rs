use crate::ast::Expr;
use crate::diagnostics::{
    Diagnostic, Location, PatchOp, Quickfix, QuickfixKind, Report, Severity, Stage,
};
use crate::x07ast::{self, X07AstFile, X07AstKind};

fn expr_ident(name: impl Into<String>) -> Expr {
    Expr::Ident {
        name: name.into(),
        ptr: String::new(),
    }
}

fn expr_list(items: Vec<Expr>) -> Expr {
    Expr::List {
        items,
        ptr: String::new(),
    }
}

#[derive(Debug, Clone)]
struct BeginStmtCtx {
    begin_ptr: String,
    stmt_index: usize,
    stmt_root_ptr: String,
}

#[derive(Debug, Clone)]
struct LintCtx {
    begin_stmt: Option<BeginStmtCtx>,
    hoist_safe: bool,
}

impl Default for LintCtx {
    fn default() -> Self {
        Self {
            begin_stmt: None,
            hoist_safe: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LintOptions {
    pub world: x07_worlds::WorldId,
    pub enable_fs: bool,
    pub enable_rr: bool,
    pub enable_kv: bool,
    pub allow_unsafe: Option<bool>,
    pub allow_ffi: Option<bool>,
}

impl LintOptions {
    pub fn allow_unsafe(&self) -> bool {
        self.allow_unsafe
            .unwrap_or_else(|| self.world.caps().allow_unsafe)
    }

    pub fn allow_ffi(&self) -> bool {
        self.allow_ffi
            .unwrap_or_else(|| self.world.caps().allow_ffi)
    }
}

pub fn lint_file(file: &X07AstFile, options: LintOptions) -> Report {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    match file.kind {
        X07AstKind::Entry => {
            if file.solve.is_none() {
                diagnostics.push(Diagnostic {
                    code: "X07-AST-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Parse,
                    message: "entry file must contain /solve".to_string(),
                    loc: Some(Location::X07Ast {
                        ptr: "/solve".to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                });
            }
        }
        X07AstKind::Module => {}
    }

    lint_world_imports(file, options, &mut diagnostics);
    lint_world_decls(file, options, &mut diagnostics);

    if let Some(solve) = &file.solve {
        lint_expr(
            solve,
            "/solve",
            options,
            &LintCtx::default(),
            &mut diagnostics,
        );
    }

    let export_slots = if file.kind == X07AstKind::Module && !file.exports.is_empty() {
        1usize
    } else {
        0usize
    };
    let extern_slots = file.extern_functions.len();
    let defn_base = export_slots + extern_slots;

    for (idx, f) in file.functions.iter().enumerate() {
        let ptr = format!("/decls/{}/body", defn_base + idx);
        lint_expr(
            &f.body,
            &ptr,
            options,
            &LintCtx::default(),
            &mut diagnostics,
        );
    }
    for (idx, f) in file.async_functions.iter().enumerate() {
        let ptr = format!("/decls/{}/body", defn_base + file.functions.len() + idx);
        lint_expr(
            &f.body,
            &ptr,
            options,
            &LintCtx::default(),
            &mut diagnostics,
        );
    }

    Report::ok().with_diagnostics(diagnostics)
}

fn lint_world_decls(file: &X07AstFile, options: LintOptions, diagnostics: &mut Vec<Diagnostic>) {
    let export_slots = if file.kind == X07AstKind::Module && !file.exports.is_empty() {
        1usize
    } else {
        0usize
    };

    if !options.allow_ffi() {
        for (idx, f) in file.extern_functions.iter().enumerate() {
            let ptr = format!("/decls/{}/name", export_slots + idx);
            diagnostics.push(Diagnostic {
                code: "X07-WORLD-FFI-0001".to_string(),
                severity: Severity::Error,
                stage: Stage::Lint,
                message: format!(
                    "ffi capability is not enabled in this world: extern decl {}",
                    f.name
                ),
                loc: Some(Location::X07Ast { ptr }),
                notes: vec![
                    "Compile with --world run-os or --world run-os-sandboxed for extern C interop."
                        .to_string(),
                ],
                related: Vec::new(),
                data: Default::default(),
                quickfix: None,
            });
        }
    }

    if !options.allow_unsafe() {
        let mut check_defn_like = |base: usize,
                                   idx: usize,
                                   name: &str,
                                   params: &[crate::program::FunctionParam],
                                   ret_ty: crate::types::Ty| {
            for (pidx, p) in params.iter().enumerate() {
                if p.ty.is_ptr_ty() {
                    diagnostics.push(Diagnostic {
                        code: "X07-WORLD-UNSAFE-0002".to_string(),
                        severity: Severity::Error,
                        stage: Stage::Lint,
                        message: format!("unsafe capability is not enabled in this world: raw pointer type in signature of {name}"),
                        loc: Some(Location::X07Ast {
                            ptr: format!("/decls/{}/params/{}/ty", base + idx, pidx),
                        }),
                        notes: vec![
                            "Compile with --world run-os or --world run-os-sandboxed for raw pointers."
                                .to_string(),
                        ],
                        related: Vec::new(),
                        data: Default::default(),
                        quickfix: None,
                    });
                }
            }
            if ret_ty.is_ptr_ty() {
                diagnostics.push(Diagnostic {
                    code: "X07-WORLD-UNSAFE-0002".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Lint,
                    message: format!("unsafe capability is not enabled in this world: raw pointer type in signature of {name}"),
                    loc: Some(Location::X07Ast {
                        ptr: format!("/decls/{}/result", base + idx),
                    }),
                    notes: vec![
                        "Compile with --world run-os or --world run-os-sandboxed for raw pointers."
                            .to_string(),
                    ],
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                });
            }
        };

        for (idx, f) in file.extern_functions.iter().enumerate() {
            check_defn_like(export_slots, idx, &f.name, &f.params, f.ret_ty);
        }
        let defn_base = export_slots + file.extern_functions.len();
        for (idx, f) in file.functions.iter().enumerate() {
            check_defn_like(defn_base, idx, &f.name, &f.params, f.ret_ty);
        }
        for (idx, f) in file.async_functions.iter().enumerate() {
            check_defn_like(
                defn_base + file.functions.len(),
                idx,
                &f.name,
                &f.params,
                f.ret_ty,
            );
        }
    }
}

fn lint_world_imports(file: &X07AstFile, options: LintOptions, diagnostics: &mut Vec<Diagnostic>) {
    if options.world.is_eval_world() {
        let has_os = file.imports.iter().any(|m| m.starts_with("std.os."));
        if has_os {
            let allowed: Vec<String> = file
                .imports
                .iter()
                .filter(|m| !m.starts_with("std.os."))
                .cloned()
                .collect();
            diagnostics.push(Diagnostic {
                code: "X07-WORLD-OS-0001".to_string(),
                severity: Severity::Error,
                stage: Stage::Lint,
                message: "std.os.* modules are not allowed in solve-* worlds".to_string(),
                loc: Some(Location::X07Ast {
                    ptr: "/imports".to_string(),
                }),
                notes: vec![
                    "Use solve-* world adapters (std.fs/std.rr/std.kv) in evaluation.".to_string(),
                    "std.os.* is standalone-only (run-os / run-os-sandboxed).".to_string(),
                ],
                related: Vec::new(),
                data: Default::default(),
                quickfix: Some(Quickfix {
                    kind: QuickfixKind::JsonPatch,
                    patch: vec![PatchOp::Replace {
                        path: "/imports".to_string(),
                        value: serde_json::Value::Array(
                            allowed.into_iter().map(serde_json::Value::String).collect(),
                        ),
                    }],
                    note: Some("Remove std.os.* imports".to_string()),
                }),
            });
        }
    }

    let mut forbidden: Vec<&str> = Vec::new();
    if !options.enable_fs {
        forbidden.push("std.fs");
    }
    if !options.enable_rr {
        forbidden.push("std.rr");
    }
    if !options.enable_kv {
        forbidden.push("std.kv");
    }
    if forbidden.is_empty() {
        return;
    }

    let has_forbidden = forbidden.iter().any(|m| file.imports.contains(*m));
    if !has_forbidden {
        return;
    }

    let mut notes = Vec::new();
    for m in &forbidden {
        if file.imports.contains(*m) {
            notes.push(format!("forbidden import in this world: {m}"));
        }
    }

    let allowed: Vec<String> = file
        .imports
        .iter()
        .filter(|m| !forbidden.contains(&m.as_str()))
        .cloned()
        .collect();

    diagnostics.push(Diagnostic {
        code: "X07-WORLD-0001".to_string(),
        severity: Severity::Error,
        stage: Stage::Lint,
        message: "program imports modules not allowed in this world".to_string(),
        loc: Some(Location::X07Ast {
            ptr: "/imports".to_string(),
        }),
        notes,
        related: Vec::new(),
        data: Default::default(),
        quickfix: Some(Quickfix {
            kind: QuickfixKind::JsonPatch,
            patch: vec![PatchOp::Replace {
                path: "/imports".to_string(),
                value: serde_json::Value::Array(
                    allowed.into_iter().map(serde_json::Value::String).collect(),
                ),
            }],
            note: Some("Remove forbidden imports".to_string()),
        }),
    });
}

fn lint_expr(
    expr: &Expr,
    ptr: &str,
    options: LintOptions,
    ctx: &LintCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => {}
        Expr::List { items, .. } => {
            if items.is_empty() {
                diagnostics.push(Diagnostic {
                    code: "X07-ARITY-0000".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Lint,
                    message: "list expression must not be empty".to_string(),
                    loc: Some(Location::X07Ast {
                        ptr: ptr.to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                });
                return;
            }
            let head = items[0].as_ident().unwrap_or("");
            lint_core_arity(head, items, ptr, diagnostics);
            lint_core_borrow_rules(head, items, ptr, ctx, diagnostics);
            lint_core_move_rules(head, items, ptr, diagnostics);
            lint_world_heads(head, ptr, options, diagnostics);

            for (idx, item) in items.iter().enumerate() {
                let child_ptr = format!("{ptr}/{idx}");

                let mut child_ctx = ctx.clone();
                match head {
                    "if" => {
                        if idx == 2 || idx == 3 {
                            child_ctx.hoist_safe = false;
                        }
                    }
                    "for" => {
                        if idx == 4 {
                            child_ctx.hoist_safe = false;
                        }
                    }
                    _ => {}
                }

                let stmt_root_is_here = child_ctx
                    .begin_stmt
                    .as_ref()
                    .map(|s| s.stmt_root_ptr == ptr)
                    .unwrap_or(false);
                let statement_block_root = matches!(head, "begin" | "unsafe")
                    && (child_ctx.begin_stmt.is_none() || stmt_root_is_here);
                if statement_block_root && idx >= 1 {
                    child_ctx.begin_stmt = Some(BeginStmtCtx {
                        begin_ptr: ptr.to_string(),
                        stmt_index: idx,
                        stmt_root_ptr: child_ptr.clone(),
                    });
                }

                lint_expr(item, &child_ptr, options, &child_ctx, diagnostics);
            }
        }
    }
}

fn lint_core_arity(head: &str, items: &[Expr], ptr: &str, diagnostics: &mut Vec<Diagnostic>) {
    match head {
        "if" => {
            if items.len() != 4 {
                diagnostics.push(Diagnostic {
                    code: "X07-ARITY-IF-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Lint,
                    message: format!("if expects 3 args; got {}", items.len().saturating_sub(1)),
                    loc: Some(Location::X07Ast {
                        ptr: ptr.to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                });
            }
        }
        "for" => {
            if items.len() != 5 {
                let mut diag = Diagnostic {
                    code: "X07-ARITY-FOR-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Lint,
                    message: format!("for expects 4 args; got {}", items.len().saturating_sub(1)),
                    loc: Some(Location::X07Ast {
                        ptr: ptr.to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                };
                if items.len() > 5 {
                    let mut new_items: Vec<Expr> = Vec::with_capacity(5);
                    new_items.extend(items[0..4].iter().cloned());
                    let mut begin_items: Vec<Expr> =
                        Vec::with_capacity(items.len().saturating_sub(3));
                    begin_items.push(expr_ident("begin"));
                    begin_items.extend(items[4..].iter().cloned());
                    new_items.push(expr_list(begin_items));
                    diag.quickfix = Some(Quickfix {
                        kind: QuickfixKind::JsonPatch,
                        patch: vec![PatchOp::Replace {
                            path: ptr.to_string(),
                            value: x07ast::expr_to_value(&expr_list(new_items)),
                        }],
                        note: Some("Wrap extra for body expressions in begin".to_string()),
                    });
                }
                diagnostics.push(diag);
            }
        }
        "begin" => {
            if items.len() < 2 {
                diagnostics.push(Diagnostic {
                    code: "X07-ARITY-BEGIN-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Lint,
                    message: "begin expects at least 1 expression".to_string(),
                    loc: Some(Location::X07Ast {
                        ptr: ptr.to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                });
            }
        }
        "unsafe" => {
            if items.len() < 2 {
                diagnostics.push(Diagnostic {
                    code: "X07-ARITY-UNSAFE-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Lint,
                    message: "unsafe expects at least 1 expression".to_string(),
                    loc: Some(Location::X07Ast {
                        ptr: ptr.to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                });
            }
            let exprs = items.len().saturating_sub(1);
            if exprs > 16 {
                diagnostics.push(Diagnostic {
                    code: "X07-UNSAFE-0001".to_string(),
                    severity: Severity::Warning,
                    stage: Stage::Lint,
                    message: format!("unsafe block is large: {exprs} expressions"),
                    loc: Some(Location::X07Ast {
                        ptr: ptr.to_string(),
                    }),
                    notes: vec!["Try to reduce the unsafe surface area.".to_string()],
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                });
            }
        }
        "let" | "set" => {
            if items.len() != 3 {
                let mut diag = Diagnostic {
                    code: "X07-ARITY-LET-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Lint,
                    message: format!(
                        "{head} expects 2 args; got {}",
                        items.len().saturating_sub(1)
                    ),
                    loc: Some(Location::X07Ast {
                        ptr: ptr.to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                };

                if items.len() > 3 {
                    let mut begin_items: Vec<Expr> = Vec::with_capacity(items.len());
                    begin_items.push(expr_ident("begin"));
                    begin_items.push(expr_list(items[0..3].to_vec()));
                    begin_items.extend(items[3..].iter().cloned());
                    diag.quickfix = Some(Quickfix {
                        kind: QuickfixKind::JsonPatch,
                        patch: vec![PatchOp::Replace {
                            path: ptr.to_string(),
                            value: x07ast::expr_to_value(&expr_list(begin_items)),
                        }],
                        note: Some(format!("Rewrite {head} with body into begin")),
                    });
                }

                diagnostics.push(diag);
            }
        }
        "return" => {
            if items.len() != 2 {
                diagnostics.push(Diagnostic {
                    code: "X07-ARITY-RETURN-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Lint,
                    message: format!(
                        "return expects 1 arg; got {}",
                        items.len().saturating_sub(1)
                    ),
                    loc: Some(Location::X07Ast {
                        ptr: ptr.to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: Default::default(),
                    quickfix: None,
                });
            }
        }
        _ => {}
    }
}

fn borrow_tmp_name(ptr: &str) -> String {
    let mut out = String::with_capacity(ptr.len() + 32);
    out.push_str("_x07_tmp_borrow");
    for ch in ptr.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => out.push(ch),
            _ => out.push('_'),
        }
    }
    out
}

fn lint_core_borrow_rules(
    head: &str,
    items: &[Expr],
    ptr: &str,
    ctx: &LintCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(owner_ptr) = (match head {
        "bytes.view" | "vec_u8.as_view" => Some(format!("{ptr}/1")),
        "bytes.subview" => Some(format!("{ptr}/1")),
        _ => None,
    }) else {
        return;
    };

    let Some(owner) = items.get(1) else {
        return;
    };
    if matches!(owner, Expr::Ident { .. }) {
        return;
    }

    let mut notes = vec![
        "This operation borrows from an owned value. It cannot borrow from a temporary expression."
            .to_string(),
    ];
    if head == "bytes.view" || head == "bytes.subview" {
        notes.push(
            "Suggested fix: bind the bytes to a variable first, then call bytes.view/bytes.subview on that variable.".to_string(),
        );
    } else if head == "vec_u8.as_view" {
        notes.push(
            "Suggested fix: bind the vec_u8 to a variable first, then call vec_u8.as_view on that variable.".to_string(),
        );
    }

    let tmp = borrow_tmp_name(ptr);
    let mut call_items: Vec<Expr> = Vec::with_capacity(items.len());
    call_items.push(expr_ident(head.to_string()));
    call_items.push(expr_ident(tmp.to_string()));
    call_items.extend(items.iter().skip(2).cloned());

    let fixed_call = expr_list(call_items);

    let quickfix = if !ctx.hoist_safe {
        None
    } else if let Some(b) = ctx.begin_stmt.as_ref() {
        Some(Quickfix {
            kind: QuickfixKind::JsonPatch,
            patch: vec![
                PatchOp::Replace {
                    path: ptr.to_string(),
                    value: x07ast::expr_to_value(&fixed_call),
                },
                PatchOp::Add {
                    path: format!("{}/{}", b.begin_ptr, b.stmt_index),
                    value: x07ast::expr_to_value(&expr_list(vec![
                        expr_ident("let"),
                        expr_ident(tmp.to_string()),
                        owner.clone(),
                    ])),
                },
            ],
            note: Some(format!("Introduce let binding for {head} owner")),
        })
    } else if ptr == "/solve" || ptr.ends_with("/body") {
        Some(Quickfix {
            kind: QuickfixKind::JsonPatch,
            patch: vec![PatchOp::Replace {
                path: ptr.to_string(),
                value: x07ast::expr_to_value(&expr_list(vec![
                    expr_ident("begin"),
                    expr_list(vec![
                        expr_ident("let"),
                        expr_ident(tmp.to_string()),
                        owner.clone(),
                    ]),
                    fixed_call,
                ])),
            }],
            note: Some(format!("Introduce let binding for {head} owner")),
        })
    } else {
        None
    };

    diagnostics.push(Diagnostic {
        code: "X07-BORROW-0001".to_string(),
        severity: Severity::Error,
        stage: Stage::Lint,
        message: format!("{head} requires an identifier owner"),
        loc: Some(Location::X07Ast { ptr: owner_ptr }),
        notes,
        related: Vec::new(),
        data: Default::default(),
        quickfix,
    });
}

fn lint_core_move_rules(head: &str, items: &[Expr], ptr: &str, diagnostics: &mut Vec<Diagnostic>) {
    if head == "if" && items.len() == 4 {
        let cond = &items[1];
        let then_branch = &items[2];
        let else_branch = &items[3];

        fn collect_bytes_view_idents(expr: &Expr, out: &mut std::collections::BTreeSet<String>) {
            match expr {
                Expr::Int { .. } | Expr::Ident { .. } => {}
                Expr::List { items, .. } => {
                    if items.len() == 2 && items[0].as_ident() == Some("bytes.view") {
                        if let Some(name) = items[1].as_ident() {
                            out.insert(name.to_string());
                        }
                    }
                    for item in items {
                        collect_bytes_view_idents(item, out);
                    }
                }
            }
        }

        fn rewrite_bytes_view_owner(expr: &Expr, from: &str, to: &str) -> Expr {
            match expr {
                Expr::Int { .. } => expr.clone(),
                Expr::Ident { name, .. } => expr_ident(name.clone()),
                Expr::List { items, .. } => {
                    if items.len() == 2
                        && items[0].as_ident() == Some("bytes.view")
                        && items[1].as_ident() == Some(from)
                    {
                        return expr_list(vec![
                            expr_ident("bytes.view"),
                            expr_ident(to.to_string()),
                        ]);
                    }
                    let new_items: Vec<Expr> = items
                        .iter()
                        .map(|it| rewrite_bytes_view_owner(it, from, to))
                        .collect();
                    expr_list(new_items)
                }
            }
        }

        let mut cond_owners = std::collections::BTreeSet::new();
        let mut branch_owners = std::collections::BTreeSet::new();
        collect_bytes_view_idents(cond, &mut cond_owners);
        collect_bytes_view_idents(then_branch, &mut branch_owners);
        collect_bytes_view_idents(else_branch, &mut branch_owners);

        let mut duplicates: Vec<String> =
            cond_owners.intersection(&branch_owners).cloned().collect();
        duplicates.sort();

        if let Some(name) = duplicates.first() {
            let tmp = "_x07_tmp_copy";
            let cond_fixed = rewrite_bytes_view_owner(cond, name, tmp);

            let fixed = expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident(tmp.to_string()),
                    expr_list(vec![
                        expr_ident("view.to_bytes"),
                        expr_ident(name.to_string()),
                    ]),
                ]),
                expr_list(vec![
                    expr_ident("if"),
                    cond_fixed,
                    then_branch.clone(),
                    else_branch.clone(),
                ]),
            ]);

            diagnostics.push(Diagnostic {
                code: "X07-MOVE-0002".to_string(),
                severity: Severity::Error,
                stage: Stage::Lint,
                message: "if uses bytes.view of the same identifier in condition and branch"
                    .to_string(),
                loc: Some(Location::X07Ast {
                    ptr: ptr.to_string(),
                }),
                notes: vec![
                    "bytes.view borrows from an owned bytes value and is move-sensitive. Using it in both the condition and a branch can trigger a use-after-move during compilation."
                        .to_string(),
                    "Suggested fix: copy the bytes for the condition (for example via view.to_bytes) and use the copy in the condition."
                        .to_string(),
                ],
                related: Vec::new(),
                data: Default::default(),
                quickfix: Some(Quickfix {
                    kind: QuickfixKind::JsonPatch,
                    patch: vec![PatchOp::Replace {
                        path: ptr.to_string(),
                        value: x07ast::expr_to_value(&fixed),
                    }],
                    note: Some("Copy bytes for if condition".to_string()),
                }),
            });
            return;
        }
    }

    if head != "bytes.concat" || items.len() != 3 {
        return;
    }

    let Some(a) = items.get(1).and_then(Expr::as_ident) else {
        return;
    };
    let Some(b) = items.get(2).and_then(Expr::as_ident) else {
        return;
    };
    if a != b {
        return;
    }

    let mut notes = vec![
        "This operation moves owned values. Using the same identifier twice will trigger a use-after-move during compilation."
            .to_string(),
        "Suggested fix: copy one side (for example: [\"bytes.concat\", [\"view.to_bytes\", [\"bytes.view\", name]], name])."
            .to_string(),
    ];

    let fixed = expr_list(vec![
        expr_ident("bytes.concat"),
        expr_list(vec![
            expr_ident("view.to_bytes"),
            expr_list(vec![expr_ident("bytes.view"), expr_ident(a.to_string())]),
        ]),
        expr_ident(a.to_string()),
    ]);

    diagnostics.push(Diagnostic {
        code: "X07-MOVE-0001".to_string(),
        severity: Severity::Error,
        stage: Stage::Lint,
        message: "bytes.concat uses the same identifier twice".to_string(),
        loc: Some(Location::X07Ast {
            ptr: format!("{ptr}/2"),
        }),
        notes: {
            // Keep a stable note set for repair-corpus goldens.
            notes.sort();
            notes
        },
        related: Vec::new(),
        data: Default::default(),
        quickfix: Some(Quickfix {
            kind: QuickfixKind::JsonPatch,
            patch: vec![PatchOp::Replace {
                path: ptr.to_string(),
                value: x07ast::expr_to_value(&fixed),
            }],
            note: Some("Copy one side to avoid use-after-move".to_string()),
        }),
    });
}

fn lint_world_heads(
    head: &str,
    ptr: &str,
    options: LintOptions,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let requires_unsafe = head == "unsafe"
        || head == "addr_of"
        || head == "addr_of_mut"
        || head == "memcpy"
        || head == "memmove"
        || head == "memset"
        || head == "bytes.as_ptr"
        || head == "bytes.as_mut_ptr"
        || head == "view.as_ptr"
        || head == "vec_u8.as_ptr"
        || head == "vec_u8.as_mut_ptr"
        || head.starts_with("ptr.");

    if requires_unsafe && !options.allow_unsafe() {
        diagnostics.push(Diagnostic {
            code: "X07-WORLD-UNSAFE-0001".to_string(),
            severity: Severity::Error,
            stage: Stage::Lint,
            message: format!("unsafe capability is not enabled in this world: {head}"),
            loc: Some(Location::X07Ast {
                ptr: ptr.to_string(),
            }),
            notes: vec![
                "Compile with --world run-os or --world run-os-sandboxed for unsafe operations."
                    .to_string(),
            ],
            related: Vec::new(),
            data: Default::default(),
            quickfix: None,
        });
    }

    if options.world.is_eval_world() && (head.starts_with("os.") || head.starts_with("std.os.")) {
        diagnostics.push(Diagnostic {
            code: "X07-WORLD-OS-0002".to_string(),
            severity: Severity::Error,
            stage: Stage::Lint,
            message: format!("OS capability is not allowed in solve-* worlds: {head}"),
            loc: Some(Location::X07Ast {
                ptr: ptr.to_string(),
            }),
            notes: vec![
                "Compile with --world run-os or --world run-os-sandboxed for os.* builtins."
                    .to_string(),
            ],
            related: Vec::new(),
            data: Default::default(),
            quickfix: None,
        });
    }

    if !options.enable_fs && (head.starts_with("fs.") || head.starts_with("std.fs.")) {
        diagnostics.push(Diagnostic {
            code: "X07-WORLD-FS-0001".to_string(),
            severity: Severity::Error,
            stage: Stage::Lint,
            message: format!("filesystem capability is not enabled in this world: {head}"),
            loc: Some(Location::X07Ast {
                ptr: ptr.to_string(),
            }),
            notes: Vec::new(),
            related: Vec::new(),
            data: Default::default(),
            quickfix: None,
        });
    }
    if !options.enable_rr && (head.starts_with("rr.") || head.starts_with("std.rr.")) {
        diagnostics.push(Diagnostic {
            code: "X07-WORLD-RR-0001".to_string(),
            severity: Severity::Error,
            stage: Stage::Lint,
            message: format!("request/response capability is not enabled in this world: {head}"),
            loc: Some(Location::X07Ast {
                ptr: ptr.to_string(),
            }),
            notes: Vec::new(),
            related: Vec::new(),
            data: Default::default(),
            quickfix: None,
        });
    }
    if !options.enable_kv && (head.starts_with("kv.") || head.starts_with("std.kv.")) {
        diagnostics.push(Diagnostic {
            code: "X07-WORLD-KV-0001".to_string(),
            severity: Severity::Error,
            stage: Stage::Lint,
            message: format!("key/value capability is not enabled in this world: {head}"),
            loc: Some(Location::X07Ast {
                ptr: ptr.to_string(),
            }),
            notes: Vec::new(),
            related: Vec::new(),
            data: Default::default(),
            quickfix: None,
        });
    }
}
