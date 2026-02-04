# Compiler limits

X07 intentionally enforces size limits during compilation to keep builds deterministic and resource-bounded.

If you hit these limits in a real project, first confirm you are not accidentally generating large modules (for example, embedding huge fixtures directly into `*.x07.json`). If the size is expected, raise the limits for the build that needs it.

## Defaults

The compiler currently enforces:

- Maximum source bytes per module: `65_536`
- Maximum AST nodes per module: `250_000`
- Maximum locals per function: `20_000`
- Maximum emitted C bytes (per compilation unit): `33_554_432`

## Overrides

You can override the defaults using environment variables:

- `X07_MAX_AST_NODES`
- `X07_MAX_LOCALS`
- `X07_MAX_C_BYTES`

Values must be positive integers. Invalid values fall back to the defaults.
