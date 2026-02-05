# Publish a package by example (companion example)

This directory is a companion to:

- `docs/packages/publishing-by-example.md`

It is a minimal, pure package that can be:

- tested via `x07 test --manifest tests/tests.json`
- packed via `x07 pkg pack --package . --out dist/<name>-<ver>.x07pkg`
- published via `x07 pkg publish --index sparse+https://registry.x07.io/index/ --package .`

