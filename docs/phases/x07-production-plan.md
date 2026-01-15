Status: in progress

Last updated: 2026-01-15

## Progress

- [x] Create repo folders under `x07lang/`.
- [x] Copy the core toolchain repo into `x07lang/x07`.
- [x] Scaffold `x07lang/x07-rfcs` (process + template + folders).
- [x] Track all `Cargo.lock` files in `x07lang/x07`.
- [x] Add governance, licensing, and policy docs (contacts TBD).
- [x] Add CI baseline + policy workflows in `x07lang/x07`.
- [x] Scaffold `x07lang/x07-registry` and `x07lang/x07-index`.
- [x] Add RFC repo CI workflow (`markdownlint` + `linkcheck`).
- [x] Run verification gates in the new repos.
- [x] Implement `x07 pkg` + registry integration (see `docs/phases/x07-package-manager.md`).
- [x] Implement end-user skills pack + validator (see `docs/phases/x07-skills-impl.md`).
- [x] Implement CI orchestrator (`scripts/ci/run.sh` + `scripts/ci/run.py`) described later in this plan.
- [x] Implement cross-repo propagation automation (Step 7.2 `OPS-01`).
- [x] Implement registry deployment + governance runbooks (Step 6 `REG-01..REG-05`).

---

## Step 0 — Decide the repo topology (recommended)

You *can* do a monorepo, but for adoption + operational clarity, I recommend **“core repo + a few satellite repos”**:

### Public repos

1. **`x07lang/x07`** (core monorepo)

   * toolchain (`x07c`, linter/formatter, `x07 pkg`, test harness)
   * `stdlib` source (x07AST JSON modules), `stdlib.lock`
   * specs/schemas (`spec/*.schema.json`), docs, skills (or link to skills repo)

2. **`x07lang/x07-rfcs`**

   * formal RFC process, templates, accepted RFCs (design changes)
   * “direction control” lives here (like Rust’s RFC repo) ([GitHub][3])

3. **`x07lang/x07-registry`**

   * registry server implementation + OpenAPI contract
   * auth/token logic, publish moderation policies

4. **`x07lang/x07-index`** (sparse index **data** repo)

   * the generated index files served over HTTP (static)
   * kept separate so it can be mirrored/CDN’d and rolled back easily
     (This mirrors Cargo’s separation of “index” from “API server” concepts.) ([Rust Documentation][4])

5. **`x07lang/x07-website`**

   * human + agent docs website
   * includes machine-readable “agent pages” (schemas, contracts, skill docs)

### Private repos

6. **`x07lang/x07-infra-private`**

   * Terraform/Helm, production secrets (never public)
   * deployment runbooks, incident playbooks

This split makes “what is upstream” vs “what is operational” extremely clear, while keeping the entire language/toolchain open.

---

## Step 1 — Pick the license (adoption-friendly + contribution-safe)

### Recommended licensing model

Use **dual‑license “MIT OR Apache‑2.0”** for the toolchain + stdlib (Rust’s model). ([Rust][5])

Why this model works well:

* The Apache‑2.0 side includes an **explicit patent license grant** (important for language/toolchain adoption). ([Apache Software Foundation][6])
* The MIT side preserves broad compatibility (including ecosystems where Apache‑2.0 alone can be awkward). ([Rust][5])

### Concrete repo changes (PR: `LEGAL-01`)

In `x07lang/x07`:

* `LICENSE-APACHE` (Apache 2.0 text)
* `LICENSE-MIT`
* `COPYRIGHT` or `NOTICE` (if needed)
* `README.md` licensing blurb: “Licensed under either Apache 2.0 or MIT, at your option”
* add `SPDX-License-Identifier: (Apache-2.0 OR MIT)` headers where appropriate

---

## Step 2 — Set up contribution governance (direction control without closing source)

### 2.1 Create an RFC process (PR: `GOV-01`)

Add a **separate RFC repo** (`x07lang/x07-rfcs`) and adopt a simple, explicit pipeline:

* `0000-template.md`
* `process.md` describing:

  * what needs an RFC (language changes, ABI, package format, stdlib baselines, registry protocol)
  * what doesn’t (bugfixes, docs, refactors)
  * decision rules (who approves/merges, quorum)
  * stabilization rules (“feature gates” and release trains)

Rust’s RFC process repo is a good reference for “controlled path for changes” as a scaling mechanism. ([GitHub][3])

### 2.2 Define project roles and merge authority (PR: `GOV-02`)

In `x07lang/x07` add `governance/`:

* `governance/TEAMS.md`

  * Core team (final say)
  * Toolchain team
  * Stdlib team
  * Packages/registry team
  * Security response team
* `governance/DECISION-MAKING.md`

  * which decisions require RFC approval
  * how ties resolve
* `governance/MAINTAINERS.md` (list GitHub handles; who has merge rights)

This is how you “guide direction” without restricting forks via license.

---

## Step 3 — Add community/legal safety rails (low friction)

### 3.1 Code of Conduct (PR: `COMM-01`)

Adopt **Contributor Covenant 2.1** as `CODE_OF_CONDUCT.md`. ([contributor-covenant.org][7])
Add `REPORTING.md` or a section with an email + escalation steps.

### 3.2 DCO sign-off (instead of a CLA) (PR: `COMM-02`)

For minimal friction, use **DCO**:

* `CONTRIBUTING.md` requires `Signed-off-by` on commits
* CI enforces sign-off (GitHub DCO app or a script)

DCO is widely used as a lightweight alternative to a CLA. ([Linux Foundation Wiki][8])

### 3.3 Security policy (PR: `SEC-01`)

Add `SECURITY.md`:

* vulnerability reporting email
* disclosure timeline
* supported versions window (“we patch the last N releases”)

---

## Step 4 — Trademark strategy (the “official X07” lever)

This is *the* key to Option A: you don’t prevent forks; you prevent confusion.

### 4.1 Register trademarks (operational step)

* Register **word mark** “X07” (and optional logo) under an entity you control.
* Own the domain(s): `x07lang.org` etc.

### 4.2 Publish a trademark policy (PR: `LEGAL-02`)

Add `TRADEMARKS.md` (and optionally `branding/`):

A good policy is very similar to Rust’s: the baseline rule is “don’t use the mark in a way that appears official/endorsed without permission.” ([The Rust Foundation][2])

Your policy should explicitly cover:

* forks / modified toolchains:

  * allowed, but must be clearly named differently (e.g., “FoobarLang (X07-derived)”)
* distributions:

  * allow packaging for OS distros with clear labeling
* events/books/courses:

  * permitted uses and what needs permission
* “official builds”:

  * only artifacts signed by your release keys can call themselves “X07 Official”

This is how you keep direction + trust without closing the code.

---

## Step 5 — Release process for “everything” (toolchain + stdlib + schemas + docs)

### 5.1 Define the canonical release artifact (PR: `REL-01`)

In `x07lang/x07/docs/releases.md` define that each release consists of:

* `x07c` binaries (per platform)
* `x07` CLI (includes `pkg`, `test`, etc.)
* **pinned**:

  * `stdlib.lock`
  * schema versions (`spec/*.schema.json`)
* checksums + signature

### 5.2 Automation: GitHub Actions release train (PR: `REL-02`)

On tag `vX.Y.Z`:

* build toolchain for supported OS/arch
* run the full CI matrix
* attach artifacts to GitHub Releases
* publish `stdlib.lock` + schema bundle as release assets
* (optional) open PR to `x07lang/x07-website` updating docs version selector

### 5.3 Versioning policy (PR: `REL-03`)

* Toolchain uses SemVer.
* Stdlib packages are SemVer and pinned in `stdlib.lock`.
* Schema files are SemVer (you already do this).

---

## Step 6 — Package repository governance + deployment (Cargo-like)

You already designed a Cargo-like sparse index flow; for Option A the difference is **operational governance**:

### 6.1 Public index + API, private secrets (PRs: `REG-01..REG-05`)

* `x07lang/x07-index` is public static content.
* `x07lang/x07-registry` is open-source server.
* production deployment secrets remain private in `infra-private`.

Cargo’s registry index uses `config.json` with `api` and an `auth-required` flag for token requirements; this is a proven design. ([Rust Documentation][4])

### 6.2 Trust model for packages

* Require auth tokens for publish (Cargo-like).
* Add moderation hooks (spam/malware).
* Add signing later (optional).

---

## Step 7 — Ensure changes propagate automatically across “all places”

This is mostly “Git + CI choreography”:

### 7.1 Single source of truth

* **Only one repo** owns canonical versions:

  * toolchain, stdlib, specs live in `x07lang/x07`
* satellite repos consume versions from releases.

### 7.2 Propagation automation (PR: `OPS-01`)

Add a release workflow that:

* pushes index updates to `x07lang/x07-index`
* triggers `x07lang/x07-website` rebuild
* tags `x07lang/x07-registry` compatibility (or opens a PR bumping API client versions)

### 7.3 Compatibility gates

In `x07lang/x07` CI:

* run `x07 pkg pack` determinism checks
* run `x07 pkg lock` against a local test registry
* run `x07 test` harness against canary programs

---

## Step 8 — What should be public vs private (under Option A)

**Public:**

* toolchain source (compiler, linter, formatter, pkg manager)
* stdlib source + `stdlib.lock`
* specs/schemas, docs, skills
* registry server implementation
* index format and tooling

**Private:**

* release signing keys
* production registry tokens, database credentials
* infra secrets / internal ops dashboards

If you want additional “direction control,” keep **merge rights and release keys** tightly managed—this is normal and doesn’t contradict open source.

---

## Step 9 — The minimum “direction control” kit to ship immediately

If you want the smallest set of PRs that gets you 80% of Option A:

1. `LEGAL-01`: Dual license MIT/Apache-2.0 (like Rust). ([Rust][5])
2. `GOV-01`: RFC repo + process docs (Rust-style). ([GitHub][3])
3. `COMM-01`: Contributor Covenant 2.1. ([contributor-covenant.org][7])
4. `COMM-02`: DCO sign-off + CI enforcement. ([Linux Foundation Wiki][8])
5. `LEGAL-02`: Trademark policy + brand usage rules (Rust-style baseline). ([The Rust Foundation][2])
6. `REL-01/02`: Release artifact definition + automated release pipeline.

Once these land, you have: open code + controlled upstream + protected official brand.

---

## A few “don’t miss” questions (worth deciding now)

* Who **owns the trademark** (you personally, an LLC, a foundation)?
  If you ever want a foundation later, you can transfer ownership—design policy files assuming that future.
* Do you want a **strict stability policy** (ABI v1 frozen, breaking changes require ABI v2 path)?
* What is your **security disclosure** posture (private reports, patch window)?
* Do you want **signed releases** from day 1 (recommended if you expect widespread installs)?

---

[1]: https://opensource.org/osd?utm_source=chatgpt.com "The Open Source Definition"
[2]: https://rustfoundation.org/policy/rust-trademark-policy/?utm_source=chatgpt.com "Rust Language Trademark Policy"
[3]: https://github.com/rust-lang/rfcs?utm_source=chatgpt.com "rust-lang/rfcs: RFCs for changes to Rust"
[4]: https://doc.rust-lang.org/cargo/reference/registry-index.html?utm_source=chatgpt.com "Registry Index - The Cargo Book"
[5]: https://rust-lang.org/policies/licenses/?utm_source=chatgpt.com "Licenses"
[6]: https://www.apache.org/licenses/LICENSE-2.0?utm_source=chatgpt.com "Apache License, Version 2.0"
[7]: https://www.contributor-covenant.org/version/2/1/code_of_conduct/?utm_source=chatgpt.com "Contributor Covenant Code of Conduct version 2.1"
[8]: https://wiki.linuxfoundation.org/dco?utm_source=chatgpt.com "Developer Certificate of Origin (DCO) - Wiki"
+++++
 For now, I’m giving a **copy/paste‑ready** checklist with **explicit file paths + proposed CI workflow/job names** that you can adopt as-is.

Below assumes the **recommended repo split**:

* `x07lang/x07` (core toolchain + stdlib + specs + skills)
* `x07lang/x07-rfcs` (governance)
* `x07lang/x07-registry` (API server)
* `x07lang/x07-index` (sparse index data)
* `x07lang/x07-website` (docs site)
* `x07lang/x07-infra-private` (private deploy)

This is Option A: **open-source toolchain + direction via governance + trademark** (Rust-style patterns are proven here: dual license, RFCs, Cargo sparse registries, trademark policy). ([Rust][1])

---

# Naming conventions used in this checklist

**CI workflow files**

* `.github/workflows/ci.yml`
* `.github/workflows/release.yml`
* `.github/workflows/policy.yml` (optional)
* `.github/workflows/deploy.yml` (registry/website)

**Required status checks (job names)**

* `ci / rustfmt`
* `ci / clippy`
* `ci / test`
* `ci / contracts`
* `ci / e2e-smoke`
* `ci / determinism`
* `release / build-matrix`
* `release / provenance`
* `release / sign`
* `release / publish`

(You can rename later—start strict, then relax.)

---

# GOV track — governance, licensing, trademark, contribution controls

## GOV‑01 — Create the RFC repo and process (direction control)

**Repo:** `x07lang/x07-rfcs`

**Adds**

* `README.md` (what RFCs are + how to propose)
* `process.md` (normative process)
* `0000-template.md` (RFC template)
* `rfcs/` folder structure:

  * `rfcs/accepted/`
  * `rfcs/active/`
  * `rfcs/drafts/`

**Why (normative reference):**

* Rust RFCs are explicitly “a consistent and controlled path for changes.” ([GitHub][2])

**CI**

* `.github/workflows/ci.yml`

  * job: `ci / markdownlint` → runs `markdownlint "**/*.md"`
  * job: `ci / linkcheck` → runs `lychee` (or similar) on markdown links

**Branch protections**

* Require: `ci / markdownlint`, `ci / linkcheck`
* Require 1 maintainer review for merges to `main`

---

## GOV‑02 — Add governance structure in core repo

**Repo:** `x07lang/x07`

**Adds**

* `governance/TEAMS.md`
* `governance/MAINTAINERS.md`
* `governance/DECISION-MAKING.md`
* `governance/RFC-REQUIREMENTS.md`

  * explicit list: what changes require RFC (ABI, package formats, stdlib baselines, registry protocols, language syntax/contracts)

**CI**

* `.github/workflows/policy.yml`

  * job: `policy / governance-files-present`

    * command: `python3 scripts/check_governance_files.py` (you create this) to ensure required docs exist.

**Branch protections**

* Require: `policy / governance-files-present`

---

## GOV‑03 — Code of Conduct + contribution policy + DCO enforcement

**Repo:** `x07lang/x07`

**Adds**

* `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1) ([Contributor Covenant][3])
* `CONTRIBUTING.md`

  * contribution flow
  * “when to open RFC” link to `x07lang/x07-rfcs`
  * requires DCO sign-off (`Signed-off-by`) ([probot.github.io][4])
* `.github/pull_request_template.md`
* `.github/ISSUE_TEMPLATE/bug_report.yml`
* `.github/ISSUE_TEMPLATE/feature_request.yml`

**CI**

* `.github/workflows/ci.yml` (or separate `.github/workflows/dco.yml`)

  * job: `ci / dco`

    * use a DCO GitHub App (Probot DCO) or equivalent gate ([probot.github.io][4])

**Branch protections**

* Require: `ci / dco`

---

## GOV‑04 — Security policy (responsible disclosure)

**Repo:** `x07lang/x07`

**Adds**

* `SECURITY.md`

  * reporting email
  * supported versions policy (e.g., “latest 2 releases”)
  * disclosure timeline

**CI**

* none required

**Branch protections**

* none required

---

## GOV‑05 — Open-source licensing (dual MIT OR Apache‑2.0)

**Repo:** `x07lang/x07`

**Adds**

* `LICENSE-MIT`
* `LICENSE-APACHE`
* `README.md` section:

  * “Licensed under MIT OR Apache‑2.0”
* Optional:

  * `NOTICE` (only if you want it; Apache 2.0 allows NOTICE handling)
* Optional enforcement:

  * `scripts/check_spdx_headers.py` (if you want SPDX headers in every Rust file)

**Rationale references**

* Rust’s official projects are generally dual licensed MIT/Apache. ([Rust][1])
* Apache 2.0 includes an explicit patent grant. ([Apache Software Foundation][5])

**CI**

* `.github/workflows/ci.yml`

  * job: `ci / licenses`

    * command: `cargo deny check licenses` (or `reuse lint` if you prefer)

**Branch protections**

* Require: `ci / licenses`

---

## GOV‑06 — Trademark policy + “official build” definition

**Repo:** `x07lang/x07`

**Adds**

* `TRADEMARKS.md` (normative)

  * explicit: forks allowed, but no implication of endorsement; “official” reserved
* `branding/` (logo files if/when you have them)
* `docs/official-builds.md`

  * defines “official artifacts = signed by X07 release keys”
* `docs/forking.md`

  * “How to fork responsibly and rename”

**Reference patterns**

* Rust trademark policy + core rule “must not appear official / endorsed”. ([The Rust Foundation][6])

**CI**

* `.github/workflows/policy.yml`

  * job: `policy / trademark-policy-present`

    * command: `test -f TRADEMARKS.md`

**Branch protections**

* Require: `policy / trademark-policy-present`

---

## GOV‑07 — Repository topology and propagation rules

**Repo:** `x07lang/x07`

**Adds**

* `docs/repo-topology.md`

  * defines the roles of `x07`, `rfcs`, `registry`, `index`, `website`, `infra-private`
* `docs/change-propagation.md`

  * what triggers what:

    * toolchain release → index update → website update

**CI**

* none required

---

# REL track — release process, provenance, distribution automation

## REL‑01 — Define release artifacts + versioning (normative)

**Repo:** `x07lang/x07`

**Adds**

* `docs/releases.md`

  * what “a release” contains:

    * toolchain binaries
    * stdlib bundle + `stdlib.lock`
    * schema bundle (`spec/*.schema.json`)
* `docs/versioning.md`

  * SemVer rules:

    * toolchain
    * stdlib packages
    * schemas
* `docs/stability.md`

  * “ABI v1 frozen” / “breaking change requires ABI v2”

**CI**

* `.github/workflows/policy.yml`

  * job: `policy / release-docs-present`

---

## REL‑02 — Core CI baseline (required checks + smoke)

**Repo:** `x07lang/x07`

**Adds/changes**

* `.github/workflows/ci.yml` with jobs:

  * `ci / rustfmt` → `cargo fmt --check`
  * `ci / clippy` → `cargo clippy --all-targets --all-features -- -D warnings`
  * `ci / test` → `cargo test --workspace`
  * `ci / contracts` → `python3 scripts/check_contracts.py`
  * `ci / e2e-smoke` → run a minimal `x07 build` + `x07 test` on fixture programs
  * `ci / determinism` → run the same smoke 3× and diff the JSON reports

**Branch protections**

* Require: all `ci / …` jobs above

---

## REL‑03 — Reproducible packaging manifest + checksums (release inputs pinned)

**Repo:** `x07lang/x07`

**Adds**

* `dist/release-manifest.schema.json`
* `scripts/build_release_manifest.py`

  * emits `dist/release-manifest.json` with:

    * toolchain git sha
    * stdlib.lock hash
    * schema versions
    * build flags used
* `scripts/sha256sum_dir.py` (if needed)

**CI**

* `.github/workflows/ci.yml`

  * job: `ci / release-manifest`

    * command: `python3 scripts/build_release_manifest.py --check`

---

## REL‑04 — Automated release workflow (GitHub Releases)

**Repo:** `x07lang/x07`

**Adds**

* `.github/workflows/release.yml` triggered on tags `v*`

  * job: `release / build-matrix`

    * builds `x07` for linux/macos/windows (and archs you support)
  * job: `release / publish`

    * uploads artifacts + `dist/release-manifest.json`

**CI**

* release workflow itself

**Branch protections**

* none (tag-triggered)

---

## REL‑05 — Supply-chain integrity: SLSA provenance + cosign signing

**Repo:** `x07lang/x07`

**Adds**

* `.github/workflows/release.yml` extra jobs:

  * `release / provenance`

    * uses `slsa-framework/slsa-github-generator` (or equivalent) ([GitHub][7])
  * `release / sign`

    * uses `sigstore/cosign` (keyless OIDC) ([GitHub][8])
* `docs/provenance.md`

  * how users verify artifacts

**Why**

* SLSA defines provenance levels and the concept of build provenance. ([SLSA][9])

---

## REL‑06 — Auto-propagate releases to `index` + `website` (cross-repo bot)

**Repo:** `x07lang/x07`

**Adds**

* `.github/workflows/release.yml` steps after publish:

  * open PR to `x07lang/x07-index` updating `config.json` or publishing new index lines
  * open PR to `x07lang/x07-website` bumping docs version selector + linking release notes
* `scripts/open_pr_index_update.py`
* `scripts/open_pr_website_update.py`

**CI**

* none required beyond release workflow

---

# REG track — package registry + sparse index + client integration

Cargo’s sparse index model is proven and documented; it includes an index `config.json` with `api` and `auth-required` keys, and supports sparse protocol when URL starts with `sparse+`. ([Rust Documentation][10])

## REG‑01 — Registry server repo skeleton + OpenAPI contract

**Repo:** `x07lang/x07-registry`

**Adds**

* `openapi/openapi.json` (your contract; versioned)
* `README.md` (how to run locally)
* `src/` server skeleton:

  * `/healthz`
  * `/v1/auth/token` (token issuance or token validation path)
  * `/v1/packages/publish` (accept upload)
  * `/v1/packages/{name}/{version}/metadata` (if needed)
* `docs/auth.md`

**CI**

* `.github/workflows/ci.yml`

  * job: `ci / test` (server tests)
  * job: `ci / openapi-validate` (validate openapi JSON)

---

## REG‑02 — Sparse index data repo skeleton + canonical validation

**Repo:** `x07lang/x07-index`

**Adds**

* `index/config.json`

  * includes `dl`, `api`, and `auth-required` support semantics ([Rust Documentation][11])
* `index/` tree root (canonical layout)
* `scripts/validate_index.py`

  * validates:

    * pathing rules
    * JSON line schema
    * stable ordering (sorted)
* `spec/index-entry.schema.json` (single source of truth)

**CI**

* `.github/workflows/ci.yml`

  * job: `ci / validate-index`

    * command: `python3 scripts/validate_index.py --check`

---

## REG‑03 — Core repo: `x07 pkg lock` sparse fetch + resolution (production)

**Repo:** `x07lang/x07`

**Adds/changes**

* `crates/x07-pkg/…` (client)
* `spec/x07.lock.schema.json` already exists in your plan
* `scripts/check_pkg_contracts.py` (if not already)
* `x07 pkg lock`:

  * fetches `sparse+https://…/{prefix}/{name}` entries
  * resolves versions deterministically
  * writes canonical lock JSON

**CI**

* `.github/workflows/ci.yml`

  * job: `ci / pkg-contracts`

    * runs:

      * `x07 pkg lock --check`
      * schema validation

---

## REG‑04 — Auth tokens end-to-end (Cargo-like)

**Repo:** `x07lang/x07-registry` + `x07lang/x07`

**Adds**

* Server:

  * token verification middleware
  * permissions model: owners/maintainers
* Client:

  * `x07 pkg login` (stores token)
  * uses Authorization header for API calls
* Index:

  * `index/config.json` supports `auth-required` semantics (private registry behavior) ([Rust Documentation][11])

**CI**

* `x07lang/x07-registry`:

  * job: `ci / auth-integration-test`
* `x07lang/x07`:

  * job: `ci / registry-integration-test`

    * spins up registry container locally and runs publish/lock

---

## REG‑05 — Publish pipeline: pack → publish → index update PR

**Repo:** `x07lang/x07` + `x07lang/x07-registry` + `x07lang/x07-index`

**Adds**

* Core:

  * `x07 pkg pack` emits deterministic tar (sorted entries, canonical mtime)
  * `x07 pkg publish` uploads pack
* Registry:

  * validates archive deterministically
  * writes package blobs to storage
  * generates index entry line
* Index:

  * accepts PR updates (bot) OR registry commits directly (prefer PR for audit trail)

**CI**

* Core:

  * `ci / pack-determinism`

    * run pack twice and byte-compare archives
* Registry:

  * `ci / publish-contracts`
* Index:

  * `ci / validate-index`

---

## REG‑06 — Moderation primitives: yank/un-yank + audit log

**Repo:** `x07lang/x07-registry` (+ `x07lang/x07-index` format extension if needed)

**Adds**

* `POST /v1/packages/{name}/{version}/yank`
* `POST /v1/packages/{name}/{version}/unyank`
* append-only audit log (server-side)
* index entry includes yanked flag (if you choose)

**CI**

* `ci / moderation-tests`

---

# Website/docs repo PRs (optional but recommended)

(These don’t need to be in GOV/REL/REG naming, but they’re part of the ecosystem.)

## REL‑07 — Docs site with “agent pages” and versioned contracts

**Repo:** `x07lang/x07-website`

**Adds**

* `docs/` (human)
* `agent/` (machine-first):

  * schemas
  * CLI contract docs
  * skills specs
* `versions.json` (maps toolchain versions to stdlib/schema bundles)

**CI**

* `.github/workflows/ci.yml`

  * `ci / build-site`
  * `ci / linkcheck`

---

# “Default required checks” summary (what you set in GitHub)

## x07lang/x07 required checks on `main`

* `ci / rustfmt`
* `ci / clippy`
* `ci / test`
* `ci / contracts`
* `ci / e2e-smoke`
* `ci / determinism`
* `ci / licenses`
* `ci / pkg-contracts`
* `policy / trademark-policy-present` (optional but good)
* `ci / dco`

## x07lang/x07-registry required checks on `main`

* `ci / test`
* `ci / openapi-validate`
* `ci / auth-integration-test` (after REG‑04)

## x07lang/x07-index required checks on `main`

* `ci / validate-index`

## x07lang/x07-rfcs required checks on `main`

* `ci / markdownlint`
* `ci / linkcheck`

---

# Why this setup gives you “control” while staying open

* License encourages adoption (Rust-standard dual licensing). ([Rust][1])
* Governance/RFC process gives upstream control (Rust-style). ([GitHub][2])
* Trademark policy protects “official X07” naming and reputation (Rust-style). ([The Rust Foundation][6])
* Registry/index follow Cargo’s proven sparse model (stability + scalability). ([Rust Documentation][10])
* Provenance/signing harden supply chain (SLSA + cosign). ([GitHub][7])

---

[1]: https://rust-lang.org/policies/licenses/?utm_source=chatgpt.com "Licenses"
[2]: https://github.com/rust-lang/rfcs?utm_source=chatgpt.com "rust-lang/rfcs: RFCs for changes to Rust"
[3]: https://www.contributor-covenant.org/version/2/1/code_of_conduct/?utm_source=chatgpt.com "Contributor Covenant Code of Conduct version 2.1"
[4]: https://probot.github.io/apps/dco/?utm_source=chatgpt.com "Developer Certificate of Origin (DCO)"
[5]: https://www.apache.org/licenses/LICENSE-2.0?utm_source=chatgpt.com "Apache License, Version 2.0"
[6]: https://rustfoundation.org/policy/rust-trademark-policy/?utm_source=chatgpt.com "Rust Language Trademark Policy"
[7]: https://github.com/slsa-framework/slsa-github-generator?utm_source=chatgpt.com "Language-agnostic SLSA provenance generation for ..."
[8]: https://github.com/sigstore/cosign?utm_source=chatgpt.com "sigstore/cosign: Code signing and transparency ..."
[9]: https://slsa.dev/spec/v0.1/levels?utm_source=chatgpt.com "SLSA • Security levels"
[10]: https://doc.rust-lang.org/cargo/reference/registries.html?utm_source=chatgpt.com "Registries - The Cargo Book"
[11]: https://doc.rust-lang.org/cargo/reference/registry-index.html?utm_source=chatgpt.com "Registry Index - The Cargo Book"

+++++

Got it — you can (and should) keep running CI manually for now. The key is to **organize CI/CD as a stable, deterministic “script contract”** inside the repo, so *any* future CI provider (GitHub Actions, Buildkite, GitLab, Jenkins, etc.) can later call the exact same entrypoints without you rewriting anything.

---

## Goal: “CI is a product interface”, not a provider choice

Treat CI as:

* a **set of named jobs** (fmt/lint/test/contracts/e2e/determinism/pack)
* each job has **stable exit codes** + **machine-readable JSON output**
* jobs can be run:

  * locally (manual)
  * by any CI system later (just “call the same scripts”)

This also directly supports your long-term goal: **100% agentic coding**. Agents can run `scripts/ci/run.py --json`, read diagnostics, repair, repeat.

---

## Step-by-step repo re-org (works with manual runs today)

### Step 1 — Create a single CI entrypoint and job folder

Add:

```
scripts/ci/
  run.sh                 # orchestrator (human-friendly)
  run.py                 # orchestrator (JSON report + strict mode)
  jobs/
    00_env.sh
    10_fmt.sh
    20_lint.sh
    30_unit.sh
    40_contracts.sh
    50_e2e_smoke.sh
    60_determinism.sh
    70_pkg_pack.sh
    80_sanitizers.sh     # optional, “nightly/release”
    90_fuzz.sh           # optional, “nightly”
  fixtures/
    ... small fixtures used by e2e_smoke + determinism
  schemas/
    ci-report.schema.json
```

**Why both `run.sh` and `run.py`**

* `run.sh`: easiest for humans (`./scripts/ci/run.sh pr`)
* `run.py`: best for agents (strict JSON, stable keys, no surprises)

### Step 2 — Define “profiles” (manual CI levels)

Pick these profiles and keep them stable:

* `dev`: fast; meant to be run constantly
* `pr`: what you consider “mergeable”
* `nightly`: slow/expensive; finds regressions early
* `release`: everything required before you publish binaries

Example mapping:

| Profile | Jobs                                              |
| ------- | ------------------------------------------------- |
| dev     | env + fmt + lint + unit                           |
| pr      | dev + contracts + e2e_smoke + pkg_pack            |
| nightly | pr + determinism + sanitizers + fuzz              |
| release | nightly + “release manifest” + signing/provenance |

### Step 3 — Standardize job exit codes + strict JSON output

Every job should follow:

* `0`: pass
* `1`: failure (assertion/validation failed)
* `2`: infra error (missing tool, missing dependency, timeout)
* `3`: non-determinism detected
* `4`: contract violation (invalid JSON output / invalid schema / bad patch)

In addition, every job writes a JSON fragment to an artifacts directory:

```
artifacts/ci/<run_id>/jobs/<job_name>.json
artifacts/ci/<run_id>/logs/<job_name>.log
artifacts/ci/<run_id>/summary.json   # final merged report
```

This “artifacts always exist” rule matters for debugging *and* for agent loops.

### Step 4 — Pin the build environment (reproducibility first)

Even without GitHub Actions, pin versions so “manual CI” is meaningful:

* `rust-toolchain.toml` (pin Rust)
* `Cargo.lock` (already)
* `requirements.txt` or `uv.lock` for Python tooling scripts
* `scripts/ci/jobs/00_env.sh` should:

  * print versions (`rustc --version`, `cargo --version`, `python3 --version`, `clang --version`, etc.)
  * fail with exit `2` if required tools are missing
  * write them into `jobs/env.json`

Optional but strongly recommended:

* `docker/ci.Dockerfile` (one canonical CI image)
* `scripts/ci/run.sh --container` runs inside it

This gives you deterministic CI even before you choose a hosted provider.

### Step 5 — Add a CI report schema (so “CI output never drifts”)

Add `scripts/ci/schemas/ci-report.schema.json` and enforce:

* required top-level keys:

  * `run_id`, `profile`, `git_sha`, `timestamp`
  * `jobs[]` with `name`, `status`, `exit_code`, `duration_ms`
  * `artifacts[]` list of produced files
* forbid additionalProperties in “strict mode” (agents love this)

Then `scripts/ci/run.py --strict` validates the final report against the schema.

---

## What “CI jobs” you should define for X07 specifically

Below are the job names I’d standardize on (even if you don’t automate them yet).

### `10_fmt`: formatting

Runs:

* `cargo fmt --check`
* `x07c fmt --check` for `.x07.json`

### `20_lint`: lint + static checks

Runs:

* `cargo clippy` (or your equivalent)
* `python3 scripts/check_contracts.py` (x07AST/x07Diag/etc)
* `python3 scripts/check_x07_parens.py` (structure)
* `python3 scripts/check_skills_outputs.py` (skill report schemas)
* `python3 scripts/check_pkg_contracts.py --check-archives` (deterministic tar entries)

### `30_unit`: unit tests

Runs:

* `cargo test --workspace`

### `40_contracts`: “schema contracts are law”

Runs:

* validate:

  * x07AST schema
  * x07Diag schema
  * x07test schema
  * workspace/package/lock schemas
* validate: sparse index JSON-line schema
* validate: canonical JSON rules (stable ordering, stable floats rules if any)

### `50_e2e_smoke`: “toolchain works end-to-end”

Runs a small set of programs that exercise:

* `x07 build`
* `x07 run`
* `x07 test`
* at least one module import + one stdlib module call
* one fixture-world task (fs) if supported

Outputs:

* program stdout/stderr captured
* runner JSON result captured

### `60_determinism`: “same input 3× => same output and metrics”

Runs the same e2e cases **3 times** and requires:

* byte-identical outputs
* identical `mem_stats` / `sched_stats` where applicable
* identical test report JSON

This is central to your project identity.

### `70_pkg_pack`: packaging determinism

Runs:

* `x07 pkg pack` twice
* compares archives byte-for-byte
* validates tar order, canonical mtimes, canonical json inside the archive

### `80_sanitizers` and `90_fuzz` (nightly/release only)

* sanitizers: ASan/UBSan builds for runtime/compiler hot paths
* fuzz: compiler parser/lowering fuzzer, x07AST validator fuzzer

---

## CD organization (even if you do it manually at first)

You can do CD with scripts now, and later wire them to automation.

Add:

```
scripts/release/
  build.sh                 # builds binaries
  manifest.py              # writes dist/release-manifest.json
  sign.sh                  # cosign sign-blob
  verify.sh                # cosign verify-blob
  publish.sh               # upload to “dist” + update website/index PRs later
dist/
  release-manifest.schema.json
```

If/when you adopt signing + provenance:

* SLSA provenance is a well-defined concept: “verifiable information about where/when/how the artifact was produced.”  ([SLSA][1])
* Sigstore Cosign provides `cosign verify …` flows for verification. ([Sigstore][2])

(You can keep these scripts “manual-run only” until you’re ready to automate.)

---

## How this transitions cleanly to CI providers later

When you’re ready to automate, you **do not redesign CI**.

You just map:

* “provider job” → `./scripts/ci/run.sh pr`
* “nightly schedule” → `./scripts/ci/run.sh nightly`
* “tag release” → `./scripts/ci/run.sh release && ./scripts/release/publish.sh`

This is exactly how GitHub Actions conceptualizes workflows: YAML file defines jobs/steps running commands. ([GitHub Docs][3])
But you don’t need to adopt Actions now — you’re just making your repo *ready*.

---

## Minimal “starter PR checklist” to get unstuck (no CI provider required)

1. **CI‑01** Add `scripts/ci/run.sh` + `scripts/ci/jobs/*` stubs + `docs/ci.md`
2. **CI‑02** Add `scripts/ci/run.py` + `scripts/ci/schemas/ci-report.schema.json`
3. **CI‑03** Implement `jobs/10_fmt.sh`, `20_lint.sh`, `30_unit.sh`
4. **CI‑04** Implement `jobs/40_contracts.sh` (schema validation)
5. **CI‑05** Implement `jobs/50_e2e_smoke.sh`
6. **CI‑06** Implement `jobs/60_determinism.sh`
7. **CI‑07** Implement `jobs/70_pkg_pack.sh` (pack determinism)
8. **CI‑08** Add `scripts/release/*` skeleton + `dist/release-manifest.schema.json`
9. **CI‑09** Add `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1) ([Contributor Covenant][4])
10. **CI‑10** Add `SECURITY.md` and `TRADEMARKS.md` (policy-only, no automation needed)

---

[1]: https://slsa.dev/spec/draft/build-provenance?utm_source=chatgpt.com "Build: Provenance - SLSA.dev"
[2]: https://docs.sigstore.dev/cosign/verifying/verify/?utm_source=chatgpt.com "Verifying Signatures - Cosign"
[3]: https://docs.github.com/actions/using-workflows/workflow-syntax-for-github-actions?utm_source=chatgpt.com "Workflow syntax for GitHub Actions"
[4]: https://www.contributor-covenant.org/version/2/1/code_of_conduct/?utm_source=chatgpt.com "Contributor Covenant Code of Conduct version 2.1"
