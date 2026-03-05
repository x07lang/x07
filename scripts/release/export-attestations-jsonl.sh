#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: export-attestations-jsonl.sh --in-dir <DIR> --out <PATH>
EOF
  exit 2
}

in_dir=""
out_path=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --in-dir)
      in_dir="${2:-}"
      shift 2
      ;;
    --out)
      out_path="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      ;;
  esac
done

[[ -n "$in_dir" && -n "$out_path" ]] || usage
[[ -d "$in_dir" ]] || { echo "--in-dir is not a directory: $in_dir" >&2; exit 2; }

python3 - "$in_dir" "$out_path" <<'PY'
import datetime as dt
import hashlib
import json
import pathlib
import sys

in_dir = pathlib.Path(sys.argv[1]).resolve()
out_path = pathlib.Path(sys.argv[2]).resolve()
generated_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

def sha256_file(path: pathlib.Path) -> tuple[str, int]:
    h = hashlib.sha256()
    n = 0
    with path.open("rb") as f:
        while True:
            chunk = f.read(1024 * 1024)
            if not chunk:
                break
            n += len(chunk)
            h.update(chunk)
    return h.hexdigest(), n

lines = []
for path in sorted(in_dir.iterdir(), key=lambda x: x.name):
    if not path.is_file():
        continue
    if path.suffix == ".tmp":
        continue
    sha, n = sha256_file(path)
    lines.append(
        json.dumps(
            {
                "predicate_type": "https://x07lang.org/attestations/release-asset/v1",
                "generated_at_utc": generated_at,
                "asset": {
                    "name": path.name,
                    "sha256": f"sha256:{sha}",
                    "bytes_len": n,
                },
            },
            sort_keys=True,
        )
    )

if not lines:
    raise SystemExit(f"no release files found in {in_dir}")

out_path.parent.mkdir(parents=True, exist_ok=True)
out_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
print(out_path)
PY
