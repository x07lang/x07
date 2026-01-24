# Scenario: diff-patch/patch-v1 (golden patch bytes)

This project computes a `patch_v1` blob for a fixed `before`/`after` pair using `ext-diff-rs`.

Goal:

- Run `x07 test` offline and verify the patch computation and application logic is correct.
- Verify the program’s output exactly matches the committed golden `patch_v1.bin`.

