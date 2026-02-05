# Sandbox policy walkthrough (companion example)

This directory is a companion to the docs page:

- `docs/worlds/sandbox-policy-walkthrough.md`

Suggested flow:

1. Generate base policies:
   - `x07 policy init --template cli`
   - `x07 policy init --template http-client`
2. Run stages by copying a stage file to `src/app.x07.json`:
   - `src/app.x07.json` (echo input)
   - `src/app.step2_fs_write.x07.json` (write `out/input.bin`)
   - `src/app.step3_http_fetch.x07.json` (HTTP GET; requires `ext-net` + allowlist)

