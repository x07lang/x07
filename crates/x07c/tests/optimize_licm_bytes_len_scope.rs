use x07c::ast::Expr;

fn ident(s: &str) -> Expr {
    Expr::Ident {
        name: s.to_string(),
        ptr: String::new(),
    }
}

fn list(items: Vec<Expr>) -> Expr {
    Expr::List {
        items,
        ptr: String::new(),
    }
}

fn has_len_hoist_from_out_of_scope_var(expr: &Expr, var: &str) -> bool {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => false,
        Expr::List { items, .. } => {
            if items.len() == 3
                && items[0].as_ident() == Some("let")
                && items[1]
                    .as_ident()
                    .is_some_and(|name| name.starts_with("__x07_len"))
            {
                if let Expr::List {
                    items: rhs_items, ..
                } = &items[2]
                {
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
                Expr::Int {
                    value: 0,
                    ptr: String::new(),
                },
                Expr::Int {
                    value: 0,
                    ptr: String::new(),
                },
            ]),
        ]),
        list(vec![ident("bytes.len"), ident("pkg_view")]),
        list(vec![ident("bytes.len"), ident("pkg_view")]),
        Expr::Int {
            value: 0,
            ptr: String::new(),
        },
    ]);

    let body = list(vec![
        ident("begin"),
        inner.clone(),
        inner,
        Expr::Int {
            value: 0,
            ptr: String::new(),
        },
    ]);

    let expr = list(vec![
        ident("for"),
        ident("i"),
        Expr::Int {
            value: 0,
            ptr: String::new(),
        },
        Expr::Int {
            value: 10,
            ptr: String::new(),
        },
        body,
    ]);

    let optimized = x07c::optimize::optimize_expr(expr);
    assert!(!has_len_hoist_from_out_of_scope_var(&optimized, "pkg_view"));
}
