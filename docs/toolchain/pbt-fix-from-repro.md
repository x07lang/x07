# Convert a PBT repro into a regression test (`x07 fix --from-pbt`)

When `x07 test --pbt` finds a counterexample, it writes a repro artifact (`repro.json`).
`x07 fix --from-pbt` turns that artifact into a committable, deterministic regression test by:

1. writing a tiny wrapper module that decodes `input` case bytes and calls the original property, and
2. appending a new `tests/tests.json` entry that feeds the minimized `input_b64`.

This lets you reproduce the failure with plain `x07 test` (no fuzz loop).

## Workflow

1. Run PBT to produce `repro.json`:

   ```bash
   x07 test --pbt --manifest tests/tests.json --keep-artifacts
   ```

2. Convert the repro into a regression test:

   ```bash
   x07 fix --from-pbt target/x07test/pbt/<...>/repro.json --write
   ```

3. Re-run unit tests (now includes the new deterministic repro test):

   ```bash
   x07 test --manifest tests/tests.json
   ```

Fix the bug, then `x07 test` should go green.

## What gets written

Given a repro with `counterexample.case_bytes_b64` decoded to `case_bytes`, `x07 fix --from-pbt` computes:

- `case_tag = "c" + sha256(case_bytes)[0..12]`

and writes:

- Wrapper module: `tests/repro/pbt/<case_tag>.x07.json`
  - module id: `repro.pbt.<case_tag>`
  - exported entry: `repro.pbt.<case_tag>.run`
- Copied repro (canonicalized): `tests/repro/pbt/<case_tag>.repro.json`
- Manifest entry appended to `tests/tests.json`:
  - `id`: `pbt_repro/<orig_test_id>/<case_tag>`
  - `entry`: `repro.pbt.<case_tag>.run`
  - `returns`: `bytes_status_v1`
  - `expect`: `pass`
  - `input_b64`: the repro’s `counterexample.case_bytes_b64`

If the wrapper/repro files already exist and are identical, `x07 fix --from-pbt` is a no-op.
If they exist but differ, it fails (never overwrites silently).

## Report output

`x07 fix --from-pbt` prints an `x07c.report@0.1.0` JSON to stdout with an info diagnostic:

- `code`: `X07-PBT-FIX-INFO-0001`
- `data`: written paths + ids + case hash metadata

## Supported parameter types (v0.1)

Wrapper generation currently supports:

- `i32`
- `bytes`
