## Purpose

X07 is intended for **100% agentic coding**. That only works if versioning is:

* **Deterministic** (same inputs ⇒ same outputs),
* **Machine-checkable** (agents can reason from contracts),
* **Upgradeable** (v1 → v2 without breaking everything),
* **Composable** (toolchain + stdlib + external packages + wire formats fit together).

This document defines the **normative** versioning rules for:

* packages (`packages/...`),
* stdlib distributions (`stdlib/std/<ver>/...`),
* wire/ABI/data encodings (`*_v1`, `X7SL v1`, `specbin_v1`, etc.),
* toolchain binaries (`x07c`, `x07`, runners),
* the lockfile (`x07.lock` / `stdlib.lock` style).

## Terms

* **Package version**: the SemVer version of a published package, e.g. `x07-ext-net@0.1.1`.
* **Contract version**: the version of a **bytes encoding** or **ABI** that must remain stable forever once published, e.g. `HttpReqV1`, `HeadersTableV1`, `X7SL v1`. Contract versions appear in function names as `_v1`, `_v2`, etc., and also in the bytes headers (“magic + version”).
* **Schema version**: the version string in `schema_version` fields for JSON artifacts/tool reports (x07AST, diagnostics, test reports). In Rust, canonical values live in `crates/x07-contracts`.
* **Facade API**: the agent-facing “single canonical way” function(s) that agents should call most of the time (usually **no suffix**), which internally selects a contract version.
* **Spec API**: explicit pack/unpack helpers for contracts (usually in `*.spec.*`) that are always suffixed (`*_v1` etc.) so agents don’t hand-roll encodings.

## Hard rule: published artifacts are immutable

Once a package version or stdlib snapshot is published, its contents **MUST NOT** be modified. Any change must be released as a new version. (This is a SemVer requirement, and it’s non-negotiable for agentic reproducibility.)

## Package SemVer policy

X07 package versions follow SemVer, with one additional rule for pre-1.0 versions.

### ≥ 1.0.0 packages

* **MAJOR**: any backward-incompatible change to the public API or behavior that clients might rely on.
* **MINOR**: backward-compatible additions.
* **PATCH**: backward-compatible bugfixes only.

### 0.y.z packages

For `0.y.z`, we adopt the Cargo-style compatibility convention:

* Changing **`y`** is treated as a **breaking** change (major-equivalent).
* Changing **`z`** is treated as a **non-breaking** change (minor/patch-equivalent).
* `0.0.z` is always breaking.

This keeps agent dependency resolution predictable during early rapid iteration.

## Contract versioning policy

Contracts are the “wire formats” and stable ABI/layout rules that allow **agents to never guess bytes**.

### Rules

1. **Every bytes contract MUST include a header with:**

   * a short **magic** (4 bytes),
   * a **contract major** (u8),
   * a **contract minor** (u8) (optional but recommended),
   * and a deterministic length/count (u32 LE) where relevant.

2. **Contract major versions (v1/v2/…) are forever.**

   * Once `*_v1` exists, you never change the meaning of its bytes layout or semantics.
   * If you need to change behavior or encoding: introduce `*_v2`.

3. **A contract version suffix is not the same as package versioning.**

   * Example: `ext.net.http.spec.req_pack_v1` can remain correct across package releases `0.1.0 → 0.1.1 → 0.2.0 → 1.0.0`.
   * Package versioning is about distribution and API surface; contract versioning is about stable encoding/ABI.

4. **Contract version upgrades are additive.**

   * You add `*_v2` alongside `*_v1`.
   * You keep `*_v1` for a long deprecation window.
   * You provide a migration story (see below).

## Facade vs spec: the “single canonical way” rule

To prevent agents from being confused by multiple ways to do the same thing:

### Spec APIs are explicit and suffixed

* `std.net.http.spec.req_get_v1(...)`
* `std.net.http.spec.req_pack_v1(...)`
* `std.text.slices.x7sl_pack_v1(...)`

These are low-level and version-pinned. Agents use them when they must build bytes precisely.

### Facade APIs are stable and unsuffixed

* `std.net.http.get(...)`
* `std.db.query(...)`
* `std.text.split_lines(...)`

Facade functions are the **recommended default** for agents. They:

* pick the best supported contract for the current package line,
* enforce canonicalization (sorting, normalization),
* apply safe defaults.

### When may a facade change its underlying contract?

Only on a **breaking package bump** (see SemVer policy above). That is:

* for `0.y.z`, changing `y` may switch facade from v1 → v2;
* for `≥1.0.0`, changing MAJOR may switch facade v1 → v2.

Within a compatible version line, facades must not silently change encodings.

## Deprecation policy for contract versions

When `*_v2` exists:

1. Keep `*_v1` supported for **at least**:

   * `>= 1.0`: two MINOR releases after v2 appears
   * `< 1.0`: two PATCH releases after v2 appears (or one `0.(y+1).0`, depending on cadence)

2. Add deterministic tooling support:

   * linter warning: “contract v1 is deprecated; prefer v2”
   * optional `x07 fix --upgrade-contracts` that rewrites known call sites

3. Remove `*_v1` only on:

   * package MAJOR bump (>=1.0), or
   * package `0.(y+1).0` bump (pre-1.0).

## How this relates to stdlib version directories

`stdlib/std/0.1.1/...` is a **distribution snapshot** of the standard library packages shipped together.

* It is versioned like a package set (SemVer-ish, but you can treat it like a platform release).
* It may contain modules that implement multiple contract versions (`*_v1`, `*_v2`) at the same time.
* Your lockfile pins which stdlib snapshot is in use.

### Rule

* Stdlib snapshots are immutable once released.
* Agents should typically target a pinned stdlib snapshot (`stdlib.lock`) to avoid drift.

## Lockfiles and determinism

All production builds must be driven by lockfiles:

* `x07.lock` (workspace lockfile): pins package versions + hashes.
* `stdlib.lock` (stdlib snapshot): pins bundled stdlib packages and hashes.
* `langdef.lock` (if LangDef is shipped as data): pins the language definition content hash.

A build is “deterministic” only if:

* toolchain version is pinned,
* lockfile is pinned,
* and the build uses canonical JSON and deterministic archives.

## Migration example: v1 → v2 without chaos

Suppose you shipped `HeadersTableV1` and later need to add a feature requiring v2.

1. Add:

   * `headers.pack_v2`, `headers.unpack_v2`, `headers.get_v2`, …
2. Keep:

   * all `*_v1` APIs.
3. In a breaking package bump:

   * update facade `headers.set(...)` to use v2 internally.
4. Provide:

   * a deterministic upgrader tool to rewrite agent code if needed.

## What version string should agents pin?

For agentic coding, recommend:

* pin **package versions** in `x07.lock`,
* rely on **facade APIs** by default,
* only use `*_v1` directly when producing/consuming bytes that are persisted or exchanged.

## Appendix: naming conventions

* Contracts:

  * `ThingV1` is the bytes/ABI format name.
  * `*_v1` functions operate on that format.
* Facades:

  * no suffix.
* Internal-only helpers:

  * prefix `_` and keep in an internal module namespace, e.g. `std.net.http._intern`.

---
