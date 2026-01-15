use x07c::ast::Expr;

fn ident(s: &str) -> Expr {
    Expr::Ident(s.to_string())
}

fn list(items: Vec<Expr>) -> Expr {
    Expr::List(items)
}

fn has_len_hoist_from_out_of_scope_var(expr: &Expr, var: &str) -> bool {
    match expr {
        Expr::Int(_) | Expr::Ident(_) => false,
        Expr::List(items) => {
            if items.len() == 3
                && items[0].as_ident() == Some("let")
                && items[1]
                    .as_ident()
                    .is_some_and(|name| name.starts_with("__x07_len"))
            {
                if let Expr::List(rhs_items) = &items[2] {
                    if rhs_items.len() == 2
                        && rhs_items[0].as_ident() == Some("bytes.len")
                        && rhs_items[1].as_ident() == Some(var)
                    {
                        return true;
                    }
                }
            }

            items
                .iter()
                .any(|child| has_len_hoist_from_out_of_scope_var(child, var))
        }
    }
}

#[test]
fn licm_bytes_len_does_not_hoist_let_bound_names() {
    // Regression: LICM must not hoist (bytes.len <name>) when <name> is introduced by a let
    // inside the loop body. Doing so produces out-of-scope identifiers after optimization.
    let inner = list(vec![
        ident("begin"),
        list(vec![
            ident("let"),
            ident("pkg_view"),
            list(vec![
                ident("view.slice"),
                ident("b"),
                Expr::Int(0),
                Expr::Int(0),
            ]),
        ]),
        list(vec![ident("bytes.len"), ident("pkg_view")]),
        list(vec![ident("bytes.len"), ident("pkg_view")]),
        Expr::Int(0),
    ]);

    let body = list(vec![ident("begin"), inner.clone(), inner, Expr::Int(0)]);

    let expr = list(vec![
        ident("for"),
        ident("i"),
        Expr::Int(0),
        Expr::Int(10),
        body,
    ]);

    let optimized = x07c::optimize::optimize_expr(expr);
    assert!(!has_len_hoist_from_out_of_scope_var(&optimized, "pkg_view"));
}
