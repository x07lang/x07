# Contributing

## Code of Conduct

Participation in this project is governed by `CODE_OF_CONDUCT.md`.

## What changes require an RFC

See `governance/RFC-REQUIREMENTS.md`. RFCs are submitted to `x07lang/x07-rfcs`.

## Development workflow

- Prefer small PRs with a clear intent.
- Keep changes deterministic and reproducible.
- Add tests for behavior changes.

### Required checks

Run the canonical gate before opening a PR:

- `./scripts/ci/check_all.sh`

## DCO sign-off

All commits must be signed off using the Developer Certificate of Origin (DCO).

Use `git commit -s ...` or add a `Signed-off-by: Name <email>` trailer to each commit.

