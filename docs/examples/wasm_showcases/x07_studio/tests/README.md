`x07 Studio` keeps its reducer-semantic checks under `frontend/tests/tests.json` and reserves the top-level CI script for the device-bundle pipeline.

Recommended local loop:

```sh
x07 pkg lock --project frontend/x07.json
x07 test --manifest frontend/tests/tests.json
bash scripts/ci/check_showcase_desktop.sh
```

The reducer tests cover storage bootstrap, storage hydrate, note persistence, export, valid import, and invalid import handling.
