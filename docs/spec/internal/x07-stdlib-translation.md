Spec-ID: x07.spec.internal.x07-stdlib-translation@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

# Stdlib translation (`x07import`)

This document describes how `x07import` maps a restricted subset of Rust/C into X07 stdlib modules.

Canonical references:
- `docs/x07import/README.md` (commands + workflow)
- `import_sources/manifest.json` (what gets generated)
- `stdlib/std/0.1.1/modules/**` (checked-in generated modules)
- Memory model + ownership rules: `docs/spec/x07-memory-management.md` (ABI v2)

---

Below is a concrete design + development plan for a **stdlib import/translation toolchain** that helps you add X07 stdlib modules faster by reusing **reference implementations written in Rust or C**, while keeping your core project constraints intact (deterministic eval, C backend, pinned stdlib, reproducible builds).

I’m going to propose a **hybrid** approach:

* **Deterministic structural transpilation** for the “easy/regular” subset (most byte/text/parse utilities).
* **LLM-assisted fallback** for the “hard/irregular” parts (JSON corner cases, regex-lite, map/set balancing), but always behind **differential/property testing gates** so it can’t silently regress correctness.

This gives you *speed* without giving up determinism or correctness.

---

## What you should (and should not) try to translate

### Translate first

Modules whose semantics are basically:

* byte/slice iteration
* arithmetic on u8/u32
* deterministic parsing/printing
* deterministic data-structure logic with stable iteration order

These map well onto X07’s current strengths and your Phase E/F direction.

### Avoid “automatic translation” early

* Anything that depends on OS APIs, threads, time, filesystem scanning, randomness, floating point, locale
* Unsafe pointer arithmetic (unless restricted and heavily checked)
* Macros-heavy C that requires preprocessor semantics beyond simple expansion
* Rust code relying heavily on traits/generics/iterators (you can still translate it, but you’ll want a *lowering stage* first)

This is why the toolchain needs a **subset + lowering IR**.

---

## Core idea: “Import → Lower → Emit → Verify” pipeline

### Overview

```
Rust/C source  ──parse──►  Source AST  ──lower──►  x07IR  ──emit──►  .x07.json module (x07AST JSON)
                                            │
                                            └──verify──►  tests + differential + fuzz
```

Status (repo reality):
- Implemented as `x07import` (`crates/x07import-core/` + `crates/x07import-cli/`).
- Canonical output is x07AST JSON module files (`*.x07.json`), not legacy S-expr sources.

Where **x07IR** is a restricted, x07-friendly intermediate representation that:

* eliminates language sugar (iterators, pattern matching, macros)
* normalizes control flow (while/for/if)
* uses explicit byte access + builders (`vec_u8.with_capacity`, `vec_u8.extend_bytes`, etc.)
* avoids undefined behavior by construction

---

## Architecture and repo layout (concrete)

### Repo layout (current)

```
import_sources/                          # importer inputs
  manifest.json
  rust/**                                 # pinned Rust refs (source of truth for modules)
  c/**                                    # pinned C refs (if any)

crates/
  x07import-core/                         # frontends + x07IR lowering + diagnostics
  x07import-cli/                          # `x07import` CLI (batch regen + checks)

stdlib/std/0.1.1/                         # embedded stdlib package
  x07-package.json
  modules/**/*.x07.json                   # generated (manifest-listed) + hand-written modules

docs/x07import/                           # importer usage docs + diagnostics catalog
scripts/ci/check_x07import_generated.sh
scripts/ci/check_stdlib_lock.sh
stdlib.lock
```

### Why this structure works well for you

* Fits the “pinned stdlib” model (stdlib package is versioned and checked in).
* Produces deterministic artifacts that can be verified in CI.
* Keeps translation tooling separate from the compiler core.

---

## Frontend choices (Rust and C)

### Rust frontend options (pick two-tier)

#### Tier 1 (fast to ship): `syn`-based parsing

`syn` is explicitly meant to parse Rust tokens into a Rust syntax tree. ([Docs.rs][1])
Use this for:

* parsing standalone `.rs` files
* a restricted subset of Rust used for stdlib imports
* quick iteration

Pros:

* stable and easy in Rust ecosystem
* no need to run rustc internals

Cons:

* no type resolution, no MIR, no borrow/move info
* you must enforce a subset manually

#### Tier 2 (more power later): `rustc_driver` typed lowering

The Rust compiler dev guide describes `rustc_driver` as the main entry for running compiler phases and recommends using it where possible. ([Rust Compiler Development Guide][2])
Use this for:

* typed MIR extraction (or HIR)
* leveraging borrow/move analysis to ensure “safe subset”
* better mapping for slices, iterators, indexing, overflow semantics

This is how you eventually get “import more idiomatic Rust” with fewer manual restrictions.

Also relevant: Rust’s borrow checker is about moves/borrows and aliasing rules. You can exploit those analyses as part of your importer’s safety gating. ([Rust Compiler Development Guide][3])

---

### C frontend options (pick one)

#### Option A (recommended): libclang AST parsing

Clang’s libclang exposes a small API for parsing source into an AST and traversing it. ([Clang][4])
Use this for:

* small C files or preprocessed `.i` output
* robust handling of C syntax/decls

#### Option B (fallback): tree-sitter-c

Tree-sitter is an incremental parsing library (originally for editor use). ([Tree-sitter][5])
Use it when:

* you only need syntax structure
* you can demand that imported C is “macro-free and already normalized”

In practice: **use libclang** for real C, tree-sitter only for “simple snippets”.

---

## x07IR: the translation “truth layer”

### x07IR must be *more restrictive* than X07 itself

Because it acts as your “safe transpilation core.” It should include only:

* Scalars: `i32` (plus “u8 via i32 masked” conventions)
* Bytes: owning bytes and (later) view bytes if G1 is active
* Vec builders: your existing `vec_u8.*` primitives
* Control flow: `if`, `loop`, `break`, `continue` (or normalized to for)
* Calls: module-local calls only (no dynamic dispatch)
* Memory: only via the X07 runtime model (no raw pointers)

Then x07IR → `.x07.json` (x07AST JSON) is straightforward.

### Determinism requirements in IR

* Stable symbol naming (alpha rename to `f0`, `f1`, `l0`, `l1`)
* Stable ordering of emitted definitions (toposort call graph, then lexical)
* Stable formatting (one canonical emitter, no pretty variability)

This ensures: *same input source → same output `.x07.json`* (important for reproducible builds + reviewability).

---

## Verification strategy: you need “translation gates” more than fancy translation

The import tool is only useful if it’s safe.

### Gate 1: subset enforcement (reject early)

Before emitting x07IR, reject:

* recursion (unless explicitly allowed)
* floats
* panics/exceptions
* heap aliasing not representable
* platform APIs
* nondeterminism (random/time)

### Gate 2: differential testing (source vs X07)

For each imported function/module:

1. Compile & run the Rust/C reference implementation
2. Compile & run the generated X07 module
3. Compare outputs for a shared test vector generator

### Gate 3: property-based testing (fast bug discovery)

Property-based testing generates inputs automatically and shrinks failures to small cases, which is perfect for import pipelines. ([Altsysrq][6])

Use Rust `proptest` as the harness generator. ([Docs.rs][7])
Even if you don’t keep proptest long-term, it’s great for import validation.

### Gate 4: fuzzing (optional, but extremely valuable for parsers)

For modules like `std.json`, `std.utf8`, `std.regex-lite`, fuzzing is how you find the weird corner cases quickly.

`cargo-fuzz` is the common Rust wrapper around libFuzzer. ([Rust Fuzz][8])
(You already know fuzzing isn’t deterministic, so treat it as an offline/pre-merge gate, not an eval-time benchmark.)

### Gate 5: deterministic perf gates (X07 mem_stats)

Since your runtime already emits `mem_stats`, your importer should also enforce:

* no leaks
* bounded realloc
* bounded memcpy
* peak_live_bytes near output + slack (especially once views exist)

This aligns with your Phase F mem and Phase G1 goals.

---

## LLM-assisted import mode (optional, but likely high ROI)

A strict transpiler will hit walls (JSON, regex-lite, map/set). For those:

### “LLM patch” mode

1. Deterministic lowering fails, produces an “unsupported construct report”
2. Tool asks the LLM to rewrite the *source* into the supported subset

   * or asks the LLM to produce an x07IR fragment / `.x07.json` fragment
3. Everything still goes through the same **differential + property** gates

This makes the LLM a *productivity tool*, not a trusted compiler.

### Why differential testing matters here

Differential testing is a well-known idea in compiler testing: compare multiple implementations to catch wrong-code without needing a formal oracle. ([Users.cs.utah.edu][9])
You’re essentially doing “reference implementation as oracle.”

---

## Concrete developer workflow

### CLI commands (current)

See `docs/x07import/README.md`.

* Regenerate from manifest: `cargo run -p x07import-cli -- batch --manifest import_sources/manifest.json`
* Drift check (no writes): `./scripts/ci/check_x07import_generated.sh`
* Diagnostics sync: `./scripts/ci/check_x07import_diagnostics_sync.sh`
* Stdlib lockfile checks: `./scripts/ci/check_stdlib_lock.sh`

### Inputs that must be pinned

* upstream module source version (git rev/tag)
* importer tool version
* x07c version
* stdlib.lock + hashes

So “regenerate stdlib” is reproducible.

---

## Development plan as PR-sized milestones (no ambiguity)

Note: the repo implementation landed as `crates/x07import-core/` + `crates/x07import-cli/` (not the `x07-translator*` names in this historical draft plan).

### PR 1 — Skeleton and golden-test infrastructure

* Add `crates/x07-translator/` with:

  * x07IR types
  * `.x07.json` emitter stub
  * golden snapshot test harness
* Add `translator/README.md` and basic scripts
* CI:

  * `cargo test -p x07-translator`
  * golden tests: ensure stable emission

### PR 2 — Rust Tier-1 importer (`syn`)

* Add `crates/x07-translator-rust/`
* Parse + lower a tiny subset:

  * `fn foo(input: &[u8]) -> Vec<u8>` patterns
  * `for i in 0..len` loops
  * indexing `[i]`
  * if/else
* First target module: `std.ascii` (or your `ascii.x07.json`)
* Add differential tests:

  * Rust reference compiled as a test helper binary
  * compare with X07 output on fixed vectors

### PR 3 — C importer using libclang

* Add `crates/x07-translator-c/` with libclang traversal ([Clang][4])
* Support very small C subset:

  * functions operating on `uint8_t* + len`
  * loops, if/else
* Translate a C “ascii-like” module as proof

### PR 4 — Shared verification harness + property testing

* Add `x07verify` that:

  * runs fixed tests
  * runs property tests via proptest (bounded) ([Altsysrq][6])
  * collects mem_stats from X07 runs
* Add CI target: `./translator/scripts/verify_import.sh --fast`

### PR 5 — Import annotations + subset spec

* Add `translator/specs/supported-subset.md`
* Add `#[x07export]`-style Rust attributes (or a comment-based marker) to:

  * restrict which functions get imported
  * supply missing type info (`u8`, `u32`, endianness)
* Add “unsupported construct” report format

### PR 6 — Scaling: utf8 + parse

* Extend Rust lowering for:

  * byte scanning state machines (utf8 validation)
  * small parser combinator subset (manual)
* Import `utf8` and `parse` modules (or produce “candidate imports”)

### PR 7 — LLM patch mode (optional)

* Add `x07import --llm-fallback`:

  * If structural importer fails, call LLM to rewrite into supported subset
  * Record prompts + outputs for reproducibility
* Still must pass diff/property gates

### PR 8 — json + map/set (hard mode)

* Treat these as “semi-automatic”:

  * importer handles skeleton + many helpers
  * LLM fills gaps
  * verification does the real trust work

---

## Benchmarks integration (so regressions are caught early)

Even if import is “just stdlib,” you want suites that exercise real stdlib usage so compiler/core-builtin and stdlib changes stay aligned with production reality.

So every imported module should also register **benchmark tasks**:

* conformance tasks (correctness)
* micro-perf tasks (memcpy/realloc/peak_live_bytes)
* fuzz corpus seeds (offline)

This gives you:

* rapid regression detection
* guidance to keep canonical patterns stable

---

## Practical recommendations for your project right now

1. **Start with Rust importer (`syn`)** because it’s easiest to iterate quickly. ([Docs.rs][1])
2. Add **typed rustc_driver** later only when the subset becomes too restrictive. ([Rust Compiler Development Guide][2])
3. For C, use **libclang** for correctness; tree-sitter is better suited for editor-style parsing. ([Clang][4])
4. Put most “trust” into **verification gates** (differential + property tests), not into “clever translation.”
5. Treat LLM assistance as **patch generation**, never as trusted compilation.

---

## One thing you should decide up front

### Do you want imported modules to be:

* **Canonical, clean X07** (best for maintenance), or
* **Mechanically translated X07** (best for automation but ugly)

I recommend: **canonical output**.

That means the emitter should prefer:

* `(vec_u8.with_capacity ...)` + `extend_bytes`
* “view-first” APIs once G1 exists
* stable helpers in `std.bytes`, `std.parse`, etc.

You can do this by making x07IR higher-level (“builder append”, “scan bytes”), then emitting the canonical X07 patterns.

---

[1]: https://docs.rs/syn?utm_source=chatgpt.com "syn - Rust"
[2]: https://rustc-dev-guide.rust-lang.org/rustc-driver/intro.html?utm_source=chatgpt.com "rustc_driver and rustc_interface"
[3]: https://rustc-dev-guide.rust-lang.org/borrow_check.html?utm_source=chatgpt.com "The borrow checker"
[4]: https://clang.llvm.org/docs/LibClang.html?utm_source=chatgpt.com "Libclang tutorial — Clang 22.0.0git documentation - LLVM"
[5]: https://tree-sitter.github.io/?utm_source=chatgpt.com "Tree-sitter: Introduction"
[6]: https://altsysrq.github.io/proptest-book/?utm_source=chatgpt.com "Introduction - Proptest"
[7]: https://docs.rs/proptest?utm_source=chatgpt.com "proptest - Rust"
[8]: https://rust-fuzz.github.io/book/cargo-fuzz.html?utm_source=chatgpt.com "Fuzzing with cargo-fuzz - Rust Fuzz Book"
[9]: https://users.cs.utah.edu/~regehr/yarpgen-oopsla20.pdf?utm_source=chatgpt.com "Random Testing for C and C++ Compilers with YARPGen"

## Archived draft (superseded)

The remaining content below is an older design draft captured during early iteration. The current
implementation is `x07import-cli` / `x07import-core`, and the current pinned stdlib lives under
`stdlib/std/0.1.1/`.

Below is a concrete, “PR-ready” spec for an **`x07import`** tool that deterministically imports a *restricted* subset of Rust into X07 stdlib modules, plus an opinionated answer on whether X07 should add more core types.

I’m assuming your current Phase‑E module form is:

* modules are `.x07.json` files containing `decls` (`export` + `defn`/`defasync`) and `imports`
* function names are fully-qualified (e.g. `std.text.ascii.normalize_lines`)
* stdlib is versioned/pinned in `stdlib/std/0.1.0/...`

---

## 1) `x07import` CLI interface (exact clap structs)

This uses clap derive (v4 style) with subcommands. (Clap’s derive model with `#[derive(Parser)]` + `#[derive(Subcommand)]` is documented in clap’s official derive tutorial. )

Create `crates/x07import/src/cli.rs`:

```rust
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Deterministic importer: Rust/C → x07IR → X07 stdlib module(s).
///
/// Goals:
/// - Deterministic output (stable ordering, stable names)
/// - Safe subset only (no ambient I/O, no nondet)
/// - Verified by differential tests (Rust reference vs X07 runner)
#[derive(Parser, Debug)]
#[command(name = "x07import")]
#[command(about = "Deterministic stdlib importer for X07 (C backend).", long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Root of stdlib packages (default: ./stdlib).
    ///
    /// Expected package layout:
    ///   <stdlib_root>/<package>/<version>/modules/<module_path>.x07.json
    #[arg(long, default_value = "stdlib")]
    pub stdlib_root: PathBuf,

    /// Package name (default: "std").
    #[arg(long, default_value = "std")]
    pub package: String,

    /// Package version (default: "0.1.0").
    #[arg(long, default_value = "0.1.0")]
    pub package_version: String,

    /// If set, don't write files; print what would change.
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,

    /// If set, run differential verification after generating.
    #[arg(long, default_value_t = true)]
    pub verify: bool,

    /// If set, update stdlib.lock after writing module(s).
    #[arg(long, default_value_t = true)]
    pub update_lock: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Import a Rust source file into an X07 module.
    Rust(ImportRust),

    /// Verify an existing generated module against a Rust reference implementation.
    Verify(VerifyModule),

    /// Reformat / normalize an existing *.x07.json deterministically
    /// (stable indentation + stable ordering for exports).
    Fmt(FmtModule),
}

#[derive(Args, Debug)]
pub struct ImportRust {
    /// Path to the Rust source file to import (e.g. ./translator/rust-ref/ascii.rs).
    #[arg(value_name = "RUST_FILE")]
    pub rust_file: PathBuf,

    /// X07 module id (symbol prefix), e.g. "std.text.ascii".
    ///
    /// Generated functions will be named:
    ///   <module_id>.<rust_fn_name>
    #[arg(long, value_name = "MODULE_ID")]
    pub module_id: String,

    /// Module output directory.
    ///
    /// If unset, defaults to:
    ///   <stdlib_root>/<package>/<package_version>/<module_slug>/
    /// where module_slug is the last segment of module_id (e.g. "ascii").
    #[arg(long)]
    pub out_dir: Option<PathBuf>,

    /// Which Rust functions to export (by Rust function name, not fully-qualified).
    ///
    /// If empty: export all `pub fn` in the file.
    #[arg(long, value_delimiter = ',', num_args = 0..)]
    pub export: Vec<String>,

    /// Import policy (what Rust subset is allowed).
    #[arg(long, value_enum, default_value_t = ImportPolicy::Safe)]
    pub policy: ImportPolicy,

    /// Emit intermediate x07IR JSON for debugging.
    #[arg(long)]
    pub emit_x07ir: Option<PathBuf>,

    /// Write a small "harness" solve program next to the module for verification.
    #[arg(long, default_value_t = true)]
    pub write_harness: bool,

    /// Rust edition assumption for parsing (does not typecheck; only affects heuristics).
    #[arg(long, default_value = "2021")]
    pub rust_edition: String,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPolicy {
    /// Only deterministic, side-effect-free constructs:
    /// - if/else
    /// - for loops over integer ranges or slices
    /// - let bindings / mutation of locals
    /// - arithmetic/compare on integers
    /// - Vec<u8> + &[u8] / String-as-bytes patterns that lower to bytes/vec_u8
    Safe,

    /// Adds a few extra syntactic forms that are still deterministic to lower:
    /// - while loops (rewritten to for + explicit index)
    /// - match on u8/u32 with small constant arms (rewritten to if ladders)
    Extended,

    /// Allows more patterns but may generate larger/slower X07:
    /// - nested matches, some iterator sugar lowered via desugaring heuristics
    Permissive,
}

#[derive(Args, Debug)]
pub struct VerifyModule {
    /// X07 module id, e.g. "std.text.ascii".
    #[arg(long)]
    pub module_id: String,

    /// Module root directory containing *.x07.json.
    #[arg(long)]
    pub module_dir: PathBuf,

    /// Path to Rust reference source (single file) OR a Cargo crate root.
    ///
    /// For v1, prefer a single file with pure functions.
    #[arg(long)]
    pub rust_ref: PathBuf,

    /// Number of randomized test cases (property tests).
    #[arg(long, default_value_t = 256)]
    pub cases: u32,

    /// Max input bytes for randomized cases.
    #[arg(long, default_value_t = 4096)]
    pub max_len: usize,

    /// Path(s) to module roots used by x07c when resolving (import ...).
    ///
    /// Typically: stdlib/<pkg>/<ver>/ plus any additional package roots.
    #[arg(long, value_delimiter = ',', num_args = 1..)]
    pub module_roots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct FmtModule {
    /// Module root directory containing *.x07.json.
    #[arg(long)]
    pub module_dir: PathBuf,

    /// If set, fail (exit != 0) if formatting would change the file.
    #[arg(long, default_value_t = false)]
    pub check: bool,
}
```

Notes:

* This CLI is **intentionally deterministic**: we’re not adding LLM-assisted translation flags here.
* `--verify` is “on by default” because the point is *safe acceleration*, not blind transpilation.

---

## 2) x07IR data types (exact Rust types)

**x07IR** is a small typed IR that is *higher-level than raw S-expr* but *low-level enough* to preserve determinism and avoid needing full Rust typechecking.

Put this in `crates/x07import-core/src/x07ir.rs`:

```rust
use serde::{Deserialize, Serialize};

/// A fully-imported module, ready to emit into X07 (*.x07.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct x07Module {
    /// X07 module id / namespace, e.g. "std.text.ascii".
    pub module_id: String,

    /// Package identity where this module will live (stdlib/std/0.1.0/…).
    pub package: x07PackageId,

    /// Exported fully-qualified symbols.
    ///
    /// Must be stable-sorted for deterministic emission.
    pub exports: Vec<String>,

    /// Functions, stable-sorted by name for deterministic emission.
    pub functions: Vec<x07Function>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct x07PackageId {
    pub name: String,    // e.g. "std"
    pub version: String, // e.g. "0.1.0"
}

/// A function in the X07 module.
/// We keep “typed params” so we can validate / refuse impossible translations early.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct x07Function {
    /// Fully-qualified function name, e.g. "std.text.ascii.normalize_lines".
    pub name: String,

    /// Parameters (names should be stable and alpha-renamed before emitting).
    pub params: Vec<x07Param>,

    /// Return type.
    pub ret: x07Ty,

    /// Body as a block (stmts + tail expr).
    pub body: x07Block,

    /// Optional importer attributes (useful for debugging & roundtrips).
    pub attrs: Vec<x07FnAttr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct x07Param {
    pub name: String,
    pub ty: x07Ty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum x07FnAttr {
    /// If the original Rust fn was `pub`.
    Public,
    /// Original Rust identifier (unqualified).
    RustName(String),
    /// Marker that this fn was auto-generated.
    Generated,
}

/// Core X07 types for importer v1.
/// (These can lower to your actual X07 types; if core only has i32/bytes,
/// this is still useful as “import-time typing”.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum x07Ty {
    I32,
    Bytes,

    // Planned (G1): zero-copy fat pointer type.
    ViewBytes,

    // Statement tail placeholder.
    Unit,
}

/// A block is a list of statements followed by a tail expression.
/// When emitting, this becomes (begin <stmts...> <tail>).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct x07Block {
    pub stmts: Vec<x07Stmt>,
    pub tail: Box<x07Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum x07Stmt {
    Let {
        name: String,
        /// Optional type hint (import-time only).
        ty: Option<x07Ty>,
        init: x07Expr,
    },
    Set {
        name: String,
        value: x07Expr,
    },
    Expr(x07Expr),
    Return(x07Expr),
}

/// Expression forms that map cleanly to your Phase-E S-expr surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum x07Expr {
    Int(i32),
    Var(String),

    /// Import-time bytes literal. The emitter decides whether to use bytes.lit
    /// or a bytes.alloc + bytes.set loop (depending on literal size & legality).
    BytesLiteral(Vec<u8>),

    /// Generic call: head is a symbol, args are expressions.
    ///
    /// Examples:
    /// - head="bytes.len"
    /// - head="std.text.ascii.to_lower_u8"
    Call {
        head: String,
        args: Vec<x07Expr>,
    },

    If {
        cond: Box<x07Expr>,
        then_: Box<x07Expr>,
        else_: Box<x07Expr>,
    },

    /// For-loop (half-open): i in [start, end)
    ///
    /// Emits: (for <idx> <start> <end> <body>)
    For {
        idx: String,
        start: Box<x07Expr>,
        end: Box<x07Expr>,
        body: Box<x07Expr>,
    },

    /// Expression-level begin.
    /// Useful for Rust blocks that contain multiple expressions.
    Begin {
        stmts: Vec<x07Stmt>,
        tail: Box<x07Expr>,
    },
}
```

Why syn + x07IR (and not directly S-expr AST)?

* `syn` can parse Rust source to an AST (it’s explicitly a parsing library, not a typechecker).
* x07IR lets you enforce a *safe deterministic subset* and emit clean X07, while still keeping a path to richer types later.

---

## 3) End-to-end example: `ascii.rs` → `stdlib/std/0.1.1/modules/std/text/ascii.x07.json` + differential tests

### 3.1 Input Rust reference: `translator/rust-ref/std_text_ascii/ascii.rs`

This is the “source of truth” for behavior.

```rust
// translator/rust-ref/std_text_ascii/ascii.rs
// Pure, deterministic: no I/O, no time, no randomness.

pub fn is_space_tab(c: u8) -> bool {
    c == b' ' || c == b'\t'
}

pub fn is_upper(c: u8) -> bool {
    c >= b'A' && c <= b'Z'
}

pub fn is_lower(c: u8) -> bool {
    c >= b'a' && c <= b'z'
}

pub fn is_alpha(c: u8) -> bool {
    is_upper(c) || is_lower(c)
}

pub fn to_lower_u8(c: u8) -> u8 {
    if is_upper(c) { c + 32 } else { c }
}

/// Trim each line (space/tab only), join with '\n'.
/// Split points: '\n' or '\r'. Empty/whitespace-only lines drop.
/// This mirrors the semantics of your existing x07 ascii.x07.json.
pub fn normalize_lines(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut line_start: usize = 0;
    let mut i: usize = 0;

    while i < input.len() {
        let c = input[i];
        if c == b'\n' || c == b'\r' {
            append_trimmed_line(input, line_start, i, &mut out);
            line_start = i + 1;
        }
        i += 1;
    }
    append_trimmed_line(input, line_start, input.len(), &mut out);

    out
}

fn append_trimmed_line(input: &[u8], start: usize, end: usize, out: &mut Vec<u8>) {
    let mut l = start;
    while l < end && is_space_tab(input[l]) {
        l += 1;
    }

    let mut r = end;
    while r > l && is_space_tab(input[r - 1]) {
        r -= 1;
    }

    if l < r {
        if !out.is_empty() {
            out.push(b'\n');
        }
        out.extend_from_slice(&input[l..r]);
    }
}

/// Extract alpha “words”, lowercase, separated by single ASCII space.
/// Non-alpha is a delimiter. Leading/trailing delimiters drop.
pub fn tokenize_words_lower(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut in_word = false;

    for &c in input {
        if is_alpha(c) {
            if !in_word {
                if !out.is_empty() {
                    out.push(b' ');
                }
                in_word = true;
            }
            out.push(to_lower_u8(c));
        } else {
            in_word = false;
        }
    }

    out
}
```

### 3.2 Output X07 module: `stdlib/std/0.1.1/modules/std/text/ascii.x07.json`

Excerpt of what `x07import rust ...` emits (the checked-in `stdlib/std/0.1.1/modules/std/text/ascii.x07.json` is the canonical output).

```lisp
; stdlib/std/0.1.1/modules/std/text/ascii.x07.json
; Generated by x07import (deterministic). Source: import_sources/rust/std_text_ascii@0.1.1/ascii.rs

(export
  std.text.ascii.first_line_view
  std.text.ascii.is_space_tab
  std.text.ascii.is_alpha
  std.text.ascii.to_lower_u8
  std.text.ascii.to_upper_u8
  std.text.ascii.normalize_lines
  std.text.ascii.tokenize_words_lower
  std.text.ascii.kth_line_view
  std.text.ascii.last_line_view
  std.text.ascii.split_u8
  std.text.ascii.split_lines_view)

(defn std.text.ascii.is_space_tab ((c i32)) i32
  (if (= c 32)
    1
    (if (= c 9) 1 0)))

(defn std.text.ascii._is_upper ((c i32)) i32
  (if (>=u c 65)
    (if (<u c 91) 1 0)
    0))

(defn std.text.ascii._is_lower ((c i32)) i32
  (if (>=u c 97)
    (if (<u c 123) 1 0)
    0))

(defn std.text.ascii.is_alpha ((c i32)) i32
  (if (std.text.ascii._is_upper c)
    1
    (if (std.text.ascii._is_lower c) 1 0)))

(defn std.text.ascii.to_lower_u8 ((c i32)) i32
  (if (std.text.ascii._is_upper c)
    (+ c 32)
    c))

(defn std.text.ascii._trim_start_space_tab ((b bytes) (start i32) (end i32)) i32
  (begin
    (for i start end
      (begin
        (let c (bytes.get_u8 b i))
        (if (std.text.ascii.is_space_tab c)
          0
          (return i))))
    end))

(defn std.text.ascii._trim_end_space_tab ((b bytes) (start i32) (end i32)) i32
  (begin
    (let last (- start 1))
    (for i start end
      (begin
        (let c (bytes.get_u8 b i))
        (if (std.text.ascii.is_space_tab c)
          0
          (set last i))))
    (+ last 1)))

(defn std.text.ascii._append_trimmed_line_space_tab ((b bytes) (start i32) (end i32) (out i32)) i32
  (begin
    (let l (std.text.ascii._trim_start_space_tab b start end))
    (let r (std.text.ascii._trim_end_space_tab b l end))
    (if (<u l r)
      (begin
        (if (<u 0 (vec_u8.len out))
          (set out (vec_u8.push out 10))
          0)
        (set out (vec_u8.extend_bytes_range out b l (- r l)))
        out)
      out)))

(defn std.text.ascii.normalize_lines ((b bytes)) bytes
  (begin
    (let n (bytes.len b))
    (let out (vec_u8.with_capacity n))
    (let line_start 0)
    (for i 0 n
      (begin
        (let c (bytes.get_u8 b i))
        (if (if (= c 10) 1 (= c 13))
          (begin
            (set out (std.text.ascii._append_trimmed_line_space_tab b line_start i out))
            (set line_start (+ i 1)))
          0)
        0))
    (set out (std.text.ascii._append_trimmed_line_space_tab b line_start n out))
    (vec_u8.into_bytes out)))

(defn std.text.ascii.tokenize_words_lower ((b bytes)) bytes
  (begin
    (let n (bytes.len b))
    (let out (vec_u8.with_capacity n))
    (let in_word 0)
    (for i 0 n
      (begin
        (let c (bytes.get_u8 b i))
        (if (std.text.ascii.is_alpha c)
          (begin
            (if (= in_word 0)
              (begin
                (if (<u 0 (vec_u8.len out))
                  (set out (vec_u8.push out 32))
                  0)
                (set in_word 1))
              0)
            (set out (vec_u8.push out (std.text.ascii.to_lower_u8 c))))
          (set in_word 0))
        0))
    (vec_u8.into_bytes out)))
```

Yes: this is basically your existing ascii module, which is ideal for an end-to-end “importer demo” because it hits:

* loops
* helper functions
* `Vec<u8>` builder patterns
* slice/extend

### 3.3 Minimal metadata for the module (recommended)

Even if your compiler currently resolves modules purely by path, **the importer should emit metadata** so you can later enforce “generated-from” + versioning.

`stdlib/std/0.1.0/ascii/module.toml`:

```toml
module_id = "std.text.ascii"
package = "std"
version = "0.1.0"
source = "std/text/ascii.x07.json"

# Optional traceability:
generated_by = "x07import"
rust_ref = "../../../translator/rust-ref/std_text_ascii/ascii.rs"
```

### 3.4 Differential test harness layout

This is the layout I’d implement so it’s deterministic and CI-friendly:

```
translator/
  rust-ref/
    std_text_ascii/
      ascii.rs

stdlib/
  std/
    0.1.0/
      ascii/
        std/text/ascii.x07.json
        module.toml

crates/
  x07import/
    tests/
      diff_std_text_ascii.rs
    Cargo.toml
    src/
      main.rs
      cli.rs
      x07ir.rs
      rust_frontend.rs
      lower.rs
      emit_x07.rs
```

### 3.5 Differential test code (property-based)

Use `proptest` for generating inputs and shrinking failures.

`crates/x07import/tests/diff_std_text_ascii.rs`:

```rust
use std::path::PathBuf;

use x07_host_runner::{compile_program_with_options, run_artifact_file, RunnerConfig, RunnerWorld};
use x07c::compile::CompileOptions;

use proptest::prelude::*;

// Pull in the Rust reference functions directly (keeps the harness simple).
mod rust_ref {
    include!("../../../translator/rust-ref/std_text_ascii/ascii.rs");
}

fn compile_harness(module_root: PathBuf, solve: serde_json::Value) -> PathBuf {
    let program_src = serde_json::to_vec(&serde_json::json!({
        "schema_version": "x07.x07ast@0.3.0",
        "kind": "entry",
        "module_id": "main",
        "imports": ["std.text.ascii"],
        "decls": [],
        "solve": solve,
    }))
    .expect("serialize x07AST entry");

    let config = RunnerConfig {
        world: RunnerWorld::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 50_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 2 * 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let compile_options = CompileOptions {
        world: x07_worlds::WorldId::SolvePure,
        enable_fs: false,
        enable_rr: false,
        enable_kv: false,
        module_roots: vec![module_root],
        emit_main: true,
        freestanding: false,
    };

    let compiled = tempfile::Builder::new()
        .prefix("x07import-diff-")
        .tempdir()
        .expect("tempdir");

    let exe_out = compiled.path().join("harness.exe");

    let compile = compile_program_with_options(
        &program_src,
        &config,
        Some(&exe_out),
        &compile_options,
        &[],
    )
    .expect("compile_program_with_options");

    assert!(compile.ok, "compile failed: {:?}", compile.compile_error);
    exe_out
}

fn run_harness(exe: &PathBuf, input: &[u8]) -> Vec<u8> {
    let config = RunnerConfig {
        world: RunnerWorld::SolvePure,
        fixture_fs_dir: None,
        fixture_fs_root: None,
        fixture_fs_latency_index: None,
        fixture_rr_dir: None,
        fixture_kv_dir: None,
        fixture_kv_seed: None,
        solve_fuel: 50_000_000,
        max_memory_bytes: 64 * 1024 * 1024,
        max_output_bytes: 2 * 1024 * 1024,
        cpu_time_limit_seconds: 5,
        debug_borrow_checks: false,
    };

    let res = run_artifact_file(&config, exe, input).expect("run_artifact_file");
    assert!(res.ok, "runner failed: exit={}, trap={:?}", res.exit_status, res.trap);
    assert_eq!(res.exit_status, 0, "nonzero exit, stderr={:?}", String::from_utf8_lossy(&res.stderr));
    res.solve_output
}

proptest! {
    // Normalize_lines should match exactly.
    #[test]
    fn diff_normalize_lines(input in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let module_root = PathBuf::from("stdlib/std/0.1.1"); // adjust if your module roots differ
        let exe = compile_harness(module_root, serde_json::json!(["std.text.ascii.normalize_lines", "input"]));
        let got = run_harness(&exe, &input);
        let expected = rust_ref::normalize_lines(&input);
        prop_assert_eq!(got, expected);
    }

    // Tokenize_words_lower should match exactly.
    #[test]
    fn diff_tokenize_words_lower(input in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let module_root = PathBuf::from("stdlib/std/0.1.1");
        let exe = compile_harness(module_root, serde_json::json!(["std.text.ascii.tokenize_words_lower", "input"]));
        let got = run_harness(&exe, &input);
        let expected = rust_ref::tokenize_words_lower(&input);
        prop_assert_eq!(got, expected);
    }
}
```

This harness does three important things:

1. **Locks semantics to Rust** for the module you imported.
2. Uses your **actual x07c + native runner** (so it catches ABI/perf regressions).
3. Uses **property testing** (so you’re not only verifying a handful of hand-picked cases).

### 3.6 The `x07import` command for this example

Example usage:

```bash
cargo run -p x07import -- \
  rust translator/rust-ref/std_text_ascii/ascii.rs \
  --module-id std.text.ascii \
  --out-dir stdlib/std/0.1.0/ascii \
  --policy safe \
  --verify \
  --update-lock
```

---

## 4) Does it make sense to add more types to X07?

Yes — but you should be deliberate about **where types exist**:

### Option 1 (recommended now): “Importer types” without core language changes

Keep X07 core types minimal (today: `i32` + `bytes` + `bytes_view` + `vec_u8` builders), but let **x07IR** carry richer types during import:

* `u8`, `u32`, `bool`, `usize`
* `&[u8]` as `BytesView` (even if lowered to bytes-copy today)
* `Vec<u8>` as `vec_u8` builder patterns

Then lower x07IR → existing X07 constructs.

This gives you:

* faster stdlib porting *now* (you can read Rust code “as typed”)
* no explosion of core semantics while Phase E–F are still stabilizing

### Option 2 (the right move for Phase G1): add **ViewBytes** as a distinct core type

This one is *worth doing in core* because it unlocks the “big win” you want:

* zero-copy slicing
* `memcpy_bytes` collapsing to near output size
* `realloc_calls` improvements through better builders and views

This aligns with your Phase G1 goals.

### Option 3 (later, Phase H): structs/enums + generic-ish containers

For “C/Rust-class” capability, you’ll eventually want:

* `struct` / records
* `enum` / tagged unions
* typed slices (`slice<T>`) and strings (`str`/`utf8`)
* `Result<T,E>` / `Option<T>` first-class

But: adding these too early increases:

* compiler complexity
* LLM search space for synthesis
* required benchmarking coverage

So I’d stage it:

* **Now (Phase E/F)**: keep core types lean; do typed import in x07IR only
* **Phase G1**: make `bytes_view` real
* **Phase H**: introduce structured types once you have a big cross-stdlib benchmark suite + canary gating

### A practical heuristic

Add a new *core* type only if it meets **at least one**:

* it enables a **new safety property** (views + borrow checks, handles, etc.)
* it enables a **major performance regime** (views again)
* it significantly reduces solver failure modes that you can’t solve with stdlib/macro patterns

Everything else can live in x07IR + stdlib for quite a while.

---
Below is a **deterministic Rust-subset → x07IR mapping spec (v1)** you can hand to developers implementing `x07import` (Rust → `stdlib/std/0.1.1/modules/**/*.x07.json`). It is intentionally strict so translation is **repeatable**, doesn’t depend on Rust name resolution/inference, and produces **LLM-repairable diagnostics**.

I’m assuming your v1 parser entry points are exactly:

* `syn::parse_file(content)` → `syn::File`
* accept `syn::Item::Fn` / `syn::ItemFn` as the primary import unit

---

## 0) Determinism contract for x07import (v1)

**Principle:** “If it’s not explicitly mapped, it’s rejected.”

Deterministic translation means:

1. **No macro expansion** (reject `macro_rules!`, `Expr::Macro`, `Stmt::Macro`, etc.)
2. **No cfg/test/feature-gated code** in imported modules (reject non-doc attributes like `#[cfg]`, `#[test]`, `#[inline]`, etc.).
3. **No name resolution**: only accept paths that are either:

   * a **single ident** (local var / function), or
   * a **known whitelisted fully-qualified path** (optional, v2).
     Enforce via `Path::get_ident()` / `Path::require_ident()`
4. **Stable output ordering**:

   * preserve **file order** of `ItemFn`s (don’t sort by name),
   * preserve **statement order**,
   * preserve **evaluation order** for expressions.
5. **No semantic dependence on Rust overflow/panic semantics**:

   * importer defines numeric semantics (see §4.3) and always emits X07-safe forms.

This “tight contract” is exactly the kind of thing you want in the Solve/x07lve LLM contexts so models don’t guess about what’s allowed. 

---

## 1) Supported syn entry points (exact) and global rejects

### 1.1 Parse entrypoints (v1)

* `syn::parse_file(&str) -> Result<syn::File>`
* `syn::File { shebang, attrs, items }`

Notes:

* `parse_file` discards BOM and preserves shebang lines; you should ignore `shebang` for semantics but keep it in diagnostics/metadata if present.

### 1.2 Top-level allowed items (v1)

Allowed:

* `syn::Item::Fn` (`syn::ItemFn`)
  Optional (recommended to support early):
* `syn::Item::Const` if and only if initializer is a literal or simple arithmetic expression in the supported expr subset.

Rejected (error):

* `Item::Use`, `Item::Mod`, `Item::Struct`, `Item::Enum`, `Item::Impl`, `Item::Trait`, `Item::Macro`, `Item::ExternCrate`, etc.

Rationale: anything needing name resolution, trait selection, generics, or macro expansion becomes non-trivial and non-deterministic quickly.

---

## 2) x07IR target (import-time IR)

Even if your *runtime* is still “only i32 + bytes”, the importer should use a slightly richer **import-time type system** to keep translation predictable and diagnostics precise, then lower it to your actual X07 surface.

### 2.1 x07IR types (v1)

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum x07Ty {
    Unit,

    // Scalar machine types (lower to i32 bits)
    Bool,   // 0/1
    U8,     // 0..255 (stored in i32)
    I32,    // signed i32 bits
    U32,    // unsigned 32-bit bits (stored in i32)

    // Bytes
    Bytes,      // owning buffer
    BytesView,  // borrowed view (ptr+len concept; may still lower to Bytes until Phase G1)

    // Builders / handles
    VecU8,      // vec_u8 value (lowers to `vec_u8` surface type)
}
```

### 2.2 x07IR expressions and statements (minimal)

```rust
pub enum x07Expr {
    Var(String),
    I32(i32),
    Bool(bool),
    U8(u8),
    BytesLit(Vec<u8>),

    Call { func: String, args: Vec<x07Expr> },      // calls to local/imported fns
    Builtin { name: String, args: Vec<x07Expr> },   // bytes.*, vec_u8.* etc

    // Structured
    If { cond: Box<x07Expr>, then_e: Box<x07Expr>, else_e: Box<x07Expr> },

    // Ops
    Bin { op: x07BinOp, a: Box<x07Expr>, b: Box<x07Expr> },
    Un  { op: x07UnOp,  x: Box<x07Expr> },

    // Indexing (lowered later)
    Index { base: Box<x07Expr>, idx: Box<x07Expr> },
}

pub enum x07Stmt {
    Let { name: String, ty: x07Ty, init: x07Expr, mutable: bool },
    Set { target: x07LValue, value: x07Expr }, // x = ...
    Expr(x07Expr),
    Return(Option<x07Expr>),

    If { cond: x07Expr, then_s: Vec<x07Stmt>, else_s: Vec<x07Stmt> },

    ForRange { var: String, start: x07Expr, end: x07Expr, body: Vec<x07Stmt> },
}
```

---

## 3) Exact supported syn nodes (v1)

This is the list you can literally encode into a “subset validator” pass.

### 3.1 Items / functions

* `syn::File`
* `syn::ItemFn` (fields include `sig`, `block`)
* `syn::Signature`
* `syn::FnArg::Typed(syn::PatType)` (reject `FnArg::Receiver`)
* `syn::Pat::Ident(syn::PatIdent)` (reject other patterns)
* `syn::ReturnType`

### 3.2 Types

Only these `syn::Type` shapes are allowed (everything else rejected):

* `Type::Path(TypePath)`
* `Type::Reference(TypeReference)`
* `Type::Slice(TypeSlice)` (only inside a reference)

### 3.3 Statements

Allow only:

* `syn::Stmt::Local(Local)`
* `syn::Stmt::Expr(Expr, Option<Token![;]>)`

Reject:

* `Stmt::Item`
* `Stmt::Macro`

### 3.4 Expressions

Allow only:

* `Expr::Block`
* `Expr::Lit`
* `Expr::Path` (single-ident only)
* `Expr::Call`
* `Expr::MethodCall`
* `Expr::Binary`
* `Expr::Unary`
* `Expr::Assign`
* `Expr::Index`
* `Expr::If`
* `Expr::Return`
* `Expr::Paren` / `Expr::Group` (ignored wrappers)
* `Expr::Range` (only in `for`-loop iterator; see §4.8)

Reject (error):

* `Expr::Macro`, `Expr::Match`, `Expr::Closure`, `Expr::Async`, `Expr::Await`, `Expr::Try`, `Expr::Struct`, etc.

Also: `syn::Expr` itself is only present with syn’s `full` or `derive` features, and most variants require `full`.

---

## 4) Deterministic mapping table (syn node → x07IR)

### 4.1 Root: `syn::File` → `x07Module`

**Constraints**

* `File.attrs`: allow only doc attributes (`#[doc = ...]`) OR require empty. (v1 recommended: allow doc-only; reject others)
* `File.items`: must be a list of allowed items only.

**Mapping**

* `x07Module { name: <derived from input path>, fns: [...], consts: [...] }`

**Diagnostics**

* `X7I100 UnsupportedItem`: any non-`ItemFn` (or allowed `ItemConst`) at top-level.

### 4.2 Function: `syn::ItemFn` → `x07Fn`

`ItemFn` contains `sig` and `block`.

**Constraints**

* `sig.generics` must be empty (`<>` not allowed).
* `sig.asyncness` must be none.
* no `unsafe`, no `extern "C"`.
* args:

  * reject `FnArg::Receiver`
  * accept only `FnArg::Typed(PatType)` where pattern is `Pat::Ident`
* return type:

  * must be one of the supported type forms (see below)

**Mapping**

* `x07Fn { name, params: Vec<(String,x07Ty)>, ret: x07Ty, body: Vec<x07Stmt> }`

**Export rules (deterministic)**

* `pub fn ...` ⇒ module export
* non-`pub` ⇒ internal

**Diagnostics**

* `X7I110 UnsupportedFnSig`: generics/async/unsafe/extern
* `X7I111 UnsupportedParamPattern`: non-`PatIdent`
* `X7I112 UnsupportedReceiver`: `self` / `&self` / `&mut self`

---

### 4.3 Type mapping: `syn::Type` → `x07Ty`

Use the `syn::Type` enum and only accept a subset (Path/Reference/Slice).

#### 4.3.1 `Type::Path(TypePath)`

**Constraints**

* Reject qualified self-types (`TypePath.qself != None`).
* Path must be either:

  * **single ident** (`Path::require_ident`)
  * OR ends with a known ident (`Vec`, `Option`, etc) if you enable “last-segment matching” (recommended v1: **only single ident** for determinism).

**Supported idents**

* `bool` → `x07Ty::Bool`
* `u8` → `x07Ty::U8`
* `i32` → `x07Ty::I32`
* `u32` → `x07Ty::U32`
* `usize` → `x07Ty::U32` (with a warning: “usize lowered to u32”)
* `Bytes` (if you define a Rust-side alias type) → `x07Ty::Bytes`

Optional (if you allow simple generics):

* `Vec<u8>` → `x07Ty::VecU8`
  Constraint: exactly one generic arg, exactly `u8`.

**Diagnostics**

* `X7I120 UnsupportedType`: any other TypePath
* `X7I121 UnsupportedGenericType`: e.g. `Vec<T>`, `Vec<u16>`, `Option<...>` (unless you explicitly add support)

#### 4.3.2 `Type::Reference(TypeReference)`

TypeReference represents `&'a T` or `&'a mut T`.

**Constraints**

* Reject lifetimes in v1 (or ignore) — but do it deterministically:

  * either require `lifetime == None`, or accept and ignore.
* Reject `&mut` in v1 (deterministic, keeps borrow surface simple).

**Supported**

* `&[u8]` → `x07Ty::BytesView`

  * expects `TypeReference.elem = Type::Slice(TypeSlice{ elem = TypePath(u8) })`

**Diagnostics**

* `X7I122 UnsupportedRefType`: any `&T` except `&[u8]`
* `X7I123 UnsupportedMutRef`: `&mut ...`

#### 4.3.3 `Type::Slice(TypeSlice)`

TypeSlice represents `[T]`.

**Constraints**

* Only legal inside `&[T]`
* Only accept `T = u8` in v1

**Diagnostics**

* `X7I124 UnsupportedSliceElem`

---

### 4.4 Statements: `syn::Stmt` → `x07Stmt`

`Stmt` variants include `Local`, `Item`, `Expr`, `Macro`.

#### 4.4.1 `Stmt::Local(Local)` (let-binding)

**Supported forms**

* `let x: <ty> = <expr>;`
* `let mut x: <ty> = <expr>;`

v1 recommendation: **require type annotation** for locals (no inference), except for `let x = <literal>;` where literal implies `I32/U8/Bool`.

**Mapping**

* `x07Stmt::Let { name, ty, init, mutable }`

**Reject**

* destructuring `let (a,b) = ...`
* `let x;` (no init)
* patterns other than ident.

**Diagnostics**

* `X7I200 UnsupportedLetPattern`
* `X7I201 MissingLetInit`
* `X7I202 MissingLetType` (if you choose “no inference”)

#### 4.4.2 `Stmt::Expr(expr, semi_opt)`

**Mapping**

* If it is a final tail expression in a block with no semicolon:

  * treat as expression result (handled at block level)
* If it has a semicolon:

  * `x07Stmt::Expr(lower_expr(expr))`

Reject:

* `Stmt::Item` (nested items)
* `Stmt::Macro`

**Diagnostics**

* `X7I210 UnsupportedStmtItem`
* `X7I211 UnsupportedStmtMacro`

---

### 4.5 Expressions: literals

**Supported**

* integer literals: `LitInt` → `x07Expr::I32(...)` (must fit signed i32, deterministic parse)
* `true/false` → `x07Expr::Bool`
* byte literal `b'x'` → `x07Expr::U8`
* byte string `b"..."` → `x07Expr::BytesLit`

**Diagnostics**

* `X7I300 UnsupportedLiteral` (float, char, string, etc)
* `X7I301 IntOutOfRange`

---

### 4.6 Expressions: paths and calls

#### 4.6.1 `Expr::Path`

**Constraint**

* path must be a **single ident** via `Path::require_ident()`

**Mapping**

* if ident matches a local variable → `x07Expr::Var`
* if ident matches a known function in module → `x07Expr::Call`
* else error (no implicit imports)

**Diagnostics**

* `X7I310 UnsupportedPath`: multi-segment path
* `X7I311 UnknownName`

#### 4.6.2 `Expr::Call(ExprCall)`

ExprCall represents `invoke(a,b)`.

**Constraint**

* `func` must be `Expr::Path` single-ident (v1)
* no turbofish generics

**Mapping**

* `x07Expr::Call { func: ident, args: ... }`
* (optional) if ident is a whitelisted builtin name, map to `x07Expr::Builtin`

**Diagnostics**

* `X7I320 UnsupportedCallee`: callee not ident
* `X7I321 UnsupportedTurbofish`

---

### 4.7 Expressions: method calls

ExprMethodCall describes `x.foo::<T>(a, b)`.

**Constraint**

* reject turbofish generics in v1 (`::<T>`)
* receiver must type-check to a known receiver class (Bytes/BytesView/VecU8).

**Deterministic builtin mapping table (v1)**

| Rust form                | Required receiver type | x07IR                                                           |
| ------------------------ | ---------------------- | --------------------------------------------------------------- |
| `b.len()`                | `Bytes`/`BytesView`    | `Builtin("bytes.len",[b])`                                      |
| `b[i]`                   | `Bytes`/`BytesView`    | `Builtin("bytes.get_u8",[b,i])` (via ExprIndex)                 |
| `v.reserve_exact(n)`     | `VecU8`                | `Builtin("vec_u8.reserve_exact",[v,n])`                         |
| `v.extend_from_slice(s)` | `VecU8`, `s: &[u8]`    | `Builtin("vec_u8.extend_bytes",[v,s])`                          |
| `Vec::with_capacity(n)`  | n/a                    | handled as associated function call (see §4.6.2 + special-case) |

If you don’t have some of these builtins yet, keep them as **importer intrinsics** and lower them to your stable X07 core via helper functions/macros in `std.vec` (still deterministic).

**Diagnostics**

* `X7I330 UnsupportedMethod`: not in whitelist
* `X7I331 UnsupportedMethodGeneric`: turbofish present
* `X7I332 ReceiverTypeMismatch`

---

### 4.8 Expressions: binary operators

Binary expressions are `ExprBinary { left, op: BinOp, right }`
Important: syn’s `BinOp` includes both arithmetic and compound assignments (`AddAssign`, etc).

**Supported BinOp subset (v1)**

* Arithmetic: `Add`, `Sub`, `Mul` (optional: `Div`, `Rem` only for non-zero divisor with explicit checks)
* Comparisons: `Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`
* Boolean: `And` (`&&`), `Or` (`||`)
* Bitwise / shifts: optional (enable once X07 has stable equivalents)

**Compound assignment**

* `x += y` parses through syn as a binary op with `AddAssign` etc.
  Lower deterministically as:
* `x = x + y` (respecting evaluation order and side effects)
* for `a[i] += y`, reject in v1 (or require explicit rewrite).

**Numeric semantics (importer-defined)**
To avoid Rust debug/release overflow differences:

* All integer arithmetic is **wrapping 32-bit**.
* `U8` operations wrap then mask to 0..255 at boundaries when stored as `U8`.

**Diagnostics**

* `X7I340 UnsupportedBinOp`
* `X7I341 CompoundAssignToNonVar`
* `X7I342 DivisionByZeroPossible` (if divisor not provably nonzero)

---

### 4.9 Expressions: if

An `if` expression has restricted else shape (else is `If` or `Block`).

**Supported**

* `if cond { <block> } else { <block> }`
* `if cond { <expr> } else { <expr> }` if both sides type-check to same x07Ty (expression form)

**Mapping**

* statement form → `x07Stmt::If`
* expression form → `x07Expr::If`

**Diagnostics**

* `X7I350 IfBranchTypeMismatch`
* `X7I351 MissingElse` (if used as expression but else missing)

---

### 4.10 for loops: `for i in 0..end { ... }`

(v1 strict; no iterators)

**Supported syn shape**

* `Expr::ForLoop` where:

  * pattern is `Pat::Ident(i)`
  * iterator is `Expr::Range` with:

    * `start` either `0` or an ident
    * `end` is an ident or `b.len()` / `bytes.len(b)`-equivalent call

**Mapping**

* `x07Stmt::ForRange { var, start, end, body }`

**Reject**

* `for x in slice.iter()`
* `for (i, x) in slice.iter().enumerate()`
* `for i in 0..=end` (inclusive ranges) in v1

**Diagnostics**

* `X7I360 UnsupportedForIter`
* `X7I361 UnsupportedForPattern`
* `X7I362 UnsupportedRangeKind`

---

## 5) Rejection diagnostics (exact shape + codes)

### 5.1 Diagnostic payload (recommendation)

Emit JSON (and human text), deterministic keys and ordering:

```json
{
  "code": "X7I330",
  "message": "Unsupported method call: `iter`",
  "span": { "line_start": 12, "col_start": 18, "line_end": 12, "col_end": 22 },
  "help": "Rewrite `for x in s.iter()` as `for i in 0..s.len() { let x = s[i]; ... }`",
  "node": "syn::ExprMethodCall"
}
```

Implementation notes:

* For spans: use `syn::spanned::Spanned` + `proc_macro2` span locations feature if you want stable line/col.
* If span locations aren’t available deterministically in your environment, fall back to a byte offset or just omit.

### 5.2 Canonical error codes (v1)

You can implement these as an enum + stable string codes:

* `X7I100 UnsupportedItem`

* `X7I110 UnsupportedFnSig`

* `X7I111 UnsupportedParamPattern`

* `X7I112 UnsupportedReceiver`

* `X7I120 UnsupportedType`

* `X7I121 UnsupportedGenericType`

* `X7I122 UnsupportedRefType`

* `X7I123 UnsupportedMutRef`

* `X7I124 UnsupportedSliceElem`

* `X7I200 UnsupportedLetPattern`

* `X7I201 MissingLetInit`

* `X7I202 MissingLetType`

* `X7I210 UnsupportedStmtItem`

* `X7I211 UnsupportedStmtMacro`

* `X7I300 UnsupportedLiteral`

* `X7I301 IntOutOfRange`

* `X7I310 UnsupportedPath`

* `X7I311 UnknownName`

* `X7I320 UnsupportedCallee`

* `X7I321 UnsupportedTurbofish`

* `X7I330 UnsupportedMethod`

* `X7I331 UnsupportedMethodGeneric`

* `X7I332 ReceiverTypeMismatch`

* `X7I340 UnsupportedBinOp`

* `X7I341 CompoundAssignToNonVar`

* `X7I342 DivisionByZeroPossible`

* `X7I350 IfBranchTypeMismatch`

* `X7I351 MissingElse`

* `X7I360 UnsupportedForIter`

* `X7I361 UnsupportedForPattern`

* `X7I362 UnsupportedRangeKind`

This is “LLM-friendly”: the Solve model can learn to rewrite toward the supported subset from the `help` strings, and you can embed the code list into the Solve prompt context. 

---

## 6) Does it make sense to add more types to X07?

Yes — but the key is **where** you add them.

### Best practice for X07 (given your deterministic goals)

1. **Add types in the compiler type-checker first (zero runtime cost):**

   * `bool`, `u8`, `u32`, `bytes_view`, `vec_u8`
   * lower them to existing runtime reps (i32/bytes)
     This improves:
   * importer correctness,
   * solver first-try rate,
   * and reduces semantic traps (“negative balance” / sign confusion).

2. **Add runtime-level representations only when needed for performance/safety:**

   * Phase G1: real `bytes_view` (fat pointer) and debug borrow checks.
   * Later: typed slices `slice<T>` etc.

So: **Yes, more types make sense**, but treat them as **compile-time “phantom types” first** so you don’t bloat the runtime or expand the search surface prematurely.

---

## 7) One practical recommendation to unblock stdlib importing immediately

Start with a “Rust prep convention” for modules you want to import:

* no `use` items (or keep but plan to reject in v1),
* no iterators; use index loops,
* no generics, no traits, no impl blocks,
* functions are “free functions” only,
* only `&[u8]`, `u8/u32/i32/bool`, and `Vec<u8>` in locals.

That’s enough to import a lot of “string/bytes utility” code deterministically, then later widen the subset once `x07import` is stable and battle-tested.
-------

Below is a **deterministic, single‑pass Rust‑subset validator** design in “Rust-like pseudocode” that your devs can implement with `syn::parse_file` + `syn::visit::Visit` and have it emit **stable diagnostic codes** in **source order**.

It’s designed to sit in `x07import` *before* the Rust→x07IR lowering: **fail fast** on unsupported constructs with clean, deterministic errors.

(References: `syn::parse_file` API and its feature requirements ([Docs.rs][1]), `syn::visit::Visit` traversal model ([Docs.rs][2]), `syn::Stmt` variants and macro ambiguity ([Docs.rs][3]), `syn::Path::require_ident` behavior ([Docs.rs][4]), and `syn::BinOp` including `AddAssign` / `+=` etc ([Docs.rs][5]).)

---

## 0) Determinism rules this validator enforces

These are the “gotchas” that usually make validators flaky:

1. **Traversal order = source order only**

   * Visit items in `file.items` order.
   * Visit statements in `block.stmts` order.
   * Visit expressions recursively in AST order (left→right).

2. **No hash iteration in output**

   * For lookups, you can use `HashMap`, but diagnostics must never iterate maps to produce messages.
   * If you need to show “known names”, store them in `BTreeSet` and iterate in sorted order (or omit them).

3. **One error per node (by default)**

   * For unsupported node kinds, emit one diagnostic and stop descending into that subtree. This prevents “cascading noise” that makes results depend on internal recursion choices.

4. **Spans: stable enough, but optional**

   * You can attach spans using `Spanned`/`Span` (line/col), but if you find spans vary across toolchains, you can still keep determinism by always emitting codes/messages in a stable order. The core requirement is code stability, not perfect span stability.

---

## 1) Diagnostic types (deterministic codes, stable formatting)

```rust
// NOTE: Replace code strings with the exact codes from your mapping table.
// These are placeholders shaped like the ones we discussed earlier.
type DiagCode = &'static str;

struct Diag {
    code: DiagCode,
    // Avoid anything nondeterministic in formatting.
    message: String,
    help: Option<String>,
    // Keep spans optional (or store byte offsets/line:col deterministically).
    span: Option<SpanRange>,
}

// For stable, portable output.
// If you can’t get line/col reliably, store byte offsets.
struct SpanRange { start_line: u32, start_col: u32, end_line: u32, end_col: u32 }
```

---

## 2) Minimal type model used by validation

You need just enough typing to reject unsupported receiver/method combinations and obvious type mismatches (e.g. `bytes.len()` called on a `u32`).

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ty {
    Unit,      // used internally; generally only allowed in stmt context
    Bool,
    I32,
    U32,
    U8,
    Bytes,     // owned bytes
    BytesView, // borrowed view (if you already model it) else drop this for v1
    VecU8,     // builder handle (maps to X07 vec_u8)
    // Add more only if your mapping table does.
}

struct FnSig {
    params: Vec<Ty>,
    ret: Ty,
}
```

---

## 3) Validator state

Key: diagnostics are appended in encounter order.

```rust
use std::collections::{BTreeMap, BTreeSet};

struct Validator<'src> {
    src: &'src str,
    diags: Vec<Diag>,

    // Collected signatures for known free functions.
    // Use BTreeMap to make any iteration stable if needed.
    fn_sigs: BTreeMap<String, FnSig>,

    // Stack of lexical scopes. Use BTreeMap for stable debug output if you ever print scopes.
    scopes: Vec<BTreeMap<String, Ty>>,

    // Current function return type context.
    cur_ret: Option<Ty>,

    // Optional: strictness toggles (v1 can keep it simple).
    allow_literal_inference: bool,
}

impl<'src> Validator<'src> {
    fn push(&mut self, code: DiagCode, span: Option<SpanRange>, msg: impl Into<String>, help: Option<String>) {
        self.diags.push(Diag { code, span, message: msg.into(), help });
    }

    fn enter_scope(&mut self) { self.scopes.push(BTreeMap::new()); }
    fn exit_scope(&mut self)  { self.scopes.pop(); }

    fn declare(&mut self, name: &str, ty: Ty) {
        // Deterministic: if duplicate in same scope, emit one diag at second decl.
        let scope = self.scopes.last_mut().unwrap();
        if scope.contains_key(name) {
            self.push("X7I_DUP_LOCAL", None,
                format!("duplicate local binding `{}` in same scope", name),
                Some("rename the binding; destructuring is not supported".into()));
        } else {
            scope.insert(name.to_string(), ty);
        }
    }

    fn lookup(&self, name: &str) -> Option<Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) { return Some(*t); }
        }
        None
    }
}
```

---

## 4) Entry point (parse + prepass + visit)

Use `syn::parse_file` (feature `parsing` + `full`) as your v1 entry point. ([Docs.rs][1])

```rust
fn validate_rust_subset(src: &str) -> Result<(), Vec<Diag>> {
    let file: syn::File = syn::parse_file(src)
        .map_err(|e| vec![Diag {
            code: "X7I_PARSE",
            span: None,
            message: format!("failed to parse Rust file: {}", e),
            help: Some("ensure the file is valid Rust and has no macros that require expansion".into()),
        }])?;

    let mut v = Validator {
        src,
        diags: vec![],
        fn_sigs: BTreeMap::new(),
        scopes: vec![],
        cur_ret: None,
        allow_literal_inference: true,
    };

    // Phase 1: collect signatures deterministically (source order).
    v.collect_fn_sigs(&file);

    // Phase 2: validate bodies via visitor.
    v.visit_file(&file);

    if v.diags.is_empty() { Ok(()) } else { Err(v.diags) }
}
```

---

## 5) Prepass: collect function signatures (deterministic)

```rust
impl<'src> Validator<'src> {
    fn collect_fn_sigs(&mut self, file: &syn::File) {
        for item in &file.items {
            if let syn::Item::Fn(item_fn) = item {
                let name = item_fn.sig.ident.to_string();

                // Validate signature shape (and still try to collect what we can).
                let sig_ok = self.validate_fn_sig_only(&item_fn.sig);
                if !sig_ok {
                    // Don’t add a signature; downstream calls to it should be "unknown".
                    continue;
                }

                let sig = self.lower_fn_sig(&item_fn.sig);
                if self.fn_sigs.contains_key(&name) {
                    self.push("X7I_DUP_FN", None,
                        format!("duplicate function `{}`", name),
                        Some("only one function with a given name is allowed in the subset".into()));
                } else {
                    self.fn_sigs.insert(name, sig);
                }
            }
        }
    }
}
```

---

## 6) Visitor: `syn::visit::Visit` skeleton (items + statements)

`syn::visit::Visit` is the cleanest way to walk the AST deterministically, and the default behavior recurses into children in field order. ([Docs.rs][2])

```rust
// Requires syn features: "visit", plus "full" for most node types.
impl<'ast, 'src> syn::visit::Visit<'ast> for Validator<'src> {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        match item {
            syn::Item::Fn(f) => self.visit_item_fn(f),

            // Everything else is unsupported in v1 (structs, impls, macros, mods, use, etc).
            _ => {
                self.push("X7I_UNSUPPORTED_ITEM", span(item),
                    "unsupported top-level item; only free functions are supported in v1",
                    Some("move code into a free function or wait for v2 (items/modules/imports)".into()));
                // Do NOT recurse into unsupported items.
            }
        }
    }

    fn visit_item_fn(&mut self, f: &'ast syn::ItemFn) {
        // 1) Validate signature.
        if !self.validate_fn_sig_only(&f.sig) {
            // Do not descend; errors already emitted.
            return;
        }

        // 2) Enter function scope.
        let ret = self.lower_return_type(&f.sig.output);
        self.cur_ret = Some(ret);

        self.enter_scope();
        for arg in &f.sig.inputs {
            self.validate_and_bind_fn_arg(arg);
        }

        // 3) Validate body.
        self.validate_block(&f.block);

        self.exit_scope();
        self.cur_ret = None;
    }
}
```

---

## 7) Signature validation helpers (deterministic diagnostic emission)

```rust
impl<'src> Validator<'src> {
    fn validate_fn_sig_only(&mut self, sig: &syn::Signature) -> bool {
        let mut ok = true;

        // Reject generics, async, const, unsafe, extern ABI, variadics, where clauses.
        if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
            ok = false;
            self.push("X7I_FN_GENERICS", span(sig),
                "generics/where-clauses are not supported in v1 subset",
                Some("remove generics; specialize to concrete types (u8/u32/i32/bytes/Vec<u8>/&[u8])".into()));
        }
        if sig.asyncness.is_some() {
            ok = false;
            self.push("X7I_FN_ASYNC", span(sig),
                "async functions are not supported in v1 subset",
                Some("use a synchronous function; async is a Phase G feature in X07".into()));
        }
        if sig.unsafety.is_some() {
            ok = false;
            self.push("X7I_FN_UNSAFE", span(sig), "unsafe fn is not supported", None);
        }
        if sig.abi.is_some() {
            ok = false;
            self.push("X7I_FN_EXTERN", span(sig), "extern ABI is not supported", None);
        }
        if sig.variadic.is_some() {
            ok = false;
            self.push("X7I_FN_VARIADIC", span(sig), "variadic args are not supported", None);
        }

        // Validate each argument type is in subset.
        for arg in &sig.inputs {
            match arg {
                syn::FnArg::Receiver(_) => {
                    ok = false;
                    self.push("X7I_FN_RECEIVER", span(arg),
                        "method receiver `self` is not supported; only free functions",
                        Some("move this function out of an impl and pass the value explicitly".into()));
                }
                syn::FnArg::Typed(pat_ty) => {
                    if !matches!(&*pat_ty.pat, syn::Pat::Ident(_)) {
                        ok = false;
                        self.push("X7I_FN_ARG_PATTERN", span(&pat_ty.pat),
                            "function arguments must be simple identifiers",
                            Some("use `fn f(x: T)` not patterns like `(x, y)`".into()));
                    }
                    if self.lower_type(&pat_ty.ty).is_none() {
                        ok = false; // lower_type emits a diag
                    }
                }
            }
        }

        // Validate return type.
        let _ = self.lower_return_type(&sig.output); // emits diag on fail
        ok
    }

    fn lower_fn_sig(&mut self, sig: &syn::Signature) -> FnSig {
        let mut params = vec![];
        for arg in &sig.inputs {
            if let syn::FnArg::Typed(pat_ty) = arg {
                let t = self.lower_type(&pat_ty.ty).unwrap_or(Ty::Unit);
                params.push(t);
            }
        }
        let ret = self.lower_return_type(&sig.output);
        FnSig { params, ret }
    }

    fn lower_return_type(&mut self, out: &syn::ReturnType) -> Ty {
        match out {
            syn::ReturnType::Default => Ty::Unit,
            syn::ReturnType::Type(_, ty) => {
                self.lower_type(ty).unwrap_or_else(|| {
                    self.push("X7I_BAD_RET_TYPE", span(out), "unsupported return type", None);
                    Ty::Unit
                })
            }
        }
    }
}
```

---

## 8) Type lowering: allow a tiny concrete set (with deterministic errors)

Use `Path::require_ident` to deterministically reject multi-segment paths when your subset requires a single identifier. ([Docs.rs][4])

```rust
impl<'src> Validator<'src> {
    fn lower_type(&mut self, ty: &syn::Type) -> Option<Ty> {
        match ty {
            syn::Type::Path(tp) => {
                // Reject qualified paths except specific whitelisted ones (if you allow them).
                let path = &tp.path;

                // If you want to only allow single-ident paths:
                let ident = match path.require_ident() {
                    Ok(id) => id.to_string(),
                    Err(_) => {
                        self.push("X7I_TYPE_PATH", span(ty),
                            "type must be a single identifier (no module paths) in v1 subset",
                            Some("use `u8`, `u32`, `i32`, `bool`, `Bytes`, `VecU8` etc".into()));
                        return None;
                    }
                };

                match ident.as_str() {
                    "u8"  => Some(Ty::U8),
                    "u32" => Some(Ty::U32),
                    "i32" => Some(Ty::I32),
                    "bool"=> Some(Ty::Bool),

                    // If your Rust subset uses Vec<u8>, handle it as Type::Path with segments;
                    // but require_ident() would fail. So either:
                    // - allow Vec<u8> via a dedicated match on Type::Path segments, OR
                    // - require a local alias type in v1 (not recommended).
                    _ => {
                        self.push("X7I_TYPE_UNSUPPORTED", span(ty),
                            format!("unsupported type `{}`", ident),
                            None);
                        None
                    }
                }
            }

            syn::Type::Reference(tr) => {
                // Only allow immutable references
                if tr.mutability.is_some() {
                    self.push("X7I_TYPE_MUT_REF", span(ty), "&mut is not supported in v1 subset", None);
                    return None;
                }
                // Allow &[u8] only (or your exact mapping table rules)
                match &*tr.elem {
                    syn::Type::Slice(ts) => self.lower_u8_slice(ts, ty),
                    _ => {
                        self.push("X7I_TYPE_REF", span(ty),
                            "only `&[u8]` references are supported in v1 subset",
                            None);
                        None
                    }
                }
            }

            syn::Type::Slice(ts) => {
                // In v1 you likely reject bare [u8] without reference
                self.push("X7I_TYPE_SLICE", span(ty),
                    "bare slice types `[T]` are not supported; use `&[u8]`",
                    None);
                None
            }

            _ => {
                self.push("X7I_TYPE_UNSUPPORTED", span(ty),
                    "unsupported type form in v1 subset",
                    None);
                None
            }
        }
    }

    fn lower_u8_slice(&mut self, ts: &syn::TypeSlice, whole: &syn::Type) -> Option<Ty> {
        match &*ts.elem {
            syn::Type::Path(tp) => {
                if let Ok(id) = tp.path.require_ident() {
                    if id == "u8" { return Some(Ty::BytesView); }
                }
                self.push("X7I_TYPE_SLICE_ELEM", span(whole),
                    "only `&[u8]` is supported (slice element must be `u8`)",
                    None);
                None
            }
            _ => {
                self.push("X7I_TYPE_SLICE_ELEM", span(whole),
                    "only `&[u8]` is supported",
                    None);
                None
            }
        }
    }
}
```

---

## 9) Statement + block validation (including macro rejection)

The `syn::Stmt` docs explain why statement-position macros are ambiguous (they can expand to local/item/expr), so rejecting them is correct for deterministic v1. ([Docs.rs][3])

```rust
enum ExprCtx { Value, Stmt } // Value means expression result is used.

impl<'src> Validator<'src> {
    fn validate_block(&mut self, block: &syn::Block) {
        self.enter_scope();

        for (idx, stmt) in block.stmts.iter().enumerate() {
            self.validate_stmt(stmt, idx == block.stmts.len() - 1);
        }

        self.exit_scope();
    }

    fn validate_stmt(&mut self, stmt: &syn::Stmt, is_last: bool) {
        match stmt {
            syn::Stmt::Local(local) => self.validate_local(local),

            syn::Stmt::Expr(expr, semi_opt) => {
                let ctx = if semi_opt.is_some() { ExprCtx::Stmt } else { ExprCtx::Value };
                let _t = self.infer_expr(expr, None, ctx);

                // Optional: if last stmt is an expr without semicolon, allow as implicit return.
                // But if your subset requires explicit `return`, enforce it deterministically here.
                if is_last && semi_opt.is_none() {
                    // If the function return is non-Unit, ensure _t matches cur_ret.
                    if let Some(ret) = self.cur_ret {
                        // If your subset allows tail expr returns:
                        self.check_type_compat(_t, ret, span(expr));
                    }
                }
            }

            syn::Stmt::Item(_item) => {
                self.push("X7I_STMT_ITEM", span(stmt),
                    "item inside function body is not supported in v1 subset",
                    Some("move items to top-level or inline the code".into()));
            }

            syn::Stmt::Macro(_m) => {
                self.push("X7I_STMT_MACRO", span(stmt),
                    "macros are not supported in v1 subset (no expansion)",
                    Some("rewrite without macros; use explicit loops and helper functions".into()));
            }
        }
    }

    fn validate_local(&mut self, local: &syn::Local) {
        // Only allow `let name[: Ty] = expr;`
        let name = match &local.pat {
            syn::Pat::Ident(pi) => pi.ident.to_string(),
            _ => {
                self.push("X7I_LET_PATTERN", span(&local.pat),
                    "let binding must be an identifier (no destructuring)",
                    None);
                return;
            }
        };

        // Type annotation is recommended for determinism; allow limited inference if you want.
        let annotated_ty: Option<Ty> = local.ty.as_ref()
            .and_then(|(_colon, ty)| self.lower_type(ty));

        let init = match &local.init {
            Some(init) => &*init.expr,
            None => {
                self.push("X7I_LET_NO_INIT", span(local),
                    "let binding must have an initializer in v1 subset",
                    None);
                return;
            }
        };

        // Infer init type with an expected type if annotated.
        let init_ty = self.infer_expr(init, annotated_ty, ExprCtx::Value);

        // If annotated, enforce compatibility.
        if let Some(t) = annotated_ty {
            self.check_type_compat(init_ty, t, span(init));
            self.declare(&name, t);
        } else if self.allow_literal_inference {
            // Only allow inference for literals, to avoid nondeterministic type inference.
            if let Some(t) = init_ty {
                self.declare(&name, t);
            } else {
                self.push("X7I_LET_NEEDS_TYPE", span(local),
                    format!("cannot infer type for `{}`; add an explicit type", name),
                    Some("example: `let x: u32 = ...;`".into()));
            }
        } else {
            self.push("X7I_LET_NEEDS_TYPE", span(local),
                format!("explicit type required for `{}` in v1 subset", name),
                None);
        }
    }
}
```

---

## 10) Expression inference + validation (the core of determinism)

This is where most subset validators become nondeterministic. The key is: **each expression variant either (a) validates and recurses deterministically, or (b) emits a single “unsupported” diag and returns None.**

Also note: in `syn`, `ExprBinary` covers `a + b` and even `a += b`, and `BinOp` includes `AddAssign`, etc. ([Docs.rs][6])

```rust
impl<'src> Validator<'src> {
    fn infer_expr(&mut self, e: &syn::Expr, expected: Option<Ty>, ctx: ExprCtx) -> Option<Ty> {
        match e {
            syn::Expr::Lit(lit) => self.infer_lit(lit, expected),

            syn::Expr::Path(p) => self.infer_path(p),

            syn::Expr::Call(call) => self.infer_call(call, expected),

            syn::Expr::MethodCall(m) => self.infer_method_call(m, expected),

            syn::Expr::Binary(b) => self.infer_binary(b, ctx),

            syn::Expr::Assign(a) => self.infer_assign(a, ctx),

            syn::Expr::If(i) => self.infer_if(i, expected),

            syn::Expr::Block(b) => {
                // Expression blocks are allowed but must be deterministic:
                // new scope, validate stmts, result type = last expr (if any) else Unit.
                self.validate_block(&b.block);
                // If you want block expressions to return a value, you must implement that.
                // v1 can reject block-expr-as-value to keep simple.
                match ctx {
                    ExprCtx::Stmt => Some(Ty::Unit),
                    ExprCtx::Value => {
                        self.push("X7I_BLOCK_EXPR", span(e),
                            "block expressions as values are not supported in v1 subset",
                            Some("lift logic into a helper function or use explicit returns".into()));
                        None
                    }
                }
            }

            syn::Expr::ForLoop(fl) => {
                if ctx == ExprCtx::Value {
                    self.push("X7I_FOR_VALUE", span(e), "for-loop cannot be used as a value", None);
                    return None;
                }
                self.enter_scope();
                // Validate `for pat in expr { ... }`
                if !matches!(&*fl.pat, syn::Pat::Ident(_)) {
                    self.push("X7I_FOR_PAT", span(&fl.pat), "for loop pattern must be an identifier", None);
                }
                // Validate iter expression: in v1, only allow `.iter()` on &[u8] or bytes.
                let _iter_ty = self.infer_expr(&fl.expr, None, ExprCtx::Value);
                self.validate_block(&fl.body);
                self.exit_scope();
                Some(Ty::Unit)
            }

            syn::Expr::While(w) => {
                if ctx == ExprCtx::Value {
                    self.push("X7I_WHILE_VALUE", span(e), "while-loop cannot be used as a value", None);
                    return None;
                }
                let cond_ty = self.infer_expr(&w.cond, Some(Ty::Bool), ExprCtx::Value);
                self.check_type_compat(cond_ty, Ty::Bool, span(&w.cond));
                self.validate_block(&w.body);
                Some(Ty::Unit)
            }

            syn::Expr::Return(r) => {
                // return expr?; enforce matches cur_ret
                let ret_ty = if let Some(expr) = &r.expr {
                    self.infer_expr(expr, self.cur_ret, ExprCtx::Value)
                } else {
                    Some(Ty::Unit)
                };
                if let Some(expected_ret) = self.cur_ret {
                    self.check_type_compat(ret_ty, expected_ret, span(r));
                }
                Some(Ty::Unit)
            }

            syn::Expr::Break(_) | syn::Expr::Continue(_) => {
                // Optional: allow only inside loops; you’d need a loop-depth counter in state.
                self.push("X7I_LOOP_CTRL", span(e), "break/continue not supported in v1 subset", None);
                None
            }

            // Many Expr variants exist; v1 rejects them deterministically.
            _ => {
                self.push("X7I_UNSUPPORTED_EXPR", span(e),
                    "unsupported expression in v1 subset",
                    Some("rewrite using literals, paths, calls, if/for/while, assignments".into()));
                None
            }
        }
    }

    fn infer_path(&mut self, p: &syn::ExprPath) -> Option<Ty> {
        // Reject qualified paths and multi-segment names in v1.
        let id = match p.path.require_ident() {
            Ok(id) => id.to_string(),
            Err(_) => {
                self.push("X7I_PATH", span(p),
                    "only simple identifiers are supported (no module paths) in v1 subset",
                    None);
                return None;
            }
        };

        if let Some(t) = self.lookup(&id) {
            return Some(t);
        }
        if let Some(sig) = self.fn_sigs.get(&id) {
            // A bare function item used as a value: disallow in v1.
            self.push("X7I_FN_AS_VALUE", span(p),
                format!("function `{}` used as a value is not supported; call it instead", id),
                None);
            return None;
        }

        self.push("X7I_UNKNOWN_NAME", span(p),
            format!("unknown name `{}`", id),
            None);
        None
    }

    fn infer_lit(&mut self, lit: &syn::ExprLit, expected: Option<Ty>) -> Option<Ty> {
        // For determinism, be strict: integer literals default to u32 unless suffixed.
        match &lit.lit {
            syn::Lit::Bool(_) => Some(Ty::Bool),

            syn::Lit::Int(li) => {
                // You can inspect suffix: li.suffix()
                // v1 rule example:
                // - if suffix is "u8"/"u32"/"i32" -> exact
                // - else default u32 (or i32), but make it deterministic and documented.
                let suffix = li.suffix();
                match suffix {
                    "u8"  => Some(Ty::U8),
                    "u32" => Some(Ty::U32),
                    "i32" => Some(Ty::I32),
                    ""    => Some(expected.unwrap_or(Ty::U32)),
                    _ => {
                        self.push("X7I_LIT_INT_SUFFIX", span(lit),
                            format!("unsupported integer literal suffix `{}`", suffix),
                            None);
                        None
                    }
                }
            }

            syn::Lit::Byte(b) => Some(Ty::U8),
            syn::Lit::ByteStr(_bs) => Some(Ty::Bytes),

            _ => {
                self.push("X7I_LIT", span(lit),
                    "unsupported literal in v1 subset",
                    Some("allowed: bool, integer, byte, byte string".into()));
                None
            }
        }
    }

    fn infer_binary(&mut self, b: &syn::ExprBinary, ctx: ExprCtx) -> Option<Ty> {
        use syn::BinOp;

        // Handle `+=` etc deterministically: only in stmt context.
        match &b.op {
            BinOp::AddAssign(_) | BinOp::SubAssign(_) | BinOp::MulAssign(_) | BinOp::DivAssign(_) => {
                if ctx == ExprCtx::Value {
                    self.push("X7I_ASSIGNOP_VALUE", span(b),
                        "compound assignment (`+=`, `-=`...) cannot be used as a value",
                        Some("use it as a statement or rewrite as `x = x + y`".into()));
                    return None;
                }
                // Validate LHS is assignable (ident path)
                let lhs_name = self.require_assignable_lhs(&b.left)?;
                let lhs_ty = self.lookup(&lhs_name).unwrap_or_else(|| {
                    self.push("X7I_UNKNOWN_NAME", span(&b.left), format!("unknown name `{}`", lhs_name), None);
                    Ty::U32
                });
                // Validate RHS matches numeric type.
                let rhs_ty = self.infer_expr(&b.right, Some(lhs_ty), ExprCtx::Value);
                self.check_type_compat(rhs_ty, lhs_ty, span(&b.right));
                Some(Ty::Unit)
            }

            // Arithmetic/logical ops: enforce numeric/bool constraints.
            BinOp::Add(_) | BinOp::Sub(_) | BinOp::Mul(_) | BinOp::Div(_) | BinOp::Rem(_) => {
                let lt = self.infer_expr(&b.left, expected, ExprCtx::Value);
                let rt = self.infer_expr(&b.right, lt, ExprCtx::Value);
                // Deterministic: require both sides same numeric type (u32 or i32) in v1
                self.check_same_numeric(lt, rt, span(b));
                lt
            }

            BinOp::Eq(_) | BinOp::Ne(_) | BinOp::Lt(_) | BinOp::Le(_) | BinOp::Gt(_) | BinOp::Ge(_) => {
                let _ = self.infer_expr(&b.left, None, ExprCtx::Value);
                let _ = self.infer_expr(&b.right, None, ExprCtx::Value);
                Some(Ty::Bool)
            }

            BinOp::And(_) | BinOp::Or(_) => {
                let lt = self.infer_expr(&b.left, Some(Ty::Bool), ExprCtx::Value);
                let rt = self.infer_expr(&b.right, Some(Ty::Bool), ExprCtx::Value);
                self.check_type_compat(lt, Ty::Bool, span(&b.left));
                self.check_type_compat(rt, Ty::Bool, span(&b.right));
                Some(Ty::Bool)
            }

            _ => {
                self.push("X7I_BINOP", span(b),
                    "unsupported binary operator in v1 subset",
                    None);
                None
            }
        }
    }

    fn infer_assign(&mut self, a: &syn::ExprAssign, ctx: ExprCtx) -> Option<Ty> {
        if ctx == ExprCtx::Value {
            self.push("X7I_ASSIGN_VALUE", span(a),
                "assignment cannot be used as a value in v1 subset",
                Some("use assignment as a statement; end it with `;`".into()));
            return None;
        }
        let lhs_name = self.require_assignable_lhs(&a.left)?;
        let lhs_ty = self.lookup(&lhs_name).unwrap_or_else(|| {
            self.push("X7I_UNKNOWN_NAME", span(&a.left), format!("unknown name `{}`", lhs_name), None);
            Ty::U32
        });
        let rhs_ty = self.infer_expr(&a.right, Some(lhs_ty), ExprCtx::Value);
        self.check_type_compat(rhs_ty, lhs_ty, span(&a.right));
        Some(Ty::Unit)
    }

    fn require_assignable_lhs(&mut self, e: &syn::Expr) -> Option<String> {
        match e {
            syn::Expr::Path(p) => {
                let id = p.path.require_ident().ok()?.to_string();
                Some(id)
            }
            _ => {
                self.push("X7I_LHS", span(e),
                    "left-hand side must be a simple identifier in v1 subset",
                    Some("no field access, indexing, deref, or patterns".into()));
                None
            }
        }
    }
}
```

---

## 11) A few “must-have” deterministic checks to match your mapping table

Depending on what you included in the mapping table, these checks are usually essential:

* **No macros anywhere** (`Expr::Macro`, `Stmt::Macro`) because no expansion happens. ([Docs.rs][3])
* **No module paths** unless explicitly whitelisted (use `Path::require_ident()` to enforce) ([Docs.rs][4])
* **No turbofish** / generic args in call paths unless explicitly supported (check `PathSegment.arguments`)
* **No mutable references** unless you decide to model borrows/views explicitly (`&mut` is a major semantic commitment)
* **No string/char literals** unless you standardize encoding rules; prefer byte strings (`b"..."`) for deterministic bytes.

---

## 12) Where this plugs into x07import

Pipeline for v1:

1. `syn::parse_file(src)` ([Docs.rs][1])
2. `validate_rust_subset(&file)` (this pass)
3. `lower_rust_to_x07ir(&file)` (assumes subset invariants hold)
4. `emit_x07(&x07ir)`
5. `diff_test` against Rust reference implementation.

---

[1]: https://docs.rs/syn/latest/syn/fn.parse_file.html?utm_source=chatgpt.com "parse_file in syn - Rust"
[2]: https://docs.rs/syn/latest/syn/visit/index.html?utm_source=chatgpt.com "syn::visit - Rust"
[3]: https://docs.rs/syn/latest/syn/enum.Stmt.html?utm_source=chatgpt.com "Stmt in syn - Rust"
[4]: https://docs.rs/syn/latest/syn/struct.Path.html?utm_source=chatgpt.com "Path in syn - Rust"
[5]: https://docs.rs/syn/latest/syn/enum.BinOp.html "BinOp in syn - Rust"
[6]: https://docs.rs/syn/latest/syn/struct.ExprBinary.html "ExprBinary in syn - Rust"
----------

Below are **concrete, PR-ready** drafts for the canonical diagnostic catalog:

* `crates/x07import/src/diagnostics.rs` — **single source of truth** (codes + default messages + help)
* `docs/x07import/diagnostics.md` — a **generated** catalog (committed for humans)

This follows the same general “stable code + explain/help” idea used by `rustc` error codes (stable IDs that tools/tests can key on).
(You can optionally integrate “pretty” reporting later with `miette` / `thiserror`, but it’s not required for v1.)

---

## `crates/x07import/src/diagnostics.rs`

```rust
//! Canonical diagnostic catalog for `x07import`.
//!
//! Design goals:
//! - **Single-sourced**: codes/messages/help live here only.
//! - **Deterministic**: stable codes, stable ordering, stable formatting.
//! - Shared by validator + lowering to prevent drift between passes.
//!
//! Notes:
//! - Codes are stable identifiers similar to rustc error codes. 
//! - Messages are intentionally short; contextual details go in `Diag.detail`.

use std::fmt;

/// The pass/stage that emitted the diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Parse,
    Validate,
    Lower,
    Emit,
}

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// Static diagnostic metadata (single-sourced).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DiagSpec {
    pub code: &'static str, // e.g. "X7I0201"
    pub stage: Stage,
    pub severity: Severity,
    pub title: &'static str,
    pub message: &'static str,
    pub help: Option<&'static str>,
}

/// Span in source file byte offsets (UTF-8 byte indices).
/// (The CLI can also convert to line/col for rendering.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// A concrete diagnostic instance.
/// - `code` chooses the canonical message/help.
/// - `detail` adds deterministic extra context (e.g. "ItemStruct", "Vec<String>").
/// - Do NOT embed large source snippets here; rendering belongs to CLI layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diag {
    pub code: DiagCode,
    pub span: Option<ByteSpan>,
    pub detail: Option<String>,
    pub notes: Vec<String>,
}

impl Diag {
    pub fn new(code: DiagCode) -> Self {
        Self {
            code,
            span: None,
            detail: None,
            notes: vec![],
        }
    }

    pub fn with_span(mut self, span: ByteSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn push_note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// Canonical base message for this code (no detail).
    pub fn base_message(&self) -> &'static str {
        self.code.spec().message
    }

    /// Full message including optional detail, in a stable format.
    pub fn full_message(&self) -> String {
        match &self.detail {
            Some(d) if !d.is_empty() => format!("{}: {}", self.base_message(), d),
            _ => self.base_message().to_string(),
        }
    }
}

/// Defines the catalog once and generates:
/// - `DiagCode` enum
/// - `DiagCode::spec()`
/// - `ALL_DIAG_SPECS` (stable ordering for docs / tooling)
/// - `DiagCode::from_code()`
macro_rules! diag_catalog {
    (
        $(
            $variant:ident => {
                code: $code:literal,
                stage: $stage:ident,
                severity: $sev:ident,
                title: $title:literal,
                message: $msg:literal,
                help: $help:expr
            }
        ),+ $(,)?
    ) => {
        /// Stable diagnostic identifiers for x07import.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum DiagCode {
            $($variant),+
        }

        impl DiagCode {
            pub const fn spec(self) -> DiagSpec {
                match self {
                    $(
                        DiagCode::$variant => DiagSpec {
                            code: $code,
                            stage: Stage::$stage,
                            severity: Severity::$sev,
                            title: $title,
                            message: $msg,
                            help: $help,
                        }
                    ),+
                }
            }

            pub const fn code(self) -> &'static str { self.spec().code }
            pub const fn stage(self) -> Stage { self.spec().stage }
            pub const fn severity(self) -> Severity { self.spec().severity }
            pub const fn title(self) -> &'static str { self.spec().title }
            pub const fn message(self) -> &'static str { self.spec().message }
            pub const fn help(self) -> Option<&'static str> { self.spec().help }

            pub fn from_code(code: &str) -> Option<DiagCode> {
                match code {
                    $(
                        $code => Some(DiagCode::$variant),
                    )+
                    _ => None,
                }
            }
        }

        impl fmt::Display for DiagCode {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.code())
            }
        }

        /// Stable-ordered list of specs for doc generation and tooling.
        ///
        /// IMPORTANT: Keep this list sorted by `code` lexicographically and unique.
        pub const ALL_DIAG_SPECS: &[DiagSpec] = &[
            $(
                DiagSpec {
                    code: $code,
                    stage: Stage::$stage,
                    severity: Severity::$sev,
                    title: $title,
                    message: $msg,
                    help: $help,
                },
            )+
        ];
    };
}

// ===============================
// v1 Catalog (Rust subset → x07IR)
// ===============================
//
// Conventions:
// - “Unsupported …” diagnostics should include detail with the encountered kind/type.
// - Validator should emit the MOST specific code available.
// - Lowering should use Validate codes where possible; use InternalBug only for invariant breaks.

diag_catalog! {
    // ---- Parse / IO ----
    ParseFile => {
        code: "X7I0001",
        stage: Parse,
        severity: Error,
        title: "Parse error",
        message: "failed to parse Rust source file",
        help: Some("Ensure the file is valid Rust (edition 2021) and contains only supported items for x07import v1 (functions).")
    },

    // ---- Top-level items ----
    UnsupportedItem => {
        code: "X7I0101",
        stage: Validate,
        severity: Error,
        title: "Unsupported item",
        message: "unsupported top-level item",
        help: Some("v1 supports only free functions (`fn`). Remove or move other items (struct/enum/impl/mod/use/const/static/macro).")
    },
    DuplicateFunction => {
        code: "X7I0102",
        stage: Validate,
        severity: Error,
        title: "Duplicate function",
        message: "duplicate function name",
        help: Some("Rename one of the functions or merge them; exported module symbols must be unique.")
    },

    // ---- Function signature constraints ----
    FnGenerics => {
        code: "X7I0201",
        stage: Validate,
        severity: Error,
        title: "Generics unsupported",
        message: "generic functions are not supported in v1",
        help: Some("Remove `<T>` generics and `where` clauses; specialize the function to concrete types.")
    },
    FnAsync => {
        code: "X7I0202",
        stage: Validate,
        severity: Error,
        title: "async unsupported",
        message: "async functions are not supported in v1",
        help: Some("Rewrite as a synchronous function (no `.await`).")
    },
    FnUnsafe => {
        code: "X7I0203",
        stage: Validate,
        severity: Error,
        title: "unsafe unsupported",
        message: "unsafe functions are not supported in v1",
        help: Some("Remove `unsafe` and unsafe operations; x07import v1 does not lower unsafe blocks.")
    },
    FnExtern => {
        code: "X7I0204",
        stage: Validate,
        severity: Error,
        title: "extern unsupported",
        message: "extern ABI functions are not supported in v1",
        help: Some("Remove `extern \"...\"`; only normal Rust functions are supported.")
    },
    FnVariadic => {
        code: "X7I0205",
        stage: Validate,
        severity: Error,
        title: "variadic unsupported",
        message: "variadic functions are not supported in v1",
        help: Some("Replace variadic args with a slice/Vec parameter.")
    },
    FnReceiver => {
        code: "X7I0206",
        stage: Validate,
        severity: Error,
        title: "method receiver unsupported",
        message: "`self` receiver parameters are not supported in v1",
        help: Some("Rewrite as a free function `fn f(x: &T, ...)` or implement in X07 directly.")
    },
    FnParamPattern => {
        code: "X7I0207",
        stage: Validate,
        severity: Error,
        title: "parameter pattern unsupported",
        message: "unsupported function parameter pattern",
        help: Some("Use simple identifier parameters (e.g., `fn f(x: i32)`), not destructuring patterns.")
    },
    FnOutputType => {
        code: "X7I0208",
        stage: Validate,
        severity: Error,
        title: "return type unsupported",
        message: "unsupported function return type",
        help: Some("Use supported returns (e.g., `()`, `bool`, `u8`, `i32`, `Vec<u8>`, `&[u8]` view if enabled).")
    },

    // ---- Types ----
    TypeUnsupported => {
        code: "X7I0301",
        stage: Validate,
        severity: Error,
        title: "Type unsupported",
        message: "unsupported type",
        help: Some("v1 supports a small set of scalar and byte-oriented types; see the Rust subset mapping table.")
    },
    TypeMutRef => {
        code: "X7I0302",
        stage: Validate,
        severity: Error,
        title: "Mutable reference unsupported",
        message: "mutable references are not supported in v1",
        help: Some("Replace `&mut T` with owned values or rewrite using explicit builder APIs (e.g., Vec).")
    },
    TypeRefTargetUnsupported => {
        code: "X7I0303",
        stage: Validate,
        severity: Error,
        title: "Reference target unsupported",
        message: "unsupported reference type target",
        help: Some("v1 supports only specific borrowed forms (typically `&[u8]` / `&str` if enabled).")
    },
    TypeGenericUnsupported => {
        code: "X7I0304",
        stage: Validate,
        severity: Error,
        title: "Generic type unsupported",
        message: "unsupported generic type usage",
        help: Some("Remove generic args (e.g., `Vec<T>` must be `Vec<u8>` in v1).")
    },

    // ---- Statements ----
    StmtItemUnsupported => {
        code: "X7I0401",
        stage: Validate,
        severity: Error,
        title: "Item statement unsupported",
        message: "unsupported item inside function body",
        help: Some("Move nested items (fn/struct/impl) to the module level, or inline logic.")
    },
    StmtMacroUnsupported => {
        code: "X7I0402",
        stage: Validate,
        severity: Error,
        title: "Macro unsupported",
        message: "macros are not supported in v1",
        help: Some("Replace macro invocations with equivalent explicit Rust code in the supported subset.")
    },
    LetPatternUnsupported => {
        code: "X7I0403",
        stage: Validate,
        severity: Error,
        title: "let pattern unsupported",
        message: "unsupported `let` pattern",
        help: Some("Use `let name = expr;` (identifier binding only), not destructuring patterns.")
    },
    LetMissingInit => {
        code: "X7I0404",
        stage: Validate,
        severity: Error,
        title: "missing initializer",
        message: "`let` binding must have an initializer in v1",
        help: Some("Use `let x = <expr>;` (no uninitialized locals).")
    },

    // ---- Expressions / control flow ----
    ExprUnsupported => {
        code: "X7I0501",
        stage: Validate,
        severity: Error,
        title: "Expression unsupported",
        message: "unsupported expression kind",
        help: Some("Rewrite using the supported expression subset (if/while/for over ranges, calls, simple ops).")
    },
    ExprMatchUnsupported => {
        code: "X7I0502",
        stage: Validate,
        severity: Error,
        title: "match unsupported",
        message: "`match` expressions are not supported in v1",
        help: Some("Rewrite using `if`/`else` chains.")
    },
    ExprClosureUnsupported => {
        code: "X7I0503",
        stage: Validate,
        severity: Error,
        title: "closure unsupported",
        message: "closures are not supported in v1",
        help: Some("Inline the closure body or rewrite as a named helper function.")
    },
    ExprTryUnsupported => {
        code: "X7I0504",
        stage: Validate,
        severity: Error,
        title: "try operator unsupported",
        message: "the `?` operator is not supported in v1",
        help: Some("Rewrite using explicit `match`/`if` on Result/Option (or avoid Result/Option in v1).")
    },
    ExprBreakUnsupported => {
        code: "X7I0505",
        stage: Validate,
        severity: Error,
        title: "break unsupported",
        message: "`break` is not supported in v1",
        help: Some("Rewrite loops to use `return` for early exit, or restructure as bounded for-range loops without break.")
    },
    ExprContinueUnsupported => {
        code: "X7I0506",
        stage: Validate,
        severity: Error,
        title: "continue unsupported",
        message: "`continue` is not supported in v1",
        help: Some("Rewrite loop bodies with an `if` guard to skip work.")
    },
    MethodUnsupported => {
        code: "X7I0507",
        stage: Validate,
        severity: Error,
        title: "method unsupported",
        message: "unsupported method call",
        help: Some("Only a small, whitelisted set of methods are supported; rewrite as free functions or primitives.")
    },
    AssignOpUnsupported => {
        code: "X7I0508",
        stage: Validate,
        severity: Error,
        title: "assignment operator unsupported",
        message: "unsupported assignment operator",
        help: Some("Use plain `=` assignment; compound assigns like `+=` may be unsupported (rewrite as `x = x + y`).")
    },

    // ---- Name resolution ----
    UnknownName => {
        code: "X7I0601",
        stage: Validate,
        severity: Error,
        title: "Unknown name",
        message: "unknown name",
        help: Some("Check spelling, ensure the symbol is in scope, or add it as an imported/translated function.")
    },
    DuplicateLocal => {
        code: "X7I0602",
        stage: Validate,
        severity: Error,
        title: "Duplicate local",
        message: "duplicate local binding",
        help: Some("Rename one of the locals; x07import v1 requires unique local names per function scope.")
    },

    // ---- Internal invariants ----
    InternalBug => {
        code: "X7I0901",
        stage: Lower,
        severity: Error,
        title: "Internal bug",
        message: "internal x07import bug (unexpected AST shape after validation)",
        help: Some("This is a bug in x07import. Please file an issue with the Rust input file and the diagnostic trace.")
    },
}

/// Helper: generate deterministic Markdown catalog for docs.
/// Keep `diagnostics.md` in sync by running the generator and checking in the result.
pub fn render_diagnostics_md() -> String {
    // Stable header.
    let mut out = String::new();
    out.push_str("# x07import diagnostics catalog\n\n");
    out.push_str("This document is generated from `crates/x07import/src/diagnostics.rs`.\n");
    out.push_str("Do not edit by hand; update the Rust catalog and regenerate.\n\n");

    out.push_str("| Code | Stage | Severity | Title | Default message | Help |\n");
    out.push_str("|---|---|---|---|---|---|\n");

    for spec in ALL_DIAG_SPECS {
        let stage = match spec.stage {
            Stage::Parse => "Parse",
            Stage::Validate => "Validate",
            Stage::Lower => "Lower",
            Stage::Emit => "Emit",
        };

        let help = spec.help.unwrap_or("");
        // Keep markdown escaping minimal; avoid `|` in help strings.
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            spec.code,
            stage,
            spec.severity.as_str(),
            spec.title,
            spec.message,
            help
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn codes_are_unique_and_sorted() {
        let mut seen = HashSet::new();
        let mut prev = "";
        for spec in ALL_DIAG_SPECS {
            assert!(seen.insert(spec.code), "duplicate diagnostic code: {}", spec.code);
            assert!(spec.code > prev, "diagnostic codes must be strictly increasing: {} <= {}", spec.code, prev);
            prev = spec.code;
        }
    }

    #[test]
    fn from_code_roundtrips() {
        for spec in ALL_DIAG_SPECS {
            let c = DiagCode::from_code(spec.code).expect("missing from_code mapping");
            assert_eq!(c.spec().code, spec.code);
        }
    }
}
```

### Why this satisfies “single-source”

* Developers only touch the `diag_catalog! { ... }` block once.
* Validator and lowering share `DiagCode` + `Diag` and must not invent their own strings.
* `docs/x07import/diagnostics.md` is **generated** from `render_diagnostics_md()` so it cannot drift (you can enforce in CI by regenerating and `git diff --exit-code`).

(If you later want richer display with spans and source snippets, `miette` is a good optional layer. )

---

## `docs/x07import/diagnostics.md`

```md
# x07import diagnostics catalog

This document is generated from `crates/x07import/src/diagnostics.rs`.
Do not edit by hand; update the Rust catalog and regenerate.

The diagnostic codes are stable identifiers (similar in spirit to rustc error codes). 

## Table

| Code | Stage | Severity | Title | Default message | Help |
|---|---|---|---|---|---|
| X7I0001 | Parse | error | Parse error | failed to parse Rust source file | Ensure the file is valid Rust (edition 2021) and contains only supported items for x07import v1 (functions). |
| X7I0101 | Validate | error | Unsupported item | unsupported top-level item | v1 supports only free functions (`fn`). Remove or move other items (struct/enum/impl/mod/use/const/static/macro). |
| X7I0102 | Validate | error | Duplicate function | duplicate function name | Rename one of the functions or merge them; exported module symbols must be unique. |
| X7I0201 | Validate | error | Generics unsupported | generic functions are not supported in v1 | Remove `<T>` generics and `where` clauses; specialize the function to concrete types. |
| X7I0202 | Validate | error | async unsupported | async functions are not supported in v1 | Rewrite as a synchronous function (no `.await`). |
| X7I0203 | Validate | error | unsafe unsupported | unsafe functions are not supported in v1 | Remove `unsafe` and unsafe operations; x07import v1 does not lower unsafe blocks. |
| X7I0204 | Validate | error | extern unsupported | extern ABI functions are not supported in v1 | Remove `extern "..."`; only normal Rust functions are supported. |
| X7I0205 | Validate | error | variadic unsupported | variadic functions are not supported in v1 | Replace variadic args with a slice/Vec parameter. |
| X7I0206 | Validate | error | method receiver unsupported | `self` receiver parameters are not supported in v1 | Rewrite as a free function `fn f(x: &T, ...)` or implement in X07 directly. |
| X7I0207 | Validate | error | parameter pattern unsupported | unsupported function parameter pattern | Use simple identifier parameters (e.g., `fn f(x: i32)`), not destructuring patterns. |
| X7I0208 | Validate | error | return type unsupported | unsupported function return type | Use supported returns (e.g., `()`, `bool`, `u8`, `i32`, `Vec<u8>`, `&[u8]` view if enabled). |
| X7I0301 | Validate | error | Type unsupported | unsupported type | v1 supports a small set of scalar and byte-oriented types; see the Rust subset mapping table. |
| X7I0302 | Validate | error | Mutable reference unsupported | mutable references are not supported in v1 | Replace `&mut T` with owned values or rewrite using explicit builder APIs (e.g., Vec). |
| X7I0303 | Validate | error | Reference target unsupported | unsupported reference type target | v1 supports only specific borrowed forms (typically `&[u8]` / `&str` if enabled). |
| X7I0304 | Validate | error | Generic type unsupported | unsupported generic type usage | Remove generic args (e.g., `Vec<T>` must be `Vec<u8>` in v1). |
| X7I0401 | Validate | error | Item statement unsupported | unsupported item inside function body | Move nested items (fn/struct/impl) to the module level, or inline logic. |
| X7I0402 | Validate | error | Macro unsupported | macros are not supported in v1 | Replace macro invocations with equivalent explicit Rust code in the supported subset. |
| X7I0403 | Validate | error | let pattern unsupported | unsupported `let` pattern | Use `let name = expr;` (identifier binding only), not destructuring patterns. |
| X7I0404 | Validate | error | missing initializer | `let` binding must have an initializer in v1 | Use `let x = <expr>;` (no uninitialized locals). |
| X7I0501 | Validate | error | Expression unsupported | unsupported expression kind | Rewrite using the supported expression subset (if/while/for over ranges, calls, simple ops). |
| X7I0502 | Validate | error | match unsupported | `match` expressions are not supported in v1 | Rewrite using `if`/`else` chains. |
| X7I0503 | Validate | error | closure unsupported | closures are not supported in v1 | Inline the closure body or rewrite as a named helper function. |
| X7I0504 | Validate | error | try operator unsupported | the `?` operator is not supported in v1 | Rewrite using explicit `match`/`if` on Result/Option (or avoid Result/Option in v1). |
| X7I0505 | Validate | error | break unsupported | `break` is not supported in v1 | Rewrite loops to use `return` for early exit, or restructure as bounded for-range loops without break. |
| X7I0506 | Validate | error | continue unsupported | `continue` is not supported in v1 | Rewrite loop bodies with an `if` guard to skip work. |
| X7I0507 | Validate | error | method unsupported | unsupported method call | Only a small, whitelisted set of methods are supported; rewrite as free functions or primitives. |
| X7I0508 | Validate | error | assignment operator unsupported | unsupported assignment operator | Use plain `=` assignment; compound assigns like `+=` may be unsupported (rewrite as `x = x + y`). |
| X7I0601 | Validate | error | Unknown name | unknown name | Check spelling, ensure the symbol is in scope, or add it as an imported/translated function. |
| X7I0602 | Validate | error | Duplicate local | duplicate local binding | Rename one of the locals; x07import v1 requires unique local names per function scope. |
| X7I0901 | Lower | error | Internal bug | internal x07import bug (unexpected AST shape after validation) | This is a bug in x07import. Please file an issue with the Rust input file and the diagnostic trace. |

## Format guideline

Suggested stable CLI format:

- `X7I#### <severity>: <default message>: <detail?>`
- followed by a `--> file:line:col` line (if available)
- then `help:` and `note:` lines.

The exact rendering is up to x07import CLI, but the **code** and **default message/help** must come from the catalog.
```

---

## Optional: how to enforce “no drift” in CI

Not required by your request, but easy and worth it:

* Add a tiny `crates/x07import/src/bin/gen_diagnostics_md.rs`:

  * prints `render_diagnostics_md()`

* CI step:

  * `cargo run -p x07import --bin gen_diagnostics_md > docs/x07import/diagnostics.md`
  * `git diff --exit-code docs/x07import/diagnostics.md`

This ensures validator/lowering can’t silently diverge from docs.

If you want richer human output later, you can wrap `Diag` into a `miette::Diagnostic` report for pretty printing.
------
