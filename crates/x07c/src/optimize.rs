use std::collections::{BTreeSet, HashMap};

use crate::ast::Expr;
use crate::fingerprint::stable_fingerprint;

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

pub fn optimize_expr(expr: Expr) -> Expr {
    let expr = const_fold(expr);
    let expr = cse_pure_subexpressions(expr);
    licm_bytes_len(expr)
}

fn const_fold(expr: Expr) -> Expr {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => expr,
        Expr::List { items, ptr } => {
            let mut items: Vec<Expr> = items.into_iter().map(const_fold).collect();
            let Some(head) = items.first().and_then(Expr::as_ident) else {
                return Expr::List { items, ptr };
            };

            match head {
                "begin" => {
                    if items.len() == 2 {
                        return items.remove(1);
                    }
                    Expr::List { items, ptr }
                }
                "if" if items.len() == 4 => {
                    let cond = items[1].clone();
                    let then_ = items[2].clone();
                    let else_ = items[3].clone();
                    match cond {
                        Expr::Int { value: i, .. } => {
                            if i == 0 {
                                else_
                            } else {
                                then_
                            }
                        }
                        _ => {
                            if then_ == else_ {
                                then_
                            } else {
                                Expr::List { items, ptr }
                            }
                        }
                    }
                }
                "+" | "-" | "*" | "/" | "%" | "=" | "<u" | ">=u" if items.len() == 3 => {
                    let a = items[1].clone();
                    let b = items[2].clone();
                    match (a, b) {
                        (Expr::Int { value: x, .. }, Expr::Int { value: y, .. }) => {
                            let v = match head {
                                "+" => x.wrapping_add(y),
                                "-" => x.wrapping_sub(y),
                                "*" => x.wrapping_mul(y),
                                "/" => {
                                    if y == 0 {
                                        0
                                    } else {
                                        ((x as u32) / (y as u32)) as i32
                                    }
                                }
                                "%" => {
                                    if y == 0 {
                                        x
                                    } else {
                                        ((x as u32) % (y as u32)) as i32
                                    }
                                }
                                "=" => (x == y) as i32,
                                "<u" => ((x as u32) < (y as u32)) as i32,
                                ">=u" => ((x as u32) >= (y as u32)) as i32,
                                _ => unreachable!(),
                            };
                            Expr::Int { value: v, ptr }
                        }
                        _ => Expr::List { items, ptr },
                    }
                }
                _ => Expr::List { items, ptr },
            }
        }
    }
}

fn licm_bytes_len(expr: Expr) -> Expr {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => expr,
        Expr::List { items, ptr } => {
            let Some(head) = items.first().and_then(Expr::as_ident) else {
                return Expr::List {
                    items: items.into_iter().map(licm_bytes_len).collect(),
                    ptr,
                };
            };

            if head == "for" && items.len() == 5 {
                let var_name = items[1].as_ident().unwrap_or("").to_string();
                let start = licm_bytes_len(items[2].clone());
                let end = licm_bytes_len(items[3].clone());
                let body = items[4].clone();

                let mut assigned = BTreeSet::new();
                collect_assigned_vars(&body, &mut assigned);

                let mut bound = BTreeSet::new();
                collect_let_bound_vars(&body, &mut bound);

                let mut candidates: HashMap<String, usize> = HashMap::new();
                collect_bytes_len_idents(&body, &var_name, &assigned, &bound, &mut candidates);
                let mut hoists: Vec<String> = candidates
                    .into_iter()
                    .filter_map(|(name, count)| (count >= 2).then_some(name))
                    .collect();
                hoists.sort();

                if hoists.is_empty() {
                    return Expr::List {
                        items: vec![
                            expr_ident("for"),
                            expr_ident(var_name),
                            start,
                            end,
                            licm_bytes_len(body),
                        ],
                        ptr,
                    };
                }

                let mut used = BTreeSet::new();
                collect_idents(
                    &Expr::List {
                        items: items.clone(),
                        ptr: String::new(),
                    },
                    &mut used,
                );

                let mut bindings: Vec<(String, String)> = Vec::with_capacity(hoists.len());
                for name in hoists {
                    let bind = fresh_name(&mut used, "__x07_len");
                    bindings.push((name, bind));
                }

                let mut new_body = licm_bytes_len(body);
                for (src, dst) in &bindings {
                    new_body = replace_bytes_len_ident(new_body, src, dst);
                }

                let mut out = Vec::with_capacity(2 + bindings.len());
                out.push(expr_ident("begin"));
                for (src, dst) in bindings {
                    out.push(expr_list(vec![
                        expr_ident("let"),
                        expr_ident(dst),
                        expr_list(vec![expr_ident("bytes.len"), expr_ident(src)]),
                    ]));
                }
                out.push(expr_list(vec![
                    expr_ident("for"),
                    expr_ident(var_name),
                    start,
                    end,
                    new_body,
                ]));
                return Expr::List { items: out, ptr };
            }

            Expr::List {
                items: items.into_iter().map(licm_bytes_len).collect(),
                ptr,
            }
        }
    }
}

fn collect_assigned_vars(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => {}
        Expr::List { items, .. } => {
            if items.first().and_then(Expr::as_ident) == Some("set") && items.len() >= 2 {
                if let Some(name) = items[1].as_ident() {
                    out.insert(name.to_string());
                }
            }
            for item in items {
                collect_assigned_vars(item, out);
            }
        }
    }
}

fn collect_bytes_len_idents(
    expr: &Expr,
    loop_var: &str,
    assigned: &BTreeSet<String>,
    bound: &BTreeSet<String>,
    counts: &mut HashMap<String, usize>,
) {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => {}
        Expr::List { items, .. } => {
            if items.first().and_then(Expr::as_ident) == Some("bytes.len") && items.len() == 2 {
                if let Some(name) = items[1].as_ident() {
                    if name != loop_var && !assigned.contains(name) && !bound.contains(name) {
                        *counts.entry(name.to_string()).or_insert(0) += 1;
                    }
                }
            }
            for item in items {
                collect_bytes_len_idents(item, loop_var, assigned, bound, counts);
            }
        }
    }
}

fn collect_let_bound_vars(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => {}
        Expr::List { items, .. } => {
            if items.first().and_then(Expr::as_ident) == Some("let") && items.len() >= 2 {
                if let Some(name) = items[1].as_ident() {
                    out.insert(name.to_string());
                }
            }
            if items.first().and_then(Expr::as_ident) == Some("for") && items.len() >= 2 {
                if let Some(name) = items[1].as_ident() {
                    out.insert(name.to_string());
                }
            }
            for item in items {
                collect_let_bound_vars(item, out);
            }
        }
    }
}

fn replace_bytes_len_ident(expr: Expr, src: &str, dst: &str) -> Expr {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => expr,
        Expr::List { items, ptr } => {
            if items.first().and_then(Expr::as_ident) == Some("bytes.len")
                && items.len() == 2
                && items[1].as_ident() == Some(src)
            {
                return Expr::Ident {
                    name: dst.to_string(),
                    ptr,
                };
            }
            Expr::List {
                items: items
                    .into_iter()
                    .map(|e| replace_bytes_len_ident(e, src, dst))
                    .collect(),
                ptr,
            }
        }
    }
}

fn cse_pure_subexpressions(expr: Expr) -> Expr {
    if is_pure(&expr) {
        return cse_in_pure(expr);
    }
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => expr,
        Expr::List { items, ptr } => Expr::List {
            items: items.into_iter().map(cse_pure_subexpressions).collect(),
            ptr,
        },
    }
}

fn cse_in_pure(expr: Expr) -> Expr {
    let root_ptr = expr.ptr().to_string();
    let mut counts: HashMap<u128, (usize, Expr)> = HashMap::new();
    collect_pure_counts(&expr, &mut counts);
    let mut candidates: Vec<(usize, usize, u128, Expr)> = Vec::new();
    for (fp, (count, sample)) in counts {
        if count < 2 {
            continue;
        }
        if matches!(sample, Expr::Int { .. } | Expr::Ident { .. }) {
            continue;
        }
        let size = sample.node_count();
        if size < 3 {
            continue;
        }
        candidates.push((size, count, fp, sample));
    }
    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
    });

    let mut used = BTreeSet::new();
    collect_idents(&expr, &mut used);

    let mut bindings: Vec<(u128, String, Expr)> = Vec::new();
    let mut mapping: HashMap<u128, String> = HashMap::new();
    for (_size, _count, fp, sample) in candidates {
        if bindings.len() >= 8 {
            break;
        }
        if mapping.contains_key(&fp) {
            continue;
        }
        let name = fresh_name(&mut used, "__x07_cse");
        mapping.insert(fp, name.clone());
        bindings.push((fp, name, sample));
    }

    if bindings.is_empty() {
        return expr;
    }

    let mut out = Vec::with_capacity(2 + bindings.len());
    out.push(expr_ident("begin"));

    let mut cur_mapping: HashMap<u128, String> = HashMap::new();
    for (fp, name, sample) in &bindings {
        let rhs = replace_pure_with_mapping(sample.clone(), &cur_mapping);
        out.push(expr_list(vec![
            expr_ident("let"),
            expr_ident(name.clone()),
            rhs,
        ]));
        cur_mapping.insert(*fp, name.clone());
    }

    out.push(replace_pure_with_mapping(expr, &cur_mapping));
    Expr::List {
        items: out,
        ptr: root_ptr,
    }
}

fn collect_pure_counts(expr: &Expr, counts: &mut HashMap<u128, (usize, Expr)>) {
    if is_pure(expr) {
        let fp = stable_fingerprint(expr);
        let e = counts.entry(fp).or_insert((0, expr.clone()));
        e.0 += 1;
    }
    if let Expr::List { items, .. } = expr {
        for item in items {
            collect_pure_counts(item, counts);
        }
    }
}

fn replace_pure_with_mapping(expr: Expr, mapping: &HashMap<u128, String>) -> Expr {
    if is_pure(&expr) {
        let expr_ptr = expr.ptr().to_string();
        let fp = stable_fingerprint(&expr);
        if let Some(name) = mapping.get(&fp) {
            return Expr::Ident {
                name: name.clone(),
                ptr: expr_ptr,
            };
        }
    }
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => expr,
        Expr::List { items, ptr } => Expr::List {
            items: items
                .into_iter()
                .map(|e| replace_pure_with_mapping(e, mapping))
                .collect(),
            ptr,
        },
    }
}

fn is_pure(expr: &Expr) -> bool {
    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => true,
        Expr::List { items, .. } => {
            let Some(head) = items.first().and_then(Expr::as_ident) else {
                return false;
            };
            match head {
                "+" | "-" | "*" | "=" | "<u" | ">=u" => {
                    items.len() == 3 && items[1..].iter().all(is_pure)
                }
                _ => false,
            }
        }
    }
}

fn collect_idents(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Int { .. } => {}
        Expr::Ident { name: s, .. } => {
            out.insert(s.clone());
        }
        Expr::List { items, .. } => {
            for item in items {
                collect_idents(item, out);
            }
        }
    }
}

fn fresh_name(used: &mut BTreeSet<String>, prefix: &str) -> String {
    let mut i = 0u32;
    loop {
        let name = format!("{prefix}{i}");
        if used.insert(name.clone()) {
            return name;
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Expr;

    use super::{const_fold, cse_pure_subexpressions, expr_ident, expr_list, licm_bytes_len};

    fn expr_int(value: i32) -> Expr {
        Expr::Int {
            value,
            ptr: String::new(),
        }
    }

    fn contains_call_head(expr: &Expr, head: &str) -> bool {
        match expr {
            Expr::Int { .. } | Expr::Ident { .. } => false,
            Expr::List { items, .. } => {
                if items.first().and_then(Expr::as_ident) == Some(head) {
                    return true;
                }
                items.iter().any(|it| contains_call_head(it, head))
            }
        }
    }

    #[test]
    fn const_fold_positive_folds_constants() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let expr = expr_list(vec![expr_ident("+"), expr_int(1), expr_int(2)]);
        let out = const_fold(expr);
        assert_eq!(out, expr_int(3));
    }

    #[test]
    fn const_fold_regression_keeps_nonconst() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let expr = expr_list(vec![expr_ident("+"), expr_ident("x"), expr_int(1)]);
        let out = const_fold(expr.clone());
        assert_eq!(out, expr);
    }

    #[test]
    fn cse_positive_introduces_let_for_repeated_pure_subexpr() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let mul = expr_list(vec![expr_ident("*"), expr_ident("x"), expr_ident("x")]);
        let expr = expr_list(vec![expr_ident("+"), mul.clone(), mul]);

        let out = cse_pure_subexpressions(expr);

        let Expr::List { items, .. } = out else {
            panic!("expected list");
        };
        assert_eq!(items.len(), 3, "expected begin + 1 let + expr");
        assert_eq!(items[0].as_ident(), Some("begin"));

        let Expr::List {
            items: let_items, ..
        } = &items[1]
        else {
            panic!("expected let binding");
        };
        assert_eq!(let_items[0].as_ident(), Some("let"));
        assert_eq!(let_items[1].as_ident(), Some("__x07_cse0"));
        assert_eq!(
            let_items[2],
            expr_list(vec![expr_ident("*"), expr_ident("x"), expr_ident("x")])
        );

        let Expr::List {
            items: sum_items, ..
        } = &items[2]
        else {
            panic!("expected rewritten expression");
        };
        assert_eq!(sum_items[0].as_ident(), Some("+"));
        assert_eq!(sum_items[1].as_ident(), Some("__x07_cse0"));
        assert_eq!(sum_items[2].as_ident(), Some("__x07_cse0"));
    }

    #[test]
    fn cse_regression_does_not_hoist_impure_calls() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let call = expr_list(vec![expr_ident("bytes.alloc"), expr_int(0)]);
        let expr = expr_list(vec![expr_ident("begin"), call.clone(), call]);
        let out = cse_pure_subexpressions(expr.clone());
        assert_eq!(out, expr);
    }

    #[test]
    fn licm_positive_hoists_repeated_bytes_len() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let len_bs = expr_list(vec![expr_ident("bytes.len"), expr_ident("bs")]);
        let body = expr_list(vec![expr_ident("begin"), len_bs.clone(), len_bs]);
        let expr = expr_list(vec![
            expr_ident("for"),
            expr_ident("i"),
            expr_int(0),
            expr_int(10),
            body,
        ]);

        let out = licm_bytes_len(expr);

        let Expr::List { items, .. } = &out else {
            panic!("expected list");
        };
        assert_eq!(items.len(), 3, "expected begin + let + for");
        assert_eq!(items[0].as_ident(), Some("begin"));

        let Expr::List {
            items: let_items, ..
        } = &items[1]
        else {
            panic!("expected let binding");
        };
        assert_eq!(let_items[0].as_ident(), Some("let"));
        assert_eq!(let_items[1].as_ident(), Some("__x07_len0"));
        assert_eq!(
            let_items[2],
            expr_list(vec![expr_ident("bytes.len"), expr_ident("bs")])
        );

        let Expr::List {
            items: for_items, ..
        } = &items[2]
        else {
            panic!("expected for");
        };
        assert_eq!(for_items[0].as_ident(), Some("for"));
        assert_eq!(for_items[1].as_ident(), Some("i"));
        assert_eq!(for_items[2], expr_int(0));
        assert_eq!(for_items[3], expr_int(10));
        assert!(
            !contains_call_head(&for_items[4], "bytes.len"),
            "expected bytes.len hoisted from loop body"
        );
    }

    #[test]
    fn licm_regression_does_not_hoist_bound_or_loop_var() {
        // REGRESSION: x07.rfc.backlog.unit-tests@0.1.0
        let len_bs = expr_list(vec![expr_ident("bytes.len"), expr_ident("bs")]);
        let body = expr_list(vec![expr_ident("begin"), len_bs.clone(), len_bs]);
        let expr = expr_list(vec![
            expr_ident("for"),
            expr_ident("bs"),
            expr_int(0),
            expr_int(10),
            body,
        ]);

        let out = licm_bytes_len(expr.clone());
        assert_eq!(out, expr);
    }
}
