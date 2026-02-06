# std_math_0001

`bench_math_bug.check_add` should pass when validating `2 + 2`, but the test currently expects the wrong value.

Acceptance:

- `x07 test --manifest tests/tests.json --module-root modules` fails before patch.
- Applying the oracle patch makes the suite pass.
