# Scaled pilot (12 tasks, 4 arms) — 2026-06-12

Second same-day run after the first pilot: suite extended 6 → 12 tasks
(added dedupe_lines, fnv1a32, frame_u32le, csv_sum_col, reverse_lines,
second_largest), runner extended with `rust` and `x07text` arms. Subject is
still ONE frontier agent (Claude Fable 5) in one warm session — the
cross-vendor cold-start run in RUNBOOK.md remains the decision-grade
experiment. What this run adds is a controlled within-subject comparison of
the new DX tooling.

## Results

| arm | pass@1 (12 tasks) | total solution bytes | vs python |
|---|---|---|---|
| python | 12/12 | 2,007 | 1.0x |
| rust | 12/12 | 5,360 | 2.7x |
| x07 (JSON) | 12/12 | 7,626 | 3.8x |
| x07text | 12/12 | 8,125 (pretty-printed) | 4.0x; **1.5x rust** |

The six NEW tasks were authored natively in x07text and passed on the
first harness run — zero repair iterations, using `x07 doc` behavioral
summaries for every stdlib call (split_lines_view trailing-newline
semantics, `std.parse.u32_dec_at` to-end-of-view parsing,
`std.u32.push_le`, small_map encodings). The six old-task x07text files are
mechanical `to-text` conversions of the first pilot's JSON (not fresh
authorship) and are labeled as such.

## Within-subject delta (the tooling effect)

Same subject, same task style, one day apart:

| condition | first-attempt task pass rate | notes |
|---|---|---|
| morning baseline (no summaries, no fuzzy doc, JSON authoring) | 1 task: failed twice before passing | silent semantic bug + borrow error |
| first pilot (fuzzy doc + summaries partly landed, JSON authoring) | 4/6 | both failures: `==` vs `=` |
| this run (full summaries + did-you-mean + x07text authoring) | 6/6 on unseen tasks | zero iterations |

This is the strongest internal evidence so far that the friction was
tooling- and surface-shaped rather than intrinsic — but it is confounded by
warm context and accumulating subject experience, so treat it as a
hypothesis the scaled run must confirm, not a conclusion.

## Other observations

- Rust's `json_name` is larger than x07's (1,191 vs 592 bytes): single-file
  rustc has no serde, so JSON scanning was hand-rolled; x07's
  `std.json.extract_path_canon_or_err` covered it. Stdlib breadth cuts both
  ways across arms; vector-judged tasks keep this honest.
- Wrapping `*` (two's-complement i32) verified deterministic in x07 —
  FNV-1a needs no special handling beyond writing the offset basis as its
  i32 bit pattern (-2128831035), which the prompt's u32 framing makes
  non-obvious; the summaries don't cover operator semantics (the guide
  does).
- `x07text` measured ~6% larger than single-line JSON in bytes, but it is
  pretty-printed (indentation); its token cost for LLM emission is roughly
  half of JSON's quote/bracket overhead per the RFC 0001 estimate. Byte
  size is reported because it is what the harness can measure objectively.

## Files

- `results/scaled-{python,rust,x07,x07text}.json` — raw runs.
- `solutions/claude-pilot/` — all four arms' solutions; `.x07t` files for
  the six new tasks are the authored originals.
