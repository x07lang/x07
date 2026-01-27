#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import math
import random
from dataclasses import dataclass
from pathlib import Path
from typing import Callable


def _repo_root() -> Path:
    p = Path(__file__).resolve()
    for parent in p.parents:
        if (parent / "Cargo.toml").is_file() and (parent / "crates").is_dir():
            return parent
    return p.parents[3]


def _b64(b: bytes) -> str:
    return base64.b64encode(b).decode("ascii")


def _stable_rng(*, seed: int, salt: str) -> random.Random:
    digest = hashlib.sha256(f"{seed}:{salt}".encode("utf-8")).digest()
    return random.Random(int.from_bytes(digest[:8], byteorder="little", signed=False))


def _binom(n: int, k: int) -> int:
    return math.comb(n, k)


def _fib(n: int) -> int:
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a


def _valid_parens(s: bytes) -> bool:
    depth = 0
    for ch in s:
        if ch == 0x28:  # (
            depth += 1
        elif ch == 0x29:  # )
            depth -= 1
        else:
            return False
        if depth < 0:
            return False
    return depth == 0


def _interval_scheduling_opt(intervals: list[tuple[int, int]]) -> int:
    best = 0
    n = len(intervals)
    for mask in range(1 << n):
        selected: list[tuple[int, int]] = []
        for i in range(n):
            if (mask >> i) & 1:
                selected.append(intervals[i])
        selected.sort(key=lambda x: x[1])
        ok = True
        last_end = None
        for s, e in selected:
            if last_end is not None and s < last_end:
                ok = False
                break
            last_end = e
        if ok:
            best = max(best, len(selected))
    return best


def _reachable(adj: list[list[int]], start: int, target: int) -> bool:
    n = len(adj)
    seen = [False] * n
    q = [start]
    seen[start] = True
    while q:
        v = q.pop(0)
        if v == target:
            return True
        for u in range(n):
            if adj[v][u] and not seen[u]:
                seen[u] = True
                q.append(u)
    return False


def _dedup_consecutive(b: bytes) -> bytes:
    if not b:
        return b""
    out = bytearray([b[0]])
    for ch in b[1:]:
        if ch != out[-1]:
            out.append(ch)
    return bytes(out)


def _rotate_left(b: bytes, k: int) -> bytes:
    if not b:
        return b""
    k = k % len(b)
    return b[k:] + b[:k]


def _is_subsequence(haystack: bytes, needle: bytes) -> bool:
    i = 0
    for ch in haystack:
        if i < len(needle) and ch == needle[i]:
            i += 1
    return i == len(needle)


@dataclass(frozen=True)
class TaskDef:
    task_id: str
    description: str
    gen: Callable[[random.Random, int], list[tuple[bytes, bytes]]]


def _gen_echo(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases = [(b"", b""), (b"abc", b"abc"), (bytes([0, 1, 2, 255]), bytes([0, 1, 2, 255]))]
    while len(cases) < n:
        ln = rng.randrange(0, 17)
        inp = bytes(rng.randrange(0, 256) for _ in range(ln))
        cases.append((inp, inp))
    return cases[:n]


def _gen_reverse(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases = [(b"", b""), (b"a", b"a"), (b"ab", b"ba"), (b"abc", b"cba")]
    while len(cases) < n:
        ln = rng.randrange(0, 17)
        inp = bytes(rng.randrange(0, 256) for _ in range(ln))
        cases.append((inp, inp[::-1]))
    return cases[:n]


def _gen_max_u8(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases = [(b"", bytes([0])), (bytes([0]), bytes([0])), (bytes([255]), bytes([255]))]
    while len(cases) < n:
        ln = rng.randrange(0, 17)
        inp = bytes(rng.randrange(0, 256) for _ in range(ln))
        expected = max(inp) if inp else 0
        cases.append((inp, bytes([expected])))
    return cases[:n]


def _gen_sum_u8_small(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases = [(b"", bytes([0])), (bytes([0, 0, 0]), bytes([0])), (bytes([15, 15, 15]), bytes([45]))]
    while len(cases) < n:
        ln = rng.randrange(0, 17)
        inp = bytes(rng.randrange(0, 16) for _ in range(ln))
        expected = sum(inp)
        cases.append((inp, bytes([expected])))
    return cases[:n]


def _gen_count_byte_u8(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases = [
        (bytes([0x41]) + b"", bytes([0])),
        (bytes([0x41]) + b"AAAA", bytes([4])),
        (bytes([0x00]) + bytes([0, 1, 0, 2, 0]), bytes([3])),
    ]
    while len(cases) < n:
        x = rng.randrange(0, 256)
        ln = rng.randrange(0, 33)
        data = bytes(rng.randrange(0, 256) for _ in range(ln))
        expected = data.count(x)
        cases.append((bytes([x]) + data, bytes([expected])))
    return cases[:n]


def _gen_fib_u8(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases = [(bytes([0]), bytes([0])), (bytes([1]), bytes([1])), (bytes([2]), bytes([1])), (bytes([13]), bytes([233]))]
    while len(cases) < n:
        k = rng.randrange(0, 14)
        cases.append((bytes([k]), bytes([_fib(k)])))
    return cases[:n]


def _gen_unique_paths_u8(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases = [
        (bytes([1, 1]), bytes([1])),
        (bytes([1, 6]), bytes([1])),
        (bytes([6, 1]), bytes([1])),
        (bytes([2, 2]), bytes([2])),
        (bytes([6, 6]), bytes([252])),
    ]
    while len(cases) < n:
        m = rng.randrange(1, 7)
        k = rng.randrange(1, 7)
        paths = _binom(m + k - 2, m - 1)
        if paths > 255:
            continue
        cases.append((bytes([m, k]), bytes([paths])))
    return cases[:n]


def _gen_interval_scheduling_u8(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = [
        (bytes([0]), bytes([0])),
        (bytes([1, 0, 1]), bytes([1])),
        (bytes([2, 0, 2, 2, 4]), bytes([2])),
        (bytes([3, 0, 10, 2, 3, 3, 4]), bytes([2])),
    ]
    while len(cases) < n:
        k = rng.randrange(0, 9)
        intervals: list[tuple[int, int]] = []
        for _ in range(k):
            start = rng.randrange(0, 16)
            end = rng.randrange(start + 1, 17)
            intervals.append((start, end))
        best = _interval_scheduling_opt(intervals)
        inp = bytes([k] + [x for s, e in intervals for x in (s, e)])
        cases.append((inp, bytes([best])))
    return cases[:n]


def _gen_graph_reachability_u8(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = []
    fixed = [
        # n=5, start=0, target=4, edges: 0->1->2->3->4
        (
            bytes([5, 0, 4])
            + bytes(
                [
                    0, 1, 0, 0, 0,
                    0, 0, 1, 0, 0,
                    0, 0, 0, 1, 0,
                    0, 0, 0, 0, 1,
                    0, 0, 0, 0, 0,
                ]
            ),
            bytes([1]),
        ),
        # n=5, start=4, target=0, no edges
        (bytes([5, 4, 0]) + bytes([0] * 25), bytes([0])),
    ]
    cases.extend(fixed)
    while len(cases) < n:
        size = 5
        start = rng.randrange(0, size)
        target = rng.randrange(0, size)
        adj = [[0] * size for _ in range(size)]
        for i in range(size):
            for j in range(size):
                if i == j:
                    continue
                adj[i][j] = 1 if rng.random() < 0.25 else 0
        ok = _reachable(adj, start, target)
        flat = [adj[i][j] for i in range(size) for j in range(size)]
        inp = bytes([size, start, target] + flat)
        cases.append((inp, bytes([1 if ok else 0])))
    return cases[:n]


def _gen_two_sum_indices(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = [
        (bytes([4, 10, 1, 2, 8, 9]), bytes([1, 2])),  # 2+8
        (bytes([2, 7, 3, 4]), bytes([0, 1])),
    ]
    while len(cases) < n:
        size = rng.randrange(2, 11)
        values = [rng.randrange(0, 128) for _ in range(size)]
        i = rng.randrange(0, size)
        j = rng.randrange(0, size)
        if i == j:
            continue
        if i > j:
            i, j = j, i
        target = values[i] + values[j]
        if target > 255:
            continue

        pairs = []
        for a in range(size):
            for b in range(a + 1, size):
                if values[a] + values[b] == target:
                    pairs.append((a, b))
        if pairs != [(i, j)]:
            continue

        inp = bytes([size, target] + values)
        cases.append((inp, bytes([i, j])))
    return cases[:n]


def _gen_valid_parentheses_u8(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = [
        (b"", bytes([1])),
        (b"()", bytes([1])),
        (b"(())", bytes([1])),
        (b")(", bytes([0])),
        (b"(()", bytes([0])),
        (b"())", bytes([0])),
    ]
    while len(cases) < n:
        ln = rng.randrange(0, 17)
        s = bytes(rng.choice([0x28, 0x29]) for _ in range(ln))
        cases.append((s, bytes([1 if _valid_parens(s) else 0])))
    return cases[:n]


def _gen_merge_sorted(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = [
        (bytes([0, 0]), b""),
        (bytes([1, 1, 2, 1, 3]), bytes([2, 3])),
        (bytes([3, 1, 2, 3, 2, 0, 4]), bytes([0, 1, 2, 2, 3])),
    ]
    while len(cases) < n:
        a_len = rng.randrange(0, 9)
        b_len = rng.randrange(0, 9)
        a = sorted(rng.randrange(0, 16) for _ in range(a_len))
        b = sorted(rng.randrange(0, 16) for _ in range(b_len))
        merged = sorted(a + b)
        inp = bytes([a_len] + a + [b_len] + b)
        cases.append((inp, bytes(merged)))
    return cases[:n]


def _gen_rle_encode(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = [
        (b"", b""),
        (b"A", bytes([1, 0x41])),
        (b"AAAB", bytes([3, 0x41, 1, 0x42])),
        (bytes([1, 1, 1, 1]), bytes([4, 1])),
    ]
    while len(cases) < n:
        ln = rng.randrange(0, 17)
        inp = bytes(rng.randrange(0, 4) for _ in range(ln))
        out = bytearray()
        i = 0
        while i < len(inp):
            j = i + 1
            while j < len(inp) and inp[j] == inp[i]:
                j += 1
            out.append(j - i)
            out.append(inp[i])
            i = j
        cases.append((inp, bytes(out)))
    return cases[:n]


def _gen_palindrome_u8(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = [
        (b"", bytes([1])),
        (b"a", bytes([1])),
        (b"aa", bytes([1])),
        (b"ab", bytes([0])),
        (b"abba", bytes([1])),
    ]
    while len(cases) < n:
        ln = rng.randrange(0, 17)
        inp = bytes(rng.randrange(0, 256) for _ in range(ln))
        cases.append((inp, bytes([1 if inp == inp[::-1] else 0])))
    return cases[:n]


def _gen_subsequence_u8(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = [
        (bytes([0, 0]), bytes([1])),
        (bytes([3, 1, 2, 3, 2, 1, 3]), bytes([1])),
        (bytes([3, 1, 2, 3, 2, 3, 1]), bytes([0])),
    ]
    while len(cases) < n:
        a_len = rng.randrange(0, 17)
        b_len = rng.randrange(0, 9)
        a = bytes(rng.randrange(0, 6) for _ in range(a_len))
        if rng.random() < 0.7:
            idxs = sorted(rng.sample(range(a_len), k=min(b_len, a_len))) if a_len else []
            b = bytes(a[i] for i in idxs) if idxs else b""
        else:
            b = bytes(rng.randrange(0, 6) for _ in range(b_len))
        ok = _is_subsequence(a, b)
        inp = bytes([len(a)]) + a + bytes([len(b)]) + b
        cases.append((inp, bytes([1 if ok else 0])))
    return cases[:n]


def _gen_rotate_left(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = [
        (bytes([0]), b""),
        (bytes([0]) + b"abc", b"abc"),
        (bytes([1]) + b"abc", b"bca"),
        (bytes([4]) + b"abc", b"bca"),
    ]
    while len(cases) < n:
        ln = rng.randrange(0, 17)
        k = rng.randrange(0, 16)
        data = bytes(rng.randrange(0, 256) for _ in range(ln))
        cases.append((bytes([k]) + data, _rotate_left(data, k)))
    return cases[:n]


def _gen_dedup_consecutive(rng: random.Random, n: int) -> list[tuple[bytes, bytes]]:
    cases: list[tuple[bytes, bytes]] = [
        (b"", b""),
        (b"A", b"A"),
        (b"AAABBBCCDAA", b"ABCDA"),
    ]
    while len(cases) < n:
        ln = rng.randrange(0, 33)
        inp = bytes(rng.randrange(0, 6) for _ in range(ln))
        cases.append((inp, _dedup_consecutive(inp)))
    return cases[:n]


def _suite_json(*, suite_id: str, tasks: list[TaskDef], seed: int, cases_per_task: int) -> dict:
    out = {"suite_id": suite_id, "world": "solve-pure", "tasks": []}
    for t in tasks:
        rng = _stable_rng(seed=seed, salt=f"{suite_id}:{t.task_id}")
        cases = t.gen(rng, cases_per_task)
        out["tasks"].append(
            {
                "task_id": t.task_id,
                "description": t.description,
                "cases": [{"input_b64": _b64(inp), "expected_b64": _b64(exp)} for inp, exp in cases],
            }
        )
    return out


def _write_or_check(path: Path, obj: dict, *, check: bool) -> None:
    encoded = json.dumps(obj, sort_keys=True, indent=2) + "\n"
    if check:
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current != encoded:
            raise SystemExit(f"mismatch: {path}")
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(encoded, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    repo_root = _repo_root()
    default_out_dir = repo_root / "labs" / "benchmarks" / "solve-pure"
    parser.add_argument("--out-dir", default=str(default_out_dir))
    parser.add_argument("--seed", type=int, default=1337)
    parser.add_argument("--cases", type=int, default=32)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    out_dir = Path(args.out_dir)
    seed = int(args.seed)
    cases_per_task = int(args.cases)
    check = bool(args.check)

    tier1_tasks = [
        TaskDef("echo", "Return the input bytes unchanged.", _gen_echo),
        TaskDef("reverse", "Return the input bytes reversed.", _gen_reverse),
        TaskDef(
            "max_byte_u8",
            "Return 1 byte: the maximum input byte value (0 if input is empty).",
            _gen_max_u8,
        ),
        TaskDef(
            "sum_u8_small",
            "Each input byte is in [0,15]. Return 1 byte: the sum of all input bytes (0 if empty).",
            _gen_sum_u8_small,
        ),
        TaskDef(
            "count_byte_u8",
            "Input: first byte is X, remaining bytes are data. Return 1 byte: count of X in data.",
            _gen_count_byte_u8,
        ),
    ]

    tier2_tasks = [
        TaskDef("fib_u8", "Input: 1 byte n in [0,13]. Output: 1 byte fib(n). fib(0)=0, fib(1)=1.", _gen_fib_u8),
        TaskDef(
            "unique_paths_u8",
            "Input: 2 bytes (m,n) with 1<=m,n<=6. Output: 1 byte number of unique paths from (0,0) to (m-1,n-1) moving only Right/Down.",
            _gen_unique_paths_u8,
        ),
        TaskDef(
            "interval_scheduling_u8",
            "Input: 1 byte k (0..8), then k intervals (start,end) as 2*k bytes with 0<=start<end<=16. Output: 1 byte maximum number of non-overlapping intervals (treat intervals as [start,end)).",
            _gen_interval_scheduling_u8,
        ),
        TaskDef(
            "graph_reachability_u8",
            "Input: n=5 (1 byte), start (1 byte), target (1 byte), then 25 bytes adjacency matrix (row-major, 0/1). Output: 1 byte 1 if a path exists from start to target, else 0.",
            _gen_graph_reachability_u8,
        ),
    ]

    tier3_tasks = [
        TaskDef(
            "two_sum_indices",
            "Input: 1 byte n (2..10), 1 byte target, then n bytes values (0..127). There is exactly one pair i<j with values[i]+values[j]==target. Output: 2 bytes (i,j).",
            _gen_two_sum_indices,
        ),
        TaskDef(
            "valid_parentheses_u8",
            "Input: bytes are ASCII '(' and ')'. Output: 1 byte 1 if parentheses are balanced, else 0.",
            _gen_valid_parentheses_u8,
        ),
        TaskDef(
            "merge_sorted",
            "Input: 1 byte n, n sorted bytes (0..15), 1 byte m, m sorted bytes (0..15). Output: merged sorted bytes of length n+m.",
            _gen_merge_sorted,
        ),
        TaskDef(
            "rle_encode",
            "Output run-length encoding as pairs (count,byte). Example: AAAB -> [3,'A',1,'B'].",
            _gen_rle_encode,
        ),
    ]

    holdout_tasks = [
        TaskDef("palindrome_u8", "Output 1 byte 1 if input is a palindrome, else 0.", _gen_palindrome_u8),
        TaskDef(
            "subsequence_u8",
            "Input: lenA (1 byte), A bytes, lenB (1 byte), B bytes. Output: 1 byte 1 if B is a subsequence of A, else 0.",
            _gen_subsequence_u8,
        ),
        TaskDef(
            "rotate_left",
            "Input: k (1 byte), then data bytes. Output: data rotated left by k positions (k modulo len(data)).",
            _gen_rotate_left,
        ),
        TaskDef(
            "dedup_consecutive",
            "Output input with consecutive duplicate bytes removed. Example: AAABBBCCDAA -> ABCDA.",
            _gen_dedup_consecutive,
        ),
    ]

    suites = [
        ("phase4-tier1-toy@0.1.0", out_dir / "phase4-tier1-toy.json", tier1_tasks),
        ("phase4-tier2-leetcode@0.1.0", out_dir / "phase4-tier2-leetcode.json", tier2_tasks),
        ("phase4-tier3-algorithms@0.1.0", out_dir / "phase4-tier3-algorithms.json", tier3_tasks),
        ("phase4-holdout@0.1.0", out_dir / "phase4-holdout.json", holdout_tasks),
    ]

    for suite_id, path, tasks in suites:
        obj = _suite_json(suite_id=suite_id, tasks=tasks, seed=seed, cases_per_task=cases_per_task)
        _write_or_check(path, obj, check=check)


if __name__ == "__main__":
    main()
