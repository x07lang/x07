# x07 Field Notes Build Notes

This showcase now ships a real `x07 Field Notes` reducer and validates the same reducer across web-ui, desktop device-host smoke, and mobile packaging.

## What The App Does

- Local notes app with three persistent note slots
- Search over note title, body, and attachment URL
- Note editor with title, body, up to two checklist items, and one attachment URL
- Settings screen with `sync_path`, optional API token, and manual sync
- Local persistence under the storage key `field_notes.state.v1`

The reducer stores the local document as a serialized JSON string in storage. On reload it accepts the stored string form and rehydrates the working state document.

## One Command Validation

From the repo root:

```sh
bash scripts/ci/check_showcase_mobile.sh
```

That script runs:

- `x07 check --project frontend/x07.json`
- `x07 test --manifest frontend/tests/tests.json`
- `x07-wasm web-ui profile validate`
- `x07-wasm web-ui build`
- `x07-wasm web-ui test` for:
  - `tests/web_ui/notes_edit.trace.json`
  - `tests/web_ui/storage_reload.trace.json`
  - `tests/web_ui/sync_success.trace.json`
  - `tests/web_ui/sync_error.trace.json`
- `x07-wasm device build|verify|run` for `device_desktop_dev`
- `x07-wasm device build|verify|package` for `device_ios_dev`
- `x07-wasm device build|verify|package` for `device_android_dev`

## Generated Outputs

The validation flow produces:

- `dist/showcase_mobile/web_ui_debug`
- `dist/showcase_mobile/device_desktop_dev_bundle`
- `dist/showcase_mobile/device_ios_dev_bundle`
- `dist/showcase_mobile/device_ios_dev_package/ios_project`
- `dist/showcase_mobile/device_android_dev_bundle`
- `dist/showcase_mobile/device_android_dev_package/android_project`

Each packaged mobile project embeds:

- `bundle.manifest.json`
- `ui/reducer.wasm`
- local host assets: `index.html`, `bootstrap.js`, `main.mjs`, `app-host.mjs`

No runtime WASM download is required.

## Sync Contract

The settings screen stores a relative `sync_path`, not a full backend URL.

The actual network destination comes from the device profile:

- `arch/device/profiles/device_desktop_dev.json`
- `arch/device/profiles/device_ios_dev.json`
- `arch/device/profiles/device_android_dev.json`

Those profiles currently point at the placeholder backend `https://example.invalid` and keep the packaged showcase in a safe non-production state.

Before pointing the app at a real backend:

1. Edit the relevant device profile.
2. Set `backend.base_url` to the real host.
3. Keep `backend.allowed_hosts` aligned with that host.
4. Keep `capabilities.network.allow_hosts` aligned with the same host policy.
5. Re-run `bash scripts/ci/check_showcase_mobile.sh`.
6. Enter the matching `sync_path` and optional token in the app settings screen.

If the device profile and the in-app `sync_path` do not match, sync will fail by design.

## Manual Native Build

For iOS:

1. Run `bash scripts/ci/check_showcase_mobile.sh`.
2. Open `dist/showcase_mobile/device_ios_dev_package/ios_project/X07DeviceApp.xcodeproj`.
3. Set signing and team information.
4. Build for a simulator or device.

For Android:

1. Run `bash scripts/ci/check_showcase_mobile.sh`.
2. Open `dist/showcase_mobile/device_android_dev_package/android_project`.
3. Let Gradle sync.
4. Configure signing if needed.
5. Build for an emulator or device.

## Policy Constraint

The packaged app must continue to ship `ui/reducer.wasm` as an embedded asset. Shipping updated behavior requires rebuilding and redistributing the app bundle, not downloading replacement WASM at runtime.
