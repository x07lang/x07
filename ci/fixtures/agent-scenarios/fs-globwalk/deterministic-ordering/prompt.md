# Scenario: fs-globwalk/deterministic-ordering (OS world)

This project walks a small fixture tree and emits a sorted list of matching paths as UTF-8 text.

Goal:

- Run `x07 run --profile os` offline and verify the emitted path list is deterministic.

