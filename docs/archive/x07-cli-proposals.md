Below is a concrete design + implementation roadmap for a **production‑ready X07 CLI package** that’s friendly to **100% agentic coding** (LLM-written apps), while staying consistent with your “one canonical way” and with your **run‑os / run‑os‑sandboxed** world split.

I’ll treat “CLI package” as: **(1) a library package implemented in X07** + **(2) small toolchain helpers** that make it easy for agents to *produce correct specs and glue code* without hand‑authoring deep structured data.

---

## 0) Design goals and constraints

### Goals

1. **Agent-friendly authoring**

   * Agents should not have to hand‑craft deeply nested JSON/x07AST to define CLIs.
   * Most CLIs should be expressible with a *small set of canonical patterns*.

2. **Production behavior**

   * Correct handling of options/operands including the `--` “end of options” delimiter (POSIX guideline). ([The Open Group][1])
   * Subcommands (`cmd subcmd [options] [operands]` is a standard grouping convention). ([Oracle Docs][2])
   * Automatic help/usage generation (like argparse/clap). ([Python documentation][3])
   * Shell completions generation (like clap_complete). ([Docs.rs][4])

3. **World separation**

   * The core parser should be **pure** (no OS access) so you can test it deterministically.
   * OS interaction (argv/env/stdout/stderr/exit) should be in a **run‑os adapter module**, never used in solve-* worlds.

### Non-goals (for v1)

* Perfect POSIX compatibility across all edge cases of every Unix utility.
* Interactive prompts / TUI.
* Internationalization.
* Windows parity for every process/environment detail (we can plan it, but not block v1).

---

## 1) Proposed package split and naming

Create one **official external package** (ship it as “blessed” with the distribution, but keep it out of core stdlib to avoid bloating the language core):

* **Package**: `x07:cli@0.1.0`
* **Modules**:

  * `std.cli` – pure parsing + match accessors
  * `std.cli.help` – help/usage rendering (pure)
  * `std.cli.complete` – completion generation (pure)
  * `std.cli.os` – run‑os adapter: load argv/env, print help/errors, exit (OS-only)

Why this design is robust:

* Agents can unit-test parsing in deterministic runs.
* The same CLI spec works in fixture worlds (tests) and OS worlds (real usage), only the **adapter changes**.

---

## 2) One canonical way to define CLIs: “Flat Spec Rows”

Your agents already struggle with deeply nested JSON. So don’t make CLI definitions a tree.

Instead: define the CLI as a **row list** (flat table), which is:

* simple to generate
* simple to validate with JSON schema
* easy for a tool to auto-format and canonicalize

### 2.1 `x07cliSpecRows v1` (canonical JSON)

A single file in the app repo:

`cli/cli.spec.json`

```json
{
  "schema_version": "x07cli.specrows@0.1.0",
  "app": { "name": "mytool", "version": "0.1.0", "about": "Example tool" },
  "rows": [
    ["root", "help", "-h", "--help", "Show help"],
    ["root", "flag", "-v", "--verbose", "Increase verbosity"],
    ["root", "opt",  "-o", "--output", "PATH", "Output path", {"default": "out.txt"}],
    ["root", "arg",  "INPUT", "Input file", {"required": true}],

    ["serve", "about", "Serve files"],
    ["serve", "opt",  "-p", "--port", "U32", "Port", {"default": 8080}],
    ["serve", "arg",  "DIR", "Directory", {"required": true}]
  ]
}
```

**Interpretation:**

* First field is the command scope: `root` or a subcommand name (e.g. `serve`).
* Second field is the row kind: `flag`, `opt`, `arg`, `about`, etc.
* Everything else is positional and schema-validated.

This achieves “one canonical way” and reduces LLM fragility.

### Why “rows” is the right tradeoff

* A tree spec is *hard to author*.
* A row list is *easy to author* and *easy to validate*.
* It’s the same philosophy as “CSV is a good interchange format”.

---

## 3) Argv and parse result encodings

### 3.1 `argv` bytes encoding (OS adapter provides this)

Define one canonical binary encoding:

**`argv_v1`**:

* `u32_le argc`
* repeated `argc` times:

  * `u32_le len`
  * `len` raw bytes (UTF‑8, but do not enforce valid UTF‑8 at this layer)

This is deterministic, efficient, and fits your existing codec style.

**OS adapter API** (run‑os only):

* `(std.cli.os.argv_v1)` -> `bytes` (argv_v1)
* `(std.cli.os.env_get key_bytes)` -> `option_bytes`
* `(std.cli.os.stdout_write b)` -> `i32`
* `(std.cli.os.stderr_write b)` -> `i32`
* `(std.cli.os.exit code_i32)` -> `i32`

### 3.2 Parse result encoding

Make the parse result a compact deterministic bytes blob with stable ordering.

**`matches_v1`**:

* `u8 tag` (1 ok, 0 error)
* if ok:

  * `u32_le selected_cmd_len + selected_cmd_bytes` (empty for root)
  * `u32_le entry_count`
  * for each entry (sorted by key bytes):

    * `u32_le key_len + key`
    * `u8 kind` (flag=1, opt=2, arg=3, multi=4)
    * `u32_le value_len + value_bytes` (or 0 for false flag)
* if err:

  * `u32_le err_code`
  * `u32_le msg_len + msg_bytes`
  * `u32_le usage_len + usage_bytes` (pre-rendered short usage)

### 3.3 Exit codes (production expectations)

Two compatible policies exist in real ecosystems:

* **Clap-style**: usage errors exit with **2** (and help may exit with 0). ([Docs.rs][5])
* **sysexits-style**: usage error is **EX_USAGE=64**. ([man7.org][6])

Recommendation for X07:

* Default to **2** for “invalid CLI invocation” (matches popular CLI practice; clap does this). ([Docs.rs][5])
* Provide constants + a switch for sysexits mapping when desired.

---

## 4) Core API surface for `std.cli` (v1)

Keep it small and canonical; avoid “fancy builder DSL”.

### `std.cli` (pure)

* `(std.cli.specrows.validate spec_json_bytes)` -> `result_i32`
  Validates JSON against schema + semantic checks (duplicate flags, etc).

* `(std.cli.specrows.compile spec_json_bytes)` -> `result_bytes`
  Produces a **compiled spec blob** `specbin_v1` (binary, stable ordering).

* `(std.cli.parse specbin_v1 argv_v1)` -> `bytes`
  Returns `matches_v1` (tagged ok/err).

* Accessors (operate on `matches_v1`):

  * `(std.cli.matches.is_ok m)` -> `i32`
  * `(std.cli.matches.cmd m)` -> `bytes` (selected subcommand or empty)
  * `(std.cli.matches.flag m key_bytes)` -> `i32` (0/1)
  * `(std.cli.matches.opt_bytes m key_bytes)` -> `option_bytes`
  * `(std.cli.matches.opt_i32 m key_bytes)` -> `option_i32` (parse `U32`/`I32` option)
  * `(std.cli.matches.arg_bytes m name_bytes)` -> `option_bytes`

### `std.cli.help` (pure)

* `(std.cli.help.render specbin_v1)` -> `bytes` (full help)
* `(std.cli.help.usage_short specbin_v1)` -> `bytes` (one-line usage)

This mirrors “auto help generation” behavior found in argparse/clap. ([Python documentation][3])

### `std.cli.complete` (pure)

* `(std.cli.complete.render specbin_v1 shell_bytes)` -> `result_bytes`
  Where `shell_bytes ∈ {bash,zsh,fish,powershell,elvish}`.
  clap_complete supports multiple shells similarly; you can match this conceptually without copying its API. ([Docs.rs][4])

### `std.cli.os` (run‑os only)

* `(std.cli.os.main spec_json_bytes handler_iface)` -> `i32`
  Optional, but recommended for “one canonical main pattern”:

  * loads argv
  * handles `--help`, `--version`
  * parses
  * on error prints diagnostics and exits with chosen code

If you don’t want interface records for handlers yet, keep `std.cli.os.main` out of v1 and just provide primitives (`argv_v1`, `stdout_write`, etc). But long-term, a canonical “main harness” helps agents.

---

## 5) Parser semantics (what you should implement)

Base semantics should follow common + POSIX expectations:

1. **Options start with `-`**; long options start with `--`.
2. **`--` ends option parsing**; remaining tokens are operands even if they start with `-`. ([The Open Group][1])
3. Short flags can be combined: `-abc` = `-a -b -c`.
4. Short option with value may attach: `-oVALUE` or `-o VALUE` (choose one canonical; I recommend supporting both).
5. Long option value may attach: `--out=VALUE` or `--out VALUE` (support both).
6. Unknown option → error with:

   * structured error code
   * short usage
   * “did you mean” suggestion (deterministic edit-distance)
7. Subcommand selection:

   * first non-option token that matches subcommand name selects it (common convention). ([Oracle Docs][2])
   * after subcommand, parse using that subcommand’s spec (with inherited global options optionally).

---

## 6) “Production-ready” features you should include in v1 vs v2

### Must have in v1

* flags/options/positional args
* subcommands
* `--help` auto
* `--version` auto
* deterministic help text ordering
* completions generation for at least bash/zsh/fish (powershell can be v1.1)
* structured error codes + stable exit behavior

### Should be v2 (after you have adoption)

* config file integration (XDG paths, etc.)

  * XDG default for config is `$HOME/.config` when `$XDG_CONFIG_HOME` unset. ([Freedesktop Specifications][7])
* env var defaults for options
* rich terminal UX (colors, progress bars)
* “docopt-like” input language as an alternate spec authoring mode (I’d avoid two ways initially; docopt exists if you ever want it). ([Docopt][8])

---

## 7) Tooling to make agentic usage reliable

Even if the library is perfect, agents will still fail if they must hand-author spec structures.

So ship **two tiny toolchain helpers** that operate on `cli/cli.spec.json`:

### 7.1 `x07 cli fmt`

* Validates schema + semantic checks
* Rewrites file with **canonical ordering** and consistent row formatting
* This becomes a hard CI gate for CLI apps.

### 7.2 `x07 cli gen`

* Generates:

  * `src/main_cli_spec.x07.json` (or `src/cli_spec.x07.json`): a module embedding compiled `specbin_v1`
  * `completions/<shell>/mytool` scripts (optional)
  * `docs/mytool.1` manpage skeleton (optional)
* Agents only edit `cli/cli.spec.json`, run `x07 cli gen`.

This is the same philosophy as `clap_complete` where completions are generated from the command spec. ([Docs.rs][4])

---

## 8) Detailed development plan (PR-sized milestones)

### CLI‑01 — Spec schema + doc

**Deliverables**

* `spec/x07cli.specrows.schema.json`
* `docs/cli/cli-v1.md` (normative):

  * option parsing rules including `--` end-of-options ([The Open Group][1])
  * error kinds + exit policy
  * argv_v1 encoding
* `scripts/check_cli_spec.py` (validates schema + semantic checks)

**Acceptance**

* Schema validates example specs.
* Semantic checks detect duplicate flags, missing required fields, invalid scope names.

---

### CLI‑02 — New package scaffold `x07:cli@0.1.0`

**Deliverables**

* `packages/x07-cli/0.1.0/package.json`
* `packages/x07-cli/0.1.0/modules/std/cli/module.x07.json` (stubs)
* `.../std/cli/help.x07.json`
* `.../std/cli/complete.x07.json`
* `.../std/cli/os.x07.json` (stubs; run‑os only)

**Acceptance**

* `x07 pkg pack` deterministically packs it (stable tar ordering, etc).
* Unit tests compile.

---

### CLI‑03 — Implement `specrows.validate` + `specrows.compile`

**Work**

* Parse spec rows
* Normalize:

  * stable sorting
  * resolve short/long forms
  * build compiled `specbin_v1` blob

**Acceptance**

* Golden tests: spec JSON → stable `specbin_v1` bytes
* Rejects invalid/ambiguous specs deterministically with error codes.

---

### CLI‑04 — Implement core parser `std.cli.parse`

**Work**

* Parse argv_v1 into tokens
* Apply parsing rules:

  * combined short flags
  * `--` delimiter ([The Open Group][1])
  * subcommands convention ([Oracle Docs][2])
  * value parsing rules

**Acceptance**

* Unit tests for:

  * `-abc`
  * `--out=V` and `--out V`
  * `--` behavior
  * unknown option errors
  * required args errors

---

### CLI‑05 — Help/usage renderer

**Work**

* Deterministic formatting:

  * stable ordering
  * width wrapping with stable rules
* Provide `help.render` and `help.usage_short`

**Acceptance**

* Golden tests for help output text.
* Help exists and is auto‑renderable (argparse/clap parity goal). ([Python documentation][3])

---

### CLI‑06 — Completion generation

**Work**

* `complete.render(specbin, shell)` returns completion script bytes
* Support bash/zsh/fish first
* Document how to install completions

**Acceptance**

* Golden tests for minimal completion scripts.
* Mirrors the “generate completions from command spec” pattern (clap_complete precedent). ([Docs.rs][4])

---

### CLI‑07 — OS adapter module (`std.cli.os`)

**Work**

* Provide `argv_v1` from OS runner
* Provide stdout/stderr writes and exit
* Provide `std.cli.os.run(spec_json_bytes)` convenience:

  * handles `--help` / `--version`
  * prints messages
  * returns exit code

**Acceptance**

* Integration test: compile sample X07 CLI app and run with:

  * `--help` → stdout + exit 0
  * invalid flag → stderr + exit 2 (clap-like), or your chosen policy ([Docs.rs][5])
  * missing required arg → exit 2

---

### CLI‑08 — Toolchain helpers: `x07 cli fmt|gen`

**Work**

* `fmt` enforces canonical JSON formatting and stable ordering
* `gen` emits:

  * compiled spec blob module
  * optional completions output
  * optional manpage skeleton

**Acceptance**

* Roundtrip: `cli.spec.json` → `cli fmt` → same stable output.
* `cli gen` output is deterministic.

---

## 9) Minimal example of intended usage pattern

**Agent-friendly canonical pattern**: keep it repetitive and standard so agents learn it.

* `cli/cli.spec.json` authored by agent
* `x07 cli gen` produces `src/cli_spec.x07.json`
* `main` does:

1. load argv
2. parse
3. if error: print + exit
4. dispatch

This mirrors how argparse “defines interface then parses argv” and generates help/errors automatically. ([Python documentation][3])

---

## 10) Benchmarks / tests you should add (even without Openx07lve)

Even though you removed evaluation, you still need **regression suites**.

Add a `tests/cli/` directory with:

* parse behavior matrix
* help text golden files
* completion scripts golden files
* OS integration tests (run compiled binary with argv)

The important principle: **make behavior stable** so agent-coded apps don’t break across versions.

---

## 11) Quick answer to your underlying question: “What unblocks fully featured CLI apps?”

You need **three pieces**, not just one:

1. **A pure CLI parsing + help library** (this package).
2. **A run‑os adapter** (argv/stdout/stderr/exit).
3. **A spec authoring workflow** that avoids hand-authored deep structures:

   * flat spec rows + schema validation
   * `x07 cli fmt`
   * `x07 cli gen`

That combination is what makes it “production ready” for agentic use.

---

[1]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap12.html?utm_source=chatgpt.com "12. Utility Conventions"
[2]: https://docs.oracle.com/cd/E23824_01/html/821-1461/intro-1.html?utm_source=chatgpt.com "Description - man pages section 1: User Commands"
[3]: https://docs.python.org/3/library/argparse.html?utm_source=chatgpt.com "argparse — Parser for command-line options, arguments ..."
[4]: https://docs.rs/clap_complete/?utm_source=chatgpt.com "clap_complete - Rust"
[5]: https://docs.rs/clap/latest/clap/type.Error.html?utm_source=chatgpt.com "Error in clap - Rust"
[6]: https://man7.org/linux/man-pages/man3/sysexits.h.3head.html?utm_source=chatgpt.com "sysexits.h(3head) - Linux manual page"
[7]: https://specifications.freedesktop.org/basedir/basedir-spec-latest.html?utm_source=chatgpt.com "XDG Base Directory Specification"
[8]: https://docopt.org/?utm_source=chatgpt.com "docopt—language for description of command-line interfaces"

++++
Use docs/phases/assets/x07cli_semantic_and_bench_bundle.tar.gz

### What you’re getting (drop-in tarball)

This tarball contains **exactly** the three things you asked for:

1. `spec/x07cli.specrows.schema.json`
2. `docs/cli/cli-v1.md` (normative v1 spec)
3. `packages/x07-cli/0.1.0/...` (first concrete package skeleton with stub `.x07.json` modules)

### Extract into your repo

From your repo root:

Use 
docs/phases/assets/x07_cli_v1_bundle.tar.gz
### Files added

```
spec/x07cli.specrows.schema.json

docs/cli/cli-v1.md

packages/x07-cli/0.1.0/package.json
packages/x07-cli/0.1.0/README.md
packages/x07-cli/0.1.0/modules/std/cli/module.x07.json
packages/x07-cli/0.1.0/modules/std/cli/help.x07.json
packages/x07-cli/0.1.0/modules/std/cli/complete.x07.json
packages/x07-cli/0.1.0/modules/std/cli/os.x07.json
```

### Key design decisions baked into the spec

These are the “hard pins” that make the CLI package reliable and LLM-friendly long-term:

* **Row-based spec (“SpecRows”)** rather than nested objects, to keep diffs stable and generation easier for agents.
* **Deterministic parsing** rules:

  * Supports `--name` and `--name=value` long options (aligned with common long-option conventions).
  * Treats `--` as a delimiter that ends option parsing (end-of-options).
* **Stable error codes** for agent repair loops.
* **Usage exit code guidance**: recommend mapping usage failures to **EX_USAGE = 64** in OS mode. ([GNU][1])
* **Completion generation** is part of the package API surface (deterministic ordering), aligned with the common “generate completion scripts per shell” model.

### Notes about the stubs

The `.x07.json` modules are intentionally **stubs**:

* They establish **final names/signatures** and the intended “pure vs OS adapter” split.
* Core functions currently return deterministic placeholder errors (`result_* .err 1099`) or placeholder bytes (`bytes.lit TODO_*`).
* You can implement them incrementally without changing the user-facing contract.

[1]: https://www.gnu.org/s/libc/manual/html_node/Getopt-Long-Options.html "Getopt Long Options (The GNU C Library)"

+++++
I put together a **deterministic semantic validator + canonical “fmt” + implied defaults** (help/version) and a **tiny solve‑pure benchmark suite** that nails down the bytes encodings and determinism using a mocked argv blob.

[Download the bundle](sandbox:/mnt/data/x07cli_semantic_and_bench_bundle.tar.gz)

Bundle contents:

* `docs/cli/cli-semantic-validator-v1.md` (normative rules: semantics + canonicalization + implied defaults)
* `scripts/check_x07cli_specrows_semantic.py` (dependency‑free semantic validator + canonicalizer)
* `benchmarks/solve-pure/cli-v1-specrows-determinism.json` (tiny pure suite, 2 cases)
* `benchmarks/solve-pure/README_cli_v1.md` (input/output encoding spec for the suite)

---

## 1) CLI semantic validator (beyond JSON Schema)

Your JSON Schema (`spec/x07cli.specrows.schema.json`) ensures “shape”. The semantic validator adds **deterministic meaning-level constraints**, which is what LLM agents actually need to avoid “valid JSON but nonsense CLI”.

### Deterministic diagnostics contract

The validator produces a stable list of diagnostics items:

```json
{
  "diagnostics": [
    {"severity":"error","code":"ECLI_DUP_LONG","scope":"root","row_index":12,"message":"duplicate long option --verbose"}
  ]
}
```

Stable ordering rule used by the script:

* error before warn
* then `(code, scope, row_index)` lexicographic

This makes it easy for a repair loop to reliably learn from failures.

### Semantic rules enforced (v1)

The bundle’s `docs/cli/cli-semantic-validator-v1.md` is the single normative spec, but the key “must-have” rules are:

**Row correctness**

* Unknown row kind ⇒ `ECLI_ROW_KIND_UNKNOWN`
* `flag` / `opt` must specify at least one name (`shortOpt` or `longOpt`) ⇒ `ECLI_FLAG_NO_NAMES` / `ECLI_OPT_NO_NAMES`

**Uniqueness per scope**

* No duplicate `shortOpt` across `help/version/flag/opt` ⇒ `ECLI_DUP_SHORT`
* No duplicate `longOpt` across `help/version/flag/opt` ⇒ `ECLI_DUP_LONG`
* No duplicate `key` across `flag/opt/arg` ⇒ `ECLI_DUP_KEY`

**Reserved help/version names**

* `--help` / `-h` reserved for `help` rows only
* `--version` / `-V` reserved for `version` rows only
  If used by any other row kind ⇒ `ECLI_RESERVED_HELP_USED` / `ECLI_RESERVED_VERSION_USED`

This matches common CLI expectations and keeps agent specs predictable. (Many CLIs treat `--` as end-of-options and `-h/--help` as conventional help; POSIX-style docs describe `--` explicitly.) ([Linux Foundation RefSpecs][1])

**Arg ordering constraints**

* Only one `arg` may have `multiple=true`, and it must be the last arg:

  * `ECLI_ARG_MULTI_DUP`
  * `ECLI_ARG_MULTI_NOT_LAST`
* No required positional arg after an optional one:

  * `ECLI_ARG_REQUIRED_AFTER_OPTIONAL`

**Option type/value constraints**

* `value_kind` must be one of `STR|PATH|U32|I32|BYTES|BYTES_HEX` ⇒ `ECLI_OPT_VALUE_KIND_UNKNOWN`
* `default` must parse for that `value_kind` ⇒ `ECLI_OPT_DEFAULT_INVALID`

**Meta consistency**

* If `meta.key` exists, it must match the row’s `key` ⇒ `ECLI_META_KEY_MISMATCH`

---

## 2) Canonical row ordering (deterministic “fmt”)

Canonicalization is critical for agentic coding because it:

* prevents “diff churn”
* makes the spec stable even if the agent generates rows in random order
* ensures identical, deterministic compiled artifacts if your compiler uses row order anywhere

### Canonical order per scope

Canonicalization keeps positional args meaningful:

* `arg` rows keep their original relative order (so the formatter never changes arg semantics)

Everything else is deterministic:

Per scope ordering:

1. `about`
2. `help`
3. `version` (root only; if present elsewhere, keep it but order after help)
4. `flag` rows sorted by `(longOpt, shortOpt, key)` (empty sorts last)
5. `opt` rows sorted by `(longOpt, shortOpt, key)` (empty sorts last)
6. `arg` rows (original order)

Also:

* meta objects are normalized (unknown keys dropped; keys ordered by canonical JSON writer).

The included tool implements exactly this in `scripts/check_x07cli_specrows_semantic.py fmt`.

---

## 3) Implied defaults (help/version)

This is the most important “agent reliability” lever: it removes 2 frequent mistakes (“forgot help/version”, “used reserved flag name accidentally”) and aligns with what devs already expect.

### Help is implied for every scope

If a scope has no `help` row, insert:

* Prefer `-h` + `--help`
* If `-h` already taken, insert help with `shortOpt=""` but keep `--help`

Many parsers/frameworks auto-insert help (Python `argparse` does so by default), and this reduces boilerplate and agent mistakes. ([Python documentation][2])

### Version is implied for root only

If `root` has no `version` row, insert:

* Prefer `-V` + `--version`
* If `-V` taken, insert with `shortOpt=""` but keep `--version`

Rust `clap` similarly provides auto-generated help/version patterns, so this convention is familiar and predictable. ([Docs.rs][3])

---

## 4) The tiny pure benchmark suite

The bundle includes:

* `benchmarks/solve-pure/cli-v1-specrows-determinism.json`
* `benchmarks/solve-pure/README_cli_v1.md`

### What it asserts

It validates, in solve‑pure (no OS world needed):

* SpecRows JSON parsing is deterministic
* SpecRows compilation + argv parsing is deterministic
* implied `--help` works even if spec didn’t include help

### Input format (mocked argv blob)

Per case, `input` bytes:

```
u32_le spec_len
spec_len bytes: UTF-8 JSON (x07cli.specrows@0.1.0)
remaining bytes: argv_blob_v1
```

`argv_blob_v1`:

```
u32_le argc
repeat argc:
  u32_le len
  len bytes (UTF-8 token)
```

### Expected output format (matches_v1)

Success:

```
u8 tag = 1
u32_le cmd_len
cmd bytes (UTF-8 scope string)
u32_le entry_count
repeat entries sorted by key bytes:
  u32_le key_len
  key bytes
  u8 kind (1=flag,2=opt,3=arg)
  u32_le value_len
  value bytes
```

The suite provides two cases:

1. normal parse: `-v --output x.txt file.bin`
2. implied help parse: `--help` is accepted even if spec had no explicit help row

---

## 5) How to use this immediately

### As a CI gate (manual scripts)

Run on every `cli.specrows.json` in your repo:

```bash
python3 scripts/check_x07cli_specrows_semantic.py check path/to/cli.specrows.json
```

To enforce canonical formatting:

```bash
python3 scripts/check_x07cli_specrows_semantic.py fmt path/to/cli.specrows.json --in-place
```

### As an agent workflow

1. agent writes/edits `cli.specrows.json`
2. agent runs `fmt` (tool inserts implied defaults + canonicalizes)
3. agent runs `check` (semantic validator)
4. agent compiles/runs
5. if `check` fails, agent repairs based on deterministic `code` + `row_index`

---

If you want, I can also draft the **exact CLI surface** you expose to agents:

* `x07 cli spec fmt --in ... --out ...`
* `x07 cli spec check --in ... --diag-json ...`
* `x07 cli spec compile --in ... --out specbin`

…and wire their outputs into your existing “diagnostic → repair” loop so the agent always has a single stable contract.

[1]: https://refspecs.linuxfoundation.org/LSB_1.3.0/gLSB/gLSB/libutil-getopt-3.html "getopt"
[2]: https://python.readthedocs.io/fr/latest/library/argparse.html "16.4. argparse — Parser for command-line options, arguments and sub-commands — documentation Python 3.7.0a0"
[3]: https://docs.rs/clap/latest/clap/_tutorial/index.html "clap::_tutorial - Rust"

+++++
I meant we also need to have external CLI package that would help end-users (coding agents) to be able create their own CLI programs in X07
+++++
Got it — you don’t mean an *internal* “CLI spec validator for X07 itself”. You mean a **user-facing library package** that lets end-user coding agents build *their own* production CLI apps in X07 (subcommands, flags, help/version, exit codes, completions), and that works both:

* in **deterministic worlds** (by feeding a mocked `argv_blob` into `solve`), and
* in **run-os / run-os-sandboxed** (by reading real OS argv, printing help/errors, exiting with codes).

Below is the design I’d recommend for an **external CLI package** that is LLM/agent-friendly and still matches real-world CLI conventions.

---

## 1) What “production CLI package” should look like for X07

### Core principle

**Do not make agents hand-author nested JSON.**
Instead: agents define the CLI in a *row-based* spec (your `SpecRows` idea), then a tool **canonicalizes + compiles** it into a small binary (“specbin”) that the runtime parser consumes.

This mirrors how many mature ecosystems separate:

* the *spec / definition* of CLI,
* the *parser engine*,
* the *help/version rendering*,
* and optional *completion generation*.

### Conventions we should follow (so users’ expectations match other CLIs)

These are the conventions you almost always want:

* `--` ends option parsing; everything after is positional operands. ([The Open Group][1])
* GNU-style long options can accept arguments as either `--arg value` or `--arg=value`. ([GNU][2])
* Auto-provide `-h/--help` by default (common behavior in major parsers).
* Return a “usage” exit code for CLI misuse; the classic Unix `sysexits` convention uses `EX_USAGE = 64`.

You can still keep X07’s “one canonical way”: **support these behaviors, but don’t add multiple competing DSLs.**

---

## 2) Package split: keep “pure parsing” separate from “OS wiring”

To keep determinism clean and adoption smooth, split the *package surface* like this:

### Package: `x07:cli@0.1.0` (pure + deterministic)

Module IDs (suggested):

* `std.cli.specrows` — types + helpers around SpecRows
* `std.cli.specbin` — decode specbin for runtime use
* `std.cli.parse` — parse `argv_blob_v1` + specbin → matches
* `std.cli.matches` — accessor helpers (get flag, get opt, get positionals)
* `std.cli.render` — optional: render help/version/completions **from specbin**

This package must be usable in **solve-pure**.

### Package: `x07:cli-os@0.1.0` (run-os/run-os-sandboxed only)

Module IDs:

* `std.os.cli` — glue: read OS argv, call `std.cli.parse`, print help/errors, exit

This keeps “real OS stuff” out of deterministic runs.

---

## 3) The one canonical input: `argv_blob_v1`

To test deterministically *without* OS worlds, define one bytes encoding for argv:

**`argv_blob_v1` encoding (bytes):**

* `u32_le argc`
* repeated `argc` times:

  * `u32_le len`
  * `len` bytes UTF-8 (argument text, no trailing NUL)

This is deterministic, portable, and easy to generate for tests.

In **run-os**, your OS runner converts `argv[]` into the same blob and hands it to the program (or makes it available via a builtin like `(os.argv_v1)`).

---

## 4) CLI definition format: SpecRows + compiled SpecBin

### SpecRows (agent-authored, easy diffs)

Keep your row-based format, but make it even more “agent-editable”:

* prefer “flat rows” with string keys
* avoid deeply nested objects
* allow JSONL (one row per line) as an alternate *source format* if you want even easier patching

### SpecBin (tool-generated, runtime-consumed)

A deterministic binary encoding compiled from SpecRows:

* stable ordering
* stable hashing of identifiers
* direct indices instead of string lookups at runtime
* precomputed help strings (optional)

**Why SpecBin matters for agentic reliability**
It eliminates a whole class of runtime complexity and “LLM mistakes”:

* program never parses JSON at runtime
* the compiler/toolchain catches spec errors early
* the runtime parser is small and deterministic

---

## 5) Runtime parse API: minimal but complete

### Proposed pure API (in `std.cli.parse`)

**`std.cli.parse_v1(specbin: bytes, argv: bytes) -> result_bytes`**

* `Ok(matches_v1_bytes)`
* `Err(code)` where `code` is one of the stable CLI error codes.

Why `result_bytes`:

* you already have a deterministic Result ABI
* errors become machine-parseable, not stringly

### `matches_v1` encoding

Keep it “flat, canonical, stable order”:

* `u32_le selected_command_id` (0 = root command)
* `u32_le flags_count`, then list of `u32_le flag_id`
* `u32_le opts_count`, then list of:

  * `u32_le opt_id`
  * `u32_le value_len`
  * `value_bytes`
* `u32_le positionals_count`, then list of:

  * `u32_le len`
  * `bytes`

Everything sorted by `(kind, id)` for stability.

### Accessors (`std.cli.matches`)

To keep agent code short and correct:

* `matches.has_flag(matches, "verbose") -> i32`
* `matches.get_opt_or(matches, "output", default_bytes) -> bytes`
* `matches.get_pos(matches, idx, default_bytes) -> bytes`

This is the “one canonical way” to consume parse output.

---

## 6) Help/version defaults (implied rows)

Implied defaults are important because agents forget to implement them.

**Hard rule**: if SpecRows doesn’t define help/version, the compiler inserts them.

* `-h` and `--help` always exist (unless explicitly disabled)
* `--version` exists if package metadata provides a version string

This matches common CLI parser behavior.

### Help rendering

You can do either:

**Option A (recommended): precompute help text in `specbin` at build time**

* deterministic
* fast runtime
* minimal stdlib complexity

**Option B: render at runtime from `specbin`**

* more flexible
* slightly more code

Agents generally benefit from Option A.

---

## 7) Completion scripts (optional but “production-ready”)

Completion generation is extremely useful for adoption, and many ecosystems provide it as a subcommand or flag.

Common pattern:

* `mytool completions --shell bash`
* prints a shell completion script to stdout

This is how many tools do it, including ecosystems that generate scripts from the CLI definition.

For X07:

* implement `std.cli.render_completion(specbin, shell_id)` → bytes (script)
* in `std.os.cli`, expose a default `completions` subcommand if requested by spec

Keep it deterministic; no runtime “dynamic completion” callbacks (those tend to be fragile across versions).

---

## 8) Exit codes + error reporting contract

### Exit codes

Use:

* `0` success
* `64` for CLI misuse / parse error (matches `EX_USAGE`)
* `1` for runtime failure not related to parsing

### Error bytes for parse failures

When parse fails, `std.cli.parse_v1` returns `Err(code)`, and a separate function provides a deterministic message:

* `std.cli.err_to_message(code) -> bytes`
* plus `std.cli.render_usage(specbin, selected_cmd)` for context

So the OS adapter can print:

* the error line
* usage/help excerpt

And the agent can learn from structured codes.

---

## 9) What’s missing today (based on what you’ve built so far)

From your last message, you already have:

* SpecRows schema + semantic validator + canonicalization + implied defaults direction
* a deterministic benchmark for spec determinism using mocked argv

What’s *still required* to make this truly an **external CLI package for end-user apps**:

1. **Runtime parser engine**

   * accepts `argv_blob_v1`
   * implements:

     * short flags grouping (`-abc`), if you want it
     * long options with `--x=y` and `--x y` ([GNU][2])
     * `--` end-of-options ([The Open Group][1])
     * subcommands
     * positionals
2. **Matches encoding + accessors**
3. **OS adapter module**

   * read argv from OS
   * print help/errors
   * exit with correct code
4. **End-user workflow glue**

   * `x07 cli spec fmt/check/compile`
   * generated `cli.specbin` (or generated module containing embedded specbin bytes)

---

## 10) Concrete development plan (high-level, production oriented)

### Milestone 1 — MVP end-user CLI apps (no completions yet)

* Implement `argv_blob_v1` and `matches_v1`
* Implement `std.cli.parse_v1`
* Implement `std.cli.matches.*` accessors
* Implement `std.os.cli.main_v1(specbin_path_or_bytes, user_main)` pattern
* Add 10–15 unit tests:

  * flags, opts, `--`, subcommands, positionals, errors

### Milestone 2 — Help/version + deterministic messaging

* Build-time help generation (preferred)
* `std.os.cli` prints deterministic help
* Standardize parse error diagnostics and exit code 64

### Milestone 3 — Completions

* `std.cli.render_completion`
* `std.os.cli` exposes a completions subcommand

### Milestone 4 — Agentic editing tools (reduces JSON pain)

Instead of agents “editing JSON by hand”, give them a tool API:

* `x07 cli spec add-flag ...`
* `x07 cli spec add-opt ...`
* `x07 cli spec add-cmd ...`
* `x07 cli spec rm ...`
* always emits canonical SpecRows

This directly addresses your recurring pain: agents are bad at deeply nested JSON diffs. Row-ops let them manipulate structure safely.

---

## 11) Quick answer: what to do next

If your goal is “agents can build CLI apps today”, the next “unblocking” implementation is:

* **finish `std.cli.parse_v1(specbin, argv_blob) -> result_bytes`**
* **finish `std.cli.matches` accessors**
* **finish `std.os.cli` adapter for run-os/run-os-sandboxed**
* **wire `x07 cli spec compile` so apps never parse JSON at runtime**

Your SpecRows + semantic validator work was the right foundation — it just needs the runtime parser + OS glue to become a real external user-facing package.

[1]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap12.html?utm_source=chatgpt.com "12. Utility Conventions"
[2]: https://www.gnu.org/software/libc/manual/html_node/Argument-Syntax.html?utm_source=chatgpt.com "Argument Syntax (The GNU C Library)"
++++++
Yes — **a `std.cli` package can absolutely play the same role in X07 that `clap` plays in Rust**, as long as you design it around X07’s core constraints:

* **LLM-first authoring** (the agent should *declare* CLI shape, not hand-build deeply nested structures)
* **deterministic parsing + deterministic diagnostics**
* **stable encodings** (so downstream tools/agents can depend on them)
* **portable conventions** (POSIX-ish option grammar)

What differs is *how* you express the spec (Clap has Rust derive + builder APIs; X07 should use a compact declarative spec like your `SpecRows`), and *how* you return results (likely as a stable “matches blob” + typed getters, not a dynamic map).

Below is a concrete “Clap → std.cli” mapping and the recommended structure for production/agentic usage.

---

## What “Clap-like” really means (and what to copy)

### Clap behaviors worth matching

1. **Declarative spec → parsed matches**

   * In Clap: you define a `Command` + `Arg`s and get `ArgMatches`.
   * In X07: you define `SpecRows` / `SpecBin` and get `Matches` + errors.

2. **Auto `--help` and `--version`**

   * Clap reserves and auto-adds the help and version flags by default; `-h/--help` and `-V/--version` are standard defaults unless disabled. ([Docs.rs][1])
     **Recommendation:** your `std.cli` semantic validator should *implicitly inject* help/version rows (unless explicitly disabled by a row flag), and it should make them non-overridable to prevent spec drift.

3. **Standard option grammar**

   * Long options often accept `--opt=VALUE` and `--opt VALUE`, and `--` terminates option parsing. ([Linux Foundation Specs][2])
     **Recommendation:** implement these exact conventions so agents can rely on “what every CLI does”.

4. **Completion script generation**

   * Clap’s ecosystem includes `clap_complete` which can generate shell completions. ([Docs.rs][3])
     **Recommendation:** add a companion module/tool that emits completion scripts deterministically from `SpecBin`.

5. **Exit code conventions**

   * Clap’s `Error::exit_code()` returns **2** when it prints to stderr (typical usage error), else **0** for help/version. ([Docs.rs][4])
     Many Unix tools use `sysexits.h` values (e.g. `EX_USAGE` = 64). ([man7.org][5])
     **Recommendation for X07:** keep **library-level parsing return codes deterministic** (no “exit” inside the library), and let the app choose:
   * **Portable default:** usage errors → exit **2** (Clap-like)
   * **Optional policy:** usage errors → exit **64** (`EX_USAGE`) for Unix-y tooling

---

## The right shape for `std.cli` in X07 (LLM-first, “one canonical way”)

### 1) Keep the *spec* outside program logic

Clap’s nicest property is you don’t “manually parse argv”; you describe the CLI. You already moved in this direction with `SpecRows`.

**For X07, the best “Clap-like” design is:**

* Author CLI spec in a flat file: `cli.specrows.json` (or `.x07.json` if you want it to live inside your package)
* Tooling compiles it into a compact canonical blob: `cli.specbin` (deterministic binary)
* Program uses a single entrypoint:

```
std.cli.parse_v1(specbin_bytes, argv_blob_bytes) -> result_cli_matches_or_err_bytes
```

Why this is better for agentic coding:

* agents are much better at editing a *table of rows* than writing deeply nested JSON
* you can validate, normalize, and auto-fill defaults at build time

### 2) Return a stable “matches blob” + typed getters (not a dynamic map)

In agentic settings, “map-like” results cause ambiguity and brittle string-key errors.

Instead, do what you already do elsewhere in X07:

* `matches` is an opaque bytes blob (or a typed record) with deterministic encoding
* access is via stable getter functions:

Examples:

* `std.cli.has_flag(matches, flag_id_bytes) -> i32`
* `std.cli.get_opt_bytes(matches, opt_id_bytes) -> option_bytes`
* `std.cli.get_opt_i32(matches, opt_id_bytes) -> option_i32`
* `std.cli.get_pos_bytes(matches, pos_index_i32) -> option_bytes`
* `std.cli.subcommand_id(matches) -> option_bytes`

This is essentially **Clap’s `ArgMatches`** but with an ABI designed for X07’s constraints.

### 3) Error output must be machine-repairable

Instead of “pretty” human errors, aim for:

* deterministic diagnostic codes (your `x07diag` approach)
* deterministic error payload fields:

  * `kind` (unknown flag, missing required arg, invalid value, etc.)
  * `arg_id`
  * `span` in argv tokens (index range)
  * `message` (optional human text)
  * `suggestions` (did-you-mean)

Then the agent can self-repair based on structured data.

Clap does have structured `ErrorKind` (and can print + exit), but your agentic loop wants “error as data” by default.

---

## So… can `std.cli` be used like Clap?

**Yes — if you make it a declarative, validated CLI spec + deterministic parser + typed match accessors + automatic help/version.**

You will not replicate Clap’s *Rust ergonomics* (derive macros) verbatim, but you can replicate its *capabilities* and its *mental model* for end users:

* “Define spec”
* “Parse argv”
* “Dispatch”
* “Auto help/version/completions”
* “Errors are deterministic + actionable”

That’s effectively “Clap, but for an LLM-first language.”

---

## Practical recommendations for your next iteration of `std.cli`

### A. Lock down the parsing grammar (don’t let it drift)

Implement as normative:

* `--` stops option parsing ([Linux Foundation Specs][2])
* `--long=value` and `--long value` are both allowed ([Linux Foundation Specs][2])
* `-abc` short-group expansion (optional but common) ([Linux Foundation Specs][2])
* deterministic conflict resolution (e.g., “last wins” or “first wins”, but fixed)

### B. Make help/version *implied defaults*

Match Clap’s default behavior:

* reserve `-h/--help` and `-V/--version` semantics ([Docs.rs][1])
* return exit_code 0 for help/version output, nonzero for real errors ([Docs.rs][4])

### C. Add `std.cli.emit_help_v1(specbin, style)` and `std.cli.emit_usage_v1(...)`

So end-user programs can do:

* if parse error: render deterministic usage + error block
* if help requested: render help and return

### D. Add a completion generator as a separate package/tool

Like clap_complete does for Clap ([Docs.rs][3]):

* `x07 cli completions --shell bash --spec cli.specrows.json`
* deterministic output (sorted, stable formatting)

### E. For agentic reliability: provide a *generator* command

Agents struggle with nested JSON; they do much better with:

* `x07 cli init` → produces starter SpecRows with placeholders
* `x07 cli add-arg ...` → modifies SpecRows safely
* `x07 cli validate --fix` → applies implied defaults + canonical ordering

This is the CLI equivalent of “don’t hand-edit lockfiles”.

---

## One decision you should make now

Do you want CLI parse errors to return:

* **exit code 2** (Clap-like, common) ([Docs.rs][4])
  or
* **exit code 64** (`EX_USAGE`) (Unix sysexits style) ([man7.org][5])

My recommendation for agentic coding:

* library returns `result_*` with a deterministic error code
* the “main template” / scaffolder chooses default exit code **2**, with an option to switch to sysexits in “Unix strict mode”

This keeps parity with Clap expectations while still supporting more “traditional” Unix conventions.

---

[1]: https://docs.rs/clap/latest/clap/struct.Arg.html "Arg in clap - Rust"
[2]: https://refspecs.linuxfoundation.org/LSB_1.3.0/gLSB/gLSB/libutil-getopt-3.html "getopt"
[3]: https://docs.rs/clap_complete/ "clap_complete - Rust"
[4]: https://docs.rs/clap/latest/clap/error/struct.Error.html "Error in clap::error - Rust"
[5]: https://man7.org/linux/man-pages/man3/sysexits.h.3head.html "sysexits.h(3head) - Linux manual page"

+++++++
Yes — **`std.cli` can absolutely play the same role for X07 that `clap` plays for Rust**, but to keep it “LLM-reliable”, the v1 surface should be:

* **tiny**
* **pure + deterministic**
* **zero global state**
* **stable byte encodings**
* **pre-rendered error/help text** (so programs don’t have to format anything)

Below is a concrete **minimal `std.cli v1` API** that hits your goals (parse/help/version/matches getters/errors) while staying small enough that agents can “pattern match” usage.

---

## Design goals for `std.cli v1`

1. **Single canonical workflow**

   * Call `parse_v1(specbin, argv_v1)` → get either matches or error.
   * If user asked help/version, you still get a *normal* parse result but with reserved flags set; your program decides whether to exit early. This avoids “exceptions as control flow”.

2. **CLI conventions you should follow**

   * `--` ends option scanning. ([man7.org][1])
   * Long options accept `--opt=arg` **and** `--opt arg`. ([Oracle Docs][2])
   * Exit code convention: parse errors exit with `2`, help/version exit `0` (this matches common CLI behavior and matches how `clap` exposes exit codes for errors/help/version). ([Docs.rs][3])
   * (Optional later) If you ever want “POSIX-ish” usage exit codes, `sysexits.h` defines `EX_USAGE=64`, etc. ([man7.org][4])
     For v1, I recommend you **do not** use `64` yet; stick to `0/2` because it’s what most agent-generated CLIs will assume.

3. **Keep parsing pure**

   * `std.cli` **never reads OS args itself**.
   * OS worlds provide `argv_v1` via `std.os.argv_v1()` (adapter), while `solve-*` tests can provide argv blobs directly.

---

## Data contracts (bytes layouts)

All layouts are **little-endian** and **length-prefixed**; all strings/IDs are **raw bytes** (usually UTF‑8/ASCII), but the library treats them as opaque bytes.

### `argv_v1` encoding

```
argv_v1 :=
  u32_le argc
  repeat argc times:
    u32_le len
    [len bytes]  // argument bytes, no trailing NUL
```

Notes:

* `argv[0]` should be the program name (as usual), but `std.cli` shouldn’t require it.

### `parse_out_v1` encoding

```
parse_out_v1 :=
  u8 tag   // 1=OK, 0=ERR
  u32_le payload_len
  [payload_len bytes]  // matches_v1 or err_v1
```

### `matches_v1` encoding

Minimal but sufficient for “clap-like” usage:

```
matches_v1 :=
  // selected command (leaf), empty means root
  u32_le cmd_id_len
  [cmd_id_len bytes]

  // flags set (unique, sorted)
  u32_le flags_count
  repeat flags_count times:
    u32_le id_len
    [id_len bytes]

  // options map (unique ids, last-wins, sorted by id)
  u32_le opts_count
  repeat opts_count times:
    u32_le id_len
    [id_len bytes]
    u32_le val_len
    [val_len bytes]

  // positional args (in order)
  u32_le pos_count
  repeat pos_count times:
    u32_le val_len
    [val_len bytes]
```

Determinism rules:

* `flags` and `opts` are **sorted lexicographically by `id_bytes`** (bytewise).
* Options are **last-wins** for repeated occurrences (v1).
* Positionals preserve the original order.

### `err_v1` encoding

Pre-render everything, so programs just print.

```
err_v1 :=
  i32_le code          // stable error kind code
  i32_le exit_code     // recommended: 2 for parse errors
  u32_le render_len
  [render_len bytes]   // UTF-8 message + usage + hint, already formatted
```

---

## Reserved flag IDs (v1)

To keep agents from guessing:

* `help`:

  * `-h`, `--help`
* `version`:

  * `-V`, `--version`

These are consistent with the conventional “auto help/version” behavior many CLIs have; `clap` also provides these conventions and treats them as special cases for exit codes. ([Docs.rs][3])

**Rule:** the compiler/tooling that produces `specbin` must auto-inject these two flags unless explicitly disabled (don’t add disable in v1).

---

## Minimal `std.cli v1` API surface

This is the “exact minimal set” I’d ship.

### 1) Parsing

* **`std.cli.parse_v1(specbin: bytes, argv_v1: bytes) -> bytes`**

  * returns `parse_out_v1`

### 2) Parse-out helpers

* **`std.cli.out.is_ok(out: bytes) -> i32`**

  * returns `1` if tag==1 else `0`

* **`std.cli.out.matches(out: bytes) -> bytes`**

  * returns `matches_v1` payload if ok, else empty bytes

* **`std.cli.out.error(out: bytes) -> bytes`**

  * returns `err_v1` payload if err, else empty bytes

### 3) Matches getters

All getters are intentionally “no surprises”:

* **`std.cli.matches.cmd_id(m: bytes) -> bytes`**

  * returns leaf command id bytes (empty means root)

* **`std.cli.matches.has_flag(m: bytes, id: bytes) -> i32`**

  * returns 1 if present else 0

* **`std.cli.matches.get_opt_bytes_or(m: bytes, id: bytes, default: bytes) -> bytes`**

  * returns option value bytes (post-validated by parser), else default

* **`std.cli.matches.get_opt_i32_or(m: bytes, id: bytes, default: i32) -> i32`**

  * v1 rule: option value bytes must be ASCII decimal with optional leading `-`
  * parse success guarantees validity; getter just converts
  * returns default if missing

* **`std.cli.matches.get_pos_bytes_or(m: bytes, index_i32: i32, default: bytes) -> bytes`**

  * 0-based index
  * returns default if out-of-range

* **`std.cli.matches.get_pos_i32_or(m: bytes, index_i32: i32, default: i32) -> i32`**

  * same conversion rule as `get_opt_i32_or`

That’s it. No iterators. No visitors. No “advanced match objects”.

### 4) Help/version rendering

These are pure formatters from `specbin`:

* **`std.cli.render_help_v1(specbin: bytes, cmd_id: bytes) -> bytes`**

  * `cmd_id` empty means root help
  * output is UTF‑8 bytes, deterministic (no locale)

* **`std.cli.render_version_v1(specbin: bytes) -> bytes`**

  * should output something like:

    * `name version\n`
  * deterministic

### 5) Error getters

* **`std.cli.err.code(e: bytes) -> i32`**

* **`std.cli.err.exit_code(e: bytes) -> i32`**

  * for parse errors, return `2` (clap-style). ([Docs.rs][3])

* **`std.cli.err.render(e: bytes) -> bytes`**

  * returns pre-rendered message (already includes usage)

---

## Minimal error code enum (stable i32)

Keep it short and stable. Example:

| Code | Name                   | Meaning                             | Recommended exit |
| ---: | ---------------------- | ----------------------------------- | ---------------: |
|    1 | `UNKNOWN_OPTION`       | `-z` or `--nope`                    |                2 |
|    2 | `MISSING_VALUE`        | `--out` requires a value            |                2 |
|    3 | `INVALID_VALUE`        | `--count abc` invalid integer/range |                2 |
|    4 | `UNEXPECTED_VALUE`     | flag given a value when not allowed |                2 |
|    5 | `UNKNOWN_COMMAND`      | subcommand not recognized           |                2 |
|    6 | `TOO_MANY_POSITIONALS` | more args than spec allows (v1)     |                2 |
|    7 | `SPEC_INVALID`         | specbin corrupted/wrong version     |                2 |
|    8 | `ARGV_INVALID`         | argv blob malformed                 |                2 |

Help/version **should not be an “error”**:

* parse returns ok matches; your program checks flags:

  * `has_flag(m, "help")` → print help and exit 0
  * `has_flag(m, "version")` → print version and exit 0

This avoids needing `HELP_REQUESTED` or `VERSION_REQUESTED` error codes.

---

## Canonical program pattern agents should learn

This pattern should be embedded in your generated guide and repeated everywhere:

```lisp
(import std.cli std.os.cli) ; std.os.cli provides argv + stdout/stderr/exit

(begin
  (let spec CLI_SPECBIN)            ; bytes constant embedded by build tool
  (let argv (std.os.cli.argv_v1))
  (let out (std.cli.parse_v1 spec argv))

  (if (std.cli.out.is_ok out)
    (begin
      (let m (std.cli.out.matches out))

      (if (std.cli.matches.has_flag m (bytes.lit help))
        (begin
          (std.os.cli.stdout_write (std.cli.render_help_v1 spec (std.cli.matches.cmd_id m)))
          (std.os.cli.exit 0))
        0)

      (if (std.cli.matches.has_flag m (bytes.lit version))
        (begin
          (std.os.cli.stdout_write (std.cli.render_version_v1 spec))
          (std.os.cli.exit 0))
        0)

      ; dispatch using cmd_id and getters...
      (main.run_with_matches m))
    (begin
      (let e (std.cli.out.error out))
      (std.os.cli.stderr_write (std.cli.err.render e))
      (std.os.cli.exit (std.cli.err.exit_code e)))))
```

The core “agent trick” here is:

* do not do clever parsing yourself
* always use the same getters
* always handle help/version before anything else

---

## What `std.cli v1` intentionally does NOT include

To keep LLM reliability high, I would *not* ship these in v1:

* optional option-arguments (`--flag[=x]`)
* repeated values (`--include a --include b`) as arrays
* complex subcommand nesting paths + inherited options (beyond leaf cmd id)
* shell completion generation
* env var fallback parsing
* “negatable flags” (`--foo/--no-foo`)
* unicode casing/locale stuff in help

Those can all come later once the basics are proven stable with agents.

---

## Why this is “clap-like” but better for agents

* Clap is extremely feature-rich; agents frequently misuse it without docs.
* This v1 `std.cli`:

  * has a *single canonical parse flow*
  * returns *pre-rendered* deterministic error strings (no formatting burden)
  * uses a *single stable matches encoding*
  * uses conventions for `--`, `--opt=arg`, `--help/-h`, `--version/-V` that match ubiquitous tooling. ([man7.org][1])

[1]: https://man7.org/linux/man-pages/man3/getopt.3.html?utm_source=chatgpt.com "getopt(3) - Linux manual page"
[2]: https://docs.oracle.com/cd/E86824_01/html/E54766/getopt-long-3c.html?utm_source=chatgpt.com "getopt_long - man pages section 3: Basic Library Functions"
[3]: https://docs.rs/clap/latest/clap/error/struct.Error.html?utm_source=chatgpt.com "Error in clap::error - Rust"
[4]: https://man7.org/linux/man-pages/man3/sysexits.h.3head.html?utm_source=chatgpt.com "sysexits.h(3head) - Linux manual page"
++++

Below is a **copy/paste‑ready, implementable v1** that gives you:

1. a **binary `specbin_v1` contract** (fields + exact layout),
2. a **deterministic “spec compiler”** pipeline (SpecRows → specbin), and
3. a **`std.os.cli` adapter API** for `run-os` / `run-os-sandboxed` (argv/stdout/stderr/exit).

I’m intentionally borrowing a couple of widely‑accepted CLI parsing conventions:

* `--` ends option parsing (everything after becomes positional args). ([Ubuntu Manpages][1])
* long options may pass values as `--opt value` or `--opt=value`; short options may pass values as `-ovalue`. ([Shadow][2])

These are “boring” but they massively reduce surprises for agents and real users.

---

## 1) specbin_v1 contract

### 1.1 Goals

* **Pure parse core:** `std.cli` should parse argv *without* requiring OS access (so you can test it in `solve-pure` using a mocked argv blob).
* **Small + deterministic:** same SpecRows ⇒ same specbin bytes.
* **LLM-friendly:** stable IDs, stable help formatting, deterministic error codes.

### 1.2 Endianness, integers, sentinels

* All integers are **little‑endian**.
* All sizes/counts/offsets are `u32`.
* “None” sentinel for indices: `U32_NONE = 0xFFFF_FFFF`.

### 1.3 `argv_v1` binary encoding (used by std.os.cli)

Even though you asked for specbin, **argv encoding must be nailed down**, because it’s the bridge from OS → pure parsing.

**`argv_v1` bytes:**

```
u32 argc
repeat argc times:
  u32 len
  u8[len] bytes  // raw bytes of arg; SHOULD be UTF-8, but treated as bytes
```

Rules:

* `argc >= 1` (argv[0] exists; program name).
* Each `len` must be <= some cap (recommend 64KiB).
* Parser treats args as bytes; it does not interpret escapes.

### 1.4 High-level specbin sections

`specbin_v1` is a self-contained binary container with 5 sections:

1. **HEADER**
2. **STRINGS**
3. **META**
4. **ARGS**
5. **COMMANDS**
6. **HELP**

(Yes, that’s 6 including header. “Sections” below refer to the payload ones.)

### 1.5 Header layout

Fixed-size header (no section table). This makes the runtime reader simple and hard to break.

**`specbin_v1` header (all u32 LE unless specified):**

| Field       |      Type | Value / meaning            |
| ----------- | --------: | -------------------------- |
| magic       | `[u8; 8]` | ASCII `X07CLI01`           |
| total_len   |       u32 | total file length in bytes |
| strings_off |       u32 | offset of STRINGS section  |
| strings_len |       u32 | length of STRINGS section  |
| meta_off    |       u32 | META section offset        |
| meta_len    |       u32 | META section length        |
| args_off    |       u32 | ARGS section offset        |
| args_len    |       u32 | ARGS section length        |
| cmds_off    |       u32 | COMMANDS section offset    |
| cmds_len    |       u32 | COMMANDS section length    |
| help_off    |       u32 | HELP section offset        |
| help_len    |       u32 | HELP section length        |
| reserved0   |       u32 | must be 0                  |
| reserved1   |       u32 | must be 0                  |

Validation rules:

* `magic` must match.
* All `(off,len)` must be within `[header_len, total_len]`.
* Sections must not overlap (strict).
* `reserved0/1 == 0` (future‑proofing).

### 1.6 STRINGS section

All strings are stored once, referenced by index.

**Layout:**

```
u32 string_count
repeat string_count:
  u32 len
  u8[len] bytes
```

Rules:

* String bytes are arbitrary bytes; conventionally UTF‑8.
* The **string table is sorted lexicographically by raw bytes** and deduplicated (determinism).

### 1.7 META section

**Layout:**

```
u32 package_name_str   // STRINGS index
u32 package_version_str
u32 package_about_str  // or U32_NONE
u32 root_cmd_index     // MUST be 0 in v1
u32 parse_flags        // bitset (below)
u32 reserved0          // 0
```

`parse_flags` bits (v1):

* bit0: allow short bundling (`-abc`) **ON by default**
* bit1: allow `--opt=value` **ON by default**
* bit2: allow short attached value (`-ovalue`) **ON by default**
* bit3: allow long abbreviations (GNU‑style) **OFF in v1** (agents hate ambiguity)

### 1.8 ARGS section

Args are global records referenced by commands (via indices).

#### Arg kinds + value kinds

* `kind`:

  * `1 = FLAG`
  * `2 = OPT`
  * `3 = POS` (positional)
* `value_kind`:

  * `0 = NONE`
  * `1 = BYTES`
  * `2 = I32` (ASCII decimal parse happens in getters, not during tokenization)

#### Arg record (fixed 32 bytes)

```
u8  kind
u8  value_kind
u8  required      // 0/1
u8  multiple      // 0/1 (only POS may set multiple=1, and only if it is the last positional)
u32 id_str        // stable id for matches getters (STRINGS index)
u32 long_str      // for FLAG/OPT: long name without leading "--" (STRINGS index) else U32_NONE
u32 short_cp      // Unicode scalar (usually ASCII), 0 if none
u32 help_str      // STRINGS index or U32_NONE
u32 default_str   // STRINGS index or U32_NONE
u32 reserved0     // 0
```

#### `arg_id_map` (for getters)

After arg records, ARGS section includes a sorted id→arg_index map:

```
u32 id_map_count
repeat id_map_count:
  u32 id_str       // STRINGS index
  u32 arg_index    // index into arg records
```

Rules:

* Sorted by `id_str`’s bytes (string value).
* Includes entries for FLAG/OPT and optionally POS (if POS has id_str != NONE).

This makes `std.cli.matches.get_*("output")` fast and deterministic.

### 1.9 COMMANDS section

Commands form a tree (root + subcommands).

Command ordering is deterministic and **root MUST be cmd_index 0**.

#### Command record (fixed)

```
u32 cmd_id_str     // stable command id (STRINGS index) e.g. "app" or "app.scan"
u32 cmd_name_str   // display token used in argv (STRINGS index), usually last path component
u32 about_str      // STRINGS index or U32_NONE
u32 parent_cmd     // cmd index or U32_NONE for root

u32 flags_begin
u32 flags_len
u32 opts_begin
u32 opts_len
u32 pos_begin
u32 pos_len
u32 sub_begin
u32 sub_len

u32 reserved0      // 0
u32 reserved1      // 0
```

After `cmd_count` command records, COMMANDS section contains the flat index arrays:

```
u32 cmd_count

command_rec[cmd_count]

u32 flags_index_count
u32 flags_index[flags_index_count]    // u32 arg_index

u32 opts_index_count
u32 opts_index[opts_index_count]      // u32 arg_index

u32 pos_index_count
u32 pos_index[pos_index_count]        // u32 arg_index (arg.kind == POS)

u32 sub_index_count
u32 sub_index[sub_index_count]        // u32 cmd_index
```

For each command, `*_begin/len` ranges refer into the corresponding flat arrays.

Rules:

* `sub_index` lists children commands; sorted by `cmd_name_str` bytes.
* `flags_index` and `opts_index` sorted by `(long_str bytes, short_cp, id_str)`.

### 1.10 HELP section

You want help/version to be reliable **without the agent re-implementing formatting**.

HELP section stores pre-rendered strings per command.

**Layout:**

```
u32 cmd_count
repeat cmd_count:
  u32 usage_len
  u8[usage_len] usage_bytes     // ASCII/UTF-8, '\n' line endings, no color
  u32 help_len
  u8[help_len] help_bytes       // full help text (includes usage)
u32 version_len
u8[version_len] version_bytes
```

Rules:

* Deterministic formatting (exact rules belong in docs, but implement now as: fixed headings, sorted args).

### 1.11 Built-in implied args: help + version

The spec compiler MUST inject two flags (unless the user already defined the same IDs with identical semantics):

* `help`:

  * id: `"help"`
  * long: `"help"`
  * short: `'h'`
* `version`:

  * id: `"version"`
  * long: `"version"`
  * short: `'V'`

Reason: agents will always need these; making them implied keeps SpecRows small, and keeps behavior consistent across all CLIs.

---

## 2) Deterministic “spec compiler” (SpecRows → specbin)

### 2.1 Inputs / outputs

**Input:** `cli.specrows.json` (your existing SpecRows v1 JSON).

**Outputs:**

* `cli.specbin` (bytes, specbin_v1)
* `cli.specrows.canon.json` (optional debug artifact: canonicalized rows)
* `cli.spec.report.json` (optional: counts, hashes, determinism info)

### 2.2 Deterministic pipeline (step-by-step)

#### Step 0 — Parse + schema validate

* Load JSON.
* Validate against `spec/x07cli.specrows.schema.json`.
* Reject unknown fields (agents tend to hallucinate keys).

#### Step 1 — Semantic validator (hard errors)

The validator must be **order-independent** and emit deterministic diagnostics (sorted by `(code, path)`).

At minimum, enforce:

**Meta**

* Exactly 1 meta row.

**Commands**

* Exactly 1 root command.
* All command IDs unique.
* Parent references exist.
* No cycles.

**Args**

* For each command scope:

  * No duplicate `--long` names.
  * No duplicate `-short` names.
* Reserved tokens not reused:

  * `--` not allowed as a name.
  * `-` not allowed as a short.
* If `multiple=1` on POS, it must be the last positional.

**Global ID rules (LLM-friendly)**

* FLAG/OPT `id` values must be unique globally (recommended for agent simplicity).
* IDs `help` and `version` are reserved.

#### Step 2 — Apply implied defaults

Inject help + version flags into **every command**, unless present with identical signature.

This gives you Clap-like ergonomics with zero agent work.

(Clap supports `--opt=value` and short attached values; copying those conventions reduces surprises. ([Shadow][2]))

#### Step 3 — Canonical ordering

Canonicalize in memory first, then (optionally) emit `cli.specrows.canon.json`.

Recommended canonical sort order:

* Commands:

  * Root first.
  * Then lexicographic by **command path** (split by `.`), component-wise.
* Args per command:

  * Flags sorted by `(long, short, id)`
  * Opts sorted by `(long, short, id)`
  * Positionals sorted by `pos_index`

This makes the resulting specbin stable, regardless of how the agent ordered rows.

#### Step 4 — Build IR (typed, lossless)

Build a typed IR with explicit references (no stringly typed parent pointers).

* command table with parent pointers and children lists
* arg table
* per-command lists of arg indices (flags/opts/pos)

#### Step 5 — Intern string table deterministically

Collect all strings used:

* ids, long names, help text, about text, package name/version/about, usage/help/version render outputs

Intern & sort:

* dedupe
* sort lexicographically by bytes
* assign indices

#### Step 6 — Assign indices deterministically

* command indices based on canonical command order
* arg indices based on canonical order of all args (global list built from per-command definitions)

Build `arg_id_map` as sorted pairs.

#### Step 7 — Render help/version deterministically

Rules to keep this robust:

* No terminal width detection.
* No color.
* No locale.
* Always `\n`.

Use stable ordering:

* flags then opts then positionals then subcommands
* each group sorted as above

#### Step 8 — Emit specbin_v1

Write:

* header placeholder (fill later)
* STRINGS
* META
* ARGS
* COMMANDS
* HELP
* go back and fill header offsets/lengths/total_len

#### Step 9 — Optional determinism hash/report

Compute:

* `sha256(specrows.canon.json)` and `sha256(specbin)` for audit/debug.
* Emit `cli.spec.report.json`.

### 2.3 Runtime parsing conventions (what your specbin enables)

Your parser should implement:

* `--` ends option parsing (everything after is positional). ([Ubuntu Manpages][1])
* long values:

  * `--opt value`
  * `--opt=value` ([Shadow][2])
* short flags bundling: `-abc` => `-a -b -c`
* short option values:

  * `-ovalue`
  * `-o value` ([Shadow][2])

Reject (v1):

* long abbreviation (`--ver` matching `--version`) — too ambiguous for agents.
* “intermixed” opts after positionals unless you explicitly decide GNU-style behavior (recommend: once a positional is consumed, later `-x` is treated as positional unless `--` was used; this is the most deterministic/simple rule).

---

## 3) std.os.cli adapter API (run-os only)

This adapter is the **only OS touchpoint** for CLI programs. Everything else stays in pure `std.cli`.

### 3.1 World gating

* `std.os.cli` is available in:

  * `run-os`
  * `run-os-sandboxed`
* It MUST be a **hard compile error** in any deterministic `solve-*` world.

### 3.2 Builtins (lowest layer, provided by runner)

These are the host calls the compiler recognizes in run-os worlds.

* `os.cli.argv_v1() -> bytes`
* `os.cli.stdout_write_all(b: bytes) -> i32`
* `os.cli.stderr_write_all(b: bytes) -> i32`
* `os.cli.exit(code_i32: i32) -> i32` (does not return; if it must return, return 0)

Semantics:

* `argv_v1` returns bytes in the `argv_v1` format defined above.
* `stdout_write_all` / `stderr_write_all`:

  * must attempt to write *all* bytes (loop on partial writes)
  * returns `1` on success, `0` on error
* `exit(code)` terminates the process with that exit code.

### 3.3 `std.os.cli` module surface (what users call)

Module: `stdlib/std/<ver>/modules/std/os/cli.x07.json`

API (minimal, stable):

* `(std.os.cli.argv_v1)` -> `bytes`
* `(std.os.cli.stdout_write_all b)` -> `i32`
* `(std.os.cli.stderr_write_all b)` -> `i32`
* `(std.os.cli.exit code)` -> `i32`

That’s it. No buffering, no formatting, no parsing here.

### 3.4 How it all ties together (intended usage pattern)

* Program embeds/loads specbin bytes (build-time).
* Program gets argv:

  * in `run-os`: `(std.os.cli.argv_v1)`
  * in `solve-pure`: argv comes from input fixture
* Program calls pure parser:

  * `(std.cli.parse_v1 specbin argv_v1)` -> result blob
* On help/version/error:

  * print bytes via `stdout_write_all` or `stderr_write_all`
  * exit with code

**Exit code recommendation**

* `0` success/help/version
* `2` usage/parse error (matches common CLI conventions; clap uses 2 for usage errors).

---

## What you can implement immediately without revisiting decisions

If you implement exactly the above:

* you get a **single deterministic binary contract** (specbin_v1)
* a deterministic spec compiler that makes LLM-produced SpecRows safe + stable
* a small OS adapter that keeps your `std.cli` pure and testable
* help/version become **implied** and don’t bloat agent prompts

[1]: https://manpages.ubuntu.com/manpages/bionic//man1/getopt.1.html?utm_source=chatgpt.com "getopt - parse command options (enhanced)"
[2]: https://shadow.github.io/docs/rust/clap/struct.Arg.html?utm_source=chatgpt.com "Arg in clap - Rust"
++++
Below is a **normative v1 binary contract** for what `std.cli.parse_v1` returns — **either** a `matches_v1` blob **or** an `err_v1` blob — both in the same “specbin-style” (magic + length + little‑endian fixed fields + deterministic ordering). This makes it impossible for agents to “guess” how to read flags/options or print errors: they only branch on the magic, then call stable getters.

I’m intentionally making **help/version** use the **same `err_v1` shape** (but with `exit_code=0` and `stream=STDOUT`), mirroring the common pattern used by Rust’s clap error object (`exit_code` 2 when printing to stderr, 0 otherwise). ([Docs.rs][1])

---

## 1) `std.cli.parse_v1` return type

### Function signature

* `std.cli.parse_v1(specbin_v1: bytes, argv_v1: bytes) -> bytes`

### Return value is exactly one of:

* **`matches_v1`** blob (success)
* **`err_v1`** blob (failure OR display action like help/version)

> The caller must distinguish by the first 8 bytes (magic).

---

## 2) Common encoding rules (apply to all CLI blobs)

### Endianness + primitive sizes

* All integers are **little‑endian**.
* `u32` is 4 bytes.
* `i32` is 4 bytes (two’s complement).
* `u8` is 1 byte.

### Strings / bytes fields

* Any “bytes” field is encoded as:

  * `u32_le length`
  * `length` raw bytes (no NUL terminator)

### Deterministic ordering rules

Whenever a blob contains a list of entries (flags/options/suggestions), it must be **sorted** by:

1. **lexicographic ascending** of the entry’s `id_bytes` (raw bytes compare),
2. then by secondary fields if needed (not used in v1).

### Deterministic normalization rules (parser behavior)

The parser MUST support:

* `--` as **end-of-options delimiter** (everything after is positional). ([The Open Group][2])
* long option arguments in either form:

  * `--name=value`
  * `--name value` ([GNU][3])

---

## 3) `matches_v1` encoding (success)

### Magic

* First 8 bytes: ASCII `"X07CLIM1"`
  (“X07 CLI Matches v1”)

### Layout

```
matches_v1 :=
  magic[8] = "X07CLIM1"
  u32_le total_len_bytes          ; includes header + payload, must equal len(blob)

  ; command context (leaf command that matched)
  bytes cmd_id                    ; e.g. "app" or "app.fetch"

  ; FLAGS (counted)
  u32_le flags_count
  repeat flags_count:
    bytes flag_id                 ; canonical ID (NOT alias), e.g. "help", "verbose"
    u32_le seen_count             ; how many times it appeared (0 never stored; must be >=1)

  ; OPTIONS (possibly multi-value)
  u32_le opts_count
  repeat opts_count:
    bytes opt_id                  ; canonical ID
    u32_le values_count           ; number of occurrences captured (>=1)
    repeat values_count:
      bytes value_bytes           ; raw value bytes (exact argv token or --opt=value substring)

  ; POSITIONALS
  u32_le pos_count
  repeat pos_count:
    bytes pos_value               ; raw positional token bytes in appearance order
```

### Canonicalization rules

* `flag_id` and `opt_id` MUST be **canonical IDs** as defined by `specbin_v1` (aliases are resolved during parsing).
* `flags_count` and `opts_count` lists MUST be sorted by `id_bytes`.
* `pos_value` list MUST preserve original order.

### Semantics required for agents

This encoding supports:

* `has_flag(id)` → `seen_count > 0`
* `flag_count(id)` → `seen_count` (useful for `-vvv`)
* `opt_count(id)` → `values_count` (0 if missing)
* `opt_last(id)` → last value
* positional iteration

---

## 4) `err_v1` encoding (failure OR display action)

### Magic

* First 8 bytes: ASCII `"X07CLIE1"`
  (“X07 CLI Error v1”)

### Layout

```
err_v1 :=
  magic[8] = "X07CLIE1"
  u32_le total_len_bytes        ; includes header + payload, must equal len(blob)

  i32_le code                   ; deterministic error/action code (see §5)
  i32_le exit_code              ; recommended process exit code (0 for help/version; 2 for usage errors)
  u32_le stream                 ; 1=STDOUT, 2=STDERR

  u32_le argv_index             ; 0-based index into argv tokens, or 0xFFFFFFFF if N/A

  bytes cmd_id                  ; command context where the error occurred

  bytes token                   ; offending token (or empty if N/A)

  bytes render_utf8             ; fully-rendered, ready-to-print message (UTF-8, ends with '\n')

  u32_le suggestions_count
  repeat suggestions_count:
    bytes suggestion             ; e.g. "--format", "fetch", "--help"
```

### Canonicalization rules

* `render_utf8` MUST be **valid UTF‑8** and MUST end with `\n`.
* `suggestions` MUST be sorted lexicographically.
* `stream` MUST be:

  * `1` (STDOUT) for **display actions** (help/version)
  * `2` (STDERR) for **usage errors** and parse errors

### Exit code convention

* Use `exit_code = 2` for CLI usage / parse errors (common convention; also aligns with clap’s “prints to stderr ⇒ 2”). ([Docs.rs][1])
* Use `exit_code = 0` for `--help` / `--version` display actions (prints to stdout). ([Docs.rs][1])

> Note: some Unix conventions use `EX_USAGE = 64` for usage errors. If you ever want that, you can add a `policy.exit_codes` override in v2; but keep v1 minimal. ([man7.org][4])

---

## 5) Deterministic `err_v1.code` catalog (minimal v1)

Reserve code ranges so future expansion doesn’t collide:

### Display actions (non-errors)

* `100` = `DISPLAY_HELP`
* `101` = `DISPLAY_VERSION`

### Parse/usage errors (stderr, exit_code=2)

* `200` = `UNKNOWN_OPTION`
* `201` = `MISSING_OPTION_VALUE`
* `202` = `UNEXPECTED_OPTION_VALUE` (value provided but option takes none)
* `203` = `UNKNOWN_SUBCOMMAND`
* `204` = `MISSING_REQUIRED_OPTION`
* `205` = `MISSING_REQUIRED_POSITIONAL`
* `206` = `TOO_MANY_POSITIONALS`
* `207` = `INVALID_VALUE` (failed validation rule)
* `208` = `AMBIGUOUS_LONG_OPTION` (prefix ambiguous, if you support abbreviations)

### Internal/spec/argv errors (still deterministic)

* `300` = `SPEC_INVALID`
* `301` = `ARGV_INVALID`
* `302` = `INTERNAL_PARSER_ERROR` (should be rare; still deterministic)

**Rule:** Any new code must specify:

* `exit_code`
* `stream`
* what `token`/`argv_index` mean for that code
* whether `suggestions` may be present

---

## 6) “Never guess” usage contract for agents

Your docs for `std.cli` should recommend *exactly one* canonical pattern:

### Canonical main flow (run‑os world)

1. Build or load `specbin_v1` (from SpecRows compiler or embedded constant).
2. Get `argv_v1` from OS adapter.
3. `out = std.cli.parse_v1(specbin, argv_v1)`
4. If `out` is `err_v1`:

   * print `render_utf8` to `stream`
   * exit `exit_code`
5. Else `out` is `matches_v1`:

   * proceed using getters (never manual parsing in user code)

### Minimum getters to implement in `std.cli` (so agents don’t parse blobs)

These can be small pure functions (bytes parsing) and/or builtins for speed later:

**Type checks**

* `std.cli.is_matches_v1(out: bytes) -> i32`
* `std.cli.is_err_v1(out: bytes) -> i32`

**Matches**

* `std.cli.matches.cmd_id(m: bytes) -> bytes_view`
* `std.cli.matches.has_flag(m: bytes, id: bytes) -> i32`
* `std.cli.matches.flag_count(m: bytes, id: bytes) -> i32`
* `std.cli.matches.opt_count(m: bytes, id: bytes) -> i32`
* `std.cli.matches.opt_last(m: bytes, id: bytes, default: bytes_view) -> bytes_view`
* `std.cli.matches.opt_n(m: bytes, id: bytes, n: i32, default: bytes_view) -> bytes_view`
* `std.cli.matches.pos_count(m: bytes) -> i32`
* `std.cli.matches.pos_n(m: bytes, n: i32, default: bytes_view) -> bytes_view`

**Err**

* `std.cli.err.code(e: bytes) -> i32`
* `std.cli.err.exit_code(e: bytes) -> i32`
* `std.cli.err.stream(e: bytes) -> i32`
* `std.cli.err.render(e: bytes) -> bytes_view`
* `std.cli.err.token(e: bytes) -> bytes_view`
* `std.cli.err.argv_index(e: bytes) -> i32` (return -1 if none)

> With these getters, agents never need to “understand the blob format” — only IDs.

---

## 7) Determinism & safety checks (implementation requirements)

Inside the parser implementation (the “std.cli adapter” / builtins), enforce:

* `total_len_bytes == actual_blob_len` (hard fail → return `SPEC_INVALID` / `INTERNAL_PARSER_ERROR`)
* all length-prefixed fields must be in bounds
* `stream` must be either 1 or 2
* `render_utf8` must be valid UTF‑8 (if not, replace with a deterministic fallback message and code `INTERNAL_PARSER_ERROR`)
* `matches_v1`:

  * `flags_count/opts_count` must be consistent with buffer length
  * lists must already be sorted; if not, sort deterministically before encoding
* ensure stable whitespace of `render_utf8` (no env-dependent paths; no terminal width wrapping; no locale)

---

## 8) Tiny guidance to unblock agent reliability immediately

If agents still struggle, don’t expand the guide; instead:

* Make the **repair loop** always call `std.cli.err.render` and treat that as the canonical message.
* Add a “printer helper”:

  * `std.cli.print_and_exit_v1(out: bytes) -> i32` (run‑os only)
  * internally: if matches → returns 0; if err → prints + exits.
    This reduces boilerplate and mistakes.

---

[1]: https://docs.rs/clap/latest/clap/type.Error.html?utm_source=chatgpt.com "Error in clap - Rust"
[2]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap12.html?utm_source=chatgpt.com "12. Utility Conventions"
[3]: https://www.gnu.org/software/libc/manual/html_node/Argument-Syntax.html?utm_source=chatgpt.com "Argument Syntax (The GNU C Library)"
[4]: https://man7.org/linux/man-pages/man3/sysexits.h.3head.html?utm_source=chatgpt.com "sysexits.h(3head) - Linux manual page"
