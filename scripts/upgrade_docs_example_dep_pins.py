#!/usr/bin/env python3
"""Align docs-example dependency pins with the bundled package set.

During a release bump, examples can pin package versions that conflict with
transitive requirements in the bundled package set (for example
`ext-net@0.1.8` while `ext-obs@0.1.2` requires `ext-net@0.1.9`). This script
makes the release train hands-off for that class of drift:

  for each docs/examples project:
    run `x07 pkg lock --offline`; on a version-conflict error, bump the
    project pin (and its vendored `.x07/deps` path) to the required version
    and retry (bounded), then canonicalize the manifest with `x07 fmt`.

Stdlib-only, deterministic, offline. Use --check in CI to fail when pins
would change; use --write to apply.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
EXAMPLES_ROOT = REPO_ROOT / "docs" / "examples"
MAX_PIN_FIXES_PER_PROJECT = 10

CONFLICT_RE = re.compile(
    r'project has (?P<name>[A-Za-z0-9_./-]+)@(?P<have>[0-9][^,\s]*), '
    r'but "(?P=name)@(?P<want>[0-9][^"\s]*)" is required'
)


def find_x07() -> str:
    candidates = [
        REPO_ROOT / "target" / "debug" / "x07",
        REPO_ROOT / "target" / "release" / "x07",
    ]
    for c in candidates:
        if c.is_file():
            return str(c)
    return "x07"


def pkg_lock(x07: str, project: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [x07, "pkg", "lock", "--project", str(project), "--offline"],
        capture_output=True,
        text=True,
        check=False,
    )


def bump_pin(project: Path, name: str, have: str, want: str) -> bool:
    doc = json.loads(project.read_text())
    changed = False
    for dep in doc.get("dependencies", []):
        if dep.get("name") == name and dep.get("version") == have:
            dep["version"] = want
            if isinstance(dep.get("path"), str):
                dep["path"] = dep["path"].replace(have, want)
            changed = True
    if changed:
        project.write_text(json.dumps(doc, indent=2) + "\n")
    return changed


def process_project(x07: str, project: Path, write: bool) -> list[str]:
    changes: list[str] = []
    for _ in range(MAX_PIN_FIXES_PER_PROJECT):
        proc = pkg_lock(x07, project)
        if proc.returncode == 0:
            break
        m = CONFLICT_RE.search(proc.stderr) or CONFLICT_RE.search(proc.stdout)
        if not m:
            break  # not a pin conflict; other gates own this failure
        name, have, want = m.group("name"), m.group("have"), m.group("want")
        if not write:
            changes.append(f"{project}: would pin {name} {have} -> {want}")
            break
        if not bump_pin(project, name, have, want):
            break
        changes.append(f"{project}: pinned {name} {have} -> {want}")
    if write and changes:
        subprocess.run(
            [x07, "fmt", "--input", str(project), "--write"],
            capture_output=True,
            check=False,
        )
        pkg_lock(x07, project)
    return changes


def reset_untracked_cache(project: Path) -> bool:
    """Remove the example's untracked .x07 cache so offline resolution matches
    the cache-free CI environment. Tracked caches are left alone."""
    cache = project.parent / ".x07"
    if not cache.is_dir():
        return False
    tracked = subprocess.run(
        ["git", "-C", str(REPO_ROOT), "ls-files", "--error-unmatch",
         str(cache.relative_to(REPO_ROOT))],
        capture_output=True,
        check=False,
    )
    if tracked.returncode == 0:
        return False
    import shutil

    shutil.rmtree(cache)
    return True


def main() -> int:
    ap = argparse.ArgumentParser()
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    ap.add_argument(
        "--reset-caches",
        action="store_true",
        help="with --write: drop untracked example .x07 caches and relock, "
        "matching the cache-free CI environment",
    )
    args = ap.parse_args()

    x07 = find_x07()
    all_changes: list[str] = []
    for project in sorted(EXAMPLES_ROOT.rglob("x07.json")):
        if args.write and args.reset_caches and reset_untracked_cache(project):
            proc = pkg_lock(x07, project)
            status = "relocked" if proc.returncode == 0 else "RELOCK FAILED"
            all_changes.append(f"{project}: cache reset, {status}")
        all_changes.extend(process_project(x07, project, args.write))

    for line in all_changes:
        print(line)
    if args.check and all_changes:
        print("ERROR: docs example dependency pins would change", file=sys.stderr)
        return 1
    print("ok: docs example dependency pins are consistent"
          if not all_changes else f"ok: applied {len(all_changes)} pin updates")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
