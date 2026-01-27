# Embedding in C (`x07 build`)

`x07 build` compiles a project to a single C file. You can use the output either as:

- a runner executable (generated `main()` with framed stdin/stdout), or
- an embeddable library entry point (`x07_solve_v2`) in freestanding mode.

For end-user distribution (a normal CLI binary that runs without the X07 toolchain installed), use `x07 bundle`.

## Generate C

```bash
x07 build --project x07.json --out build/program.c
```

Optional: emit a C header:

```bash
x07 build --project x07.json --emit-c-header build/x07.h
```

## Freestanding mode (library embedding)

Freestanding mode generates a C file that exports `x07_solve_v2`:

```bash
x07 build --project x07.json --out build/program.c --emit-c-header build/x07.h --freestanding
```

The header declares:

```c
bytes_t x07_solve_v2(
    uint8_t* arena_mem,
    uint32_t arena_cap,
    const uint8_t* input_ptr,
    uint32_t input_len
);
```

Usage model:

- You provide an arena buffer (`arena_mem`, `arena_cap`).
- `x07_solve_v2` runs `solve(input_bytes) -> bytes`.
- The returned `bytes_t` points into the arena buffer.

Example `test_embed.c`:

```c
#include "x07.h"

#include <stdint.h>
#include <stdio.h>

int main(void) {
  static uint8_t arena[64 * 1024 * 1024];
  const uint8_t input[] = {'h', 'i', '\n'};
  bytes_t out = x07_solve_v2(
      arena,
      (uint32_t)sizeof(arena),
      input,
      (uint32_t)sizeof(input)
  );
  (void)fwrite(out.ptr, 1, out.len, stdout);
  return 0;
}
```

Build and run:

```bash
clang -O2 -Ibuild -o test_embed test_embed.c build/program.c
./test_embed
```

## Runner executable mode (framed I/O)

The default (non-freestanding) output includes a `main()` that uses the standard X07 executable framing:

- stdin: `u32_le(len)` then `len` bytes
- stdout: `u32_le(len)` then `len` bytes

If you pipe raw text into the executable, it will interpret the first 4 bytes as the length prefix (which can look like an “out of memory” failure).

See: [Running programs](running-programs.md#executable-io-framing-advanced).
