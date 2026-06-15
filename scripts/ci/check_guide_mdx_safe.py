#!/usr/bin/env python3
"""CI guard: the generated language guide must be MDX-safe.

The website (Docusaurus/MDX) parses a `<` *outside* inline-code as the start of a
JSX tag. A `<` not followed by a tag-name start character (a letter, `$`, `_`, or
`/`) is a hard parse error that fails the site build — this is exactly what broke
the v0.2.16 Pages deploy (`i32<->f64` in the f64 guide section). The existing
`check_language_guide_sync` canary only checks that the committed guide *matches*
`x07c -- guide`; it does not check MDX safety. This guard closes that gap so an
unsafe `<` is caught in x07 CI instead of at website-deploy time.

It scans the committed guide (which the sync canary guarantees reflects the
`crates/x07c/src/guide.rs` source).
"""

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUIDE = REPO_ROOT / "docs" / "spec" / "language-guide.md"

# Inline-code spans (backtick-delimited) are literal in MDX, so `<` inside them is safe.
CODE_SPAN = re.compile(r"`[^`]*`")
# A `<` that does not begin a valid JSX tag name (letter / `$` / `_` / `/`).
MDX_UNSAFE_LT = re.compile(r"<(?![A-Za-z$_/])")


def main() -> int:
    if not GUIDE.is_file():
        print(f"check_guide_mdx_safe: ERROR: missing {GUIDE}", file=sys.stderr)
        return 2
    text = GUIDE.read_text(encoding="utf-8")
    bad = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        outside_code = CODE_SPAN.sub("", line)
        if MDX_UNSAFE_LT.search(outside_code):
            bad.append((lineno, line.rstrip()))

    rel = GUIDE.relative_to(REPO_ROOT)
    if bad:
        print(
            f"check_guide_mdx_safe: FAIL ({len(bad)} MDX-unsafe `<` in {rel})",
            file=sys.stderr,
        )
        for lineno, line in bad:
            print(f"  line {lineno}: {line}", file=sys.stderr)
        print(
            "hint: a bare `<` outside backticks is parsed as JSX by the website MDX build.",
            file=sys.stderr,
        )
        print(
            "      Wrap the token in backticks, or reword it (e.g. `i32<->f64` -> `i32/f64`),",
            file=sys.stderr,
        )
        print(
            "      in crates/x07c/src/guide.rs, then regenerate the guide docs.",
            file=sys.stderr,
        )
        return 1

    print(f"check_guide_mdx_safe: OK ({rel}, {len(text.splitlines())} lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
