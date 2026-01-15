# Versioning policy (short)

X07 versioning has two layers:

1) **Package version** (e.g., `0.1.1`)
2) **Wire/spec version** (e.g., `_v1` suffixes, `MAGIC+VERSION` headers)

## Why `_v1` exists everywhere

Agents need stability.

A function name like `parse_v1` means:

- its bytes encoding is pinned,
- its error code space is pinned,
- its semantics are pinned.

New encodings become `_v2` (and live side-by-side).

## How this relates to package versions

Package versions can change for:

- bug fixes
- performance improvements
- new helpers

…but `_v1` contracts must remain compatible.

Breaking changes require:

- `_v2` APIs, or
- a new major package version once you reach 1.0.
