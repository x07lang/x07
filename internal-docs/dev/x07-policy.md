# X07 Placement Policy v1

(Deterministic decision procedure for feature placement across compiler/runtime kernel, stdlib, external packages, and host adapters/tools.)

(Deterministic, LLM‑first, single C backend)

## Goals this policy optimizes for

1. **Determinism & comparability** (same inputs → same outputs/metrics).
2. **LLM reliability** (avoid fragile syntax/pattern traps).
3. **Minimal trusted computing base (TCB)** (small compiler/runtime kernel).
4. **Reproducible builds** (no ambient environment dependencies like file ordering/time). ([Reproducible Builds][1])
5. **Stable semantics** (stdlib changes without hidden rewrites or ambient dependencies).

---

# 0) Determinism classes (the first “routing” decision)

Every feature/API must be labeled as one of:

### A. Pure deterministic

* No filesystem/network/time/env/process.
* Deterministic by construction.

### B. Deterministic “fixture world” (evaluation worlds)

* I/O allowed **only via world-scoped adapters** (fixture FS, fixture RR, deterministic KV).
* No ambient OS calls.
* Directory ordering and any “randomness” must be fixed/seeded/pinned. ([Reproducible Builds][2])

### C. OS world (standalone-only)

* Real filesystem/network/time/env/process.
* Not used in deterministic suites; not comparable; can be sandboxed.

This matters because a lot of languages’ “standard” APIs are not deterministic by default (e.g., typical hash maps iterate in arbitrary order and may vary run-to-run). Rust’s `HashMap` explicitly documents “arbitrary order,” and its default `RandomState` is intentionally randomized, producing different iteration orders to mitigate DoS risks. ([Rust Documentation][3])
Your deterministic tiers must avoid this by design (or define deterministic “emit”/ordering rules).

---

# 1) What goes in the compiler/runtime kernel (trusted code)

## 1.1 MUST be in compiler/runtime if it affects **semantics, safety, or determinism**

Put it here if **any** of these are true:

### A) It defines the language’s semantic model

Examples:

* Integer semantics (wrap rules, shifts, comparisons).
* Evaluation order rules.
* Concurrency scheduling semantics (for deterministic scheduling).

### B) It enforces memory safety invariants that cannot be expressed reliably as library code

Examples:

* Ownership/move invalidation (use-after-move must be compile error).
* Borrow/view lifetime tracking in debug (borrow violations tracking).
* Leak gates / mem_stats instrumentation.

### C) It creates/guards capability boundaries

Examples:

* “solve-* worlds” forbid OS calls.
* `run-os` / `run-os-sandboxed` policy enforcement.

### D) It prevents C undefined behavior in generated code

With a C backend, the compiler/runtime must be responsible for *never emitting UB*:

* Signed overflow is UB in C-family semantics; you must generate safe patterns (e.g., use `uint32_t` and explicit wrap) rather than relying on signed overflow. ([gnu.org][4])
* Shifts by negative or >= bit-width are UB; the compiler must mask/guard shift counts deterministically. ([Microsoft Learn][5])

**Rule:** if the only correct implementation requires emitting C that could become UB unless carefully controlled → compiler/runtime.

## 1.2 SHOULD be in compiler/runtime if it’s a deterministic optimization pass

Examples:

* Deterministic CSE, constant folding, dead code elimination.
  These are not language semantics, but they’re compiler responsibilities, and must be deterministic (stable ordering, stable hashing, no env dependence).

## 1.3 MUST NOT be in compiler/runtime if it’s “just a helper algorithm”

If it’s implementable in X07 stdlib without new primitives and without harming determinism, it should not bloat the TCB.

---

# 2) No compile-time macros

X07 intentionally has no compile-time rewrite-rule macro system.

Anything that looks like “syntax sugar” or “normalization” must be handled via:

* **Core builtins / structured forms** (compiler/runtime) if it affects semantics, safety, determinism, or C UB avoidance.
* **Stdlib APIs** for reusable algorithms and data structures.
* **Tooling normalization** (formatter/linter) for canonicalizing equivalent AST shapes without changing semantics.

---

# 3) What goes in stdlib (baseline standard library packages)

Stdlib is the **foundation of “useful” programming**, filling the gap between a working language and a usable one. ([Software Engineering Stack Exchange][7])

## 3.1 Stdlib MUST be where reusable algorithms and data structures live

Examples:

* `std.text` (ascii/utf8 scanning, normalization)
* `std.json`, `std.csv`, `std.regex-lite`
* deterministic `std.map/set` and collections
* deterministic PRNG (seeded)
* parsing/formatting utilities
* “emitters” for deterministic serialization of collections

## 3.2 Stdlib MUST be deterministic within its tier

* Pure stdlib: deterministic by construction.
* World adapters: deterministic against fixtures in eval worlds.
* No hidden time/env/randomness.

**Determinism trap to avoid:** common hash maps have “arbitrary” iteration order and can be intentionally randomized by default (e.g., Rust). ([Rust Documentation][3])
So X07’s stdlib collections should:

* either define stable iteration order,
* or define **normative emitter APIs** that sort/canonicalize outputs.

## 3.3 Stdlib SHOULD be layered (like core/alloc/std philosophy)

Rust splits functionality into `core` (no deps), `alloc` (heap/collections), and `std` (OS features). ([Rust Documentation][8])
Your equivalent structure for X07:

* **std.core**: pure helpers (bytes/views, math helpers, parsing primitives)
* **std.alloc**: collections, buffers, builders (Vec/Deque/Map/Set, etc.)
* **std.world.fs/rr/kv**: adapters around `std.io` traits
* **std.os.***: OS worlds only (not used in deterministic suites)

This separation makes it obvious what is allowed in deterministic evaluation and what isn’t.

## 3.4 Stdlib SHOULD be implemented in X07 (or imported) and versioned

* Prefer X07 source modules (LLM-readable, auditable).
* If imported from Rust/C (x07import), the output X07 module must still be deterministic and pinned.

---

# 4) What goes in external libraries

External packages are for **non-baseline** functionality.

## 4.1 External libraries SHOULD be used when…

* Domain-specific (crypto suites, compression codecs, SQL client protocols, etc.)
* Heavy or niche dependencies not appropriate for baseline stdlib
* Fast iteration without “stdlib stability” guarantees

## 4.2 External libraries MUST declare determinism tier

Package metadata MUST state one of:

* pure
* fixture-world (fs/rr/kv)
* os-world-only

And your build tooling MUST refuse to compile OS-world-only deps into `solve-*` worlds.

## 4.3 External libs MUST be pinned & hashed

For reproducibility, lockfiles must pin exact versions + content hashes (like `Cargo.lock` style), to avoid “silent drift.” Reproducible builds depend on controlling inputs and avoiding environment variance. ([Reproducible Builds][1])

---

# 5) “Anything else”: host adapters, tooling, skills

## 5.1 Host adapters (runner-side)

Put here anything that:

* touches the OS
* enforces sandbox policy
* mounts deterministic fixture worlds
* implements `run-os-sandboxed` policies

Deterministic suite worlds should be strict/fixture-backed; OS worlds are opt-in and never used in deterministic suites.

## 5.2 Toolchain and developer UX (LLM-first)

Put here:

* formatter
* linter
* diagnostic engine (x07diag)
* auto-repair pipeline
* package manager and lockfile generator

These are not language semantics, but they’re essential for “100% agentic coding.”

---

# 6) The deterministic decision tree (copy into docs)

Use this to decide placement:

1. **Does it require OS access (fs/net/time/env/process)?**
   → Yes: **Host adapter + `std.os.*` (OS world only)**
   → No: continue

2. **Does it define semantics or enforce safety invariants?**
   (ownership/moves, borrow rules, fuel, limits, ABI layouts, determinism gates)
   → Yes: **compiler/runtime kernel**
   → No: continue

3. **Is it syntax/shape normalization or non-local control flow sugar?**
   (avoid parse traps, `try`, `if/for` block wrappers, canonical ergonomic patterns)
   → Yes: **tooling normalization** (or a structured core form if it cannot be expressed safely as a function)
   → No: continue

4. **Is it a reusable algorithm/data structure needed broadly?**
   (text/json/maps/collections/streams/parsing/fmt)
   → Yes: **stdlib baseline**
   → No: continue

5. **Is it optional/domain-specific or heavy?**
   → Yes: **external package**
   → No: default to **stdlib** unless it needs compiler/runtime support.

---

# 7) Hard “red lines” (enforce in CI)

## 7.1 Language surface red lines

Core builtins / structured forms MUST be avoided unless they are required for semantics, safety, determinism, or codegen correctness.

## 7.2 Stdlib determinism red lines

Stdlib in deterministic tiers MUST NOT:

* depend on ambient filesystem ordering or current time ([Reproducible Builds][2])
* use randomized hashing/iteration order without canonical emitters ([Rust Documentation][3])
* include OS-world-only adapters in `solve-*`

## 7.3 C backend correctness red lines

Compiler must not emit UB-prone C constructs:

* signed overflow reliance ([gnu.org][4])
* UB shifts ([Microsoft Learn][5])
* out-of-bounds memory access
* nondeterministic libc calls

---

# 8) Practical mapping examples (so devs don’t argue endlessly)

### Example: `i32.max`, `i32.abs`, `i32.clamp`

* Prefer **stdlib** (`std.core.math`) if definable from existing primitives.
* Add a **core builtin** only when required to avoid C UB or enforce semantics/determinism.

### Example: `bytes.count_matches_u8`

* Implement as **stdlib** (`std.bytes.count_matches_u8`) unless you need new primitives.

### Example: “zero-copy slicing” and borrow checks

* The **view type** and borrow rules: **compiler/runtime**.
* Helper utilities like `std.view.split_once`: **stdlib**.
* Prefer explicit loops and stdlib helpers over new surface sugar.

### Example: Hash map iteration

* Deterministic iteration order is **stdlib contract**, because real-world HashMaps are intentionally arbitrary/randomized. ([Rust Documentation][3])
  Implement either:
* deterministic stable order for iter, **or**
* `emit_*` APIs that sort/canonicalize.

### Example: Reproducible module resolution

* Deterministic search path rules: **compiler** (no ambient scanning).

---

# 9) Recommended repo “enforcement hooks” (policy into code)

To make this policy deterministic and self-enforcing:

1. **Package metadata fields** for each stdlib/external module:

   * `determinism_tier: pure | fixture | os_only`
   * `worlds_allowed: ["solve-pure", "solve-fs", ...]`
2. **Compiler hard errors** when compiling `solve-*` and encountering `os_only`.
3. **Lockfile (`stdlib.lock`) + hash verification** (reproducibility).
4. **CI** runs:

   * determinism check (same input runs N times ⇒ identical outputs/metrics)
   * bundle of canaries across major suites

This aligns with reproducible build guidance: avoid uncontrolled environment inputs and ensure stable build behavior. ([Reproducible Builds][1])

---

[1]: https://reproducible-builds.org/ "Reproducible Builds — a set of software development practices that ..."
[2]: https://reproducible-builds.org/docs/deterministic-build-systems/ "Deterministic build systems — reproducible-builds.org"
[3]: https://doc.rust-lang.org/std/collections/struct.HashMap.html "HashMap in std::collections - Rust"
[4]: https://www.gnu.org/software/autoconf/manual/autoconf-2.63/html_node/Integer-Overflow-Basics.html "Integer Overflow Basics - Autoconf"
[5]: https://learn.microsoft.com/en-us/cpp/cpp/left-shift-and-right-shift-operators-input-and-output?view=msvc-170 "Left shift and right shift operators: << and >>"
[7]: https://softwareengineering.stackexchange.com/questions/386296/why-are-standard-libraries-not-programming-language-primitives "Why are standard libraries not programming language ..."
[8]: https://doc.rust-lang.org/core/ "core"
