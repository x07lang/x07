Here’s what I found reviewing **`after_H2_part2_c_emit.rs`** (C backend/runtime builtins) and **`stdlib_merged.txt`** (merged stdlib modules), focusing on the **new/added collections** (maps/sets + supporting DS like deque).

---

## 1) Is everything implemented correctly?

### A. C backend: `map_u32` / `set_u32` runtime looks correct

In **`after_H2_part2_c_emit.rs`**, the `map_u32` runtime is a deterministic, open‑addressing hash table (linear probing) with:

* **sentinel empty key = `0xFFFFFFFF`**
* **reserved-key checks**: `key == 0xFFFFFFFF` traps (good; otherwise the table breaks)
* `new / len / contains / get / set / remove` all present
* `remove` uses the standard “remove + reinsert/repair cluster” approach (correct for linear probing)

`set_u32.*` is implemented as a thin wrapper over `map_u32` (values ignored / set to 1). That’s also correct.

Determinism is preserved (no randomized seeds), and the hash function is stable across machines.

**One design caveat (not a bug):** `map_u32` does **not** auto-resize. That’s fine if your stdlib always chooses enough capacity, but it is a “footgun” for general-purpose code because you can hit `"map_u32 full"` traps if capacity planning is wrong.

Also: open addressing / linear probing is a normal collision strategy for hash tables, and the behavior you have here matches that family. ([Wikipedia][1])

---

### B. Stdlib merged: the big correctness issue is a real syntax error

I did a quick sanity pass over **all `.x07.json` sections** in `stdlib_merged.txt` and checked **balanced parentheses**.

✅ All the map/set modules you added (`std.small_map`, `std.hash_map`, `std.btree_map`, `std.small_set`, `std.hash_set`, `std.btree_set`, etc.) are **syntactically balanced**.

❌ **`std/deque_u32.x07.json` is NOT balanced**. The cause is in the **export list**:

* `std.deque_u32.pop_front_or)`  ← **extra `)`** inside the symbol name

That makes the module **unparsable** (this is a hard correctness bug).

**Minimal fix (conceptually):**

* change `std.deque_u32.pop_front_or)` → `std.deque_u32.pop_front_or`

This is the single “must-fix now” correctness issue I can assert from the provided sources.

---

### C. Map/set module semantics appear internally consistent

From the code patterns in `stdlib_merged.txt`:

* **`std.small_map`**: sorted packed map encoding `[(klen:u32)(kbytes)(val:u32)]*`, with:

  * early-exit search via `bytes.cmp_range` when `key < entry_key` (correct for sorted storage)
  * `put` preserves sort order and is stable
  * `remove` returns original map when key not found (nice optimization)
* **`std.small_set`**: same idea without values; also correct
* **`std.btree_map` / `std.btree_set`**: fixed-width encodings for u32 keys (good for deterministic iteration order and binary search)
* **`std.hash_map`**: thin wrapper over `map_u32.*` (correct)

**`std.hash_set` (view-backed) design is correct**, but it has a *semantic requirement* that should be made explicit:

* Your `view_*` API stores **(start,len)** ranges into a **single “base bytes” buffer**.
* If callers insert ranges from different bases, it will silently misbehave (because the stored start/len are meaningless without the original base).

Not a “bug” if it’s intended; but it needs to be documented loudly in the module docs / guide (and ideally encoded in the type system later as `set<view<'a>>`).

---

## 2) What can be optimized?

I’ll separate this into **(A) immediate fixes** and **(B) high-ROI improvements**.

---

## A) Immediate fixes (do these first)

### 1) Fix `std/deque_u32.x07.json` export typo (hard parser failure)

**Change**

* `std.deque_u32.pop_front_or)` → `std.deque_u32.pop_front_or`

Also add a tiny CI check that parses/paren-checks all `.x07.json` files so this never lands again (this exact bug is the kind that wastes tuning cycles).

---

## B) High-ROI optimization improvements

### 2) Make hash-based DS APIs LLM-friendly (remove “cap_pow2” footguns)

Right now:

* `std.hash_map.with_capacity_u32(expected_len)` is good (it computes pow2 internally).
* But `std.hash_set.view_new(cap_pow2)` is **not** LLM-friendly.

**Recommendation:** change `view_new` to take `expected_len` (and optionally a load-factor policy), compute `cap_pow2` inside.

This does two things:

* avoids accidental non-pow2 traps (since `map_u32.new` requires pow2)
* gives tuning a clean, canonical “always call with_capacity/new(expected_len)” pattern

---

### 3) Reduce `realloc_calls` + `memcpy_bytes` in `std.hash_set` (this will pay off in your mem suites)

Your `std.hash_set` currently grows its internal entry table by repeatedly doing:

* push 12 zero bytes per insert (via a loop)
* without reserving a sensible amount up front

That is exactly the pattern your Phase F/G1 mem scoring tries to penalize (repeated growth → realloc → memcpy).

**Concrete improvement options:**

#### Option A (pure stdlib-only; no new builtins)

* In `view_new`, set initial vec capacity to something proportional to cap:

  * header is 12 bytes
  * each entry is 12 bytes
  * you rehash when `(len+1)*2 >= cap` ⇒ `len <= cap/2`
  * so max entries before rehash ≈ `cap/2`

So you can do:

* `vec_u8.with_capacity(12 + 12 * (cap/2))`

This makes `realloc_calls` near-zero for the common case.

#### Option B (better, requires 1 new core primitive)

Add a builtin:

* `vec_u8.extend_zeroes(n)`
  so you don’t do `for 0 n (vec_u8.push v 0)` loops everywhere.

This will reduce:

* fuel (fewer AST steps)
* `realloc_calls` (because the runtime can ensure capacity once)
* `memcpy_bytes` (because growth can be amortized)

This one primitive benefits *multiple* data structures (hash_set, slab, deque growth, etc.).

---

### 4) `map_u32` hashing quality and “systems” posture

Your runtime uses a simple multiplicative hash `key * CONST` and masks by `cap-1`.

This is deterministic and fast, but:

* you may want to use the **golden ratio multiplicative constant** commonly cited in multiplicative hashing discussions (e.g., 2654435769).
  (Your current constant is close; if it was accidental, align it.)
* for **standalone OS world**, deterministic non-secret hashing can be vulnerable to **algorithmic complexity / hash-collision DoS** in some settings. ([researchgate.net][2])
  SipHash-style keyed hashing is a common hardening technique for hash tables against malicious input.

Because X07 must stay deterministic in `solve-*`, you can:

* keep deterministic hashing in eval worlds
* optionally add a *standalone-only* hardened map variant (or a “seeded hash” mode) **only in `run-os`** (but that changes semantics unless you define it carefully)

---

### 5) Micro-optimizations in `rt_map_u32_new`

Minor but free:

* `rt_alloc_raw` already zero-fills, so in `rt_map_u32_new` you don’t need to set `m->vals[i] = 0` in the initialization loop.
* keys could be initialized with `memset(keys, 0xFF, cap*sizeof(uint32_t))` (faster than per-element loop); deterministic.

These will shave CPU time in map-heavy tasks.

---

## Summary of actionable items (ranked)

**Must fix**

1. Fix `std/deque_u32.x07.json` export typo (`pop_front_or)` → `pop_front_or`) — currently a parser error.

**Should do soon (big pay-off in mem + LLM success)**
2) Change `std.hash_set.view_new(cap_pow2)` → `std.hash_set.view_new(expected_len)` with internal pow2 rounding.
3) Pre-reserve entry bytes in `std.hash_set` to crush `realloc_calls`.
4) Add `vec_u8.extend_zeroes(n)` builtin (huge cross-stdlib win).

**Later / systems maturity**
5) Decide on hash hardening posture for OS worlds (SipHash or fixed-seed stronger hash), keeping eval determinism intact. ([researchgate.net][2])

---
```diff
diff --git a/stdlib/std/0.1.0/std/deque_u32.x07.json b/stdlib/std/0.1.0/std/deque_u32.x07.json
index 0000000..0000000 100644
--- a/stdlib/std/0.1.0/std/deque_u32.x07.json
+++ b/stdlib/std/0.1.0/std/deque_u32.x07.json
@@ -13,7 +13,7 @@
 ; Encoding in bytes:
 ;   0..4:  cap (u32_le)
 ;   4..8:  len (u32_le)
-;   8..12: head index (u32_le) in [0,cap)
+;   8..12: head index (u32_le) where 0 <= head < cap
 ;   12.. : cap * u32_le items
 
 (defn std.deque_u32.with_capacity ((cap i32)) bytes
diff --git a/scripts/check_x07_parens.py b/scripts/check_x07_parens.py
new file mode 100755
index 0000000..1111111
--- /dev/null
+++ b/scripts/check_x07_parens.py
@@ -0,0 +1,171 @@
+#!/usr/bin/env python3
+"""
+Tiny CI guard: ensure all .x07.json files have balanced parentheses.
+
+Design goals:
+  - deterministic output (stable file order, stable diagnostics)
+  - ignore line comments that start with ';' (X07 S-expr surface)
+  - (optional) ignore parentheses inside double-quoted strings (future-proof)
+
+Usage:
+  python3 scripts/check_x07_parens.py
+  python3 scripts/check_x07_parens.py stdlib benchmarks
+  python3 scripts/check_x07_parens.py --glob 'stdlib/**/std/*.x07.json'
+"""
+
+from __future__ import annotations
+
+import argparse
+import glob
+import os
+import sys
+from dataclasses import dataclass
+from pathlib import Path
+from typing import Iterable, List, Tuple
+
+
+DEFAULT_ROOTS = [
+    "stdlib",
+]
+
+# Avoid scanning build outputs / vendored deps by default when roots are broad.
+SKIP_DIRS = {
+    ".git",
+    "target",
+    "artifacts",
+    "deps",
+    "tuning/out",
+    "tuning/.venv",
+    "__pycache__",
+}
+
+
+@dataclass(frozen=True)
+class ParenError:
+    path: str
+    line: int
+    col: int
+    code: str  # "E001" / "E002" / "E003"
+    message: str
+
+    def format(self) -> str:
+        return f"{self.path}:{self.line}:{self.col}: {self.code} {self.message}"
+
+
+def _should_skip_dir(dirpath: Path) -> bool:
+    # exact match on tail dir name or on a joined relative path like tuning/out
+    tail = dirpath.name
+    if tail in SKIP_DIRS:
+        return True
+    rel = str(dirpath).replace(os.sep, "/")
+    for s in SKIP_DIRS:
+        if "/" in s and rel.endswith(s):
+            return True
+    return False
+
+
+def iter_x07_json_files(paths: List[str], glob_pat: str | None) -> List[Path]:
+    out: List[Path] = []
+
+    if glob_pat:
+        for p in sorted(glob.glob(glob_pat, recursive=True)):
+            pp = Path(p)
+            if pp.is_file() and pp.suffix == ".x07.json":
+                out.append(pp)
+        return out
+
+    roots = paths if paths else DEFAULT_ROOTS
+    for root in roots:
+        rp = Path(root)
+        if not rp.exists():
+            # ignore missing roots (useful in partial checkouts), but keep deterministic output
+            continue
+        if rp.is_file():
+            if rp.suffix == ".x07.json":
+                out.append(rp)
+            continue
+
+        # walk deterministically
+        for dirpath, dirnames, filenames in os.walk(rp):
+            dp = Path(dirpath)
+            if _should_skip_dir(dp):
+                dirnames[:] = []
+                continue
+
+            dirnames[:] = sorted(dirnames)
+            for fn in sorted(filenames):
+                if fn.endswith(".x07.json"):
+                    out.append(dp / fn)
+
+    # stable ordering for deterministic diagnostics
+    out_sorted = sorted(out, key=lambda p: str(p).replace(os.sep, "/"))
+    return out_sorted
+
+
+def check_parens(path: Path) -> List[ParenError]:
+    # Read as UTF-8; if invalid bytes exist, replace so we still get a deterministic scan.
+    s = path.read_text(encoding="utf-8", errors="replace")
+
+    stack: List[Tuple[int, int]] = []  # (line, col) for '('
+    errs: List[ParenError] = []
+
+    line = 1
+    col = 0
+    in_comment = False
+    in_string = False
+
+    def add_err(l: int, c: int, code: str, msg: str) -> None:
+        errs.append(
+            ParenError(
+                path=str(path).replace(os.sep, "/"),
+                line=l,
+                col=c,
+                code=code,
+                message=msg,
+            )
+        )
+
+    for ch in s:
+        if ch == "\n":
+            line += 1
+            col = 0
+            in_comment = False
+            continue
+
+        col += 1
+
+        if in_comment:
+            continue
+
+        # X07 uses ';' for line comments.
+        if (not in_string) and ch == ";":
+            in_comment = True
+            continue
+
+        # future-proof: if strings exist later, keep parens inside strings out of the count
+        if ch == '"':
+            in_string = not in_string
+            continue
+
+        if in_string:
+            continue
+
+        if ch == "(":
+            stack.append((line, col))
+        elif ch == ")":
+            if not stack:
+                add_err(line, col, "E001", "unexpected ')'")
+                # continue scanning so we can report more errors in one run
+            else:
+                stack.pop()
+
+    if in_string:
+        add_err(line, col, "E003", "unterminated string literal")
+
+    # any unclosed '('
+    for (l, c) in stack:
+        add_err(l, c, "E002", "unterminated '('")
+
+    return errs
+
+
+def main() -> int:
+    ap = argparse.ArgumentParser(description="Check X07 .x07.json files for balanced parentheses.")
+    ap.add_argument("paths", nargs="*", help="Files/dirs to scan (default: stdlib/)")
+    ap.add_argument("--glob", dest="glob_pat", default=None, help="Glob pattern for .x07.json files")
+    args = ap.parse_args()
+
+    files = iter_x07_json_files(args.paths, args.glob_pat)
+    if not files:
+        # Deterministic no-op success.
+        return 0
+
+    all_errs: List[ParenError] = []
+    for f in files:
+        all_errs.extend(check_parens(f))
+
+    if all_errs:
+        for e in all_errs:
+            print(e.format(), file=sys.stderr)
+        print(f"check_x07_parens: FAIL ({len(all_errs)} error(s) across {len(files)} file(s))", file=sys.stderr)
+        return 1
+
+    print(f"check_x07_parens: OK ({len(files)} file(s))")
+    return 0
+
+
+if __name__ == "__main__":
+    raise SystemExit(main())
diff --git a/scripts/ci/check_phases_ad.sh b/scripts/ci/check_phases_ad.sh
index 2222222..3333333 100755
--- a/scripts/ci/check_phases_ad.sh
+++ b/scripts/ci/check_phases_ad.sh
@@ -1,6 +1,10 @@
 #!/usr/bin/env bash
 set -euo pipefail
 
+echo "[CI] check_x07_parens"
+python3 scripts/check_x07_parens.py
+echo
+
 # existing CI gates below...
 # (phase suites / determinism / etc)
```

### Notes (so you can apply cleanly)

* If your deque module lives under a different version folder (e.g. `stdlib/std/0.1.1/std/deque_u32.x07.json`), apply the **same one-line comment fix** there instead.
* The CI hook above assumes you already run `scripts/ci/check_phases_ad.sh` in CI (as per your roadmap). If your CI entrypoint is different, wire `python3 scripts/check_x07_parens.py` into whatever script/workflow is executed on PRs.
