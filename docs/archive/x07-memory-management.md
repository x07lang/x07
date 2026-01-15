# Memory management (native C backend)

Note: this document is archived; the current spec is `docs/spec/x07-memory-management.md`.

## Current implementation (Phase F–G1)

- Allocation is a fixed-capacity arena (`X07_MEM_CAP`) that is reset at the end of each run; there is no user-visible `free`.
- The runner captures deterministic `mem_stats` and (in debug-borrow builds) `debug_stats.borrow_violations`; suites can gate/scoring can reward low `realloc_calls`, low `memcpy_bytes`, and low `peak_live_bytes`.
- Phase G1 introduces explicit zero-copy views:
  - `bytes_view` + `view.*` builtins (`bytes.view`, `bytes.subview`, `view.slice`, `view.as_bytes`, …)
- `bytes.slice` allocates and copies into a new buffer (use views for zero-copy slicing).
- To catch misuse early, run the native runner with `--debug-borrow-checks` (Phase G1).

Memory-focused suites:

- `benchmarks/solve-*/phaseF-mem-suite.json` and `benchmarks/solve-*/phaseF-mem2-suite.json`

---

With a **C backend**, “memory management” is really two problems:

1. **What memory model X07 exposes to X07 programs** (semantics / safety / determinism).
2. **How you implement that model in C** without inheriting C’s sharp edges.

Given you’re running tuning with strict comparability metrics (fuel medians, program size, etc.), you want memory behavior that is **deterministic, bounded, and hard to misuse**. 

Below are the best options, then a recommended design path for X07, and finally concrete ways to avoid C memory hazards in the implementation.

---

## Best options for memory management (language/runtime level)

### Option A — Region/Arena allocation (recommended default for X07)

**Idea:** all allocations go into a **region (arena)**, and you free the entire region at once (e.g., end of `solve()` or end of a lexical scope).

Region-based memory management: objects are assigned to a region that can be deallocated all-at-once. ([Wikipedia][1])

**Why it’s great for X07**

* **Deterministic**: allocation is typically a simple bump pointer; freeing is “drop whole region”. (Very stable for benchmarking and tuning.)
* **Eliminates whole classes of bugs**: no per-object `free()`, so no double-free / most UAF patterns.
* **Fast** and simple to implement.
* Fits your current world shape: `solve(bytes)->bytes` naturally maps to “allocate everything for this invocation, then drop”.

**Main tradeoff**

* You can’t free individual objects early (unless you add sub-arenas), so peak memory might be higher.

**How to make it safe**

* **No pointers/handles from an arena may escape its lifetime**. This is enforced by:

  * compiler rules (“values allocated in region R cannot be returned / stored in globals unless moved to an outer region”), or
  * runtime checks in debug.

---

### Option B — Rust-like Ownership/Borrowing (strongest long-term “systems language” story)

Rust’s ownership system is explicitly designed to provide memory safety **without a garbage collector** by enforcing rules at compile time. ([Rust Documentation][2])

**Why it’s attractive**

* Lets you be “C/Rust-level” while staying memory safe.
* Great for performance and determinism (no GC pauses).

**Cost**

* Most complex to implement (type/lifetime system, borrow checking, error reporting).
* For an LLM-only language, you can simplify, but you still need a consistent rule set.

**Practical hybrid**

* Use arenas for “most stuff” + ownership/borrowing rules mainly to prevent escaping references and to enable controlled deallocation.

---

### Option C — Reference Counting / ARC (practical, deterministic, mid-complexity)

**Idea:** objects keep a reference count; free when count hits zero.

**Pros**

* Deterministic deallocation (unlike tracing GC).
* Easier than borrow-checking.
* Good interop with C runtime structures.

**Cons**

* Overhead (inc/dec on assignment / passing).
* Cycles leak unless you add cycle detection or forbid cyclic structures.

ARC can be a good “Phase G/H” feature if you later add richer heap graphs (maps/objects).

---

### Option D — Tracing Garbage Collection (easiest for “users”, hardest for your determinism goals)

**Pros**

* Simplest semantics for code generation (no lifetimes, no RC).

**Cons**

* Runtime complexity (collector, barriers if incremental).
* Harder to keep “comparable” performance for tuning; even if single-threaded, GC heuristics can create run-to-run variation.

I’d only do GC if you **truly** need it (e.g., dynamic graphs everywhere) and you accept a heavier runtime.

---

## What I’d recommend for X07 (safe, deterministic, C-friendly)

### Baseline: “Arena-per-solve” + “value structs and opaque handles”

For your current shape (`solve(bytes)->bytes`) and Phase F deterministic worlds, the safest and simplest design is:

1. **All X07 allocations use an arena tied to the invocation**

   * The runtime creates an arena at entry.
   * Every `bytes.alloc`, `vec_u8.with_capacity`, etc. allocates from that arena.
   * Arena is destroyed at function exit.

2. **X07 values are ABI-stable structs and opaque handles**

* Programs never see a raw pointer.
* Generated C uses ABI-stable value layouts for slices/buffers (`bytes`, `bytes_view`, `vec_u8`) plus opaque handles for some runtime objects (`iface`, maps/sets, bufread, chans, tasks). See `docs/spec/abi/abi-v1.md`.

3. **No user-visible `free()` in Phase F**

* In a deterministic, sandboxed evaluation setting, explicit `free` mostly creates failure modes (double free, use-after-free).
* If you need early frees later:

  * add **sub-arenas**: `with_region { ... }` (free that region at end)
  * or add `free()` only under an “unsafe/advanced” feature with verifier rules.

### Add memory safety invariants in the spec (even if surface syntax is opaque)

* Every `bytes` value must carry its `len` (and maybe `cap`), and every access is bounds-checked.
* `bytes.get_u8(b, i)` traps or returns a specified value on OOB (choose one and make it stable).
* `bytes.slice` traps if out of bounds; use `std.bytes.slice` for a clamping helper.

This turns “C’s memory unsafety” into “X07 runtime checks”, which you control.

---

## How to avoid C memory problems in the implementation

Even with a safe language model, your C runtime can still have bugs. Here’s how to harden it.

### 1) Make generated C “boring and safe”

**Rule:** generated C should never do:

* pointer arithmetic on raw pointers,
* manual `free()` except via your runtime,
* unchecked array indexing.

Instead, generate calls into a small audited runtime:

* `rt_bytes_get_u8(handle, idx)`
* `rt_bytes_set_u8(handle, idx, val)`
* `rt_alloc_bytes(len)` (arena)
* `rt_vec_push(...)`, etc.

### 2) Use sanitizers as mandatory gates in CI (they catch the classic C failures)

* **AddressSanitizer (ASan)** detects memory access bugs like use-after-free and buffer overflows. ([Clang][3])
* **UndefinedBehaviorSanitizer (UBSan)** catches many forms of C/C++ undefined behavior, including out-of-bounds, misaligned pointers, signed integer overflow, etc. ([LLVM Releases][4])

Practical policy:

* Every PR runs: `-fsanitize=address,undefined` builds of the runtime and a representative benchmark slice.
* If you run “fast CI”, at least run UBSan always; ASan nightly.

### 3) Avoid UB by construction (especially integer semantics)

If X07’s integer model is “wraparound” (mod 2^32), never implement it with signed overflow in C. Implement arithmetic on `uint32_t` and use explicit conversions for signed comparisons.

(UBSan explicitly calls out signed integer overflow as UB it can detect.) ([LLVM Releases][4])

### 4) Add debug heap features (cheap, high value)

In debug builds:

* keep an allocation table (handle → {ptr,len,alive,arena_id})
* poison freed regions (or mark `alive=false`)
* validate every handle use (double-free, UAF)
* optionally add canaries around buffers

These checks make “bad runtime bugs” show up early and deterministically.

### 5) Make memory limits explicit and enforceable

Since you score deterministically, define:

* max total arena bytes per invocation
* max number of allocations
* max live handles

If the program exceeds limits:

* fail deterministically with a specific error (not OS OOM behavior).

This prevents tuning from “winning” by exhausting memory or depending on OS allocator quirks.

---

## A safe “roadmap” for memory management tuning

### Phase now (C backend, Phase F worlds)

* **Arena-per-solve** (default)
* **No explicit free**
* Handles + bounds checks
* Sanitizers and debug heap

### Later (when you want C/Rust-level long-running programs)

Pick one of:

* **Scoped regions** (`with_region`) + move rules (most deterministic, still simple)
* **Ownership/borrowing** (strongest long-term safety story; like Rust’s rules) ([Rust Documentation][2])
* **ARC** for shared structures (if you introduce graphs/maps heavily)

You can even combine:

* arenas for most ephemeral objects,
* ARC for long-lived/shared objects,
* and restrict what can escape scope boundaries.

---

## Quick “decision guide” for you

If you want **maximum safety and simplicity** with minimal language complexity:
✅ **Arena/region** as the default.

If you want “X07 is truly a systems language competitor” long-term:
✅ **Arenas now**, then add a **Rust-like ownership layer** later (even a simplified form) because ownership is specifically meant to provide memory safety without GC. ([Rust Documentation][2])

If you want “easy sharing/persistent objects” and can tolerate overhead:
✅ **ARC** (with cycle strategy).

---

If you tell me one more thing — **do you intend X07 programs to be long-running processes** (servers), or still “transactional” (`solve()`-style) even in Phase F? — I’ll recommend a single best memory model and an implementation plan (runtime structs + verifier rules + CI gates) that matches your exact direction.

[1]: https://en.wikipedia.org/wiki/Region-based_memory_management?utm_source=chatgpt.com "Region-based memory management"
[2]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html?utm_source=chatgpt.com "Understanding Ownership"
[3]: https://clang.llvm.org/docs/AddressSanitizer.html?utm_source=chatgpt.com "AddressSanitizer — Clang 22.0.0git documentation - LLVM"
[4]: https://releases.llvm.org/19.1.0/tools/clang/docs/UndefinedBehaviorSanitizer.html?utm_source=chatgpt.com "UndefinedBehaviorSanitizer — Clang 19.1.0 documentation"
+++++++

If X07 is meant to be “general purpose like C or Rust”, your memory model has to support:

* **long‑running programs** (servers, tools, daemons) that can *reclaim memory incrementally*, not only “free everything at end of solve()”
* **systems‑grade performance** (predictable overhead, no surprise pauses unless you choose GC)
* **safety-by-default** (no use‑after‑free, double free, OOB reads/writes in *safe* code)
* an **escape hatch** for low-level work (FFI, OS calls, custom allocators) that is explicitly marked unsafe

Also: because you already score things like runtime/fuel and you care about comparability and determinism in evaluation, the memory system must be *predictable and bounded* in the “tuning arena” even if the general-purpose runtime later allows richer behavior. 

Below are the best options and a concrete recommendation that avoids “C backend” pitfalls.

---

## The memory-management options (and what they imply)

### 1) Manual `malloc/free` (C-style)

* **Pros:** maximal control, minimal language complexity
* **Cons:** all the classic C failure modes (UAF, double free, leaks, OOB, lifetime confusion)

This is the **worst default** for an LLM-only language: tuning and solvers will eventually exploit footguns, and you’ll spend most time chasing runtime issues.

### 2) Ownership + borrowing (Rust-style, “safe systems”)

Rust’s core claim here is: ownership enables memory safety guarantees **without a garbage collector**, by enforcing a set of rules at compile time. ([Rust Documentation][1])

* **Pros:** best “C/Rust-class” story: predictable performance, deterministic destruction (RAII), high safety
* **Cons:** compiler complexity (borrow checking, lifetimes, error messages)

### 3) Reference counting / ARC (Swift-style)

* **Pros:** deterministic reclamation; simpler than borrow checker; good for shared data
* **Cons:** overhead (inc/dec); **cycles leak** unless you add weak refs/cycle handling

### 4) Tracing GC (Go/Java-style)

* **Pros:** easiest semantics for authors; no ownership friction
* **Cons:** runtime complexity and potential pauses; harder to keep strict “benchmark comparability” unless you’re very careful

### 5) Regions / arenas (Hanson / Cyclone / “memory contexts”)

Region-based memory management assigns objects to a region that can be freed all at once. ([Wikipedia][2])

* **Pros:** extremely fast allocation/deallocation, deterministic, great for temporary data
* **Cons:** **not sufficient alone** for general-purpose long-running programs unless you structure everything into “phases” and free whole regions regularly

> In practice, systems languages often use regions/arenas as an optimization tool, but still need either ownership/free, RC, or GC for general heap objects.

---

## Recommended X07 design: “Rust-like safe heap + optional arenas + explicit unsafe”

If your target is “like C or Rust” (performance + control + safety), the best default is:

### A) Safe-by-default ownership for heap objects

Core rule set (Rust-like):

* Every heap object has exactly one **owner**.
* Moves transfer ownership; copying is explicit (`clone`).
* When the owner goes out of scope, destruction is deterministic (RAII).
* References are *borrows*:

  * `&T` shared (read-only)
  * `&mut T` unique (read/write)
* Borrowing rules prevent aliasing mutable references and prevent references from outliving the owner (enforced by the compiler). ([Rust Documentation][1])

You do **not** need to copy Rust exactly. For an LLM-first language you can make the syntax/IR very regular, and you can start with a simplified rule set:

**Pragmatic simplification for X07**

* Step 1: implement **ownership + move-only** values + deterministic drop
* Step 2: add **borrows** but restrict them to *lexical blocks* (no fancy inference at first)
* Step 3: add richer lifetime inference once the model and tooling are stable

### B) Add ARC types only where you truly need shared ownership

Provide library-level shared ownership types:

* `Rc<T>` (single-threaded, cheap)
* `Arc<T>` (thread-safe, atomic)

Make them explicit so you can still reason about performance, and so tuning can be scored against “cheaper” ownership-only solutions.

### C) Provide region/arena allocation as a *first-class optimization tool*

Keep regions/arenas because they’re perfect for:

* parsing (temporary ASTs)
* request-scoped allocations in servers
* compiler/runtime internal work

But don’t make arenas the only heap mechanism.

Region idea (from region-based memory management): group allocations into a region and free the region at once. ([Wikipedia][2])

A good X07 story is:

* “Normal heap objects”: ownership/borrowing / drop
* “Bulk temporary objects”: arena scopes (`with_region { … }`)

### D) “Unsafe” is explicit, small, and quarantined

You will eventually need:

* raw pointers
* FFI handles
* manual allocator hooks
* memory-mapped IO, etc.

Put these behind an explicit `unsafe` boundary and restrict what can be used in tuning runs (e.g., forbid unsafe in “competitive” suites).

This matches the “C/Rust-class language” expectation: safe core + explicit unsafe escape hatch.

---

## How to avoid C memory issues even though your backend is C

The backend being C doesn’t force C’s memory unsafety on your users—**unless you expose it**. You can keep X07 safe by making your generated C very constrained and pushing dangerous operations into a small audited runtime.

### 1) Do not expose raw pointers to X07 code

Use **opaque handles** and runtime-checked accessors.

Instead of generated C doing:

* `ptr[i] = …`

Generate calls like:

* `rt_bytes_get_u8(handle, i)`
* `rt_bytes_set_u8(handle, i, v)`
* `rt_vec_push(vec_handle, v)`

This gives you:

* bounds checks
* provenance checks (correct heap, correct region)
* predictable errors (trap/return error) instead of silent corruption

### 2) Implement “debug heap” checks in your runtime

In debug builds:

* generation counters (detect use-after-free by stale handles)
* poisoned freed blocks
* redzones/canaries around allocations
* allocation table tracking

These checks make it much harder for runtime bugs to survive.

### 3) Make sanitizers mandatory in CI for the runtime and codegen tests

This is *the* best way to catch C memory mistakes early.

**AddressSanitizer (ASan)** is specifically designed to detect memory errors like out-of-bounds and use-after-free (it’s compiler instrumentation + runtime). ([Clang][3])

**UndefinedBehaviorSanitizer (UBSan)** instruments code to detect undefined behavior; common causes include signed integer overflow, misaligned pointers, null pointer misuse, etc. ([Fuchsia][4])

Policy that works well:

* PR CI: `-fsanitize=undefined` always; ASan at least on a nightly job (ASan can slow builds/runs)
* Always run a representative benchmark subset under sanitizers (not just unit tests)

### 4) Make integer semantics defined (avoid UB-by-overflow)

If your language wants wraparound arithmetic, **never implement it using signed overflow in C**. Use `uint32_t/uint64_t` for arithmetic and implement signed comparisons explicitly.

This pairs well with UBSan because signed overflow is a classic UB it can detect. ([Fuchsia][4])

### 5) Enforce hard memory limits at runtime

For safety and predictability:

* max heap bytes
* max allocations
* max object sizes
* deterministic OOM behavior (return error/trap), not “OS killed you”

This matters both for general-purpose reliability and for preventing “tuning hacks”.

---

## What this gives you long term

With the recommended model you can credibly aim at “C/Rust-class” usage:

* **Performance & predictability:** ownership/RAII + optional arenas
* **Safety:** safe code cannot UAF/double free/OOB by construction
* **Control when needed:** explicit unsafe + FFI
* **Implementable on C backend:** generated C is simple; runtime is small and heavily tested (sanitizers)

And you can evolve it incrementally:

1. unique ownership + RAII + safe `Vec/String/Map`
2. lexical borrows + slices/views
3. ARC for shared structures + (later) concurrency primitives
4. unsafe + FFI maturity

---

## If you want one “best answer” to adopt now

**Adopt: ownership + deterministic drop (RAII) as the default, plus arenas as an optimization, plus explicit `unsafe` for low-level.**
This is the closest to Rust’s core safety story (memory safety without GC via ownership rules checked by the compiler) while still being practical to implement and compatible with a C backend. ([Rust Documentation][1])

If you paste how your current X07 values are represented in C (e.g., `bytes` layout, `vec_u8` layout, any existing `alloc/free` builtins), I can propose a concrete **runtime ABI** for:

* `Box<T>`, `Vec<T>`, slices,
* ownership moves,
* borrow “views” with lifetime checks (debug) and compile-time rules (release),
  and show exactly where checks live (compiler vs runtime) so you get safety without killing performance.

[1]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html?utm_source=chatgpt.com "Understanding Ownership"
[2]: https://en.wikipedia.org/wiki/Region-based_memory_management?utm_source=chatgpt.com "Region-based memory management"
[3]: https://clang.llvm.org/docs/AddressSanitizer.html?utm_source=chatgpt.com "AddressSanitizer — Clang 22.0.0git documentation - LLVM"
[4]: https://fuchsia.dev/fuchsia-src/contribute/open_projects/graduated/ubsan?utm_source=chatgpt.com "Undefined behavior issues (UBSan) - Fuchsia"
++++
Below is a **concrete, implementable ABI spec** for X07-on-C that gives you:

* **Box<T>, Vec<T>, slices** with stable, C-friendly layouts
* **Move-only ownership** (Rust-like) that avoids classic C double‑free/use‑after‑move footguns
* **Borrow “views”** (`&T`, `&mut T`, `&[T]`, `&mut [T]`) that are:

  * **zero-cost in release** (compile-time borrow checker)
  * **runtime-checked in debug** (deterministic lifetime + aliasing checks)
* A clear split of **what’s enforced by the compiler vs runtime**.

I’m intentionally mirroring *proven* shapes from Rust:

* `Vec` is fundamentally a **(pointer, capacity, length)** triple. ([Rust Documentation][1])
* Slices are a **fat pointer: (pointer, length)**. ([Rust Book][2])
* Borrowing rule (“one mutable or many immutable; references must be valid”). ([Rust Documentation][3])
* Debug-mode dynamic checking inspiration: Rust’s Miri/Stacked Borrows checks unsafe aliasing at runtime. ([Programming Languages & Verification][4])

Note: in this repo’s C backend, `bytes.slice` allocates and copies (counts toward `mem_stats.memcpy_bytes`). For zero-copy slicing, use `bytes_view` (`bytes.subview` / `view.slice`) and `view.as_bytes` when a `bytes` value is required.

---

# 0) ABI goals and constraints

**Goals**

1. **Release builds**: safety comes from **static ownership + borrow checking** (no runtime borrow overhead).
2. **Debug builds**: add **deterministic dynamic checks** for:

   * use-after-scope of borrows (lifetime)
   * illegal aliasing (mutable + alias)
   * free while borrowed
   * (optional) use-after-move / double-drop detection

**Constraints**

* C backend is the single backend: ABI must be expressible as C structs + C functions.
* “Fully featured like C/Rust”: must support zero-copy slices, heap ownership, deterministic destructors.

---

# 1) Fundamental ABI types

`x07_rt_abi.h` (shipped by runtime; included by all generated C code)

```c
#pragma once
#include <stdint.h>
#include <stddef.h>
#include <stdalign.h>

typedef uintptr_t x07_usize;
typedef intptr_t  x07_isize;
typedef uint8_t   x07_bool;

#define X07_TRUE  ((x07_bool)1)
#define X07_FALSE ((x07_bool)0)

_Static_assert(sizeof(x07_usize) == sizeof(void*), "x07_usize must match pointer size");
```

**Why `x07_usize`:** all container lengths/capacities and slice lengths should be pointer-sized (like Rust) to scale to large memory.

---

# 2) Box<T> ABI

## 2.1 Layout (Release)

```c
#define X07_DEFINE_BOX(T, Name) \
  typedef struct { T* ptr; } Name;
```

Example instantiation the compiler can emit:

```c
X07_DEFINE_BOX(uint8_t, x07_box_u8)
X07_DEFINE_BOX(MyStruct, x07_box_MyStruct)
```

### Box<T> invariants (Release)

* `ptr == NULL` means “moved-out / empty box” (never a valid owned allocation).
* Otherwise `ptr` points to a heap allocation holding exactly one `T` with alignment `alignof(T)`.

This “nullable owner” makes **drop idempotent**, which is very useful in C codegen.

## 2.2 Construction / Drop lowering

The compiler lowers:

* `box_new<T>(value)` into `alloc(sizeof(T), alignof(T))`, store, then `box.ptr = p`.
* `drop(box)` into:

  1. `drop_glue_T(box.ptr)` if `T` needs drop
  2. `x07_rt_free(box.ptr, sizeof(T), alignof(T))`
  3. set `box.ptr = NULL`

No runtime “Box API” is required; only allocator + panic is required.

---

# 3) Vec<T> ABI

## 3.1 Layout (Release)

```c
#define X07_DEFINE_VEC(T, Name) \
  typedef struct { T* ptr; x07_usize len; x07_usize cap; } Name;
```

Example:

```c
X07_DEFINE_VEC(uint8_t, x07_vec_u8)
X07_DEFINE_VEC(MyStruct, x07_vec_MyStruct)
```

### Vec<T> invariants (Release)

* `len <= cap`
* If `cap == 0`, then `ptr` may be `NULL` and `len` must be `0`.
* If `cap > 0`, `ptr` points to a heap allocation for `cap` elements of `T` (contiguous).

This matches the canonical “ptr/len/cap” model you want (same essential triple as Rust). ([Rust Documentation][1])

## 3.2 Element drop glue

If `T` has a destructor, `drop(vec)` must:

* for `i in 0..len`: call `drop_glue_T(&ptr[i])`
* free the buffer

The runtime does not need to know `T` if the compiler emits this loop.

---

# 4) Slice<T> ABI (shared + mutable views)

This is where you unlock Phase D→E→F performance and real ergonomics. In this repo’s C backend, zero-copy slices are expressed with `bytes_view` and the `view.*` builtins; `bytes.slice` allocates and copies.

## 4.1 Layout (Release)

Shared slice (`&[T]`):

```c
#define X07_DEFINE_SLICE(T, Name) \
  typedef struct { const T* ptr; x07_usize len; } Name;
```

Mutable slice (`&mut [T]`):

```c
#define X07_DEFINE_SLICE_MUT(T, Name) \
  typedef struct { T* ptr; x07_usize len; } Name;
```

These are “fat pointers” (pointer + length), same idea as Rust slices. ([Rust Book][2])

### Slice invariants (Release)

* If `len == 0`, `ptr` is a non-`NULL` sentinel (e.g., arena base).
* If `len > 0`, `ptr` must be non-null and point to at least `len` contiguous elements.
* Slices do **not** own memory. Lifetimes are enforced by the compiler (release) or runtime (debug).

---

# 5) Ownership moves ABI

In X07 you want **move-by-default** for owning types (`Box<T>`, `Vec<T>`, custom structs that contain them).

## 5.1 Move lowering (Release)

For move-only types, the compiler emits a “move helper” that:

* bit-copies the value
* clears the source into a safe “empty state” (so accidental drop is harmless)

### Example: Vec move helper

```c
static inline x07_vec_u8 x07_move_vec_u8(x07_vec_u8* src) {
  x07_vec_u8 tmp = *src;
  src->ptr = NULL;
  src->len = 0;
  src->cap = 0;
  return tmp;
}
```

### Example: Box move helper

```c
static inline x07_box_u8 x07_move_box_u8(x07_box_u8* src) {
  x07_box_u8 tmp = *src;
  src->ptr = NULL;
  return tmp;
}
```

**Compiler rule (release):** after a move, the source variable is “moved-out” and any use is a compile error.

**Why this avoids C hazards:** even if a codegen bug accidentally drops a moved-out value, it won’t double free.

---

# 6) Borrow views ABI and safety model

You want Rust-like borrowing:

* Any number of **immutable** borrows OR exactly one **mutable** borrow
* Borrows must never outlive the referent ([Rust Documentation][3])

We implement this as:

* **Release**: enforced statically (compiler borrow checker)
* **Debug**: enforced dynamically (runtime borrow table + checks)

---

# 7) Release-mode enforcement: compile-time only

## 7.1 What compiler must prove (Release)

### Ownership / move rules

* An owned value can be **moved** exactly once unless it’s `Copy`.
* After move: the binding is invalid and cannot be used.
* `drop` runs exactly once for each owned value at end of scope (unless moved).

### Borrow rules

* You can create `&T` (shared) many times.
* You can create `&mut T` only when no `&T` or `&mut T` are alive.
* While a value is borrowed (shared or mutable), you cannot:

  * move it
  * mutate it (unless through `&mut`)
  * reallocate its backing storage in ways that could invalidate borrows (e.g. growing a vec while there’s a slice into it)

### Lifetimes

* At minimum: **lexical lifetimes** (borrow ends at scope end).
* Later: NLL (non-lexical lifetimes) for better ergonomics.

**Release builds are zero-cost** because borrow state is not represented at runtime.

---

# 8) Debug-mode enforcement: deterministic runtime checks

This is the “safety net” to ensure:

* unsafe code and compiler bugs don’t silently corrupt memory
* tuning doesn’t exploit UB in your C output

It is conceptually similar to Rust’s Miri/Stacked Borrows style runtime checking of aliasing, but we’ll do it in a simpler deterministic way. ([Programming Languages & Verification][4])

## 8.1 Runtime metadata model

We add a debug-only allocation table and borrow table.

### Allocation table

For each heap allocation, runtime assigns an `alloc_id` and records:

* `base_ptr`
* `size_bytes`
* `alive` flag
* `shared_borrows` count
* `mut_borrow_active` flag

### Borrow table

Each borrow returns a `borrow_id` that records:

* `alloc_id`
* `kind` (shared / mut)
* `range` within allocation (`off_bytes`, `len_bytes`)
* `active` flag

## 8.2 Debug ABI: borrow-capable view types

To do lifetime checking, the view must carry `borrow_id` (and alloc/range).

### Debug slice types

```c
#ifdef X07_DEBUG_BORROW
typedef uint64_t x07_alloc_id;
typedef uint64_t x07_borrow_id;

#define X07_DEFINE_SLICE_DBG(T, Name) \
  typedef struct { \
    const T* ptr; \
    x07_usize len; \
    x07_alloc_id aid; \
    x07_borrow_id bid; \
    x07_usize off_bytes; \
  } Name;

#define X07_DEFINE_SLICE_MUT_DBG(T, Name) \
  typedef struct { \
    T* ptr; \
    x07_usize len; \
    x07_alloc_id aid; \
    x07_borrow_id bid; \
    x07_usize off_bytes; \
  } Name;
#endif
```

### Debug reference types (`&T` / `&mut T`)

```c
#ifdef X07_DEBUG_BORROW
#define X07_DEFINE_REF_DBG(T, Name) \
  typedef struct { \
    const T* ptr; \
    x07_alloc_id aid; \
    x07_borrow_id bid; \
    x07_usize off_bytes; \
  } Name;

#define X07_DEFINE_MUTREF_DBG(T, Name) \
  typedef struct { \
    T* ptr; \
    x07_alloc_id aid; \
    x07_borrow_id bid; \
    x07_usize off_bytes; \
  } Name;
#endif
```

## 8.3 Runtime functions (Debug)

```c
#ifdef X07_DEBUG_BORROW
typedef enum {
  X07_BK_SHARED = 0,
  X07_BK_MUT    = 1,
} x07_borrow_kind;

typedef enum {
  X07_ACC_READ  = 0,
  X07_ACC_WRITE = 1,
} x07_access_kind;

x07_alloc_id  x07_dbg_alloc_register(void* base_ptr, x07_usize size_bytes);
void          x07_dbg_alloc_unregister(x07_alloc_id aid);

x07_borrow_id x07_dbg_borrow_acquire(
  x07_alloc_id aid,
  x07_borrow_kind kind,
  x07_usize off_bytes,
  x07_usize len_bytes
);

void x07_dbg_borrow_release(x07_borrow_id bid);

// Called at deref/index-time to validate lifetime + alias + bounds-of-borrow-range.
void x07_dbg_borrow_check(
  x07_borrow_id bid,
  x07_access_kind acc,
  x07_usize off_bytes,
  x07_usize len_bytes
);
#endif
```

### Determinism note

These checks must fail in a deterministic way (fixed error codes/messages). No nondeterministic timing, no OS randomness.

## 8.4 Where the compiler injects checks (Debug)

When the compiler creates a borrow:

* `&x` or `slice_of_vec(v)`
  → emit `x07_dbg_borrow_acquire(...)` and store `bid` into the view struct

At end of borrow lifetime:

* emit `x07_dbg_borrow_release(bid)`

At use sites (deref/index/copyout):

* for read: emit `x07_dbg_borrow_check(bid, READ, ...)`
* for write through `&mut`: emit `x07_dbg_borrow_check(bid, WRITE, ...)`

This is exactly how you get **lifetime checks in debug** with **zero overhead in release**.

---

# 9) Exactly where checks live (compiler vs runtime)

## 9.1 Summary table

| Property                        | Release build                               | Debug build                                                |
| ------------------------------- | ------------------------------------------- | ---------------------------------------------------------- |
| Use-after-move                  | **Compiler error** (move checker)           | Compiler error + optional runtime “moved sentinel” asserts |
| Double free                     | Prevented by move+drop analysis             | Runtime detects invalid free / double free                 |
| Free while borrowed             | Compiler error                              | Runtime rejects freeing with active borrows                |
| Mutable aliasing (`&mut` + `&`) | **Compiler error** (borrow checker)         | Runtime rejects acquiring conflicting borrows              |
| Borrow outlives owner           | **Compiler error** (lifetime analysis)      | Runtime rejects deref after borrow release                 |
| Slice bounds (`idx < len`)      | Runtime check for safe indexing (like Rust) | Same + optional borrow-range check                         |
| Raw pointers / unsafe           | Allowed only in `unsafe` blocks             | Strong runtime checking when possible                      |

The borrow rule itself (“one mutable or many immutable, refs must be valid”) is the core invariant you’re enforcing. ([Rust Documentation][3])

---

# 10) How this maps onto your existing `bytes` world

In this repo today, zero-copy parsing/slicing is done via `bytes_view` and the `view.*` builtins (Phase G1). `bytes.slice` allocates and copies.
That unlocks:

* view-style parsing (track offsets / sub-slices without copying)
* “copy only what you return” patterns (Phase F-Mem2 style)
* cheap adapters over I/O results (fs/rr/kv bytes blobs)

For efficient owned output construction, use `vec_u8` builders (`vec_u8.reserve_exact`, `vec_u8.extend_bytes`, `vec_u8.as_bytes`).
If you need an owned copy of a slice, use `std.bytes.copy` or a `vec_u8` builder.

Phase G1 already includes `bytes_view`; use it to keep parsing zero-copy and copy only what you return.

---

# 11) Minimal “required runtime” for this ABI

Even for a systems language, you can keep runtime small. In release, you essentially need:

```c
void* x07_rt_alloc(x07_usize size, x07_usize align);
void* x07_rt_realloc(void* p, x07_usize old_size, x07_usize new_size, x07_usize align);
void  x07_rt_free(void* p, x07_usize size, x07_usize align);
_Noreturn void x07_rt_panic(const char* msg);
```

In debug, add the borrow/alloc tracking API in §8.

---

# 12) What you get at the end

With this ABI, x07c can generate C that:

* has **Rust-like ownership/borrowing semantics**
* produces **C-like performance** in release builds
* has **deterministic dynamic safety checks** in debug builds (critical for tuning and for catching UB / compiler miscompiles early)

…and you can scale it to:

* `String` as `Vec<u8>` + UTF-8 invariants
* `Map/Set` built on `Vec` + hashing
* arenas, bump allocators, etc — still compatible with `Box/Vec/slice` ABI.

---

If you want, I can also provide (in the same ABI style) the next layer you’ll need for “C/Rust-level usage”:

* `Option<Box<T>>` null-pointer optimization rules
* `Result<T,E>` ABI and error propagation
* `trait object` / vtable ABI (or “interface records”)
* `Allocator` trait ABI for custom allocators (kernels/embedded)

[1]: https://doc.rust-lang.org/std/vec/struct.Vec.html?utm_source=chatgpt.com "Vec in std"
[2]: https://rust-book.cs.brown.edu/ch04-04-slices.html?utm_source=chatgpt.com "The Slice Type - The Rust Programming Language"
[3]: https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html?utm_source=chatgpt.com "References and Borrowing - The Rust Programming ..."
[4]: https://plv.mpi-sws.org/rustbelt/stacked-borrows/paper.pdf?utm_source=chatgpt.com "Stacked Borrows: An Aliasing Model for Rust"
++++++
**Benchmarks**
Great — adding **memory‑model features + memory‑aware benchmarks** is exactly how you prevent Phase F from turning into “just I/O plumbing” and instead make tuning learn *how to write real programs efficiently*.

Below is a concrete plan to extend Phase F with the memory system we discussed (ownership + Vec/Box + slices/views + optional arenas) and add benchmarks + scoring so tuning converges to the best usage patterns.

In this repo, `bytes.slice` allocates and copies, so use `bytes_view` for zero-copy reads/slices; the memory suites are meant to penalize unnecessary copying patterns (repeated concat loops, per-item copying into intermediate buffers).

---

# Phase F‑Mem: what you’re adding

## Deliverables

1. **Memory model implementation (runtime + compiler)**

   * `Box<T>` (owning heap allocation)
   * `Vec<T>` (owning growable buffer)
   * `slice` / borrowed “views” (`&[u8]`, `&mut [u8]`) that are **zero‑copy** in release builds
   * ownership moves + `drop`/RAII cleanup
   * debug-only lifetime/alias checks (optional but strongly recommended)

   This is the “Rust-like” direction: ownership enables memory safety guarantees without needing GC. ([Rust Documentation][1])

2. **Deterministic memory instrumentation**

   * track allocations/reallocations/frees and bytes (total + peak live)
   * track copy-bytes (optional but very useful to discourage hidden O(n²) concat patterns)
   * enforce “no leaks at solve() return” for benchmark suites

3. **Phase F‑Mem benchmark ladders**

   * `solve-pure` memory microbenchmarks (no I/O; pure memory patterns)
   * memory‑stress tasks in `solve-fs`, `solve-rr`, `solve-kv` that combine I/O with allocation patterns (string building, parsing, concatenation, caching)

4. **suite-runner scoring hooks**

   * integrate memory stats into score (soft penalty)
   * enforce leak-free + deterministic policies (hard gates)
   * optionally run an additional debug/sanitizer gate on finalists

## Implemented in this repo (Phase F)

- Deterministic `mem_stats` emitted by the C backend and surfaced by `x07-host-runner` as `mem_stats` on each solve result.
- `fs_read_file_calls` instrumentation for `solve-fs`.
- `vec_u8.with_capacity`, `vec_u8.reserve_exact`, `vec_u8.extend_bytes`, `vec_u8.extend_bytes_range`, `vec_u8.as_bytes` primitives (used by `stdlib/std/0.1.1/modules/std/bytes.x07.json`).
- Phase F-Mem suites + fixtures:
  - `benchmarks/solve-pure/phaseF-mem-suite.json`
  - `benchmarks/solve-fs/phaseF-mem-suite.json`
  - `benchmarks/solve-rr/phaseF-mem-suite.json`
  - `benchmarks/solve-kv/phaseF-mem-suite.json`
- Phase F-Mem2 suites + fixtures (rung2):
  - `benchmarks/solve-pure/phaseF-mem2-suite.json`
  - `benchmarks/solve-fs/phaseF-mem2-suite.json`
  - `benchmarks/solve-rr/phaseF-mem2-suite.json`
  - `benchmarks/solve-kv/phaseF-mem2-suite.json`

---

# 1) Runtime memory instrumentation you need (deterministic)

You want to measure “how well programs use memory” without relying on OS RSS or allocator internals (which vary). So you must instrument **your own allocation API**, not system malloc directly.

## 1.1 Wrap all program allocations behind a single ABI

Even if you ultimately use malloc/jemalloc/mimalloc/etc., the *program* must call only:

* `x07_rt_alloc(layout)`
* `x07_rt_realloc(ptr, old_layout, new_size)`
* `x07_rt_free(ptr, layout)`

(Or your equivalent.)

Then you add a deterministic “tracker” inside those functions.

## 1.2 Memory stats struct

Per `solve()` invocation, maintain:

```c
typedef struct {
  uint64_t alloc_calls;
  uint64_t realloc_calls;
  uint64_t free_calls;

  uint64_t bytes_alloc_total;     // sum of sizes requested by alloc/realloc growth
  uint64_t bytes_freed_total;

  uint64_t live_bytes;            // current outstanding bytes (logical)
  uint64_t peak_live_bytes;

  uint64_t live_allocs;
  uint64_t peak_live_allocs;

  // Optional but very strong signal:
  uint64_t memcpy_bytes;          // bytes copied by runtime helpers (concat/copy/slice_copy)
} x07_mem_stats;
```

**Deterministic rules**

* `live_bytes` and `peak_live_bytes` are computed from requested layout sizes (not OS pages).
* `realloc` bookkeeping is deterministic: if `new_size > old_size`, increase `bytes_alloc_total` by `new_size - old_size` and update live/peak accordingly.

## 1.3 Leak policy (Phase F suites)

For Phase F benchmarks, make “no leaks at return” a **hard gate**:

* `live_allocs == 0` and `live_bytes == 0` at the end of solve()

This prevents cheating where the process exit “cleans up”. It also forces correct RAII/drop behavior.

---

# 2) Memory features to expose so tuning has real choices

To let tuning discover good patterns, you need **multiple ways** to do the same thing, with different memory profiles:

### 2.1 Keep current “copying” bytes helpers (baseline)

There are many easy-to-generate but copy-heavy patterns (byte-at-a-time loops, repeated concatenation, unnecessary intermediate buffers).
Keep the surface expressive enough that these naive solutions still exist (and benchmarks can punish them).

### 2.2 Add slice/view semantics (the “new fast path”)

In this repo, `bytes_view` is the explicit borrowed view type (Phase G1). `bytes.slice` allocates and copies.

* `bytes_view` = (ptr,len) view into bytes/vec storage (with debug-borrow tracking)
* `bytes.view(b) -> bytes_view`
* `bytes.subview(b, start, len) -> bytes_view` (zero-copy)
* `view.get_u8`, `view.len`, `view.slice`, `view.as_bytes`

Then:

* parsing tasks can be solved with **zero allocations** except final output buffer

### 2.3 Add Vec reserve/with_capacity (avoid realloc churn)

Expose:

* `vec_u8.with_capacity(n)`
* `vec_u8.reserve_exact(v, n)` (or reserve)
* `vec_u8.extend_bytes(v, b)`
* `vec_u8.extend_bytes_range(v, b, start, len)`
* `vec_u8.as_bytes(v)`

This creates a very clear “better solution” path that tuning can converge to: pre-size once, write once.

### 2.4 Optional arenas/regions (Phase F‑Mem “advanced”)

If you want tuning to explore arena patterns, implement an allocator that frees all at once (a region/arena). Region-based memory management is a known technique: objects are allocated into a region and reclaimed by destroying the region, often very cheaply. ([Stanford Theory][2])

Expose it as:

* `arena.with(|a| { ... })` (lexical scope)
* `vec_u8_in.with_allocator(a, cap)` (optional)
* Or a simpler “scratch allocator” used only by certain stdlib routines.

This can reduce allocation/free overhead for short-lived temporaries.

---

# 3) Benchmarks: Phase F‑Mem suite design

Your goal is to add benchmarks where:

* naive solutions allocate/copy a lot
* good solutions do **one allocation** (or reuse) + use views
* suite-runner can score them purely from deterministic `mem_stats`

I recommend adding **one new “mem-focused” suite per world**, without replacing the canonical Phase F suites:

* `benchmarks/solve-pure/phaseF-mem-suite.json`
* `benchmarks/solve-fs/phaseF-mem-suite.json`
* `benchmarks/solve-rr/phaseF-mem-suite.json`
* `benchmarks/solve-kv/phaseF-mem-suite.json`

That way you can keep Phase F functional coverage stable and evolve memory behavior separately.

---

## 3A) `solve-pure` PhaseF‑Mem tasks (no I/O)

These are designed to force memory patterns even without capabilities.

### Task 1: `mem_join_segments`

**Input format:** bytes with `0x00` separators
Example: `a\0bb\0ccc\0`
**Output:** join segments with `|` delimiter (ASCII `0x7C`) (no trailing delimiter)

**Why it’s memory-relevant**

* naive LLM code: repeated `bytes.concat` in a loop ⇒ O(n²) copying + many allocations
* efficient: first compute output size, allocate once, fill, or build with `Vec` + reserve

**Assertions**

* Must be leak-free at return (hard gate)
* Score on alloc_calls, realloc_calls, memcpy_bytes, peak_live_bytes

### Task 2: `mem_parse_tlv_sum`

**Input format:** TLV stream:

* `[u8 len][len bytes payload] ...`
  **Output:** `u32le` sum of the *last byte* of each payload (0 if len=0)

**Why it’s memory-relevant**

* naive: copy each payload into an intermediate buffer before reading it
* efficient: index directly into `input` (or use `bytes_view` slices) ⇒ near-zero allocations

### Task 3: `mem_filter_bytes`

**Input:** bytes
**Output:** only bytes `>= 128` (keep order)

**Why it’s memory-relevant**

* output length unknown ⇒ encourages `Vec.push`
* efficient: `Vec.with_capacity(len(input))`, `push`, `as_bytes`
* score punishes realloc churn

### Task 4: `mem_early_exit_cleanup`

**Input:** `[u32le n][n u32le values]`
**Output:** empty if any value is `0`, else output `u32le sum(values)`

**Why it’s memory-relevant**

* forces early-return path
* if someone allocates a scratch buffer and returns early without drop glue, leak gate catches it

---

## 3B) `solve-fs` PhaseF‑Mem tasks (fixture-backed FS + allocation patterns)

Add a couple of fixture files specifically to trigger expensive concatenations/parsing.

### Fixture additions (minimal)

Under your deterministic FS fixture tree, add:

* `data/seg/001.bin`, `002.bin`, …, `020.bin`

  * small binary blobs of varying sizes (e.g., 8–512 bytes)
* `cfg/seg_list.txt`

  * newline-separated file names to concatenate (sorted)

### Task: `fs_concat_segments_from_list`

**Input:** path to list file (`cfg/seg_list.txt`)
**Behavior:**

* read list, for each file in list: `fs.read_file("data/seg/<name>")`
* output concatenation of all segment bytes

**Why it’s memory-relevant**

* naive: `out = bytes.concat(out, seg)` loop ⇒ O(n²)
* good: precompute total len, allocate once, copy sequentially, OR `Vec` with reserve

You already instrument FS bytes read/call counts; now memory stats adds “how expensive was output building”.

---

## 3C) `solve-rr` PhaseF‑Mem tasks (fixture-backed request/response)

Add responses with:

* many small fields
* repeated records

### Task: `rr_fetch_many_and_build_table`

**Input:** newline-separated paths (e.g. `/user/1\n/user/2\n...`)
**Behavior:**

* request each path
* parse response body records
* output a packed table format (e.g., `[u32 count][count times: u8 len][name bytes]`)

**Why it’s memory-relevant**

* encourages slice parsing, avoiding per-record allocations
* encourages pre-allocation/reserve when building output

(Keep request count gates: exactly one request per line; score punishes extra requests.)

---

## 3D) `solve-kv` PhaseF‑Mem tasks (seeded store)

Seed store includes a few values that are non-trivial size (e.g., 4KB blobs), plus many small ones.

### Task: `kv_get_many_and_join`

**Input:** newline-separated keys
**Behavior:**

* for each key: `kv.get(key)` (empty if missing)
* output concatenation of found values with `0x00` separators

**Why it’s memory-relevant**

* classic “builder” benchmark
* naive repeated concat vs reserved vec

---

# 4) Scoring: add memory cost so tuning prefers good memory usage

You already have IO stats hooks for Phase F worlds. Extend the per-case artifact with:

```json
"mem_stats": {
  "alloc_calls": 12,
  "realloc_calls": 1,
  "free_calls": 12,
  "bytes_alloc_total": 8192,
  "peak_live_bytes": 4096,
  "memcpy_bytes": 16384,
  "live_bytes_at_end": 0,
  "live_allocs_at_end": 0
}
```

## 4.1 Hard gates (fail the case / candidate)

* `live_bytes_at_end == 0` and `live_allocs_at_end == 0`
* if debug mode borrow checks are enabled, no borrow/alias traps

## 4.2 Soft penalty (optimize)

Define deterministic memory cost:

```text
mem_cost =
  200 * alloc_calls +
  400 * realloc_calls +
  1   * bytes_alloc_total +
  2   * peak_live_bytes +
  1   * memcpy_bytes
```

Then incorporate into your existing `runtime_cost` or as a separate term in combined_score:

* prefer `median(mem_cost)` across cases
* optionally also track `p95(mem_cost)` to avoid “one giant blow-up case”

This makes tuning converge toward:

* fewer allocations
* fewer reallocations (reserve)
* lower peak live bytes (drop scopes / streaming)
* fewer copies (views instead of slice_copy/concat loops)

---

# 5) Safety gate: optional sanitizer run on finalists

Since you’re generating C, you can optionally add a **sanitizer gate** in CI (not necessarily every evaluation, but at least for the “best-of-generation” candidates):

* build runner/runtime + candidate with AddressSanitizer (see `./scripts/ci/check_asan_c_backend.sh`)
  ASan is designed to detect memory errors like use-after-free and various buffer overruns via instrumentation. ([Clang][3])

This is not purely about performance — it’s your “trust anchor” that the evolved patterns aren’t exploiting UB.

---

# 6) Why this will actually help tuning (and not just add noise)

Benchmarks have to create *stable gradients*. The suite above does that:

* If a program does unnecessary copying (byte-by-byte copy loops, repeated concatenation), it will produce **measurably larger** `alloc_calls` and `memcpy_bytes`.
* If it uses `Vec.reserve` + one pass, realloc drops to ~0 and memcpy shrinks.
* If it uses slice views, allocations in parsing loops drop to ~0.
* If it has early returns, leak gate forces correct drop glue.

That’s exactly what you need for legacy-outer-loop-driver to “feel” the improvement.

---
[1]: https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html?utm_source=chatgpt.com "Understanding Ownership"
[2]: https://theory.stanford.edu/~aiken/publications/papers/pldi98a.pdf?utm_source=chatgpt.com "Memory Management with Explicit Regions"
[3]: https://clang.llvm.org/docs/AddressSanitizer.html?utm_source=chatgpt.com "AddressSanitizer — Clang 22.0.0git documentation - LLVM"

++++++++++

Here are the generated Phase F “memory” benchmark suites **for each world** plus a **minimal deterministic fixture delta** and an **suite-runner.py patch outline** for `mem_stats` + leak gates + `mem_cost` scoring.

## Suite files in this repo

Phase F-Mem (“rung 1”) suites:

* `benchmarks/solve-pure/phaseF-mem-suite.json`
* `benchmarks/solve-fs/phaseF-mem-suite.json`
* `benchmarks/solve-rr/phaseF-mem-suite.json`
* `benchmarks/solve-kv/phaseF-mem-suite.json`

Fixtures:

* `solve-fs`: `benchmarks/fixtures/fs/solve-fs/phaseF-mem-suite-fs@0.1.0/root/`
* `solve-rr`: `benchmarks/fixtures/rr/solve-rr/phaseF-mem-suite-rr@0.1.0/`
* `solve-kv`: `benchmarks/fixtures/kv/solve-kv/phaseF-mem-suite-kv@0.1.0/seed.evkv`

Rung2 notes: `benchmarks/solve-*/phaseF-mem2-suite.json`.

---

## What’s inside each `phaseF-mem-suite.json`

### 1) `solve-pure` — memory patterns with no I/O

Tasks (all bytes-in/bytes-out):

* `phaseFmem/pure_join_segments_pipe`
  Split by `0x00`, ignore empty segments, join with `|`.
  **Why it’s a memory benchmark:** repeated concat is quadratic; best is precompute length + one allocation (or `Vec` reserve + linear extend).

* `phaseFmem/pure_tlv_sum_lastbyte_u32le`
  Parse TLV `[len:u8][payload]…`, output `u32le` sum of *last byte* of each payload.
  **Why it’s a memory benchmark:** incentivizes “view”/slice reads (`bytes_view`) instead of per-record copying into temporary buffers.

* `phaseFmem/pure_filter_ge_128`
  Filter bytes ≥ 128.

* `phaseFmem/pure_sum_u32_or_empty_if_any_zero`
  Early-exit on zero → stresses cleanup/Drop correctness (no leaks on early return).

Each task includes `assertions.mem_stats_required` and `assertions.leak_free_required` fields (the suite-runner enforces these and blends mem-cost into `combined_score`).

---

### 2) `solve-fs` — deterministic fixture FS concatenation (copy-heavy on purpose)

Two tasks, both driven by a **list file** in a mounted fixture FS:

* `phaseFmem/fs_concat_segments_no_sep`
  Read list file → read each referenced binary file → concatenate.

* `phaseFmem/fs_concat_segments_sep00`
  Same, but insert `0x00` between segment contents.

These avoid directory enumeration because **directory iteration order is not guaranteed** by POSIX and can vary by filesystem/implementation, which breaks deterministic benchmarking.

---

### 3) `solve-rr` — deterministic request/response fixture repeat-concat

One task:

* `phaseFmem/rr_fetch_blob_repeat`
  Input `[which:u8][repeat:u16le]` selects `/blob/A` or `/blob/B` → `rr.send_request` returns a small ResponseV1 → output response body repeated `repeat` times.
  **Why it’s a memory benchmark:** best solution = *one request* + *one allocation* + linear fill; worst = repeated concat loops.

Fixture keying uses **SHA‑256(request_bytes)**; SHA‑256 is standardized in NIST FIPS 180‑4.

---

### 4) `solve-kv` — seeded deterministic KV + repeat/join

Two tasks:

* `phaseFmem/kv_get_blob_repeat`
  Get `blobA`/`blobB` once, repeat output.

* `phaseFmem/kv_join_keys_sep00_skip_missing`
  Newline-separated keys → join found values with `0x00` separators.

KV store is seeded from a single binary `seed.evkv` file (included).

---

## Minimal fixture tree delta

This is the *smallest* stable fixture content I could choose that still produces meaningful allocation/copy pressure.

### A) FS fixtures to add

Root path used by the fs suite:

```
benchmarks/fixtures/fs/solve-fs/phaseF-mem-suite-fs@0.1.0/root/
```

Add these files:

**List files**

* `cfg/seg_list_small.txt` (UTF‑8 text; exact bytes):

  ```
  data/seg/s01.bin\n
  data/seg/s03.bin\n
  data/seg/s05.bin\n
  ```
* `cfg/seg_list_large.txt` (UTF‑8 text; exact bytes):

  ```
  data/seg/s01.bin\n
  data/seg/s02.bin\n
  ...
  data/seg/s10.bin\n
  ```

**Binary segments**

* `data/seg/s01.bin` = byte `0x41` (“A”) repeated 8 times
* `data/seg/s02.bin` = byte `0x42` (“B”) repeated 16 times
* `data/seg/s03.bin` = byte `0x43` (“C”) repeated 32 times
* `data/seg/s04.bin` = bytes `0x00,0x01,...,0x3F` (64 bytes, increasing)
* `data/seg/s05.bin` = byte `0x44` (“D”) repeated 64 times
* `data/seg/s06.bin` = byte `0x45` (“E”) repeated 128 times
* `data/seg/s07.bin` = byte `0x46` (“F”) repeated 256 times
* `data/seg/s08.bin` = byte `0x47` (“G”) repeated 512 times
* `data/seg/s09.bin` = byte `0x48` (“H”) repeated 1024 times
* `data/seg/s10.bin` = byte `0x49` (“I”) repeated 2048 times

(Those repetition definitions *are* exact bytes; they’re also all included in the tarball.)

---

### B) RR fixtures to add

Root path used by the rr suite:

```
benchmarks/fixtures/rr/solve-rr/phaseF-mem-suite-rr@0.1.0/
```

**Request encoding** (in suite + index):

* `RequestV1 = [u8 ver=1][u8 method][u8 path_len][path bytes][u8 body_len][body bytes]`

**Response encoding**:

* `ResponseV1 = [u8 ver=1][u8 status][u8 body_len][body bytes]`

Add:

* `index.json` (included)
* Two response blobs:

  * `responses/996022c8...d5652.bin` → `/blob/A` → body = 200 bytes `'A'`
  * `responses/9d5f4422...7aba.bin` → `/blob/B` → body = 240 bytes `'B'`

These filenames are `sha256(request_bytes).hex()`. (Standard SHA‑256. )

---

### C) KV fixtures to add

Root path used by the kv suite:

```
benchmarks/fixtures/kv/solve-kv/phaseF-mem-suite-kv@0.1.0/
```

Add:

* `seed.evkv` (binary; included)

Format used in the suite:

* Header: `[magic "X7KV"][u16le ver=1][u32le record_count]`
* Records: `[u32le klen][k bytes][u32le vlen][v bytes]`

Included records:

* `alpha -> "one"`
* `beta  -> "two"`
* `blobA -> 200 bytes of 'A'`
* `blobB -> 240 bytes of 'B'`

---

## Python suite-runner patch outline

You asked for a patch outline that shows exactly where to:

* (a) read `mem_stats`,
* (b) enforce leak gates,
* (c) blend `mem_cost` into `combined_score`.

Below I’ll anchor to the suite-runner structure you have in the uploaded `suite-runner.py` (the loop inside `_evaluate_single_task` where it currently reads `fuel_used`). Even if your C-backend runner differs, the **integration points** are the same.

### Step 0 — runner must emit `mem_stats` (and must sample it at the right time)

**Critical detail:** leak gating must *not* flag the returned output buffer as a leak.

So your runner must:

1. call `solve(input)->output_bytes`,
2. copy/consume output into host memory,
3. call the runtime’s “free output bytes” function (or whatever your ABI requires),
4. *then* sample `mem_stats` and print JSON.

Otherwise, `live_bytes` will include the output allocation and all cases will “leak”.

Also: for C backend, you can add a **debug-only** ASan build gate to catch UAF/double-free/etc. AddressSanitizer is specifically designed to catch things like heap-use-after-free and buffer overflows.

---

### Step 1 — define the expected `mem_stats` shape (JSON)

Add to runner JSON something like:

```json
"mem_stats": {
  "alloc_calls": 12,
  "realloc_calls": 3,
  "free_calls": 15,
  "bytes_alloc_total": 8192,
  "bytes_freed_total": 8192,
  "peak_live_bytes": 3072,
  "live_allocs": 0,
  "live_bytes": 0,
  "memcpy_bytes": 6144
}
```

Minimum required for your requested scoring:

* `alloc_calls`
* `realloc_calls`
* `peak_live_bytes`
* `bytes_alloc_total`
* `memcpy_bytes`
* leak gate: `live_allocs`, `live_bytes`

---

### Step 2 — read `mem_stats` per case in `_evaluate_single_task`

In your uploaded suite-runner, this is the exact pattern to patch:

Current location (conceptually):

* function `_evaluate_single_task`
* inner loop: `for case_idx, case in enumerate(task.cases):`
* right after:

  ```py
  out, runner_json, runner_stdout = _run_eval_component(...)
  fuel_used = runner_json.get("fuel_used")
  ```

Add:

```py
mem = runner_json.get("mem_stats") or {}
alloc_calls = int(mem.get("alloc_calls") or 0)
realloc_calls = int(mem.get("realloc_calls") or 0)
peak_live_bytes = int(mem.get("peak_live_bytes") or 0)
bytes_alloc_total = int(mem.get("bytes_alloc_total") or 0)
memcpy_bytes = int(mem.get("memcpy_bytes") or 0)

live_allocs = int(mem.get("live_allocs") or 0)
live_bytes  = int(mem.get("live_bytes")  or 0)

case_mem_cost = (
    alloc_calls * MEM_W_ALLOC
  + realloc_calls * MEM_W_REALLOC
  + peak_live_bytes * MEM_W_PEAK
  + bytes_alloc_total * MEM_W_ALLOC_BYTES
  + memcpy_bytes * MEM_W_MEMCPY
)
case_mem_costs.append(case_mem_cost)

# Optional: store raw stats per case (for artifacts)
case_mem_stats.append({
  "alloc_calls": alloc_calls,
  "realloc_calls": realloc_calls,
  "peak_live_bytes": peak_live_bytes,
  "bytes_alloc_total": bytes_alloc_total,
  "memcpy_bytes": memcpy_bytes,
  "live_allocs": live_allocs,
  "live_bytes": live_bytes,
})
```

Where weights are defined once near the top (or read from env), e.g.:

```py
MEM_W_ALLOC = int(os.environ.get("X07_MEM_W_ALLOC") or 200)
MEM_W_REALLOC = int(os.environ.get("X07_MEM_W_REALLOC") or 400)
MEM_W_PEAK = int(os.environ.get("X07_MEM_W_PEAK") or 2)
MEM_W_ALLOC_BYTES = int(os.environ.get("X07_MEM_W_ALLOC_BYTES") or 1)
MEM_W_MEMCPY = int(os.environ.get("X07_MEM_W_MEMCPY") or 1)
```

---

### Step 3 — enforce the leak gate

Still inside the same per-case loop, after reading `live_allocs/live_bytes`:

```py
leak_free_required = True  # or read from suite/task assertions
if leak_free_required and (live_allocs != 0 or live_bytes != 0):
    case_vm_error = True
    case_failures.append({
        "case_idx": case_idx,
        "reason": "leak_gate_failed",
        "live_allocs": live_allocs,
        "live_bytes": live_bytes,
        "mem_stats": mem,
        "runner": runner_json,
        "runner_stdout": runner_stdout[:2000],
    })
    continue
```

This treats leaks the same way you already treat VM traps/runner failure.

---

### Step 4 — persist `mem_cost` into attempt cache + summary

In the same function, you already compute:

```py
avg_fuel = ...
attempt_summary = {
  ...
  "avg_fuel": avg_fuel,
}
```

Add:

```py
avg_mem_cost = (sum(case_mem_costs) / len(case_mem_costs)) if case_mem_costs else None
attempt_summary["avg_mem_cost"] = avg_mem_cost
attempt_summary["peak_live_bytes_max"] = max((s["peak_live_bytes"] for s in case_mem_stats), default=0)
attempt_summary["memcpy_bytes_sum"] = sum((s["memcpy_bytes"] for s in case_mem_stats), default=0)
```

Also include these in the cached JSON and in `task_reports` so you can debug regressions.

---

### Step 5 — aggregate mem_cost in `_evaluate_suite`

You currently aggregate `solved_fuels`. Add:

* `solved_mem_costs: list[int] = []`
* append `int(result.best_mem_cost)` when solved
* compute suite median/mean.

Update `SuiteEvalResult` to include:

* `mem_cost: float`
* `solved_mem_costs: list[int]`

---

### Step 6 — blend `mem_cost` into `combined_score`

Right now your uploaded suite-runner returns:

```py
"combined_score": float(res.solve_rate),
```

That will not move tuning toward better memory usage.

A safe, monotonic approach:

* Keep `solve_rate` as a hard gate (if it’s <1, don’t “reward” efficiency).
* When solve_rate == 1, compute an efficiency score.

Example (simple + stable):

```py
if res.solve_rate < 1.0:
    combined = res.solve_rate
else:
    fuel_norm = float(os.environ.get("X07_SCORE_FUEL_NORM") or 1_000_000)
    mem_norm  = float(os.environ.get("X07_SCORE_MEM_NORM")  or 1_000_000)
    eff = 1.0 / (1.0 + (res.runtime_cost / fuel_norm) + (res.mem_cost / mem_norm))
    combined = 1.0 * eff
```

Then return metrics:

```py
"combined_score": float(combined),
"runtime_cost": float(res.runtime_cost),
"mem_cost": float(res.mem_cost),
```

This guarantees:

* all correct solvers stay near 1,
* but the best “memory+runtime efficient” solvers become strictly better than wasteful ones.

---

## Why these mem benchmarks will actually drive tuning

* They’re dominated by **concat/repeat patterns**, where:

  * naive approach → many allocations/reallocations + lots of memcpy,
  * optimal approach → precompute size + allocate once + linear fill.

* They’re consistent across worlds (`pure`, `fs`, `rr`, `kv`) so tuning can discover reusable “structural macros” or canonical library code patterns once and reuse everywhere.

* They’re deterministic (fixture-backed I/O only; no ambient FS/net/time).
  For FS, we also avoid directory ordering pitfalls because POSIX does not guarantee `readdir()` order.
