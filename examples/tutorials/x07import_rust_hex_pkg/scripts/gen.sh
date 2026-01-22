#!/usr/bin/env bash
set -euo pipefail

mkdir -p modules/ext
x07import-cli rust \
  --in import_sources/rust/hex.rs \
  --module-id ext.hex \
  --out modules
