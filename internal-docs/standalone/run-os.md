# run-os world (standalone-only)

`run-os` is a standalone-only capability world for X07 programs compiled to native C.

## Properties

- Real OS filesystem/network/time/env/process access (capability-scoped).
- Not deterministic (by design).
- Not used by deterministic suites.

## Contract with stdlib

Stdlib should be written against `std.io` traits and `std.world.*` adapters:

- `std.fs` imports `std.world.fs`
- `std.os.*` modules forward to `std.world.*` (standalone-only)

In deterministic suites, `std.world.*` resolves to fixture-backed adapters.
In standalone, it resolves to OS-backed adapters.

## Usage (`x07-os-runner`)

Compile+run a program:

```bash
cargo run -p x07-os-runner -- \
  --program examples/h3/read_file_by_stdin.x07.json \
  --world run-os \
  --input /tmp/in.bin
```

Run an already-compiled artifact:

```bash
cargo run -p x07-os-runner -- \
  --artifact target/app \
  --world run-os \
  --input /tmp/in.bin
```
