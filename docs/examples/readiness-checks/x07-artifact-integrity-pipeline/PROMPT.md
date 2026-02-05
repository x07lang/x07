# Prompt 4 — Crypto + compression + archive + diff integrity pipeline

```text
You are an X07 coding agent building `x07-artifact-integrity-pipeline`.

GOAL
Build a production-style “artifact packer” tool with subcommands:
- pack: directory -> tar -> compress -> manifest
- verify: manifest + blob -> verify checksums/hashes/signatures -> unpack (optional)
- diff: two manifests -> compute patch_v1 and apply it

START PROJECT
https://x07lang.org/docs/getting-started/agent-quickstart
mkdir myapp
cd myapp
x07 init

DEPENDENCIES (lock them)
Pure-ish integrity + encoding:
- ext-crypto-rs
- ext-checksum-rs
- ext-base64-rs
- ext-hex-rs
- ext-byteorder-rs
- ext-u64-rs
- ext-uuid-rs
- ext-semver-rs
- ext-diff-rs (must use patch_v1 format)

Archiving/compression:
- ext-tar-rs
- ext-compress-rs (ext.compress/ext.zip/ext.zstd)

OS/FFI coverage (run-os only):
- ext-openssl-c (ed25519/rand/hash)
- ext-zlib-c (zlib bindings)

CLI:
- ext-cli

REQUIREMENTS
A) Produce a manifest JSON that includes:
   - semver for artifact schema versioning
   - uuid for artifact id
   - per-file metadata (path, size, sha256, crc32c, optional xxhash64)
   - archive format + compression parameters
   - base64 + hex representations for selected digests
B) Determinism: with fixed input files and fixed compression settings, output must be stable.
C) Implement “patching”:
   - given two manifests+blobs, compute a patch_v1 that transforms old->new (bytes and/or lines)
   - apply the patch_v1 and verify the resulting blob hash matches the target.
D) Demonstrate correct endian-safe encoding for binary headers using ext.byteorder and/or std.codec.

TESTS
- fixtures/ with a small directory tree (text + binary)
- tests that run in run-os-sandboxed with:
  - allow-read-root fixtures/
  - allow-write-root out/
  - deny all network
- verify includes both ext.crypto and ext.openssl hash paths, and includes zlib usage path.

DELIVERABLES
- `x07 bundle --profile sandbox` produces a normal CLI binary that can be run without framed I/O.
- Provide README commands to run pack/verify/diff.
```

