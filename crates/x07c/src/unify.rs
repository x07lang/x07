use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TyTerm {
    Named(String),
    App { head: String, args: Vec<TyTerm> },
    TParam(String),
    Meta(u32),
    Never,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TyInfoTerm {
    pub ty: TyTerm,
    pub brand: Option<String>,
    pub view_full: bool,
}

impl TyInfoTerm {
    pub fn unbranded(ty: TyTerm) -> Self {
        Self {
            ty,
            brand: None,
            view_full: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Subst {
    meta: BTreeMap<u32, TyTerm>,
}

impl Subst {
    pub fn bind(&mut self, id: u32, term: TyTerm) {
        self.meta.insert(id, term);
    }

    pub fn resolve(&self, term: &TyTerm) -> TyTerm {
        match term {
            TyTerm::Meta(id) => match self.meta.get(id) {
                None => TyTerm::Meta(*id),
                Some(t) => self.resolve(t),
            },
            TyTerm::Named(s) => TyTerm::Named(s.clone()),
            TyTerm::Never => TyTerm::Never,
            TyTerm::TParam(name) => TyTerm::TParam(name.clone()),
            TyTerm::App { head, args } => TyTerm::App {
                head: head.clone(),
                args: args.iter().map(|a| self.resolve(a)).collect(),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnifyError {
    pub lhs: TyTerm,
    pub rhs: TyTerm,
    pub reason: String,
}

pub fn unify(subst: &mut Subst, a: &TyTerm, b: &TyTerm) -> Result<(), UnifyError> {
    let a = subst.resolve(a);
    let b = subst.resolve(b);
    match (a, b) {
        (TyTerm::Meta(id), TyTerm::Meta(id2)) if id == id2 => Ok(()),
        (TyTerm::Meta(id), other) => unify_meta(subst, id, other),
        (other, TyTerm::Meta(id)) => unify_meta(subst, id, other),
        (TyTerm::Never, TyTerm::Never) => Ok(()),
        (TyTerm::Named(a), TyTerm::Named(b)) => {
            if a == b {
                Ok(())
            } else {
                Err(UnifyError {
                    lhs: TyTerm::Named(a),
                    rhs: TyTerm::Named(b),
                    reason: "named_mismatch".to_string(),
                })
            }
        }
        (TyTerm::TParam(a), TyTerm::TParam(b)) => {
            if a == b {
                Ok(())
            } else {
                Err(UnifyError {
                    lhs: TyTerm::TParam(a),
                    rhs: TyTerm::TParam(b),
                    reason: "tparam_mismatch".to_string(),
                })
            }
        }
        (TyTerm::App { head: ah, args: aa }, TyTerm::App { head: bh, args: ba }) => {
            if ah != bh || aa.len() != ba.len() {
                return Err(UnifyError {
                    lhs: TyTerm::App { head: ah, args: aa },
                    rhs: TyTerm::App { head: bh, args: ba },
                    reason: "app_mismatch".to_string(),
                });
            }
            for (x, y) in aa.iter().zip(ba.iter()) {
                unify(subst, x, y)?;
            }
            Ok(())
        }
        (lhs, rhs) => Err(UnifyError {
            lhs,
            rhs,
            reason: "mismatch".to_string(),
        }),
    }
}

fn unify_meta(subst: &mut Subst, id: u32, rhs: TyTerm) -> Result<(), UnifyError> {
    if occurs_in_meta(id, &rhs, subst) {
        return Err(UnifyError {
            lhs: TyTerm::Meta(id),
            rhs,
            reason: "occurs_check".to_string(),
        });
    }
    subst.bind(id, rhs);
    Ok(())
}

fn occurs_in_meta(id: u32, t: &TyTerm, subst: &Subst) -> bool {
    match subst.resolve(t) {
        TyTerm::Meta(mid) => mid == id,
        TyTerm::Named(_) | TyTerm::Never | TyTerm::TParam(_) => false,
        TyTerm::App { args, .. } => args.iter().any(|a| occurs_in_meta(id, a, subst)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unify_meta_with_named_is_deterministic() {
        let mut s = Subst::default();
        unify(&mut s, &TyTerm::Meta(0), &TyTerm::Named("i32".to_string())).expect("unify");
        assert_eq!(
            s.resolve(&TyTerm::Meta(0)),
            TyTerm::Named("i32".to_string())
        );
    }

    #[test]
    fn unify_structural_app() {
        let mut s = Subst::default();
        let a = TyTerm::App {
            head: "vec".to_string(),
            args: vec![TyTerm::Meta(0)],
        };
        let b = TyTerm::App {
            head: "vec".to_string(),
            args: vec![TyTerm::Named("i32".to_string())],
        };
        unify(&mut s, &a, &b).expect("unify");
        assert_eq!(
            s.resolve(&TyTerm::Meta(0)),
            TyTerm::Named("i32".to_string())
        );
    }

    #[test]
    fn unify_occurs_check_fails() {
        let mut s = Subst::default();
        let a = TyTerm::Meta(0);
        let b = TyTerm::App {
            head: "vec".to_string(),
            args: vec![TyTerm::Meta(0)],
        };
        let err = unify(&mut s, &a, &b).expect_err("must fail occurs check");
        assert_eq!(err.reason, "occurs_check");
    }
}
