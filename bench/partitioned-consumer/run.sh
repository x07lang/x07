#!/usr/bin/env bash
set -euo pipefail

out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --out)
      out="${2:-}"
      shift 2
      ;;
    *)
      echo "usage: $0 --out <path>" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${out}" ]]; then
  echo "missing --out" >&2
  exit 2
fi

if [[ "${BENCH_DRY:-0}" == "1" ]]; then
  cat >"${out}" <<'JSON'
{
  "schema_version": "x07.service_bench.partitioned_consumer@0.1.0",
  "kind": "partitioned-consumer",
  "lag_samples": [],
  "catch_up_s": 0,
  "throughput_eps": 0
}
JSON
  echo "ok: wrote ${out}"
  exit 0
fi

echo "partitioned-consumer bench: real mode is not implemented in this repo; set BENCH_DRY=1" >&2
exit 2
