#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
exec ./check_trust_network_example.sh \
  --label certified_network_capsule_v1 \
  --example-dir docs/examples/certified_network_capsule_v1 \
  --template certified-network-capsule \
  --profile arch/trust/profiles/trusted_program_sandboxed_net_v1.json \
  --entry capsule.main
