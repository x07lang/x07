from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


SEMVER_TAG_RE = re.compile(r"^v(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)$")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write_text(path: Path, data: str) -> None:
    path.write_text(data, encoding="utf-8")


def parse_tag(raw: str) -> tuple[str, str]:
    tag = raw.strip()
    if not tag:
        raise ValueError("--tag must be non-empty")
    if not tag.startswith("v"):
        tag = f"v{tag}"
    if SEMVER_TAG_RE.fullmatch(tag) is None:
        raise ValueError(f"invalid tag (expected vX.Y.Z): {tag!r}")
    return tag, tag[1:]


def replace_package_version_in_cargo_toml(src: str, *, new_version: str) -> tuple[str, bool]:
    lines = src.splitlines(keepends=True)
    in_package = False
    replaced = False

    for idx, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_package = stripped == "[package]"
            continue
        if not in_package:
            continue

        m = re.match(r'^(?P<prefix>\s*version\s*=\s*")(?P<ver>[^"]+)(?P<suffix>"\s*(?:#.*)?)\n?$', line)
        if m:
            if m.group("ver") == new_version:
                return src, False
            newline = "\n" if line.endswith("\n") else ""
            lines[idx] = f'{m.group("prefix")}{new_version}{m.group("suffix")}{newline}'
            replaced = True
            break

    if not replaced:
        raise ValueError("missing [package].version")

    return "".join(lines), True


def replace_dependency_version_literals(src: str, *, old_version: str, new_version: str) -> tuple[str, int]:
    if old_version == new_version:
        return src, 0

    pattern = re.compile(rf'(\bversion\s*=\s*"){re.escape(old_version)}(")')
    out, count = pattern.subn(rf'\1{new_version}\2', src)
    return out, count


def read_cargo_package_version(cargo_toml: Path) -> str:
    src = read_text(cargo_toml)
    in_package = False
    for line in src.splitlines():
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            in_package = stripped == "[package]"
            continue
        if not in_package:
            continue
        m = re.match(r'^\s*version\s*=\s*"(?P<ver>[^"]+)"\s*(?:#.*)?$', line)
        if m:
            return m.group("ver")
    raise ValueError(f"missing [package].version in {cargo_toml}")


def update_file_replace_literal(path: Path, *, old: str, new: str) -> bool:
    src = read_text(path)
    if old == new:
        if new in src:
            return False
        raise ValueError(f"missing expected tag {new!r}")
    if old in src:
        out = src.replace(old, new)
        if out != src:
            write_text(path, out)
            return True
        return False
    if new in src:
        return False
    raise ValueError(f"missing expected tag {old!r} (or already updated tag {new!r})")


def check_versioned_literal_file(path: Path, *, old: str, new: str) -> bool:
    src = read_text(path)
    if old == new:
        if new in src:
            return False
        raise ValueError(f"missing expected tag {new!r}")
    if old in src:
        return True
    if new in src:
        return False
    raise ValueError(f"missing expected tag {old!r} (or already updated tag {new!r})")


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--tag", required=True, help="New release tag (for example: v0.0.21)")
    ap.add_argument("--check", action="store_true", help="Fail if changes are required")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    repo_root = Path(__file__).resolve().parents[1]

    new_tag, new_version = parse_tag(args.tag)

    primary_cargo = repo_root / "crates" / "x07" / "Cargo.toml"
    old_version = read_cargo_package_version(primary_cargo)
    old_tag = f"v{old_version}"

    changed: list[str] = []

    crate_tomls = sorted((repo_root / "crates").glob("*/Cargo.toml"))
    for cargo_toml in crate_tomls:
        src = read_text(cargo_toml)
        out, replaced_pkg = replace_package_version_in_cargo_toml(src, new_version=new_version)
        out, dep_rewrites = replace_dependency_version_literals(out, old_version=old_version, new_version=new_version)
        if out != src:
            if args.check:
                changed.append(str(cargo_toml.relative_to(repo_root)))
                continue
            write_text(cargo_toml, out)
            changed.append(str(cargo_toml.relative_to(repo_root)))
        else:
            _ = replaced_pkg
            _ = dep_rewrites

    versioned_literal_files = [
        repo_root / "docs" / "getting-started" / "install.md",
        repo_root / "docs" / "getting-started" / "installer.md",
        repo_root / "scripts" / "build_channels_json.py",
        repo_root / "scripts" / "build_skills_pack.py",
    ]
    for path in versioned_literal_files:
        if not path.is_file():
            raise ValueError(f"missing expected file: {path.relative_to(repo_root)}")
        if args.check:
            try:
                needs_update = check_versioned_literal_file(path, old=old_tag, new=new_tag)
            except ValueError:
                changed.append(str(path.relative_to(repo_root)))
            else:
                if needs_update:
                    changed.append(str(path.relative_to(repo_root)))
            continue
        if update_file_replace_literal(path, old=old_tag, new=new_tag):
            changed.append(str(path.relative_to(repo_root)))

    if changed:
        changed = sorted(set(changed))
        if args.check:
            for rel in changed:
                print(rel)
            print("ERROR: version bump required (run without --check)", file=sys.stderr)
            return 1
        for rel in changed:
            print(f"updated: {rel}")
        print(f"ok: bumped {old_tag} -> {new_tag}")
        return 0

    if args.check:
        print(f"ok: already at {new_tag}")
    else:
        print(f"ok: no changes needed ({new_tag})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
