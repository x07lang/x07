#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_FIXTURE = ROOT / "tests" / "fixtures" / "generics_mono_map" / "main.x07.json"
EXPECTED_SCHEMA_VERSION = "x07.mono.map@0.1.0"
MONO_NAME_MARKER = "__x07_mono_v1__"


@dataclass(frozen=True)
class Fixture:
    name: str
    program: Path


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser(description="CI checks for x07c monomorphization map determinism.")
    ap.add_argument(
        "--program",
        type=Path,
        default=DEFAULT_FIXTURE,
        help=f"Path to an x07AST entry program (default: {DEFAULT_FIXTURE.relative_to(ROOT)})",
    )
    ap.add_argument(
        "--x07c",
        type=Path,
        default=None,
        help="Path to x07c binary (default: use `cargo run -q -p x07c --`).",
    )
    ap.add_argument("--verbose", action="store_true")
    return ap.parse_args(argv)


def _json_canon(x: Any) -> str:
    return json.dumps(x, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _fail(msg: str) -> int:
    print(f"ERROR: {msg}", file=sys.stderr)
    return 2


def run_x07c_compile(*, program: Path, mono_map_out: Path, c_out: Path, x07c: Path | None, verbose: bool) -> None:
    if x07c is None:
        cmd = [
            "cargo",
            "run",
            "-q",
            "-p",
            "x07c",
            "--",
            "compile",
            "--program",
            str(program),
            "--world",
            "solve-pure",
            "--emit-mono-map",
            str(mono_map_out),
            "--out",
            str(c_out),
        ]
        cwd = ROOT
    else:
        cmd = [
            str(x07c),
            "compile",
            "--program",
            str(program),
            "--world",
            "solve-pure",
            "--emit-mono-map",
            str(mono_map_out),
            "--out",
            str(c_out),
        ]
        cwd = ROOT

    env = os.environ.copy()
    env.update(
        {
            "RUST_BACKTRACE": "0",
            "CARGO_TERM_COLOR": "never",
        }
    )
    proc = subprocess.run(
        cmd,
        cwd=str(cwd),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if proc.returncode != 0:
        stdout = proc.stdout.strip()
        stderr = proc.stderr.strip()
        if verbose:
            details = f"\nstdout:\n{stdout}\nstderr:\n{stderr}\n"
        else:
            tail = "\n".join([line for line in (stdout + "\n" + stderr).splitlines() if line][-20:])
            details = f"\n(last 20 lines)\n{tail}\n"
        raise RuntimeError(f"x07c compile failed (exit={proc.returncode}){details}")


def validate_mono_map(doc: dict[str, Any], *, fixture_name: str) -> None:
    schema_version = doc.get("schema_version")
    if schema_version != EXPECTED_SCHEMA_VERSION:
        raise ValueError(
            f"{fixture_name}: schema_version mismatch: expected {EXPECTED_SCHEMA_VERSION!r}, got {schema_version!r}"
        )

    limits = doc.get("limits")
    if not isinstance(limits, dict):
        raise ValueError(f"{fixture_name}: limits must be an object")
    max_specializations = limits.get("max_specializations")
    if not isinstance(max_specializations, int):
        raise ValueError(f"{fixture_name}: limits.max_specializations must be an integer")

    items = doc.get("items")
    if not isinstance(items, list):
        raise ValueError(f"{fixture_name}: items must be an array")

    if len(items) > max_specializations:
        raise ValueError(
            f"{fixture_name}: specialization count exceeds cap: {len(items)} > {max_specializations}"
        )

    stats = doc.get("stats")
    if not isinstance(stats, dict):
        raise ValueError(f"{fixture_name}: stats must be an object")
    emitted = stats.get("specializations_emitted")
    if emitted != len(items):
        raise ValueError(
            f"{fixture_name}: stats.specializations_emitted mismatch: expected {len(items)}, got {emitted!r}"
        )

    keys: list[tuple[str, str]] = []
    specialized_names: set[str] = set()
    for idx, item in enumerate(items):
        if not isinstance(item, dict):
            raise ValueError(f"{fixture_name}: items[{idx}] must be an object")
        generic = item.get("generic")
        specialized = item.get("specialized")
        type_args = item.get("type_args")
        if not isinstance(generic, str) or generic == "":
            raise ValueError(f"{fixture_name}: items[{idx}].generic must be a non-empty string")
        if not isinstance(specialized, str) or specialized == "":
            raise ValueError(f"{fixture_name}: items[{idx}].specialized must be a non-empty string")
        if MONO_NAME_MARKER not in specialized:
            raise ValueError(
                f"{fixture_name}: items[{idx}].specialized missing marker {MONO_NAME_MARKER!r}: {specialized!r}"
            )
        if not isinstance(type_args, list):
            raise ValueError(f"{fixture_name}: items[{idx}].type_args must be an array")
        if specialized in specialized_names:
            raise ValueError(f"{fixture_name}: duplicate specialized name: {specialized!r}")
        specialized_names.add(specialized)
        keys.append((generic, _json_canon(type_args)))

    sorted_keys = sorted(keys)
    if keys != sorted_keys:
        for i in range(min(len(keys), len(sorted_keys))):
            if keys[i] != sorted_keys[i]:
                raise ValueError(
                    f"{fixture_name}: items not sorted at index {i}: got={keys[i]!r} expected={sorted_keys[i]!r}"
                )
        raise ValueError(f"{fixture_name}: items not sorted")

    if len(set(keys)) != len(keys):
        raise ValueError(f"{fixture_name}: duplicate items detected (generic + type_args)")


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    program = args.program.resolve()
    if not program.is_file():
        return _fail(f"missing --program file: {program}")

    fixtures = [Fixture(name=program.name, program=program)]

    with TemporaryDirectory(prefix="x07-mono-map-") as tmp:
        tmp_dir = Path(tmp)
        for fixture in fixtures:
            mono_map_path = tmp_dir / f"{fixture.name}.mono_map.json"
            c_out_path = tmp_dir / f"{fixture.name}.out.c"
            try:
                run_x07c_compile(
                    program=fixture.program,
                    mono_map_out=mono_map_path,
                    c_out=c_out_path,
                    x07c=args.x07c,
                    verbose=args.verbose,
                )
            except Exception as e:
                return _fail(str(e))

            try:
                with mono_map_path.open("r", encoding="utf-8") as f:
                    doc = json.load(f)
            except Exception as e:
                return _fail(f"{fixture.name}: parse mono map JSON: {e}")

            try:
                if not isinstance(doc, dict):
                    raise ValueError("top-level must be an object")
                validate_mono_map(doc, fixture_name=fixture.name)
            except Exception as e:
                return _fail(str(e))

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
