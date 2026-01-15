# X07 string/text notes (regex + slices)

Status: implemented. Normative contracts: `docs/text/regex-v1.md`, `docs/text/x7sl-v1.md`.

From the two modules you attached:

* `std.regex-lite` is a **purposefully tiny** literal matcher/counting helper.
* `ext.regex` is a **real regex engine** with a compile step and an exec step, returning a small “doc bytes” result that can encode either **ERR(code,pos)** or **OK(is_match,start,end)**.

## What you already have is a good foundation for agentic + production use

The biggest “production risk” with regex isn’t feature count — it’s **runtime blowups** from backtracking engines (ReDoS / catastrophic backtracking). It’s common enough to have its own CWE (CWE‑1333). ([CWE][1])

Your `ext.regex` is structured like an **automata/NFA-style engine** (parse → postfix → NFA compile → simulate), which is the same family of approach promoted by Russ Cox as “simple and fast” and used by non-backtracking engines like RE2. That approach is specifically used to avoid pathological exponential backtracking behavior. ([Swtch][2])

This is also aligned with Rust’s `regex` philosophy: Rust deliberately **does not support look-around or backreferences** (features that push you toward backtracking and can break time guarantees) and instead uses an automata-based approach. ([Docs.rs][3])

So: you’re already pointed in the “right direction” for **100% autonomous agents** (predictable, bounded, safe).

## What `ext.regex` appears to support (based on the module you attached)

Your engine has the classic “usable subset”:

* **Literals**
* **Dot** / “any byte” token
* **Anchors**: beginning/end (it has `_tok_bol` / `_tok_eol`)
* **Grouping** for precedence
* **Alternation** (`|`)
* **Concatenation**
* **Quantifiers**: `*`, `+`, `?`
* **Bounded repeat** (there’s a `_op_repeat` / parse errors like invalid repeat)
* **Character classes** `[...]` and parse errors for invalid/unclosed class
* **Escapes** and **builtin classes** (you explicitly build classes for `\d \D \w \W \s \S` + common escapes like `\n \r \t`)

It also already has **agent-friendly result accessors**:

* `ext.regex.is_err / err_code / err_pos`
* `ext.regex.is_match / match_start / match_end / match_len`

And the encoding is simple and stable:

* **ERR doc**: tag=0 + u32 code + u32 pos
* **OK doc**: tag=1 + byte is_match + u32 start + u32 end

That’s a *great* design for agents (no manual offset slicing required just to check results).

## Status (in-tree)

The string/text + regex pieces called out below are now implemented with pinned contracts:

* X7SL v1 slice lists: `docs/text/x7sl-v1.md` + `std.text.slices` (emitted by `std.text.ascii.split_*`, `ext.regex.{find_all_x7sl_v1,split_v1}`, and `ext.unicode.unicode_grapheme_slices` OK payload).
* Regex v1: `docs/text/regex-v1.md` (captures, exec-from, find-all, split, replace, opts).

## The biggest “is it enough?” answer

It’s **enough** for:

* log parsing, config parsing
* extracting tokens / scanning
* validating input formats
* most CLI/agent automation tasks
* deterministic fixture-world tests and OS-world usage where you still want predictable behavior

It’s **not yet “full-featured”** in the way people usually mean “PCRE-like regex,” and that’s a good thing for determinism. The right question is:

> Is it enough as a **safe RE2/Rust-regex-class engine**?

Mostly yes — but there are a few high-impact gaps that will matter for real-world agent-built programs.

## The highest-impact gaps to address next

### Gap 1: Capturing groups (the #1 usability gap)

Right now you return only whole-match `(start,end)`. Many real programs need to extract parts (e.g., parse `key=value` with groups).

**Recommendation (safe + deterministic):**
Add “captures v1” support, but keep it RE2/Rust-regex-class safe:

* Capturing parentheses allowed.
* No backreferences.
* No look-around.

This is consistent with the “finite automata” approach and is discussed as feasible even for NFA engines with submatch tracking. ([Swtch][2])

**Suggested minimal API:**

* `ext.regex.exec_caps_v1(compiled, text_view) -> bytes` returning either:

  * ERR(code,pos)
  * OK + overall match + `cap_count` + a fixed table of `(start,end)` per capture (u32/u32), using `0xFFFFFFFF` for “missing”.

Agents love this because they can do:

* compile once
* exec → get cap[1] cap[2]
* no slicing math

### Gap 2: “Exec from offset” + match iteration (find next / find all)

Agents often implement find-all loops. If you force them to do subview math manually, they will break it.

**Add these two helpers:**

* `ext.regex.exec_from_v1(compiled, text_view, start_i32) -> bytes` (start/end are absolute)
* `ext.regex.find_all_x7sl_v1(compiled, text_view, max_matches_i32) -> bytes`

  * returns an X7SL v1 slice list of matches (see `docs/text/x7sl-v1.md`)

These are small, deterministic, and massively reduce agent error rate.

### Gap 3: Replace / split helpers (agent ergonomics)

You can build them from `find_all`, but agents will make mistakes (especially with empty matches).

**Minimal safe set:**

* `ext.regex.replace_all_v1(compiled, text_view, repl_bytes_view, cap_limit_i32) -> bytes`
* `ext.regex.split_v1(compiled, text_view, max_parts_i32) -> bytes`

  * returns an X7SL v1 slice list of segments (see `docs/text/x7sl-v1.md`)

### Gap 4: Flags/options must be explicit

Case-insensitive, multiline, dotall are extremely common, and agents will assume them.

**Do NOT overload syntax with inline flags at first.**
Instead:

* `ext.regex.compile_opts_v1(pattern_view, opts_u32) -> bytes`

  * `OPTS_CASEI`, `OPTS_MULTILINE`, `OPTS_DOTALL`
* Keep `compile()` as opts=0.

### Gap 5: Unicode semantics (only if you truly need it)

Right now it’s a **byte regex engine** with ASCII-like classes. That’s fine for many workloads.
But if you want “text regex” over UTF‑8 codepoints and Unicode categories, it becomes much bigger:

* boundary handling
* case folding
* `\p{…}` properties

**Recommendation:** keep `ext.regex` explicitly *byte-based* in v1, and add a separate “utf8 regex” later, or keep Unicode support shallow (only `\p{ASCII}`-like subsets).

## What you should NOT add (and why)

To keep agentic coding safe and predictable, avoid features that push you toward backtracking or explosive behavior:

* **Backreferences** (`\1`, `\k<name>`)
* **Look-around** (`(?=...)`, `(?!...)`, `(?<=...)`, `(?<!...)`)
* **Catastrophic backtracking–prone constructs** in a backtracking engine

This matches the deliberate design constraints of Rust regex and RE2-style engines. ([Docs.rs][3])
And it directly reduces the CWE-1333 / ReDoS class of production issues. ([CWE][1])

## One critical spec item to pin now: match semantics

Your implementation calls `_match_longest_from`, which strongly suggests **leftmost-longest** behavior (POSIX-style) rather than the more common “leftmost-first (Perl-like)” behavior.

Neither is “wrong,” but agents will get confused unless you pin it.

**Action:**

* Add `docs/net/regex-v1.md` (or `docs/text/regex-v1.md`)
* Include:

  * supported syntax
  * match semantics (leftmost-longest vs leftmost-first)
  * empty-match rules (how “find all” advances to avoid infinite loops)
  * all error codes + meaning
  * hard limits (max pattern bytes, max NFA states, max captures)

Russ Cox explicitly discusses both POSIX and Perl submatch rules in automata approaches; if you add captures later, you must choose and pin one. ([Swtch][2])

## So… do you need to “improve it”?

If your target is “PCRE compatibility,” then yes, you’re missing a lot — but I **would not** recommend going that direction for X07’s goals.

If your target is “production-ready for autonomous agents” with predictable behavior, then:

* Your current `ext.regex` is already a strong base.
* The improvements you should make are **mostly ergonomics + spec pinning**, not fancy regex features.

### The top 5 improvements (highest ROI for agents)

1. **Pin the regex-v1 contract** (syntax + semantics + limits + error codes).
2. **exec_from_v1** (offset-aware matching).
3. **find_all_u32le_v1** (match iteration without manual slicing).
4. **captures v1** (even just numbered capture groups).
5. **compile_opts_v1** (explicit flags).

[1]: https://cwe.mitre.org/data/definitions/1333.html?utm_source=chatgpt.com "CWE-1333: Inefficient Regular Expression Complexity (4.19)"
[2]: https://swtch.com/~rsc/regexp/?utm_source=chatgpt.com "Implementing Regular Expressions"
[3]: https://docs.rs/regex/latest/regex/?utm_source=chatgpt.com "regex - Rust"
+++++
Here’s what you have today (from the attached x07AST JSON modules), what’s *still missing* for “production‑ready text/string” work, and a concrete design + dev plan to close the gaps while keeping everything **LLM-friendly, deterministic-by-default, and easy to use without hand‑rolled binary slicing**.

## 1) What you already have (quick review)

### `std.text.ascii` (from `ascii.x07.json`)

Exports a good “ASCII utilities core”:

* Character predicates + case transforms:

  * `is_alpha`, `is_space_tab`
  * `to_lower`, `to_upper`
* Line and delimiter processing:

  * `normalize_lines(bytes) -> bytes` (CR/LF handling, trims space+tab ends)
  * `split_lines_view(bytes_view) -> bytes` (emits **slice table** of line ranges)
  * `split_u8(bytes_view, delim_u8) -> bytes` (emits **slice table**)
  * `first_line_view / kth_line_view / last_line_view` (returns `bytes_view`)
* Tokenization:

  * `tokenize_words_lower(bytes_view) -> bytes` (produces lowercase words separated by spaces)

**Strengths**

* Uses `bytes_view` heavily (good for G1 memory model).
* Many functions are deterministic and avoid per-byte `bytes.concat` loops by using `vec_u8` builders.

**Gap**

* Some outputs are “structured bytes” (slice tables) but there’s no *standardized* format contract (magic/version/count) and no “agent-friendly accessors” (`count`, `view_at`, etc.), so agents will end up doing manual offset math.

---

### `std.text.utf8` (from `utf8.x07.json`)

Exports:

* `count_codepoints_or_neg1(bytes_view) -> i32`
* `is_valid(bytes_view) -> i32`
* `validate_or_empty(bytes) -> bytes`

**Strengths**

* Implements *real UTF‑8 validation logic* and codepoint counting.
* Uses the modern UTF‑8 constraints (4‑byte max, excludes invalid leading bytes like C0/C1 and F5..FF, etc.), aligned with RFC 3629’s constraints. ([RFC Editor][1])

**Gaps**

* API is *too minimal* for real-world work:

  * no `next_cp` iterator / decode-at-offset helper
  * sentinel returns (`-1`, `empty`) are convenient but ambiguous in production (empty can be valid; `-1` as error is okay but non-extensible)
* No explicit error codes / `result_*` wrappers.

---

### `ext.unicode` (from `unicode.x07.json`)

Exports a lot, including:

* Encoding conversions:

  * `decode_{latin1,utf16le,utf16be,win1252}_to_utf8`
  * `encode_utf8_to_{utf16le,utf16be,win1252}`
* Unicode-ish utilities:

  * `unicode_nfkc_basic(bytes_view) -> bytes` (returns a doc with ok/err)
  * `unicode_grapheme_slices(bytes_view) -> bytes` (returns slice table / doc)
  * `utf8_decode_u32le(bytes_view) -> bytes` and `encode_utf8_u32le(bytes_view) -> bytes`
* “Doc accessors”:

  * `unicode_is_err`, `unicode_err_code`, `unicode_get_bytes`

**Strengths**

* This is already the start of a “Unicode extension package”.
* Provides grapheme slicing (very useful for user-visible segmentation), and conversion tooling.

**Critical gap**

* `unicode_nfkc_basic` is explicitly a *basic* NFKC-ish subset, not full Unicode normalization.

  * Full normalization forms (NFC/NFD/NFKC/NFKD) are specified by UAX #15. ([Unicode][2])
* Grapheme segmentation has a spec in UAX #29; you should decide whether your implementation is “best-effort subset” or aims to match UAX #29 rules. ([Unicode][3])

---

### `ext.aho_corasick` (from `aho_corasick.x07.json`)

Exports:

* `compile(bytes_view) -> bytes` (doc)
* `find_first(compiled, hay) -> bytes` (doc)
* doc accessors: `is_err`, `err_code`, `match_start`, `match_end`, `match_pat_ix`

**Strengths**

* Aho-Corasick is the right primitive for **multi-pattern** search; extremely useful in log scanning, tokenization, protocol parsing, etc.

**Gap**

* You need a pinned contract for:

  * “needles encoding”
  * “compiled blob format versioning”
  * match result doc encoding
* And you want higher-level helpers so agents don’t parse offsets manually.

---

### `ext.byteorder` (from `byteorder.x07.json`)

Exports read/write for `u16/u32` LE/BE.

**Strengths**

* Essential for binary protocols.

**Gap**

* Potential API overlap/confusion with existing `std.codec/std.u32` style modules.
* Needs a single canonical surface so agents don’t guess “which one is the blessed one”.

---

## 2) The biggest gaps to fill (to make this “production-ready” for agentic coding)

### Gap A — No single “canonical” facade for text

Right now you effectively have *three* entry points:

* `std.text.ascii`
* `std.text.utf8`
* `ext.unicode`

That is fine for humans, but agents benefit from “one way”:

* `std.text` (facade) should be the *default import*; it re-exports stable v1 APIs and hides internal complexity.

### Gap B — Missing pinned **binary formats** for structured outputs

You already return structured blobs:

* slice tables (start/len pairs)
* doc blobs (tag + payload; with error code accessors)

But you haven’t pinned:

* `magic + version + count + row encoding`
* required sorting properties (if any)
* failure modes and error codes

Without these, agents will keep “hand-rolling” encodings and make mistakes.

### Gap C — Error model is inconsistent across modules

Examples:

* UTF-8 uses `-1` sentinel or “empty bytes”
* Unicode uses a tagged doc blob + accessor helpers
* Aho-Corasick uses doc blob + helpers

For production, standardize on:

* `result_i32` / `result_bytes` (your typed result system)
* and keep convenience wrappers (like `validate_or_empty`) as thin layers on top.

### Gap D — Unicode completeness policy isn’t pinned

You need an explicit stance:

* **Tier 0**: ASCII / UTF‑8 validation + “basic normalization” (what you already have)
* **Tier 1**: “Unicode correctness v1”:

  * normalization forms per UAX #15 ([Unicode][2])
  * grapheme segmentation per UAX #29 ([Unicode][3])
  * case folding based on Unicode’s published case folding data (CaseFolding.txt / related UCD files)

If you don’t pin this, you’ll end up with subtle, untestable differences across packages.

### Gap E — Streaming-friendly text utilities

For “real programs”, most text is read from:

* filesystem streams
* HTTP response streams
* DB row streams

You want text APIs that work directly on:

* `bytes_view` windows from `std.io.bufread` (fill/consume model)

That’s how you keep memcpy/realloc low and make big inputs feasible.

---

## 3) Proposed design (LLM-first, small canonical surface, scalable)

### 3.1 Add a **facade module**: `std.text`

**Goal:** agents import one module and use 95% of needed operations.

`std.text` should:

* re-export ASCII basics and UTF‑8 validation/counting
* expose a small set of *named v1 contracts* for structured outputs
* provide “do the right thing” helpers:

  * e.g. `text.lines_slices_v1(view)` → standardized slice-table
  * `text.utf8_validate_v1(bytes)` → `result_bytes` or `result_i32`

### 3.2 Standardize a slice-table contract: `X7SL` (“x07 Slice List”) v1

You already emit `(start,len)` pairs; formalize it as:

* Header:

  * `magic = "X7SL"` (4 bytes ASCII)
  * `version_u32le = 1`
  * `count_u32le`
* Rows (count times):

  * `start_u32le`
  * `len_u32le`

**Then standardize helper functions** so agents never do math:

* `std.text.slices.count_v1(x7sl) -> i32`
* `std.text.slices.start_v1(x7sl, idx) -> i32`
* `std.text.slices.len_v1(x7sl, idx) -> i32`
* `std.text.slices.view_at_v1(base_view, x7sl, idx) -> bytes_view`
* `std.text.slices.copy_at_v1(base_view, x7sl, idx) -> bytes` (copy)

This *massively* reduces agent errors.

### 3.3 Standardize doc/result encodings

If you already have typed `result_bytes`/`result_i32`, use them.

* `std.text.utf8.validate_v1(bytes)` should return `result_bytes`:

  * `Ok(original_bytes)` or `Err(TEXT_ERR_UTF8_INVALID)`
* Keep `validate_or_empty` as a wrapper.

Same for unicode and aho:

* Wrap existing doc blobs with `result_*` helpers.
* Or pin doc format as v1 and keep accessors; but don’t mix styles long-term.

### 3.4 Unicode policy: Two tiers

Because full Unicode is huge and changes by version, define:

**Tier “basic” (already implemented)**

* UTF‑8 validation per RFC 3629 ([RFC Editor][1])
* “NFKC basic” mapping (explicitly documented as *not* full UAX #15) ([Unicode][2])
* best-effort grapheme slicing

**Tier “full” (roadmap)**

* full normalization forms NFC/NFD/NFKC/NFKD per UAX #15 ([Unicode][2])
* grapheme segmentation aligned to UAX #29 rules ([Unicode][3])
* case folding using Unicode’s published casefold data (CaseFolding.txt), pinned to a Unicode version

### 3.5 Canonical surface vs internal modules

Adopt the same pattern you used for collections (“facade + internal impls”):

* **Public:** `std.text.*` (small, stable)
* **Internal:** `std.text._ascii_impl`, `std.text._unicode_impl`, `ext.unicode.*` etc.
* The guide + skills should strongly prefer the facade.

---

## 4) Development plan (concrete, implementable steps)

### Milestone T0 — “Make existing APIs usable by agents”

1. **Pin the X7SL v1 format**

* Add `docs/text/x7sl-v1.md`
* Implement `std.text.slices` with pack/unpack/accessors
* Update:

  * `std.text.ascii.split_lines_view` to return X7SL v1
  * `std.text.ascii.split_u8` to return X7SL v1
  * `ext.unicode.unicode_grapheme_slices` to return X7SL v1 (or wrap it)

2. **Normalize error handling**

* Add `std.text.utf8.validate_v1 -> result_bytes`
* Add `std.text.utf8.count_codepoints_v1 -> result_i32`
* Keep old functions for compatibility, but document them as “convenience”.

3. **Hide `ext.byteorder` behind a canonical module**

* Either:

  * re-export it as `std.byteorder` and stop listing `ext.byteorder` in public docs
  * or move the canon surface into `std.codec` (`read_u16_le_at`, `write_u16_le`, `read_u32_be_at`, …)

### Milestone T1 — “Streaming text (big inputs)”

Add a small streaming layer that works with your existing `std.io.bufread`:

* `std.text.stream.read_line_x7sl_v1(bufread_handle) -> result_bytes`

  * returns X7SL slices referencing the current fill window (or copies as needed)
* `std.text.stream.read_lines_collect_v1(reader_iface, max_bytes) -> result_bytes`

  * returns one big bytes + X7SL for lines (agent-friendly)
* `std.text.stream.split_ascii_ws_x7sl_v1(view) -> bytes` (X7SL)

This unlocks building CLIs and servers that parse big text without copying.

### Milestone T2 — “Unicode correctness v1”

Decide and pin:

* Unicode version used for tables
* Supported operations: normalization + case folding + grapheme segmentation
* Exact error codes for invalid UTF‑8 / invalid sequences / unsupported code points

Then implement:

* `std.text.unicode.nfkc_v1(bytes_view) -> result_bytes`
* `std.text.unicode.casefold_v1(bytes_view) -> result_bytes`
* `std.text.unicode.graphemes_x7sl_v1(bytes_view) -> result_bytes` (X7SL or Err)

Reference the normative definitions:

* UAX #15 for normalization forms ([Unicode][2])
* UAX #29 for grapheme boundaries ([Unicode][3])
* Case folding data contract (CaseFolding.txt / UCD)

### Milestone T3 — “Search & regex”

1. Wrap Aho-Corasick in a *fully pinned* contract:

* needles encoding v1
* compiled blob format versioning (or keep opaque but versioned)
* match result format v1
* agent-friendly helpers:

  * `ac.compile_v1(patterns_x7sl_or_lines)`
  * `ac.find_first_v1(compiled, hay_view) -> result_bytes`
  * accessors (start/end/pat_ix) you already have

2. Regex

* Decide if `regex-lite` remains the only regex, or create:

  * `std.text.regex` with a deterministic subset (no backtracking explosions)
* Provide “literal fast paths” via Aho where possible.

### Milestone T4 — “Encoding utilities”

High-impact missing pieces for real apps:

* `std.text.base64` (encode/decode)
* `std.text.hex` (encode/decode)
* `std.text.percent` (URL percent encode/decode)
* `std.text.idna` (optional; for international domains)

These can be added as separate packages if you want to keep `std.text` minimal.

---

## 5) What I’d change right now in your existing modules (low-risk, high impact)

1. **Replace “raw slice-pairs bytes” outputs with X7SL v1 everywhere**

* `split_lines_view`, `split_u8`, `unicode_grapheme_slices`

2. **Add agent-facing accessors (don’t make them decode tables)**

* Without this, “structured bytes” will *always* be a source of agent bugs.

3. **Unify “success/error” style**

* UTF‑8: stop using “empty bytes = invalid” as the primary contract; keep it as a convenience wrapper.
* Prefer `result_*` so error space can grow safely and unambiguously.

4. **Declare the Unicode completeness tier explicitly**

* “basic NFKC” is fine, but it must be documented as such (not full UAX #15). ([Unicode][2])
* Grapheme segmentation should be declared “matches UAX #29” or “best-effort subset”; if you claim full, add conformance tests from UAX #29 test data. ([Unicode][3])

---

## 6) If you want the *next step* as a drop‑in bundle

[1]: https://www.rfc-editor.org/rfc/rfc3629 "RFC 3629:  UTF-8, a transformation format of ISO 10646 "
[2]: https://unicode.org/reports/tr15/ "UAX #15: Unicode Normalization Forms"
[3]: https://www.unicode.org/reports/tr29/ "UAX #29: Unicode Text Segmentation"
++++

Here’s the **X7SL v1 drop‑in bundle for `stdlib/std/0.1.1/`** (docs + new module + patched `std.text.ascii` + pure smoke suite + fixtures):

Use docs/phases/assets/x7sl_v1_bundle_stdlib_0.1.1.tar.gz.

## What’s inside (paths are repo‑relative)

### 1) Normative pinned spec

* `docs/text/x7sl-v1.md`
  Defines **X7SL v1** encoding:

  * header: `X7SL` + `version_u32_le=1` + `count_u32_le`
  * rows: `count × (start_u32_le, len_u32_le)`
  * exact length rule: `12 + count*8`
  * deterministic validation error codes for `std.text.slices.validate_v1`

### 2) New stdlib helper module

* `stdlib/std/0.1.1/modules/std/text/slices.x07.json`
  Exports:

  * Builder:

    * `std.text.slices.builder_new_v1(cap_hint)` → `vec_u8`
    * `std.text.slices.builder_push_v1(out,start,len)` → `vec_u8`
    * `std.text.slices.builder_finish_v1(out,count)` → `bytes`
  * Validation / accessors:

    * `std.text.slices.validate_v1(x7sl)` → `result_i32` (`OK(count)` or `ERR(code)`)
    * `std.text.slices.count_v1(x7sl)` → `i32` (count or `-1` on invalid)
    * `std.text.slices.start_v1(x7sl,idx)` → `i32`
    * `std.text.slices.len_v1(x7sl,idx)` → `i32`
    * `std.text.slices.view_at_v1(base_view,x7sl,idx)` → `bytes_view`
    * `std.text.slices.copy_at_v1(base_view,x7sl,idx)` → `bytes` (explicit copy)

### 3) Patched ASCII module to emit X7SL v1 (header+rows)

* `stdlib/std/0.1.1/modules/std/text/ascii.x07.json`
  Updated functions:

  * `std.text.ascii.split_lines_view` now returns **X7SL v1 bytes** (instead of “raw u32 pairs”)
  * `std.text.ascii.split_u8` now returns **X7SL v1 bytes**

Consumers should use `std.text.slices.*` helpers (validate/count/start/len/view_at/copy_at) instead of manual offset math.

### 4) Tiny pure smoke suite + fixtures (asserts X7SL bytes exactly)

* `benchmarks/solve-pure/x7sl-v1-smoke.json`
* Fixtures:

  * `benchmarks/fixtures/pure/solve-pure/x7sl-v1@0.1.1/...`

Tasks included:

* `text_ascii_split_lines_x7sl_v1`

  * cases:

    * `simple_lf` (`b"a\\nb\\n"`)
    * `crlf_and_last_line` (`b"a\\r\\nb\\r\\nc"`)
* `text_ascii_split_u8_x7sl_v1`

  * case:

    * `simple` (`b"a,b,,c"`)

All expected outputs are **byte-for-byte X7SL v1** (including magic/version/count and exact start/len values).

## Status (in-tree)

X7SL v1 is implemented in-tree:

* Normative spec: `docs/text/x7sl-v1.md`
* Stdlib: `stdlib/std/0.1.1/modules/std/text/slices.x07.json`
* Producers:

  * `std.text.ascii.split_lines_view`
  * `std.text.ascii.split_u8`
  * `ext.unicode.unicode_grapheme_slices` (OK doc payload is X7SL v1)
* Bench smoke suite: `benchmarks/solve-pure/x7sl-v1-smoke.json`
  * fixtures: `benchmarks/fixtures/pure/solve-pure/x7sl-v1@0.1.1/`

The archived bundles live under `docs/phases/assets/x7sl_v1_*`.

The **incremental X7SL v1 bundle** (also applied in-tree) adds:

* **(A)** makes it explicit in **`docs/spec/language-guide.md`** (and your **guide generator**) that **all “slice list outputs” are X7SL v1**, and
* **(B)** adds **one more X7SL task** covering the edgecase matrix: **empty input / leading separator / trailing separator / consecutive separators**.

Incremental bundle archive: `docs/phases/assets/x7sl_v1_incremental_bundle_stdlib_0.1.1.tar.gz`

---

## What’s inside the bundle

### 1) Updated X7SL smoke suite (bumped to `@0.1.1`) + new edgecase task

**File:**

* `benchmarks/solve-pure/x7sl-v1-smoke.json`

**Changes:**

* `suite_id`: `solve-pure/x7sl-v1@0.1.1`
* `fixture_root`: `benchmarks/fixtures/pure/solve-pure/x7sl-v1@0.1.1`
* Adds new task:

**New task id:**

* `text_ascii_split_u8_x7sl_v1_edgecases`

**Semantics being asserted:**

* Split by newline `0x0A` (`\n`) using `std.text.ascii.split_u8`
* Include empty segments (because split emits `separators + 1` segments)
* Output is **X7SL v1 bytes**

**Cases included (edgecase matrix):**

* `empty` → `""`
* `no_sep` → `"ABC"`
* `leading_sep` → `"\nA"`
* `trailing_sep` → `"A\n"`
* `consecutive_sep` → `"A\n\nB"`
* `only_sep` → `"\n"`

### 2) Fixture tree for the new suite id

**Directory:**

* `benchmarks/fixtures/pure/solve-pure/x7sl-v1@0.1.1/`

This bundle includes:

* copies of the prior `@0.1.0` X7SL fixtures (so `@0.1.1` is self-contained)
* the **new edgecase fixtures** under:

`benchmarks/fixtures/pure/solve-pure/x7sl-v1@0.1.1/text_ascii_split_u8_x7sl_v1_edgecases/`

Each case has:

* `<case>.in` (raw bytes)
* `<case>.out` (expected X7SL bytes)

---

## Guide + guide-generator updates

X7SL v1 is documented for both the spec guide and the generated guide:

* `docs/spec/language-guide.md` describes X7SL v1 and `std.text.slices`.
* `crates/x07c/src/guide.rs` matches the same contract.

---

## Notes on semantics (why this bites the right edgecases)

The new task uses **newline as separator**, so it naturally covers:

* **empty lines** (consecutive separators)
* **leading separator** (leading empty segment)
* **trailing separator** (trailing empty segment)

…and it does so without introducing any new “special” logic beyond your existing `split_u8` semantics.
