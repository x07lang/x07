#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any, Optional, Tuple


SEMVER_RE = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$"
)
PKG_NAME_RE = re.compile(r"^[a-z][a-z0-9_-]{0,127}$")
PROFILE_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")

SOLVE_WORLDS = {"solve-pure", "solve-fs", "solve-rr", "solve-kv", "solve-full"}
ALL_WORLDS = SOLVE_WORLDS | {"run-os", "run-os-sandboxed"}


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def eprint(msg: str) -> None:
    print(msg, file=sys.stderr)


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception as ex:
        raise ValueError(f"{path}: invalid JSON: {ex}") from ex


def is_rel_path(s: str) -> bool:
    if not s or not isinstance(s, str):
        return False
    p = Path(s)
    if p.is_absolute():
        return False
    # Reject Windows drive prefixes and any '..' segments.
    for part in p.parts:
        if part == "..":
            return False
        if len(part) >= 2 and part[1] == ":" and part[0].isalpha():
            return False
    return True


def parse_semver(s: str) -> Optional[Tuple[int, int, int]]:
    if not isinstance(s, str):
        return None
    if not SEMVER_RE.match(s):
        return None
    parts = s.split(".", 2)
    try:
        # Split off prerelease/build for the patch part.
        patch_part = parts[2].split("-", 1)[0].split("+", 1)[0]
        return (int(parts[0]), int(parts[1]), int(patch_part))
    except Exception:
        return None


def validate_dep(dep: Any, rel: str, idx: int) -> list[str]:
    errs: list[str] = []
    if not isinstance(dep, dict):
        return [f"{rel}: dependencies[{idx}] must be an object"]

    name = dep.get("name")
    version = dep.get("version")
    path = dep.get("path")

    if not isinstance(name, str) or not PKG_NAME_RE.match(name):
        errs.append(f"{rel}: dependencies[{idx}].name must match {PKG_NAME_RE.pattern!r}")
    if not isinstance(version, str) or parse_semver(version) is None:
        errs.append(f"{rel}: dependencies[{idx}].version must be semver")
    if not isinstance(path, str) or not is_rel_path(path):
        errs.append(f"{rel}: dependencies[{idx}].path must be a relative path")

    return errs


def validate_profile(name: str, prof: Any, rel: str) -> list[str]:
    errs: list[str] = []
    if not PROFILE_NAME_RE.match(name):
        errs.append(f"{rel}: profiles key {name!r} must match {PROFILE_NAME_RE.pattern!r}")
        return errs
    if not isinstance(prof, dict):
        errs.append(f"{rel}: profiles[{name!r}] must be an object")
        return errs

    world = prof.get("world")
    if not isinstance(world, str) or world not in ALL_WORLDS:
        errs.append(f"{rel}: profiles[{name!r}].world must be one of {sorted(ALL_WORLDS)}")
        return errs

    policy = prof.get("policy")
    if world == "run-os-sandboxed":
        if not isinstance(policy, str) or not is_rel_path(policy):
            errs.append(f"{rel}: profiles[{name!r}].policy is required for run-os-sandboxed and must be a relative path")
    else:
        if policy is not None:
            errs.append(f"{rel}: profiles[{name!r}].policy is only valid for run-os-sandboxed")

    runner = prof.get("runner")
    if runner is not None and runner not in ("auto", "host", "os"):
        errs.append(f"{rel}: profiles[{name!r}].runner must be one of ['auto','host','os'] when present")

    input_ = prof.get("input")
    if input_ is not None and (not isinstance(input_, str) or not is_rel_path(input_)):
        errs.append(f"{rel}: profiles[{name!r}].input must be a relative path when present")

    auto_ffi = prof.get("auto_ffi")
    if auto_ffi is not None and not isinstance(auto_ffi, bool):
        errs.append(f"{rel}: profiles[{name!r}].auto_ffi must be boolean when present")

    def check_u64(field: str) -> None:
        v = prof.get(field)
        if v is None:
            return
        if not isinstance(v, int) or v < 0:
            errs.append(f"{rel}: profiles[{name!r}].{field} must be a non-negative integer when present")

    for f in (
        "solve_fuel",
        "cpu_time_limit_seconds",
        "max_memory_bytes",
        "max_output_bytes",
    ):
        check_u64(f)

    cc_profile = prof.get("cc_profile")
    if cc_profile is not None and cc_profile not in ("default", "size"):
        errs.append(f"{rel}: profiles[{name!r}].cc_profile must be one of ['default','size'] when present")

    return errs


def validate_project(doc: Any, rel: str) -> list[str]:
    errs: list[str] = []
    if not isinstance(doc, dict):
        return [f"{rel}: root must be a JSON object"]

    if doc.get("schema_version") != "x07.project@0.2.0":
        errs.append(f"{rel}: schema_version must be 'x07.project@0.2.0'")

    world = doc.get("world")
    if not isinstance(world, str) or world not in ALL_WORLDS:
        errs.append(f"{rel}: world must be one of {sorted(ALL_WORLDS)}")

    entry = doc.get("entry")
    if not isinstance(entry, str) or not entry.endswith(".x07.json") or not is_rel_path(entry):
        errs.append(f"{rel}: entry must be a relative *.x07.json path")

    module_roots = doc.get("module_roots")
    if not isinstance(module_roots, list) or not module_roots:
        errs.append(f"{rel}: module_roots must be a non-empty array")
    else:
        seen: set[str] = set()
        for i, r in enumerate(module_roots):
            if not isinstance(r, str) or not is_rel_path(r):
                errs.append(f"{rel}: module_roots[{i}] must be a relative path")
                continue
            if r in seen:
                errs.append(f"{rel}: module_roots[{i}] duplicates {r!r}")
            seen.add(r)

    lockfile = doc.get("lockfile")
    if lockfile is not None and lockfile != "" and (not isinstance(lockfile, str) or not is_rel_path(lockfile)):
        errs.append(f"{rel}: lockfile must be a relative path or null when present")

    deps = doc.get("dependencies")
    if deps is not None:
        if not isinstance(deps, list):
            errs.append(f"{rel}: dependencies must be an array when present")
        else:
            for i, dep in enumerate(deps):
                errs.extend(validate_dep(dep, rel, i))

    link = doc.get("link")
    if link is not None:
        if not isinstance(link, dict):
            errs.append(f"{rel}: link must be an object when present")
        else:
            for k in ("libs", "search_paths", "frameworks"):
                v = link.get(k)
                if v is None:
                    continue
                if not isinstance(v, list) or not all(isinstance(s, str) and s for s in v):
                    errs.append(f"{rel}: link.{k} must be an array of non-empty strings when present")
            static = link.get("static")
            if static is not None and not isinstance(static, bool):
                errs.append(f"{rel}: link.static must be boolean when present")

    profiles = doc.get("profiles")
    if profiles is not None:
        if not isinstance(profiles, dict):
            errs.append(f"{rel}: profiles must be an object when present")
        else:
            for k, v in profiles.items():
                errs.extend(validate_profile(str(k), v, rel))

    default_profile = doc.get("default_profile")
    if default_profile is not None:
        if not isinstance(default_profile, str) or not default_profile.strip():
            errs.append(f"{rel}: default_profile must be a non-empty string when present")
        elif profiles is not None and isinstance(profiles, dict):
            if default_profile not in profiles:
                errs.append(f"{rel}: default_profile {default_profile!r} is not present in profiles")

    return errs


def iter_project_paths(root: Path) -> list[Path]:
    out: list[Path] = []
    for p in root.rglob("x07.json"):
        if not p.is_file():
            continue
        parts = set(p.parts)
        if ".git" in parts or "target" in parts:
            continue
        out.append(p)
    out.sort(key=lambda x: x.as_posix())
    return out


def main() -> int:
    root = repo_root()
    paths = iter_project_paths(root)
    if not paths:
        eprint("ERROR: no x07.json files found to validate")
        return 2

    all_errs: list[str] = []
    for p in paths:
        rel = str(p.relative_to(root))
        try:
            doc = load_json(p)
        except ValueError as ex:
            all_errs.append(str(ex))
            continue
        all_errs.extend(validate_project(doc, rel))

    if all_errs:
        eprint("ERROR: project manifest validation failed")
        for m in all_errs:
            eprint(f"  - {m}")
        return 2

    print(f"ok: validated {len(paths)} project manifests (x07.json)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
