//! Deterministic near-match ranking for unknown symbol/module queries.
//!
//! Used by typecheck unknown-callee diagnostics and `x07 doc` not-found
//! suggestions so agents get "did you mean" candidates instead of dead ends.

/// Maximum edit distance accepted for a leaf-segment match, relative to the
/// query leaf length. Below this the candidate is considered "near".
fn leaf_threshold(leaf_len: usize) -> usize {
    (leaf_len / 3).max(1)
}

fn leaf(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(_, l)| l)
}

fn prefix(name: &str) -> &str {
    name.rsplit_once('.').map_or("", |(p, _)| p)
}

/// Bounded Levenshtein distance; returns `None` when the distance exceeds `cap`.
fn levenshtein_capped(a: &str, b: &str, cap: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > cap {
        return None;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur: Vec<usize> = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        let mut row_min = cur[0];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
            row_min = row_min.min(cur[j + 1]);
        }
        if row_min > cap {
            return None;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    (prev[b.len()] <= cap).then_some(prev[b.len()])
}

/// Score a candidate against the query; lower is better, `None` means "not a
/// plausible suggestion". Deterministic for stable diagnostics. Exact leaf
/// matches rank first, then leaf typos (edit distance), then substring hits,
/// then whole-name typos.
fn score(query: &str, candidate: &str) -> Option<usize> {
    if candidate == query {
        return None;
    }
    let q_leaf = leaf(query);
    let c_leaf = leaf(candidate);
    let same_prefix = !prefix(query).is_empty() && prefix(query) == prefix(candidate);
    let prefix_bonus = usize::from(!same_prefix);

    if c_leaf == q_leaf {
        return Some(prefix_bonus);
    }
    let cap = leaf_threshold(q_leaf.len());
    if let Some(d) = levenshtein_capped(q_leaf, c_leaf, cap) {
        return Some(2 + 2 * d + prefix_bonus);
    }
    // Substring hits serve keyword discovery ("split" -> split_u8). Reverse
    // containment needs a length guard so tiny leaves ("lit") don't match
    // unrelated long queries.
    let forward = q_leaf.len() >= 3 && c_leaf.contains(q_leaf);
    let reverse = q_leaf.contains(c_leaf) && c_leaf.len() * 2 >= q_leaf.len();
    if forward || reverse {
        return Some(8 + prefix_bonus);
    }
    // Whole-name distance catches wrong-module-path queries like
    // `std.byte.len` -> `std.bytes.len`.
    let cap = (query.len() / 4).clamp(1, 3);
    levenshtein_capped(query, candidate, cap).map(|d| 12 + 2 * d)
}

/// Rank `candidates` by similarity to `query`, returning at most `max` names,
/// best first. Ties break lexicographically so output is deterministic.
pub fn rank_similar<'a, I>(query: &str, candidates: I, max: usize) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut scored: Vec<(usize, &str)> = candidates
        .into_iter()
        .filter_map(|c| score(query, c).map(|s| (s, c)))
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    scored.truncate(max);
    scored.into_iter().map(|(_, c)| c.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typo_in_leaf_is_suggested() {
        let names = ["std.bytes.len", "std.bytes.eq", "std.vec.len"];
        let got = rank_similar("std.bytes.lenn", names, 3);
        assert_eq!(got.first().map(String::as_str), Some("std.bytes.len"));
    }

    #[test]
    fn exact_leaf_in_other_module_ranks_first() {
        let names = ["std.text.ascii.split_u8", "std.bytes.eq"];
        let got = rank_similar("std.text.split_u8", names, 3);
        assert_eq!(
            got.first().map(String::as_str),
            Some("std.text.ascii.split_u8")
        );
    }

    #[test]
    fn substring_leaf_matches() {
        let names = ["std.text.ascii.split_u8", "std.text.ascii.split_lines_view"];
        let got = rank_similar("std.text.ascii.split", names, 3);
        // Equal scores fall back to lexicographic order for determinism.
        assert_eq!(
            got,
            vec![
                "std.text.ascii.split_lines_view".to_string(),
                "std.text.ascii.split_u8".to_string(),
            ]
        );
    }

    #[test]
    fn unrelated_names_are_not_suggested() {
        let names = ["std.json.encode", "std.prng.next_u32"];
        assert!(rank_similar("std.bytes.lenn", names, 3).is_empty());
    }

    #[test]
    fn exact_match_is_excluded() {
        let names = ["std.bytes.len"];
        assert!(rank_similar("std.bytes.len", names, 3).is_empty());
    }

    #[test]
    fn output_is_deterministic_and_capped() {
        let names = ["a.f1", "a.f2", "a.f3", "a.f4"];
        let got = rank_similar("a.f", names, 2);
        assert_eq!(got, vec!["a.f1".to_string(), "a.f2".to_string()]);
    }
}
