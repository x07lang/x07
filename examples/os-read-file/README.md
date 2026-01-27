# Example: `std.fs.read` in fixture worlds vs OS worlds

This example demonstrates that `std.fs.read` resolves through `std.world.fs`:

- In deterministic worlds (`solve-fs` / `solve-full`), `std.world.fs` resolves to the fixture-backed adapter.
- In OS worlds (`run-os` / `run-os-sandboxed`), `std.world.fs` resolves to the OS-backed adapter.

The program reads a file whose path is provided as the raw input bytes.

## Run in `solve-fs` (fixture filesystem)

```bash
printf 'hello.txt' > /tmp/in.bin
mkdir -p /tmp/fixture && printf 'hi\n' > /tmp/fixture/hello.txt

x07 run --repair=off \
  --program examples/os-read-file/read_file_by_stdin.x07.json \
  --world solve-fs \
  --fixture-fs-dir /tmp/fixture \
  --input /tmp/in.bin
```

## Run in `run-os` (real filesystem)

```bash
printf 'README.md' > /tmp/in.bin

x07 run --repair=off \
  --program examples/os-read-file/read_file_by_stdin.x07.json \
  --world run-os \
  --input /tmp/in.bin
```

## Run in `run-os-sandboxed` (policy restricted)

```bash
printf 'README.md' > /tmp/in.bin

x07 run --repair=off \
  --program examples/os-read-file/read_file_by_stdin.x07.json \
  --world run-os-sandboxed \
  --policy examples/os-read-file/run-os-policy.example.json \
  --input /tmp/in.bin
```
