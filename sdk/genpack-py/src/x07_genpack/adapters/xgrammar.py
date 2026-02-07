from __future__ import annotations

from ..types import X07AstGenpack


def to_json_schema_string(genpack: X07AstGenpack) -> str:
    return genpack.schema.raw


def to_ebnf_string(genpack: X07AstGenpack, *, variant: str = "min") -> str:
    chosen = genpack.grammar.variants.get(variant)
    if chosen is None:
        available = ",".join(sorted(genpack.grammar.variants.keys()))
        raise ValueError(f"missing grammar variant: {variant}; available={available}")
    return chosen.cfg
