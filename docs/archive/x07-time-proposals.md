# X07 time v1 (ext.time.*)

This phase lands a **single canonical**, **agent-friendly** external time stack plus a small deterministic tzdb adapter.

The deliverable is intentionally a package (not a language change): `packages/ext/x07-ext-time-rs/0.1.0/`.

## What’s implemented (in-tree)

### Package + modules

- Package manifest: `packages/ext/x07-ext-time-rs/0.1.0/x07-package.json`
- Modules (module root: `packages/ext/x07-ext-time-rs/0.1.0/modules`):
  - `ext.time.duration` — DurationDocV1 bytes contract + checked arithmetic.
  - `ext.time.rfc3339` — RFC3339 v1 parse/format + bracketed `[TZID]` suffix support.
  - `ext.time.civil` — CivilDocV1 conversions (unix seconds ↔ civil components).
  - `ext.time.instant` — InstantDocV1 helpers (same layout as DurationDocV1).
  - `ext.time.tzdb` — deterministic tzdb snapshot adapter (offset lookup).
  - `ext.time.os` — run‑OS adapters (now/sleep/local tzid).

### Pinned specs (v1 contracts)

These are the pinned contracts that agents should follow (no ad-hoc byte slicing):

- `docs/time/time-v1.md`
- `docs/time/duration-v1.md`
- `docs/time/rfc3339-v1.md`
- `docs/time/civil-v1.md`
- `docs/time/tzdb-v1.md`
- `docs/time/os-time-v1.md`

### Native tzdb backend + staging

`ext.time.tzdb` is backed by a small native adapter:

- Rust staticlib: `crates/x07-time-native/` (exports `ev_time_tzdb_*_v1` C ABI)
- ABI header: `crates/x07c/include/x07_time_abi_v1.h`
- Build + stage into `deps/x07/`:

```bash
./scripts/build_ext_time.sh
```

### Smoke verification

Run the full time smoke gate (pure + tzdb + run-os + run-os-sandboxed policy):

```bash
./scripts/ci/check_time_smoke.sh
```

Smoke suite JSONs live in `benchmarks/smoke/time-*.json`.

## World gating + sandbox policy

- Deterministic `solve-*` worlds:
  - `ext.time.duration`, `ext.time.rfc3339`, `ext.time.civil`, `ext.time.instant`, `ext.time.tzdb`
- Non-deterministic `run-os*` worlds:
  - `ext.time.os` only

In `run-os-sandboxed`, OS time is gated by `schemas/run-os-policy.schema.json#/properties/time`.
The time section includes:

- `time.enabled`
- `time.allow_wall_clock` (now)
- `time.allow_sleep` + `time.max_sleep_ms` (sleep)
- `time.allow_local_tzid` (local tzid)

Policy-denied behavior is **non-trapping** and uses deterministic error docs (see `docs/time/os-time-v1.md`).

## Editing workflow (agent-friendly)

### x07AST (`*.x07.json`)

For deeply nested x07AST JSON, use structured patches:

```bash
cargo run -p x07 -- ast apply-patch --in path.x07.json --patch patch.json --out path.x07.json --validate
```

See: `docs/dev/x07-ast.md`.

### x07import-generated sources

Some modules are generated from Rust sources under `import_sources/`.
When changing those, regenerate via:

```bash
cargo run -p x07import-cli -- batch --manifest import_sources/manifest.json
```

## Bundle assets (reference)

The drop-in reference bundles for this phase are kept under `docs/phases/assets/`:

- `docs/phases/assets/x07_ext_time_v1_bundle.tar.gz`
- `docs/phases/assets/x07_ext_time_v1_civil_delta_bundle.tar.gz`

Note: `local_tzid_v1` can be disallowed if you consider it fingerprinting.

---

## Historical proposal notes (pre-implementation)

The sections below are the original proposal text from before the v1 implementation landed.
They include bundle-internal paths and forward-looking ideas that may not match the in-tree implementation above.

### Change 1: introduce explicit “magic + version” for RFC3339 docs (via new v3, don’t break v2)

Your duration doc already has a version byte; your RFC3339 docs don’t.
For long-term stability, you really want **self-describing docs**:

* `X7TI` InstantV1
* `X7DU` DurationV1
* `X7ZT` ZonedDateTimeV1
* `X7LD` LocalDateTimeV1
* `X7TZ` tzdb “zone handle” if you ever add caching

This makes:

* validators trivial,
* errors better,
* future changes much safer.

### Change 2: fix `duration.sub_v1` underflow behavior (or document it as unsigned-only)

If you keep it “unsigned duration”, rename it to make that explicit (so agents don’t use it for instant diffs).

Otherwise:

* add `sub_signed_v1` that errors on underflow or returns signed.

### Change 3: formalize RFC9557 tzid suffix behavior

You already carry tzid bytes; pin the exact parse surface so users don’t invent incompatible variants. RFC 9557 is the right reference for bracketed identifiers.

### Change 4: pin `-00:00` semantics

RFC3339 explicitly calls this out; if your parse collapses it to `0`, you’ll break real-world semantics on roundtrip.

---

### Benchmarks / smoke tests (historical)

Even if you’re not doing Openx07lve now, you still want deterministic smoke suites.

### Pure smoke (no OS)

1. RFC3339 parse/format roundtrip

* cases:

  * `Z`
  * `+05:30`
  * `-00:00` (must preserve “unknown offset” semantics)
  * fractional seconds truncation/normalization
  * leap second `:60` behavior (your chosen rule)

2. TZ conversion (pure, pinned tzdb)

* fixed tzdb version (e.g. `2025c`)
* known timestamps around DST transitions:

  * America/Los_Angeles around spring forward + fall back
* test both directions:

  * instant→local
  * local→instant with each disambiguation mode

### run-os smoke (nondeterministic inputs, deterministic shape)

1. `now_instant_v1`:

* assert doc validates and unix_hi/lo within plausible bounds (not exact time).

2. `local_tzid_v1`:

* either empty or valid tzid that tzdb recognizes.

---

### Staged development plan (historical)

### TIME‑01: Spec + error spaces

* Write pinned docs:

  * `docs/time/instant-v1.md`
  * `docs/time/duration-v2.md` (or v1 if you can change now)
  * `docs/time/zdt-v1.md`
  * `docs/time/rfc3339-v3.md` (explicitly referencing RFC3339 + RFC9557)
  * `docs/time/tzdb-v1.md`
* Define non-overlapping error code ranges:

  * `SPEC_ERR_TIME_*` (parse/layout)
  * `TZDB_ERR_*` (unknown tzid, ambiguous, nonexistent)
  * `OS_TIME_ERR_*` (policy/unsupported)

### TIME‑02: `ext.time.instant` pure implementation

* Pack/unpack/accessors + add/sub/diff with duration

### TIME‑03: signed duration support

* normalize, checked arithmetic, constructors (`from_ms`, `from_s`, etc.)

### TIME‑04: tzdb snapshot + conversion shim

* Ship tzdb snapshot pinned (start with tzdb `2025c` for concreteness; you can update later).
* Implement tz conversion functions (deterministic) based on TZif rules.

### TIME‑05: RFC3339 v3 boundary codec

* parse_v3 returns `ZonedDateTimeV1` (includes instant, tzid, offset_known flag)
* format_v3 consumes that doc

### TIME‑06: run‑os adapter

* now/local tzid/sleep gated by policy

---

### “Full featured” scope (historical)

If you want to ship something end-users can rely on immediately, Time v1 should minimally guarantee:

* **Instant arithmetic** (add/sub/diff) with signed durations
* **RFC3339 parse/format** with correct `-00:00` + fractional seconds handling per your pinned rules
* **Timezone conversion using pinned tzdb** (instant↔local) with deterministic disambiguation
* **OS now()** (run‑os only), returning InstantV1

Then Time v2 can add:

* richer formatting patterns
* ISO8601 duration parsing
* weekday/week-number, etc.

---
Use docs/phases/assets/x07_ext_time_v1_bundle.tar.gz

### What’s in the bundle

#### 1) Docs (pinned v1 contracts)

* `docs/time/time-v1.md` — overview + module layout
* `docs/time/duration-v1.md` — **X7DU** bytes doc encoding + errors + canonical API
* `docs/time/rfc3339-v1.md` — RFC3339 parse/format contract + **X7TS** parse-doc layout
  (RFC3339 is the timestamp spec we’re targeting.) ([RFC Editor][1])
* `docs/time/tzdb-v1.md` — timezone offset lookup contract + pinned tzdb snapshot guidance
  (tzdb comes from IANA; snapshot should be pinned to a specific tzdb release like `2025c`.) ([IANA][2])
  (Zoneinfo files are TZif; RFC8536 is the canonical reference.) ([RFC Editor][3])
* `docs/time/os-time-v1.md` — run-os* adapters (now/sleep/local tzid) + policy gating

#### 2) Schema/policy patch

* `schemas/run-os-policy.time.section.json` — the full **time** section (with new knobs)
* `schemas/run-os-policy.time.patch.json` — JSON-patch ops to extend your existing `time` section with:

  * `allow_sleep`
  * `max_sleep_ms`
  * `allow_local_tzid`

Also includes ready-to-use example policies:

* `schemas/run-os-policy.time-allow-min.example.json`
* `schemas/run-os-policy.time-deny-now.example.json`

#### 3) Package skeleton (built directly on your current impls)

Package root:

* `packages/ext/x07-ext-time-rs/0.1.0/package.json`

Modules:

* `packages/ext/x07-ext-time-rs/0.1.0/modules/ext/time/duration.x07.json`
  → **copied directly** from your current `ext.time.duration`
* `packages/ext/x07-ext-time-rs/0.1.0/modules/ext/time/rfc3339.x07.json`
  → **copied directly** from your current `ext.time.rfc3339`, with one intentional breaking cleanup:

  * introduces **canonical** `parse_v1` + `format_v1` (64-bit unix seconds via lo/hi)
  * de-exports the old truncating `parse`/`format` and the explicit `*_v2` names
  * adds `format_doc_v1(doc)` helper so agents don’t need to re-plumb fields

New modules added (thin wrappers / hooks for the “full time package” pieces):

* `ext.time.instant` — instant as “duration-shaped” bytes doc + convenience wrappers
* `ext.time.tzdb` — timezone offset lookups against a pinned tzdb snapshot (native hook)
* `ext.time.os` — run-os* adapters (native hooks)

#### 4) Smoke suite JSON shapes + smoke programs

Pure smoke:

* `benchmarks/smoke/time-pure-smoke.json` (schema `x07.smoke_suite@0.1.0`)
* `tests/external_pure/time_smoke/src/main.x07.json`

  * validates:

    * RFC3339 parse/format roundtrip on a fixed timestamp
    * duration arithmetic normalization

Run‑OS smoke:

* `benchmarks/run-os/time-os-smoke.json` (schema `x07.bench_suite@0.1.0`)
* `tests/external_os/time_os_smoke/src/main.x07.json`

  * checks `now_instant_v1` shape + `sleep_ms_v1(0)` succeeds

Run‑OS‑sandboxed policy smoke:

* `benchmarks/run-os-sandboxed/time-policy-smoke.json`
* `tests/external_os/time_policy_deny_now/src/main.x07.json`

  * expects `now_instant_v1` returns an error doc under deny policy

---

### The key “production-ready” deltas this bundle bakes in

#### RFC3339: one canonical API (no 32-bit truncation)

RFC3339 timestamps routinely exceed 32-bit unix seconds; so `parse_v1`/`format_v1` are the only exported, canonical entry points now (internally they call your existing `parse_v2`/`format_v2`). ([RFC Editor][1])

#### Time zones: deterministic tzdb snapshot (not host OS zoneinfo)

A “full” timezone story requires DST transitions; the right source of truth is the IANA tz database. ([IANA][2])
To keep results deterministic across machines, `ext.time.tzdb` is specified as *pinned snapshot* backed by TZif-derived data (TZif reference: RFC8536). ([RFC Editor][3])

#### OS time: policy-gated and world-scoped

Anything that touches the wall clock, sleeps, or reads local tz config is isolated behind `ext.time.os` and extended policy knobs (`allow_sleep`, `max_sleep_ms`, `allow_local_tzid`). This keeps solve worlds deterministic while still unblocking real run-os agents.

---

### Native hooks you’ll need (names are pinned in README inside bundle)

To make `ext.time.os` and `ext.time.tzdb` actually work, implement these builtins (or equivalent wiring) in your runner/toolchain:

* `os.time.now_instant_v1() -> bytes`
* `os.time.sleep_ms_v1(ms:i32) -> i32`
* `os.time.local_tzid_v1() -> bytes`
* `os.time.tzdb_is_valid_tzid_v1(tzid:bytes_view) -> i32`
* `os.time.tzdb_offset_duration_v1(tzid:bytes_view, unix_lo:i32, unix_hi:i32) -> bytes`  (Duration doc)
* `os.time.tzdb_snapshot_id_v1() -> bytes`

The bundle’s docs specify the required encodings and the deterministic behavior expected.

---

[1]: https://www.rfc-editor.org/rfc/rfc3339.html "RFC 3339:  Date and Time on the Internet: Timestamps "
[2]: https://www.iana.org/time-zones "Time Zone Database"
[3]: https://www.rfc-editor.org/rfc/rfc8536.html "RFC 8536: The Time Zone Information Format (TZif)"
+++++
Here’s the **drop‑in delta bundle** that adds a fully working **`ext.time.civil`** module (CivilDocV1), pins its bytes contract, wires it into the ext‑time package manifest, and extends the existing pure smoke program to assert the civil doc **byte‑for‑byte via accessors**.

Use docs/phases/assets/x07_ext_time_v1_civil_delta_bundle.tar.gz

### What’s inside (historical)

### New pinned spec

* `docs/time/civil-v1.md`

  * Defines **CivilDocV1** encoding (tagged ok/err docs)
  * Defines error codes and the v1 API

### Updated time package overview

* `docs/time/time-v1.md`

  * Now explicitly lists `ext.time.civil`
  * Lists pinned spec files including `civil-v1.md`

### New module

* `packages/ext/x07-ext-time-rs/0.1.0/modules/ext/time/civil.x07.json`

  * Fully implemented x07AST JSON (no TODOs)
  * Implements:

    * `from_unix_s_v1(unix_lo, unix_hi, offset_s, nanos) -> CivilDocV1`
    * `from_ts_v1(ts_doc_view) -> CivilDocV1` (consumes `ext.time.rfc3339` doc)
    * `make_v1(year,month,day,hour,minute,second,nanos,offset_s) -> CivilDocV1`
    * `to_ts_v1(civ_doc_view) -> rfc3339-style ts doc (tzid_len=0)`
    * Accessors: `year_i32`, `month_i32`, …, `unix_s_lo_u32`, `unix_s_hi_u32`, etc.
    * Derived helper: `weekday_iso_v1` (Mon=1 … Sun=7)

### Package manifest update

* `packages/ext/x07-ext-time-rs/0.1.0/package.json`

  * Adds `ext.time.civil` to `modules`

### Smoke program update (pure)

* `tests/external_pure/time_smoke/src/main.x07.json`

  * Imports `ext.time.civil`
  * Adds deterministic checks:

    * `from_ts_v1(parse("2020-01-02T03:04:05Z"))` yields the expected civil fields
    * `make_v1(2020,1,2,3,4,5,0,0)` yields the expected unix seconds
    * `weekday_iso_v1 == 4` for that date
  * Still returns the same `OK\n` / `FAIL\n` blobs (so existing benchmark expectations remain stable)

### Included benchmark file (unchanged shape)

* `benchmarks/smoke/time-pure-smoke.json`

### Quick “how to apply” (historical)

1. Untar into your repo root (or copy the included paths over).
2. Ensure the ext‑time package build/publish step includes the updated `package.json`.
3. Run your existing pure smoke runner for `benchmarks/smoke/time-pure-smoke.json` (it should stay green, now exercising civil conversions too).

Similarly provide:

* `ext.time.civil.add_days_v1` / `add_seconds_v1` (safe arithmetic that keeps encoding canonical),
* a small `ext.time.civil.normalize_offset_v1` (optional: enforce offset multiple-of-60),
* and a `civil ↔ tzdb` helper (`from_instant_tzid_v1(unix,nanos,tzid,tzdb_snapshot)`), so agents don’t stitch tzdb + civil manually.
