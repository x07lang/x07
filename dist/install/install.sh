#!/bin/sh
set -eu

pick_python() {
  if [ -n "${X07_PYTHON:-}" ]; then
    echo "$X07_PYTHON"
    return 0
  fi
  if command -v python3 >/dev/null 2>&1; then
    echo python3
    return 0
  fi
  if command -v python >/dev/null 2>&1; then
    echo python
    return 0
  fi
  return 1
}

PY="$(pick_python || true)"
if [ -z "$PY" ]; then
  echo "ERROR: python is required to run the X07 installer (set X07_PYTHON or install python3)." >&2
  exit 2
fi

exec "$PY" - "$@" <<'PY'
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SCHEMA_VERSION = "x07.install.report@0.1.0"
DEFAULT_CHANNELS_URL = "https://x07lang.org/install/channels.json"


@dataclass
class InstallError(Exception):
    exit_code: int
    code: str
    message: str
    hint: str | None = None


def eprint(msg: str) -> None:
    sys.stderr.write(msg + "\n")


def log(args: argparse.Namespace, msg: str) -> None:
    if args.quiet:
        return
    eprint(msg)


def detect_target() -> str:
    os_name = sys.platform
    machine = os.uname().machine if hasattr(os, "uname") else ""

    if os_name.startswith("linux"):
        if machine == "x86_64":
            return "x86_64-unknown-linux-gnu"
        if machine in ("aarch64", "arm64"):
            return "aarch64-unknown-linux-gnu"
    if os_name == "darwin":
        if machine == "x86_64":
            return "x86_64-apple-darwin"
        if machine in ("aarch64", "arm64"):
            return "aarch64-apple-darwin"
    raise InstallError(2, "ASSET_NOT_FOUND", f"unsupported host for installer: os={os_name} arch={machine}")


def _curl_get_bytes(url: str, timeout_sec: int) -> bytes:
    proc = subprocess.run(
        [
            "curl",
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--max-time",
            str(timeout_sec),
            url,
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        err = proc.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(err or f"curl failed with exit code {proc.returncode}")
    return proc.stdout


def _curl_download_file(url: str, out: Path, timeout_sec: int) -> None:
    proc = subprocess.run(
        [
            "curl",
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--max-time",
            str(timeout_sec),
            "--output",
            str(out),
            url,
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0:
        out.unlink(missing_ok=True)
        err = proc.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(err or f"curl failed with exit code {proc.returncode}")


def _http_get_bytes(url: str, timeout_sec: int) -> bytes:
    errors: list[str] = []
    if shutil.which("curl"):
        try:
            return _curl_get_bytes(url, timeout_sec=timeout_sec)
        except Exception as e:
            errors.append(f"curl: {e}")
    try:
        with urllib.request.urlopen(url, timeout=timeout_sec) as resp:
            return resp.read()
    except urllib.error.URLError as e:
        errors.append(f"python: {e}")
        raise InstallError(10, "NETWORK_ERROR", f"failed to download: {url}", hint="; ".join(errors))


def _download_to_file(url: str, out: Path, timeout_sec: int) -> None:
    errors: list[str] = []
    if shutil.which("curl"):
        try:
            _curl_download_file(url, out, timeout_sec=timeout_sec)
            return
        except Exception as e:
            errors.append(f"curl: {e}")
    try:
        with urllib.request.urlopen(url, timeout=timeout_sec) as resp, out.open("wb") as f:
            shutil.copyfileobj(resp, f, length=1024 * 1024)
    except urllib.error.URLError as e:
        out.unlink(missing_ok=True)
        errors.append(f"python: {e}")
        raise InstallError(10, "NETWORK_ERROR", f"failed to download: {url}", hint="; ".join(errors))


def http_get_json(url: str) -> dict[str, Any]:
    body = _http_get_bytes(url, timeout_sec=60)
    try:
        doc = json.loads(body.decode("utf-8"))
    except Exception as e:
        raise InstallError(10, "NETWORK_ERROR", f"invalid JSON from: {url}", hint=str(e))
    if not isinstance(doc, dict):
        raise InstallError(10, "NETWORK_ERROR", f"channels manifest must be an object: {url}")
    return doc


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def download_file(url: str, out: Path, expected_sha256: str, args: argparse.Namespace) -> None:
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_suffix(out.suffix + ".tmp")
    _download_to_file(url, tmp, timeout_sec=600)
    actual = sha256_file(tmp)
    if actual.lower() != expected_sha256.strip().lower():
        tmp.unlink(missing_ok=True)
        raise InstallError(11, "SHA256_MISMATCH", f"sha256 mismatch for {url}", hint=f"expected={expected_sha256} got={actual}")
    tmp.replace(out)


def safe_extract_tar_gz(archive: Path, out_dir: Path) -> None:
    try:
        with tarfile.open(archive, mode="r:gz") as tf:
            members = tf.getmembers()
            for m in members:
                name = m.name
                dest = (out_dir / name).resolve()
                if not str(dest).startswith(str(out_dir.resolve()) + os.sep):
                    raise InstallError(12, "EXTRACT_FAILED", "tarball contains unsafe path", hint=name)
            tf.extractall(out_dir, filter="data")
    except InstallError:
        raise
    except Exception as e:
        raise InstallError(12, "EXTRACT_FAILED", f"failed to extract tar.gz: {archive}", hint=str(e))


def safe_extract_zip(archive: Path, out_dir: Path) -> None:
    try:
        with zipfile.ZipFile(archive) as zf:
            for name in zf.namelist():
                dest = (out_dir / name).resolve()
                if not str(dest).startswith(str(out_dir.resolve()) + os.sep):
                    raise InstallError(12, "EXTRACT_FAILED", "zip contains unsafe path", hint=name)
            zf.extractall(out_dir)
    except InstallError:
        raise
    except Exception as e:
        raise InstallError(12, "EXTRACT_FAILED", f"failed to extract zip: {archive}", hint=str(e))


def extract_archive(archive: Path, fmt: str, out_dir: Path) -> None:
    if fmt == "tar.gz":
        safe_extract_tar_gz(archive, out_dir)
        return
    if fmt == "zip":
        safe_extract_zip(archive, out_dir)
        return
    raise InstallError(12, "EXTRACT_FAILED", f"unsupported archive format: {fmt}")


def find_file(root: Path, filename: str) -> Path | None:
    for p in root.rglob(filename):
        if p.is_file():
            return p
    return None


def ensure_executable(path: Path) -> None:
    st = path.stat()
    path.chmod(st.st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def detect_profile_path() -> Path:
    home = Path.home()
    shell = os.environ.get("SHELL") or ""
    shell_name = Path(shell).name if shell else ""

    if shell_name == "zsh":
        return home / ".zprofile"
    if shell_name == "bash":
        bash_profile = home / ".bash_profile"
        if bash_profile.exists():
            return bash_profile
    return home / ".profile"


def maybe_modify_path(root: Path, args: argparse.Namespace) -> tuple[bool, str | None]:
    if args.no_modify_path:
        return False, None

    profile = detect_profile_path()
    line = f'export PATH="{root / "bin"}:$PATH"'
    try:
        existing = profile.read_text(encoding="utf-8") if profile.exists() else ""
        if line in existing:
            return False, None
        with profile.open("a", encoding="utf-8") as f:
            if existing and not existing.endswith("\n"):
                f.write("\n")
            f.write("# Added by X07 installer (x07up)\n")
            f.write(line + "\n")
        return True, None
    except Exception as e:
        return False, str(e)


def run_x07up(args: argparse.Namespace, root: Path, channels_url: str, cmd: list[str]) -> subprocess.CompletedProcess[str]:
    exe = root / "bin" / "x07up"
    if not exe.is_file():
        raise InstallError(13, "X07UP_BOOTSTRAP_FAILED", "x07up not installed", hint=str(exe))
    argv = [str(exe), "--root", str(root), "--channels-url", channels_url, "--quiet", "--json"] + cmd
    return subprocess.run(argv, check=False, text=True, capture_output=True)


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser(add_help=False)
    ap.add_argument("--help", "-h", action="store_true")
    ap.add_argument("--yes", "-y", action="store_true")
    ap.add_argument("--root", default=str(Path.home() / ".x07"))
    ap.add_argument("--channel", default="stable")
    ap.add_argument("--toolchain", default="")
    ap.add_argument("--channels-url", default=DEFAULT_CHANNELS_URL)
    ap.add_argument("--profile", default="full", choices=["full", "minimal"])
    ap.add_argument("--no-modify-path", action="store_true")
    ap.add_argument("--quiet", action="store_true")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args(argv)
    if args.help:
        ap.print_help(sys.stdout)
        raise SystemExit(0)
    return args


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    root = Path(args.root).expanduser().resolve()
    target = detect_target()

    report: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "ok": False,
        "root": str(root),
        "bin_dir": str(root / "bin"),
        "channels_url": args.channels_url,
        "channel": args.channel,
        "toolchain": None,
        "x07up": None,
        "profile": args.profile,
        "target": target,
        "modified_path": False,
        "path_hint": f'export PATH="{root / "bin"}:$PATH"',
        "installed": None,
        "warnings": [],
    }

    try:
        log(args, f"fetch channels manifest: {args.channels_url}")
        manifest = http_get_json(args.channels_url)
        if manifest.get("schema_version") != "x07.install.channels@0.1.0":
            raise InstallError(10, "NETWORK_ERROR", "unsupported channels.json schema_version", hint=str(manifest.get("schema_version")))

        channels = manifest.get("channels") or {}
        toolchains = manifest.get("toolchains") or {}
        x07up = manifest.get("x07up") or {}

        if args.toolchain:
            toolchain_tag = args.toolchain
        else:
            if args.channel not in channels:
                raise InstallError(2, "CHANNEL_NOT_FOUND", f"channel not found: {args.channel}")
            toolchain_tag = channels[args.channel]["toolchain"]

        if toolchain_tag not in toolchains:
            raise InstallError(2, "TOOLCHAIN_NOT_FOUND", f"toolchain not found: {toolchain_tag}")

        min_req = (toolchains[toolchain_tag].get("min_required") or {}).get("x07up")
        if min_req:
            x07up_tag = min_req
        else:
            if args.channel not in channels:
                raise InstallError(2, "CHANNEL_NOT_FOUND", f"channel not found: {args.channel}")
            x07up_tag = channels[args.channel]["x07up"]

        if x07up_tag not in x07up:
            raise InstallError(2, "X07UP_NOT_FOUND", f"x07up release not found: {x07up_tag}")

        report["toolchain"] = toolchain_tag
        report["x07up"] = x07up_tag

        x07up_assets = (x07up[x07up_tag].get("assets") or {})
        if target not in x07up_assets:
            raise InstallError(2, "ASSET_NOT_FOUND", "no x07up asset for detected target", hint=f"target={target} available={list(x07up_assets.keys())}")

        x07up_asset = x07up_assets[target]
        x07up_url = x07up_asset["url"]
        x07up_sha = x07up_asset["sha256"]
        x07up_fmt = x07up_asset.get("format") or "tar.gz"

        log(args, f"download x07up: {x07up_url}")
        downloads = root / "cache" / "downloads"
        archive_name = Path(urllib.parse.urlparse(x07up_url).path).name or f"x07up-{x07up_tag}-{target}"
        x07up_archive = downloads / archive_name
        download_file(x07up_url, x07up_archive, x07up_sha, args)

        log(args, "extract x07up")
        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            extract_archive(x07up_archive, x07up_fmt, td_path)
            found = find_file(td_path, "x07up") or find_file(td_path, "x07up.exe")
            if not found:
                raise InstallError(12, "EXTRACT_FAILED", "x07up binary not found in archive", hint=str(x07up_archive))
            (root / "bin").mkdir(parents=True, exist_ok=True)
            dst = root / "bin" / "x07up"
            shutil.copy2(found, dst)
            ensure_executable(dst)

        spec = toolchain_tag if args.toolchain else args.channel
        log(args, f"install toolchain via x07up: {spec}")
        proc = run_x07up(args, root, args.channels_url, ["install", spec, "--profile", args.profile, "--yes"])
        if proc.returncode != 0:
            raise InstallError(14, "TOOLCHAIN_INSTALL_FAILED", "x07up install failed", hint=proc.stdout.strip() or proc.stderr.strip())

        if args.profile == "full":
            log(args, "install skills (user)")
            proc = run_x07up(args, root, args.channels_url, ["skills", "install", "--user"])
            if proc.returncode != 0:
                report["warnings"].append("skills install failed (user)")

        log(args, "verify: x07up + x07 are runnable")
        proc = subprocess.run([str(root / "bin" / "x07up"), "--root", str(root), "show", "--json"], check=False, text=True, capture_output=True)
        if proc.returncode != 0:
            raise InstallError(13, "X07UP_BOOTSTRAP_FAILED", "x07up show failed after install", hint=proc.stderr.strip())

        proc = subprocess.run([str(root / "bin" / "x07"), "--help"], check=False, text=True, capture_output=True)
        if proc.returncode != 0:
            raise InstallError(14, "TOOLCHAIN_INSTALL_FAILED", "x07 not runnable after install", hint=proc.stderr.strip())

        modified, path_warn = maybe_modify_path(root, args)
        report["modified_path"] = modified
        if path_warn:
            report["warnings"].append(f"path modify failed: {path_warn}")

        report["installed"] = {
            "x07up_path": str(root / "bin" / "x07up"),
            "x07_path": str(root / "bin" / "x07"),
            "toolchain_dir": str(root / "toolchains" / toolchain_tag),
            "components": ["docs", "skills"] if args.profile == "full" else [],
        }
        report["ok"] = True

    except InstallError as e:
        report["ok"] = False
        report["error"] = {"code": e.code, "message": e.message, "hint": e.hint}
        if args.json:
            sys.stdout.write(json.dumps(report, sort_keys=True, indent=2) + "\n")
        else:
            eprint(f"ERROR: {e.code}: {e.message}")
            if e.hint:
                eprint(f"hint: {e.hint}")
        return e.exit_code

    if args.json:
        sys.stdout.write(json.dumps(report, sort_keys=True, indent=2) + "\n")
    else:
        print(f"ok: installed X07 to {root}")
        print(report["path_hint"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
PY
