from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any, Generic, Literal, Mapping, TypedDict, TypeVar, Union


T = TypeVar("T")


class CliSource(TypedDict, total=False):
    kind: Literal["cli"]
    x07Path: str
    cwd: str
    env: Mapping[str, str]


class DirSource(TypedDict):
    kind: Literal["dir"]
    dir: str


GenpackSource = Union[CliSource, DirSource]


@dataclass(frozen=True)
class ToolchainInfo:
    x07_path: str
    x07_version: str | None


@dataclass(frozen=True)
class JsonDoc(Generic[T]):
    raw: str
    json: T
    sha256_hex: str


@dataclass(frozen=True)
class GrammarVariant:
    name: Literal["min", "pretty"]
    cfg: str
    sha256_hex: str


@dataclass(frozen=True)
class GrammarBundle:
    raw: str
    json: dict[str, Any]
    schema_version: str
    x07ast_schema_version: str | None
    variants: dict[str, GrammarVariant]
    semantic_supplement: JsonDoc[dict[str, Any]]


@dataclass(frozen=True)
class X07AstGenpack:
    toolchain: ToolchainInfo
    schema: JsonDoc[dict[str, Any]]
    grammar: GrammarBundle


@dataclass(frozen=True)
class CacheEntry:
    key: str
    dir: Path
