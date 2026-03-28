#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: publish-channel-manifest.sh --channel <stable|beta|nightly> --bundle <PATH> [--publish-dir <DIR>]
EOF
  exit 2
}

channel=""
bundle=""
publish_dir="dist/channels"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --channel)
      channel="${2:-}"
      shift 2
      ;;
    --bundle)
      bundle="${2:-}"
      shift 2
      ;;
    --publish-dir)
      publish_dir="${2:-}"
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

[[ -n "$channel" && -n "$bundle" ]] || usage

case "$channel" in
  stable|beta|nightly) ;;
  *)
    echo "invalid --channel: $channel" >&2
    exit 2
    ;;
esac

[[ -f "$bundle" ]] || { echo "bundle not found: $bundle" >&2; exit 2; }

python3 - "$bundle" "$channel" <<'PY'
import json
import pathlib
import sys

bundle_path = pathlib.Path(sys.argv[1])
channel = sys.argv[2]
doc = json.loads(bundle_path.read_text(encoding="utf-8"))
if not isinstance(doc, dict):
    raise SystemExit("bundle must be a JSON object")
if doc.get("schema_version") != "x07.release.bundle@0.1.0":
    raise SystemExit(f"unexpected schema_version: {doc.get('schema_version')!r}")
if doc.get("channel") != channel:
    raise SystemExit(f"bundle channel mismatch: bundle={doc.get('channel')!r} arg={channel!r}")
PY

mkdir -p "$publish_dir"
dest="${publish_dir}/${channel}.json"
tmp="${dest}.tmp"
cp -f "$bundle" "$tmp"
mv -f "$tmp" "$dest"
printf '%s\n' "$dest"
