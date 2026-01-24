# Publish a package by example (x07-package.json → x07.io)

This page is a step-by-step workflow for creating and publishing an X07 package to the public registry.

It is written to keep the story consistent across:

- `x07` (toolchain)
- x07lang.org docs
- x07.io (registry UI)

If you just want the reference flags, see [Publishing](publishing.md).

## Mental model

Two similar-looking JSON files have different jobs:

- `x07.json` — project manifest (apps + local dev). Defines module roots, entrypoint, profiles, dependencies.
- `x07-package.json` — package manifest (publish contract). Defines name/version, module root, and which module IDs are included in the artifact.

A package repo will typically contain both:

- `x07-package.json` (publish)
- `x07.json` (local tests, demos, CI)

## Example package: `ext-hello-demo`

This tutorial publishes a tiny library module plus a published tests module:

- `ext.hello`
- `ext.hello.tests`

If you are publishing to the public registry, choose a unique package name (for example, include your org/handle).

Companion example (in the `x07` repo):

- `examples/tutorials/package_publish_ext_hello/`

## Step 0: create the skeleton

```bash
mkdir ext-hello-demo
cd ext-hello-demo
x07 init --package
```

Normalize the layout to the canonical “publishable modules” convention:

```bash
rm -rf src
mkdir -p modules/ext/hello
```

## Step 1: write the library module

Create `modules/ext/hello.x07.json`:

```json
{
  "schema_version": "x07.x07ast@0.1.0",
  "kind": "module",
  "module_id": "ext.hello",
  "imports": [],
  "decls": [
    { "kind": "export", "names": ["ext.hello.hello_v1"] },
    {
      "kind": "defn",
      "name": "ext.hello.hello_v1",
      "params": [{ "name": "name", "ty": "bytes_view" }],
      "result": "bytes",
      "body": [
        "begin",
        ["let", "prefix", ["bytes.concat", ["bytes.lit", "hello,"], ["bytes1", 32]]],
        ["let", "tmp", ["bytes.concat", "prefix", ["view.to_bytes", "name"]]],
        ["bytes.concat", "tmp", ["bytes1", 10]]
      ]
    }
  ]
}
```

## Step 2: add a published tests module (recommended)

Create `modules/ext/hello/tests.x07.json`:

```json
{
  "schema_version": "x07.x07ast@0.1.0",
  "kind": "module",
  "module_id": "ext.hello.tests",
  "imports": ["ext.hello", "std.test"],
  "decls": [
    { "kind": "export", "names": ["ext.hello.tests.test_hello_v1"] },
    {
      "kind": "defn",
      "name": "ext.hello.tests.test_hello_v1",
      "params": [],
      "result": "result_i32",
      "body": [
        "begin",
        ["let", "name", ["bytes.lit", "x07"]],
        ["let", "got", ["ext.hello.hello_v1", ["bytes.view", "name"]]],
        ["let", "expected_prefix", ["bytes.concat", ["bytes.lit", "hello,"], ["bytes1", 32]]],
        ["let", "expected_tmp", ["bytes.concat", "expected_prefix", "name"]],
        ["let", "expected", ["bytes.concat", "expected_tmp", ["bytes1", 10]]],
        ["try", ["std.test.assert_bytes_eq", "got", "expected", ["std.test.code_assert_bytes_eq"]]],
        ["std.test.pass"]
      ]
    }
  ]
}
```

## Step 3: write `x07-package.json`

Replace `x07-package.json` with:

```json
{
  "schema_version": "x07.package@0.1.0",
  "name": "ext-hello-demo",
  "version": "0.1.0",
  "description": "Tutorial package: ext.hello.hello_v1(name) -> bytes.",
  "docs": "Tutorial package used in the x07lang.org publishing-by-example docs.\n\nModules:\n- ext.hello\n- ext.hello.tests\n\nUsage:\n- Add: x07 pkg add ext-hello-demo@VERSION\n- Import: ext.hello\n- Call: ext.hello.hello_v1\n",
  "module_root": "modules",
  "modules": ["ext.hello", "ext.hello.tests"],
  "meta": {
    "determinism_tier": "pure",
    "worlds_allowed": ["solve-pure", "solve-fs", "solve-rr", "solve-kv", "solve-full", "run-os", "run-os-sandboxed"],
    "import_mode": "handwritten"
  }
}
```

## Step 4: add a CI-friendly tests manifest

Create `tests/tests.json`:

```json
{
  "schema_version": "x07.tests_manifest@0.1.0",
  "tests": [
    { "id": "hello_v1", "world": "solve-pure", "entry": "ext.hello.tests.test_hello_v1", "expect": "pass" }
  ]
}
```

Create a dev `x07.json` so `x07 test` can automatically discover your module roots:

```json
{
  "schema_version": "x07.project@0.2.0",
  "world": "solve-pure",
  "entry": "modules/ext/hello/tests.x07.json",
  "module_roots": ["modules"],
  "lockfile": "x07.lock.json",
  "dependencies": []
}
```

Run tests:

```bash
x07 test --manifest tests/tests.json
```

## Step 5: pack locally (optional sanity check)

```bash
mkdir -p dist
x07 pkg pack --package . --out dist/ext-hello-demo-0.1.0.x07pkg
```

## Step 6: publish to the official registry

The official index URL is:

- `sparse+https://registry.x07.io/index/`

### 6.1 Login

Prefer stdin so tokens don’t end up in shell history:

```bash
printf '%s' "$X07_TOKEN" | x07 pkg login --index sparse+https://registry.x07.io/index/ --token-stdin
```

### 6.2 Publish

```bash
x07 pkg publish --index sparse+https://registry.x07.io/index/ --package .
```

## Step 7: consume the package (end-to-end verification)

In a fresh directory:

```bash
mkdir -p /tmp/consume-hello && cd /tmp/consume-hello
x07 init
x07 pkg add ext-hello-demo@VERSION --sync
```

Then import `ext.hello` and call `ext.hello.hello_v1`.
