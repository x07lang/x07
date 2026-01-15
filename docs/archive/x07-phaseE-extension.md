Yes — **if those stdlib packages are “baseline” in Phase E (meaning: you expect the solver + tuning to depend on them in later phases), you should add Phase E benchmarks for them.** Otherwise two bad things happen:

1. **They won’t be “real” dependencies**: the solver can ignore them and still pass Phase E, so you’ll discover later (Phase F+) that the APIs are missing, confusing, or too expensive.

2. **They won’t be stable**: without benchmark pressure, tuning can accidentally degrade their ergonomics/perf (or the module/import story) and you won’t notice until much later.

Below is the practical way to do it without exploding suite size or accidentally turning Phase E into “Phase D again”.

---

## The key principle: Phase E benchmarks should be “stdlib contract + composability” (not deep algorithms)

Phase E is about *modules/packages/stdlib being usable and reproducible*. So Phase E benchmarks for these packages should:

* be **short tasks** that strongly prefer using the new module/function,
* assert **byte-exact outputs**, which forces you to define determinism rules (ordering, formatting),
* be **multi-module** (imports required) to validate the module system and lockfile story,
* be **cheap** (few cases, small inputs) but with holdouts to prevent hardcoding.

---

## Text / JSON: you *must* benchmark determinism, or you can’t safely “assert bytes”

### Why JSON needs Phase E benchmarks

JSON objects are *unordered* by spec (“An object is an unordered collection…”). ([IETF Datatracker][1])
So if you want a benchmark that checks exact bytes, you need a canonical representation rule. RFC 8785 (JCS) exists precisely to produce deterministic JSON byte sequences using deterministic property sorting and strict serialization rules. ([RFC Editor][2])

### Minimal Phase E text/json benchmarks (recommended)

Add **3–4 tasks** that force the solver to import and use `std.text.ascii` / `std.text.utf8` / `std.json`:

1. **`text.normalize_lines_ascii`**

   * Input: ASCII bytes containing spaces + `\r\n`/`\n`, trailing spaces, blank lines
   * Output: normalized lines joined with `\n`, trimming ASCII whitespace, removing empty lines
   * Why it matters: exercises `split_lines`, `trim`, `join` in one place (very common later in FS/HTTP fixtures)

2. **`utf8.validate_or_empty`**

   * Input: arbitrary bytes
   * Output: if valid UTF‑8 → echo input; else → empty bytes
   * Why it matters: you need a crisp “UTF‑8 validity contract” early, because later worlds will feed you text-y fixtures

3. **`json.canonicalize_small`**

   * Input: JSON object (no floats at first; integers/strings/bools/null/arrays/objects), keys in random order
   * Output: canonical JSON bytes (sorted keys, minimal whitespace) per your chosen rule (I strongly recommend “JCS subset”)
   * Why it matters: locks down deterministic printing and avoids “tests fail because key order differs” (since object member order is not semantically meaningful). ([IETF Datatracker][1])

4. **Optional: `json.get_path_or_err`**

   * Input: JSON + “path” encoded simply (e.g., `key1\0key2\0`)
   * Output: value bytes or error code string
   * Why it matters: forces `Result`-style propagation while also testing JSON error behavior

### Regex-lite (optional) — only benchmark it if you constrain it to be safe

If you include `std.regex-lite`, you should **only** do so as a non-backtracking / RE2-like subset, because backtracking regex engines can have catastrophic worst-case behavior; linear-time engines exist specifically to avoid that.

So:

* **If regex-lite is included**: add 1 benchmark that would kill backtracking engines (e.g., nested quantifiers), and assert it runs within strict fuel.
* **If regex-lite isn’t included yet**: do *not* add benchmarks; keep it “optional later” to avoid becoming the Phase E blocker.

---

## Data structures: yes — you need benchmarks mainly to lock down ordering + determinism

### Why maps/sets need explicit benchmarks

If you expose a “hash map” that iterates in arbitrary order, you will instantly get nondeterministic outputs unless you also define ordering rules.

Even Rust’s `HashMap` explicitly documents iteration in **arbitrary order**, and is **randomly seeded** by default. ([Rust Documentation][2])
So your Phase E baseline `std.map/std.set` should either:

* be **ordered** (B‑tree style), or
* define **sorted iteration** explicitly (iterate keys in sorted order), or
* define stable insertion order (but still then you must specify it and test it).

### Minimal Phase E DS benchmarks (recommended)

Add **2–3 tasks** that force `std.map/std.set` and check deterministic order:

1. **`map.word_freq_sorted`**

   * Input: ASCII “words” (space/newline separated)
   * Output: lines `word=count\n` sorted lexicographically by `word`
   * Why it matters: this forces stable map semantics + stable iteration/serialization, and later becomes a building block for “log parsing” tasks.

2. **`set.unique_lines_sorted`**

   * Input: newline-separated lines
   * Output: unique lines, sorted, joined with `\n`
   * Why it matters: stable ordering is mandatory for byte-exact outputs.

3. **`vec.pipeline_map_filter`**

   * Input: list of u8 (or packed u32s)
   * Output: filter+map pipeline result
   * Why it matters: ensures the solver learns the “Vec helpers” API and doesn’t rebuild loops manually.

> Note: since your current core still relies heavily on copying slices (`bytes.slice` macro expands to allocate+copy), you shouldn’t try to “reward zero-copy” in Phase E yet — that belongs to the later “views” milestone. 

---

## Error model: yes — because it prevents Phase D–style “semantic traps” and improves first_try_rate

You already saw that solver failures often come from **semantic mismatches** (“negative balance” checks under modulo arithmetic, return typing constraints, etc.). Phase E is a good time to lock down:

* `Option` vs `Result`
* canonical error codes (bytes)
* propagation patterns (macros help)

### Minimal Phase E error-model benchmarks (recommended)

Add **2 tasks**:

1. **`parse_i32_or_err`**

   * Input: ASCII bytes
   * Output: 4-byte LE i32 if parse ok, else `ERR` (or error code byte)
   * Why it matters: canonicalizes the “parse fails” behavior and forces structured error handling.

2. **`json_parse_or_err`**

   * Input: JSON bytes
   * Output: `OK` if valid, else `ERR`
   * Why it matters: ensures parsing failures are not “UB/panic/garbage output,” and teaches the solver to propagate errors correctly.

---

## How to integrate these into Phase E without blowing up the suite

### Recommended structure

Keep Phase E focused by splitting into **two suites**:

* **`benchmarks/solve-pure/phaseE-suite.json`**
  Focus: modules + packages + lockfile determinism + “multi-module compilation works”.

* **`benchmarks/solve-pure/phaseE-stdlib-suite.json`** (new)
  Focus: baseline stdlib contracts (text/json/ds/errors). Small, ~8–12 tasks.

Then update your suite-runner cascade so Phase E tuning runs:

* Stage 3 uses `phaseD-suite.json` as a **regression gate** (must pass to avoid losing Phase D best),
* plus `phaseE-suite.json` + `phaseE-stdlib-suite.json` as the **selection pressure** for Phase E improvements.

This is the cleanest way to **not lose Phase D** while still pushing the new capabilities.

### What not to do

* Don’t add 50+ stdlib tests as solver benchmarks.

  * Put **deep correctness** into CI “unit vectors” (prewritten X07 programs) instead.
  * Keep tuning benchmarks as “LLM-ergonomics + contract enforcement”.

---

## Short answer checklist (what to benchmark in Phase E)

* ✅ `std.text.ascii` / `std.text.utf8`: normalize lines, split/trim/join, utf8 validate (2–3 tasks)
* ✅ `std.json`: parse + canonicalize (1–2 tasks) — canonicalization matters because JSON objects are unordered ([IETF Datatracker][1])
* ✅ `std.map/std.set`: word frequency + unique lines, *sorted output required* (2 tasks) — because hash maps iterate in arbitrary order and can be randomly seeded ([Rust Documentation][2])
* ✅ `std.result/std.option`: parse-or-error + parse-json-or-error (2 tasks)
* ➕ `std.regex-lite`: only if it is non-backtracking / RE2-like; then add 1 “evil regex” benchmark to enforce linear-time behavior

---

**Suite files**

- Source JSON spec: `benchmarks/solve-pure/phaseE-stdlib-suite.source.json`
- Generated benchmark suite: `benchmarks/solve-pure/phaseE-stdlib-suite.json`
- Regenerate/check: `python3 benchmarks/solve-pure/generate_phaseE_stdlib_suite.py` / `python3 benchmarks/solve-pure/generate_phaseE_stdlib_suite.py --check`

Notes:

- The suite-runner enforces `assertions.required_imports` by checking that the candidate program calls the required symbol(s), so each task pins a concrete `std.*` function name.

[1]: https://datatracker.ietf.org/doc/html/rfc8259?utm_source=chatgpt.com "RFC 8259 - The JavaScript Object Notation (JSON) Data ..."
[2]: https://doc.rust-lang.org/std/collections/struct.HashMap.html?utm_source=chatgpt.com "HashMap in std::collections - Rust"
