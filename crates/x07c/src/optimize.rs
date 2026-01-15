use std::collections::{BTreeSet, HashMap};

use crate::ast::Expr;
use crate::fingerprint::stable_fingerprint;

pub fn optimize_expr(expr: Expr) -> Expr {
    let expr = const_fold(expr);
    let expr = cse_pure_subexpressions(expr);
    licm_bytes_len(expr)
}

fn const_fold(expr: Expr) -> Expr {
    match expr {
        Expr::Int(_) | Expr::Ident(_) => expr,
        Expr::List(items) => {
            let mut items: Vec<Expr> = items.into_iter().map(const_fold).collect();
            let Some(head) = items.first().and_then(Expr::as_ident) else {
                return Expr::List(items);
            };

            match head {
                "begin" => {
                    if items.len() == 2 {
                        return items.remove(1);
                    }
                    Expr::List(items)
                }
                "if" if items.len() == 4 => {
                    let cond = items[1].clone();
                    let then_ = items[2].clone();
                    let else_ = items[3].clone();
                    match cond {
                        Expr::Int(i) => {
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
                                Expr::List(items)
                            }
                        }
                    }
                }
                "+" | "-" | "*" | "/" | "%" | "=" | "<u" | ">=u" if items.len() == 3 => {
                    let a = items[1].clone();
                    let b = items[2].clone();
                    match (a, b) {
                        (Expr::Int(x), Expr::Int(y)) => {
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
                            Expr::Int(v)
                        }
                        _ => Expr::List(items),
                    }
                }
                _ => Expr::List(items),
            }
        }
    }
}

fn licm_bytes_len(expr: Expr) -> Expr {
    match expr {
        Expr::Int(_) | Expr::Ident(_) => expr,
        Expr::List(items) => {
            let Some(head) = items.first().and_then(Expr::as_ident) else {
                return Expr::List(items.into_iter().map(licm_bytes_len).collect());
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
                    return Expr::List(vec![
                        Expr::Ident("for".to_string()),
                        Expr::Ident(var_name),
                        start,
                        end,
                        licm_bytes_len(body),
                    ]);
                }

                let mut used = BTreeSet::new();
                collect_idents(&Expr::List(items.clone()), &mut used);

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
                out.push(Expr::Ident("begin".to_string()));
                for (src, dst) in bindings {
                    out.push(Expr::List(vec![
                        Expr::Ident("let".to_string()),
                        Expr::Ident(dst),
                        Expr::List(vec![Expr::Ident("bytes.len".to_string()), Expr::Ident(src)]),
                    ]));
                }
                out.push(Expr::List(vec![
                    Expr::Ident("for".to_string()),
                    Expr::Ident(var_name),
                    start,
                    end,
                    new_body,
                ]));
                return Expr::List(out);
            }

            Expr::List(items.into_iter().map(licm_bytes_len).collect())
        }
    }
}

fn collect_assigned_vars(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Int(_) | Expr::Ident(_) => {}
        Expr::List(items) => {
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
        Expr::Int(_) | Expr::Ident(_) => {}
        Expr::List(items) => {
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
        Expr::Int(_) | Expr::Ident(_) => {}
        Expr::List(items) => {
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
        Expr::Int(_) | Expr::Ident(_) => expr,
        Expr::List(items) => {
            if items.first().and_then(Expr::as_ident) == Some("bytes.len")
                && items.len() == 2
                && items[1].as_ident() == Some(src)
            {
                return Expr::Ident(dst.to_string());
            }
            Expr::List(
                items
                    .into_iter()
                    .map(|e| replace_bytes_len_ident(e, src, dst))
                    .collect(),
            )
        }
    }
}

fn cse_pure_subexpressions(expr: Expr) -> Expr {
    if is_pure(&expr) {
        return cse_in_pure(expr);
    }
    match expr {
        Expr::Int(_) | Expr::Ident(_) => expr,
        Expr::List(items) => Expr::List(items.into_iter().map(cse_pure_subexpressions).collect()),
    }
}

fn cse_in_pure(expr: Expr) -> Expr {
    let mut counts: HashMap<u128, (usize, Expr)> = HashMap::new();
    collect_pure_counts(&expr, &mut counts);
    let mut candidates: Vec<(usize, usize, u128, Expr)> = Vec::new();
    for (fp, (count, sample)) in counts {
        if count < 2 {
            continue;
        }
        if matches!(sample, Expr::Int(_) | Expr::Ident(_)) {
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
    out.push(Expr::Ident("begin".to_string()));

    let mut cur_mapping: HashMap<u128, String> = HashMap::new();
    for (fp, name, sample) in &bindings {
        let rhs = replace_pure_with_mapping(sample.clone(), &cur_mapping);
        out.push(Expr::List(vec![
            Expr::Ident("let".to_string()),
            Expr::Ident(name.clone()),
            rhs,
        ]));
        cur_mapping.insert(*fp, name.clone());
    }

    out.push(replace_pure_with_mapping(expr, &cur_mapping));
    Expr::List(out)
}

fn collect_pure_counts(expr: &Expr, counts: &mut HashMap<u128, (usize, Expr)>) {
    if is_pure(expr) {
        let fp = stable_fingerprint(expr);
        let e = counts.entry(fp).or_insert((0, expr.clone()));
        e.0 += 1;
    }
    if let Expr::List(items) = expr {
        for item in items {
            collect_pure_counts(item, counts);
        }
    }
}

fn replace_pure_with_mapping(expr: Expr, mapping: &HashMap<u128, String>) -> Expr {
    if is_pure(&expr) {
        let fp = stable_fingerprint(&expr);
        if let Some(name) = mapping.get(&fp) {
            return Expr::Ident(name.clone());
        }
    }
    match expr {
        Expr::Int(_) | Expr::Ident(_) => expr,
        Expr::List(items) => Expr::List(
            items
                .into_iter()
                .map(|e| replace_pure_with_mapping(e, mapping))
                .collect(),
        ),
    }
}

fn is_pure(expr: &Expr) -> bool {
    match expr {
        Expr::Int(_) | Expr::Ident(_) => true,
        Expr::List(items) => {
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
        Expr::Int(_) => {}
        Expr::Ident(s) => {
            out.insert(s.clone());
        }
        Expr::List(items) => {
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
