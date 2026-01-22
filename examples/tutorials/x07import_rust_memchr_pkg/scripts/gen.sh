#!/usr/bin/env bash
set -euo pipefail

mkdir -p modules/ext
x07import-cli rust \
  --in import_sources/rust/memchr.rs \
  --module-id ext.memchr \
  --out modules

