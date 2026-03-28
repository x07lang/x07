# `x07 Field Notes` Prompt

Implementation brief reproduced below.

```text
You are implementing a mobile device app demo for both iOS and Android packaging templates.

Goal: Build “x07 Field Notes”:
- Same reducer works on desktop + mobile.
- Offline-first: uses std.web_ui.effects.storage for notes, checklists, and attachment metadata (attachments can be URLs only for MVP).
- Sync: uses std.web_ui.effects.http to sync notes to a backend URL if configured.
- UI:
  1) Notes list + search
  2) Create/edit note
  3) Settings screen: configure optional sync endpoint (URL) + API token (stored locally)
  4) Show last sync status + errors

Repo layout:
- frontend/ (x07 web-ui reducer project)
- arch/device/ (device profile registry + mobile profiles if needed)
- scripts/ci/check_showcase_mobile.sh

Hard requirements:
- Produce iOS and Android packages via:
  - x07-wasm device build
  - x07-wasm device verify
  - x07-wasm device package --target ios
  - x07-wasm device package --target android
- The generated templates MUST include only embedded local assets + reducer.wasm (no downloading new code at runtime).
- Provide manual build notes in docs/MOBILE_BUILD.md, but CI must at least generate the template outputs deterministically.

Important constraint:
- Do NOT design a system that downloads and executes new WASM code at runtime on iOS; iOS app review rules restrict downloading/executing code to change app behavior. Keep the WASM reducer shipped with the app bundle; updates go through app updates.

Use examples/device_min and x07-wasm-backend mobile templates as references.
```
