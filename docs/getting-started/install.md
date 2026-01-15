# Install X07

X07 ships as a small toolchain with stable, JSON-first contracts:

- `x07` — test harness + helpers (`test`, `ast`, `pkg`)
- `x07c` — compiler + agent tooling (`fmt`, `lint`, `fix`, `apply-patch`, `lock`, `build`, `compile`)
- `x07-host-runner` — deterministic native runner (solve-* worlds)
- `x07-os-runner` — standalone runner (run-os* worlds)

## Supported platforms

X07 aims to be usable on:

- macOS (Apple Silicon + Intel)
- Linux (glibc + musl variants)
- Windows

See [platform support and smoke tests](../worlds/os-worlds.md#platform-support).

## Option A: install a prebuilt release

1. Download the latest X07 release artifact for your platform (from the project’s releases).
2. Unpack it.
3. Add the `bin/` directory to your `PATH`.

Verify:

- `x07 --help`
- `x07c --help`
- `x07-host-runner --help`

## Option B: build from source (Rust toolchain)

If you have `cargo` installed:

- `cargo build -p x07 -p x07c -p x07-host-runner --release`
- the resulting binaries will be in `target/release/`

Then verify with the same commands above.

## Troubleshooting

### “Permission denied” on macOS

If Gatekeeper blocks the binary:

- remove the quarantine attribute on the unpacked binaries (or sign them in your org).

### “C compiler not found”

X07 compiles to C and invokes a system C compiler.

Install:

- macOS: Xcode Command Line Tools
- Linux: `clang` or `gcc`
- Windows: MSVC Build Tools or clang-cl

### “I can run `x07` but `x07 run` fails”

Use `x07-host-runner --help` and ensure a working `cc` is available (override via `X07_CC`).
