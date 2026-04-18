# MCP quality: install

Hardproof (`hardproof`) is a standalone verifier CLI. The public beta is distributed as prebuilt binaries from GitHub Releases.

Scan runs in the `hardproof` binary (no Node.js toolchain required). Use `hardproof doctor` to check your environment before you run verification in CI.

Hardproof runs as a **live verifier** on interactive terminals: `hardproof scan` streams a structured event log (`scan.events.jsonl`) during the run and writes a stable report (`scan.json`) at the end.

## Install from release artifacts

1) Download the release asset for your OS/arch from `x07lang/hardproof` GitHub Releases.

2) Extract the archive and place `hardproof` on your `PATH`.

3) Verify installation:

```sh
hardproof --help
hardproof doctor
```

## Quick sanity scan

Run a scan against a local HTTP MCP server:

```sh
hardproof scan --url "http://127.0.0.1:3000/mcp" --out out/scan --ui rich
```

Artifacts under `out/scan/`:

- `scan.json` (machine-readable scan report)
- `scan.events.jsonl` (event stream; also useful for CI log streaming)

Score truth semantics are explicit in the report:

- `score_mode=full` means `overall_score` is populated and eligible as a full score.
- `score_mode=partial` means the scan is not publishable (`score_truth_status=partial`). `overall_score` is still computed as the effective score (matching `partial_score`), and `gating_reasons` explain what evidence is missing (commonly Trust inputs).

Token/context usage truth is explicit under `scan.json.usage_metrics` (`requested_usage_mode`, `usage_status`, plus the effective `usage_mode`):

- `estimate` (deterministic estimates)
- `tokenizer_exact` (exact counts under `--tokenizer openai:o200k_base` / `openai:cl100k_base`)
- `trace_observed` (observed counts from `--token-trace <path>`)
- `mixed` (per-metric mix of exact + observed)

## Codespaces install helper

If you are evaluating via `x07lang/x07-mcp` Codespaces, the repo includes a helper that installs the verifier binary (defaults to the latest beta, or you can pass a specific tag):

```sh
./scripts/dev/install_hardproof.sh
```

Then run:

```sh
~/.local/bin/hardproof --help
```
