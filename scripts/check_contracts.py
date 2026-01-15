from __future__ import annotations

from pathlib import Path
import subprocess
import sys


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    cmd = ["bash", str(root / "scripts" / "ci" / "check_llm_contracts.sh")]
    proc = subprocess.run(cmd, cwd=root)
    return proc.returncode


if __name__ == "__main__":
    raise SystemExit(main())

