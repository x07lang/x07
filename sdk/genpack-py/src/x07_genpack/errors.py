from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from .error_codes import CODE_DOCS, GenpackErrorCode


@dataclass(frozen=True)
class GenpackWarning:
    code: str
    message: str
    data: dict[str, Any]


class GenpackError(RuntimeError):
    def __init__(self, code: GenpackErrorCode, message: str, *, data: dict[str, Any] | None = None):
        self.code = code
        self.data = data or {}
        super().__init__(f"{code}: {message}")


def format_error(code: GenpackErrorCode, *, default: str) -> str:
    doc = CODE_DOCS.get(code)
    if not isinstance(doc, dict):
        return default
    summary = doc.get("summary")
    if isinstance(summary, str) and summary:
        return summary
    return default
