# x07cli Semantic Validator v1

This document is **normative** for `x07cli.specrows@0.1.0` beyond the JSON Schema (`spec/x07cli.specrows.schema.json`).

It defines:

- deterministic semantic validation rules (errors + warnings),
- deterministic canonicalization (“fmt”) rules,
- and the implied defaults (`help` / `version`) behavior.

The intent is **LLM-first** authoring: an agent should be able to generate a `cli.specrows.json`, run a formatter, and always get a stable canonical representation + deterministic diagnostics.

---

## Glossary

- **SpecRows**: the JSON representation of CLI specs. Primary authoring format.
- **Row**: one entry in `rows`, represented as a JSON array.
- **Scope**: `"root"` or `"root.<subcmd>.<subcmd>"` (dot-separated).
- **Short option**: `"-x"` (exactly 2 bytes: `-` and 1 ASCII letter).
- **Long option**: `"--name"` (ASCII `[a-z][a-z0-9-]*`).

---

## Required behavior

### 1) Validation is deterministic

Given the same input JSON bytes:

- the validator MUST produce identical diagnostics (same codes, same ordering, same rendered messages),
- and the formatter MUST produce identical canonical JSON bytes.

No timestamps, no host paths, no non-deterministic hashing.

---

## Semantic validation rules

### A) Row shape and required fields

Even if JSON Schema passes, the validator MUST enforce:

1. **Row kind must be known**: `about`, `help`, `version`, `flag`, `opt`, `arg`.
2. **Flag rows require at least one of short/long**:
   - invalid if `shortOpt==""` AND `longOpt==""`.
3. **Opt rows require at least one of short/long**:
   - invalid if `shortOpt==""` AND `longOpt==""`.
4. **Arg rows require non-empty `POS_NAME` and `key`** (schema already restricts, but keep for clarity).

Diagnostics:
- `ECLI_ROW_KIND_UNKNOWN`
- `ECLI_FLAG_NO_NAMES`
- `ECLI_OPT_NO_NAMES`

---

### B) Uniqueness and conflicts per scope

Within each **scope**:

- `shortOpt` values must be unique across `{flag,opt,help,version}` (ignoring empty short).
- `longOpt` values must be unique across `{flag,opt,help,version}` (ignoring empty long).
- `key` must be unique across `{flag,opt,arg}`.

Reserved:
- `--help` and `--version` are RESERVED long options.
- `-h` and `-V` are RESERVED short options.
- They may only appear on `help` / `version` rows respectively.

Diagnostics:
- `ECLI_DUP_SHORT`
- `ECLI_DUP_LONG`
- `ECLI_DUP_KEY`
- `ECLI_RESERVED_HELP_USED`
- `ECLI_RESERVED_VERSION_USED`

---

### C) Arg ordering constraints

Within each scope:

- Positional `arg` rows define parse order.
- At most one arg may be `"multiple": true` (metaArg.multiple). If present, it MUST be the final arg.
- Once an arg has `"required": false`, no later arg may be `"required": true` (prevents ambiguous arity).

Diagnostics:
- `ECLI_ARG_MULTI_NOT_LAST`
- `ECLI_ARG_MULTI_DUP`
- `ECLI_ARG_REQUIRED_AFTER_OPTIONAL`

---

### D) Type/value constraints for options

For `opt` rows:

- `value_kind` MUST be one of:
  - `"STR"`, `"PATH"`, `"U32"`, `"I32"`, `"BYTES"`, `"BYTES_HEX"`
- If `metaOpt.default` is present, it MUST be parseable for the `value_kind`:
  - `U32` => ASCII decimal digits, value fits 0..2^32-1 (stored mod 2^32)
  - `I32` => optional leading `-`, digits, fits i32 range
  - `BYTES_HEX` => even-length hex; decoded bytes length <= metaOpt.max_len (if set)

Diagnostics:
- `ECLI_OPT_VALUE_KIND_UNKNOWN`
- `ECLI_OPT_DEFAULT_INVALID`

---

### E) About/help/version row constraints

Per scope:
- At most one `about` row.
- At most one `help` row.
- At most one `version` row.

Diagnostics:
- `ECLI_ABOUT_DUP`
- `ECLI_HELP_DUP`
- `ECLI_VERSION_DUP`

---

## Implied defaults: help/version

The compiler/formatter MUST behave as if these rows exist:

### Help (all scopes)

If a scope has no `help` row, insert:

```
[scope, "help", "-h", "--help", "Show help"]
```

If `-h` is already used by another valid row, insert help with **no short**:

```
[scope, "help", "", "--help", "Show help"]
```

But if `--help` is already used by a non-help row, that is an error (`ECLI_RESERVED_HELP_USED`).

### Version (root scope only)

If `root` has no `version` row, insert:

```
["root", "version", "-V", "--version", "Show version"]
```

If `-V` is used, insert version with **no short**:

```
["root", "version", "", "--version", "Show version"]
```

But if `--version` is used by a non-version row, that is an error (`ECLI_RESERVED_VERSION_USED`).

---

## Canonicalization (“fmt”) rules

Canonicalization MUST NOT change the meaning of positional args.
Therefore:

- `arg` rows keep their relative order as authored.
- All other row types may be re-ordered deterministically.

Per scope, canonical row ordering is:

1. `about`
2. `help`
3. `version` (root only; if present in non-root, keep it but order it after help)
4. `flag` rows sorted by `(longOpt, shortOpt, key)` (empty sorts last)
5. `opt` rows sorted by `(longOpt, shortOpt, key)` (empty sorts last)
6. `arg` rows in original order

Additional canonicalization steps:

- If a row includes `meta.key`, it MUST match the row `key`. If it mismatches: error `ECLI_META_KEY_MISMATCH`.
- Normalize `meta` objects:
  - drop unknown keys
  - sort object keys lexicographically for stable JSON
- Enforce stable JSON encoding:
  - UTF-8
  - `separators=(',',':')`
  - sort object keys

Diagnostics:
- `ECLI_META_KEY_MISMATCH`

---

## Notes / rationale

- `--` end-of-options is widely used by POSIX-style utilities, and many parsers treat it as “stop parsing options”.
  Your parser should support this because it’s a standard user expectation. (Reference: POSIX Utility Syntax Guidelines.) 

- Many ecosystems auto-insert help flags (e.g., Python's `argparse` adds `-h/--help` by default; Rust's `clap` provides auto help/version).
  x07cli mirrors that behavior, but does so deterministically and explicitly in the compiled spec.

- For parse failures in OS worlds, prefer a stable “usage error” exit code (many conventions use 2 or EX_USAGE=64).
  The pure-world parser should not exit; it should return a deterministic error record and let the OS adapter decide.

