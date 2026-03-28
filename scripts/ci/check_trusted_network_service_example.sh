#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
exec ./check_trust_network_example.sh \
  --label trusted_network_service_v1 \
  --example-dir docs/examples/trusted_network_service_v1 \
  --template trusted-network-service \
  --profile arch/trust/profiles/trusted_program_sandboxed_net_v1.json \
  --entry example.main
