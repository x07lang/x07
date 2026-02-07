from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any, Iterable, Mapping, cast

from .error_codes import (
    X07_GENPACK_E_BUNDLE_SHAPE_INVALID,
    X07_GENPACK_E_CACHE_IO,
    X07_GENPACK_E_DIR_MISSING_ARTIFACT,
    X07_GENPACK_E_DIR_READ_FAILED,
    X07_GENPACK_E_GRAMMAR_BUNDLE_JSON_PARSE,
    X07_GENPACK_E_GRAMMAR_BUNDLE_VERSION_MISMATCH,
    X07_GENPACK_E_HASH_MISMATCH,
    X07_GENPACK_E_MATERIALIZE_IO,
    X07_GENPACK_E_SCHEMA_JSON_PARSE,
    X07_GENPACK_E_SCHEMA_VERSION_MISMATCH,
    X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH,
    X07_GENPACK_E_STDOUT_NOT_UTF8,
    X07_GENPACK_E_SUBPROCESS_FAILED,
    X07_GENPACK_E_SUBPROCESS_TIMEOUT,
    X07_GENPACK_E_VARIANT_MISSING,
    X07_GENPACK_E_X07_NOT_FOUND,
)
from .errors import GenpackError
from .types import (
    CacheEntry,
    CliSource,
    DirSource,
    GenpackSource,
    GrammarBundle,
    GrammarVariant,
    JsonDoc,
    ToolchainInfo,
    X07AstGenpack,
)


EXPECTED_GRAMMAR_BUNDLE_VERSION = "x07.ast.grammar_bundle@0.1.0"
EXPECTED_SEMANTIC_VERSION = "x07.x07ast.semantic@0.1.0"
EXPECTED_SCHEMA_ID = "https://x07.io/spec/x07ast.schema.json"
REQUIRED_DIR_ARTIFACTS = (
    "x07ast.schema.json",
    "x07ast.min.gbnf",
    "x07ast.pretty.gbnf",
    "x07ast.semantic.json",
)


def _sha256_hex_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _sha256_hex_text(data: str) -> str:
    return _sha256_hex_bytes(data.encode("utf-8"))


def _preview(text: str, limit: int = 160) -> str:
    return text if len(text) <= limit else (text[:limit] + "...")


class GenpackClient:
    def __init__(
        self,
        source: GenpackSource | None = None,
        timeout_s: float = 30.0,
        cache_dir: Path | None = None,
        strict: bool = True,
    ):
        self._source = self._normalize_source(source)
        self._timeout_s = timeout_s
        self._cache_dir = cache_dir
        self._strict = strict

    def get_x07ast_schema(self) -> JsonDoc[dict[str, Any]]:
        if self._source["kind"] == "dir":
            dir_source = cast(DirSource, self._source)
            path = Path(dir_source["dir"]) / "x07ast.schema.json"
            raw = self._read_dir_text(path)
            return self._parse_schema_doc(raw, argv=[str(path)])

        raw = self._run_x07(["ast", "schema", "--json-schema"])
        return self._parse_schema_doc(raw, argv=["ast", "schema", "--json-schema"])

    def get_x07ast_grammar_bundle(self) -> GrammarBundle:
        if self._source["kind"] == "dir":
            dir_source = cast(DirSource, self._source)
            return self._bundle_from_dir(Path(dir_source["dir"]))

        raw = self._run_x07(["ast", "grammar", "--cfg"])
        return self._parse_grammar_bundle_doc(raw, argv=["ast", "grammar", "--cfg"])

    def get_x07ast_genpack(self) -> X07AstGenpack:
        cache_entry = self._cache_entry()
        if cache_entry is not None:
            cached = self._read_cache(cache_entry)
            if cached is not None:
                return cached

        schema = self.get_x07ast_schema()
        grammar = self.get_x07ast_grammar_bundle()
        self._check_schema_alignment(schema=schema, grammar=grammar)

        out = X07AstGenpack(
            toolchain=ToolchainInfo(
                x07_path=self._x07_path(),
                x07_version=self._discover_toolchain_version(),
            ),
            schema=schema,
            grammar=grammar,
        )

        if cache_entry is not None:
            self._write_cache(cache_entry, out)

        return out

    def materialize(self, out_dir: Path) -> None:
        out_dir = out_dir.resolve()
        try:
            out_dir.mkdir(parents=True, exist_ok=True)
        except OSError as exc:
            raise GenpackError(
                X07_GENPACK_E_MATERIALIZE_IO,
                "failed to create output directory",
                data={"out_dir": str(out_dir), "io_error": str(exc)},
            ) from exc

        if self._source["kind"] == "dir":
            dir_source = cast(DirSource, self._source)
            src_dir = Path(dir_source["dir"]).resolve()
            try:
                for name in (*REQUIRED_DIR_ARTIFACTS, "manifest.json"):
                    src = src_dir / name
                    if src.exists():
                        shutil.copyfile(src, out_dir / name)
            except OSError as exc:
                raise GenpackError(
                    X07_GENPACK_E_MATERIALIZE_IO,
                    "failed to copy materialized artifacts",
                    data={"out_dir": str(out_dir), "io_error": str(exc)},
                ) from exc
            return

        _ = self._run_x07(["ast", "grammar", "--cfg", "--out-dir", str(out_dir)])

    def compile_for_xgrammar(self, tokenizer_info: Any, *, prefer: str = "schema") -> Any:
        try:
            import xgrammar as xgr  # type: ignore[import-not-found]
        except Exception as exc:  # pragma: no cover - optional dependency path
            raise RuntimeError("xgrammar is not installed; install with extras: x07-genpack[xgrammar]") from exc

        genpack = self.get_x07ast_genpack()
        compiler = xgr.GrammarCompiler(tokenizer_info=tokenizer_info)
        if prefer == "grammar":
            variant = genpack.grammar.variants.get("min")
            if variant is None:
                raise GenpackError(
                    X07_GENPACK_E_VARIANT_MISSING,
                    "required grammar variant is missing",
                    data={"variant": "min", "available_variants": sorted(genpack.grammar.variants.keys())},
                )
            return compiler.compile_grammar(variant.cfg)
        return compiler.compile_json_schema(genpack.schema.raw)

    @staticmethod
    def _normalize_source(source: GenpackSource | None) -> GenpackSource:
        if source is None:
            return cast(GenpackSource, {"kind": "cli"})
        kind = source.get("kind") if isinstance(source, dict) else None
        if kind == "dir":
            dir_source = cast(DirSource, source)
            return {"kind": "dir", "dir": dir_source["dir"]}
        cli_source = cast(CliSource, source)
        normalized: CliSource = {"kind": "cli"}
        if "x07Path" in cli_source:
            normalized["x07Path"] = cli_source["x07Path"]
        if "cwd" in cli_source:
            normalized["cwd"] = cli_source["cwd"]
        if "env" in cli_source:
            normalized["env"] = dict(cli_source["env"])
        return normalized

    def _x07_path(self) -> str:
        if self._source["kind"] != "cli":
            return "<dir-mode>"
        cli_source = cast(CliSource, self._source)
        return cli_source.get("x07Path", os.environ.get("X07_BIN", "x07"))

    def _cli_env(self) -> Mapping[str, str] | None:
        if self._source["kind"] != "cli":
            return None
        cli_source = cast(CliSource, self._source)
        return cli_source.get("env")

    def _cli_cwd(self) -> str | None:
        if self._source["kind"] != "cli":
            return None
        cli_source = cast(CliSource, self._source)
        return cli_source.get("cwd")

    def _run_x07(self, argv: list[str]) -> str:
        x07_path = self._x07_path()
        cmd = [x07_path, *argv]
        try:
            proc = subprocess.run(
                cmd,
                cwd=self._cli_cwd(),
                env=dict(self._cli_env()) if self._cli_env() is not None else None,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=self._timeout_s,
                check=False,
            )
        except FileNotFoundError as exc:
            raise GenpackError(
                X07_GENPACK_E_X07_NOT_FOUND,
                "x07 executable was not found",
                data={"x07_path": x07_path, "path_env": os.environ.get("PATH", "")},
            ) from exc
        except subprocess.TimeoutExpired as exc:
            raise GenpackError(
                X07_GENPACK_E_SUBPROCESS_TIMEOUT,
                "x07 command timed out",
                data={"argv": cmd, "timeout_s": self._timeout_s},
            ) from exc

        if proc.returncode != 0:
            stderr = proc.stderr.decode("utf-8", errors="replace")
            raise GenpackError(
                X07_GENPACK_E_SUBPROCESS_FAILED,
                "x07 subprocess returned non-zero exit status",
                data={"argv": cmd, "exit_code": proc.returncode, "stderr": stderr},
            )

        try:
            return proc.stdout.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise GenpackError(
                X07_GENPACK_E_STDOUT_NOT_UTF8,
                "x07 stdout was not valid UTF-8",
                data={"argv": cmd},
            ) from exc

    def _parse_schema_doc(self, raw: str, *, argv: Iterable[str]) -> JsonDoc[dict[str, Any]]:
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise GenpackError(
                X07_GENPACK_E_SCHEMA_JSON_PARSE,
                "failed to parse schema JSON",
                data={"argv": list(argv), "stdout_prefix": _preview(raw)},
            ) from exc

        if not isinstance(parsed, dict):
            raise GenpackError(
                X07_GENPACK_E_SCHEMA_JSON_PARSE,
                "schema payload must be a JSON object",
                data={"argv": list(argv), "stdout_prefix": _preview(raw)},
            )

        if self._strict:
            schema_id = parsed.get("$id")
            if not isinstance(schema_id, str) or schema_id != EXPECTED_SCHEMA_ID:
                raise GenpackError(
                    X07_GENPACK_E_SCHEMA_VERSION_MISMATCH,
                    "schema $id does not match expected x07AST schema",
                    data={"expected": EXPECTED_SCHEMA_ID, "actual": schema_id},
                )
            if not isinstance(parsed.get("$schema"), str):
                raise GenpackError(
                    X07_GENPACK_E_SCHEMA_VERSION_MISMATCH,
                    "schema is missing $schema",
                    data={"expected": "json-schema URL", "actual": parsed.get("$schema")},
                )

        return JsonDoc(raw=raw, json=parsed, sha256_hex=_sha256_hex_text(raw))

    def _parse_grammar_bundle_doc(self, raw: str, *, argv: Iterable[str]) -> GrammarBundle:
        try:
            parsed = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise GenpackError(
                X07_GENPACK_E_GRAMMAR_BUNDLE_JSON_PARSE,
                "failed to parse grammar bundle JSON",
                data={"argv": list(argv), "stdout_prefix": _preview(raw)},
            ) from exc

        if not isinstance(parsed, dict):
            raise GenpackError(
                X07_GENPACK_E_BUNDLE_SHAPE_INVALID,
                "grammar bundle payload must be a JSON object",
                data={"missing_fields": ["schema_version", "variants", "semantic_supplement"]},
            )

        schema_version = parsed.get("schema_version")
        if not isinstance(schema_version, str):
            raise GenpackError(
                X07_GENPACK_E_BUNDLE_SHAPE_INVALID,
                "grammar bundle is missing schema_version",
                data={"missing_fields": ["schema_version"]},
            )

        if self._strict and schema_version != EXPECTED_GRAMMAR_BUNDLE_VERSION:
            raise GenpackError(
                X07_GENPACK_E_GRAMMAR_BUNDLE_VERSION_MISMATCH,
                "grammar bundle schema_version mismatch",
                data={"expected": EXPECTED_GRAMMAR_BUNDLE_VERSION, "actual": schema_version},
            )

        x07ast_schema_version = parsed.get("x07ast_schema_version")
        if x07ast_schema_version is not None and not isinstance(x07ast_schema_version, str):
            raise GenpackError(
                X07_GENPACK_E_BUNDLE_SHAPE_INVALID,
                "x07ast_schema_version must be a string",
                data={"missing_fields": ["x07ast_schema_version"]},
            )

        variants_obj = parsed.get("variants")
        if not isinstance(variants_obj, list):
            raise GenpackError(
                X07_GENPACK_E_BUNDLE_SHAPE_INVALID,
                "variants must be an array",
                data={"missing_fields": ["variants"]},
            )

        variants: dict[str, GrammarVariant] = {}
        for item in variants_obj:
            if not isinstance(item, dict):
                continue
            name = item.get("name")
            cfg = item.get("cfg")
            if not isinstance(name, str) or not isinstance(cfg, str):
                continue
            if name not in ("min", "pretty"):
                continue
            variants[name] = GrammarVariant(
                name=cast(str, name),
                cfg=cfg,
                sha256_hex=_sha256_hex_text(cfg),
            )

        if "min" not in variants:
            raise GenpackError(
                X07_GENPACK_E_VARIANT_MISSING,
                "grammar variant `min` is required",
                data={"variant": "min", "available_variants": sorted(variants.keys())},
            )

        semantic = parsed.get("semantic_supplement")
        if not isinstance(semantic, dict):
            raise GenpackError(
                X07_GENPACK_E_BUNDLE_SHAPE_INVALID,
                "semantic_supplement must be a JSON object",
                data={"missing_fields": ["semantic_supplement"]},
            )

        semantic_version = semantic.get("schema_version")
        if self._strict and semantic_version != EXPECTED_SEMANTIC_VERSION:
            raise GenpackError(
                X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH,
                "semantic supplement schema_version mismatch",
                data={"expected": EXPECTED_SEMANTIC_VERSION, "actual": semantic_version},
            )

        semantic_raw = json.dumps(semantic, separators=(",", ":"), sort_keys=True)
        semantic_doc = JsonDoc(
            raw=semantic_raw,
            json=semantic,
            sha256_hex=_sha256_hex_text(semantic_raw),
        )

        return GrammarBundle(
            raw=raw,
            json=parsed,
            schema_version=schema_version,
            x07ast_schema_version=x07ast_schema_version,
            variants=variants,
            semantic_supplement=semantic_doc,
        )

    def _bundle_from_dir(self, dir_path: Path) -> GrammarBundle:
        missing = [name for name in REQUIRED_DIR_ARTIFACTS if not (dir_path / name).is_file()]
        if missing:
            raise GenpackError(
                X07_GENPACK_E_DIR_MISSING_ARTIFACT,
                "directory source is missing generation-pack files",
                data={"dir": str(dir_path), "missing_paths": missing},
            )

        schema_raw = self._read_dir_text(dir_path / "x07ast.schema.json")
        schema_doc = self._parse_schema_doc(schema_raw, argv=[str(dir_path / "x07ast.schema.json")])

        min_cfg = self._read_dir_text(dir_path / "x07ast.min.gbnf")
        pretty_cfg = self._read_dir_text(dir_path / "x07ast.pretty.gbnf")
        semantic_raw = self._read_dir_text(dir_path / "x07ast.semantic.json")
        try:
            semantic_obj = json.loads(semantic_raw)
        except json.JSONDecodeError as exc:
            raise GenpackError(
                X07_GENPACK_E_GRAMMAR_BUNDLE_JSON_PARSE,
                "failed to parse semantic supplement JSON",
                data={"argv": [str(dir_path / "x07ast.semantic.json")], "stdout_prefix": _preview(semantic_raw)},
            ) from exc
        if not isinstance(semantic_obj, dict):
            raise GenpackError(
                X07_GENPACK_E_BUNDLE_SHAPE_INVALID,
                "semantic supplement must decode to an object",
                data={"missing_fields": ["semantic_supplement"]},
            )

        semantic_version = semantic_obj.get("schema_version")
        if self._strict and semantic_version != EXPECTED_SEMANTIC_VERSION:
            raise GenpackError(
                X07_GENPACK_E_SEMANTIC_SUPPLEMENT_VERSION_MISMATCH,
                "semantic supplement schema_version mismatch",
                data={"expected": EXPECTED_SEMANTIC_VERSION, "actual": semantic_version},
            )

        schema_props = schema_doc.json.get("properties")
        x07ast_schema_version: str | None = None
        if isinstance(schema_props, dict):
            sv = schema_props.get("schema_version")
            if isinstance(sv, dict):
                const = sv.get("const")
                if isinstance(const, str):
                    x07ast_schema_version = const

        bundle_obj: dict[str, Any] = {
            "schema_version": EXPECTED_GRAMMAR_BUNDLE_VERSION,
            "x07ast_schema_version": x07ast_schema_version,
            "format": "gbnf_v1",
            "variants": [
                {"name": "min", "cfg": min_cfg},
                {"name": "pretty", "cfg": pretty_cfg},
            ],
            "semantic_supplement": semantic_obj,
            "sha256": {
                "min_cfg": _sha256_hex_text(min_cfg),
                "pretty_cfg": _sha256_hex_text(pretty_cfg),
                "semantic_supplement": _sha256_hex_text(semantic_raw),
            },
        }

        self._verify_manifest_hashes(
            dir_path,
            {
                "x07ast.schema.json": _sha256_hex_text(schema_raw),
                "x07ast.min.gbnf": _sha256_hex_text(min_cfg),
                "x07ast.pretty.gbnf": _sha256_hex_text(pretty_cfg),
                "x07ast.semantic.json": _sha256_hex_text(semantic_raw),
            },
        )

        raw = json.dumps(bundle_obj, separators=(",", ":"), sort_keys=False)
        return self._parse_grammar_bundle_doc(raw, argv=[str(dir_path)])

    def _read_dir_text(self, path: Path) -> str:
        try:
            return path.read_text(encoding="utf-8")
        except OSError as exc:
            raise GenpackError(
                X07_GENPACK_E_DIR_READ_FAILED,
                "failed to read directory artifact",
                data={"path": str(path), "io_error": str(exc)},
            ) from exc

    def _verify_manifest_hashes(self, dir_path: Path, actual_hashes: Mapping[str, str]) -> None:
        manifest_path = dir_path / "manifest.json"
        if not manifest_path.is_file():
            return

        manifest_text = self._read_dir_text(manifest_path)
        try:
            manifest = json.loads(manifest_text)
        except json.JSONDecodeError:
            return
        if not isinstance(manifest, dict):
            return

        artifacts = manifest.get("artifacts")
        if not isinstance(artifacts, list):
            return

        for artifact in artifacts:
            if not isinstance(artifact, dict):
                continue
            name = artifact.get("name")
            expected = artifact.get("sha256")
            if not isinstance(name, str) or not isinstance(expected, str):
                continue
            actual = actual_hashes.get(name)
            if actual is None:
                continue
            if actual != expected:
                raise GenpackError(
                    X07_GENPACK_E_HASH_MISMATCH,
                    "artifact hash mismatch against manifest",
                    data={"path": str(dir_path / name), "expected_sha256": expected, "actual_sha256": actual},
                )

    def _check_schema_alignment(self, *, schema: JsonDoc[dict[str, Any]], grammar: GrammarBundle) -> None:
        if grammar.x07ast_schema_version is None:
            return

        props = schema.json.get("properties")
        if not isinstance(props, dict):
            return
        schema_version_prop = props.get("schema_version")
        if not isinstance(schema_version_prop, dict):
            return
        const = schema_version_prop.get("const")
        if not isinstance(const, str):
            return

        if self._strict and const != grammar.x07ast_schema_version:
            raise GenpackError(
                X07_GENPACK_E_SCHEMA_VERSION_MISMATCH,
                "x07ast schema version mismatch between schema and grammar bundle",
                data={"expected": const, "actual": grammar.x07ast_schema_version},
            )

    def _discover_toolchain_version(self) -> str | None:
        if self._source["kind"] != "cli":
            return None

        x07_path = self._x07_path()
        try:
            proc = subprocess.run(
                [x07_path, "--version"],
                cwd=self._cli_cwd(),
                env=dict(self._cli_env()) if self._cli_env() is not None else None,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                timeout=min(self._timeout_s, 5.0),
                check=False,
            )
        except Exception:
            return None

        if proc.returncode != 0:
            return None
        try:
            text = proc.stdout.decode("utf-8").strip()
        except UnicodeDecodeError:
            return None
        if not text:
            return None
        if " " in text:
            return text.split(" ", 1)[1].strip() or None
        return text

    def _cache_entry(self) -> CacheEntry | None:
        if self._cache_dir is None:
            return None
        version = self._discover_toolchain_version() or "unknown"
        key = f"{version}__{EXPECTED_GRAMMAR_BUNDLE_VERSION}__{EXPECTED_SEMANTIC_VERSION}"
        safe_key = "".join(ch if ch.isalnum() or ch in "._-" else "_" for ch in key)
        return CacheEntry(key=safe_key, dir=self._cache_dir / safe_key)

    def _read_cache(self, entry: CacheEntry) -> X07AstGenpack | None:
        schema_path = entry.dir / "schema.json"
        grammar_path = entry.dir / "grammar_bundle.json"
        if not schema_path.is_file() or not grammar_path.is_file():
            return None
        try:
            schema_raw = schema_path.read_text(encoding="utf-8")
            grammar_raw = grammar_path.read_text(encoding="utf-8")
        except OSError as exc:
            raise GenpackError(
                X07_GENPACK_E_CACHE_IO,
                "failed to read cache entry",
                data={"cache_dir": str(entry.dir), "io_error": str(exc)},
            ) from exc

        schema = self._parse_schema_doc(schema_raw, argv=[str(schema_path)])
        grammar = self._parse_grammar_bundle_doc(grammar_raw, argv=[str(grammar_path)])
        self._check_schema_alignment(schema=schema, grammar=grammar)
        return X07AstGenpack(
            toolchain=ToolchainInfo(x07_path=self._x07_path(), x07_version=self._discover_toolchain_version()),
            schema=schema,
            grammar=grammar,
        )

    def _write_cache(self, entry: CacheEntry, genpack: X07AstGenpack) -> None:
        try:
            entry.dir.mkdir(parents=True, exist_ok=True)
            (entry.dir / "schema.json").write_text(genpack.schema.raw, encoding="utf-8")
            (entry.dir / "grammar_bundle.json").write_text(genpack.grammar.raw, encoding="utf-8")
        except OSError as exc:
            raise GenpackError(
                X07_GENPACK_E_CACHE_IO,
                "failed to write cache entry",
                data={"cache_dir": str(entry.dir), "io_error": str(exc)},
            ) from exc


def get_x07ast_genpack(
    source: GenpackSource | None = None,
    timeout_s: float = 30.0,
    cache_dir: Path | None = None,
    strict: bool = True,
) -> X07AstGenpack:
    return GenpackClient(source=source, timeout_s=timeout_s, cache_dir=cache_dir, strict=strict).get_x07ast_genpack()


def get_x07ast_schema_str(
    source: GenpackSource | None = None,
    timeout_s: float = 30.0,
    cache_dir: Path | None = None,
    strict: bool = True,
) -> str:
    return GenpackClient(source=source, timeout_s=timeout_s, cache_dir=cache_dir, strict=strict).get_x07ast_schema().raw


def get_x07ast_gbnf_min(
    source: GenpackSource | None = None,
    timeout_s: float = 30.0,
    cache_dir: Path | None = None,
    strict: bool = True,
) -> str:
    bundle = GenpackClient(source=source, timeout_s=timeout_s, cache_dir=cache_dir, strict=strict).get_x07ast_grammar_bundle()
    variant = bundle.variants.get("min")
    if variant is None:
        raise GenpackError(
            X07_GENPACK_E_VARIANT_MISSING,
            "grammar variant `min` is required",
            data={"variant": "min", "available_variants": sorted(bundle.variants.keys())},
        )
    return variant.cfg
