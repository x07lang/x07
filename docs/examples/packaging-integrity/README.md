# Packaging integrity examples

These fixtures are designed to run offline:

- `x07 pkg verify` against a signed `file://` sparse index
- `x07 pkg check-semver` against local package directories

## Verify (signed local index)

From this directory:

```bash
INDEX_URL="sparse+file://$PWD/signed-index/"
x07 pkg verify integrity-demo@0.1.0 --index "$INDEX_URL" --offline
```

Optional: pack the package and compare the digest to the index entry `cksum`:

```bash
x07 pkg pack --package pkg/integrity-demo --out /tmp/integrity-demo-0.1.0.x07pkg
shasum -a 256 /tmp/integrity-demo-0.1.0.x07pkg
```

## Semver checks

Compatible change (should succeed):

```bash
x07 pkg check-semver --old semver/old --new semver/new-compatible
```

Breaking change (should fail with exit code 20):

```bash
x07 pkg check-semver --old semver/old --new semver/new-breaking
```

