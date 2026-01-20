# Contributing

X07 welcomes contributions, especially in:

- external packages and adapters
- smoke tests and cross-platform hardening
- documentation improvements
- performance regression coverage

## Contribution principles

- Every new bytes encoding needs a pinned v1 doc and helpers.
- Every new module needs a smoke harness.
- Every new tool output needs a schema and contract self-test.

## How to contribute

1. Fork the repo
2. Run `./scripts/ci/check_all.sh`
3. Submit a PR with:
   - clear intent
   - deterministic tests
   - pinned contracts where relevant
