#!/usr/bin/env bash
set -euo pipefail

export X07_NETWORK_EXAMPLE_MODE=static
cd "$(dirname "${BASH_SOURCE[0]}")"
exec ./check_certified_network_capsule_example.sh
