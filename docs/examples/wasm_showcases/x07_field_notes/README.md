# `x07 Field Notes`

This docs copy mirrors the reducer source, device profiles, and replay traces for agent discovery. The canonical runnable example, CI script, and generated mobile projects live in `x07-wasm-backend/examples/x07_field_notes/`.

Mobile and desktop packaging showcase for a single `std.web_ui` reducer. The app keeps notes, checklist items, attachment URLs, and sync settings in local storage, then proves the same reducer through web-ui replay, desktop smoke, and iOS/Android project generation.

- Prompt: [`PROMPT.md`](PROMPT.md)
- Mobile build notes: [`docs/MOBILE_BUILD.md`](docs/MOBILE_BUILD.md)

## What It Demonstrates

- Offline-first notes via `std.web_ui.effects.storage`
- Manual sync via `std.web_ui.effects.http`
- Deterministic web-ui trace replay before packaging
- Desktop smoke plus iOS and Android project generation from the same reducer
- Store-safe packaging with embedded `ui/reducer.wasm` and local host assets only

## Run With Released Tools

Install the required components once:

```sh
x07up component add wasm
x07up component add device-host
```

Run the full showcase gate from the canonical example directory in `x07-wasm-backend/examples/x07_field_notes/`:

```sh
bash scripts/ci/check_showcase_mobile.sh
```

Focused reducer replay:

```sh
x07 pkg lock --project frontend/x07.json
x07-wasm web-ui build --project frontend/x07.json --profile web_ui_debug --out-dir dist/showcase_mobile/web_ui_debug --clean --json
x07-wasm web-ui test --dist-dir dist/showcase_mobile/web_ui_debug --case tests/web_ui/notes_edit.trace.json --json
```

## Run From The Workspace

```sh
PATH="<workspace>/x07/target/debug:<workspace>/x07-wasm-backend/target/debug:<workspace>/x07-device-host/target/debug:$PATH" \
  X07_DEVICE_HOST_DESKTOP="<workspace>/x07-device-host/target/debug/x07-device-host-desktop" \
  bash scripts/ci/check_showcase_mobile.sh
```

## Files To Start With

- Device profiles: [`arch/device/profiles/device_desktop_dev.json`](arch/device/profiles/device_desktop_dev.json), [`arch/device/profiles/device_ios_dev.json`](arch/device/profiles/device_ios_dev.json), [`arch/device/profiles/device_android_dev.json`](arch/device/profiles/device_android_dev.json)
- Reducer source: [`frontend/src/app.x07.json`](frontend/src/app.x07.json)
- Trace fixtures: [`tests/web_ui/notes_edit.trace.json`](tests/web_ui/notes_edit.trace.json), [`tests/web_ui/storage_reload.trace.json`](tests/web_ui/storage_reload.trace.json), [`tests/web_ui/sync_success.trace.json`](tests/web_ui/sync_success.trace.json)
- Validation notes: [`tests/README.md`](tests/README.md)
