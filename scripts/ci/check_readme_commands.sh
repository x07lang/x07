#!/usr/bin/env bash
set -euo pipefail

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
  else
    python_bin="python3"
  fi
fi

cargo build -p x07 -p x07-host-runner -p x07-os-runner >/dev/null

x07_bin="${X07_BIN:-}"
if [[ -z "${x07_bin}" ]]; then
  x07_bin="$(./scripts/ci/find_x07.sh)"
fi

bin_dir="$(cd "$(dirname "$x07_bin")" && pwd)"
export PATH="$bin_dir:$PATH"

tmp_dir="$(mktemp -t x07_readme_cmds_XXXXXX -d)"
cleanup() { rm -rf "$tmp_dir"; }
trap cleanup EXIT

cat >"$tmp_dir/program.x07.json" <<'JSON'
{
  "schema_version": "x07.x07ast@0.1.0",
  "kind": "entry",
  "module_id": "main",
  "imports": [],
  "decls": [],
  "solve": ["bytes.alloc", 0]
}
JSON

printf 'PING' >"$tmp_dir/input.bin"

cat >"$tmp_dir/patch.json" <<'JSON'
[
  {"op":"add","path":"/imports/-","value":"std.bytes"}
]
JSON

extract_readme_commands() {
  local readme_path="${root}/README.md"
  awk '
    BEGIN { section=""; in_block=0; capture_section=0; capture_block=0; }
    /^## / { capture_section=0; next; }
    /^# / { capture_section=0; next; }
    /^### / {
      section=$0;
      sub(/^### /,"",section);
      capture_section=(section=="Run a Program" || section=="Agent Tooling");
      next;
    }
    /^```/ {
      if (in_block==0) {
        in_block=1;
        capture_block=(capture_section && ($0=="```bash" || $0=="```sh" || $0=="```shell" || $0=="```"));
        next;
      } else {
        in_block=0;
        capture_block=0;
        next;
      }
    }
    {
      if (in_block && capture_block) print $0;
    }
  ' "$readme_path"
}

commands_file="$tmp_dir/commands.txt"
extract_readme_commands >"$commands_file"
if [[ ! -s "$commands_file" ]]; then
  echo "ERROR: no README commands extracted (expected sections: Run a Program, Agent Tooling)" >&2
  exit 1
fi

cd "$tmp_dir"

while IFS= read -r raw; do
  line="$(echo "$raw" | sed -e 's/[[:space:]]*$//')"
  if [[ -z "$line" ]]; then
    continue
  fi
  if [[ "$line" == \#* ]]; then
    continue
  fi

  # Strip trailing " # ..." comments.
  line="$(echo "$line" | sed -e 's/[[:space:]]\+#.*$//')"
  if [[ -z "$line" ]]; then
    continue
  fi

  set +e
  stdout="$($line 2>"$tmp_dir/stderr.txt")"
  code="$?"
  set -e

  if [[ "$code" -ne 0 ]]; then
    echo "ERROR: README command failed ($code): $line" >&2
    cat "$tmp_dir/stderr.txt" >&2 || true
    echo "$stdout" >&2
    exit 1
  fi

  if [[ -n "$stdout" ]]; then
    printf '%s' "$stdout" | "$python_bin" -c 'import json,sys; json.loads(sys.stdin.read())'
  fi
done <"$commands_file"

echo "ok: README commands"
