## 1) Are your current X07 stdlib data structures “efficient enough”?

**For Phase E/F/H2-style benchmarks and “LLM-first, deterministic, compact” goals: mostly yes.
For “C/Rust-level general-purpose usage”: not yet (by design).**

From the `.x07.json` modules you shared (notably `map.x07.json` / `set.x07.json`), the current `std.map` / `std.set` family is essentially a **sorted, packed byte-encoding** with **linear scans** and **whole-structure rebuild on insert** (it builds a new `bytes` result using `vec_u8.with_capacity` and copies segments). That gives you:

### What’s good about it (and why it worked well so far)

* **Deterministic by construction**: key order is lexical (bytewise) and stable.
* **Very compact representation**: no per-node heap allocations; everything lives in one `bytes` blob.
* **Great for small N**: for small maps/sets, a linear scan over contiguous memory can be fast due to cache locality (similar motivation to why B-tree nodes pack multiple keys contiguously). ([Rust Documentation][1])
* **LLM-friendly**: fewer moving parts than a real hash table.

### Where it becomes “not efficient enough”

* **Insertion/update is O(n) *and copies O(n) bytes** (effectively “immutable packed map” semantics). Building a map of size `m` can drift toward **O(m²)** total work and high `memcpy_bytes`.
* **Lookups are linear** in the number of entries unless you add binary search (you currently scan).
* **It cannot naturally store borrowed keys/views without copying** because the container itself is a `bytes` blob. Once you start rewarding “borrowed views” and punishing `memcpy_bytes`, this becomes a bottleneck.

So: **keep these implementations** as your “SmallMap/SmallSet” (or “OrderedBytesMap/Set”) baseline because they’re deterministic and compact, but **don’t rely on them as the only general-purpose map/set** once you want C/Rust-level productivity and performance.

A useful rule of thumb:

* `SmallMap`/`SmallSet` (sorted packed representation) is great up to “tens” of entries.
* Past that, you need a **real HashMap** (average O(1)) and/or a **real ordered map** (O(log n) + stable order/range queries).

---

## 2) What additional data structures should be “core stdlib essentials”?

If the goal is “X07 on the same level of general usage as C/C++/Rust”, you need at least the equivalents of the **core collections set** that most real programs reach for.

Below is a **pragmatic minimum set**, with determinism notes and why each matters.

### A. Must-have (core collections baseline)

#### 1) `HashMap<K,V>` + `HashSet<K>` (deterministic flavor)

* Most programs want **fast key lookup**.
* **Determinism requirement**: you must NOT rely on randomized hashing in eval.

  * Rust’s `HashMap` is explicitly **randomly seeded** by default and uses SipHash 1-3 (subject to change), which is great for HashDoS resistance but **not deterministic across runs**. ([Rust Documentation][2])
  * Rust also documents that iterators visit items in **arbitrary order**. ([Rust Documentation][2])

**What to do in X07**:

* Provide `std.hash_map` / `std.hash_set` with:

  * a **fixed, specified hash function + fixed seed** in deterministic worlds (eval),
  * and (optionally later) an OS-world “randomized seed” wrapper for adversarial environments.
* Treat iteration order as **unspecified but deterministic** given the same program + hash + growth policy.
* When stable order is required, users should:

  * sort keys, or
  * use `BTreeMap`.

#### 2) `BTreeMap<K,V>` + `BTreeSet<K>` (ordered)

* Needed for:

  * stable sorted iteration,
  * range queries,
  * predictable worst-case behavior.
* Rust’s docs describe `BTreeMap` as an **ordered map based on a B-tree**, with iterators producing items **in key order**. ([Rust Documentation][1])

**Why it matters for X07**:

* In deterministic evaluation, an ordered map is extremely useful for **stable output**, **canonicalization**, and **reproducible reports**.

#### 3) `VecDeque<T>` (queue / ring buffer)

* BFS, parsing pipelines, schedulers, async runtimes—every one of these eventually wants a queue.
* Rust’s `VecDeque` is a **growable ring buffer** with **O(1) amortized** inserts/removes at both ends. ([Rust Documentation][3])

This is a “high ROI” container for your Phase G2/H2/H3 “real programs”.

#### 4) `BinaryHeap<T>` (priority queue)

* Needed for Dijkstra/A*, scheduling, top-K, streaming quantiles, etc.
* Rust docs: priority queue implemented with a binary heap; push/pop are **O(log n)** and peek is **O(1)**. ([Rust Documentation][4])

### B. Strongly recommended (for systems-style work)

#### 5) `BitSet` / `BitVec`

* Efficient visited sets, bloom-like filters, dense flags, DP bitsets.
* Often dramatically reduces memory and increases speed versus `HashSet<u32>` for dense ranges.

#### 6) `SmallVec<T, N>` (small-buffer optimization) — optional but high value

* Great for parsers, tokens, path segments, URL segments, small collections.
* Helps keep `alloc_calls`/`realloc_calls` down.

#### 7) `Arena` / `Bump` typed allocator wrappers

* You already have an arena-like allocator model; exposing a typed arena API makes building trees/graphs much easier and reduces allocation churn.

### C. What not to prioritize in “core”

#### Linked lists

Rust’s own `LinkedList` docs include an explicit warning: it is **almost always better to use Vec or VecDeque** because array-based containers are typically faster and more cache friendly. ([Rust Documentation][5])
So: don’t spend early core-stdlib budget here unless you have a specific workload that proves otherwise.

---

## Practical recommendation for X07 core stdlib policy

To avoid “stdlib bloat” but still reach C/Rust-class productivity:

### Keep three tiers

1. **Core ABI + primitives** (your H1/H2 work): `Box`, `Vec`, `Slice/View`, `Option`, `Result`, plus `String`/`StrView` utilities.
2. **Core collections** (the set above): `HashMap`, `HashSet`, `BTreeMap`, `BTreeSet`, `VecDeque`, `BinaryHeap`, `BitVec`.
3. Everything else as **packages** (LRU cache, rope, trie, regex engines beyond regex-lite, etc.)

### Determinism rule-of-thumb

* In deterministic evaluation worlds:

  * `HashMap` must use a **specified deterministic hasher/seed** (no ambient randomness). ([Rust Documentation][2])
  * iteration order may be “arbitrary”, but must be *stable given identical inputs/ops*.
* For stable order requirements:

  * use `BTreeMap` (guaranteed key order iteration) ([Rust Documentation][1])
  * or sort `HashMap` keys (like Rust examples recommend when comparing results from arbitrary-order iteration). ([Rust Documentation][2])

---

## Bottom line

1. **Your current map/set implementations are “efficient enough” for the current deterministic benchmark-driven phases** (small N, correctness-first, LLM-friendly, deterministic).
   They are **not** efficient enough as the *only* general-purpose map/set for a C/Rust-class language.

2. **Yes, you should implement both `HashMap` and `BTreeMap` (and their Set variants)**, plus `VecDeque` and `BinaryHeap` as core collections. These mirror the “common core” of practical systems languages. ([Rust Documentation][6])

+++++
Below is a concrete, X07‑specific plan that (a) keeps your current “sorted packed” map/set approach, but labels it correctly as a **small, deterministic, compact** collection, and (b) extends **Phase H2** with a **collections baseline** that can scale to “real programs” without breaking determinism.

## 1) X07‑specific split: `std.small_map` (sorted, packed, deterministic, compact)

### Why you want this split (determinism + semantics)

Most mainstream “hash map” defaults are *not* deterministic in the way X07 needs:

* Rust’s `HashMap` is **randomly seeded by default** (to resist HashDoS), so behavior like iteration order can vary run‑to‑run unless you control the hasher/seed.
* Rust’s `BTreeMap` is an **ordered map** that stores entries “in key order”, which is deterministic by definition.
* C++ `std::unordered_map` elements are **not sorted in any particular order**; they’re organized into buckets based on hash. ([C++ Reference][1])

Your current stdlib `std/map.x07.json` + `std/set.x07.json` implementations (packed, sorted by key, “insert by rebuild”) are exactly the deterministic, stable‑semantics structure you want for evaluation… but they’re **not** a general HashMap/BTreeMap replacement. They’re a “small map / flat map” style structure.

### What `std.small_map` should mean in X07

**Definition (language / stdlib contract):**

> `std.small_map` is a map optimized for **small cardinality** and **deterministic behavior** using a **sorted packed representation**. It is compact and stable (iteration order = key order). It is *not* optimized for large N insertions.

This mirrors the deterministic property you like in ordered maps (BTreeMap stores in key order) , but implemented as a flat packed structure (your current design).

### Recommended representation (keep yours)

Keep your current packed bytes encoding; make it official:

**`SmallMapBytesU32` (bytes → u32)**

* Representation: a single `bytes` blob containing repeated entries:

  * `[key_len:u32_le][key_bytes…][value:u32_le]…`
* Invariant: entries are **strictly sorted by key** (lexicographic byte compare).
* Operations rebuild the blob deterministically (using `vec_u8.with_capacity` + `extend_bytes`) and return a new `bytes`.

This is:

* deterministic,
* compact,
* easy to serialize,
* *very* LLM‑friendly (“map is just an opaque blob”).

### Proposed public API (v1)

Refactor your current `std.map.word_freq_sorted_ascii` into a real reusable DS API.

**Module:** `stdlib/std/<ver>/modules/std/small_map.x07.json`
**Exports (suggested):**

* `std.small_map.empty_bytes_u32 : bytes`
* `std.small_map.len_bytes_u32(map: bytes) -> i32`
* `std.small_map.get_bytes_u32(map: bytes, key: bytes) -> i32` (return 0 if missing, or return Option later)
* `std.small_map.insert_bytes_u32(map: bytes, key: bytes, val: i32) -> bytes`
* `std.small_map.inc1_bytes_u32(map: bytes, key: bytes) -> bytes`
* `std.small_map.remove_bytes_u32(map: bytes, key: bytes) -> bytes` (optional in v1)
* `std.small_map.iter_bytes_u32(map: bytes) -> bytes` (optional: returns packed `[klen][k][v]…` which is the map itself)

Then implement:

* `std.text.ascii.word_freq_sorted` on top of `std.small_map.*` (so “word freq” becomes an algorithm, not the only “map” you have).

### Semantics guarantees (must be written down)

These are the invariants that make it deterministic and evaluable:

1. **Ordering:** iteration order is **key order** (lexicographic).
2. **Uniqueness:** inserting an existing key replaces the value (or increments for `inc1`).
3. **No ambient randomness:** comparisons/hashes are pure deterministic.
4. **Complexity contract:** optimized for small N; insertion is O(n) shifting/rebuild.

### When the LLM should choose `small_map`

Give the solver (and guide docs) a crisp heuristic:

* Use `std.small_map` when:

  * expected unique keys ≤ **32** (tunable),
  * you want stable sorted output,
  * you want compact representation,
  * you’re doing config/header‑like maps.

* Don’t use it when:

  * you’re inserting hundreds/thousands of keys (then you want `std.hash_map` or `std.btree_map`).

That split alone reduces “wrong DS chosen” failures and helps tuning discover the intended patterns.

---

## 2) Extend Phase H2 with collections: A) Must‑have baseline, B) Strongly recommended systems set

You asked for a **detailed dev plan + benchmarks** so tuning can learn canonical, allocator‑aware patterns.

I’ll frame this as Phase H2‑COL (collections extension), split into PR‑sized milestones, with benchmark tasks that directly reward:

* **views / zero‑copy** (memcpy_bytes),
* **capacity planning** (realloc_calls),
* **early‑drop correctness** (leak gates already exist).

### Key design rule for all new collections in deterministic eval

For anything map/set‑like, you must define **stable iteration semantics** explicitly, because mainstream “unordered” maps don’t promise stable order (and in practice can vary) ([C++ Reference][1]).

So for X07:

* `std.small_map`: key‑sorted (by definition)
* `std.btree_map`: key‑sorted (by definition; matches ordered map semantics )
* `std.hash_map`: **insertion‑ordered** *or* **key‑sorted view** (pick one as the semantic default)

**Recommendation:** make `hash_map.iter()` insertion‑ordered, and provide `hash_map.keys_sorted()` (or `iter_sorted`) for deterministic ordered output. That keeps “hash map” useful and still allows stable deterministic output when needed.

---

# Phase H2 extension plan

## A. Must‑have (core collections baseline)

### PR H2‑COL‑01 — Introduce `std.small_map` + `std.small_set` as first‑class collections

**Goal:** stop calling these `std.map`/`std.set`; name them correctly and make them reusable.

**Deliverables**

* Add:

  * `stdlib/std/<ver>/modules/std/small_map.x07.json`
  * `stdlib/std/<ver>/modules/std/small_set.x07.json`
* Keep compatibility:

  * keep `std.map.word_freq_sorted_ascii` but reimplement it via `std.small_map.*`
  * keep `std.set.unique_lines_sorted` but via `std.small_set.*`
* Update docs:

  * `docs/stdlib/collections.md` (or similar): “small_map vs hash_map vs btree_map”
* Update `stdlib.lock` to pin new module hashes.

**Benchmarks (add to Phase H2)**
Create `benchmarks/solve-pure/phaseH2-collections-suite.json` (or merge into `phaseH2-suite.json`), add tasks:

1. `small_map_u32_smoke`

* Input: sequence of `(key_len,key_bytes,val)` pairs (<=16 entries) + query key
* Assert output: correct value
* Mem assertions:

  * `max_realloc_calls: 0` (must use `vec_u8.with_capacity`)
  * `max_memcpy_bytes: small` (constant-ish)

2. `small_set_unique_sorted_smoke`

* Input: <=32 newline‑separated tokens
* Output: sorted unique tokens joined by `\n`
* Mem assertions:

  * `max_realloc_calls: 0..1` (depending on strategy)

These tasks ensure the DS is real and stable.

---

### PR H2‑COL‑02 — Deterministic hashing primitives (`std.hash`)

You cannot build a `hash_map` without pinning hash behavior.

**Deliverables**

* `stdlib/std/<ver>/modules/std/hash.x07.json`
* Exports:

  * `std.hash.fnv1a32_view(v: bytes_view) -> i32`
  * `std.hash.fnv1a32_bytes(b: bytes) -> i32`
  * `std.hash.mix32(x: i32) -> i32` (for integer keys)
  * (optional) `std.hash.seeded_fnv1a32_view(seed: i32, v: bytes_view) -> i32`
* Rules:

  * no OS randomness in eval worlds
  * deterministic seed constant if you support seeding

**Benchmark**
3) `hash_fn_determinism`

* Input: bytes blob
* Output: 4 bytes = hash32(input) repeated twice (or hash of slice)
* Assert: exact match (this locks semantics)

---

### PR H2‑COL‑03 — `std.hash_map` + `std.hash_set` (deterministic)

This is the real “general map” for large N.

**Minimum viable semantics (v1)**

* Keys: start with **bytes_view** keys (zero‑copy) in solve‑pure; allow bytes keys too.
* Values: start with `u32` (or i32) values (counts/indices) for v1; expand later.
* Iteration:

  * `iter()` is insertion‑order OR bucket order (but bucket order ties semantics to capacity/rehash → bad)
  * so: define `iter()` = insertion order.

**Implementation strategy (works with your current primitives)**

* Use open addressing (linear/quadratic probing).
* Store arrays in packed bytes blobs:

  * `state[]` (u8: empty/filled/tombstone)
  * `hash[]` (u32)
  * `key_off[]` (u32 offset into key pool)
  * `key_len[]` (u32)
  * `val[]` (u32)
  * `ins_order[]` (u32 index list for insertion order)
* Key pool: one `vec_u8` / bytes arena storing key bytes *only if* you want owned keys.

  * For **view keys** (preferred in solve‑pure), don’t pool/copy; instead store `(base_id, start, len)` which requires view metadata support. If your view is `(ptr,len)` only, you can’t persist it safely in portable form.
  * Practical compromise for eval: views into `input` only. Then store `(start,len)` offsets into `input` rather than ptr. That’s deterministic and compact.

So define `hash_map_viewkey` for solve‑pure:

* key is `(start,len)` into `input` (or a designated “arena base bytes”)
* comparisons use `view.slice(bytes.view input start len)`; no copies.

**Benchmarks**
4) `hash_map_count_distinct_u32_64k`

* Input: 64KB of 4‑byte little‑endian keys (16K keys)
* Output: u32 = number of distinct keys
* Mem assertions:

  * `max_realloc_calls`: near 0 (requires `with_capacity`/`reserve_exact`)
  * `max_peak_live_bytes`: bounded (hash table + small slack)

5. `hash_map_word_count_view_256k`

* Input: 256KB ascii text (words separated by spaces/newlines)
* Output: u32 = number of unique words
* Critical mem assertions:

  * `max_memcpy_bytes`: **close to output size + small slack** (if they copy words, memcpy explodes)
  * `max_realloc_calls`: low

This task specifically rewards “borrowed views” and punishes accidental per‑token copying.

---

### PR H2‑COL‑04 — `std.btree_map` + `std.btree_set` (ordered collections)

Why this matters:

* You need ordered maps for deterministic iteration/range queries.
* BTreeMap semantics are “entries in key order”  which is exactly what you want.

**Deliverables**

* `stdlib/std/<ver>/modules/std/btree_map.x07.json`
* `stdlib/std/<ver>/modules/std/btree_set.x07.json`
* v1 restriction: keys are `u32` or `bytes_view` (pick one to start); values `u32`

**Benchmarks**
6) `btree_range_sum_u32`

* Input: sorted or unsorted `(key,value)` pairs + a list of `(lo,hi)` range queries
* Output: sum of values for keys in all ranges (u32)
* This rewards ordered DS (or forces sort+prefix sums; still fine, but your ordered map becomes the canonical path)

7. `btree_sorted_emit`

* Input: 4‑byte keys (N=4096)
* Output: emit keys sorted unique (or the first K)
* Asserts stable deterministic ordering.

---

## B. Strongly recommended (for systems‑style work)

These aren’t “strictly required” to claim a language exists, but they’re crucial if you want C/Rust‑like productivity without rewriting DS every time.

### PR H2‑SYS‑01 — `std.deque` (ring buffer / VecDeque)

**Why:** queues are everywhere (parsers, schedulers, BFS). Rust includes `VecDeque` for a reason.

**Deliverables**

* `stdlib/std/<ver>/modules/std/deque_u32.x07.json` (start specialized)
* Operations: `new`, `with_capacity`, `push_back`, `pop_front`, `len`

**Bench**

* `deque_sliding_window_sum`

  * Input: u32 stream + window size
  * Output: checksum of window sums
  * Mem: realloc low

### PR H2‑SYS‑02 — `std.heap` (binary heap / priority queue)

**Bench**

* `heap_topk_checksum`

  * Input: u32 list + k
  * Output: checksum of k smallest/largest

### PR H2‑SYS‑03 — `std.bitset` / `std.bitmap`

**Why:** systems code needs dense sets (permissions, flags, visited arrays).

**Bench**

* `bitset_intersection_count`

  * Input: two sets of u16
  * Output: intersection size

### PR H2‑SYS‑04 — `std.slab` (handle-based pool)

**Why:** stable IDs without pointer games; very systems‑friendly.

**Bench**

* allocate/free pattern with deterministic handle reuse rules
* assert no leaks on early exits

### PR H2‑SYS‑05 — `std.lru_cache` (pure, deterministic)

**Bench**

* repeated lookup workload, verify deterministic eviction order

---

# How to integrate this into Phase H2 so tuning learns the right patterns

## Update suites & bundles

* Add a dedicated collections suite:

  * `benchmarks/solve-pure/phaseH2-collections-suite.json`
* Add it to your “single bundle” that tuning runs (your `phaseH1H2.json` bundle):

  * include: `phaseH2-collections-suite` + existing H1/H2 suites

This makes it impossible for overlay tuning to “forget” map/set rules just because it’s currently optimizing FS tasks.

## Mem-based scoring / assertions (already in your system)

Your suite-runner already reads `mem_stats` and gates leaks. For these collections tasks, you should *use per-case assertions aggressively*:

* **Borrowed view reward:** tight `max_memcpy_bytes`
* **Capacity planning reward:** tight `max_realloc_calls`
* **Avoid per-line copying:** `max_peak_live_bytes` close to output + small slack

This aligns tuning pressure with the patterns you want.

---

# Summary recommendation (what to implement first)

If you want the best payoff-to-effort ratio:

1. **H2‑COL‑01:** refactor current `std/map.x07.json` + `std/set.x07.json` into `std.small_map` + `std.small_set` (and rebase word_freq / unique_lines onto them).
2. **H2‑COL‑02:** add `std.hash` (deterministic hash).
3. **H2‑COL‑03:** implement `std.hash_map` for large N; start with u32 keys or input-view keys.
4. **H2‑COL‑04:** add `std.btree_map` for ordered/range tasks.
5. Then add B‑tier DS (deque/heap/bitset/slab) guided by benchmarks.
+++++++

Yes — if you *expose* both `std.small_map` and `std.hash_map` as “peer” choices, the solver LLM will sometimes pick the wrong one (especially under prompt pressure), and you’ll see it oscillate across runs/phases. Humans handle “pick the right collection” with experience + profiling; an LLM will mostly follow the *most salient pattern* in its prompt/guide, which can be fragile.

That said, you can keep the performance win of a specialized small-map **without** forcing the LLM to choose.

## Recommendation: one canonical `std.map`, internal adaptive representation

Make **one public map type** (`std.map`) and treat `std.small_map` / `std.hash_map` as **internal implementations** that the LLM never imports directly.

This mirrors what “LLM-friendly” wants: *one way to do maps*, while you still optimize behind the API. (Python’s `dict` is basically “one map for everything” and still performant; it even guarantees insertion-order iteration since 3.7. ([Python documentation][1]))

### How `std.map` works internally

* Start in **small mode**: store entries in a compact sorted/packed array (“small map” representation).
* When `len > SMALL_THRESHOLD` (e.g., 8 or 16), **upgrade** once to a deterministic hash table representation (“hash map” representation).
* Keep a stable, deterministic iteration contract. Rust’s `HashMap` iterates in *arbitrary* order (and is often randomized-seeded for HashDoS resistance), which is exactly the kind of thing you *don’t* want in deterministic evaluation. ([Rust Documentation][2])
  If you need a “sorted-by-key” contract, Rust points users to `BTreeMap` (ordered by key). ([Rust Documentation][3])

### Exposed surface (what the LLM sees)

Give the LLM a minimal “one true map” API:

* `map.new() -> Map<K,V>`
* `map.with_capacity(n: u32) -> Map<K,V>`  **(the only “choice”)**
* `map.get(&map, key) -> Option<V>` (or `Result<Option<V>,E>` later)
* `map.insert(&mut map, key, val)`
* `map.remove(&mut map, key)`
* `map.len(&map) -> u32`
* `map.iter(&map) -> Iter<(K,V)>` with deterministic order contract:

  * either insertion-order (Python-like), or
  * sorted-by-key (BTreeMap-like)

If you do insertion-order, store a separate `Vec<K>` of insertion order or an index list; if you do sorted-by-key, small-mode is free (already sorted) and hash-mode can iterate by sorting keys (costly) or maintain an order structure.

### Why this is better for LLMs

* The LLM doesn’t have to decide “small vs hash vs tree”.
* The only “hint” is `with_capacity(n)`, which is **easy to teach** and also naturally aligns with your `realloc_calls` / `mem_stats` scoring pressure.
* You still get the real performance benefit. Even Go’s runtime discussions treat “small maps (<= 8 elems)” as a meaningful optimization target. ([GitHub][4])

## If you still want to expose `std.small_map` publicly, do it safely

Sometimes you *do* want a specialized API (e.g., super-compact representation, known small N, sorted iteration). If you expose it, reduce confusion with guardrails:

### 1) Make it “obviously special” in naming and docs

* Keep the name `std.small_map` (good) but brand it hard:

  * “**Only for ≤ 16 entries**”
  * “**Sorted iteration**”
  * “**O(n) insert/remove**”
* In the generated guide / solver prompt, include a single line:

  * “Use `std.map` by default. Do not use `std.small_map` unless you *know* the map stays tiny.”

Rust’s own collections docs explicitly teach when to use `HashMap` vs `BTreeMap`. ([Rust Documentation][5])
You can do the same, but keep the rule *one sentence long*.

### 2) Provide a lint (or compile-time warning) when imported

If a program imports `std.small_map`, emit a warning like:

* `W-SMALLMAP-001: std.small_map imported; prefer std.map unless max entries <= 16.`

In evaluation mode you can even make this a **soft penalty** (score) instead of a hard error, so tuning learns to avoid it.

### 3) Add an “escape hatch” constructor, not a second “default”

Instead of `small_map.new()`, make it:

* `map.new_small()` or `map.new_sorted_small()`
  So the *namespace* stays `std.map` and the LLM keeps one mental model: “I’m using map; I’m just selecting a mode.”

## The simplest LLM-first rule-of-thumb

If you adopt the **adaptive `std.map`** approach, the rule becomes:

* “Use `std.map` always.”
* “If you can estimate the number of inserts, use `map.with_capacity(n)`.”

That’s it. No confusion, no split-brain.

## Practical next step for your repo

Given your plan to add both modules:

1. Implement `std.small_map` and `std.hash_map` as **private implementation modules** (still in `stdlib/std/...` but not imported by default; don’t show them in the solver context/guide as “primary types”).
2. Implement `std.map` as the *public facade* that uses:

   * small_map representation for small sizes
   * hash_map representation after threshold
3. Add 1–2 H2 benchmarks that:

   * are guaranteed small maps (≤8 keys) and reward low alloc/memcpy
   * are guaranteed big maps (e.g., count uniques in 64KB input) and reward low rehash/realloc
4. Make the solver prompt say: “Use std.map. Use map.with_capacity when you know sizes.” (and remove any examples that import `std.small_map` directly).

+++++

Here’s a revised **`std.map` PR skeleton** that implements the **facade + internal impls** approach, so the solver LLM has **one canonical way** to use maps (`std.map.*`), while you still keep your planned **small packed map** + **hash map** implementations under the hood.

This mirrors the *Facade* pattern idea: expose a simplified interface while hiding internal subsystems.
It also keeps you deterministic by design (avoid “random-seeded HashMap iteration order” pitfalls common in mainstream libs).

---

## Design in one sentence

**Expose only `std.map` to user/LLM code; implement it by dispatching to internal `std.map._small` and `std.map._hash`, and enforce that those internal modules can’t be imported by programs.**

Why this is the right shape for LLMs:

* LLMs do better with **one canonical abstraction** than “pick a data structure.”
* For small sizes, a contiguous/sorted structure is often faster than a tree/map due to locality, so it’s perfectly reasonable to hide a “small vector map” behind the facade.
* For determinism and reproducibility, you want a stable story for ordering/iteration (HashMap-like structures often aren’t stable by default).
  (BTree/ordered maps are deterministic by definition of “ordered by key,” but you can still use hashing internally as long as your public iteration is explicitly specified.)

---

## PR skeleton: `x07:stdlib-map@0.1.1` (facade + internal impls)

### 0) Directory layout (repo-style)

```
deps/
  x07/
    x07:stdlib-map@0.1.1/
      package.json
      modules/
        std/
          map.x07.json
          map/
            _small.x07.json
            _hash.x07.json

benchmarks/
  solve-pure/
    phaseH2-map-suite.json

stdlib.lock   # add/upgrade stdlib-map entry
```

Notes:

* `deps/` matches your “module_resolution.allowed_roots = [program, deps]” policy.
* Internal impls live under `std/map/_*.x07.json` so they’re obviously not user-facing.

---

## 1) `package.json` (facade exports only)

`deps/x07/x07:stdlib-map@0.1.1/package.json`

```json
{
  "schema_version": "x07.package@0.1.0",
  "package_id": "x07:stdlib-map",
  "version": "0.1.1",
  "description": "Deterministic Map facade. Internal implementations: std.map._small (packed sorted) + std.map._hash (deterministic hash table).",

  "modules": [
    {
      "module_id": "std.map",
      "path": "modules/std/map.x07.json",
      "visibility": "public"
    },
    {
      "module_id": "std.map._small",
      "path": "modules/std/map/_small.x07.json",
      "visibility": "internal"
    },
    {
      "module_id": "std.map._hash",
      "path": "modules/std/map/_hash.x07.json",
      "visibility": "internal"
    }
  ],

  "exports": [
    "std.map"
  ],

  "dependencies": [
    "x07:stdlib-bytes@0.1.0",
    "x07:stdlib-vec@0.1.0",
    "x07:stdlib-view@0.1.0",
    "x07:stdlib-result@0.1.0"
  ],

  "enforcement": {
    "hide_internal_from_guide": true,
    "internal_import_policy": "same_package_only"
  }
}
```

### Enforced behavior you want (compiler/guide rules)

* **Guide generator**: omit any module with `visibility=internal`.
* **Compiler**:

  * If the *importer is a program module* (root = `program`), importing an `internal` module is a **hard error**.
  * If the importer is a module from **the same package**, allow the import.

That one rule prevents the “LLM guesses internal module IDs” failure mode.

---

## 2) `std.map` facade module stub

`deps/x07/x07:stdlib-map@0.1.1/modules/std/map.x07.json`

```lisp
; std.map (PUBLIC FACADE)
; The only map API LLM/user programs should use.
;
; Guarantees:
; - deterministic behavior across runs
; - stable, documented semantics
; - internal representations are not part of the API

(import std.map._small)
(import std.map._hash)

(export
  std.map.new
  std.map.len
  std.map.get_u32_or
  std.map.put_u32
  std.map.inc_u32
  std.map.items_sorted_kv_u32le)

; Representation:
; - map is an opaque bytes blob with a 1-byte tag prefix:
;   0 => small (packed sorted)
;   1 => hash (deterministic table)
; Payload format is private to each impl.

(defn std.map._tag ((m bytes)) i32
  (if (= (bytes.len m) 0)
    0
    (bytes.get_u8 m 0)))

(defn std.map._payload ((m bytes)) bytes
  (bytes.slice m 1 (- (bytes.len m) 1)))

(defn std.map._wrap ((tag i32) (payload bytes)) bytes
  (begin
    (let out (bytes.alloc (+ 1 (bytes.len payload))))
    (bytes.set_u8 out 0 tag)
    (for i 0 (bytes.len payload)
      (bytes.set_u8 out (+ 1 i) (bytes.get_u8 payload i)))
    out))

(defn std.map.new () bytes
  (std.map._wrap 0 (std.map._small.new_payload)))

(defn std.map.len ((m bytes)) i32
  (begin
    (let tag (std.map._tag m))
    (let p (std.map._payload m))
    (if (= tag 0)
      (std.map._small.len_payload p)
      (std.map._hash.len_payload p))))

(defn std.map.get_u32_or ((m bytes) (key bytes) (def i32)) i32
  (begin
    (let tag (std.map._tag m))
    (let p (std.map._payload m))
    (if (= tag 0)
      (std.map._small.get_u32_or_payload p key def)
      (std.map._hash.get_u32_or_payload p key def))))

(defn std.map.put_u32 ((m bytes) (key bytes) (v i32)) bytes
  (begin
    (let tag (std.map._tag m))
    (let p (std.map._payload m))
    (if (= tag 0)
      (begin
        (let p2 (std.map._small.put_u32_payload p key v))
        ; optional: auto-promote if small becomes “too big”
        (if (std.map._small.should_promote_payload p2)
          (std.map._wrap 1 (std.map._hash.from_small_payload p2))
          (std.map._wrap 0 p2)))
      (std.map._wrap 1 (std.map._hash.put_u32_payload p key v)))))

(defn std.map.inc_u32 ((m bytes) (key bytes)) bytes
  (std.map.put_u32 m key (+ (std.map.get_u32_or m key 0) 1)))

(defn std.map.items_sorted_kv_u32le ((m bytes)) bytes
  (begin
    (let tag (std.map._tag m))
    (let p (std.map._payload m))
    ; Public determinism rule: this ALWAYS returns keys in sorted order.
    ; Hash impl must sort before emitting, even if internal storage is unordered.
    (if (= tag 0)
      (std.map._small.items_sorted_kv_u32le_payload p)
      (std.map._hash.items_sorted_kv_u32le_payload p))))
```

Key point: the **LLM never chooses** small vs hash. The facade chooses.

---

## 3) Internal impl stub: packed sorted “small map”

`deps/x07/x07:stdlib-map@0.1.1/modules/std/map/_small.x07.json`

```lisp
; std.map._small (INTERNAL)
; Packed sorted map: deterministic, compact, great for small N.
; Not importable by user programs.

(export
  std.map._small.new_payload
  std.map._small.len_payload
  std.map._small.get_u32_or_payload
  std.map._small.put_u32_payload
  std.map._small.should_promote_payload
  std.map._small.items_sorted_kv_u32le_payload)

(defn std.map._small.new_payload () bytes
  (bytes.alloc 0))

(defn std.map._small.len_payload ((p bytes)) i32
  ; TODO: implement packed format; stub returns 0
  0)

(defn std.map._small.get_u32_or_payload ((p bytes) (key bytes) (def i32)) i32
  ; TODO
  def)

(defn std.map._small.put_u32_payload ((p bytes) (key bytes) (v i32)) bytes
  ; TODO
  p)

(defn std.map._small.should_promote_payload ((p bytes)) i32
  ; TODO: return 1 when entry_count > threshold (e.g., 32)
  0)

(defn std.map._small.items_sorted_kv_u32le_payload ((p bytes)) bytes
  ; TODO: already sorted by design
  (bytes.alloc 0))
```

---

## 4) Internal impl stub: deterministic hash map

`deps/x07/x07:stdlib-map@0.1.1/modules/std/map/_hash.x07.json`

```lisp
; std.map._hash (INTERNAL)
; Deterministic hash table for larger N.
; Must still provide deterministic output for public iterators (sort keys on emit).

(export
  std.map._hash.from_small_payload
  std.map._hash.len_payload
  std.map._hash.get_u32_or_payload
  std.map._hash.put_u32_payload
  std.map._hash.items_sorted_kv_u32le_payload)

(defn std.map._hash.from_small_payload ((small bytes)) bytes
  ; TODO: build hash table from small representation
  (bytes.alloc 0))

(defn std.map._hash.len_payload ((p bytes)) i32
  ; TODO
  0)

(defn std.map._hash.get_u32_or_payload ((p bytes) (key bytes) (def i32)) i32
  ; TODO
  def)

(defn std.map._hash.put_u32_payload ((p bytes) (key bytes) (v i32)) bytes
  ; TODO
  p)

(defn std.map._hash.items_sorted_kv_u32le_payload ((p bytes)) bytes
  ; TODO: gather keys + sort + emit
  (bytes.alloc 0))
```

Determinism note (why we’re strict): mainstream hash maps are often **random-seeded** specifically to avoid DoS, which makes ordering non-deterministic across runs.
Your facade avoids that by defining sorted emission (and deterministic hash seeds internally if needed).

---

## 5) Benchmark task JSON (validates facade + forbids internal imports)

Create a small suite file that’s cheap and directly pressures the facade behavior:

`benchmarks/solve-pure/phaseH2-map-suite.json`

```json
{
  "suite_id": "phaseH2-map-suite@0.1.0",
  "world": "solve-pure",
  "tasks": [
    {
      "task_id": "pure/map_facade_word_freq_smoke",
      "description": "Count ASCII words and emit sorted frequencies. Must use std.map (facade), not internal impl modules.",
      "assertions": {
        "require_lockfile": true,
        "min_modules": 2,
        "module_resolution": {
          "allowed_roots": ["program", "deps"],
          "forbid_ambient_fs": true
        },
        "required_imports": [
          "x07:stdlib-text@0.1.0",
          "x07:stdlib-map@0.1.1"
        ],
        "forbid_module_imports": [
          "std.map._small",
          "std.map._hash"
        ]
      },
      "cases": [
        { "name": "small", "input_b64": "YSBiIGE=", "expected_b64": "YT0yCmI9MQ==" },
        { "name": "medium", "input_b64": "YSBiIGMgZCBhIGIgYw==", "expected_b64": "YT0yCmI9MgpjPTIKZD0x" }
      ]
    },

    {
      "task_id": "pure/map_facade_promote_threshold",
      "description": "Forces more distinct keys; implementation may promote from small->hash, but output must remain sorted & correct.",
      "assertions": {
        "require_lockfile": true,
        "module_resolution": {
          "allowed_roots": ["program", "deps"],
          "forbid_ambient_fs": true
        },
        "required_imports": [
          "x07:stdlib-map@0.1.1"
        ]
      },
      "cases": [
        {
          "name": "many_keys",
          "input_b64": "YQpiCmMKZAplCmY=\n",
          "expected_b64": "",
          "notes": "Replace with a generated input/expected once std.map API is wired into a canonical helper for this test."
        }
      ]
    }
  ]
}
```

(If your suite-runner doesn’t yet support `forbid_module_imports`, keep it in the suite as a TODO and add the check when you wire up internal-module enforcement; it’s the right place to assert “LLM cannot import internal impls.”)

---

## 6) `stdlib.lock` entry shape (update `stdlib-map`)

Add/replace the stdlib-map entry so the lock pins **exact module hashes** (including internal modules), but only `std.map` is public.

```json
{
  "lockfile_version": 1,
  "packages": [
    {
      "package_id": "x07:stdlib-map",
      "version": "0.1.1",
      "content_sha256": "SHA256_OF_PACKAGE_DIR",
      "modules": [
        { "module_id": "std.map",        "path": "modules/std/map.x07.json",           "sha256": "..." , "visibility": "public" },
        { "module_id": "std.map._small", "path": "modules/std/map/_small.x07.json",    "sha256": "..." , "visibility": "internal" },
        { "module_id": "std.map._hash",  "path": "modules/std/map/_hash.x07.json",     "sha256": "..." , "visibility": "internal" }
      ],
      "deps": [
        { "package_id": "x07:stdlib-text", "version": "0.1.0" }
      ]
    }
  ]
}
```

---

## What this accomplishes (LLM-choice problem solved)

* The LLM sees **one way**: `std.map.*`.
* You still get both impls:

  * `_small`: packed sorted (fast and tiny for small sizes; good cache locality).
  * `_hash`: scalable for large sizes (still deterministic at the API boundary).
* Your determinism story stays crisp:

  * no “random seeds → different iteration order” surprises like typical `HashMap`.
  * ordered emission is explicitly defined (like ordered map semantics).

---
