Spec-ID: x07.spec.internal.x07-stdlib@0.1.0
Status: draft
Applies-to: toolchain >= v0.0.95
Related schemas: []

 C standard library review → X07 core stdlib essentials

## First: “C standard library” vs “systems programming”

The ISO C standard library (libc) includes things like memory management, string handling, math, and standard I/O—exposed via headers like `<stdlib.h>`, `<string.h>`, `<stdio.h>`, `<math.h>`, etc. ([C++ Reference][1])

But **networking and many OS features are not in ISO C**; they come from OS APIs (commonly POSIX). For example, `socket()` is specified as a system interface in POSIX/The Open Group docs. ([The Open Group][4])
Similarly, `open()` is an OS syscall interface on Unix-like systems (documented in the Linux man-pages). ([man7.org][3])

Rust makes this layering explicit:

* `std` is the full “hosted” standard library (OS interaction). ([Rust Documentation][5])
* `alloc` is the heap/collections layer used even in `no_std` contexts. ([Rust Documentation][6])

X07 should copy this structure.

## Recommendation: X07 stdlib should be 3 tiers

### Tier 1: `core` (pure, always available, deterministic)

This is the minimum “build everything else” substrate. If you want to be “C/Rust level”, these are the non-negotiables:

**A) Memory + bytes primitives (the `<string.h>` heart)**

* `memcpy`, `memmove`, `memset`, `memcmp` equivalents (for `bytes` and slices)
* bounds-checked slice operations
* stable byte comparison and equality
  These mirror the role of `<string.h>` in C, which is foundational for virtually everything else. ([C++ Reference][7])

**B) Allocation interface (the `<stdlib.h>` heart)**

* `alloc(size)`, `free(ptr)`, `realloc(ptr,size)` (or equivalent)
* optional: arena/bump allocator hooks (deterministic)
  C’s `<stdlib.h>` explicitly covers general utilities including memory management. ([C++ Reference][1])

**C) Integer/bit utilities**

* fixed-width ints (at least u32/u64 and signed variants)
* bitwise ops, shifts (well-defined), rotate, popcount, clz/ctz
* wrapping/checked/saturating arithmetic helpers (so code doesn’t reinvent pitfalls)

**D) Deterministic codecs**

* endian encode/decode: `u32_le`, `u64_le`, varints (if you need)
* safe parsing of length-prefixed structures (critical for RR/KV/FS formats)

**E) Core algorithms**

* deterministic sort (stable sort for bytes and integer arrays)
* search (linear, binary)
* simple hashing (stable non-crypto hash for maps)

**F) Errors**

* Result-like pattern (error codes + message bytes)
* assertions/panic hooks (for dev), but in deterministic suites you typically disable panics and return a failure code

> Why these belong in *core*: they are universally needed, pure, deterministic, and they enable higher-level data structures and parsers.

### Tier 2: `alloc` (pure, but heap-enabled)

This corresponds to Rust’s `alloc` crate: heap collections and smart containers. ([Rust Documentation][6])

Minimum essentials:

* `Vec<u8>`, `Vec<u32>` (and eventually generic Vec)
* `String` (UTF‑8 by convention; keep it as “bytes with UTF‑8 validation” initially)
* `HashMap` / `HashSet` (or at least a map) with deterministic behavior:

  * deterministic hashing seed (fixed) for reproducible runs
  * deterministic iteration order if you expose iteration (or don’t expose iteration yet)

These unlock “real programming” without dragging in I/O.

### Tier 3: `std` (world-scoped I/O)

This is where fixture-backed I/O worlds land.

World-scoped modules/packages:

* `std::fs` (fixture-backed)
* `std::rr` (fixture-backed request/response)
* `std::kv` (fixture-backed deterministic store)
* `std::time` (add later via fixture “clock” if needed)
* `std::net` (don’t expose real sockets in deterministic suites; `socket()` is OS-level and nondeterministic across environments) ([The Open Group][4])

Because directory traversal order is not stable, `std::fs::list_dir()` should either:

* return entries sorted, or
* clearly document it returns “filesystem order” and require callers to sort (but then your benchmark must enforce sorting). ([man7.org][2])

---

## What should NOT be in core (but can be packages)

To keep packaging/composability intact, don’t bake these into the compiler/runtime:

* JSON/XML parsers
* regex
* compression
* TLS/crypto stacks
* full HTTP parser
* database clients

These should be **versioned packages** that depend on `core`/`alloc` and on world-specific `std` modules when needed.

---

## Putting it together: “Minimum core builtins that enable a C/Rust-class ecosystem”

If you implement **only** the following in the built-in core (and everything else as packages), you can still grow into a full ecosystem:

1. allocator + memory primitives (malloc/free/realloc + memcpy/memmove/memset/memcmp) ([C++ Reference][1])
2. bytes/slice API (safe indexing, slicing, concatenation building blocks)
3. integer/bit utilities + deterministic codecs (endianness, varint)
4. deterministic sort/search/hash
5. error/result conventions

…and then fixture-backed I/O worlds add `std::fs`, `std::rr`, `std::kv` as world-scoped I/O instead of exposing POSIX APIs directly like `open()`/`socket()` in the language surface. ([man7.org][3])

---

## Concrete artifacts in this repo

Bench suites and fixtures:

- Suites: `labs/benchmarks/` (see `labs/benchmarks/README.md`)
- Default suite bundle: `labs/benchmarks/bundles/regression.json`
- Fixtures: `ci/fixtures/bench/**` (FS/RR/KV fixture worlds)
- Fixture snapshot helper script: `./scripts/build_fixture_fs_tree.sh`

[1]: https://en.cppreference.com/w/c/header.html?utm_source=chatgpt.com "C Standard Library headers"
[2]: https://man7.org/linux/man-pages/man3/readdir.3.html "readdir(3) - Linux manual page"
[3]: https://man7.org/linux/man-pages/man2/open.2.html?utm_source=chatgpt.com "open(2) - Linux manual page"
[4]: https://pubs.opengroup.org/onlinepubs/9699919799/functions/socket.html?utm_source=chatgpt.com "socket"
[5]: https://doc.rust-lang.org/std/?utm_source=chatgpt.com "std - Rust"
[6]: https://doc.rust-lang.org/alloc/?utm_source=chatgpt.com "alloc - Rust"
[7]: https://en.cppreference.com/w/c/header/string.html?utm_source=chatgpt.com "Standard library header <string.h>"
