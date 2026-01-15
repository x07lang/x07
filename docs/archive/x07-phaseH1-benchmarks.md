Here are the **concrete Phase H1 benchmark files** (repo‑style, `solve-pure`) designed so that **running H1 + H2** is enough going forward (you don’t need to keep re-running earlier phase suites separately). The **H1 suite carries forward “canary” tasks** for core byte/loop semantics (Phase B/C/D/F class) *and* adds **ABI/type‑exercising tasks** for Box/Vec/Option/Result/views/interfaces.

ABI reminder (for your H1 ABI docs/CI): Rust’s default (“Rust”) representation intentionally **does not guarantee field order/layout beyond minimal soundness properties**, so ABI stability needs an explicit spec (your X07 ABI v1) rather than relying on Rust defaults. ([Rust Documentation][1])

See files:
benchmarks/solve-pure/phaseH1-debug-suite.json
benchmarks/solve-pure/phaseH1-suite.json
benchmarks/solve-pure/phaseH1-smoke.json

## What’s inside (high level)

### `phaseH1-smoke.json` (fast gate)

Small set of “does the H1 surface + ABI plumbing still work?” tasks:

* `smoke/echo`
* `smoke/box_select_reverse` (Box + move semantics encouraged)
* `smoke/vec_dup4` (**max_realloc_calls = 0**)
* `smoke/view_extract_window` (view/slice window extraction; **memcpy/realloc/peak** assertions)
* `smoke/result_try_gate` (Result + try/early return encouraged)
* `smoke/interface_dispatch_checksum_u8` (interface/vtable encouraged)

### `phaseH1-suite.json` (full H1 ladder, solves + performance shaping)

Includes two buckets:

#### A) “Carry-forward canaries” (so you don’t re-run older phases)

These cover the **core solve‑pure semantics** that historically regress when rewrite rules churn:

* `canary/echo`
* `canary/reverse_bytes`
* `canary/sum_u8_u32le`
* `canary/max_u8_or_empty`
* `canary/replace_byte_u8`
* `canary/filter_eq_u8`
  Includes a **4096‑byte alternating payload** case with:

  * `max_realloc_calls: 0`
  * `max_memcpy_bytes: 4096`
  * `max_peak_live_bytes: 65536`
* `canary/find_first_u8_u32le`
* `canary/count_runs_u8_u32le`

These are intentionally “Phase F‑class” but rewritten as a **stable H1 baseline** so tuning can’t “forget” basic loop/bytes patterns while optimizing for later phases.

#### B) H1 ABI/type exercisers (Box/Vec/Result/Option/View/Interface)

* `h1/box_select_reverse` (Box: borrow + move patterns)
* `h1/vec_dup4` (Vec capacity planning; **realloc=0** targets)
* `h1/result_try_gate` (Result/try early return encouraged)
* `h1/option_box_maybe_reverse` (Option<Box<bytes>> usage encouraged)
* `h1/view_first_line_len_u32le` (**max_memcpy_bytes=64**, **peak** bounded)
* `h1/view_extract_window`
  Includes a **16KB payload** case extracting a 4KB window with:

  * `max_memcpy_bytes: 8192`
  * `max_realloc_calls: 0`
  * `max_peak_live_bytes: 32768`
* `h1/interface_dispatch_checksum_u8` (interface/vtable encouraged)

### `phaseH1-debug-suite.json` (debug-only borrow/alloc safety checks)

Small set that runs with `debug_stats_required` and asserts:

* `max_borrow_violations: 0`

This suite is meant to be a **CI/regression guard** (and a canary pre-score gate if you choose), not necessarily a scoring-heavy tuning target.

## How this satisfies your “run only H1 + H2 going forward” goal

* **H1** now includes the “old-phase core” you keep needing (pure byte/loop correctness patterns + memory shaping on selected tasks).
* **H2** (the suites you already generated/added) covers:

  * stdlib parity modules (text/json/regex/csv/map/set/prng),
  * deterministic worlds (fs/rr/kv/full),
  * IO streaming + buffering,
  * concurrency + replay + sched stats,
  * “mini-app” integration ladders.

So your default tuning bundle can become:

* `benchmarks/solve-pure/phaseH1-suite.json`
* plus the Phase H2 suites (pure/fs/rr/kv/full) you already have

…and you stop running older phases as separate benchmark suites, while still keeping **coverage via canary tasks**.

If you want a single “bench bundle” JSON that lists all H1+H2 suites in one place, use:

* `benchmarks/bundles/phaseH1H2.json` (or `benchmarks/bundles/phaseH1.json` / `benchmarks/bundles/phaseH2.json`)

[1]: https://doc.rust-lang.org/reference/type-layout.html "Type layout - The Rust Reference"
