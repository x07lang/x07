use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::ast::Expr;
use crate::fingerprint::stable_fingerprint;
use crate::program::FunctionParam;
use crate::program::Program;
use crate::types::Ty;
use crate::x07ast::ContractClauseAst;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimpleTy {
    I32,
    NonI32,
}

type LocalTyEnv = BTreeMap<String, SimpleTy>;

fn expr_ident(name: impl Into<String>) -> Expr {
    Expr::Ident {
        name: name.into(),
        ptr: String::new(),
    }
}

fn expr_int(value: i32) -> Expr {
    Expr::Int {
        value,
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
    optimize_expr_with_seed(expr, &LocalTyEnv::new())
}

pub(crate) fn optimize_expr_with_params(expr: Expr, params: &[FunctionParam]) -> Expr {
    let seed = seed_env_from_params(params);
    optimize_expr_with_seed(expr, &seed)
}

fn seed_env_from_params(params: &[FunctionParam]) -> LocalTyEnv {
    let mut out: LocalTyEnv = LocalTyEnv::new();
    for p in params {
        let ty = match p.ty {
            Ty::I32 => SimpleTy::I32,
            _ => SimpleTy::NonI32,
        };
        out.insert(p.name.clone(), ty);
    }
    out
}

fn optimize_expr_with_seed(expr: Expr, seed: &LocalTyEnv) -> Expr {
    let expr = const_fold(expr);
    let expr = strength_reduce(expr);
    let expr = dce_unused_lets(expr, seed);
    let expr = unroll_small_fors(expr, seed);
    let expr = cse_pure_subexpressions(expr);
    let expr = licm_bytes_len(expr);
    dce_unused_lets(expr, seed)
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
                "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<u" | ">>u" | "=" | "!="
                | "<" | "<=" | ">" | ">=" | "<u" | "<=u" | ">u" | ">=u"
                    if items.len() == 3 =>
                {
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
                                "&" => ((x as u32) & (y as u32)) as i32,
                                "|" => ((x as u32) | (y as u32)) as i32,
                                "^" => ((x as u32) ^ (y as u32)) as i32,
                                "<<u" => {
                                    let sh = (y as u32) & 31;
                                    ((x as u32).wrapping_shl(sh)) as i32
                                }
                                ">>u" => {
                                    let sh = (y as u32) & 31;
                                    ((x as u32).wrapping_shr(sh)) as i32
                                }
                                "=" => (x == y) as i32,
                                "!=" => (x != y) as i32,
                                "<" => (x < y) as i32,
                                "<=" => (x <= y) as i32,
                                ">" => (x > y) as i32,
                                ">=" => (x >= y) as i32,
                                "<u" => ((x as u32) < (y as u32)) as i32,
                                "<=u" => ((x as u32) <= (y as u32)) as i32,
                                ">u" => ((x as u32) > (y as u32)) as i32,
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

fn strength_reduce(expr: Expr) -> Expr {
    fn is_u32_power_of_two(v: i32) -> Option<u32> {
        let u = v as u32;
        if u == 0 {
            return None;
        }
        if (u & (u - 1)) != 0 {
            return None;
        }
        Some(u.trailing_zeros())
    }

    fn with_root_ptr(expr: Expr, ptr: &str) -> Expr {
        match expr {
            Expr::Int { value, .. } => Expr::Int {
                value,
                ptr: ptr.to_string(),
            },
            Expr::Ident { name, .. } => Expr::Ident {
                name,
                ptr: ptr.to_string(),
            },
            Expr::List { items, .. } => Expr::List {
                items,
                ptr: ptr.to_string(),
            },
        }
    }

    match expr {
        Expr::Int { .. } | Expr::Ident { .. } => expr,
        Expr::List { items, ptr } => {
            let Some(head) = items.first().and_then(Expr::as_ident) else {
                return Expr::List {
                    items: items.into_iter().map(strength_reduce).collect(),
                    ptr,
                };
            };

            if items.len() == 3 && matches!(head, "*" | "/" | "%") {
                let a = strength_reduce(items[1].clone());
                let b = strength_reduce(items[2].clone());

                match head {
                    "*" => {
                        if let Expr::Int { value: x, .. } = a {
                            if x == 0 && is_pure(&b) {
                                return Expr::Int { value: 0, ptr };
                            } else if x == 1 {
                                return with_root_ptr(b, &ptr);
                            } else if let Some(k) = is_u32_power_of_two(x) {
                                return Expr::List {
                                    items: vec![expr_ident("<<u"), b, expr_int(k as i32)],
                                    ptr,
                                };
                            }
                        }
                        if let Expr::Int { value: y, .. } = b {
                            if y == 0 && is_pure(&a) {
                                return Expr::Int { value: 0, ptr };
                            } else if y == 1 {
                                return with_root_ptr(a, &ptr);
                            } else if let Some(k) = is_u32_power_of_two(y) {
                                return Expr::List {
                                    items: vec![expr_ident("<<u"), a, expr_int(k as i32)],
                                    ptr,
                                };
                            }
                        }
                    }
                    "/" => {
                        if let Expr::Int { value: y, .. } = b {
                            if y == 0 && is_pure(&a) {
                                return Expr::Int { value: 0, ptr };
                            } else if y == 1 {
                                return with_root_ptr(a, &ptr);
                            } else if let Some(k) = is_u32_power_of_two(y) {
                                return Expr::List {
                                    items: vec![expr_ident(">>u"), a, expr_int(k as i32)],
                                    ptr,
                                };
                            }
                        }
                        if let Expr::Int { value: x, .. } = a {
                            if x == 0 && is_pure(&b) {
                                return Expr::Int { value: 0, ptr };
                            }
                        }
                    }
                    "%" => {
                        if let Expr::Int { value: y, .. } = b {
                            if y == 0 {
                                return with_root_ptr(a, &ptr);
                            } else if y == 1 && is_pure(&a) {
                                return Expr::Int { value: 0, ptr };
                            } else if y == 1 {
                                // Do not reduce if it would drop evaluation of `a`.
                            } else if is_u32_power_of_two(y).is_some() {
                                let mask = ((y as u32) - 1) as i32;
                                return Expr::List {
                                    items: vec![expr_ident("&"), a, expr_int(mask)],
                                    ptr,
                                };
                            }
                        }
                        if let Expr::Int { value: x, .. } = a {
                            if x == 0 && is_pure(&b) {
                                return Expr::Int { value: 0, ptr };
                            }
                        }
                    }
                    _ => {}
                }

                return Expr::List {
                    items: vec![expr_ident(head), a, b],
                    ptr,
                };
            }

            Expr::List {
                items: items.into_iter().map(strength_reduce).collect(),
                ptr,
            }
        }
    }
}

fn dce_unused_lets(expr: Expr, seed: &LocalTyEnv) -> Expr {
    fn infer_simple_ty(expr: &Expr, env: &LocalTyEnv) -> Option<SimpleTy> {
        match expr {
            Expr::Int { .. } => Some(SimpleTy::I32),
            Expr::Ident { name, .. } => match env.get(name) {
                Some(SimpleTy::I32) => Some(SimpleTy::I32),
                _ => None,
            },
            Expr::List { items, .. } => {
                let head = items.first().and_then(Expr::as_ident)?;
                if items.len() != 3 {
                    return None;
                }
                if !is_pure_i32_head(head) {
                    return None;
                }
                let a = infer_simple_ty(&items[1], env)?;
                let b = infer_simple_ty(&items[2], env)?;
                (a == SimpleTy::I32 && b == SimpleTy::I32).then_some(SimpleTy::I32)
            }
        }
    }

    fn free_vars(expr: &Expr, bound: &mut Vec<BTreeSet<String>>, out: &mut BTreeSet<String>) {
        match expr {
            Expr::Int { .. } => {}
            Expr::Ident { name, .. } => {
                if bound.iter().rev().any(|s| s.contains(name)) {
                    return;
                }
                out.insert(name.clone());
            }
            Expr::List { items, .. } => {
                let Some(head) = items.first().and_then(Expr::as_ident) else {
                    for item in items {
                        free_vars(item, bound, out);
                    }
                    return;
                };
                let args = &items[1..];

                match head {
                    "begin" | "unsafe" => {
                        bound.push(BTreeSet::new());
                        for a in args {
                            if let Expr::List {
                                items: let_items, ..
                            } = a
                            {
                                if let_items.first().and_then(Expr::as_ident) == Some("let")
                                    && let_items.len() == 3
                                {
                                    if let Some(name) = let_items[1].as_ident() {
                                        free_vars(&let_items[2], bound, out);
                                        bound.last_mut().expect("scope").insert(name.to_string());
                                        continue;
                                    }
                                }
                            }
                            free_vars(a, bound, out);
                        }
                        bound.pop();
                    }
                    "if" if args.len() == 3 => {
                        free_vars(&args[0], bound, out);
                        for branch in [&args[1], &args[2]] {
                            bound.push(BTreeSet::new());
                            free_vars(branch, bound, out);
                            bound.pop();
                        }
                    }
                    "for" if args.len() == 4 => {
                        free_vars(&args[1], bound, out);
                        free_vars(&args[2], bound, out);
                        bound.push(BTreeSet::new());
                        if let Some(var) = args[0].as_ident() {
                            bound.last_mut().expect("scope").insert(var.to_string());
                        }
                        free_vars(&args[3], bound, out);
                        bound.pop();
                    }
                    "set" | "set0" if args.len() == 2 => {
                        if let Some(name) = args[0].as_ident() {
                            if !bound.iter().rev().any(|s| s.contains(name)) {
                                out.insert(name.to_string());
                            }
                        } else {
                            free_vars(&args[0], bound, out);
                        }
                        free_vars(&args[1], bound, out);
                    }
                    "bytes.lit" | "bytes.view_lit" | "i32.lit" => {
                        for (idx, a) in args.iter().enumerate() {
                            if idx == 0 {
                                continue;
                            }
                            free_vars(a, bound, out);
                        }
                    }
                    _ => {
                        for a in args {
                            free_vars(a, bound, out);
                        }
                    }
                }
            }
        }
    }

    fn go(expr: Expr, env: &LocalTyEnv) -> Expr {
        match expr {
            Expr::Int { .. } | Expr::Ident { .. } => expr,
            Expr::List { items, ptr } => {
                let Some(head) = items.first().and_then(Expr::as_ident) else {
                    return Expr::List {
                        items: items.into_iter().map(|e| go(e, env)).collect(),
                        ptr,
                    };
                };
                let head = head.to_string();
                let is_unsafe = head == "unsafe";
                if matches!(head.as_str(), "begin" | "unsafe") {
                    let mut env_here: LocalTyEnv = env.clone();

                    let mut exprs: Vec<Expr> = Vec::with_capacity(items.len().saturating_sub(1));
                    let mut is_safe_let_rhs: Vec<bool> = Vec::with_capacity(exprs.len());

                    for raw in items.into_iter().skip(1) {
                        let e = go(raw, &env_here);
                        let mut safe_rhs = false;

                        if let Expr::List {
                            items: let_items, ..
                        } = &e
                        {
                            if let_items.first().and_then(Expr::as_ident) == Some("let")
                                && let_items.len() == 3
                            {
                                if let Some(name) = let_items[1].as_ident() {
                                    safe_rhs = infer_simple_ty(&let_items[2], &env_here)
                                        == Some(SimpleTy::I32);
                                    let ty = infer_simple_ty(&let_items[2], &env_here)
                                        .unwrap_or(SimpleTy::NonI32);
                                    env_here.insert(name.to_string(), ty);
                                }
                            }
                        }

                        exprs.push(e);
                        is_safe_let_rhs.push(safe_rhs);
                    }

                    if exprs.is_empty() {
                        return Expr::List {
                            items: vec![expr_ident(head.as_str())],
                            ptr,
                        };
                    }

                    let mut live: BTreeSet<String> = BTreeSet::new();
                    {
                        let mut bound: Vec<BTreeSet<String>> = Vec::new();
                        free_vars(exprs.last().expect("non-empty"), &mut bound, &mut live);
                    }

                    let mut kept: Vec<Expr> = Vec::with_capacity(exprs.len());
                    for (idx, e) in exprs.into_iter().enumerate().rev() {
                        let is_tail = idx == is_safe_let_rhs.len() - 1;
                        if is_tail {
                            kept.push(e);
                            continue;
                        }

                        if let Expr::List {
                            items: let_items, ..
                        } = &e
                        {
                            if let_items.first().and_then(Expr::as_ident) == Some("let")
                                && let_items.len() == 3
                            {
                                if let Some(name) = let_items[1].as_ident() {
                                    if !live.contains(name) && is_safe_let_rhs[idx] {
                                        continue;
                                    }

                                    live.remove(name);
                                    let mut bound: Vec<BTreeSet<String>> = Vec::new();
                                    free_vars(&let_items[2], &mut bound, &mut live);
                                    kept.push(e);
                                    continue;
                                }
                            }
                        }

                        let mut bound: Vec<BTreeSet<String>> = Vec::new();
                        free_vars(&e, &mut bound, &mut live);
                        kept.push(e);
                    }

                    kept.reverse();

                    if kept.len() == 1 && !is_unsafe {
                        return kept.remove(0);
                    }

                    let mut out = Vec::with_capacity(1 + kept.len());
                    out.push(expr_ident(head.as_str()));
                    out.extend(kept);
                    return Expr::List { items: out, ptr };
                }

                Expr::List {
                    items: items.into_iter().map(|e| go(e, env)).collect(),
                    ptr,
                }
            }
        }
    }

    go(expr, seed)
}

fn unroll_small_fors(expr: Expr, seed: &LocalTyEnv) -> Expr {
    const UNROLL_MAX_ITERS: u32 = 8;

    fn contains_set_to_var(expr: &Expr, var: &str) -> bool {
        match expr {
            Expr::Int { .. } | Expr::Ident { .. } => false,
            Expr::List { items, .. } => {
                if matches!(items.first().and_then(Expr::as_ident), Some("set" | "set0"))
                    && items.len() >= 2
                    && items[1].as_ident() == Some(var)
                {
                    return true;
                }
                items.iter().any(|e| contains_set_to_var(e, var))
            }
        }
    }

    fn contains_nested_for_var(expr: &Expr, var: &str) -> bool {
        match expr {
            Expr::Int { .. } | Expr::Ident { .. } => false,
            Expr::List { items, .. } => {
                if items.first().and_then(Expr::as_ident) == Some("for")
                    && items.len() >= 2
                    && items[1].as_ident() == Some(var)
                {
                    return true;
                }
                items.iter().any(|e| contains_nested_for_var(e, var))
            }
        }
    }

    fn go(expr: Expr, env: &LocalTyEnv) -> Expr {
        match expr {
            Expr::Int { .. } | Expr::Ident { .. } => expr,
            Expr::List { items, ptr } => {
                let Some(head) = items.first().and_then(Expr::as_ident) else {
                    return Expr::List {
                        items: items.into_iter().map(|e| go(e, env)).collect(),
                        ptr,
                    };
                };

                if matches!(head, "begin" | "unsafe") {
                    let mut env_here = env.clone();
                    let items_len = items.len();
                    let mut it = items.into_iter();
                    let mut out: Vec<Expr> = Vec::with_capacity(items_len);
                    out.push(it.next().expect("non-empty"));
                    for raw in it {
                        let e = go(raw, &env_here);
                        if let Expr::List {
                            items: let_items, ..
                        } = &e
                        {
                            if let_items.first().and_then(Expr::as_ident) == Some("let")
                                && let_items.len() == 3
                            {
                                if let Some(name) = let_items[1].as_ident() {
                                    env_here.insert(name.to_string(), SimpleTy::NonI32);
                                }
                            }
                        }
                        out.push(e);
                    }
                    return Expr::List { items: out, ptr };
                }

                if head == "for" && items.len() == 5 {
                    let Some(var) = items[1].as_ident() else {
                        return Expr::List {
                            items: items.into_iter().map(|e| go(e, env)).collect(),
                            ptr,
                        };
                    };

                    let start = go(items[2].clone(), env);
                    let end = go(items[3].clone(), env);

                    let mut body_env = env.clone();
                    body_env.insert(var.to_string(), SimpleTy::I32);
                    let body = go(items[4].clone(), &body_env);

                    let (Expr::Int { value: s_i32, .. }, Expr::Int { value: e_i32, .. }) =
                        (&start, &end)
                    else {
                        return Expr::List {
                            items: vec![expr_ident("for"), expr_ident(var), start, end, body],
                            ptr,
                        };
                    };

                    if contains_set_to_var(&body, var) || contains_nested_for_var(&body, var) {
                        return Expr::List {
                            items: vec![expr_ident("for"), expr_ident(var), start, end, body],
                            ptr,
                        };
                    }

                    let s = *s_i32 as u32;
                    let e = *e_i32 as u32;
                    let iters = e.saturating_sub(s);
                    if iters > UNROLL_MAX_ITERS {
                        return Expr::List {
                            items: vec![expr_ident("for"), expr_ident(var), start, end, body],
                            ptr,
                        };
                    }

                    let init_head = if env.contains_key(var) { "set" } else { "let" };

                    let mut out: Vec<Expr> = Vec::new();
                    out.push(expr_ident("begin"));
                    out.push(expr_list(vec![
                        expr_ident(init_head),
                        expr_ident(var),
                        start,
                    ]));

                    for idx in 0..iters {
                        out.push(expr_list(vec![
                            expr_ident("begin"),
                            body.clone(),
                            expr_int(0),
                        ]));
                        if idx + 1 < iters {
                            out.push(expr_list(vec![
                                expr_ident("set"),
                                expr_ident(var),
                                expr_list(vec![expr_ident("+"), expr_ident(var), expr_int(1)]),
                            ]));
                        }
                    }

                    out.push(expr_int(0));

                    return Expr::List { items: out, ptr };
                }

                Expr::List {
                    items: items.into_iter().map(|e| go(e, env)).collect(),
                    ptr,
                }
            }
        }
    }

    go(expr, seed)
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
            if matches!(items.first().and_then(Expr::as_ident), Some("set" | "set0"))
                && items.len() >= 2
            {
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
            is_pure_i32_head(head) && items.len() == 3 && items[1..].iter().all(is_pure)
        }
    }
}

fn is_pure_i32_head(head: &str) -> bool {
    matches!(
        head,
        "+" | "-"
            | "*"
            | "/"
            | "%"
            | "&"
            | "|"
            | "^"
            | "<<u"
            | ">>u"
            | "="
            | "!="
            | "<"
            | "<="
            | ">"
            | ">="
            | "<u"
            | "<=u"
            | ">u"
            | ">=u"
            | "&&"
            | "||"
    )
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

pub(crate) fn inline_called_once_i32_pure(program: &mut Program) {
    const INLINE_MAX_NODES: usize = 12;

    fn module_id_of_fn(name: &str) -> &str {
        name.rsplit_once('.').map(|(m, _)| m).unwrap_or("")
    }

    fn is_inline_body_expr(expr: &Expr, params: &BTreeSet<String>) -> bool {
        match expr {
            Expr::Int { .. } => true,
            Expr::Ident { name, .. } => params.contains(name),
            Expr::List { items, .. } => {
                let Some(head) = items.first().and_then(Expr::as_ident) else {
                    return false;
                };
                is_pure_i32_head(head)
                    && items.len() == 3
                    && is_inline_body_expr(&items[1], params)
                    && is_inline_body_expr(&items[2], params)
            }
        }
    }

    fn collect_call_heads(expr: &Expr, out: &mut BTreeMap<String, usize>) {
        match expr {
            Expr::Int { .. } | Expr::Ident { .. } => {}
            Expr::List { items, .. } => {
                if let Some(head) = items.first().and_then(Expr::as_ident) {
                    *out.entry(head.to_string()).or_insert(0) += 1;
                }
                for it in items {
                    collect_call_heads(it, out);
                }
            }
        }
    }

    fn collect_call_heads_in_contracts(
        clauses: &[ContractClauseAst],
        out: &mut BTreeMap<String, usize>,
    ) {
        for clause in clauses {
            collect_call_heads(&clause.expr, out);
            for w in &clause.witness {
                collect_call_heads(w, out);
            }
        }
    }

    fn substitute_idents(expr: Expr, mapping: &HashMap<String, String>) -> Expr {
        match expr {
            Expr::Int { .. } => expr,
            Expr::Ident { name, ptr } => {
                if let Some(dst) = mapping.get(&name) {
                    Expr::Ident {
                        name: dst.clone(),
                        ptr,
                    }
                } else {
                    Expr::Ident { name, ptr }
                }
            }
            Expr::List { items, ptr } => Expr::List {
                items: items
                    .into_iter()
                    .map(|e| substitute_idents(e, mapping))
                    .collect(),
                ptr,
            },
        }
    }

    struct InlineCand {
        module_id: String,
        param_names: Vec<String>,
        body: Expr,
    }

    let mut candidates: BTreeMap<String, InlineCand> = BTreeMap::new();
    for f in &program.functions {
        if !f.requires.is_empty() || !f.ensures.is_empty() || !f.invariant.is_empty() {
            continue;
        }
        if f.ret_ty != Ty::I32 {
            continue;
        }
        if f.params.iter().any(|p| p.ty != Ty::I32) {
            continue;
        }
        if f.body.node_count() > INLINE_MAX_NODES {
            continue;
        }
        let params: BTreeSet<String> = f.params.iter().map(|p| p.name.clone()).collect();
        if !is_inline_body_expr(&f.body, &params) {
            continue;
        }
        candidates.insert(
            f.name.clone(),
            InlineCand {
                module_id: module_id_of_fn(&f.name).to_string(),
                param_names: f.params.iter().map(|p| p.name.clone()).collect(),
                body: f.body.clone(),
            },
        );
    }
    if candidates.is_empty() {
        return;
    }

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    collect_call_heads(&program.solve, &mut counts);
    for f in &program.functions {
        collect_call_heads(&f.body, &mut counts);
        collect_call_heads_in_contracts(&f.requires, &mut counts);
        collect_call_heads_in_contracts(&f.ensures, &mut counts);
        collect_call_heads_in_contracts(&f.invariant, &mut counts);
    }
    for f in &program.async_functions {
        collect_call_heads(&f.body, &mut counts);
        collect_call_heads_in_contracts(&f.requires, &mut counts);
        collect_call_heads_in_contracts(&f.ensures, &mut counts);
        collect_call_heads_in_contracts(&f.invariant, &mut counts);
    }

    let mut to_inline: BTreeSet<String> = BTreeSet::new();
    for name in candidates.keys() {
        if counts.get(name).copied().unwrap_or(0) == 1 {
            to_inline.insert(name.clone());
        }
    }
    if to_inline.is_empty() {
        return;
    }

    fn rewrite_expr(
        expr: Expr,
        caller_module: &str,
        cands: &BTreeMap<String, InlineCand>,
        remaining: &mut BTreeSet<String>,
    ) -> Expr {
        match expr {
            Expr::Int { .. } | Expr::Ident { .. } => expr,
            Expr::List { items, ptr } => {
                let Some(head) = items.first().and_then(Expr::as_ident) else {
                    return Expr::List {
                        items: items
                            .into_iter()
                            .map(|e| rewrite_expr(e, caller_module, cands, remaining))
                            .collect(),
                        ptr,
                    };
                };

                if remaining.contains(head) {
                    let Some(cand) = cands.get(head) else {
                        return Expr::List { items, ptr };
                    };
                    if cand.module_id != caller_module {
                        // Same-module only.
                    } else if items.len() == 1 + cand.param_names.len() {
                        let mut used: BTreeSet<String> = BTreeSet::new();
                        collect_idents(
                            &Expr::List {
                                items: items.clone(),
                                ptr: String::new(),
                            },
                            &mut used,
                        );

                        let mut mapping: HashMap<String, String> =
                            HashMap::with_capacity(cand.param_names.len());
                        let mut lets: Vec<Expr> = Vec::with_capacity(cand.param_names.len());

                        for (idx, param) in cand.param_names.iter().enumerate() {
                            let tmp = fresh_name(&mut used, "__x07_inl");
                            mapping.insert(param.clone(), tmp.clone());
                            lets.push(expr_list(vec![
                                expr_ident("let"),
                                expr_ident(tmp),
                                rewrite_expr(
                                    items[1 + idx].clone(),
                                    caller_module,
                                    cands,
                                    remaining,
                                ),
                            ]));
                        }

                        let body = substitute_idents(cand.body.clone(), &mapping);

                        let mut out = Vec::with_capacity(2 + lets.len());
                        out.push(expr_ident("begin"));
                        out.extend(lets);
                        out.push(body);

                        remaining.remove(head);
                        return Expr::List { items: out, ptr };
                    }
                }

                Expr::List {
                    items: items
                        .into_iter()
                        .map(|e| rewrite_expr(e, caller_module, cands, remaining))
                        .collect(),
                    ptr,
                }
            }
        }
    }

    fn rewrite_contracts(
        clauses: &mut [ContractClauseAst],
        caller_module: &str,
        cands: &BTreeMap<String, InlineCand>,
        remaining: &mut BTreeSet<String>,
    ) {
        for clause in clauses {
            clause.expr = rewrite_expr(clause.expr.clone(), caller_module, cands, remaining);
            let witness = std::mem::take(&mut clause.witness);
            clause.witness = witness
                .into_iter()
                .map(|e| rewrite_expr(e, caller_module, cands, remaining))
                .collect();
        }
    }

    let mut remaining = to_inline.clone();
    program.solve = rewrite_expr(program.solve.clone(), "main", &candidates, &mut remaining);
    for f in &mut program.functions {
        let caller_module = module_id_of_fn(&f.name).to_string();
        f.body = rewrite_expr(f.body.clone(), &caller_module, &candidates, &mut remaining);
        rewrite_contracts(&mut f.requires, &caller_module, &candidates, &mut remaining);
        rewrite_contracts(&mut f.ensures, &caller_module, &candidates, &mut remaining);
        rewrite_contracts(
            &mut f.invariant,
            &caller_module,
            &candidates,
            &mut remaining,
        );
    }
    for f in &mut program.async_functions {
        let caller_module = module_id_of_fn(&f.name).to_string();
        f.body = rewrite_expr(f.body.clone(), &caller_module, &candidates, &mut remaining);
        rewrite_contracts(&mut f.requires, &caller_module, &candidates, &mut remaining);
        rewrite_contracts(&mut f.ensures, &caller_module, &candidates, &mut remaining);
        rewrite_contracts(
            &mut f.invariant,
            &caller_module,
            &candidates,
            &mut remaining,
        );
    }

    let inlined: BTreeSet<String> = to_inline.difference(&remaining).cloned().collect();

    if inlined.is_empty() {
        return;
    }

    program.functions.retain(|f| !inlined.contains(&f.name));
}

#[cfg(test)]
mod tests {
    use crate::ast::Expr;
    use crate::program::{FunctionDef, FunctionParam, Program};
    use crate::types::Ty;
    use crate::x07ast::ContractClauseAst;

    use super::{
        const_fold, cse_pure_subexpressions, dce_unused_lets, expr_ident, expr_int, expr_list,
        inline_called_once_i32_pure, licm_bytes_len, strength_reduce, unroll_small_fors,
        LocalTyEnv, SimpleTy,
    };

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
    fn dce_positive_removes_unused_let_i32() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let expr = expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("let"),
                expr_ident("t"),
                expr_list(vec![expr_ident("+"), expr_ident("x"), expr_int(1)]),
            ]),
            expr_int(0),
        ]);
        let mut seed = LocalTyEnv::new();
        seed.insert("x".to_string(), SimpleTy::I32);
        let out = dce_unused_lets(expr, &seed);
        assert_eq!(out, expr_int(0));
    }

    #[test]
    fn dce_regression_does_not_remove_non_i32_let() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let expr = expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![
                expr_ident("let"),
                expr_ident("t"),
                expr_list(vec![expr_ident("bytes.alloc"), expr_int(1)]),
            ]),
            expr_int(0),
        ]);
        let out = dce_unused_lets(expr.clone(), &LocalTyEnv::new());
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
    fn strength_reduce_positive_pow2_ops() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let mul = expr_list(vec![expr_ident("*"), expr_ident("x"), expr_int(8)]);
        assert_eq!(
            strength_reduce(mul),
            expr_list(vec![expr_ident("<<u"), expr_ident("x"), expr_int(3)])
        );

        let div = expr_list(vec![expr_ident("/"), expr_ident("x"), expr_int(8)]);
        assert_eq!(
            strength_reduce(div),
            expr_list(vec![expr_ident(">>u"), expr_ident("x"), expr_int(3)])
        );

        let rem = expr_list(vec![expr_ident("%"), expr_ident("x"), expr_int(8)]);
        assert_eq!(
            strength_reduce(rem),
            expr_list(vec![expr_ident("&"), expr_ident("x"), expr_int(7)])
        );
    }

    #[test]
    fn strength_reduce_regression_does_not_touch_non_pow2() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let mul = expr_list(vec![expr_ident("*"), expr_ident("x"), expr_int(7)]);
        assert_eq!(strength_reduce(mul.clone()), mul);

        let div = expr_list(vec![expr_ident("/"), expr_ident("x"), expr_int(3)]);
        assert_eq!(strength_reduce(div.clone()), div);

        let rem = expr_list(vec![expr_ident("%"), expr_ident("x"), expr_int(3)]);
        assert_eq!(strength_reduce(rem.clone()), rem);
    }

    #[test]
    fn inline_positive_called_once_pure_i32_is_inlined_and_removed() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let mut program = Program {
            functions: vec![FunctionDef {
                name: "main.inc".to_string(),
                requires: Vec::new(),
                ensures: Vec::new(),
                invariant: Vec::new(),
                params: vec![FunctionParam {
                    name: "x".to_string(),
                    ty: Ty::I32,
                    brand: None,
                }],
                ret_ty: Ty::I32,
                ret_brand: None,
                body: expr_list(vec![expr_ident("+"), expr_ident("x"), expr_int(1)]),
            }],
            async_functions: Vec::new(),
            extern_functions: Vec::new(),
            solve: expr_list(vec![expr_ident("main.inc"), expr_int(7)]),
        };

        inline_called_once_i32_pure(&mut program);

        assert!(program.functions.is_empty(), "expected helper removed");
        assert_eq!(
            program.solve,
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![
                    expr_ident("let"),
                    expr_ident("__x07_inl0"),
                    expr_int(7),
                ]),
                expr_list(vec![expr_ident("+"), expr_ident("__x07_inl0"), expr_int(1),]),
            ])
        );
    }

    #[test]
    fn inline_regression_called_twice_is_not_inlined() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let inc = FunctionDef {
            name: "main.inc".to_string(),
            requires: Vec::new(),
            ensures: Vec::new(),
            invariant: Vec::new(),
            params: vec![FunctionParam {
                name: "x".to_string(),
                ty: Ty::I32,
                brand: None,
            }],
            ret_ty: Ty::I32,
            ret_brand: None,
            body: expr_list(vec![expr_ident("+"), expr_ident("x"), expr_int(1)]),
        };
        let solve = expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![expr_ident("main.inc"), expr_int(1)]),
            expr_list(vec![expr_ident("main.inc"), expr_int(2)]),
        ]);
        let mut program = Program {
            functions: vec![inc],
            async_functions: Vec::new(),
            extern_functions: Vec::new(),
            solve: solve.clone(),
        };

        inline_called_once_i32_pure(&mut program);

        assert_eq!(program.functions.len(), 1);
        assert_eq!(program.solve, solve);
    }

    #[test]
    fn inline_regression_contracts_prevent_inlining() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let inc = FunctionDef {
            name: "main.inc".to_string(),
            requires: vec![ContractClauseAst {
                id: Some("r0".to_string()),
                expr: expr_int(1),
                witness: Vec::new(),
            }],
            ensures: Vec::new(),
            invariant: Vec::new(),
            params: vec![FunctionParam {
                name: "x".to_string(),
                ty: Ty::I32,
                brand: None,
            }],
            ret_ty: Ty::I32,
            ret_brand: None,
            body: expr_list(vec![expr_ident("+"), expr_ident("x"), expr_int(1)]),
        };
        let solve = expr_list(vec![expr_ident("main.inc"), expr_int(7)]);
        let mut program = Program {
            functions: vec![inc],
            async_functions: Vec::new(),
            extern_functions: Vec::new(),
            solve: solve.clone(),
        };

        inline_called_once_i32_pure(&mut program);

        assert_eq!(program.functions.len(), 1);
        assert_eq!(program.solve, solve);
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

    #[test]
    fn unroll_positive_small_const_range_unrolls() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let body = expr_list(vec![
            expr_ident("set"),
            expr_ident("sum"),
            expr_list(vec![expr_ident("+"), expr_ident("sum"), expr_ident("i")]),
        ]);
        let expr = expr_list(vec![
            expr_ident("for"),
            expr_ident("i"),
            expr_int(0),
            expr_int(4),
            body.clone(),
        ]);

        let out = unroll_small_fors(expr, &LocalTyEnv::new());

        let inc = expr_list(vec![
            expr_ident("set"),
            expr_ident("i"),
            expr_list(vec![expr_ident("+"), expr_ident("i"), expr_int(1)]),
        ]);
        assert_eq!(
            out,
            expr_list(vec![
                expr_ident("begin"),
                expr_list(vec![expr_ident("let"), expr_ident("i"), expr_int(0)]),
                expr_list(vec![expr_ident("begin"), body.clone(), expr_int(0)]),
                inc.clone(),
                expr_list(vec![expr_ident("begin"), body.clone(), expr_int(0)]),
                inc.clone(),
                expr_list(vec![expr_ident("begin"), body.clone(), expr_int(0)]),
                inc,
                expr_list(vec![expr_ident("begin"), body, expr_int(0)]),
                expr_int(0),
            ])
        );
    }

    #[test]
    fn unroll_regression_large_range_is_not_unrolled() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let expr = expr_list(vec![
            expr_ident("for"),
            expr_ident("i"),
            expr_int(0),
            expr_int(9),
            expr_int(0),
        ]);
        let out = unroll_small_fors(expr.clone(), &LocalTyEnv::new());
        assert_eq!(out, expr);
    }

    #[test]
    fn unroll_regression_body_assigns_loop_var_is_not_unrolled() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let expr = expr_list(vec![
            expr_ident("for"),
            expr_ident("i"),
            expr_int(0),
            expr_int(4),
            expr_list(vec![expr_ident("set"), expr_ident("i"), expr_int(0)]),
        ]);
        let out = unroll_small_fors(expr.clone(), &LocalTyEnv::new());
        assert_eq!(out, expr);
    }

    #[test]
    fn unroll_regression_uses_set_when_loop_var_already_bound() {
        // REGRESSION: x07.rfc.backlog.optimizer@0.1.0
        let expr = expr_list(vec![
            expr_ident("begin"),
            expr_list(vec![expr_ident("let"), expr_ident("i"), expr_int(99)]),
            expr_list(vec![
                expr_ident("for"),
                expr_ident("i"),
                expr_int(0),
                expr_int(2),
                expr_int(0),
            ]),
            expr_ident("i"),
        ]);
        let out = unroll_small_fors(expr, &LocalTyEnv::new());

        let Expr::List { items, .. } = out else {
            panic!("expected outer begin");
        };
        let Expr::List {
            items: unrolled, ..
        } = &items[2]
        else {
            panic!("expected unrolled loop");
        };
        let Expr::List { items: init, .. } = &unrolled[1] else {
            panic!("expected init");
        };
        assert_eq!(init[0].as_ident(), Some("set"));
    }
}
