#!/usr/bin/env bash
set -euo pipefail

mkdir -p modules/ext
x07import-cli c \
  --in import_sources/c/smoke.c \
  --module-id ext.smoke \
  --out modules

