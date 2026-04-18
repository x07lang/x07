from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


SEMVER_TAG_RE = re.compile(r"^v(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)$")
SEMVER_VERSION_RE = re.compile(r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)$")
SEMVER_TAG_IN_TEXT_RE = re.compile(r"v(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)")
COMPAT_FILENAME_RE = re.compile(r"^(?P<version>(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))\.json$")
BUNDLE_INPUT_FILENAME_RE = re.compile(r"^(?P<version>(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))\.input\.json$")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write_text(path: Path, data: str) -> None:
    path.write_text(data, encoding="utf-8")


def read_json(path: Path) -> object:
    return json.loads(read_text(path))


def write_json(path: Path, data: object) -> None:
    write_text(path, json.dumps(data, indent=2) + "\n")


def parse_tag(raw: str) -> tuple[str, str]:
    tag = raw.strip()
    if not tag:
        raise ValueError("--tag must be non-empty")
    if not tag.startswith("v"):
        tag = f"v{tag}"
    if SEMVER_TAG_RE.fullmatch(tag) is None:
        raise ValueError(f"invalid tag (expected vX.Y.Z): {tag!r}")
    return tag, tag[1:]


def parse_version(raw: str) -> tuple[int, int, int]:
    m = SEMVER_VERSION_RE.fullmatch(raw)
    if m is None:
        raise ValueError(f"invalid version (expected X.Y.Z): {raw!r}")
    return (int(m.group("major")), int(m.group("minor")), int(m.group("patch")))


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
    out, count = pattern.subn(lambda m: f"{m.group(1)}{new_version}{m.group(2)}", src)
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


def replace_versioned_examples(*, rel_path: str, src: str, new_tag: str) -> str:
    if rel_path == "docs/getting-started/install.md":
        out, n = re.subn(
            rf"(x07up override set\s+){SEMVER_TAG_IN_TEXT_RE.pattern}",
            rf"\g<1>{new_tag}",
            src,
        )
        if n == 0:
            raise ValueError(f"missing expected x07up override example in {rel_path}")
        return out

    if rel_path == "docs/getting-started/installer.md":
        out = src
        out, n1 = re.subn(
            rf"(x07up override set\s+){SEMVER_TAG_IN_TEXT_RE.pattern}",
            rf"\g<1>{new_tag}",
            out,
        )
        out, n2 = re.subn(
            rf'(channel\s*=\s*"){SEMVER_TAG_IN_TEXT_RE.pattern}(")',
            rf"\g<1>{new_tag}\g<2>",
            out,
        )
        out, n3 = re.subn(
            rf"(tag like `){SEMVER_TAG_IN_TEXT_RE.pattern}(`)",
            rf"\g<1>{new_tag}\g<2>",
            out,
        )
        if n1 == 0 or n2 == 0 or n3 == 0:
            raise ValueError(f"missing expected pinned toolchain examples in {rel_path}")
        return out

    if rel_path in ("scripts/build_channels_json.py", "scripts/build_skills_pack.py"):
        out, n = re.subn(
            rf"(for example:\s*){SEMVER_TAG_IN_TEXT_RE.pattern}",
            rf"\g<1>{new_tag}",
            src,
        )
        if n == 0:
            raise ValueError(f"missing expected example tag in {rel_path}")
        return out

    raise ValueError(f"unsupported versioned literal file: {rel_path}")


def replace_x07_registry_git_tag_dependency(src: str, *, dep: str, new_tag: str) -> tuple[str, bool]:
    pattern = re.compile(
        rf'(^\s*{re.escape(dep)}\s*=\s*\{{[^\n}}]*\bgit\s*=\s*"https://github\.com/x07lang/x07"[^\n}}]*\btag\s*=\s*")'
        rf"{SEMVER_TAG_IN_TEXT_RE.pattern}"
        rf'(")',
        flags=re.MULTILINE,
    )
    out, n = pattern.subn(lambda m: f"{m.group(1)}{new_tag}{m.group(2)}", src)
    if n == 0:
        raise ValueError(f"missing expected {dep} git dependency tagged from https://github.com/x07lang/x07")
    return out, out != src


def workspace_lock_is_current(repo_root: Path) -> bool:
    proc = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
        cwd=repo_root,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return proc.returncode == 0


def refresh_workspace_lock(repo_root: Path) -> bool:
    cargo_lock = repo_root / "Cargo.lock"
    before = cargo_lock.read_bytes() if cargo_lock.is_file() else None
    subprocess.run(["cargo", "update", "-w"], cwd=repo_root, check=True)
    after = cargo_lock.read_bytes() if cargo_lock.is_file() else None
    return before != after


def latest_versioned_file(dir_path: Path, *, pattern: re.Pattern[str]) -> Path:
    candidates: list[tuple[tuple[int, int, int], Path]] = []
    for path in dir_path.iterdir():
        if not path.is_file():
            continue
        m = pattern.fullmatch(path.name)
        if m is None:
            continue
        candidates.append((parse_version(m.group("version")), path))
    if not candidates:
        raise ValueError(f"no versioned files found in {dir_path}")
    return max(candidates, key=lambda item: item[0])[1]


def compat_upper_bound(version: str) -> str:
    major, minor, _patch = parse_version(version)
    if major == 0:
        return f"0.{minor + 1}.0"
    return f"{major + 1}.0.0"


def ensure_release_compat(repo_root: Path, *, new_version: str, check: bool) -> str | None:
    compat_dir = repo_root / "releases" / "compat"
    target = compat_dir / f"{new_version}.json"
    source = target if target.is_file() else latest_versioned_file(compat_dir, pattern=COMPAT_FILENAME_RE)
    doc = read_json(source)
    if not isinstance(doc, dict):
        raise ValueError(f"expected JSON object in {source.relative_to(repo_root)}")
    expected = dict(doc)
    expected["x07_core"] = f">={new_version},<{compat_upper_bound(new_version)}"
    rel = str(target.relative_to(repo_root))
    if target.is_file() and read_json(target) == expected:
        return None
    if check:
        return rel
    write_json(target, expected)
    return rel


def ensure_bundle_input(repo_root: Path, *, new_version: str, check: bool) -> str | None:
    bundles_dir = repo_root / "releases" / "bundles"
    target = bundles_dir / f"{new_version}.input.json"
    source = target if target.is_file() else latest_versioned_file(bundles_dir, pattern=BUNDLE_INPUT_FILENAME_RE)
    doc = read_json(source)
    if not isinstance(doc, dict):
        raise ValueError(f"expected JSON object in {source.relative_to(repo_root)}")
    expected = dict(doc)
    expected["min_x07up_version"] = new_version
    rel = str(target.relative_to(repo_root))
    if target.is_file() and read_json(target) == expected:
        return None
    if check:
        return rel
    write_json(target, expected)
    return rel


def ensure_generated_versions_json(repo_root: Path, *, new_version: str, check: bool) -> str | None:
    target = repo_root / "docs" / "_generated" / "versions.json"
    rel = str(target.relative_to(repo_root))

    if check:
        if not target.is_file():
            return rel
        try:
            doc = read_json(target)
        except json.JSONDecodeError:
            return rel
        if not isinstance(doc, dict):
            return rel
        toolchain = doc.get("toolchain")
        if not isinstance(toolchain, dict):
            return rel
        for key in ("x07", "x07c", "x07up"):
            if toolchain.get(key) != new_version:
                return rel
        return None

    before = target.read_bytes() if target.is_file() else None
    subprocess.run([sys.executable, "scripts/gen_versions_json.py", "--write"], cwd=repo_root, check=True)
    after = target.read_bytes() if target.is_file() else None
    if before == after:
        return None
    return rel


def ensure_docs_example_lockfiles(repo_root: Path, *, new_version: str, check: bool) -> list[str]:
    docs_root = repo_root / "docs" / "examples"
    if not docs_root.is_dir():
        return []
    lockfiles = sorted(docs_root.rglob("x07.lock.json"))
    if not lockfiles:
        return []

    def is_mismatched(path: Path) -> bool:
        try:
            doc = read_json(path)
        except json.JSONDecodeError:
            return True
        if not isinstance(doc, dict):
            return True
        toolchain = doc.get("toolchain")
        if not isinstance(toolchain, dict):
            return True
        return toolchain.get("x07_version") != new_version or toolchain.get("x07c_version") != new_version

    mismatched = [str(p.relative_to(repo_root)) for p in lockfiles if is_mismatched(p)]
    if check:
        return mismatched
    if not mismatched:
        return []

    subprocess.run(
        [sys.executable, "scripts/upgrade_docs_example_lockfiles.py", "--write"],
        cwd=repo_root,
        check=True,
    )
    return mismatched


def ensure_ci_fixture_lockfiles(repo_root: Path, *, new_version: str, check: bool) -> list[str]:
    fixtures_root = repo_root / "ci" / "fixtures"
    if not fixtures_root.is_dir():
        return []
    lockfiles = sorted(fixtures_root.rglob("x07.lock.json"))
    if not lockfiles:
        return []

    def is_mismatched(path: Path) -> bool:
        try:
            doc = read_json(path)
        except json.JSONDecodeError:
            return True
        if not isinstance(doc, dict):
            return True
        toolchain = doc.get("toolchain")
        if not isinstance(toolchain, dict):
            return True
        return toolchain.get("x07_version") != new_version or toolchain.get("x07c_version") != new_version

    mismatched = [str(p.relative_to(repo_root)) for p in lockfiles if is_mismatched(p)]
    if check:
        return mismatched
    if not mismatched:
        return []

    subprocess.run(
        [sys.executable, "scripts/upgrade_docs_example_lockfiles.py", "--root", "ci/fixtures", "--write"],
        cwd=repo_root,
        check=True,
    )
    return mismatched


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
    crate_manifest_changed = False

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
            crate_manifest_changed = True
            changed.append(str(cargo_toml.relative_to(repo_root)))
        else:
            _ = replaced_pkg
            _ = dep_rewrites

    if args.check:
        if not workspace_lock_is_current(repo_root):
            changed.append("Cargo.lock")
    elif crate_manifest_changed and refresh_workspace_lock(repo_root):
        changed.append("Cargo.lock")

    versioned_literal_files = [
        repo_root / "docs" / "getting-started" / "install.md",
        repo_root / "docs" / "getting-started" / "installer.md",
        repo_root / "scripts" / "build_channels_json.py",
        repo_root / "scripts" / "build_skills_pack.py",
    ]
    for path in versioned_literal_files:
        if not path.is_file():
            raise ValueError(f"missing expected file: {path.relative_to(repo_root)}")
        rel = str(path.relative_to(repo_root))
        src = read_text(path)
        try:
            out = replace_versioned_examples(rel_path=rel, src=src, new_tag=new_tag)
        except ValueError:
            if args.check:
                changed.append(rel)
                continue
            raise
        if out != src:
            if args.check:
                changed.append(rel)
                continue
            write_text(path, out)
            changed.append(rel)

    release_metadata_updates = [
        ensure_release_compat(repo_root, new_version=new_version, check=args.check),
        ensure_bundle_input(repo_root, new_version=new_version, check=args.check),
        ensure_generated_versions_json(repo_root, new_version=new_version, check=args.check),
    ]
    changed.extend(rel for rel in release_metadata_updates if rel is not None)

    changed.extend(ensure_docs_example_lockfiles(repo_root, new_version=new_version, check=args.check))
    changed.extend(ensure_ci_fixture_lockfiles(repo_root, new_version=new_version, check=args.check))

    registry_repo_root = repo_root.parent / "x07-registry"
    registry_cargo = registry_repo_root / "Cargo.toml"
    if registry_cargo.is_file():
        rel = str(registry_cargo.relative_to(repo_root.parent))
        src = read_text(registry_cargo)
        try:
            out = src
            out, _ = replace_x07_registry_git_tag_dependency(out, dep="x07-worlds", new_tag=new_tag)
            out, _ = replace_x07_registry_git_tag_dependency(out, dep="x07c", new_tag=new_tag)
        except ValueError:
            if args.check:
                changed.append(rel)
                out = src
            else:
                raise
        if out != src:
            if args.check:
                changed.append(rel)
            else:
                write_text(registry_cargo, out)
                changed.append(rel)

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
