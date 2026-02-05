# Example: `std.fs.read` in OS worlds

This example reads a file whose path is provided as raw input bytes.

## Run in `run-os`

```bash
cd docs/examples/os-read-file
printf 'README.md' | x07 run --repair=off --project x07.json --profile os --stdin
```

## Run in `run-os-sandboxed` (policy restricted)

```bash
cd docs/examples/os-read-file
printf 'README.md' | x07 run --repair=off --project x07.json --profile sandbox --stdin
```
