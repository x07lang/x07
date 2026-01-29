# Regex v1 (ext.regex)

This document pins the v1 contract for `ext.regex` (reference implementation: `packages/ext/x07-ext-regex/0.2.3/modules/ext/regex.x07.json`).

Build note: `ext.regex` is backed by a native static library; stage it with `./scripts/build_ext_regex.sh` before compiling programs that use it.

## Scope

- Deterministic **byte regex** engine (no backreferences, no look-around).
- Inputs are `bytes_view`; patterns operate on bytes (ASCII-oriented syntax and classes).

## Compile

- `ext.regex.compile(pattern: bytes_view) -> bytes`
- `ext.regex.compile_opts_v1(pattern: bytes_view, opts_u32: i32) -> bytes`

Compile result doc format:

- **ERR**: `tag_u8=0` + `code_u32_le` + `pos_u32_le`
- **OK**: `tag_u8=1` + opaque compiled blob (versioned by the package version)

Options (`opts_u32` bitmask):

- `ext.regex.opts_casei_v1() -> i32` (bit `1`)
- `ext.regex.opts_multiline_v1() -> i32` (bit `2`)
- `ext.regex.opts_dotall_v1() -> i32` (bit `4`)

## Exec (match docs)

- `ext.regex.exec(compiled: bytes_view, text: bytes_view) -> bytes`
- `ext.regex.exec_from_v1(compiled: bytes_view, text: bytes_view, start_i32: i32) -> bytes` (absolute start/end)

Exec result doc format:

- **ERR**: `tag_u8=0` + `code_u32_le` + `pos_u32_le`
- **OK**: `tag_u8=1` + `is_match_u8` + `start_u32_le` + `end_u32_le`

Accessors:

- `ext.regex.is_err(doc)`, `ext.regex.err_code(doc)`, `ext.regex.err_pos(doc)`
- `ext.regex.is_match(doc)`, `ext.regex.match_start(doc)`, `ext.regex.match_end(doc)`, `ext.regex.match_len(doc)`

Match semantics:

- Search is **leftmost-longest**: it picks the smallest start offset, and for that start picks the longest match.
- Helpers that iterate matches (see below) advance on empty matches by `pos = s+1` when `e == s`.

## Captures v1

- `ext.regex.exec_caps_v1(compiled: bytes_view, text: bytes_view) -> bytes`
- `ext.regex.exec_caps_from_v1(compiled: bytes_view, text: bytes_view, start_i32: i32) -> bytes` (absolute start/end)

Captures result doc format:

- **ERR**: `tag_u8=0` + `code_u32_le` + `pos_u32_le`
- **OK**: `tag_u8=1` + `is_match_u8` + `start_u32_le` + `end_u32_le` + `cap_count_u32_le` + `cap_count * (cap_start_u32_le, cap_end_u32_le)`

Missing captures use `0xFFFFFFFF` for both `cap_start` and `cap_end`.

Capture indexing:

- `cap_i` is **1-based**: `1..cap_count`
- Only capturing `(...)` groups count; non-capturing `(?:...)` groups do not.

Accessors:

- `ext.regex.caps_count_v1(doc) -> i32`
- `ext.regex.cap_start_v1(doc, cap_i) -> i32`
- `ext.regex.cap_end_v1(doc, cap_i) -> i32`
- `ext.regex.cap_len_v1(doc, cap_i) -> i32`
- `ext.regex.cap_view_v1(text, doc, cap_i) -> bytes_view`

## X7SL helpers

These helpers return X7SL v1 slice lists (see [X7SL v1](x7sl-v1.md)), intended to be consumed via `std.text.slices`.

- `ext.regex.find_all_x7sl_v1(compiled: bytes_view, text: bytes_view, max_matches_i32: i32) -> bytes` (match slices)
- `ext.regex.split_v1(compiled: bytes_view, text: bytes_view, max_parts_i32: i32) -> bytes` (segment slices; includes empty segments)

## Replace

- `ext.regex.replace_all_v1(compiled: bytes_view, text: bytes_view, repl: bytes_view, cap_limit_i32: i32) -> bytes`

Semantics:

- `repl` is inserted **literally** (no `$1`/`${name}` substitutions in v1).
- `cap_limit_i32 <= 0` means “unlimited replacements”.
