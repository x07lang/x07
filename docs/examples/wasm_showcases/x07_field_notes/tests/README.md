`x07 Field Notes` keeps its behavioral coverage in deterministic web-ui trace fixtures under `tests/web_ui/`.

Recommended local loop:

```sh
x07 pkg lock --project frontend/x07.json
x07-wasm web-ui build --project frontend/x07.json --profile web_ui_debug --out-dir dist/showcase_mobile/web_ui_debug --clean --json
x07-wasm web-ui test --dist-dir dist/showcase_mobile/web_ui_debug --case tests/web_ui/notes_edit.trace.json --json
bash scripts/ci/check_showcase_mobile.sh
```

The trace set covers note editing, storage reload, successful sync, and sync error reporting.
