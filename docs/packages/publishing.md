# Publishing packages

X07 supports a Cargo-like workflow:

- pack
- lock
- publish

For a copy/paste end-to-end tutorial, see: [Publishing by example](publishing-by-example.md).

## Quickstart (package repo)

Create a publishable package repo:

```bash
mkdir mypkg
cd mypkg
x07 init --package
```

Then follow the canonical workflow (also printed in the init JSON report):

```bash
# Edit x07-package.json: set description/docs; bump version
x07 test --manifest tests/tests.json
x07 pkg pack --package . --out dist/<name>-<version>.x07pkg
x07 pkg login --index sparse+https://registry.x07.io/index/
x07 pkg publish --index sparse+https://registry.x07.io/index/ --package .
```

## Source-only packages

Packages publish only x07AST JSON sources.

That keeps builds:

- deterministic
- auditable
- portable across platforms

## Registry authentication

Publishing uses bearer tokens.

To publish to the official registry:

- Browse packages: https://x07.io/packages
- Index URL: `sparse+https://registry.x07.io/index/`

### Getting a token

1. Sign in with GitHub at https://x07.io/settings/tokens
2. Create a token with the `publish` scope (copy it now; you won’t be able to see it again)

Do not share tokens between people.

Publishing requires a verified email on your GitHub account.

### Storing a token locally

Prefer prompting or stdin so the token doesn’t end up in shell history:

```bash
# Prompts for token (recommended for humans)
x07 pkg login --index sparse+https://registry.x07.io/index/

# Non-interactive (recommended for CI)
printf '%s' "$X07_TOKEN" | x07 pkg login --index sparse+https://registry.x07.io/index/ --token-stdin
```

### Publishing

Ensure `x07-package.json` contains a non-empty `description` and `docs`, bump the `version`, then publish the directory:

```bash
x07 pkg publish --index sparse+https://registry.x07.io/index/ --package .
```

## Versioning

- package versions follow semver-style rules
- wire formats are pinned by `_v1` suffixes (see versioning policy)
