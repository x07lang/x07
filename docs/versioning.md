# Versioning

The canonical, full description of how X07 versions its toolchain, stdlib
packages, and machine-readable contracts lives in
[Versioning policy](versioning-policy.md).

Quick map:

- **Toolchain** (`x07`, `x07c`) — SemVer.
- **Stdlib packages** — versioned independently and pinned by `stdlib.lock`.
- **Schemas/contracts** — versioned by their `schema_version` identifiers (and
  `_v1`/`_v2` function suffixes); contracts do not change in-place.
