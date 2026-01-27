from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SKILL_NAME_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
BACKTICK_TOKEN_RE = re.compile(r"`([^`\n]+)`")


@dataclass(frozen=True)
class Skill:
    name: str
    description: str
    dir_name: str
    skill_md: Path
    metadata: dict[str, str]
    body: str


class SkillError(Exception):
    pass


def parse_frontmatter(lines: list[str], *, path: Path) -> tuple[dict[str, Any], int]:
    if not lines or lines[0].strip() != "---":
        raise SkillError(f"missing YAML frontmatter start '---': {path}")
    end_idx = None
    for i in range(1, len(lines)):
        if lines[i].strip() == "---":
            end_idx = i
            break
    if end_idx is None:
        raise SkillError(f"missing YAML frontmatter end '---': {path}")

    fm_lines = lines[1:end_idx]
    data: dict[str, Any] = {}
    cur_section: str | None = None
    for raw in fm_lines:
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        indent = len(raw) - len(raw.lstrip(" "))
        if indent not in (0, 2):
            raise SkillError(f"unsupported YAML indentation (expected 0 or 2 spaces): {path}: {raw!r}")

        line = raw.strip()
        if ":" not in line:
            raise SkillError(f"invalid YAML line (expected key: value): {path}: {raw!r}")
        key, value = line.split(":", 1)
        key = key.strip()
        value = value.strip()

        if indent == 0:
            cur_section = None
            if value == "":
                if key != "metadata":
                    raise SkillError(f"only 'metadata' may be a nested mapping in frontmatter: {path}")
                if "metadata" in data:
                    raise SkillError(f"duplicate metadata block: {path}")
                data["metadata"] = {}
                cur_section = "metadata"
            else:
                if key == "metadata":
                    raise SkillError(f"metadata must be a block mapping (metadata: ... on next lines): {path}")
                data[key] = value
        else:
            if cur_section != "metadata":
                raise SkillError(f"unexpected nested key outside metadata: {path}: {raw!r}")
            meta = data.get("metadata")
            if not isinstance(meta, dict):
                raise SkillError(f"internal error: metadata block not a dict: {path}")
            meta[key] = value

    return data, end_idx + 1


def parse_skill(skill_md: Path) -> Skill:
    text = skill_md.read_text(encoding="utf-8")
    lines = text.splitlines()
    front, body_start = parse_frontmatter(lines, path=skill_md)
    name = front.get("name")
    description = front.get("description")
    metadata = front.get("metadata", {})
    if not isinstance(name, str) or not name:
        raise SkillError(f"frontmatter must contain non-empty name: {skill_md}")
    if not isinstance(description, str) or not description:
        raise SkillError(f"frontmatter must contain non-empty description: {skill_md}")
    if not isinstance(metadata, dict) or not all(isinstance(k, str) and isinstance(v, str) for k, v in metadata.items()):
        raise SkillError(f"metadata must be a flat string map: {skill_md}")

    return Skill(
        name=name,
        description=description,
        dir_name=skill_md.parent.name,
        skill_md=skill_md,
        metadata=metadata,
        body="\n".join(lines[body_start:]).lstrip("\n"),
    )


def iter_skill_dirs(skills_root: Path) -> list[Path]:
    dirs: list[Path] = []
    for p in sorted(skills_root.iterdir()):
        if not p.is_dir():
            continue
        if p.name.startswith("."):
            continue
        dirs.append(p)
    return dirs


def validate_backtick_paths(root: Path, skill: Skill) -> list[str]:
    errors: list[str] = []
    for token in BACKTICK_TOKEN_RE.findall(skill.body):
        if any(ch.isspace() for ch in token):
            continue
        if "<" in token or ">" in token:
            continue
        if token.startswith(("docs/", "scripts/", "./docs/", "./scripts/")):
            errors.append(
                f"{skill.skill_md}: end-user skills pack must not reference toolchain repo paths: `{token}`"
            )
            continue

        rel: str | None = None
        if token.startswith("./"):
            rel = token[2:]
        elif token.startswith(("references/", "assets/")):
            rel = token
        if rel is None:
            continue

        path = skill.skill_md.parent / rel
        if not path.exists():
            errors.append(f"{skill.skill_md}: referenced path does not exist: `{token}`")
    return errors


def validate_skill(root: Path, skills_root: Path, skill: Skill) -> list[str]:
    errors: list[str] = []
    if not SKILL_NAME_RE.match(skill.name):
        errors.append(f"{skill.skill_md}: invalid skill name (expected lowercase kebab-case): {skill.name!r}")
    if skill.dir_name != skill.name:
        errors.append(f"{skill.skill_md}: directory name must match frontmatter name: dir={skill.dir_name!r} name={skill.name!r}")
    if "evolang" in skill.name or "evolang" in skill.description.lower():
        errors.append(f"{skill.skill_md}: evolang references must not appear in X07 skills")

    if skill.metadata.get("kind") == "script-backed":
        scripts_dir = skill.skill_md.parent / "scripts"
        if not scripts_dir.is_dir():
            errors.append(f"{skill.skill_md}: script-backed skill must have scripts/ directory")
        else:
            has_file = any(p.is_file() for p in scripts_dir.iterdir())
            if not has_file:
                errors.append(f"{skill.skill_md}: scripts/ directory is empty")

    errors.extend(validate_backtick_paths(root, skill))
    return errors


def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="Validate skills without writing files")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    _args = parse_args(argv)
    root = Path(__file__).resolve().parents[1]
    skills_root = root / "skills" / "pack" / ".agent" / "skills"
    if not skills_root.is_dir():
        print("ERROR: missing skills/pack/.agent/skills/ (end-user skills pack root)", file=sys.stderr)
        return 2

    skill_dirs = iter_skill_dirs(skills_root)
    if not skill_dirs:
        print("ERROR: no skills found under skills/pack/.agent/skills/", file=sys.stderr)
        return 2

    skills: list[Skill] = []
    errors: list[str] = []
    for d in skill_dirs:
        skill_md = d / "SKILL.md"
        if not skill_md.is_file():
            errors.append(f"{d}: missing SKILL.md")
            continue
        try:
            skills.append(parse_skill(skill_md))
        except SkillError as e:
            errors.append(str(e))

    seen: set[str] = set()
    for s in skills:
        if s.name in seen:
            errors.append(f"duplicate skill name: {s.name!r}")
        seen.add(s.name)
        errors.extend(validate_skill(root, skills_root, s))

    if errors:
        for e in errors:
            print(f"ERROR: {e}", file=sys.stderr)
        return 1

    print(f"ok: skills validated ({len(skills)} skills)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
