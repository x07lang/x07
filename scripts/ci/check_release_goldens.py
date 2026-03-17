#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PYTHON = sys.executable
RELEASE_SCRIPTS = ROOT / "scripts" / "release"
TARGETS = [
    ("aarch64-apple-darwin", ".tar.gz"),
    ("aarch64-unknown-linux-gnu", ".tar.gz"),
    ("x86_64-apple-darwin", ".tar.gz"),
    ("x86_64-pc-windows-msvc", ".zip"),
    ("x86_64-unknown-linux-gnu", ".tar.gz"),
]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def render_json(doc: object) -> str:
    return json.dumps(doc, indent=2, sort_keys=True) + "\n"


def run_python(script: Path, *args: str) -> None:
    subprocess.run(
        [PYTHON, str(script), *args],
        cwd=ROOT,
        check=True,
        text=True,
        capture_output=True,
    )


def asset_bytes(name: str) -> bytes:
    return f"ASSET:{name}\n".encode("utf-8")


def write_assets(dist_dir: Path, names: list[str]) -> None:
    for name in names:
        path = dist_dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(asset_bytes(name))


def checksums_text(dist_dir: Path, out_name: str) -> str:
    lines: list[str] = []
    for path in sorted(dist_dir.iterdir(), key=lambda x: x.name):
        if not path.is_file():
            continue
        if path.name == out_name or path.name.endswith(".tmp"):
            continue
        if path.name.endswith("-release.json") or path.name.endswith("-bundle.json"):
            continue
        lines.append(f"{sha256_file(path)}  {path.name}")
    return "\n".join(lines) + "\n"


def release_asset(repo: str, tag: str, path: Path, kind: str, target: str | None = None) -> dict[str, object]:
    doc: dict[str, object] = {
        "name": path.name,
        "kind": kind,
        "url": f"{repo}/releases/download/{tag}/{path.name}",
        "sha256": f"sha256:{sha256_file(path)}",
        "bytes_len": path.stat().st_size,
    }
    if target is not None:
        doc["target"] = target
    return doc


def write_json(path: Path, doc: object) -> None:
    path.write_text(render_json(doc), encoding="utf-8")


def assert_text(path: Path, expected: str) -> None:
    actual = path.read_text(encoding="utf-8")
    if actual != expected:
        raise SystemExit(f"golden mismatch: {path}")


def assert_contains(path: Path, needle: str) -> None:
    actual = path.read_text(encoding="utf-8")
    if needle not in actual:
        raise SystemExit(f"missing expected text in {path}: {needle!r}")


def formal_verification_release_fixture() -> None:
    check_all = ROOT / "scripts" / "ci" / "check_all.sh"
    for needle in [
        "./scripts/ci/check_verified_core_pure_example.sh",
        "./scripts/ci/check_verified_core_fixture.sh",
        "./scripts/ci/check_trusted_sandbox_program_example.sh",
        "./scripts/ci/check_trusted_network_service_example.sh",
    ]:
        assert_contains(check_all, needle)

    for script in [
        ROOT / "scripts" / "ci" / "check_verified_core_pure_example.sh",
        ROOT / "scripts" / "ci" / "check_verified_core_fixture.sh",
        ROOT / "scripts" / "ci" / "check_trusted_sandbox_program_example.sh",
        ROOT / "scripts" / "ci" / "check_trust_network_example.sh",
    ]:
        assert_contains(script, "assert_strict_certificate.py")
        assert_contains(script, "X07_REVIEW_ARTIFACTS_DIR")

    release_workflow = ROOT / ".github" / "workflows" / "release.yml"
    for needle in [
        "X07_REVIEW_ARTIFACTS_DIR",
        "X07_FORMAL_PERF_REPORT_OUT",
        "formal-verification-review",
        "formal-verification-review-vm",
        "check_trusted_sandbox_program_example.sh",
        "check_trusted_network_service_example.sh",
    ]:
        assert_contains(release_workflow, needle)


def core_fixture(tmp_dir: Path) -> None:
    version = "0.1.52"
    tag = f"v{version}"
    dist_dir = tmp_dir / "core-dist"
    dist_dir.mkdir()
    toolchain_names = [f"x07-{version}-{target}{ext}" for target, ext in TARGETS]
    x07up_names = [f"x07up-v{version}-{target}{ext}" for target, ext in TARGETS]
    extra_names = [
        f"x07-{version}-attestations.jsonl",
    ]
    write_assets(dist_dir, toolchain_names + x07up_names + extra_names)

    checksums_name = f"x07-{version}-checksums.txt"
    checksums_path = dist_dir / checksums_name
    run_python(
        RELEASE_SCRIPTS / "gen-checksums.py",
        "--in-dir",
        str(dist_dir),
        "--out",
        str(checksums_path),
    )
    assert_text(checksums_path, checksums_text(dist_dir, checksums_name))

    compat_path = tmp_dir / "core-compat.json"
    compat_doc = {
        "device_host": "0.1.0",
        "std_web_ui": "0.1.5",
        "x07_core": ">=0.1.52,<0.1.53",
        "x07_wasm": "0.1.0",
    }
    write_json(compat_path, compat_doc)

    release_path = dist_dir / f"x07-{version}-release.json"
    run_python(
        RELEASE_SCRIPTS / "gen-component-release.py",
        "--schema",
        "x07.component.release@0.1.0",
        "--component",
        "x07_core",
        "--version",
        version,
        "--tag",
        tag,
        "--repo",
        "https://github.com/x07lang/x07",
        "--assets-dir",
        str(dist_dir),
        "--compat-file",
        str(compat_path),
        "--published-at-utc",
        "2026-03-05T00:00:00Z",
        "--out",
        str(release_path),
    )
    expected_release = {
        "schema_version": "x07.component.release@0.1.0",
        "component": "x07_core",
        "version": version,
        "tag": tag,
        "repo": "https://github.com/x07lang/x07",
        "published_at_utc": "2026-03-05T00:00:00Z",
        "assets": sorted(
            [
                release_asset("https://github.com/x07lang/x07", tag, dist_dir / name, "archive", target)
                for name, (target, _ext) in zip(toolchain_names, TARGETS, strict=True)
            ]
            + [
                release_asset(
                    "https://github.com/x07lang/x07",
                    tag,
                    dist_dir / extra_names[0],
                    "attestations",
                ),
                release_asset(
                    "https://github.com/x07lang/x07",
                    tag,
                    checksums_path,
                    "checksums",
                ),
            ]
            + [
                release_asset("https://github.com/x07lang/x07", tag, dist_dir / name, "installer_archive", target)
                for name, (target, _ext) in zip(x07up_names, TARGETS, strict=True)
            ],
            key=lambda asset: (str(asset["kind"]), str(asset["name"])),
        ),
        "compatibility": compat_doc,
    }
    assert_text(release_path, render_json(expected_release))

    bundle_input_path = tmp_dir / "bundle-input.json"
    bundle_input_doc = {
        "published_at_utc": "2026-03-05T00:20:00Z",
        "min_x07up_version": version,
        "packages": {"std_web_ui": "0.1.5"},
        "x07_wasm": {
            "version": "0.1.0",
            "tag": "v0.1.0",
            "release_manifest_url": "https://github.com/x07lang/x07-wasm-backend/releases/download/v0.1.0/x07-wasm-0.1.0-release.json",
            "release_manifest_sha256": "sha256:" + ("1" * 64),
        },
        "x07_web_ui_host": {
            "version": "0.1.5",
            "tag": "v0.1.5",
            "release_manifest_url": "https://github.com/x07lang/x07-web-ui/releases/download/v0.1.5/x07-web-ui-host-0.1.5-release.json",
            "release_manifest_sha256": "sha256:" + ("2" * 64),
        },
        "x07_device_host": {
            "version": "0.1.0",
            "tag": "v0.1.0",
            "release_manifest_url": "https://github.com/x07lang/x07-device-host/releases/download/v0.1.0/x07-device-host-0.1.0-release.json",
            "release_manifest_sha256": "sha256:" + ("3" * 64),
        },
    }
    write_json(bundle_input_path, bundle_input_doc)
    bundle_path = dist_dir / f"x07-{version}-bundle.json"
    run_python(
        RELEASE_SCRIPTS / "gen-bundle.py",
        "--schema",
        "x07.release.bundle@0.1.0",
        "--channel",
        "stable",
        "--input",
        str(bundle_input_path),
        "--core-manifest",
        str(release_path),
        "--out",
        str(bundle_path),
    )
    expected_bundle = {
        "schema_version": "x07.release.bundle@0.1.0",
        "channel": "stable",
        "published_at_utc": "2026-03-05T00:20:00Z",
        "min_x07up_version": version,
        "x07_core": {
            "version": version,
            "tag": tag,
            "release_manifest_url": f"https://github.com/x07lang/x07/releases/download/{tag}/{release_path.name}",
            "release_manifest_sha256": f"sha256:{sha256_file(release_path)}",
        },
        "x07_wasm": bundle_input_doc["x07_wasm"],
        "x07_web_ui_host": bundle_input_doc["x07_web_ui_host"],
        "x07_device_host": bundle_input_doc["x07_device_host"],
        "packages": {"std_web_ui": "0.1.5"},
    }
    assert_text(bundle_path, render_json(expected_bundle))


def component_fixture(
    tmp_dir: Path,
    *,
    component: str,
    base_name: str,
    version: str,
    repo: str,
    compat_doc: dict[str, str],
    asset_names: list[str],
    published_at_utc: str,
    metadata_items: list[str] | None = None,
) -> None:
    dist_dir = tmp_dir / f"{component}-dist"
    dist_dir.mkdir()
    attestations_name = f"{base_name}{version}-attestations.jsonl"
    write_assets(dist_dir, asset_names + [attestations_name])
    checksums_name = f"{base_name}{version}-checksums.txt"
    checksums_path = dist_dir / checksums_name
    run_python(
        RELEASE_SCRIPTS / "gen-checksums.py",
        "--in-dir",
        str(dist_dir),
        "--out",
        str(checksums_path),
    )
    assert_text(checksums_path, checksums_text(dist_dir, checksums_name))

    compat_path = tmp_dir / f"{component}-compat.json"
    write_json(compat_path, compat_doc)

    out_name = f"{base_name}{version}-release.json"
    release_path = dist_dir / out_name
    cmd = [
        "--schema",
        "x07.component.release@0.1.0",
        "--component",
        component,
        "--version",
        version,
        "--tag",
        f"v{version}",
        "--repo",
        repo,
        "--assets-dir",
        str(dist_dir),
        "--compat-file",
        str(compat_path),
        "--published-at-utc",
        published_at_utc,
        "--out",
        str(release_path),
    ]
    for item in metadata_items or []:
        cmd.extend(["--metadata", item])
    run_python(RELEASE_SCRIPTS / "gen-component-release.py", *cmd)

    expected_assets: list[dict[str, object]] = []
    for name in asset_names:
        target = None
        if name.endswith((".tar.gz", ".zip")) and not name.startswith("x07-web-ui-host-") and not name.startswith("x07-device-host-mobile-templates-"):
            for candidate, _ext in TARGETS:
                if candidate in name:
                    target = candidate
                    break
        expected_assets.append(
            release_asset(
                repo,
                f"v{version}",
                dist_dir / name,
                {
                    "x07_wasm": "archive",
                    "x07_web_ui_host": "host_bundle",
                    "x07_device_host": "archive",
                }.get(component, "archive"),
                target,
            )
        )
    attest_path = dist_dir / attestations_name
    expected_assets.append(release_asset(repo, f"v{version}", attest_path, "attestations"))
    expected_assets.append(release_asset(repo, f"v{version}", checksums_path, "checksums"))
    if component == "x07_device_host":
        templates = dist_dir / f"x07-device-host-mobile-templates-{version}.zip"
        abi = dist_dir / f"x07-device-host-abi-{version}.json"
        for asset in expected_assets:
            if asset["name"] == templates.name:
                asset["kind"] = "templates"
                asset.pop("target", None)
            if asset["name"] == abi.name:
                asset["kind"] = "abi_snapshot"
                asset.pop("target", None)
    expected_doc: dict[str, object] = {
        "schema_version": "x07.component.release@0.1.0",
        "component": component,
        "version": version,
        "tag": f"v{version}",
        "repo": repo,
        "published_at_utc": published_at_utc,
        "assets": sorted(expected_assets, key=lambda asset: (str(asset["kind"]), str(asset["name"]))),
        "compatibility": compat_doc,
    }
    if metadata_items:
        metadata: dict[str, str] = {}
        for item in metadata_items:
            key, value = item.split("=", 1)
            metadata[key] = value
        expected_doc["metadata"] = metadata
    assert_text(release_path, render_json(expected_doc))


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="x07-release-goldens-") as tmp:
        tmp_dir = Path(tmp)
        core_fixture(tmp_dir)
        component_fixture(
            tmp_dir,
            component="x07_wasm",
            base_name="x07-wasm-",
            version="0.1.0",
            repo="https://github.com/x07lang/x07-wasm-backend",
            compat_doc={
                "device_host": "0.1.0",
                "std_web_ui": "0.1.5",
                "x07_core": ">=0.1.52,<0.1.53",
            },
            asset_names=[f"x07-wasm-0.1.0-{target}{ext}" for target, ext in TARGETS],
            published_at_utc="2026-03-05T00:05:00Z",
            metadata_items=[
                "host_abi_hash=sha256:" + ("4" * 64),
                "package_name=std-web-ui",
                "package_version=0.1.5",
            ],
        )
        component_fixture(
            tmp_dir,
            component="x07_web_ui_host",
            base_name="x07-web-ui-host-",
            version="0.1.5",
            repo="https://github.com/x07lang/x07-web-ui",
            compat_doc={
                "device_host": "0.1.0",
                "std_web_ui": "0.1.5",
                "x07_core": ">=0.1.52,<0.1.53",
                "x07_wasm": "0.1.0",
            },
            asset_names=["x07-web-ui-host-0.1.5.zip"],
            published_at_utc="2026-03-05T00:10:00Z",
            metadata_items=[
                "host_abi_hash=sha256:" + ("5" * 64),
                "package_name=std-web-ui",
                "package_version=0.1.5",
            ],
        )
        component_fixture(
            tmp_dir,
            component="x07_device_host",
            base_name="x07-device-host-",
            version="0.1.0",
            repo="https://github.com/x07lang/x07-device-host",
            compat_doc={
                "std_web_ui": "0.1.5",
                "x07_core": ">=0.1.52,<0.1.53",
            },
            asset_names=[
                f"x07-device-host-desktop-0.1.0-{target}{ext}" for target, ext in TARGETS
            ]
            + [
                "x07-device-host-mobile-templates-0.1.0.zip",
                "x07-device-host-abi-0.1.0.json",
            ],
            published_at_utc="2026-03-05T00:15:00Z",
            metadata_items=[
                "host_abi_hash=sha256:" + ("6" * 64),
            ],
        )
        formal_verification_release_fixture()
    print("ok: release script goldens")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
