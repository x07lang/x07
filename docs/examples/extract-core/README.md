# Extract core example

This example demonstrates a minimal kernel/shell split:

- `src/kernel.x07.json`: deterministic domain logic
- `src/shell.x07.json`: parsing + shaping output

## Run (dry input)

With empty input, the shell uses a default request and prints a small JSON response.

```sh
x07 run --project x07.json --profile os
```

## Run (binary input)

The shell accepts 8 bytes:

- u32 `items_count` (LE)
- u32 `unit_price_cents` (LE)

```sh
python3 - <<'PY'
import struct, sys
sys.stdout.buffer.write(struct.pack("<II", 3, 1250))
PY > /tmp/in.bin

x07 run --project x07.json --profile os --input /tmp/in.bin
```

## Tests

```sh
x07 test --manifest tests/tests.json
```
