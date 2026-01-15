# Phase H3 example: `std.fs.read` (fixture + OS)

This example demonstrates the Phase H3 binding rule:

- In deterministic worlds (`solve-fs` / `solve-full`), `std.world.fs` resolves to the fixture-backed adapter.
- In standalone worlds (`run-os` / `run-os-sandboxed`), `std.world.fs` resolves to the OS-backed adapter.

The program reads a file whose path is provided as the raw input bytes.

## Run in `solve-fs` (fixture filesystem)

```bash
printf 'hello.txt' > /tmp/in.bin
mkdir -p /tmp/fixture && printf 'hi\n' > /tmp/fixture/hello.txt

cargo run -p x07-host-runner -- \
  --program examples/h3/read_file_by_stdin.x07.json \
  --world solve-fs \
  --fixture-fs-dir /tmp/fixture \
  --input /tmp/in.bin
```

## Run in `run-os` (real filesystem)

```bash
printf 'README.md' > /tmp/in.bin

cargo run -p x07-os-runner -- \
  --program examples/h3/read_file_by_stdin.x07.json \
  --world run-os \
  --input /tmp/in.bin
```

## Run in `run-os-sandboxed` (policy restricted)

```bash
printf 'README.md' > /tmp/in.bin

cargo run -p x07-os-runner -- \
  --program examples/h3/read_file_by_stdin.x07.json \
  --world run-os-sandboxed \
  --policy examples/h3/run-os-policy.example.json \
  --input /tmp/in.bin
```
