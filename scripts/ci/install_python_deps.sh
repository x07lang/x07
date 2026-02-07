#!/usr/bin/env bash
set -euo pipefail

python="${X07_PYTHON:-python3}"

"$python" -m pip install --upgrade pip
"$python" -m pip install 'jsonschema==4.25.1'

