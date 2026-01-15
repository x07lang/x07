from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:
        raise SystemExit(f"ERROR: parse {path}: {e}")


def render_json(obj: Any) -> str:
    return json.dumps(obj, sort_keys=True, indent=2) + "\n"


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", type=Path, default=Path("index/config.json"))
    ap.add_argument("--dl", default=None, help="Download base URL (must end with '/')")
    ap.add_argument("--api", default=None, help="API base URL (must end with '/')")
    ap.add_argument("--auth-required", default=None, choices=["true", "false"])
    ap.add_argument("--check", action="store_true", help="Validate without writing files")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    path: Path = args.config
    obj = read_json(path)
    if not isinstance(obj, dict):
        raise SystemExit(f"ERROR: {path} must be a JSON object")

    for k in ["dl", "api", "auth-required"]:
        if k not in obj:
            raise SystemExit(f"ERROR: {path} missing key: {k!r}")

    if args.dl is not None:
        if not args.dl.endswith("/"):
            raise SystemExit("ERROR: --dl must end with '/'")
        obj["dl"] = args.dl
    if args.api is not None:
        if not args.api.endswith("/"):
            raise SystemExit("ERROR: --api must end with '/'")
        obj["api"] = args.api
    if args.auth_required is not None:
        obj["auth-required"] = args.auth_required == "true"

    rendered = render_json(obj)
    if args.check:
        if path.read_text(encoding="utf-8") != rendered:
            print(f"ERROR: {path} would change", file=sys.stderr)
            return 1
        print("ok: index/config.json up to date")
        return 0

    path.write_text(rendered, encoding="utf-8")
    print(f"ok: updated {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
