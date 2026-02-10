# X07 Documentation

- [Home](index.md)
- [Why X07](why-x07.md)

## Getting started
- [Agent initial prompt](getting-started/agent-initial-prompt.md)
- [Agent quickstart](getting-started/agent-quickstart.md)
- [Available skills](getting-started/available-skills.md)
- [Install](getting-started/install.md)
- [Installer / x07up](getting-started/installer.md)
- [First project](getting-started/first-project.md)
- [Agent workflow](getting-started/agent-workflow.md)

## Agent
- [Agent contracts](agent/contract.md)
- [Readiness checks](agent/readiness-checks.md)
- [Roadmap apps](agent/apps.md)

## Recipes
- [Patch-based recipes](recipes/index.md)
- [Agent patterns](recipes/agent-patterns.md)

## Language
- [Overview](language/overview.md)
- [Syntax & x07AST](language/syntax-x07ast.md)
- [Generics](language/generics.md)
- [Types & memory model](language/types-memory.md)
- [Concurrency & multiprocessing](language/concurrency-multiprocessing.md)
- [Streaming pipes](language/stream-pipes.md)
- [Budget scopes](language/budget-scopes.md)

## Worlds
- [Worlds overview](worlds/index.md)
- [OS worlds](worlds/os-worlds.md)
- [Record/replay](worlds/record-replay.md)
- [Sandbox policy walkthrough](worlds/sandbox-policy-walkthrough.md)

## Toolchain
- [CLI](toolchain/cli.md)
- [Benchmarks](toolchain/benchmarks.md)
- [Compiler limits](toolchain/compiler-limits.md)
- [Architecture check](toolchain/arch-check.md)
- [Schema derive](toolchain/schema-derive.md)
- [State machines](toolchain/state-machines.md)
- [Running programs](toolchain/running-programs.md)
- [Embedding in C](toolchain/embedding-in-c.md)
- [Repair loop](toolchain/repair-loop.md)
- [Diagnostics & repair](toolchain/diagnostics-repair.md)
- [Diagnostic codes](toolchain/diagnostic-codes.md)
- [Review & trust artifacts](toolchain/review-trust.md)
- [Testing](toolchain/testing.md)
- [Property-based testing](toolchain/pbt.md)
- [PBT repro → regression test](toolchain/pbt-fix-from-repro.md)
- [Test manifest](toolchain/tests-manifest.md)
- [Testing by example](toolchain/testing-by-example.md)
- [Performance comparisons](dev/perf-compare.md)

## Genpack
- [Overview](genpack/index.md)
- [XGrammar cookbook](genpack/xgrammar.md)
- [Outlines cookbook](genpack/outlines.md)

## Spec
- [Spec index](spec/index.md)
- [Spec changelog](spec/changelog.md)
- [Language guide](spec/language-guide.md)
- [Core](spec/internal/x07-core.md)
- [Type system v1](spec/internal/types/type-system-v1.md)
- [Memory management](spec/internal/x07-memory-management.md)
- [C backend](spec/internal/x07-c-backend.md)
- [Testing v1](spec/internal/x07-testing-v1.md)
- [ABI v1](spec/abi/abi-v1.md)
- [ABI v2](spec/abi/abi-v2.md)
- [ABI stability policy](spec/abi/stability-policy.md)
- [Published spec manifest](spec/spec-index.md)

## Packages
- [Packages & workspaces](packages/index.md)
- [Publishing](packages/publishing.md)
- [Publishing by example](packages/publishing-by-example.md)

## x07import
- [Overview](x07import/README.md)
- [Porting by example](x07import/porting-by-example.md)

## Libraries
- [Stdlib](libraries/stdlib.md)
- [External packages](libraries/external.md)
- [Agentic design](libraries/agentic-design.md)

## Contracts
- Networking
  - [Networking bytes ABIs v1](net/net-v1.md)
  - [Networking error docs v1](net/errors-v1.md)
- Databases
  - [DB v1](db/db-v1.md)
  - [SQLite v1](db/sqlite-v1.md)
  - [Postgres v1](db/postgres-v1.md)
  - [MySQL v1](db/mysql-v1.md)
  - [Redis v1](db/redis-v1.md)
  - [Pool v1](db/pool-v1.md)
- Filesystem
  - [FS v1](fs/fs-v1.md)
- Time
  - [Time v1](time/time-v1.md)
  - [Duration v1](time/duration-v1.md)
  - [RFC3339 v1](time/rfc3339-v1.md)
  - [Civil v1](time/civil-v1.md)
  - [TZDB v1](time/tzdb-v1.md)
  - [OS time v1](time/os-time-v1.md)
- Math
  - [Math v1](math/math-v1.md)
  - [Vendor v1](math/vendor-v1.md)
  - [Native backend](math/native-backend-v1.md)
  - [Smoke suites](math/smoke-suites-v1.md)
- Text
  - [Regex v1](text/regex-v1.md)
  - [X7SL v1](text/x7sl-v1.md)
- OS
  - [Process ABI v1](os/process-abi-v1.md)

- [FAQ](faq.md)
- [Versioning policy](versioning-policy.md)
- [References](references.md)

## Guides
- [CLI apps](guides/cli-apps.md)
- [Data interoperability](guides/data-interop.md)
- [Web services](guides/web-apps.md)
- [Crawling](guides/crawling.md)
- [Messaging](guides/messaging.md)
- [Networking](guides/networking.md)
- [Databases](guides/databases.md)

## Project
- [About](about.md)
- [Repositories](project/repositories.md)
- [Package policy](project/package-policy.md)
- [Governance](governance.md)
- [Contributing](contributing.md)
