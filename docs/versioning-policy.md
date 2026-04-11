# Versioning policy

X07 versioning is layered so agents can rely on stable machine-readable contracts while the
toolchain and ecosystem continue to evolve.

The key idea: **contracts do not change in-place**. New behavior comes with new versions and a
documented migration story.

See also:

- `docs/reference/compat.md` (compatibility contract)
- `docs/packages/index.md` (projects, lockfiles, and dependency resolution)
- `docs/stability.md` (stability principles)

## Stable contracts: `_v1` and schema ids

A function name like `parse_v1` means:

- its bytes encoding is pinned
- its error code space is pinned
- its semantics are pinned

New encodings become `_v2` (and live side-by-side).

Similarly, JSON schemas and on-disk formats are versioned by explicit identifiers like:

- `x07.project@0.5.0` (project manifest)
- `x07.lock@0.4.0` (project lockfile)
- `x07.x07ast@0.8.0` (program/module format)

## Toolchain SemVer policy

The toolchain (`x07`, `x07c`) is released with SemVer tags (`vX.Y.Z`).

Policy:

- Toolchain releases should be **compatible by default** with official packages and documented public
  surfaces.
- If a change risks breaking downstream code, it must come with either:
  - a compatibility switch, or
  - a mechanical migration path (`x07 fix` and `x07 migrate`)
- Any compat-relevant behavior change must be documented in `docs/reference/compat.md` and backed by
  a regression surface in `tests/compat_corpus/`.

## Package SemVer policy

Packages are versioned independently (SemVer) and pinned by lockfiles.

Package versions may change for:

- bug fixes
- performance improvements
- new helpers

But versioned contracts (`*_v1`, spec schemas, and other explicitly versioned envelopes) must remain
compatible. Breaking changes require:

- `_v2` APIs, or
- a new major version when packages reach 1.0.

## Package manifest compatibility metadata

Publishable packages use `x07-package.json` (`x07.package@0.1.0`) plus `meta` fields.

Policy:

- Packages must declare transitive hard requirements through `meta.requires_packages` so dependency
  resolution is explicit.
- Packages must declare the worlds they support in `meta.worlds_allowed` (and stay within those
  capability bounds).
- Packages may declare toolchain support via `meta.x07c_compat` (a SemVer requirement string, for
  example `\">=0.1.111 <0.2.0\"`). When present, `x07 pkg lock` treats incompatible packages as
  ineligible and fails with a deterministic error.
- When a package’s exported contracts change (types, encodings, semantics), it must bump its own
  SemVer version and keep old versioned APIs (`*_v1`) intact.

## Lockfile schema evolution policy

Projects pin dependency graphs with `x07.lock.json` (`x07.lock@…`).

Policy:

- Lockfiles are canonical JSON and must be deterministic.
- The current lockfile schema (`x07.lock@0.4.0`) includes `toolchain` identity and `registry` provenance to make upgrades debuggable.
- Tooling should continue to **read** older lockfile schema versions. If a lockfile cannot be read,
  the toolchain must fail with a deterministic error and a migration path.
- CI should use `x07 pkg lock --check` to fail on lock drift instead of mutating workspaces.
- Schema bumps should be additive when possible and gated by the compat corpus.

Notes:

- `x07 pkg lock` writes `x07.lock@0.4.0` by default; use `x07 pkg lock --lock-version 0.3` only when interoperating with an older toolchain.
- After a toolchain upgrade, use `x07 pkg repair --toolchain current` to rewrite incompatible locks deterministically.
