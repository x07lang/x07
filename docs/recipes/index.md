# Recipes (patch-based, runnable)

These recipes are designed for coding agents: start from `x07 init`, apply a small RFC 6902 JSON Patch, then run via `x07 run`.

Each recipe assumes the default `x07 init` skeleton:

- `src/app.x07.json` exports `app.solve` at JSON Pointer `/decls/1/body`
- `src/main.x07.json` calls `app.solve` under JSON Pointer `/solve`
- `x07.json.world` is `run-os` and `x07.json.default_profile` is `os`

For reference, these patches are also checked into the toolchain repo under `docs/recipes/patches/`.

## How to use a recipe

```bash
x07 init
cat > /tmp/patch.json <<'JSON'
[{"op":"replace","path":"/decls/1/body","value":["bytes.lit","example"]}]
JSON
x07 ast apply-patch --in src/app.x07.json --patch /tmp/patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json
x07 run
```

`x07 run` prints a JSON report to stdout; the program’s output bytes are in `solve_output_b64` (base64).

## Recipes

### 1) Hello output (bytes-in/bytes-out)

This rewrites `app.solve` to ignore input and return a fixed bytes literal.

```bash
cat > /tmp/x07-hello-output.patch.json <<'JSON'
[{"op":"replace","path":"/decls/1/body","value":["bytes.lit","Hello_from_X07"]}]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-hello-output.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json
x07 run
```

Expected: `solve_output_b64` decodes to `Hello_from_X07`.

Common failures:

- If you see `X07INIT_EXISTS`, run in an empty directory (or delete the existing `x07.json`/`src/` skeleton first).

### 2) Prefix input bytes (bytes-in/bytes-out)

This uses `bytes.concat` to return `prefix + input`.

```bash
cat > /tmp/x07-prefix-input.patch.json <<'JSON'
[{"op":"replace","path":"/decls/1/body","value":["bytes.concat",["bytes.lit","prefix:"],["view.to_bytes","b"]]}]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-prefix-input.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json

printf 'hello' > /tmp/input.bin
x07 run --input /tmp/input.bin
```

Expected: `solve_output_b64` decodes to `prefix:hello`.

### 3) Parse newline-separated fields (bytes-in/bytes-out)

This returns the *second* line of input (`k=1`) using `std.text.ascii.kth_line_view`.

```bash
cat > /tmp/x07-kth-line.patch.json <<'JSON'
[
  {"op":"add","path":"/imports/0","value":"std.text.ascii"},
  {"op":"replace","path":"/decls/1/body","value":["view.to_bytes",["std.text.ascii.kth_line_view","b",1]]}
]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-kth-line.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json

cat > /tmp/input.txt <<'TXT'
first
second
TXT
x07 run --input /tmp/input.txt
```

Expected: `solve_output_b64` decodes to `second`.

### 4) Parse CLI args via `ext-cli` (argv_v1 via `x07 run -- ...`)

This uses `ext.cli.parse_specrows` to parse `argv_v1`. Run it with `x07 run -- ...` so the toolchain encodes `argv_v1` for you.

```bash
# Pick NAME@VERSION from the registry catalog (or the capability map for canonical picks).
x07 pkg add NAME@VERSION --sync

cat > /tmp/x07-ext-cli-name.patch.json <<'JSON'
[
  {"op":"add","path":"/imports/0","value":"ext.cli"},
  {"op":"replace","path":"/decls/1/body","value":["begin",["let","spec",["bytes.lit","{\\\"schema_version\\\":\\\"x07cli.specrows@0.1.0\\\",\\\"app\\\":{\\\"name\\\":\\\"tool\\\",\\\"version\\\":\\\"0.1.0\\\"},\\\"rows\\\":[[\\\"root\\\",\\\"opt\\\",\\\"\\\",\\\"--name\\\",\\\"name\\\",\\\"NAME\\\",\\\"\\\",{\\\"required\\\":true}]]}"]],["let","argv_v1",["view.to_bytes","b"]],["let","doc",["ext.cli.parse_specrows","spec","argv_v1"]],["if",["=",["ext.cli.is_ok",["bytes.view","doc"]],0],["return",["ext.cli.err_usage",["bytes.view","doc"]]],0],["let","name",["option_bytes.unwrap_or",["ext.cli.matches_get",["bytes.view","doc"],["bytes.view",["bytes.lit","name"]],2],["bytes.alloc",0]]],["bytes.concat",["bytes.lit","name="],"name"]]}
]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-ext-cli-name.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json

x07 run -- tool --name Alice
```

Expected: `solve_output_b64` decodes to `name=Alice`.

Common failures:

- If you see a package error while locking, re-run `x07 pkg lock --project x07.json` (or use `--sync` as shown).

### 5) Find a substring (bytes-in/bytes-out)

This finds the first index of `needle` in the input and returns the index as decimal (or `-1`).

```bash
cat > /tmp/x07-find-literal.patch.json <<'JSON'
[
  {"op":"add","path":"/imports/0","value":"std.regex-lite"},
  {"op":"add","path":"/imports/1","value":"std.fmt"},
  {"op":"replace","path":"/decls/1/body","value":["std.fmt.s32_to_dec",["std.regex-lite.find_literal","b",["bytes.view",["bytes.lit","needle"]]]]}
]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-find-literal.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json

printf 'hayneedlehay' > /tmp/input.bin
x07 run --input /tmp/input.bin
```

Expected: `solve_output_b64` decodes to `3`.

### 6) Canonicalize JSON (bytes-in/bytes-out)

This returns a canonical JSON encoding for a small JSON document.

```bash
cat > /tmp/x07-json-canon.patch.json <<'JSON'
[
  {"op":"add","path":"/imports/0","value":"std.json"},
  {"op":"replace","path":"/decls/1/body","value":["std.json.canonicalize_small","b"]}
]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-json-canon.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json

printf '{\"b\":2,\"a\":1}' > /tmp/input.json
x07 run --input /tmp/input.json
```

Expected: `solve_output_b64` decodes to `{\"a\":1,\"b\":2}`.

### 7) Async task + await (bytes-in/bytes-out)

This adds a `defasync` and uses `task.spawn` + `await`.

```bash
cat > /tmp/x07-async-hello.patch.json <<'JSON'
[
  {"op":"add","path":"/decls/0/names/-","value":"app.hello_task"},
  {"op":"add","path":"/decls/-","value":{"kind":"defasync","name":"app.hello_task","params":[],"result":"bytes","body":["bytes.lit","hello_from_task"]}}
]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-async-hello.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json

cat > /tmp/x07-async-hello.main.patch.json <<'JSON'
[
  {"op":"replace","path":"/solve","value":["begin",["let","h",["app.hello_task"]],["task.spawn","h"],["await","h"]]}
]
JSON

x07 ast apply-patch --in src/main.x07.json --patch /tmp/x07-async-hello.main.patch.json --out src/main.x07.json --validate
x07 fmt --input src/main.x07.json --write
x07 lint --input src/main.x07.json
x07 run
```

Expected: `solve_output_b64` decodes to `hello_from_task`.

### 8) Write a file (run-os-sandboxed)

This writes `out/hello.txt` under the default sandbox profile’s base policy.

```bash
x07 policy init --template cli
mkdir -p out

cat > /tmp/x07-sandbox-write-file.patch.json <<'JSON'
[
  {"op":"add","path":"/imports/-","value":"std.os.fs"},
  {"op":"replace","path":"/decls/1/body","value":["begin",["std.os.fs.write_file",["bytes.lit","out/hello.txt"],["bytes.lit","Hello_from_sandbox"]],["bytes.lit","ok"]]}
]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-sandbox-write-file.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json

x07 run --profile sandbox
cat out/hello.txt
```

Expected:

- `out/hello.txt` contains `Hello_from_sandbox`
- `solve_output_b64` decodes to `ok`

Common failures:

- If you see `missing policy file`, run `x07 policy init --template cli` (it creates `.x07/policies/base/cli.sandbox.base.policy.json`).
- If you see policy denial for filesystem writes, ensure you’re writing under `out/` (or pass `--allow-write-root <path>`).

### 9) HTTP GET in a sandbox (run-os-sandboxed + `ext-net`)

This fetches `https://example.com/` using `std.net.http.get_v1`, writes the body to `out/example.html`, and returns `status=<code>`.

```bash
# Ensure native prerequisites for libcurl/OpenSSL are installed.
x07 doctor

# Add networking stack (transitive helper packages are auto-added at lock time).
# Pick NAME@VERSION from the registry catalog (or the capability map for canonical picks).
x07 pkg add NAME@VERSION --sync

# Generate a base deny-by-default HTTP client policy.
x07 policy init --template http-client
mkdir -p out

cat > /tmp/x07-sandbox-http-get.patch.json <<'JSON'
[
  {"op":"add","path":"/imports/-","value":"std.net.http"},
  {"op":"add","path":"/imports/-","value":"std.net.http.spec"},
  {"op":"add","path":"/imports/-","value":"std.fmt"},
  {"op":"add","path":"/imports/-","value":"std.os.fs"},
  {"op":"replace","path":"/decls/1/body","value":["begin",["let","url",["bytes.lit","https://example.com/"]],["let","caps",["std.net.http.spec.caps_default_v1"]],["let","resp",["std.net.http.get_v1","url","caps"]],["let","dv",["bytes.view","resp"]],["if",["std.net.http.resp_is_err_v1","dv"],["begin",["let","code",["std.net.http.resp_err_code_v1","dv"]],["let","code_b",["std.fmt.s32_to_dec","code"]],["bytes.concat",["bytes.lit","http_error_code="],"code_b"]],["begin",["let","status",["std.net.http.resp_status_v1","dv"]],["let","body",["std.net.http.resp_body_v1","dv"]],["std.os.fs.write_file",["bytes.lit","out/example.html"],"body"],["bytes.concat",["bytes.lit","status="],["std.fmt.u32_to_dec","status"]]]]}
]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-sandbox-http-get.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 lint --input src/app.x07.json

x07 run --profile sandbox \
  --policy .x07/policies/base/http-client.sandbox.base.policy.json \
  --allow-host example.com:443
```

Expected:

- `out/example.html` contains the fetched response body bytes
- `solve_output_b64` decodes to `status=200` (or another HTTP status)

Common failures:

- If you see C compile/link errors, run `x07up doctor --json` and install the suggested native deps.
- If you see C compile/link errors, run `x07 doctor` and install the suggested native deps.
- If you see a policy denial, ensure you passed `--allow-host example.com:443` (and that the policy is the `http-client` template).
