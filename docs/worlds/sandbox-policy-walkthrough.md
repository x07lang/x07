# Sandbox policy walkthrough (run-os-sandboxed)

This tutorial is a practical workflow for building an OS-world X07 program under a **policy-gated sandbox** (`run-os-sandboxed`):

- start from a minimal base policy
- run the program and observe what the policy blocks
- grant the smallest additional capability required
- keep policy changes explicit and reviewable

`run-os-sandboxed` is policy enforcement + limits. It is not a hardened sandbox against hostile code. See [OS worlds](os-worlds.md).

## What you will do

You will start from `x07 init` and evolve the program through stages:

1. No OS capabilities (echo input).
2. Filesystem output (write `out/input.bin`).
3. HTTP fetch (requires a net-enabled base policy + explicit allowlist).
4. Iterate using `x07 run` policy override flags (`--allow-host`, `--deny-host`, `--allow-*-root`).

## Prerequisites

- Toolchain installed: `x07 --help`
- OS runner installed: `x07-os-runner --help`

For the HTTP stage:

- Run `x07 doctor` and install the suggested native prerequisites (libcurl/OpenSSL, C compiler, etc).

Companion example (in the `x07` repo):

- `examples/tutorials/sandbox_policy_walkthrough/`

## Step 0: create a project

```bash
mkdir sandbox-policy-demo
cd sandbox-policy-demo
x07 init
```

`x07 init` creates a `sandbox` profile in `x07.json` that points at:

- `.x07/policies/base/cli.sandbox.base.policy.json`

The profile exists immediately, but the policy file does not until you generate it.

## Step 1: generate the base policy (no networking)

Generate the `cli` base policy:

```bash
x07 policy init --template cli
```

Expected file created:

- `.x07/policies/base/cli.sandbox.base.policy.json`

## Step 2: run the default program (in the sandbox profile)

Create a small input:

```bash
printf 'hello' > /tmp/in.bin
```

Run:

```bash
x07 run --profile sandbox --input /tmp/in.bin
```

The OS runner prints a JSON report. The program’s output bytes are `solve_output_b64`. Decode it:

```bash
x07 run --profile sandbox --input /tmp/in.bin \
  | python3 -c 'import sys,json,base64; r=json.load(sys.stdin); sys.stdout.buffer.write(base64.b64decode(r["solve_output_b64"]))'
```

Expected bytes: `hello`.

## Step 3: add filesystem output (no policy changes)

Replace `src/app.x07.json` with:

```json
{
  "schema_version": "x07.x07ast@0.2.0",
  "kind": "module",
  "module_id": "app",
  "imports": ["std.os.fs"],
  "decls": [
    { "kind": "export", "names": ["app.solve"] },
    {
      "kind": "defn",
      "name": "app.solve",
      "params": [{ "name": "b", "ty": "bytes_view" }],
      "result": "bytes",
      "body": [
        "begin",
        ["let", "data", ["view.to_bytes", "b"]],
        ["let", "rc", ["std.os.fs.write_file", ["bytes.lit", "out/input.bin"], "data"]],
        ["if", ["=", "rc", 0], ["bytes.lit", "ok"], ["bytes.lit", "write_failed"]]
      ]
    }
  ]
}
```

Create `out/` (the base policy allows writing under `out`, but it does not create directories for you):

```bash
mkdir -p out
```

Run again:

```bash
x07 run --profile sandbox --input /tmp/in.bin
ls -la out/input.bin
```

## Step 4: add HTTP fetch (net-enabled base policy + allowlist)

### 4.1 Add the networking stack

`std.net.*` is provided by `ext-net`:

```bash
# Pick NAME@VERSION from the registry catalog (or the capability map for canonical picks).
x07 pkg add NAME@VERSION --sync
```

Use the capability map for the canonical set (it includes `ext-net` plus required helper packages). Do not rely on `meta.requires_packages` being present in every published package version.

### 4.2 Switch the base policy to `http-client`

The `cli` base policy has `net.enabled=false`, so `--allow-host` overrides will be rejected.

Generate a net-enabled base policy:

```bash
x07 policy init --template http-client
```

Edit `x07.json` and update only the sandbox profile policy path:

```json
{
  "profiles": {
    "sandbox": {
      "policy": ".x07/policies/base/http-client.sandbox.base.policy.json"
    }
  }
}
```

### 4.3 Replace the module with an HTTP GET program

Replace `src/app.x07.json` with:

```json
{
  "schema_version": "x07.x07ast@0.2.0",
  "kind": "module",
  "module_id": "app",
  "imports": ["std.fmt", "std.net.http", "std.net.http.spec", "std.os.fs"],
  "decls": [
    { "kind": "export", "names": ["app.solve"] },
    {
      "kind": "defn",
      "name": "app.solve",
      "params": [{ "name": "b", "ty": "bytes_view" }],
      "result": "bytes",
      "body": [
        "begin",
        ["let", "url", ["view.to_bytes", "b"]],
        ["let", "caps", ["std.net.http.spec.caps_default_v1"]],
        ["let", "resp", ["std.net.http.get_v1", "url", "caps"]],
        ["let", "v", ["bytes.view", "resp"]],
        [
          "if",
          ["std.net.http.resp_is_err_v1", "v"],
          [
            "begin",
            ["let", "code", ["std.net.http.resp_err_code_v1", "v"]],
            ["let", "code_b", ["std.fmt.s32_to_dec", "code"]],
            ["bytes.concat", ["bytes.lit", "http_error_code="], "code_b"]
          ],
          [
            "begin",
            ["let", "body", ["std.net.http.resp_body_v1", "v"]],
            ["let", "_", ["std.os.fs.write_file", ["bytes.lit", "out/body.bin"], "body"]],
            "body"
          ]
        ]
      ]
    }
  ]
}
```

Run with an explicit allowlist:

```bash
printf 'https://example.com/' > /tmp/url.bin
mkdir -p out

x07 run --profile sandbox --input /tmp/url.bin --allow-host example.com:443
ls -la out/body.bin
```

## Step 5: iterate with policy override flags (without editing JSON)

### Allow/deny hosts

Allow multiple ports with a comma-separated list:

```bash
x07 run --profile sandbox --input /tmp/url.bin --allow-host example.com:80,443
```

Deny wins after merge:

```bash
x07 run --profile sandbox --input /tmp/url.bin \
  --allow-host example.com:80,443 \
  --deny-host example.com:80
```

Deny all ports for a host:

```bash
x07 run --profile sandbox --input /tmp/url.bin \
  --allow-host example.com:443 \
  --deny-host example.com:*
```

### Keep allowlists in files (agent-friendly)

`.x07/allow-hosts.dev.txt`:

```text
example.com:443
example.com:80
```

Run:

```bash
x07 run --profile sandbox --input /tmp/url.bin --allow-host-file .x07/allow-hosts.dev.txt
```

### Temporarily extend filesystem roots

The base templates are intentionally conservative. You can extend roots for a one-off run:

```bash
mkdir -p tmp
x07 run --profile sandbox --input /tmp/url.bin --allow-write-root tmp
```

Notes:

- `--allow-host*` requires `net.enabled=true` in the base policy.
- `--allow-*-root` requires `fs.enabled=true` in the base policy.
- Overrides materialize a derived policy under `.x07/policies/_generated/` (inspect it and promote changes into a committed base policy when they become permanent).

## Step 6: when you need env vars, edit the base policy

There are no `x07 run` CLI overrides for env allowlists. If you want to read environment variables:

1. Edit the base policy file.
2. Set `env.enabled=true`.
3. Add specific keys to `env.allow_keys`.

See also: [CLI](../toolchain/cli.md) (policy flags) and [Networking](../guides/networking.md).
