# std_logic_0002

`bench_logic_bug.check_guard` should validate a true guard condition, but the comparison constant is incorrect.

Acceptance:

- `x07 test --manifest tests/tests.json --module-root modules` fails before patch.
- Applying the oracle patch makes the suite pass.
