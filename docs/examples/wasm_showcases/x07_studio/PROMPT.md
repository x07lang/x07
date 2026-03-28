# `x07 Studio` Prompt

Implementation brief reproduced below.

```text
You are implementing a desktop device app using x07-device-host + x07-wasm device pipeline.

Goal: Build a desktop app called “x07 Studio”:
- Runs as a device bundle (web-ui reducer wasm + pinned host ABI).
- Uses std.web_ui.effects.storage for persistent data (projects, notes, settings).
- UI requirements:
  1) Left sidebar: projects list + search
  2) Main view: editable note (markdown-like plain text is ok)
  3) Tags + filter
  4) “Export project” button: generates a JSON export and triggers a download via a web-ui effect (use whatever mechanism exists; if not available, implement an “export view” with copy/paste).
  5) “Import project” flow: paste JSON import + validate.

Repo layout:
- frontend/ (x07 web-ui reducer project)
- arch/device/ (device profile registry + dev/release profiles)
- tests/ (at least 2 deterministic traces: create project, edit note, reload shows persistence; import/export)
- scripts/ci/check_showcase_desktop.sh

Hard requirements:
- Use ONLY the device pipeline commands:
  - x07-wasm device build
  - x07-wasm device verify
  - x07-wasm device provenance attest
  - x07-wasm device provenance verify
  - x07-wasm device package --target desktop
  - x07-wasm device run --target desktop --headless-smoke (for CI smoke)
- Every command invocation uses --json --report-out --quiet-json.
- No external validators; rely on x07-wasm device verify + provenance verify.
- CI script must be fully deterministic and produce stable outputs.

Use examples/device_min and x07-web-ui examples/web_ui_form as references.
```
