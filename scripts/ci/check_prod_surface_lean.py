#!/usr/bin/env python3
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

try:
    import tomllib  # py>=3.11
except Exception:  # pragma: no cover
    tomllib = None  # type: ignore


ROOT = Path(__file__).resolve().parents[2]

# NOTE: This is intentionally strict. Any new tracked root entry should be deliberate.
ALLOWED_TRACKED_ROOT_ENTRIES = {
    # dotfiles
    ".dockerignore",
    ".gitattributes",
    ".github",
    ".gitignore",

    # canonical agent/dev guidance
    "AGENT.md",
    "CODE_OF_CONDUCT.md",
    "CONTRIBUTING.md",
    "README.md",
    "SECURITY.md",
    "SUPPORT.md",
    "TRADEMARKS.md",
    "LICENSE-APACHE",
    "LICENSE-MIT",

    # rust/workspace
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",

    # locks
    "stdlib.lock",
    "stdlib.os.lock",

    # production content
    "arch",
    "branding",
    "catalog",
    "ci",
    "crates",
    "deps",
    "dist",
    "docs",
    "examples",
    "governance",
    "locks",
    "packages",
    "schemas",
    "scripts",
    "skills",
    "spec",
    "stdlib",
    "tests",
    "worlds",

    # optional labs (may be absent in trimmed checkouts)
    "labs",
}

FORBIDDEN_TRACKED_ROOT_ENTRIES = {
    ".claude",
    "CLAUDE.md",
    "AGENTS.md",
    "benchmarks",
    "fuzz",
    "scripts/bench",
}

# These strings are user-facing framing that should not appear in production docs/guides.
FORBIDDEN_PUBLIC_STRINGS = [
    "Track B",
    "Phase H",
    "phaseH",
]

# These path fragments must not be referenced outside labs after the migration.
FORBIDDEN_PATH_FRAGMENTS = [
    "benchmarks/",
    "benchmarks\\",
    "scripts/bench/",
    "scripts\\bench\\",
]

# If you want x07import to remain shipped but not part of the default workspace build/test surface,
# keep it out of workspace.default-members.
OPTIONAL_WORKSPACE_MEMBERS = [
    "crates/x07import-core",
    "crates/x07import-cli",
]


def die(msg: str) -> None:
    print(msg, file=sys.stderr)
    raise SystemExit(1)


def tracked_root_entries() -> list[str]:
    if (ROOT / ".git").exists():
        try:
            out = subprocess.check_output(
                ["git", "-c", f"safe.directory={ROOT}", "ls-files", "-z"], cwd=ROOT
            )
            entries: set[str] = set()
            for raw in out.split(b"\0"):
                if not raw:
                    continue
                rel = raw.decode("utf-8", errors="replace")
                p = ROOT / rel
                if not p.exists():
                    continue
                entries.add(rel.split("/", 1)[0])
            return sorted(entries)
        except Exception:
            pass

    # Fallback: use filesystem entries (best-effort; includes untracked).
    ignore = {"target", "tmp"}
    return sorted(
        [p.name for p in ROOT.iterdir() if p.name not in {".git"} and p.name not in ignore]
    )


def check_root_allowlist() -> None:
    entries = tracked_root_entries()

    forbidden_present = sorted([e for e in entries if e in FORBIDDEN_TRACKED_ROOT_ENTRIES])
    if forbidden_present:
        die(
            "error: forbidden tracked root entries present:\n"
            + "\n".join(f"  - {e}" for e in forbidden_present)
            + "\n\nMove these under labs/ or delete them."
        )

    unknown = sorted([e for e in entries if e not in ALLOWED_TRACKED_ROOT_ENTRIES])
    if unknown:
        die(
            "error: unexpected tracked top-level repo entries (production surface leak):\n"
            + "\n".join(f"  - {e}" for e in unknown)
            + "\n\nIf this is intentional, either:\n"
            "  (a) move it under labs/, or\n"
            "  (b) add it to ALLOWED_TRACKED_ROOT_ENTRIES (deliberate choice).\n"
        )

    # Also forbid legacy paths that are not top-level entries.
    for rel in ("scripts/bench",):
        if (ROOT / rel).exists():
            die(f"error: forbidden path exists in production surface: {rel}")


def iter_text_files(base: Path) -> list[Path]:
    exts = {
        ".md",
        ".txt",
        ".sh",
        ".ps1",
        ".py",
        ".rs",
        ".toml",
        ".yml",
        ".yaml",
        ".json",
    }
    out: list[Path] = []
    for p in base.rglob("*"):
        if not p.is_file():
            continue
        if "labs" in p.parts:
            continue
        if "target" in p.parts:
            continue
        if p.suffix.lower() in exts:
            out.append(p)
    return out


def check_public_strings_and_paths() -> None:
    # Keep this scoped to user-facing guidance.
    scan_roots = [
        ROOT / "docs",
        ROOT / "examples",
        ROOT / "skills",
        ROOT / "README.md",
        ROOT / "AGENT.md",
    ]

    files: list[Path] = []
    for r in scan_roots:
        if r.is_file():
            files.append(r)
        elif r.exists():
            files.extend(iter_text_files(r))

    violations: list[str] = []
    for p in files:
        try:
            text = p.read_text("utf-8", errors="replace")
        except Exception:
            continue

        for s in FORBIDDEN_PUBLIC_STRINGS:
            if s in text:
                violations.append(f"{p.relative_to(ROOT)}: contains forbidden public string: {s!r}")

        for frag in FORBIDDEN_PATH_FRAGMENTS:
            if frag in text:
                violations.append(f"{p.relative_to(ROOT)}: references forbidden path fragment: {frag!r}")

    if violations:
        die("error: production docs/guides contain forbidden references:\n" + "\n".join(f"  - {v}" for v in violations))


def check_workspace_default_members() -> None:
    cargo_toml = ROOT / "Cargo.toml"
    if not cargo_toml.exists():
        die("error: Cargo.toml missing at repo root")

    if tomllib is None:
        die("error: tomllib unavailable (need Python 3.11+) for this CI gate")

    data = tomllib.loads(cargo_toml.read_text("utf-8"))
    ws = data.get("workspace", {})
    default_members = ws.get("default-members")

    if not default_members:
        die(
            "error: workspace.default-members is not set.\n"
            "Set it explicitly so `cargo test` at the workspace root selects the production surface.\n"
            "This is required to keep labs optional."
        )

    if not isinstance(default_members, list):
        die("error: workspace.default-members must be a list")

    default_members = [str(x) for x in default_members]

    leaks = [m for m in OPTIONAL_WORKSPACE_MEMBERS if m in default_members]
    if leaks:
        die(
            "error: optional members are included in workspace.default-members:\n"
            + "\n".join(f"  - {m}" for m in leaks)
            + "\n\nRemove them from default-members; they can still be built explicitly."
        )


def main() -> None:
    os.chdir(ROOT)
    check_root_allowlist()
    check_public_strings_and_paths()
    check_workspace_default_members()
    print("ok: production surface is lean; labs remains optional.")


if __name__ == "__main__":
    main()
