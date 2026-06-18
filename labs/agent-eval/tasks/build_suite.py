#!/usr/bin/env python3
"""Build the agent-eval task suite: tag existing tasks with difficulty bands and
append reference-backed new tasks to reach 30 tasks (10 per band).

Single source of truth: each new task's transform is one Python snippet used
both to GENERATE its vectors (so expected outputs are correct by construction)
and to EMIT a reference Python solution (so the whole suite is validated
end-to-end via `runner.py --lang python`).

All task I/O stays within ASCII (bytes <= 0x7F) so a vector string round-trips
through the runner's `vector["input"].encode()` (UTF-8) back to the intended
bytes. The builder asserts this.

Usage:
  python3 build_suite.py            # rewrite tasks/tasks.json + reference solutions
  python3 build_suite.py --check    # fail if tasks.json would change
"""
from __future__ import annotations

import argparse
import json
import sys
from collections import Counter  # noqa: F401  (used by exec'd snippets)
from pathlib import Path

HERE = Path(__file__).resolve().parent
TASKS_JSON = HERE / "tasks.json"
REF_DIR = HERE.parent / "solutions" / "reference"

# Difficulty bands for the existing 12 tasks.
#   a = byte/text transforms, b = data-structure logic, c = protocol/codec
EXISTING_BANDS = {
    "upper_ascii": "a",
    "count_newlines": "a",
    "rle_encode": "a",
    "reverse_lines": "a",
    "sum_lines": "b",
    "top_word": "b",
    "dedupe_lines": "b",
    "csv_sum_col": "b",
    "second_largest": "b",
    "json_name": "c",
    "fnv1a32": "c",
    "frame_u32le": "c",
}

# Each new task: id, title, band, prompt, transform snippet (data:bytes -> out:bytes),
# and input vectors (bytes, all <= 0x7F). Snippets run with `data` bound and the
# header imports available; they must assign `out` (bytes).
NEW_TASKS = [
    # ---- band a: byte/text transforms ----
    (
        "rot13", "ROT13 letters", "a",
        "Read all input bytes. Apply ROT13 to ASCII letters (a-z and A-Z rotate "
        "by 13 within their case); every other byte passes through unchanged.",
        "out=bytes(((c-97+13)%26+97) if 97<=c<=122 else ((c-65+13)%26+65) if 65<=c<=90 else c for c in data)",
        [b"Hello, World!", b"", b"abc xyz", b"n N z Z"],
    ),
    (
        "swap_case", "Swap ASCII case", "a",
        "Read all input bytes. Swap the case of ASCII letters (a-z -> A-Z and "
        "A-Z -> a-z); every other byte passes through unchanged.",
        "out=bytes((c-32) if 97<=c<=122 else (c+32) if 65<=c<=90 else c for c in data)",
        [b"Hello World 123", b"", b"aA bB", b"Mixed CaSe!"],
    ),
    (
        "strip_trailing_spaces", "Strip trailing line whitespace", "a",
        "Split input on 0x0A newlines into lines (a trailing newline does not "
        "create a final empty line; a missing trailing newline still ends a final "
        "line). Remove trailing spaces (0x20) and tabs (0x09) from each line. "
        "Output each stripped line followed by 0x0A. Empty input produces empty output.",
        "s=data.split(b'\\n')\n"
        "if s and s[-1]==b'': s=s[:-1]\n"
        "out=b''.join(l.rstrip(b' \\t')+b'\\n' for l in s)",
        [b"a  \nb\t \n", b"hello   ", b"", b"x\n   \ny"],
    ),
    (
        "collapse_spaces", "Collapse space runs", "a",
        "Read all input bytes. Replace every maximal run of one or more 0x20 "
        "space bytes with a single 0x20 space. All other bytes pass through unchanged.",
        "import re\nout=re.sub(b' +', b' ', data)",
        [b"a   b  c", b"", b"  x  ", b"no  doubles\nhere   too"],
    ),
    (
        "byte_count", "Count bytes", "a",
        "Read all input bytes. Output the total number of input bytes as decimal "
        "ASCII digits with no trailing newline.",
        "out=str(len(data)).encode()",
        [b"hello", b"", b"a\nb\n", b"x"],
    ),
    (
        "remove_vowels", "Remove ASCII vowels", "a",
        "Read all input bytes. Output the same bytes with every ASCII vowel "
        "removed (a, e, i, o, u in both lower and upper case). All other bytes "
        "pass through unchanged.",
        "out=bytes(c for c in data if c not in b'aeiouAEIOU')",
        [b"hello world", b"", b"AEIOU xyz", b"bcd"],
    ),
    # ---- band b: data-structure logic ----
    (
        "sort_ints", "Sort integers", "b",
        "Input is zero or more non-negative decimal integers (< 1000000) "
        "separated by ASCII whitespace. Output the integers sorted ascending, "
        "separated by single 0x20 spaces, no trailing newline. Empty input "
        "produces empty output.",
        "xs=[int(x) for x in data.split()]\n"
        "out=(' '.join(str(x) for x in sorted(xs))).encode() if xs else b''",
        [b"3 1 2", b"10\n5 20", b"", b"7", b"5 5 1"],
    ),
    (
        "unique_sorted", "Unique sorted integers", "b",
        "Input is zero or more non-negative decimal integers (< 1000000) "
        "separated by ASCII whitespace. Output the DISTINCT integers sorted "
        "ascending, separated by single 0x20 spaces, no trailing newline. Empty "
        "input produces empty output.",
        "xs=[int(x) for x in data.split()]\n"
        "out=(' '.join(str(x) for x in sorted(set(xs)))).encode() if xs else b''",
        [b"3 1 2 1 3", b"5 5 5", b"", b"9 1 9 2"],
    ),
    (
        "mode_int", "Most frequent integer", "b",
        "Input is one or more non-negative decimal integers (< 1000000) separated "
        "by ASCII whitespace. Output the most frequently occurring integer as "
        "decimal ASCII, no trailing newline; break frequency ties by the smallest "
        "value.",
        "xs=[int(x) for x in data.split()]\n"
        "c=Counter(xs)\n"
        "out=str(max(c, key=lambda k:(c[k],-k))).encode()",
        [b"1 2 2 3", b"5 5 1 1", b"7", b"3 3 2 2 2 9"],
    ),
    (
        "running_sum", "Running sum", "b",
        "Input is zero or more non-negative decimal integers (< 1000000) "
        "separated by ASCII whitespace. Output the cumulative running sum: after "
        "reading each integer, output the sum so far as decimal ASCII followed by "
        "0x0A. Empty input produces empty output.",
        "xs=[int(x) for x in data.split()]\n"
        "s=0; parts=[]\n"
        "for x in xs:\n    s+=x; parts.append(str(s))\n"
        "out=(''.join(p+'\\n' for p in parts)).encode()",
        [b"1 2 3", b"5", b"", b"10 0 5"],
    ),
    (
        "int_histogram", "Integer histogram", "b",
        "Input is zero or more non-negative decimal integers (< 1000000) "
        "separated by ASCII whitespace. For each DISTINCT value in ascending "
        "order, output 'value:count' (value decimal, a single 0x3A colon, count "
        "decimal) followed by 0x0A. Empty input produces empty output.",
        "xs=[int(x) for x in data.split()]\n"
        "c=Counter(xs)\n"
        "out=(''.join('%d:%d\\n'%(k,c[k]) for k in sorted(c))).encode()",
        [b"1 1 2", b"5", b"", b"3 3 3 1"],
    ),
    # ---- band c: protocol/codec ----
    (
        "deframe_u32le", "Decode length-prefixed frames", "c",
        "Input is zero or more frames concatenated. Each frame is a 4-byte "
        "little-endian u32 length L followed by exactly L payload bytes. The input "
        "is well-formed (no truncation). Output each frame's payload followed by "
        "0x0A, in order. Empty input produces empty output.",
        "out=bytearray(); i=0\n"
        "while i<len(data):\n"
        "    L=int.from_bytes(data[i:i+4],'little'); i+=4\n"
        "    out+=data[i:i+L]; i+=L; out+=b'\\n'\n"
        "out=bytes(out)",
        [bytes([2, 0, 0, 0]) + b"ab" + bytes([1, 0, 0, 0]) + b"c",
         bytes([3, 0, 0, 0]) + b"hey", b"", bytes([0, 0, 0, 0])],
    ),
    (
        "base64_encode", "Base64 encode", "c",
        "Read all input bytes. Output their standard Base64 encoding (RFC 4648, "
        "alphabet A-Za-z0-9+/, with '=' padding), no trailing newline.",
        "import base64\nout=base64.b64encode(data)",
        [b"", b"f", b"fo", b"foo", b"hello"],
    ),
    (
        "base64_decode", "Base64 decode", "c",
        "Input is a valid standard Base64 string (RFC 4648, with '=' padding, no "
        "whitespace). Output the decoded raw bytes.",
        "import base64\nout=base64.b64decode(data)",
        [b"Zm9v", b"aGVsbG8=", b"", b"Zg=="],
    ),
    (
        "hex_encode", "Hex encode", "c",
        "Read all input bytes. Output their lowercase hexadecimal encoding (two "
        "hex digits per byte, 0-9a-f), no trailing newline.",
        "out=data.hex().encode()",
        [b"", b"A", b"hi", b"hello", b"0"],
    ),
    (
        "hex_decode", "Hex decode", "c",
        "Input is an even-length lowercase hexadecimal string (0-9a-f, no "
        "whitespace). Output the decoded raw bytes.",
        "out=bytes.fromhex(data.decode())",
        [b"41", b"6869", b"", b"68656c6c6f"],
    ),
    (
        "crc32", "CRC-32 checksum", "c",
        "Compute the CRC-32 (IEEE 802.3 / zlib variant: polynomial 0xEDB88320, "
        "initial value 0xFFFFFFFF, final XOR 0xFFFFFFFF, reflected) of all input "
        "bytes. Output the 32-bit result as decimal ASCII, no trailing newline.",
        "import zlib\nout=str(zlib.crc32(data)&0xffffffff).encode()",
        [b"", b"a", b"hello", b"x07"],
    ),
    (
        "json_array_len", "JSON array length", "c",
        "Input is a single well-formed JSON array (the top-level value is an "
        "array). Output the number of top-level elements as decimal ASCII, no "
        "trailing newline. Elements may be any JSON value; nesting and whitespace vary.",
        "import json\nout=str(len(json.loads(data.decode()))).encode()",
        [b"[1,2,3]", b"[]", b'["a", [1,2], {"x":1}]', b"[ 1 , 2 ]"],
    ),
]

PY_HEADER = (
    "import sys, base64, zlib, json, re\n"
    "from collections import Counter\n"
    "data = sys.stdin.buffer.read()\n"
)


def _run_snippet(src: str, data: bytes) -> bytes:
    ns = {"data": data, "Counter": Counter}
    exec(src, ns)  # noqa: S102 (trusted, in-repo snippets)
    out = ns["out"]
    if not isinstance(out, (bytes, bytearray)):
        raise SystemExit(f"snippet produced non-bytes output: {type(out)}")
    return bytes(out)


def _ascii_str(b: bytes, what: str, task_id: str) -> str:
    if any(byte > 0x7F for byte in b):
        raise SystemExit(f"{task_id}: {what} has a byte > 0x7F; vectors must be ASCII-safe")
    return b.decode("ascii")


def build_new_tasks() -> list[dict]:
    out = []
    for task_id, title, band, prompt, src, inputs in NEW_TASKS:
        vectors = []
        for inp in inputs:
            got = _run_snippet(src, inp)
            vectors.append({
                "input": _ascii_str(inp, "input", task_id),
                "expected": _ascii_str(got, "expected", task_id),
            })
        out.append({
            "id": task_id,
            "title": title,
            "band": band,
            "prompt": prompt,
            "vectors": vectors,
        })
    return out


def emit_reference_solutions() -> None:
    """Write a complete 30-task Python reference baseline to solutions/reference/.

    The 18 new tasks come from their snippets; the 12 hand-authored tasks reuse
    the validated pilot Python solutions, so `runner.py --lang python
    --solutions solutions/reference` checks the whole suite in one shot.
    """
    REF_DIR.mkdir(parents=True, exist_ok=True)
    for task_id, _title, _band, _prompt, src, _inputs in NEW_TASKS:
        body = PY_HEADER + src + "\nsys.stdout.buffer.write(out)\n"
        (REF_DIR / f"{task_id}.py").write_text(body)
    pilot = HERE.parent / "solutions" / "claude-pilot"
    for task_id in EXISTING_BANDS:
        src_py = pilot / f"{task_id}.py"
        if src_py.is_file():
            (REF_DIR / f"{task_id}.py").write_text(src_py.read_text())


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="fail if tasks.json would change")
    args = ap.parse_args(argv)

    doc = json.loads(TASKS_JSON.read_text())
    by_id = {t["id"]: t for t in doc["tasks"]}

    # Tag existing tasks with bands (insert 'band' right after 'title').
    for task_id, band in EXISTING_BANDS.items():
        if task_id not in by_id:
            raise SystemExit(f"existing task not found: {task_id}")
        t = by_id[task_id]
        rebuilt = {}
        for k, v in t.items():
            rebuilt[k] = v
            if k == "title":
                rebuilt["band"] = band
        if "band" not in rebuilt:
            rebuilt["band"] = band
        by_id[task_id] = rebuilt

    # Base is exactly the 12 hand-authored tasks (in original order); drop any
    # previously appended generated tasks so re-running is idempotent.
    existing = [by_id[t["id"]] for t in doc["tasks"] if t["id"] in EXISTING_BANDS]
    new = build_new_tasks()
    seen = {t["id"] for t in existing}
    for t in new:
        if t["id"] in seen:
            raise SystemExit(f"duplicate task id: {t['id']}")
    doc["tasks"] = existing + new

    bands = Counter(t["band"] for t in doc["tasks"])
    payload = json.dumps(doc, indent=2, ensure_ascii=True) + "\n"

    if args.check:
        current = TASKS_JSON.read_text()
        if current != payload:
            raise SystemExit("tasks.json out of date; run build_suite.py")
        print(f"ok: tasks.json current ({len(doc['tasks'])} tasks, bands {dict(bands)})")
        return 0

    TASKS_JSON.write_text(payload)
    emit_reference_solutions()
    print(f"wrote {len(doc['tasks'])} tasks (bands {dict(bands)}); "
          f"{len(NEW_TASKS)} reference solutions in {REF_DIR.relative_to(HERE.parent.parent)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
