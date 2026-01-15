# Publishing packages

X07 supports a Cargo-like workflow:

- pack
- lock
- publish

## Source-only packages

Packages publish only x07AST JSON sources.

That keeps builds:

- deterministic
- auditable
- portable across platforms

## Registry authentication

Registries can use simple API tokens.

## Versioning

- package versions follow semver-style rules
- wire formats are pinned by `_v1` suffixes (see versioning policy)
