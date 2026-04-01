# Website docs sync

`x07` is the source of truth for toolchain docs (`x07/docs/`). The `x07lang/x07-website` repo vendors a copy of those docs under `x07-website/docs/` and generates Docusaurus inputs under `x07-website/site/`.

This page documents two supported sync paths:

1) release-based sync (preferred for tagged toolchain releases)
2) docs-only sync (when you want to update the website docs without cutting a new `x07` release)

## Release-based sync (tagged toolchain release)

Use this path when you have a toolchain tag `vX.Y.Z` and want the website to reflect the released bundle.

1) Make doc changes in `x07/docs/`.

2) Make sure any **new** docs files are tracked by git before building the bundle:

```sh
git add docs/
```

`python3 scripts/build_docs_bundle.py` uses `git ls-files` when possible, so untracked files are not included in the bundle.

3) Build the docs bundle:

```sh
python3 scripts/build_docs_bundle.py --tag vX.Y.Z
```

4) Sync the website (includes docs + agent portal + installer assets):

```sh
cd ../x07-website
python3 ../x07/scripts/open_pr_website_update.py --tag vX.Y.Z --bundle ../x07/dist/x07-docs-vX.Y.Z.tar.gz
python3 scripts/site/gen_docusaurus_inputs.py
python3 scripts/check_site.py --check
```

## Docs-only sync (no new `x07` release)

Use this path when you want to update the website docs from the current `x07/docs/` tree without creating a new toolchain tag.

1) Make doc changes in `x07/docs/`. If you added new files, stage them so they are tracked:

```sh
git add docs/
```

2) Sync docs into `x07-website`:

```sh
cd ../x07-website
python3 scripts/sync_docs_from_toolchain.py --toolchain-repo ../x07
```

Notes:
- The website repo’s `scripts/check_site.py --check` requires `docs/latest/` to be byte-identical to `docs/v<latest_toolchain_version>/`, so the docs-only sync updates both.
- Website-only overlays under `x07-website/docs/_overlays/` are applied during sync.

3) Run the website checks:

```sh
python3 scripts/check_site.py --check
bash scripts/ci/check_all.sh
```

