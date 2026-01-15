#!/usr/bin/env bash
set -euo pipefail

# CI entrypoint: build the native ext-db backends and run DB smoke suites.

repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

root="$(repo_root)"
cd "$root"

./scripts/ci/check_tools.sh >/dev/null

python_bin="${X07_PYTHON:-}"
if [[ -z "${python_bin}" ]]; then
  if [[ -x ".venv/bin/python" ]]; then
    python_bin=".venv/bin/python"
  elif command -v python3 >/dev/null 2>&1; then
    python_bin="python3"
  else
    python_bin="python"
  fi
fi

./scripts/build_ext_db_sqlite.sh >/dev/null
./scripts/build_ext_db_pg.sh >/dev/null
./scripts/build_ext_db_mysql.sh >/dev/null
./scripts/build_ext_db_redis.sh >/dev/null

cargo build -p x07-host-runner >/dev/null
cargo build -p x07-os-runner >/dev/null

pick_runner() {
  local env_override="$1"
  local bin_name="$2"

  if [[ -n "$env_override" ]]; then
    echo "$env_override"
    return 0
  fi

  local candidates=(
    "$root/target/debug/$bin_name"
    "$root/target/debug/$bin_name.exe"
    "$root/target/release/$bin_name"
    "$root/target/release/$bin_name.exe"
  )
  for c in "${candidates[@]}"; do
    if [[ -x "$c" ]]; then
      echo "$c"
      return 0
    fi
  done

  echo "" >&2
  echo "ERROR: $bin_name not found under target/{debug,release}/ (set env override)" >&2
  echo "Tried:" >&2
  for c in "${candidates[@]}"; do
    echo "  $c" >&2
  done
  return 2
}

X07_HOST_RUNNER="$(pick_runner "${X07_HOST_RUNNER:-}" "x07-host-runner")"
X07_OS_RUNNER="$(pick_runner "${X07_OS_RUNNER:-}" "x07-os-runner")"

DB_CORE_ROOT="${X07_EXT_DB_CORE_MODULE_ROOT:-packages/ext/x07-ext-db-core/0.1.0/modules}"
DB_SQLITE_ROOT="${X07_EXT_DB_SQLITE_MODULE_ROOT:-packages/ext/x07-ext-db-sqlite/0.1.0/modules}"
DB_PG_ROOT="${X07_EXT_DB_PG_MODULE_ROOT:-packages/ext/x07-ext-db-postgres/0.1.0/modules}"
DB_MYSQL_ROOT="${X07_EXT_DB_MYSQL_MODULE_ROOT:-packages/ext/x07-ext-db-mysql/0.1.0/modules}"
DB_REDIS_ROOT="${X07_EXT_DB_REDIS_MODULE_ROOT:-packages/ext/x07-ext-db-redis/0.1.0/modules}"
DATA_MODEL_ROOT="${X07_EXT_DATA_MODEL_MODULE_ROOT:-packages/ext/x07-ext-data-model/0.1.0/modules}"
HEX_ROOT="${X07_EXT_HEX_MODULE_ROOT:-packages/ext/x07-ext-hex-rs/0.1.0/modules}"

for r in "$DB_CORE_ROOT" "$DB_SQLITE_ROOT" "$DB_PG_ROOT" "$DB_MYSQL_ROOT" "$DB_REDIS_ROOT" "$DATA_MODEL_ROOT" "$HEX_ROOT"; do
  if [[ ! -d "$r" ]]; then
    echo "ERROR: module root not found at $r" >&2
    exit 2
  fi
done

run_suite() {
  local suite="$1"
  echo "[db-smoke] running suite: $suite"
  "$python_bin" scripts/ci/run_smoke_suite.py \
    --suite "$suite" \
    --host-runner "$X07_HOST_RUNNER" \
    --os-runner "$X07_OS_RUNNER" \
    --module-root "$DB_CORE_ROOT" \
    --module-root "$DB_SQLITE_ROOT" \
    --module-root "$DB_PG_ROOT" \
    --module-root "$DB_MYSQL_ROOT" \
    --module-root "$DB_REDIS_ROOT" \
    --module-root "$DATA_MODEL_ROOT" \
    --module-root "$HEX_ROOT"
}

run_suite "benchmarks/smoke/db-sqlite-os-sandboxed-smoke.json"
run_suite "benchmarks/smoke/db-sqlite-os-sandboxed-policy-deny-smoke.json"
run_suite "benchmarks/smoke/db-pool-fairness-os-sandboxed-smoke.json"
run_suite "benchmarks/smoke/db-pool-max-concurrency-os-sandboxed-smoke.json"
run_suite "benchmarks/smoke/db-pool-no-leak-close-os-sandboxed-smoke.json"

if [[ "${X07_DB_NETWORK_SMOKE:-}" == "1" ]]; then
  run_suite "benchmarks/smoke/db-pg-os-sandboxed-smoke.json"
  run_suite "benchmarks/smoke/db-mysql-os-sandboxed-smoke.json"
  run_suite "benchmarks/smoke/db-redis-os-sandboxed-smoke.json"
else
  echo "[db-smoke] skipping network DB smokes (set X07_DB_NETWORK_SMOKE=1)"
fi

echo "[db-smoke] OK"
