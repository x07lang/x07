from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path

import pytest

from x07_genpack import GenpackClient
from x07_genpack.error_codes import (
    X07_GENPACK_E_HASH_MISMATCH,
    X07_GENPACK_E_SCHEMA_JSON_PARSE,
    X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH,
)
from x07_genpack.errors import GenpackError


def _x07_bin() -> str:
    return os.environ.get("X07_BIN", "x07")


def _ensure_x07_available() -> None:
    proc = subprocess.run([_x07_bin(), "--version"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if proc.returncode != 0:
        pytest.skip("x07 binary is not available for integration tests")


def _fresh_dir(name: str) -> Path:
    root = Path("target") / "sdk-genpack-py-tests" / name
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)
    return root


@pytest.mark.integration
def test_cli_source_parses_schema_and_bundle() -> None:
    _ensure_x07_available()
    client = GenpackClient(source={"kind": "cli", "x07Path": _x07_bin()})

    schema = client.get_x07ast_schema()
    bundle = client.get_x07ast_grammar_bundle()

    assert schema.json["$id"] == "https://x07.io/spec/x07ast.schema.json"
    assert bundle.schema_version == "x07.ast.grammar_bundle@0.1.0"
    assert "min" in bundle.variants
    assert bundle.variants["min"].cfg.startswith("root ::= ")


@pytest.mark.integration
def test_dir_mode_matches_cli_mode() -> None:
    _ensure_x07_available()
    out_dir = _fresh_dir("dir_mode_matches")

    cli = GenpackClient(source={"kind": "cli", "x07Path": _x07_bin()})
    cli.materialize(out_dir)

    dir_client = GenpackClient(source={"kind": "dir", "dir": str(out_dir)})
    cli_pack = cli.get_x07ast_genpack()
    dir_pack = dir_client.get_x07ast_genpack()

    assert cli_pack.schema.sha256_hex == dir_pack.schema.sha256_hex
    assert cli_pack.grammar.variants["min"].sha256_hex == dir_pack.grammar.variants["min"].sha256_hex
    assert cli_pack.grammar.semantic_supplement.sha256_hex == dir_pack.grammar.semantic_supplement.sha256_hex


@pytest.mark.integration
def test_dir_mode_rejects_semantic_version_mismatch() -> None:
    _ensure_x07_available()
    out_dir = _fresh_dir("semantic_version_mismatch")

    cli = GenpackClient(source={"kind": "cli", "x07Path": _x07_bin()})
    cli.materialize(out_dir)

    semantic_path = out_dir / "x07ast.semantic.json"
    semantic = json.loads(semantic_path.read_text(encoding="utf-8"))
    semantic["schema_version"] = "x07.x07ast.semantic@999.0.0"
    semantic_path.write_text(json.dumps(semantic), encoding="utf-8")

    client = GenpackClient(source={"kind": "dir", "dir": str(out_dir)}, strict=True)
    with pytest.raises(GenpackError) as exc_info:
        _ = client.get_x07ast_grammar_bundle()
    assert exc_info.value.code == X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH


@pytest.mark.integration
def test_dir_mode_rejects_invalid_schema_json() -> None:
    _ensure_x07_available()
    out_dir = _fresh_dir("invalid_schema_json")

    cli = GenpackClient(source={"kind": "cli", "x07Path": _x07_bin()})
    cli.materialize(out_dir)

    (out_dir / "x07ast.schema.json").write_text("{broken", encoding="utf-8")

    client = GenpackClient(source={"kind": "dir", "dir": str(out_dir)}, strict=True)
    with pytest.raises(GenpackError) as exc_info:
        _ = client.get_x07ast_schema()
    assert exc_info.value.code == X07_GENPACK_E_SCHEMA_JSON_PARSE


@pytest.mark.integration
def test_dir_mode_rejects_manifest_hash_mismatch() -> None:
    _ensure_x07_available()
    out_dir = _fresh_dir("manifest_hash_mismatch")

    cli = GenpackClient(source={"kind": "cli", "x07Path": _x07_bin()})
    cli.materialize(out_dir)

    manifest_path = out_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    for artifact in manifest.get("artifacts", []):
        if artifact.get("name") == "x07ast.min.gbnf":
            artifact["sha256"] = "0" * 64
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    client = GenpackClient(source={"kind": "dir", "dir": str(out_dir)}, strict=True)
    with pytest.raises(GenpackError) as exc_info:
        _ = client.get_x07ast_grammar_bundle()
    assert exc_info.value.code == X07_GENPACK_E_HASH_MISMATCH
