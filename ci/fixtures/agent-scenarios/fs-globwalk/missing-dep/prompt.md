# Scenario: fs-globwalk/missing-dep (self-repair, OS world)

You are given a broken X07 project snapshot.

Goal:

- The project fails to compile because it imports `std.os.fs.walk` but is missing `ext-path-glob-rs`.
- Make `x07 run --profile os` succeed deterministically.

Expected workflow:

1. Run `x07 run --profile os` and read the compile error.
2. Apply the suggested fix: `x07 pkg add ext-path-glob-rs@0.1.1 --sync`.
3. Re-run `x07 run --profile os` and verify it succeeds.

