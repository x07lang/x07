#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, NoReturn
from urllib.parse import urlparse


SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


@dataclass(frozen=True)
class Semver:
    major: int
    minor: int
    patch: int

    @staticmethod
    def parse(s: str) -> "Semver | None":
        m = SEMVER_RE.match(s.strip())
        if not m:
            return None
        return Semver(int(m.group(1)), int(m.group(2)), int(m.group(3)))


@dataclass(frozen=True)
class PackageSpec:
    name: str
    version: str

    def __str__(self) -> str:
        return f"{self.name}@{self.version}"

    def sort_key(self) -> tuple[str, int, int, int]:
        sv = Semver.parse(self.version)
        if sv is None:
            return (self.name, 0, 0, 0)
        return (self.name, sv.major, sv.minor, sv.patch)


def _die(msg: str, code: int = 2) -> NoReturn:
    print(msg, file=sys.stderr)
    raise SystemExit(code)


def _repo_root() -> Path:
    # scripts/publish_ext_packages.py -> scripts -> repo root
    return Path(__file__).resolve().parents[1]


def _pick_python(root: Path) -> str:
    env = os.environ.get("X07_PYTHON")
    if env and env.strip():
        return env
    venv = root / ".venv" / "bin" / "python"
    if venv.is_file() and os.access(venv, os.X_OK):
        return str(venv)
    return "python3"


def _run(
    cmd: list[str],
    *,
    cwd: Path,
    allow_fail: bool = False,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        cmd,
        cwd=str(cwd),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    )
    if not allow_fail and proc.returncode != 0:
        stdout = (proc.stdout or "").rstrip()
        stderr = (proc.stderr or "").rstrip()
        msg = [f"ERROR: command failed ({proc.returncode}): {' '.join(cmd)}"]
        if stdout:
            msg.append("")
            msg.append("stdout:")
            msg.append(stdout)
        if stderr:
            msg.append("")
            msg.append("stderr:")
            msg.append(stderr)
        _die("\n".join(msg), code=proc.returncode or 1)
    return proc


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        _die(f"ERROR: missing file: {path}")
    except UnicodeDecodeError as e:
        _die(f"ERROR: invalid UTF-8 in {path}: {e}")
    except json.JSONDecodeError as e:
        _die(f"ERROR: invalid JSON in {path}: {e}")


def _write_json(path: Path, obj: Any) -> None:
    path.write_text(json.dumps(obj, indent=2) + "\n", encoding="utf-8")


def _normalize_index_url(raw: str) -> str:
    s = raw.strip()
    if not s:
        _die("ERROR: --index must be non-empty")
    if s.startswith("sparse+"):
        url = s[len("sparse+") :]
    else:
        url = s
    if not url.endswith("/"):
        url = f"{url}/"
    p = urlparse(url)
    if not p.scheme or not p.netloc:
        _die(f"ERROR: invalid index url: {raw!r}")
    return f"sparse+{url}"


def _index_base(index_url: str) -> str:
    if index_url.startswith("sparse+"):
        return index_url[len("sparse+") :]
    return index_url


def _curl_get_text(url: str) -> tuple[int, str]:
    proc = subprocess.run(
        ["curl", "-sS", "-L", "-w", "\n%{http_code}", url],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if proc.returncode != 0:
        _die(f"ERROR: curl failed for {url}: {(proc.stderr or '').strip()}")
    out = proc.stdout
    if "\n" not in out:
        _die(f"ERROR: curl output missing status line for {url}")
    body, code_str = out.rsplit("\n", 1)
    try:
        code = int(code_str.strip())
    except ValueError:
        _die(f"ERROR: curl returned invalid status code for {url}: {code_str!r}")
    return code, body


def _curl_get_json(url: str) -> tuple[int, Any]:
    code, body = _curl_get_text(url)
    if body.strip() == "":
        return code, None
    try:
        return code, json.loads(body)
    except json.JSONDecodeError as e:
        _die(f"ERROR: {url}: expected JSON (HTTP {code}): {e}")


def _load_index_config(index_url: str) -> dict[str, Any]:
    base = _index_base(index_url)
    if not base.endswith("/"):
        base = f"{base}/"
    url = f"{base}config.json"
    code, doc = _curl_get_json(url)
    if code != 200:
        _die(f"ERROR: failed to fetch index config: HTTP {code}: {url}")
    if not isinstance(doc, dict):
        _die(f"ERROR: index config must be a JSON object: {url}")
    return doc


def _credentials_path() -> Path:
    env = (os.environ.get("X07_PKG_HOME") or "").strip()
    if env:
        return Path(env) / "credentials.json"
    return Path.home() / ".x07" / "credentials.json"


def _has_token_for_index(index_url: str) -> bool:
    path = _credentials_path()
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return False
    except Exception as e:
        _die(f"ERROR: failed to read {path}: {e}")
    if not isinstance(doc, dict):
        _die(f"ERROR: invalid credentials file (expected object): {path}")
    tokens = doc.get("tokens")
    if not isinstance(tokens, dict):
        return False
    tok = tokens.get(index_url)
    return isinstance(tok, str) and bool(tok.strip())


def _parse_pkg_spec(raw: str, *, context: str) -> PackageSpec:
    s = raw.strip()
    if "@" not in s:
        _die(f"ERROR: {context}: expected NAME@VERSION, got {raw!r}")
    name, version = s.split("@", 1)
    name = name.strip()
    version = version.strip()
    if not name or not version:
        _die(f"ERROR: {context}: expected NAME@VERSION, got {raw!r}")
    if Semver.parse(version) is None:
        _die(f"ERROR: {context}: version must be semver (MAJOR.MINOR.PATCH): {raw!r}")
    return PackageSpec(name=name, version=version)


def _capability_targets(root: Path) -> set[PackageSpec]:
    path = root / "catalog" / "capabilities.json"
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:
        _die(f"ERROR: parse {path.relative_to(root)}: {e}")
    if not isinstance(doc, dict):
        _die(f"ERROR: {path.relative_to(root)} must be a JSON object")
    if doc.get("schema_version") != "x07.capabilities@0.1.0":
        _die(f"ERROR: {path.relative_to(root)}: unexpected schema_version: {doc.get('schema_version')!r}")

    caps = doc.get("capabilities") or []
    if not isinstance(caps, list):
        _die(f"ERROR: {path.relative_to(root)}: capabilities must be an array")

    out: set[PackageSpec] = set()

    def add_ref(ref: Any, *, context: str) -> None:
        if not isinstance(ref, dict):
            return
        name = ref.get("name")
        version = ref.get("version")
        if not isinstance(name, str) or not isinstance(version, str):
            return
        out.add(_parse_pkg_spec(f"{name}@{version}", context=context))

    for cap in caps:
        if not isinstance(cap, dict):
            continue
        add_ref(cap.get("canonical"), context="capabilities.canonical")
        alts = cap.get("alternatives") or []
        if isinstance(alts, list):
            for alt in alts:
                add_ref(alt, context="capabilities.alternatives")

    return out


def _latest_ext_version(*, root: Path, pkg_name: str) -> str:
    pkg_dir = root / "packages" / "ext" / f"x07-{pkg_name}"
    if not pkg_dir.is_dir():
        _die(f"ERROR: missing ext package dir: {pkg_dir.relative_to(root)}")

    versions: list[tuple[Semver, str]] = []
    for child in pkg_dir.iterdir():
        if not child.is_dir():
            continue
        v = Semver.parse(child.name)
        if v is None:
            continue
        versions.append((v, child.name))
    if not versions:
        _die(f"ERROR: no semver versions under: {pkg_dir.relative_to(root)}")

    versions.sort(key=lambda it: (it[0].major, it[0].minor, it[0].patch))
    return versions[-1][1]


def _sync_capabilities_versions(*, root: Path, write: bool) -> list[str]:
    path = root / "catalog" / "capabilities.json"
    doc = _read_json(path)
    if not isinstance(doc, dict):
        _die("ERROR: catalog/capabilities.json: expected JSON object")
    if doc.get("schema_version") != "x07.capabilities@0.1.0":
        _die(
            f"ERROR: catalog/capabilities.json: unexpected schema_version: {doc.get('schema_version')!r}"
        )

    caps = doc.get("capabilities")
    if not isinstance(caps, list):
        _die("ERROR: catalog/capabilities.json: capabilities must be an array")

    changes: list[str] = []

    def sync_pkg(obj: dict[str, Any], key: str) -> None:
        pkg = obj.get(key)
        if not isinstance(pkg, dict):
            return
        name = pkg.get("name")
        version = pkg.get("version")
        if not isinstance(name, str) or not name:
            return
        if not isinstance(version, str) or not version:
            return
        latest = _latest_ext_version(root=root, pkg_name=name)
        if latest != version:
            pkg["version"] = latest
            changes.append(f"{path.relative_to(root)}: {name}: {version} -> {latest}")

    for cap in caps:
        if not isinstance(cap, dict):
            continue
        sync_pkg(cap, "canonical")
        alts = cap.get("alternatives") or []
        if isinstance(alts, list):
            for alt in alts:
                if isinstance(alt, dict):
                    name = alt.get("name")
                    version = alt.get("version")
                    if not isinstance(name, str) or not name:
                        continue
                    if not isinstance(version, str) or not version:
                        continue
                    latest = _latest_ext_version(root=root, pkg_name=name)
                    if latest != version:
                        alt["version"] = latest
                        changes.append(
                            f"{path.relative_to(root)}: {name}: {version} -> {latest}"
                        )

    if changes and write:
        _write_json(path, doc)
    return changes


def _sync_cli_module_roots(*, root: Path, write: bool) -> list[str]:
    path = root / "crates" / "x07" / "src" / "cli.rs"
    text = path.read_text(encoding="utf-8")

    pkg_versions = {
        "ext-cli": _latest_ext_version(root=root, pkg_name="ext-cli"),
        "ext-data-model": _latest_ext_version(root=root, pkg_name="ext-data-model"),
        "ext-json-rs": _latest_ext_version(root=root, pkg_name="ext-json-rs"),
    }

    changes: list[str] = []
    updated = text
    for name, ver in pkg_versions.items():
        pkg_dir = f"x07-{name}"
        pat = re.compile(
            rf"(packages/ext/{re.escape(pkg_dir)}/)(\d+\.\d+\.\d+)(/modules)"
        )

        def repl(m: re.Match[str]) -> str:
            old = m.group(2)
            if old != ver:
                changes.append(f"{path.relative_to(root)}: {name}: {old} -> {ver}")
            return f"{m.group(1)}{ver}{m.group(3)}"

        updated = pat.sub(repl, updated)

    if changes and write:
        path.write_text(updated, encoding="utf-8")
    return changes


def _iter_project_manifests(root: Path, rel_roots: Iterable[str]) -> Iterable[Path]:
    for rel in rel_roots:
        base = root / rel
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("x07.json")):
            yield path


def _normalize_dep_path(*, dep_name: str, version: str) -> str:
    return f".x07/deps/{dep_name}/{version}"


def _sync_project_dependencies(
    *, root: Path, project_manifest: Path, write: bool
) -> list[str]:
    doc = _read_json(project_manifest)
    if not isinstance(doc, dict):
        return []

    deps = doc.get("dependencies")
    if not isinstance(deps, list) or not deps:
        return []

    changes: list[str] = []
    for dep in deps:
        if not isinstance(dep, dict):
            continue
        name = dep.get("name")
        version = dep.get("version")
        if not isinstance(name, str) or not name.startswith("ext-"):
            continue
        if not isinstance(version, str) or not version:
            continue
        latest = _latest_ext_version(root=root, pkg_name=name)
        if latest == version:
            continue
        dep["version"] = latest
        dep["path"] = _normalize_dep_path(dep_name=name, version=latest)
        changes.append(f"{project_manifest.relative_to(root)}: {name}: {version} -> {latest}")

    if changes and write:
        _write_json(project_manifest, doc)
    return changes


def _seed_project_deps(*, root: Path, project_dir: Path, project_manifest: Path) -> None:
    doc = _read_json(project_manifest)
    deps = doc.get("dependencies") or []
    if not isinstance(deps, list):
        _die("ERROR: x07.json: dependencies must be an array")

    for dep in deps:
        if not isinstance(dep, dict):
            _die(f"ERROR: x07.json: dependency must be object: {dep!r}")
        name = dep.get("name")
        version = dep.get("version")
        rel_path = dep.get("path")
        if not isinstance(name, str) or not name:
            _die(f"ERROR: x07.json: dependency.name must be string: {dep!r}")
        if not isinstance(version, str) or not version:
            _die(f"ERROR: x07.json: dependency.version must be string: {dep!r}")
        if not isinstance(rel_path, str) or not rel_path:
            _die(f"ERROR: x07.json: dependency.path must be string: {dep!r}")

        src = root / "packages" / "ext" / f"x07-{name}" / version
        if not src.is_dir():
            _die(f"ERROR: missing official package dir for {name}@{version}: {src}")

        dst = (project_dir / rel_path).resolve()
        if project_dir.resolve() not in dst.parents and dst != project_dir.resolve():
            _die(f"ERROR: dependency path escapes project dir: {rel_path}")

        if dst.exists():
            if dst.is_dir():
                shutil.rmtree(dst)
            else:
                dst.unlink()
        dst.parent.mkdir(parents=True, exist_ok=True)

        def ignore(src_dir: str, names: list[str]) -> set[str]:
            _ = src_dir
            skip = {"dist", "target", "__pycache__", ".DS_Store"}
            return {n for n in names if n in skip}

        shutil.copytree(src, dst, ignore=ignore)


def _clean_project_state(project_dir: Path) -> None:
    deps_dir = project_dir / ".x07" / "deps"
    if deps_dir.exists():
        shutil.rmtree(deps_dir)


def _sync_ext_package_versions(
    *, root: Path, rel_roots: list[str], write: bool, python_bin: str
) -> tuple[list[str], list[Path]]:
    cap_changes = _sync_capabilities_versions(root=root, write=write)
    cli_changes = _sync_cli_module_roots(root=root, write=write)

    project_changes: list[str] = []
    changed_projects: list[Path] = []
    for manifest in _iter_project_manifests(root, rel_roots):
        changes = _sync_project_dependencies(root=root, project_manifest=manifest, write=write)
        if changes:
            project_changes.extend(changes)
            changed_projects.append(manifest)

    all_changes = cap_changes + cli_changes + project_changes
    if not write:
        return all_changes, changed_projects

    if changed_projects:
        x07_bin = _find_x07_bin(root)
        for manifest in changed_projects:
            project_dir = manifest.parent
            _clean_project_state(project_dir)
            _seed_project_deps(root=root, project_dir=project_dir, project_manifest=manifest)
            _run(
                [str(x07_bin), "pkg", "lock", "--project", str(manifest), "--offline"],
                cwd=project_dir,
            )
            _clean_project_state(project_dir)

    # Keep the external packages lock consistent with the packages tree.
    _run([python_bin, "scripts/generate_external_packages_lock.py", "--write"], cwd=root)

    return all_changes, changed_projects


def _local_pkg_dir(root: Path, spec: PackageSpec) -> Path:
    if not spec.name.startswith("ext-"):
        _die(f"ERROR: unsupported non-ext package: {spec}")
    return root / "packages" / "ext" / f"x07-{spec.name}" / spec.version


def _load_local_manifest(root: Path, spec: PackageSpec) -> dict[str, Any]:
    pkg_dir = _local_pkg_dir(root, spec)
    manifest_path = pkg_dir / "x07-package.json"
    if not manifest_path.is_file():
        _die(f"ERROR: missing local package manifest for {spec}: {manifest_path.relative_to(root)}")
    try:
        doc = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as e:
        _die(f"ERROR: parse {manifest_path.relative_to(root)}: {e}")
    if not isinstance(doc, dict):
        _die(f"ERROR: {manifest_path.relative_to(root)} must be a JSON object")
    if doc.get("name") != spec.name:
        _die(
            f"ERROR: {manifest_path.relative_to(root)}: name mismatch (expected {spec.name!r}, got {doc.get('name')!r})"
        )
    if doc.get("version") != spec.version:
        _die(
            f"ERROR: {manifest_path.relative_to(root)}: version mismatch (expected {spec.version!r}, got {doc.get('version')!r})"
        )
    return doc


def _requires_packages(doc: dict[str, Any]) -> list[PackageSpec]:
    meta = doc.get("meta") or {}
    if not isinstance(meta, dict):
        return []
    req = meta.get("requires_packages") or []
    if not isinstance(req, list):
        return []
    out: list[PackageSpec] = []
    for idx, raw in enumerate(req):
        if not isinstance(raw, str):
            _die(f"ERROR: meta.requires_packages[{idx}] must be a string, got {type(raw).__name__}")
        out.append(_parse_pkg_spec(raw, context=f"meta.requires_packages[{idx}]"))
    return out


def _collect_closure(root: Path, seeds: set[PackageSpec]) -> tuple[dict[PackageSpec, dict[str, Any]], dict[PackageSpec, list[PackageSpec]]]:
    manifests: dict[PackageSpec, dict[str, Any]] = {}
    deps: dict[PackageSpec, list[PackageSpec]] = {}
    stack = list(seeds)
    while stack:
        spec = stack.pop()
        if spec in manifests:
            continue
        doc = _load_local_manifest(root, spec)
        manifests[spec] = doc
        req = _requires_packages(doc)
        deps[spec] = req
        for dep in req:
            stack.append(dep)
    return manifests, deps


def _topo_sort(nodes: set[PackageSpec], deps: dict[PackageSpec, list[PackageSpec]]) -> list[PackageSpec]:
    visiting: set[PackageSpec] = set()
    visited: set[PackageSpec] = set()
    out: list[PackageSpec] = []

    def visit(n: PackageSpec) -> None:
        if n in visited:
            return
        if n in visiting:
            _die(f"ERROR: dependency cycle detected at {n}")
        visiting.add(n)
        for d in deps.get(n, []):
            if d in nodes:
                visit(d)
        visiting.remove(n)
        visited.add(n)
        out.append(n)

    for n in sorted(nodes, key=lambda s: s.sort_key()):
        visit(n)
    return out


def _registry_versions_by_name(*, api_base: str, name: str) -> dict[str, dict[str, Any]]:
    base = api_base.rstrip("/")
    url = f"{base}/packages/{name}"
    code, doc = _curl_get_json(url)
    if code == 404:
        return {}
    if code != 200:
        _die(f"ERROR: registry API failed: HTTP {code}: {url}")
    if not isinstance(doc, dict):
        _die(f"ERROR: registry API returned non-object JSON for {url}")
    versions = doc.get("versions") or []
    if not isinstance(versions, list):
        _die(f"ERROR: registry API returned invalid versions list for {url}")
    out: dict[str, dict[str, Any]] = {}
    for v in versions:
        if not isinstance(v, dict):
            continue
        ver = v.get("version")
        if isinstance(ver, str) and ver:
            out[ver] = v
    return out


def _find_x07_bin(root: Path) -> Path:
    script = root / "scripts" / "ci" / "find_x07.sh"
    if not script.is_file():
        _die(f"ERROR: missing helper: {script.relative_to(root)}")
    res = subprocess.run(
        [str(script)],
        cwd=str(root),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if res.returncode != 0:
        _die(f"ERROR: find_x07.sh failed:\nstdout:\n{res.stdout}\nstderr:\n{res.stderr}")
    out = (res.stdout or "").strip()
    if not out:
        _die("ERROR: find_x07.sh produced empty output")
    p = Path(out)
    if not p.is_absolute():
        p = root / p
    return p.resolve()


def _module_roots_for_closure(root: Path, manifests: dict[PackageSpec, dict[str, Any]], closure: set[PackageSpec]) -> list[Path]:
    roots: list[Path] = []
    seen: set[Path] = set()
    for spec in sorted(closure, key=lambda s: s.sort_key()):
        doc = manifests.get(spec)
        if doc is None:
            doc = _load_local_manifest(root, spec)
        module_root = doc.get("module_root")
        if not isinstance(module_root, str) or not module_root.strip():
            _die(f"ERROR: {spec}: missing/invalid module_root in x07-package.json")
        p = (_local_pkg_dir(root, spec) / module_root).resolve()
        if p in seen:
            continue
        seen.add(p)
        roots.append(p)
    return roots


def _closure_for_spec(seed: PackageSpec, deps: dict[PackageSpec, list[PackageSpec]]) -> set[PackageSpec]:
    out: set[PackageSpec] = set()
    stack = [seed]
    while stack:
        n = stack.pop()
        if n in out:
            continue
        out.add(n)
        for d in deps.get(n, []):
            stack.append(d)
    return out


def _latest_by_name(specs: Iterable[PackageSpec], name: str) -> PackageSpec | None:
    best: PackageSpec | None = None
    for spec in specs:
        if spec.name != name:
            continue
        if best is None or spec.sort_key() > best.sort_key():
            best = spec
    return best


def _extra_test_closure_for_spec(
    *,
    spec: PackageSpec,
    manifests: dict[PackageSpec, dict[str, Any]],
    deps: dict[PackageSpec, list[PackageSpec]],
) -> set[PackageSpec]:
    if spec.name == "ext-json-rs":
        dm = _latest_by_name(manifests.keys(), "ext-data-model")
        if dm is None:
            _die(f"ERROR: missing local manifest for ext-data-model (required to test {spec})")
        return _closure_for_spec(dm, deps)
    return set()


def _run_preflight(root: Path, python_bin: str) -> None:
    changes, _changed_projects = _sync_ext_package_versions(
        root=root,
        rel_roots=["docs/examples/agent-gate", "ci/fixtures/agent-scenarios"],
        write=False,
        python_bin=python_bin,
    )
    if changes:
        lines = ["ERROR: ext package pins/lockfiles are out of date."]
        lines.extend(f"  {line}" for line in changes)
        lines.append("")
        lines.append("hint: python3 scripts/publish_ext_packages.py sync --write")
        _die("\n".join(lines), code=1)

    jobs: list[list[str]] = [
        [python_bin, "scripts/check_pkg_contracts.py", "--check"],
        ["bash", "scripts/ci/check_external_packages_lock.sh"],
        [python_bin, "scripts/ci/check_package_manifests.py"],
        [python_bin, "scripts/ci/check_capabilities_catalog.py"],
        [python_bin, "scripts/ci/check_package_policy.py"],
    ]
    for cmd in jobs:
        _run(cmd, cwd=root)


def _run_x07_parens_for_dirs(root: Path, python_bin: str, dirs: list[Path]) -> None:
    # check_x07_parens.py enforces canonical formatting with the current x07c formatter.
    # This is intentionally scoped to the package module roots we are about to publish to
    # avoid failing on older, already-published package versions.
    if not dirs:
        return
    chunk: list[str] = []
    for p in dirs:
        chunk.append(str(p))
        if len(chunk) >= 32:
            _run([python_bin, "scripts/check_x07_parens.py", *chunk], cwd=root)
            chunk = []
    if chunk:
        _run([python_bin, "scripts/check_x07_parens.py", *chunk], cwd=root)


def _publish_main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Publish missing ext package versions to a registry index.")
    ap.add_argument(
        "--index",
        default="sparse+https://registry.x07.io/index/",
        help="Sparse index base URL (default: official registry).",
    )
    ap.add_argument(
        "--package",
        dest="packages",
        action="append",
        default=[],
        help="Explicit package spec NAME@VERSION to publish (may be repeated). Overrides capabilities.json selection.",
    )
    ap.add_argument(
        "--check",
        action="store_true",
        help="Check for missing versions and exit non-zero if any are missing; do not publish.",
    )
    ap.add_argument(
        "--no-preflight",
        action="store_true",
        help="Skip offline preflight checks (formatting/contracts/locks/policy).",
    )
    ap.add_argument(
        "--no-tests",
        action="store_true",
        help="Skip per-package `x07 test` runs before publishing.",
    )
    args = ap.parse_args(argv)

    root = _repo_root()
    index_url = _normalize_index_url(args.index)
    idx_cfg = _load_index_config(index_url)
    api = idx_cfg.get("api")
    if not isinstance(api, str) or not api.strip():
        _die("ERROR: index config missing non-empty 'api' field")
    auth_required = bool(idx_cfg.get("auth-required"))
    if auth_required and not _has_token_for_index(index_url):
        creds = _credentials_path()
        _die(
            "ERROR: registry auth is required, but no token is configured for this index.\n"
            f"  index: {index_url}\n"
            f"  credentials: {creds}\n"
            f"  hint: x07 pkg login --index {index_url}\n"
        )

    python_bin = _pick_python(root)
    if not args.no_preflight:
        _run_preflight(root, python_bin)

    if args.packages:
        targets = {_parse_pkg_spec(s, context="--package") for s in args.packages}
    else:
        targets = _capability_targets(root)
        if not targets:
            _die("ERROR: capabilities.json did not yield any package targets")

    manifests, deps = _collect_closure(root, targets)

    published_by_name: dict[str, set[str]] = {}
    for spec in sorted(manifests.keys(), key=lambda s: s.sort_key()):
        if spec.name not in published_by_name:
            vers = _registry_versions_by_name(api_base=api, name=spec.name)
            published_by_name[spec.name] = set(vers.keys())

    missing: set[PackageSpec] = set()
    for spec in manifests.keys():
        published = published_by_name.get(spec.name, set())
        if spec.version not in published:
            missing.add(spec)

    if not missing:
        print("ok: no missing package versions")
        return 0

    missing_sorted = sorted(missing, key=lambda s: s.sort_key())
    print("missing versions:")
    for spec in missing_sorted:
        print(f"- {spec}")

    if args.check:
        return 1

    if not args.no_preflight:
        module_roots = _module_roots_for_closure(root, manifests, missing)
        _run_x07_parens_for_dirs(root, python_bin, module_roots)

    publish_order = _topo_sort(missing, deps)

    x07_bin = _find_x07_bin(root)
    for spec in publish_order:
        pkg_dir = _local_pkg_dir(root, spec)

        if not args.no_tests:
            tests_manifest = pkg_dir / "tests" / "tests.json"
            if tests_manifest.is_file():
                closure = _closure_for_spec(spec, deps)
                closure |= _extra_test_closure_for_spec(
                    spec=spec,
                    manifests=manifests,
                    deps=deps,
                )
                module_roots = _module_roots_for_closure(root, manifests, closure)
                cmd = [
                    str(x07_bin),
                    "test",
                    "--manifest",
                    str(tests_manifest),
                    "--json",
                    "false",
                ]
                for mr in module_roots:
                    cmd.extend(["--module-root", str(mr)])
                _run(cmd, cwd=root)

        pub = _run(
            [
                str(x07_bin),
                "pkg",
                "publish",
                "--index",
                index_url,
                "--package",
                str(pkg_dir),
            ],
            cwd=root,
        )
        try:
            report = json.loads(pub.stdout)
        except Exception as e:
            _die(f"ERROR: pkg publish output was not JSON for {spec}: {e}\n{pub.stdout}")
        if not isinstance(report, dict) or report.get("ok") is not True:
            _die(f"ERROR: pkg publish failed for {spec}:\n{pub.stdout}\n{pub.stderr}")
        cksum = (report.get("result") or {}).get("cksum")
        if not isinstance(cksum, str) or len(cksum) != 64:
            _die(f"ERROR: pkg publish response missing cksum for {spec}:\n{pub.stdout}")

        # Verify via the registry API (bypasses sparse index caching).
        versions = _registry_versions_by_name(api_base=api, name=spec.name)
        vinfo = versions.get(spec.version)
        if not isinstance(vinfo, dict):
            _die(f"ERROR: publish did not become visible in registry API for {spec}")
        api_cksum = vinfo.get("cksum")
        if api_cksum != cksum:
            _die(
                f"ERROR: registry cksum mismatch for {spec}: publish={cksum} api={api_cksum!r}"
            )

        print(f"published: {spec}")
        time.sleep(0.2)

    print("ok: published all missing versions")
    return 0


def _sync_main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(
        prog="publish_ext_packages.py sync",
        description="Sync ext package version pins (capabilities/cli/examples) to the latest checked-in ext versions.",
    )
    ap.add_argument(
        "--roots",
        nargs="*",
        default=["docs/examples/agent-gate", "ci/fixtures/agent-scenarios"],
        help="Relative directories to scan for x07.json files.",
    )
    ap.add_argument(
        "--write",
        action="store_true",
        help="Write changes (default is check-only).",
    )
    args = ap.parse_args(argv)

    root = _repo_root()
    python_bin = _pick_python(root)
    changes, _changed_projects = _sync_ext_package_versions(
        root=root,
        rel_roots=list(args.roots),
        write=bool(args.write),
        python_bin=python_bin,
    )

    if changes and not args.write:
        for line in changes:
            print(line, file=sys.stderr)
        print(
            "ERROR: ext package pins/lockfiles are out of date (re-run with `python3 scripts/publish_ext_packages.py sync --write`)",
            file=sys.stderr,
        )
        return 1

    if not changes:
        print("ok: ext package pins/lockfiles are up to date")
    else:
        print("ok: wrote ext package pins/lockfiles")
    return 0


def main(argv: list[str]) -> int:
    if argv and argv[0] == "sync":
        return _sync_main(argv[1:])
    return _publish_main(argv)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
