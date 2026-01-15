from __future__ import annotations

from pathlib import Path
import sys


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    required = [
        root / "governance" / "TEAMS.md",
        root / "governance" / "MAINTAINERS.md",
        root / "governance" / "DECISION-MAKING.md",
        root / "governance" / "RFC-REQUIREMENTS.md",
    ]

    missing = [p for p in required if not p.is_file()]
    if missing:
        for p in missing:
            print(f"ERROR: missing required governance file: {p.relative_to(root)}", file=sys.stderr)
        return 1

    print("ok: governance files present")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

