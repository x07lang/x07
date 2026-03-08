# `x07 Studio`

This docs copy mirrors the reducer source and device profiles for agent discovery. The canonical runnable example, CI script, and provenance material live in `x07-wasm-backend/examples/x07_studio/`.

Desktop device-app showcase for the `x07-wasm device` pipeline. The reducer keeps a searchable project list, editable note body, tag filtering, and import/export flows with persistent local storage.

- Prompt: [`PROMPT.md`](PROMPT.md)
- Reducer entry: [`frontend/src/app.x07.json`](frontend/src/app.x07.json)
- Reducer tests: `x07 test --manifest frontend/tests/tests.json`

## What It Demonstrates

- Desktop bundle build + verify + provenance + package + headless smoke
- Persistent state via `std.web_ui.effects.storage`
- Project search, note editing, tags, export, and import validation
- A copy/paste export path that works without a first-class download effect

## Run With Released Tools

Install the required components once:

```sh
x07up component add wasm
x07up component add device-host
```

Run the device showcase gate from the canonical example directory in `x07-wasm-backend/examples/x07_studio/`:

```sh
bash scripts/ci/check_showcase_desktop.sh
```

For reducer-semantic checks:

```sh
x07 pkg lock --project frontend/x07.json
x07 test --manifest frontend/tests/tests.json
```

## Run From The Workspace

```sh
PATH="<workspace>/x07/target/debug:<workspace>/x07-wasm-backend/target/debug:<workspace>/x07-device-host/target/debug:$PATH" \
  X07_DEVICE_HOST_DESKTOP="<workspace>/x07-device-host/target/debug/x07-device-host-desktop" \
  bash scripts/ci/check_showcase_desktop.sh
```

## Files To Start With

- Device profiles: [`arch/device/profiles/device_dev.json`](arch/device/profiles/device_dev.json), [`arch/device/profiles/device_release.json`](arch/device/profiles/device_release.json)
- Reducer source: [`frontend/src/app.x07.json`](frontend/src/app.x07.json)
- Reducer tests: [`frontend/tests/studio_tests.x07.json`](frontend/tests/studio_tests.x07.json)
- Verification notes: [`tests/README.md`](tests/README.md)
