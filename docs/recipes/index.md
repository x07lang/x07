# Recipes (patch-based, runnable)

These recipes are designed for coding agents: start from `x07 --init`, apply a small RFC 6902 JSON Patch, then run deterministically.

## How to use a recipe

```bash
x07 --init
cat > /tmp/patch.json <<'JSON'
[{"op":"replace","path":"/decls/1/body","value":["bytes.lit","example"]}]
JSON
x07 ast apply-patch --in src/app.x07.json --patch /tmp/patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
```

## Recipes

### 1) Hello output (pure)

This rewrites `app.solve` to ignore input and return a fixed bytes literal.

```bash
cat > /tmp/x07-hello-output.patch.json <<'JSON'
[{"op":"replace","path":"/decls/1/body","value":["bytes.lit","Hello from X07"]}]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-hello-output.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write
x07 run
```

### 2) Record → replay HTTP body (solve-rr)

Steps:

```bash
# Record a real HTTP response body into fixtures/rr/.
x07 rr record --out fixtures/rr example.com https://example.com

# Make the program read via solve-rr (std.rr.fetch).
cat > /tmp/x07-solve-rr-fetch.patch.json <<'JSON'
[{"op":"add","path":"/imports/0","value":"std.rr"},{"op":"replace","path":"/decls/1/body","value":["std.rr.fetch","b"]}]
JSON

x07 ast apply-patch --in src/app.x07.json --patch /tmp/x07-solve-rr-fetch.patch.json --out src/app.x07.json --validate
x07 fmt --input src/app.x07.json --write

# Run deterministically in solve-rr.
x07 run --world solve-rr --fixture-rr-dir fixtures/rr --input-b64 ZXhhbXBsZS5jb20=
```
