use std::collections::BTreeMap;

use serde_json::Value;

use crate::ast::Expr;
use crate::diagnostics::{Diagnostic, Location, PatchOp, Quickfix, QuickfixKind, Severity, Stage};
use crate::unify::{unify, Subst, TyInfoTerm, TyTerm, UnifyError};
use crate::x07ast::{
    expr_to_value, ty_to_name, type_ref_from_expr, type_ref_to_value, ContractClauseAst, TypeParam,
    TypeRef, X07AstFile,
};

#[derive(Debug, Clone, Default)]
pub struct TypecheckOptions {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TappRewrite {
    pub call_ptr: String,
    pub callee: String,
    pub inferred_type_args: Vec<TypeRef>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct TypecheckReport {
    pub diagnostics: Vec<Diagnostic>,
    pub tapp_rewrites: Vec<TappRewrite>,
}

#[derive(Debug, Default, Clone)]
pub struct TypecheckSigs {
    sigs: BTreeMap<String, FnSigAst>,
}

impl TypecheckSigs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, file: &X07AstFile) {
        add_file_sigs(file, &mut self.sigs);
    }

    pub fn add_builtins(&mut self) {
        add_builtin_sigs(&mut self.sigs);
    }

    pub fn for_file_with_builtins(file: &X07AstFile) -> Self {
        let mut sigs = Self::new();
        sigs.add_file(file);
        sigs.add_builtins();
        sigs
    }
}

#[derive(Debug, Clone)]
struct FnSigAst {
    name: String,
    type_params: Vec<TypeParam>,
    params: Vec<(String, TypeRef, Option<String>)>,
    result: TypeRef,
    result_brand: Option<String>,
    decl_ptr: String,
    is_async: bool,
}

#[derive(Debug, Clone)]
enum ConstraintOrigin {
    CallArg {
        callee: String,
        arg_index: usize,
        callee_decl_ptr: Option<String>,
    },
    Return,
    IfCond,
    IfBranch,
    SetAssign {
        name: String,
    },
    ContractExpr,
    ExprCheck,
}

impl ConstraintOrigin {
    fn key(&self) -> &'static str {
        match self {
            ConstraintOrigin::CallArg { .. } => "call_arg",
            ConstraintOrigin::Return => "return",
            ConstraintOrigin::IfCond => "if_cond",
            ConstraintOrigin::IfBranch => "if_branch",
            ConstraintOrigin::SetAssign { .. } => "set_assign",
            ConstraintOrigin::ContractExpr => "contract_expr",
            ConstraintOrigin::ExprCheck => "expr_check",
        }
    }
}

#[derive(Debug, Clone)]
struct Constraint {
    lhs: TyTerm,
    rhs: TyTerm,
    blame_ptr: String,
    origin: ConstraintOrigin,
}

#[derive(Debug, Clone)]
struct PendingImplicitTapp {
    call_ptr: String,
    callee: String,
    type_params: Vec<String>,
    meta_ids: Vec<u32>,
}

struct InferState<'a> {
    sigs: &'a BTreeMap<String, FnSigAst>,
    module_id: String,
    fn_ret: TyTerm,
    meta_counter: u32,
    env_stack: Vec<BTreeMap<String, TyInfoTerm>>,
    constraints: Vec<Constraint>,
    subst: Subst,
    diagnostics: Vec<Diagnostic>,
    pending_tapp: Vec<PendingImplicitTapp>,
}

impl<'a> InferState<'a> {
    fn new(sigs: &'a BTreeMap<String, FnSigAst>, module_id: &str, fn_ret: TyTerm) -> Self {
        Self {
            sigs,
            module_id: module_id.to_string(),
            fn_ret,
            meta_counter: 0,
            env_stack: vec![BTreeMap::from([(
                "input".to_string(),
                TyInfoTerm::unbranded(TyTerm::Named("bytes_view".to_string())),
            )])],
            constraints: Vec::new(),
            subst: Subst::default(),
            diagnostics: Vec::new(),
            pending_tapp: Vec::new(),
        }
    }

    fn should_diag_unknown_callee(&self, callee: &str) -> bool {
        match callee.split_once('.') {
            None => false,
            Some((mod_id, _)) => mod_id == self.module_id,
        }
    }

    fn fresh_meta(&mut self) -> TyTerm {
        let id = self.meta_counter;
        self.meta_counter = self.meta_counter.saturating_add(1);
        TyTerm::Meta(id)
    }

    fn push_scope(&mut self) {
        self.env_stack.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.env_stack.pop();
    }

    fn bind(&mut self, name: String, ty: TyInfoTerm) {
        if let Some(scope) = self.env_stack.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn lookup(&self, name: &str) -> Option<TyInfoTerm> {
        for scope in self.env_stack.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }

    fn add_constraint(
        &mut self,
        lhs: TyTerm,
        rhs: TyTerm,
        blame_ptr: String,
        origin: ConstraintOrigin,
    ) {
        self.constraints.push(Constraint {
            lhs,
            rhs,
            blame_ptr,
            origin,
        });
    }

    fn check_expr(&mut self, expr: &Expr, want: &TyTerm, origin: ConstraintOrigin) {
        let got = self.infer_expr(expr, Some(want));
        if matches!(got.ty, TyTerm::Never) {
            return;
        }
        self.add_constraint(got.ty, want.clone(), expr.ptr().to_string(), origin);
    }

    fn check_contract_expr(&mut self, expr: &Expr, want: &TyTerm) {
        let got = self.infer_expr(expr, None);
        if matches!(got.ty, TyTerm::Never) {
            return;
        }
        self.add_constraint(
            got.ty,
            want.clone(),
            expr.ptr().to_string(),
            ConstraintOrigin::ContractExpr,
        );
    }

    fn infer_expr(&mut self, expr: &Expr, want: Option<&TyTerm>) -> TyInfoTerm {
        match expr {
            Expr::Int { .. } => TyInfoTerm::unbranded(TyTerm::Named("i32".to_string())),
            Expr::Ident { name, .. } => {
                if name == "input" {
                    return TyInfoTerm::unbranded(TyTerm::Named("bytes_view".to_string()));
                }
                if let Some(v) = self.lookup(name) {
                    return v;
                }
                self.diagnostics.push(Diagnostic {
                    code: "X07-TYPE-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Type,
                    message: format!("unknown identifier: {name:?}"),
                    loc: Some(Location::X07Ast {
                        ptr: expr.ptr().to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: BTreeMap::new(),
                    quickfix: None,
                });
                TyInfoTerm::unbranded(self.fresh_meta())
            }
            Expr::List { items, ptr } => {
                let list_ptr = ptr.as_str();
                let Some(head) = items.first().and_then(Expr::as_ident) else {
                    for it in items {
                        let _ = self.infer_expr(it, None);
                    }
                    return TyInfoTerm::unbranded(self.fresh_meta());
                };

                match head {
                    "begin" | "unsafe" => self.infer_begin(list_ptr, items, want),
                    "let" => self.infer_let(list_ptr, items),
                    "set" => self.infer_set(list_ptr, items),
                    "set0" => self.infer_set0(list_ptr, items),
                    "if" => self.infer_if(list_ptr, items, want),
                    "for" => self.infer_for(list_ptr, items),
                    "return" => self.infer_return(list_ptr, items),
                    "try" => self.infer_try(list_ptr, items, want),
                    "tapp" => self.infer_tapp(list_ptr, items, want),
                    _ if head.starts_with("ty.") => {
                        self.infer_ty_intrinsic(list_ptr, head, items, want)
                    }
                    _ => self.infer_call(list_ptr, head, items, want),
                }
            }
        }
    }

    fn infer_begin(
        &mut self,
        _list_ptr: &str,
        items: &[Expr],
        want: Option<&TyTerm>,
    ) -> TyInfoTerm {
        if items.len() < 2 {
            return TyInfoTerm::unbranded(self.fresh_meta());
        }
        self.push_scope();
        for e in &items[1..items.len() - 1] {
            let _ = self.infer_expr(e, None);
        }
        let tail = items.last().expect("len >= 2");
        let out = if let Some(want) = want {
            self.check_expr(tail, want, ConstraintOrigin::ExprCheck);
            TyInfoTerm::unbranded(want.clone())
        } else {
            self.infer_expr(tail, None)
        };
        self.pop_scope();
        out
    }

    fn infer_let(&mut self, _list_ptr: &str, items: &[Expr]) -> TyInfoTerm {
        if items.len() != 3 {
            return TyInfoTerm::unbranded(self.fresh_meta());
        }
        let Some(name) = items.get(1).and_then(Expr::as_ident) else {
            return TyInfoTerm::unbranded(self.fresh_meta());
        };
        let init_ty = self.infer_expr(&items[2], None);
        self.bind(name.to_string(), init_ty.clone());
        init_ty
    }

    fn infer_set(&mut self, list_ptr: &str, items: &[Expr]) -> TyInfoTerm {
        if items.len() != 3 {
            return TyInfoTerm::unbranded(self.fresh_meta());
        }
        let Some(name) = items.get(1).and_then(Expr::as_ident) else {
            return TyInfoTerm::unbranded(self.fresh_meta());
        };
        let Some(var) = self.lookup(name) else {
            self.diagnostics.push(Diagnostic {
                code: "X07-TYPE-SET-0001".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: format!("set of unknown local: {name:?}"),
                loc: Some(Location::X07Ast {
                    ptr: list_ptr.to_string(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data: BTreeMap::from([("local".to_string(), Value::String(name.to_string()))]),
                quickfix: None,
            });
            let _ = self.infer_expr(&items[2], None);
            return TyInfoTerm::unbranded(self.fresh_meta());
        };
        self.check_expr(
            &items[2],
            &var.ty,
            ConstraintOrigin::SetAssign {
                name: name.to_string(),
            },
        );
        TyInfoTerm {
            ty: var.ty,
            brand: var.brand,
            view_full: var.view_full,
        }
    }

    fn infer_set0(&mut self, list_ptr: &str, items: &[Expr]) -> TyInfoTerm {
        if items.len() != 3 {
            return TyInfoTerm::unbranded(self.fresh_meta());
        }
        let Some(name) = items.get(1).and_then(Expr::as_ident) else {
            return TyInfoTerm::unbranded(self.fresh_meta());
        };
        let Some(var) = self.lookup(name) else {
            self.diagnostics.push(Diagnostic {
                code: "X07-TYPE-SET-0001".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: format!("set0 of unknown local: {name:?}"),
                loc: Some(Location::X07Ast {
                    ptr: list_ptr.to_string(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data: BTreeMap::from([("local".to_string(), Value::String(name.to_string()))]),
                quickfix: None,
            });
            let _ = self.infer_expr(&items[2], None);
            return TyInfoTerm::unbranded(self.fresh_meta());
        };
        self.check_expr(
            &items[2],
            &var.ty,
            ConstraintOrigin::SetAssign {
                name: name.to_string(),
            },
        );
        TyInfoTerm::unbranded(TyTerm::Named("i32".to_string()))
    }

    fn infer_if(&mut self, list_ptr: &str, items: &[Expr], want: Option<&TyTerm>) -> TyInfoTerm {
        if items.len() != 4 {
            return TyInfoTerm::unbranded(self.fresh_meta());
        }
        let cond = &items[1];
        self.check_expr(
            cond,
            &TyTerm::Named("i32".to_string()),
            ConstraintOrigin::IfCond,
        );
        let then_e = &items[2];
        let else_e = &items[3];

        if let Some(want) = want {
            self.push_scope();
            self.check_expr(then_e, want, ConstraintOrigin::ExprCheck);
            self.pop_scope();
            self.push_scope();
            self.check_expr(else_e, want, ConstraintOrigin::ExprCheck);
            self.pop_scope();
            return TyInfoTerm::unbranded(want.clone());
        }

        self.push_scope();
        let then_ty = self.infer_expr(then_e, None);
        self.pop_scope();
        self.push_scope();
        let else_ty = self.infer_expr(else_e, None);
        self.pop_scope();

        match (&then_ty.ty, &else_ty.ty) {
            (TyTerm::Never, TyTerm::Never) => TyInfoTerm::unbranded(TyTerm::Never),
            (TyTerm::Never, _) => else_ty,
            (_, TyTerm::Never) => then_ty,
            _ => {
                self.add_constraint(
                    then_ty.ty.clone(),
                    else_ty.ty.clone(),
                    list_ptr.to_string(),
                    ConstraintOrigin::IfBranch,
                );
                then_ty
            }
        }
    }

    fn infer_for(&mut self, _list_ptr: &str, items: &[Expr]) -> TyInfoTerm {
        if items.len() != 5 {
            return TyInfoTerm::unbranded(self.fresh_meta());
        }
        let Some(var) = items.get(1).and_then(Expr::as_ident) else {
            return TyInfoTerm::unbranded(self.fresh_meta());
        };
        self.check_expr(
            &items[2],
            &TyTerm::Named("i32".to_string()),
            ConstraintOrigin::ExprCheck,
        );
        self.check_expr(
            &items[3],
            &TyTerm::Named("i32".to_string()),
            ConstraintOrigin::ExprCheck,
        );
        self.push_scope();
        self.bind(
            var.to_string(),
            TyInfoTerm::unbranded(TyTerm::Named("i32".to_string())),
        );
        let _ = self.infer_expr(&items[4], None);
        self.pop_scope();
        TyInfoTerm::unbranded(TyTerm::Named("i32".to_string()))
    }

    fn infer_return(&mut self, _list_ptr: &str, items: &[Expr]) -> TyInfoTerm {
        if items.len() != 2 {
            return TyInfoTerm::unbranded(TyTerm::Never);
        }
        let want = self.fn_ret.clone();
        self.check_expr(&items[1], &want, ConstraintOrigin::Return);
        TyInfoTerm::unbranded(TyTerm::Never)
    }

    fn infer_try(&mut self, list_ptr: &str, items: &[Expr], want: Option<&TyTerm>) -> TyInfoTerm {
        if items.len() != 2 {
            return TyInfoTerm::unbranded(self.fresh_meta());
        }
        let arg = self.infer_expr(&items[1], None);
        self.add_constraint(
            arg.ty.clone(),
            self.fn_ret.clone(),
            items[1].ptr().to_string(),
            ConstraintOrigin::ExprCheck,
        );

        let out = match arg.ty {
            TyTerm::Named(s) if s == "result_i32" => TyTerm::Named("i32".to_string()),
            TyTerm::Named(s) if s == "result_bytes" => TyTerm::Named("bytes".to_string()),
            _ => self.fresh_meta(),
        };

        if let Some(want) = want {
            self.add_constraint(
                out.clone(),
                want.clone(),
                list_ptr.to_string(),
                ConstraintOrigin::ExprCheck,
            );
            TyInfoTerm::unbranded(want.clone())
        } else {
            TyInfoTerm::unbranded(out)
        }
    }

    fn infer_tapp(&mut self, list_ptr: &str, items: &[Expr], want: Option<&TyTerm>) -> TyInfoTerm {
        if items.len() < 3 {
            return TyInfoTerm::unbranded(self.fresh_meta());
        }
        let Some(callee) = items.get(1).and_then(Expr::as_ident) else {
            return TyInfoTerm::unbranded(self.fresh_meta());
        };
        let Some(sig) = self.sigs.get(callee) else {
            if self.should_diag_unknown_callee(callee) {
                self.diagnostics.push(Diagnostic {
                    code: "X07-TYPE-CALL-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Type,
                    message: format!("unknown callee: {callee:?}"),
                    loc: Some(Location::X07Ast {
                        ptr: list_ptr.to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: BTreeMap::from([(
                        "callee".to_string(),
                        Value::String(callee.to_string()),
                    )]),
                    quickfix: None,
                });
            }

            match items.get(2) {
                Some(Expr::List { items: tys, .. })
                    if tys.first().and_then(Expr::as_ident) == Some("tys") =>
                {
                    for it in &items[3..] {
                        let _ = self.infer_expr(it, None);
                    }
                }
                _ => {}
            }

            return if let Some(want) = want {
                TyInfoTerm::unbranded(want.clone())
            } else {
                TyInfoTerm::unbranded(self.fresh_meta())
            };
        };

        if sig.type_params.is_empty() {
            return self.infer_call(list_ptr, callee, items, want);
        }

        let arity = sig.type_params.len();

        let (type_arg_exprs, value_args) = match items.get(2) {
            Some(Expr::List { items: tys, .. })
                if tys.first().and_then(Expr::as_ident) == Some("tys") =>
            {
                (tys.iter().skip(1).cloned().collect::<Vec<_>>(), &items[3..])
            }
            _ => {
                if items.len() < 2 + arity {
                    self.diagnostics.push(Diagnostic {
                        code: "X07-TYPE-CALL-0003".to_string(),
                        severity: Severity::Error,
                        stage: Stage::Type,
                        message: format!(
                            "tapp arity mismatch for {callee:?}: expected {arity} type args"
                        ),
                        loc: Some(Location::X07Ast {
                            ptr: list_ptr.to_string(),
                        }),
                        notes: Vec::new(),
                        related: Vec::new(),
                        data: BTreeMap::from([
                            ("callee".to_string(), Value::String(callee.to_string())),
                            ("want".to_string(), Value::Number((arity as u64).into())),
                            (
                                "got".to_string(),
                                Value::Number((items.len().saturating_sub(2) as u64).into()),
                            ),
                        ]),
                        quickfix: None,
                    });
                    for it in &items[2..] {
                        let _ = self.infer_expr(it, None);
                    }
                    return TyInfoTerm::unbranded(self.fresh_meta());
                }
                (items[2..2 + arity].to_vec(), &items[2 + arity..])
            }
        };

        if type_arg_exprs.len() != arity {
            self.diagnostics.push(Diagnostic {
                code: "X07-TYPE-CALL-0003".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: format!("tapp arity mismatch for {callee:?}: expected {arity} type args"),
                loc: Some(Location::X07Ast {
                    ptr: list_ptr.to_string(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data: BTreeMap::from([
                    ("callee".to_string(), Value::String(callee.to_string())),
                    ("want".to_string(), Value::Number((arity as u64).into())),
                    (
                        "got".to_string(),
                        Value::Number((type_arg_exprs.len() as u64).into()),
                    ),
                ]),
                quickfix: None,
            });
        }

        let mut subst: BTreeMap<String, TyTerm> = BTreeMap::new();
        for (tp, e) in sig.type_params.iter().zip(type_arg_exprs.iter()) {
            let tr = type_ref_from_expr(e);
            let t = tr
                .map(|tr| type_ref_to_term(&tr))
                .unwrap_or_else(|_| self.fresh_meta());
            subst.insert(tp.name.clone(), t);
        }

        self.infer_call_with_subst(list_ptr, sig, value_args, want, subst)
    }

    fn infer_ty_intrinsic(
        &mut self,
        list_ptr: &str,
        head: &str,
        items: &[Expr],
        want: Option<&TyTerm>,
    ) -> TyInfoTerm {
        if items.len() < 2 {
            return TyInfoTerm::unbranded(self.fresh_meta());
        }
        let tr = match type_ref_from_expr(&items[1]) {
            Ok(tr) => tr,
            Err(_) => return TyInfoTerm::unbranded(self.fresh_meta()),
        };
        let tt = type_ref_to_term(&tr);

        let out = match head {
            "ty.size_bytes" | "ty.size" => {
                for it in items.iter().skip(2) {
                    let _ = self.infer_expr(it, None);
                }
                TyTerm::Named("i32".to_string())
            }
            "ty.read_le_at" => {
                for it in items.iter().skip(2) {
                    let _ = self.infer_expr(it, None);
                }
                tt.clone()
            }
            "ty.write_le_at" => {
                for it in items.iter().skip(2) {
                    let _ = self.infer_expr(it, None);
                }
                TyTerm::Named("bytes".to_string())
            }
            "ty.push_le" => {
                for it in items.iter().skip(2) {
                    let _ = self.infer_expr(it, None);
                }
                TyTerm::Named("vec_u8".to_string())
            }
            "ty.clone" => {
                let want_arity = 2;
                let got_arity = items.len().saturating_sub(1);
                if got_arity != want_arity {
                    self.diagnostics.push(Diagnostic {
                        code: "X07-TYPE-CALL-0003".to_string(),
                        severity: Severity::Error,
                        stage: Stage::Type,
                        message: format!(
                            "arity mismatch for {head:?}: expected {want_arity} args got {got_arity}"
                        ),
                        loc: Some(Location::X07Ast {
                            ptr: list_ptr.to_string(),
                        }),
                        notes: Vec::new(),
                        related: Vec::new(),
                        data: BTreeMap::from([
                            ("callee".to_string(), Value::String(head.to_string())),
                            (
                                "want".to_string(),
                                Value::Number((want_arity as u64).into()),
                            ),
                            ("got".to_string(), Value::Number((got_arity as u64).into())),
                        ]),
                        quickfix: None,
                    });
                }
                if let Some(x) = items.get(2) {
                    self.check_expr(
                        x,
                        &tt,
                        ConstraintOrigin::CallArg {
                            callee: head.to_string(),
                            arg_index: 1,
                            callee_decl_ptr: None,
                        },
                    );
                }
                for it in items.iter().skip(3) {
                    let _ = self.infer_expr(it, None);
                }
                tt.clone()
            }
            "ty.drop" => {
                let want_arity = 2;
                let got_arity = items.len().saturating_sub(1);
                if got_arity != want_arity {
                    self.diagnostics.push(Diagnostic {
                        code: "X07-TYPE-CALL-0003".to_string(),
                        severity: Severity::Error,
                        stage: Stage::Type,
                        message: format!(
                            "arity mismatch for {head:?}: expected {want_arity} args got {got_arity}"
                        ),
                        loc: Some(Location::X07Ast {
                            ptr: list_ptr.to_string(),
                        }),
                        notes: Vec::new(),
                        related: Vec::new(),
                        data: BTreeMap::from([
                            ("callee".to_string(), Value::String(head.to_string())),
                            (
                                "want".to_string(),
                                Value::Number((want_arity as u64).into()),
                            ),
                            ("got".to_string(), Value::Number((got_arity as u64).into())),
                        ]),
                        quickfix: None,
                    });
                }
                if let Some(x) = items.get(2) {
                    self.check_expr(
                        x,
                        &tt,
                        ConstraintOrigin::CallArg {
                            callee: head.to_string(),
                            arg_index: 1,
                            callee_decl_ptr: None,
                        },
                    );
                }
                for it in items.iter().skip(3) {
                    let _ = self.infer_expr(it, None);
                }
                TyTerm::Named("i32".to_string())
            }
            "ty.lt" | "ty.eq" | "ty.cmp" | "ty.hash32" => {
                for it in items.iter().skip(2) {
                    let _ = self.infer_expr(it, None);
                }
                TyTerm::Named("i32".to_string())
            }
            _ => {
                for it in items.iter().skip(2) {
                    let _ = self.infer_expr(it, None);
                }
                self.fresh_meta()
            }
        };

        if let Some(want) = want {
            self.add_constraint(
                out.clone(),
                want.clone(),
                list_ptr.to_string(),
                ConstraintOrigin::ExprCheck,
            );
            TyInfoTerm::unbranded(want.clone())
        } else {
            TyInfoTerm::unbranded(out)
        }
    }

    fn infer_call(
        &mut self,
        list_ptr: &str,
        callee: &str,
        items: &[Expr],
        want: Option<&TyTerm>,
    ) -> TyInfoTerm {
        if callee == "bytes.lit" {
            if let Some(want) = want {
                self.add_constraint(
                    TyTerm::Named("bytes".to_string()),
                    want.clone(),
                    list_ptr.to_string(),
                    ConstraintOrigin::ExprCheck,
                );
                return TyInfoTerm::unbranded(want.clone());
            }
            return TyInfoTerm::unbranded(TyTerm::Named("bytes".to_string()));
        }
        if callee == "bytes.view_lit" {
            if let Some(want) = want {
                self.add_constraint(
                    TyTerm::Named("bytes_view".to_string()),
                    want.clone(),
                    list_ptr.to_string(),
                    ConstraintOrigin::ExprCheck,
                );
                return TyInfoTerm::unbranded(want.clone());
            }
            return TyInfoTerm::unbranded(TyTerm::Named("bytes_view".to_string()));
        }

        let Some(sig) = self.sigs.get(callee) else {
            if self.should_diag_unknown_callee(callee) {
                self.diagnostics.push(Diagnostic {
                    code: "X07-TYPE-CALL-0001".to_string(),
                    severity: Severity::Error,
                    stage: Stage::Type,
                    message: format!("unknown callee: {callee:?}"),
                    loc: Some(Location::X07Ast {
                        ptr: list_ptr.to_string(),
                    }),
                    notes: Vec::new(),
                    related: Vec::new(),
                    data: BTreeMap::from([(
                        "callee".to_string(),
                        Value::String(callee.to_string()),
                    )]),
                    quickfix: None,
                });
            }
            for it in items.iter().skip(1) {
                let _ = self.infer_expr(it, None);
            }
            return if let Some(want) = want {
                TyInfoTerm::unbranded(want.clone())
            } else {
                TyInfoTerm::unbranded(self.fresh_meta())
            };
        };

        if sig.type_params.is_empty() {
            return self.infer_call_with_subst(list_ptr, sig, &items[1..], want, BTreeMap::new());
        }

        // Implicit tapp inference.
        let mut tp_names: Vec<String> = Vec::with_capacity(sig.type_params.len());
        let mut meta_ids: Vec<u32> = Vec::with_capacity(sig.type_params.len());
        let mut subst: BTreeMap<String, TyTerm> = BTreeMap::new();
        for tp in &sig.type_params {
            let meta = match self.fresh_meta() {
                TyTerm::Meta(id) => {
                    meta_ids.push(id);
                    id
                }
                _ => unreachable!("fresh_meta always returns Meta"),
            };
            tp_names.push(tp.name.clone());
            subst.insert(tp.name.clone(), TyTerm::Meta(meta));
        }
        self.pending_tapp.push(PendingImplicitTapp {
            call_ptr: list_ptr.to_string(),
            callee: callee.to_string(),
            type_params: tp_names,
            meta_ids,
        });

        self.infer_call_with_subst(list_ptr, sig, &items[1..], want, subst)
    }

    fn infer_call_with_subst(
        &mut self,
        list_ptr: &str,
        sig: &FnSigAst,
        value_args: &[Expr],
        want: Option<&TyTerm>,
        type_subst: BTreeMap<String, TyTerm>,
    ) -> TyInfoTerm {
        let want_arity = sig.params.len();
        let got_arity = value_args.len();
        if want_arity != got_arity {
            let mut related = Vec::new();
            if !sig.decl_ptr.is_empty() {
                related.push(Location::X07Ast {
                    ptr: sig.decl_ptr.clone(),
                });
            }
            self.diagnostics.push(Diagnostic {
                code: "X07-TYPE-CALL-0003".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: format!(
                    "arity mismatch for {:?}: expected {want_arity} args got {got_arity}",
                    sig.name
                ),
                loc: Some(Location::X07Ast {
                    ptr: list_ptr.to_string(),
                }),
                notes: Vec::new(),
                related,
                data: BTreeMap::from([
                    ("callee".to_string(), Value::String(sig.name.clone())),
                    (
                        "want".to_string(),
                        Value::Number((want_arity as u64).into()),
                    ),
                    ("got".to_string(), Value::Number((got_arity as u64).into())),
                ]),
                quickfix: None,
            });
        }

        let n = std::cmp::min(want_arity, got_arity);
        let callee_decl_ptr = if sig.decl_ptr.is_empty() {
            None
        } else {
            Some(sig.decl_ptr.clone())
        };
        for (idx, arg) in value_args.iter().enumerate().take(n) {
            let (_pname, pty, _pbrand) = &sig.params[idx];
            let want_ty = type_ref_to_term_with_subst(pty, &type_subst);
            self.check_expr(
                arg,
                &want_ty,
                ConstraintOrigin::CallArg {
                    callee: sig.name.clone(),
                    arg_index: idx,
                    callee_decl_ptr: callee_decl_ptr.clone(),
                },
            );
        }

        let mut ret = type_ref_to_term_with_subst(&sig.result, &type_subst);
        let ret_brand = sig.result_brand.clone();
        if sig.is_async {
            // Calling a `defasync` returns an opaque task handle (i32 in x07AST).
            ret = TyTerm::Named("i32".to_string());
        }
        if let Some(want) = want {
            self.add_constraint(
                ret.clone(),
                want.clone(),
                list_ptr.to_string(),
                ConstraintOrigin::ExprCheck,
            );
            return TyInfoTerm {
                ty: want.clone(),
                brand: ret_brand,
                view_full: false,
            };
        }
        TyInfoTerm {
            ty: ret,
            brand: ret_brand,
            view_full: false,
        }
    }

    fn solve_constraints(&mut self) {
        self.constraints.sort_by(|a, b| {
            a.blame_ptr
                .cmp(&b.blame_ptr)
                .then_with(|| a.origin.key().cmp(b.origin.key()))
                .then_with(|| match (&a.origin, &b.origin) {
                    (
                        ConstraintOrigin::CallArg {
                            callee: ac,
                            arg_index: ai,
                            ..
                        },
                        ConstraintOrigin::CallArg {
                            callee: bc,
                            arg_index: bi,
                            ..
                        },
                    ) => ac.cmp(bc).then_with(|| ai.cmp(bi)),
                    _ => std::cmp::Ordering::Equal,
                })
        });

        for c in &self.constraints {
            if let Err(err) = unify(&mut self.subst, &c.lhs, &c.rhs) {
                self.diagnostics.push(diag_for_unify_error(c, &err));
                break;
            }
        }
    }
}

fn typecheck_sort_diagnostics_in_place(diagnostics: &mut [Diagnostic]) {
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
}

fn diag_for_unify_error(c: &Constraint, err: &UnifyError) -> Diagnostic {
    let mut data: BTreeMap<String, Value> = BTreeMap::new();

    match &c.origin {
        ConstraintOrigin::CallArg {
            callee,
            arg_index,
            callee_decl_ptr,
        } => {
            data.insert("callee".to_string(), Value::String(callee.clone()));
            data.insert(
                "arg_index".to_string(),
                Value::Number((*arg_index as u64).into()),
            );
            data.insert("expected".to_string(), ty_term_to_value_like(&err.rhs));
            data.insert("got".to_string(), ty_term_to_value_like(&err.lhs));
            let mut related = Vec::new();
            if let Some(ptr) = callee_decl_ptr.as_deref().filter(|p| !p.is_empty()) {
                related.push(Location::X07Ast {
                    ptr: ptr.to_string(),
                });
            }
            Diagnostic {
                code: "X07-TYPE-CALL-0002".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: format!("call arg type mismatch for {callee:?} (arg {arg_index})"),
                loc: Some(Location::X07Ast {
                    ptr: c.blame_ptr.clone(),
                }),
                notes: Vec::new(),
                related,
                data,
                quickfix: None,
            }
        }
        ConstraintOrigin::Return => {
            data.insert("expected".to_string(), ty_term_to_value_like(&err.rhs));
            data.insert("got".to_string(), ty_term_to_value_like(&err.lhs));
            Diagnostic {
                code: "X07-TYPE-RET-0001".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: "return type mismatch".to_string(),
                loc: Some(Location::X07Ast {
                    ptr: c.blame_ptr.clone(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data,
                quickfix: None,
            }
        }
        ConstraintOrigin::IfCond => {
            data.insert("got".to_string(), ty_term_to_value_like(&err.lhs));
            Diagnostic {
                code: "X07-TYPE-IF-0001".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: "if condition must be i32".to_string(),
                loc: Some(Location::X07Ast {
                    ptr: c.blame_ptr.clone(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data,
                quickfix: None,
            }
        }
        ConstraintOrigin::IfBranch => {
            data.insert("then".to_string(), ty_term_to_value_like(&err.lhs));
            data.insert("else".to_string(), ty_term_to_value_like(&err.rhs));
            Diagnostic {
                code: "X07-TYPE-IF-0002".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: "if branches mismatch".to_string(),
                loc: Some(Location::X07Ast {
                    ptr: c.blame_ptr.clone(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data,
                quickfix: None,
            }
        }
        ConstraintOrigin::SetAssign { name } => {
            data.insert("local".to_string(), Value::String(name.clone()));
            data.insert("expected".to_string(), ty_term_to_value_like(&err.rhs));
            data.insert("got".to_string(), ty_term_to_value_like(&err.lhs));
            Diagnostic {
                code: "X07-TYPE-SET-0002".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: format!("assignment mismatch for {name:?}"),
                loc: Some(Location::X07Ast {
                    ptr: c.blame_ptr.clone(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data,
                quickfix: None,
            }
        }
        ConstraintOrigin::ExprCheck => {
            data.insert("lhs".to_string(), ty_term_to_value_like(&err.lhs));
            data.insert("rhs".to_string(), ty_term_to_value_like(&err.rhs));
            data.insert("reason".to_string(), Value::String(err.reason.clone()));
            Diagnostic {
                code: "X07-TYPE-UNIFY-0001".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: "unification failure".to_string(),
                loc: Some(Location::X07Ast {
                    ptr: c.blame_ptr.clone(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data,
                quickfix: None,
            }
        }
        ConstraintOrigin::ContractExpr => {
            data.insert("expected".to_string(), ty_term_to_value_like(&err.rhs));
            data.insert("got".to_string(), ty_term_to_value_like(&err.lhs));
            Diagnostic {
                code: "X07-CONTRACT-0001".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: "contract clause must typecheck to i32".to_string(),
                loc: Some(Location::X07Ast {
                    ptr: c.blame_ptr.clone(),
                }),
                notes: Vec::new(),
                related: Vec::new(),
                data,
                quickfix: None,
            }
        }
    }
}

fn collect_expr_values(expr: &Expr, out: &mut BTreeMap<String, Value>) {
    out.insert(expr.ptr().to_string(), expr_to_value(expr));
    if let Expr::List { items, .. } = expr {
        for item in items {
            collect_expr_values(item, out);
        }
    }
}

fn collect_contract_clause_expr_values(
    clauses: &[ContractClauseAst],
    out: &mut BTreeMap<String, Value>,
) {
    for c in clauses {
        collect_expr_values(&c.expr, out);
        for w in &c.witness {
            collect_expr_values(w, out);
        }
    }
}

fn file_expr_values(file: &X07AstFile) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    if let Some(solve) = &file.solve {
        collect_expr_values(solve, &mut out);
    }
    for f in &file.functions {
        collect_contract_clause_expr_values(&f.requires, &mut out);
        collect_contract_clause_expr_values(&f.ensures, &mut out);
        collect_contract_clause_expr_values(&f.invariant, &mut out);
        collect_expr_values(&f.body, &mut out);
    }
    for f in &file.async_functions {
        collect_contract_clause_expr_values(&f.requires, &mut out);
        collect_contract_clause_expr_values(&f.ensures, &mut out);
        collect_contract_clause_expr_values(&f.invariant, &mut out);
        collect_expr_values(&f.body, &mut out);
    }
    out
}

fn tapp_call_rewrite_expr(
    before_expr_json: &Value,
    callee: &str,
    type_args: &[TypeRef],
) -> Option<Value> {
    let before = before_expr_json.as_array()?;
    if before.is_empty() {
        return None;
    }

    let mut out: Vec<Value> = Vec::with_capacity(before.len() + 2);
    out.push(Value::String("tapp".to_string()));
    out.push(Value::String(callee.to_string()));

    let mut tys: Vec<Value> = Vec::with_capacity(type_args.len() + 1);
    tys.push(Value::String("tys".to_string()));
    for ta in type_args {
        tys.push(type_ref_to_value(ta));
    }
    out.push(Value::Array(tys));

    out.extend(before.iter().skip(1).cloned());
    Some(Value::Array(out))
}

fn diag_for_tapp_elab(
    r: &TappRewrite,
    before_expr_json: &Value,
    after_expr_json: Value,
) -> Diagnostic {
    Diagnostic {
        code: "X07-TAPP-ELAB-0001".to_string(),
        severity: Severity::Error,
        stage: Stage::Rewrite,
        message: format!("insert inferred tapp for {:?}", r.callee),
        loc: Some(Location::X07Ast {
            ptr: r.call_ptr.clone(),
        }),
        notes: Vec::new(),
        related: Vec::new(),
        data: BTreeMap::from([
            ("callee".to_string(), Value::String(r.callee.clone())),
            (
                "type_args".to_string(),
                Value::Array(r.inferred_type_args.iter().map(type_ref_to_value).collect()),
            ),
        ]),
        quickfix: Some(Quickfix {
            kind: QuickfixKind::JsonPatch,
            patch: vec![
                PatchOp::Test {
                    path: r.call_ptr.clone(),
                    value: before_expr_json.clone(),
                },
                PatchOp::Replace {
                    path: r.call_ptr.clone(),
                    value: after_expr_json,
                },
            ],
            note: Some("Insert tapp".to_string()),
        }),
    }
}

fn drain_pending_tapp(
    pending: Vec<PendingImplicitTapp>,
    subst: &Subst,
    diagnostics: &mut Vec<Diagnostic>,
    tapp_rewrites: &mut BTreeMap<String, TappRewrite>,
) {
    for p in pending {
        let mut inferred: Vec<TypeRef> = Vec::new();
        let mut unresolved: Vec<String> = Vec::new();
        for (name, mid) in p.type_params.iter().zip(p.meta_ids.iter()) {
            let t = subst.resolve(&TyTerm::Meta(*mid));
            if term_has_meta(&t) {
                unresolved.push(name.clone());
                continue;
            }
            if let Some(tr) = term_to_type_ref(&t) {
                inferred.push(tr);
            } else {
                unresolved.push(name.clone());
            }
        }
        if !unresolved.is_empty() {
            diagnostics.push(Diagnostic {
                code: "X07-TAPP-INFER-0001".to_string(),
                severity: Severity::Error,
                stage: Stage::Type,
                message: format!(
                    "cannot infer type args; explicit tapp required: {:?}",
                    p.callee
                ),
                loc: Some(Location::X07Ast { ptr: p.call_ptr }),
                notes: Vec::new(),
                related: Vec::new(),
                data: BTreeMap::from([
                    ("callee".to_string(), Value::String(p.callee)),
                    (
                        "unresolved_type_params".to_string(),
                        Value::Array(unresolved.into_iter().map(Value::String).collect()),
                    ),
                ]),
                quickfix: None,
            });
            continue;
        }

        tapp_rewrites.insert(
            p.call_ptr.clone(),
            TappRewrite {
                call_ptr: p.call_ptr,
                callee: p.callee,
                inferred_type_args: inferred,
            },
        );
    }
}

fn ty_term_to_value_like(tt: &TyTerm) -> Value {
    match tt {
        TyTerm::Named(s) => Value::String(s.clone()),
        TyTerm::Never => Value::String("never".to_string()),
        TyTerm::TParam(name) => Value::Array(vec![
            Value::String("t".to_string()),
            Value::String(name.clone()),
        ]),
        TyTerm::Meta(id) => Value::Array(vec![
            Value::String("meta".to_string()),
            Value::Number((*id as u64).into()),
        ]),
        TyTerm::App { head, args } => {
            let mut items = Vec::with_capacity(args.len() + 1);
            items.push(Value::String(head.clone()));
            for a in args {
                items.push(ty_term_to_value_like(a));
            }
            Value::Array(items)
        }
    }
}

fn type_ref_to_term(tr: &TypeRef) -> TyTerm {
    if let Some(mono) = tr.as_mono_ty() {
        let name = ty_to_name(mono);
        if name == "never" {
            return TyTerm::Never;
        }
        return TyTerm::Named(name.to_string());
    }
    match tr {
        TypeRef::Named(s) => {
            if s == "never" {
                TyTerm::Never
            } else {
                TyTerm::Named(s.clone())
            }
        }
        TypeRef::Var(name) => TyTerm::TParam(name.clone()),
        TypeRef::App { head, args } => TyTerm::App {
            head: head.clone(),
            args: args.iter().map(type_ref_to_term).collect(),
        },
    }
}

fn type_ref_to_term_with_subst(tr: &TypeRef, subst: &BTreeMap<String, TyTerm>) -> TyTerm {
    if let Some(mono) = tr.as_mono_ty() {
        let name = ty_to_name(mono);
        if name == "never" {
            return TyTerm::Never;
        }
        return TyTerm::Named(name.to_string());
    }

    match tr {
        TypeRef::Named(s) => {
            if s == "never" {
                TyTerm::Never
            } else {
                TyTerm::Named(s.clone())
            }
        }
        TypeRef::Var(name) => subst
            .get(name)
            .cloned()
            .unwrap_or_else(|| TyTerm::TParam(name.clone())),
        TypeRef::App { head, args } => TyTerm::App {
            head: head.clone(),
            args: args
                .iter()
                .map(|a| type_ref_to_term_with_subst(a, subst))
                .collect(),
        },
    }
}

fn term_has_meta(tt: &TyTerm) -> bool {
    match tt {
        TyTerm::Meta(_) => true,
        TyTerm::Named(_) | TyTerm::Never | TyTerm::TParam(_) => false,
        TyTerm::App { args, .. } => args.iter().any(term_has_meta),
    }
}

fn term_to_type_ref(tt: &TyTerm) -> Option<TypeRef> {
    match tt {
        TyTerm::Meta(_) => None,
        TyTerm::Never => Some(TypeRef::Named("never".to_string())),
        TyTerm::Named(s) => Some(TypeRef::Named(s.clone())),
        TyTerm::TParam(name) => Some(TypeRef::Var(name.clone())),
        TyTerm::App { head, args } => {
            let mut out_args: Vec<TypeRef> = Vec::with_capacity(args.len());
            for a in args {
                out_args.push(term_to_type_ref(a)?);
            }
            Some(TypeRef::App {
                head: head.clone(),
                args: out_args,
            })
        }
    }
}

fn add_file_sigs(file: &X07AstFile, sigs: &mut BTreeMap<String, FnSigAst>) {
    let export_slots = if file.kind == crate::x07ast::X07AstKind::Module && !file.exports.is_empty()
    {
        1usize
    } else {
        0usize
    };

    for (idx, f) in file.extern_functions.iter().enumerate() {
        let decl_idx = export_slots + idx;
        sigs.insert(
            f.name.clone(),
            FnSigAst {
                name: f.name.clone(),
                type_params: Vec::new(),
                params: f
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), p.ty.clone(), p.brand.clone()))
                    .collect(),
                result: f
                    .result
                    .clone()
                    .unwrap_or(TypeRef::Named("i32".to_string())),
                result_brand: None,
                decl_ptr: format!("/decls/{decl_idx}"),
                is_async: false,
            },
        );
    }

    let defn_base = export_slots + file.extern_functions.len();
    for (idx, f) in file.functions.iter().enumerate() {
        let decl_idx = defn_base + idx;
        sigs.insert(
            f.name.clone(),
            FnSigAst {
                name: f.name.clone(),
                type_params: f.type_params.clone(),
                params: f
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), p.ty.clone(), p.brand.clone()))
                    .collect(),
                result: f.result.clone(),
                result_brand: f.result_brand.clone(),
                decl_ptr: format!("/decls/{decl_idx}"),
                is_async: false,
            },
        );
    }
    let sync_fns_count = file.functions.len();
    for (idx, f) in file.async_functions.iter().enumerate() {
        let decl_idx = defn_base + sync_fns_count + idx;
        sigs.insert(
            f.name.clone(),
            FnSigAst {
                name: f.name.clone(),
                type_params: f.type_params.clone(),
                params: f
                    .params
                    .iter()
                    .map(|p| (p.name.clone(), p.ty.clone(), p.brand.clone()))
                    .collect(),
                result: f.result.clone(),
                result_brand: f.result_brand.clone(),
                decl_ptr: format!("/decls/{decl_idx}"),
                is_async: true,
            },
        );
    }
}

fn add_builtin_sigs(sigs: &mut BTreeMap<String, FnSigAst>) {
    fn mono(name: &str, params: &[(&str, &str)], result: &str) -> FnSigAst {
        FnSigAst {
            name: name.to_string(),
            type_params: Vec::new(),
            params: params
                .iter()
                .map(|(n, ty)| ((*n).to_string(), TypeRef::Named((*ty).to_string()), None))
                .collect(),
            result: TypeRef::Named(result.to_string()),
            result_brand: None,
            decl_ptr: String::new(),
            is_async: false,
        }
    }

    let bin_i32_ops = [
        "+", "-", "*", "/", "%", "=", "!=", "<", "<=", ">", ">=", "<u", ">=u", ">u", "<<u", ">>u",
        "<=u", "&", "|", "^", "&&", "||",
    ];
    for op in bin_i32_ops {
        sigs.insert(
            op.to_string(),
            mono(op, &[("a", "i32"), ("b", "i32")], "i32"),
        );
    }

    sigs.insert(
        "bytes.alloc".to_string(),
        mono("bytes.alloc", &[("n", "i32")], "bytes"),
    );
    sigs.insert(
        "bytes.len".to_string(),
        mono("bytes.len", &[("b", "bytes_view")], "i32"),
    );
    sigs.insert(
        "bytes.get_u8".to_string(),
        mono("bytes.get_u8", &[("b", "bytes_view"), ("i", "i32")], "i32"),
    );
    sigs.insert(
        "bytes.set_u8".to_string(),
        mono(
            "bytes.set_u8",
            &[("b", "bytes"), ("i", "i32"), ("v", "i32")],
            "bytes",
        ),
    );
    sigs.insert(
        "bytes.view".to_string(),
        mono("bytes.view", &[("b", "bytes")], "bytes_view"),
    );
    sigs.insert(
        "bytes.subview".to_string(),
        mono(
            "bytes.subview",
            &[("b", "bytes"), ("start", "i32"), ("len", "i32")],
            "bytes_view",
        ),
    );

    sigs.insert(
        "view.len".to_string(),
        mono("view.len", &[("v", "bytes_view")], "i32"),
    );
    sigs.insert(
        "view.get_u8".to_string(),
        mono("view.get_u8", &[("v", "bytes_view"), ("i", "i32")], "i32"),
    );
    sigs.insert(
        "view.slice".to_string(),
        mono(
            "view.slice",
            &[("v", "bytes_view"), ("start", "i32"), ("len", "i32")],
            "bytes_view",
        ),
    );
    sigs.insert(
        "view.to_bytes".to_string(),
        mono("view.to_bytes", &[("v", "bytes_view")], "bytes"),
    );

    sigs.insert(
        "vec_u8.with_capacity".to_string(),
        mono("vec_u8.with_capacity", &[("cap", "i32")], "vec_u8"),
    );
    sigs.insert(
        "vec_u8.push".to_string(),
        mono("vec_u8.push", &[("v", "vec_u8"), ("x", "i32")], "vec_u8"),
    );
    sigs.insert(
        "vec_u8.extend_bytes".to_string(),
        mono(
            "vec_u8.extend_bytes",
            &[("v", "vec_u8"), ("b", "bytes_view")],
            "vec_u8",
        ),
    );
    sigs.insert(
        "vec_u8.into_bytes".to_string(),
        mono("vec_u8.into_bytes", &[("v", "vec_u8")], "bytes"),
    );
}

fn contract_has_clauses(
    requires: &[ContractClauseAst],
    ensures: &[ContractClauseAst],
    invariant: &[ContractClauseAst],
) -> bool {
    !(requires.is_empty() && ensures.is_empty() && invariant.is_empty())
}

fn contract_collect_ident_ptrs(expr: &Expr, needle: &str, out: &mut Vec<String>) {
    match expr {
        Expr::Ident { name, .. } if name == needle => out.push(expr.ptr().to_string()),
        Expr::List { items, .. } => {
            for item in items {
                contract_collect_ident_ptrs(item, needle, out);
            }
        }
        Expr::Int { .. } => {}
        Expr::Ident { .. } => {}
    }
}

fn contract_collect_binding_ptrs(expr: &Expr, out: &mut Vec<(String, String)>) {
    let Expr::List { items, .. } = expr else {
        return;
    };
    let Some(head) = items.first().and_then(Expr::as_ident) else {
        for item in items {
            contract_collect_binding_ptrs(item, out);
        }
        return;
    };

    match head {
        "let" => {
            if let Some(name) = items.get(1).and_then(Expr::as_ident) {
                if let Some(name_expr) = items.get(1) {
                    out.push((name.to_string(), name_expr.ptr().to_string()));
                }
            }
            if let Some(init) = items.get(2) {
                contract_collect_binding_ptrs(init, out);
            }
        }
        "for" => {
            if let Some(name) = items.get(1).and_then(Expr::as_ident) {
                if let Some(name_expr) = items.get(1) {
                    out.push((name.to_string(), name_expr.ptr().to_string()));
                }
            }
            for item in items.iter().skip(2) {
                contract_collect_binding_ptrs(item, out);
            }
        }
        "begin" | "if" | "unsafe" | "return" | "try" | "tapp" | "set" | "set0" => {
            for item in items.iter().skip(1) {
                contract_collect_binding_ptrs(item, out);
            }
        }
        _ => {
            for item in items.iter().skip(1) {
                contract_collect_binding_ptrs(item, out);
            }
        }
    }
}

fn diag_contract_err(code: &str, ptr: String, message: String) -> Diagnostic {
    let mut notes = Vec::new();
    if code == "X07-CONTRACT-0002" {
        notes.push(format!(
            "Allowed contract-pure heads/operators: {}; plus any `option_*` and `result_*`. Module calls are disallowed (only builtins/operators).",
            CONTRACT_PURE_CALL_HEAD_ALLOWLIST.join(", ")
        ));
    }
    Diagnostic {
        code: code.to_string(),
        severity: Severity::Error,
        stage: Stage::Type,
        message,
        loc: Some(Location::X07Ast { ptr }),
        notes,
        related: Vec::new(),
        data: BTreeMap::new(),
        quickfix: None,
    }
}

const CONTRACT_PURE_CALL_HEAD_ALLOWLIST: &[&str] = &[
    "+",
    "-",
    "*",
    "/",
    "%",
    "=",
    "!=",
    "<",
    "<=",
    ">",
    ">=",
    "<u",
    "<=u",
    ">u",
    ">=u",
    "<<u",
    ">>u",
    "&",
    "|",
    "^",
    "&&",
    "||",
    "bytes.lit",
    "bytes.view_lit",
    "i32.lit",
    "bytes.view",
    "bytes.subview",
    "bytes.len",
    "bytes.get_u8",
    "bytes.eq",
    "bytes.cmp_range",
    "view.len",
    "view.get_u8",
    "view.slice",
    "view.to_bytes",
];

fn contract_pure_call_head(head: &str) -> bool {
    CONTRACT_PURE_CALL_HEAD_ALLOWLIST.contains(&head)
        || head.starts_with("option_")
        || head.starts_with("result_")
}

fn contract_collect_impurity(expr: &Expr, out: &mut Vec<(String, String)>) {
    let Expr::List { items, ptr } = expr else {
        return;
    };
    let list_ptr = ptr.to_string();
    let Some(head) = items.first().and_then(Expr::as_ident) else {
        out.push((
            list_ptr.clone(),
            "contract expression is not pure: list head must be an identifier".to_string(),
        ));
        for item in items {
            contract_collect_impurity(item, out);
        }
        return;
    };

    match head {
        "begin" => {
            for item in items.iter().skip(1) {
                contract_collect_impurity(item, out);
            }
        }
        "let" => {
            if let Some(init) = items.get(2) {
                contract_collect_impurity(init, out);
            }
        }
        "if" => {
            for item in items.iter().skip(1) {
                contract_collect_impurity(item, out);
            }
        }
        "tapp" => {
            let callee = items.get(1).and_then(Expr::as_ident).unwrap_or("");
            if !contract_pure_call_head(callee) {
                out.push((
                    list_ptr.clone(),
                    format!("contract expression is not pure: disallowed callee {callee:?}"),
                ));
            }
            for item in items.iter().skip(3) {
                contract_collect_impurity(item, out);
            }
        }
        "unsafe" | "set" | "set0" | "for" | "return" | "try" => {
            out.push((
                list_ptr.clone(),
                format!("contract expression is not pure: disallowed form {head:?}"),
            ));
        }
        _ => {
            if !contract_pure_call_head(head) {
                out.push((
                    list_ptr.clone(),
                    format!("contract expression is not pure: disallowed call {head:?}"),
                ));
            }
            for item in items.iter().skip(1) {
                contract_collect_impurity(item, out);
            }
        }
    }
}

fn contract_witness_ty_allowed(tt: &TyTerm) -> bool {
    match tt {
        TyTerm::Named(s) => {
            s == "i32" || s == "bytes" || s == "bytes_view" || s.starts_with("result_")
        }
        TyTerm::Never | TyTerm::TParam(_) | TyTerm::Meta(_) | TyTerm::App { .. } => false,
    }
}

fn contract_ty_brief(tt: &TyTerm) -> String {
    match tt {
        TyTerm::Named(s) => s.clone(),
        TyTerm::Never => "never".to_string(),
        TyTerm::TParam(name) => name.clone(),
        TyTerm::Meta(id) => format!("?{id}"),
        TyTerm::App { head, .. } => head.clone(),
    }
}

pub fn typecheck_file_with_sigs(
    file: &X07AstFile,
    sigs: &TypecheckSigs,
    _opts: &TypecheckOptions,
) -> TypecheckReport {
    typecheck_file_impl(file, &sigs.sigs)
}

pub fn typecheck_file_local(file: &X07AstFile, _opts: &TypecheckOptions) -> TypecheckReport {
    let sigs = TypecheckSigs::for_file_with_builtins(file);
    typecheck_file_impl(file, &sigs.sigs)
}

fn typecheck_file_impl(file: &X07AstFile, sigs: &BTreeMap<String, FnSigAst>) -> TypecheckReport {
    let expr_values = file_expr_values(file);
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut tapp_rewrites: BTreeMap<String, TappRewrite> = BTreeMap::new();
    let export_slots = if file.kind == crate::x07ast::X07AstKind::Module && !file.exports.is_empty()
    {
        1usize
    } else {
        0usize
    };
    let defn_base = export_slots + file.extern_functions.len();

    if let Some(solve) = &file.solve {
        let mut infer = InferState::new(sigs, &file.module_id, TyTerm::Named("bytes".to_string()));
        let _ = infer.infer_expr(solve, Some(&TyTerm::Named("bytes".to_string())));
        infer.solve_constraints();
        drain_pending_tapp(
            std::mem::take(&mut infer.pending_tapp),
            &infer.subst,
            &mut diagnostics,
            &mut tapp_rewrites,
        );

        diagnostics.append(&mut infer.diagnostics);
    }

    for (idx, f) in file.functions.iter().enumerate() {
        let decl_idx = defn_base + idx;
        let mut infer = InferState::new(sigs, &file.module_id, type_ref_to_term(&f.result));
        for p in &f.params {
            infer.bind(
                p.name.clone(),
                TyInfoTerm {
                    ty: type_ref_to_term(&p.ty),
                    brand: p.brand.clone(),
                    view_full: false,
                },
            );
        }
        let mut witness_types: Vec<(String, TyTerm)> = Vec::new();

        if contract_has_clauses(&f.requires, &f.ensures, &f.invariant) {
            for (pidx, p) in f.params.iter().enumerate() {
                if p.name == "__result" {
                    let ptr = format!("/decls/{decl_idx}/params/{pidx}/name");
                    let msg = "reserved name is not allowed here: \"__result\"".to_string();
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0004", ptr, msg));
                }
            }

            let mut bindings: Vec<(String, String)> = Vec::new();
            contract_collect_binding_ptrs(&f.body, &mut bindings);
            for c in &f.requires {
                contract_collect_binding_ptrs(&c.expr, &mut bindings);
                for w in &c.witness {
                    contract_collect_binding_ptrs(w, &mut bindings);
                }
            }
            for c in &f.ensures {
                contract_collect_binding_ptrs(&c.expr, &mut bindings);
                for w in &c.witness {
                    contract_collect_binding_ptrs(w, &mut bindings);
                }
            }
            for c in &f.invariant {
                contract_collect_binding_ptrs(&c.expr, &mut bindings);
                for w in &c.witness {
                    contract_collect_binding_ptrs(w, &mut bindings);
                }
            }

            for (name, ptr) in bindings {
                if name == "__result" {
                    let msg = "reserved name is not allowed here: \"__result\"".to_string();
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0004", ptr, msg));
                }
            }

            let want_bool = TyTerm::Named("i32".to_string());
            for c in &f.requires {
                let mut impure: Vec<(String, String)> = Vec::new();
                contract_collect_impurity(&c.expr, &mut impure);
                for w in &c.witness {
                    contract_collect_impurity(w, &mut impure);
                }
                for (ptr, msg) in impure {
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0002", ptr, msg));
                }

                let mut ptrs: Vec<String> = Vec::new();
                contract_collect_ident_ptrs(&c.expr, "__result", &mut ptrs);
                for w in &c.witness {
                    contract_collect_ident_ptrs(w, "__result", &mut ptrs);
                }
                for ptr in ptrs {
                    let msg = "\"__result\" is only available in ensures clauses".to_string();
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0003", ptr, msg));
                }

                infer.check_contract_expr(&c.expr, &want_bool);
                for w in &c.witness {
                    let ty = infer.infer_expr(w, None).ty;
                    witness_types.push((w.ptr().to_string(), ty));
                }
            }

            infer.push_scope();
            infer.bind(
                "__result".to_string(),
                TyInfoTerm {
                    ty: type_ref_to_term(&f.result),
                    brand: f.result_brand.clone(),
                    view_full: false,
                },
            );
            for c in &f.ensures {
                let mut impure: Vec<(String, String)> = Vec::new();
                contract_collect_impurity(&c.expr, &mut impure);
                for w in &c.witness {
                    contract_collect_impurity(w, &mut impure);
                }
                for (ptr, msg) in impure {
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0002", ptr, msg));
                }

                infer.check_contract_expr(&c.expr, &want_bool);
                for w in &c.witness {
                    let ty = infer.infer_expr(w, None).ty;
                    witness_types.push((w.ptr().to_string(), ty));
                }
            }
            infer.pop_scope();

            for c in &f.invariant {
                let mut impure: Vec<(String, String)> = Vec::new();
                contract_collect_impurity(&c.expr, &mut impure);
                for w in &c.witness {
                    contract_collect_impurity(w, &mut impure);
                }
                for (ptr, msg) in impure {
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0002", ptr, msg));
                }

                let mut ptrs: Vec<String> = Vec::new();
                contract_collect_ident_ptrs(&c.expr, "__result", &mut ptrs);
                for w in &c.witness {
                    contract_collect_ident_ptrs(w, "__result", &mut ptrs);
                }
                for ptr in ptrs {
                    let msg = "\"__result\" is only available in ensures clauses".to_string();
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0003", ptr, msg));
                }

                infer.check_contract_expr(&c.expr, &want_bool);
                for w in &c.witness {
                    let ty = infer.infer_expr(w, None).ty;
                    witness_types.push((w.ptr().to_string(), ty));
                }
            }
        }

        let want = infer.fn_ret.clone();
        let _ = infer.infer_expr(&f.body, Some(&want));
        infer.solve_constraints();
        for (ptr, ty) in std::mem::take(&mut witness_types) {
            let resolved = infer.subst.resolve(&ty);
            if !contract_witness_ty_allowed(&resolved) {
                let msg = format!(
                    "contract witness has unsupported type: {} (allowed: i32, bytes, bytes_view, result_*)",
                    contract_ty_brief(&resolved)
                );
                infer
                    .diagnostics
                    .push(diag_contract_err("X07-CONTRACT-0005", ptr, msg));
            }
        }
        drain_pending_tapp(
            std::mem::take(&mut infer.pending_tapp),
            &infer.subst,
            &mut diagnostics,
            &mut tapp_rewrites,
        );

        diagnostics.append(&mut infer.diagnostics);
    }

    for (idx, f) in file.async_functions.iter().enumerate() {
        let decl_idx = defn_base + file.functions.len() + idx;
        let mut infer = InferState::new(sigs, &file.module_id, type_ref_to_term(&f.result));
        for p in &f.params {
            infer.bind(
                p.name.clone(),
                TyInfoTerm {
                    ty: type_ref_to_term(&p.ty),
                    brand: p.brand.clone(),
                    view_full: false,
                },
            );
        }
        let mut witness_types: Vec<(String, TyTerm)> = Vec::new();

        if contract_has_clauses(&f.requires, &f.ensures, &f.invariant) {
            for (pidx, p) in f.params.iter().enumerate() {
                if p.name == "__result" {
                    let ptr = format!("/decls/{decl_idx}/params/{pidx}/name");
                    let msg = "reserved name is not allowed here: \"__result\"".to_string();
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0004", ptr, msg));
                }
            }

            let mut bindings: Vec<(String, String)> = Vec::new();
            contract_collect_binding_ptrs(&f.body, &mut bindings);
            for c in &f.requires {
                contract_collect_binding_ptrs(&c.expr, &mut bindings);
                for w in &c.witness {
                    contract_collect_binding_ptrs(w, &mut bindings);
                }
            }
            for c in &f.ensures {
                contract_collect_binding_ptrs(&c.expr, &mut bindings);
                for w in &c.witness {
                    contract_collect_binding_ptrs(w, &mut bindings);
                }
            }
            for c in &f.invariant {
                contract_collect_binding_ptrs(&c.expr, &mut bindings);
                for w in &c.witness {
                    contract_collect_binding_ptrs(w, &mut bindings);
                }
            }

            for (name, ptr) in bindings {
                if name == "__result" {
                    let msg = "reserved name is not allowed here: \"__result\"".to_string();
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0004", ptr, msg));
                }
            }

            let want_bool = TyTerm::Named("i32".to_string());
            for c in &f.requires {
                let mut impure: Vec<(String, String)> = Vec::new();
                contract_collect_impurity(&c.expr, &mut impure);
                for w in &c.witness {
                    contract_collect_impurity(w, &mut impure);
                }
                for (ptr, msg) in impure {
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0002", ptr, msg));
                }

                let mut ptrs: Vec<String> = Vec::new();
                contract_collect_ident_ptrs(&c.expr, "__result", &mut ptrs);
                for w in &c.witness {
                    contract_collect_ident_ptrs(w, "__result", &mut ptrs);
                }
                for ptr in ptrs {
                    let msg = "\"__result\" is only available in ensures clauses".to_string();
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0003", ptr, msg));
                }

                infer.check_contract_expr(&c.expr, &want_bool);
                for w in &c.witness {
                    let ty = infer.infer_expr(w, None).ty;
                    witness_types.push((w.ptr().to_string(), ty));
                }
            }

            infer.push_scope();
            infer.bind(
                "__result".to_string(),
                TyInfoTerm {
                    ty: type_ref_to_term(&f.result),
                    brand: f.result_brand.clone(),
                    view_full: false,
                },
            );
            for c in &f.ensures {
                let mut impure: Vec<(String, String)> = Vec::new();
                contract_collect_impurity(&c.expr, &mut impure);
                for w in &c.witness {
                    contract_collect_impurity(w, &mut impure);
                }
                for (ptr, msg) in impure {
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0002", ptr, msg));
                }

                infer.check_contract_expr(&c.expr, &want_bool);
                for w in &c.witness {
                    let ty = infer.infer_expr(w, None).ty;
                    witness_types.push((w.ptr().to_string(), ty));
                }
            }
            infer.pop_scope();

            for c in &f.invariant {
                let mut impure: Vec<(String, String)> = Vec::new();
                contract_collect_impurity(&c.expr, &mut impure);
                for w in &c.witness {
                    contract_collect_impurity(w, &mut impure);
                }
                for (ptr, msg) in impure {
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0002", ptr, msg));
                }

                let mut ptrs: Vec<String> = Vec::new();
                contract_collect_ident_ptrs(&c.expr, "__result", &mut ptrs);
                for w in &c.witness {
                    contract_collect_ident_ptrs(w, "__result", &mut ptrs);
                }
                for ptr in ptrs {
                    let msg = "\"__result\" is only available in ensures clauses".to_string();
                    infer
                        .diagnostics
                        .push(diag_contract_err("X07-CONTRACT-0003", ptr, msg));
                }

                infer.check_contract_expr(&c.expr, &want_bool);
                for w in &c.witness {
                    let ty = infer.infer_expr(w, None).ty;
                    witness_types.push((w.ptr().to_string(), ty));
                }
            }
        }

        let want = infer.fn_ret.clone();
        let _ = infer.infer_expr(&f.body, Some(&want));
        infer.solve_constraints();
        for (ptr, ty) in std::mem::take(&mut witness_types) {
            let resolved = infer.subst.resolve(&ty);
            if !contract_witness_ty_allowed(&resolved) {
                let msg = format!(
                    "contract witness has unsupported type: {} (allowed: i32, bytes, bytes_view, result_*)",
                    contract_ty_brief(&resolved)
                );
                infer
                    .diagnostics
                    .push(diag_contract_err("X07-CONTRACT-0005", ptr, msg));
            }
        }
        drain_pending_tapp(
            std::mem::take(&mut infer.pending_tapp),
            &infer.subst,
            &mut diagnostics,
            &mut tapp_rewrites,
        );

        diagnostics.append(&mut infer.diagnostics);
    }

    for r in tapp_rewrites.values() {
        let Some(before_expr_json) = expr_values.get(&r.call_ptr) else {
            continue;
        };
        let Some(after_expr_json) =
            tapp_call_rewrite_expr(before_expr_json, &r.callee, &r.inferred_type_args)
        else {
            continue;
        };
        diagnostics.push(diag_for_tapp_elab(r, before_expr_json, after_expr_json));
    }

    typecheck_sort_diagnostics_in_place(diagnostics.as_mut_slice());

    TypecheckReport {
        diagnostics,
        tapp_rewrites: tapp_rewrites.into_values().collect(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{json, Value};
    use x07_contracts::X07AST_SCHEMA_VERSION;

    use crate::unify::{Subst, TyTerm};
    use crate::x07ast::{canonicalize_x07ast_file, parse_x07ast_json, TypeRef};

    use super::{drain_pending_tapp, typecheck_file_local, PendingImplicitTapp, TypecheckOptions};

    #[test]
    fn pending_tapp_unresolved_emits_explicit_tapp_required_diag() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let pending = PendingImplicitTapp {
            call_ptr: "/solve/0".to_string(),
            callee: "main.id".to_string(),
            type_params: vec!["A".to_string()],
            meta_ids: vec![0],
        };
        let subst = Subst::default();
        let mut diagnostics = Vec::new();
        let mut tapp_rewrites = BTreeMap::new();

        drain_pending_tapp(vec![pending], &subst, &mut diagnostics, &mut tapp_rewrites);

        assert!(
            tapp_rewrites.is_empty(),
            "unexpected rewrites: {tapp_rewrites:?}"
        );
        assert_eq!(diagnostics.len(), 1, "unexpected diags: {diagnostics:?}");
        let diag = diagnostics.first().expect("len == 1");
        assert_eq!(diag.code, "X07-TAPP-INFER-0001");

        let unresolved = diag
            .data
            .get("unresolved_type_params")
            .and_then(|v| v.as_array())
            .expect("unresolved_type_params must be an array");
        assert_eq!(unresolved, &vec![Value::String("A".to_string())]);
    }

    #[test]
    fn pending_tapp_resolved_records_rewrite() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let pending = PendingImplicitTapp {
            call_ptr: "/solve/0".to_string(),
            callee: "main.id".to_string(),
            type_params: vec!["A".to_string()],
            meta_ids: vec![0],
        };
        let mut subst = Subst::default();
        subst.bind(0, TyTerm::Named("i32".to_string()));
        let mut diagnostics = Vec::new();
        let mut tapp_rewrites = BTreeMap::new();

        drain_pending_tapp(vec![pending], &subst, &mut diagnostics, &mut tapp_rewrites);

        assert!(diagnostics.is_empty(), "unexpected diags: {diagnostics:?}");
        assert_eq!(
            tapp_rewrites.len(),
            1,
            "unexpected rewrites: {tapp_rewrites:?}"
        );
        let r = tapp_rewrites.get("/solve/0").expect("rewrite present");
        assert_eq!(
            r.inferred_type_args,
            vec![TypeRef::Named("i32".to_string())]
        );
    }

    #[test]
    fn typecheck_contract_clause_must_be_i32() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let doc = json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": [],
            "decls": [
                {
                    "kind": "defn",
                    "name": "main.f",
                    "params": [{"name": "x", "ty": "bytes"}],
                    "result": "bytes",
                    "requires": [{"expr": ["bytes.view", "x"]}],
                    "body": ["bytes.alloc", 0],
                }
            ],
        });
        let bytes = serde_json::to_vec(&doc).expect("encode x07AST json");
        let mut file = parse_x07ast_json(&bytes).expect("parse x07AST");
        canonicalize_x07ast_file(&mut file);

        let report = typecheck_file_local(&file, &TypecheckOptions::default());
        let diag = report
            .diagnostics
            .iter()
            .find(|d| d.code == "X07-CONTRACT-0001")
            .expect("expected X07-CONTRACT-0001");
        assert_eq!(
            diag.data.get("expected"),
            Some(&Value::String("i32".to_string()))
        );
        assert_eq!(
            diag.data.get("got"),
            Some(&Value::String("bytes_view".to_string()))
        );
    }

    #[test]
    fn typecheck_call_arg_mismatch_includes_callee_and_arg_index() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let doc = json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": [],
            "decls": [
                {
                    "kind": "defn",
                    "name": "main.id",
                    "params": [{"name": "x", "ty": "i32"}],
                    "result": "i32",
                    "body": "x",
                }
            ],
            "solve": ["begin", ["let", "b", ["bytes.alloc", 0]], ["main.id", "b"], ["bytes.alloc", 0]],
        });
        let bytes = serde_json::to_vec(&doc).expect("encode x07AST json");
        let mut file = parse_x07ast_json(&bytes).expect("parse x07AST");
        canonicalize_x07ast_file(&mut file);

        let report = typecheck_file_local(&file, &TypecheckOptions::default());
        let diag = report
            .diagnostics
            .iter()
            .find(|d| d.code == "X07-TYPE-CALL-0002")
            .expect("expected X07-TYPE-CALL-0002");
        assert_eq!(
            diag.data.get("callee"),
            Some(&Value::String("main.id".to_string()))
        );
        assert_eq!(diag.data.get("arg_index").and_then(|v| v.as_u64()), Some(0));
        assert_eq!(
            diag.data.get("expected"),
            Some(&Value::String("i32".to_string()))
        );
        assert_eq!(
            diag.data.get("got"),
            Some(&Value::String("bytes".to_string()))
        );
    }

    #[test]
    fn typecheck_set0_unifies_if_branches_as_i32() {
        let doc = json!({
            "schema_version": X07AST_SCHEMA_VERSION,
            "kind": "entry",
            "module_id": "main",
            "imports": [],
            "decls": [],
            "solve": ["begin",
                ["let", "b", ["bytes.alloc", 0]],
                ["if", 1, ["set0", "b", ["bytes.alloc", 1]], 0],
                "b"
            ],
        });
        let bytes = serde_json::to_vec(&doc).expect("encode x07AST json");
        let mut file = parse_x07ast_json(&bytes).expect("parse x07AST");
        canonicalize_x07ast_file(&mut file);

        let report = typecheck_file_local(&file, &TypecheckOptions::default());
        assert!(
            report.diagnostics.is_empty(),
            "unexpected diags: {:?}",
            report.diagnostics
        );
    }
}
