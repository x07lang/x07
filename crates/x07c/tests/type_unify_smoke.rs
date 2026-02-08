use x07c::unify::{unify, Subst, TyTerm};

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
