# XTAL certification targets

XTAL projects often describe **multiple public operations** in `spec/*.x07spec.json`, but certification is always scoped to a **selected entry symbol**.

This page explains how the relevant knobs fit together so you can pick a clear “verify many, certify one” or “verify + certify all” posture.

## The three surfaces

### 1) Spec operations (`spec/*.x07spec.json`)

- `operations[*].name` are the public operations XTAL treats as the spec surface.
- `x07 xtal verify` runs verification for **every** declared operation entry.

### 2) XTAL manifest entrypoints (`arch/xtal/xtal.json`)

- `entrypoints[]` is the list of **certification targets**.
- `x07 xtal certify` selects from `entrypoints[]` (with `--entry` or `--all`) and runs `x07 trust certify` for each selected entry.

### 3) Project operational entry (`x07.json`)

- `project.operational_entry_symbol` identifies the operational entry that is treated as the shipped “root” for strong trust claims.
- Strong trust profiles require `x07 trust certify --entry` to match `project.operational_entry_symbol` (the certificate is about the operational entry, not a proof-only helper).

## Trust profile entrypoints (`arch/trust/profiles/*.json`)

Trust profiles have their own `entrypoints[]` allowlist.

Rules of thumb:

- If `--entry` is not listed in the profile, certification is rejected.
- Strong profiles additionally require the chosen `--entry` to match `project.operational_entry_symbol`.

## Common patterns

### Single operational entry (executable-style)

- Spec: one operation.
- `arch/xtal/xtal.json`: one entrypoint (same symbol).
- `x07.json`: `operational_entry_symbol` matches it.
- Trust profile `entrypoints[]` contains it.

### Verify many, certify one (library-style)

Use this when you want the spec surface to include multiple operations, but you want certification to center on a single operational entry symbol.

- Spec: multiple operations.
- `x07 xtal verify`: verifies all operations.
- `arch/xtal/xtal.json entrypoints[]`: selects one operational entry (or a small subset).
- `x07.json operational_entry_symbol`: matches the entry you certify.

Concrete example: `docs/examples/agent-gate/xtal/workflow-graph/`.

### Certify all entrypoints (multi-target)

Use this when each public operation should have its own certificate bundle:

- Add each operation symbol you intend to certify to `arch/xtal/xtal.json entrypoints[]`.
- Run `x07 xtal certify --all`.

This produces a separate `target/xtal/cert/<entry>/...` directory per certified entry.

