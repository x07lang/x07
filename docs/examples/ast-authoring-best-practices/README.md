# Example project: AST authoring best practices

This directory is a standalone X07 project intended for trying the “AST authoring ergonomics” workflow:

- `x07 check --ast` (schema/shape + lint only)
- `x07 check` (full typecheck + backend-check)
- `x07 fix` (apply lint quickfixes)

## Files

- `docs/examples/ast-authoring-best-practices/x07.json`: project manifest
- `docs/examples/ast-authoring-best-practices/x07.lock.json`: pinned deps
- `docs/examples/ast-authoring-best-practices/src/main.x07.json`: entry module

## Commands

From repo root:

- Regenerate lockfile: `x07 pkg lock --project docs/examples/ast-authoring-best-practices/x07.json`
- AST-only check: `x07 check --project docs/examples/ast-authoring-best-practices/x07.json --ast`
- Full check: `x07 check --project docs/examples/ast-authoring-best-practices/x07.json`
- Run: `printf 'hello' | x07 run --repair=off --project docs/examples/ast-authoring-best-practices/x07.json --stdin`
