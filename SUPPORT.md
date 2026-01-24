# Support

This project is designed to be used by both humans and LLM-powered coding agents.
To keep triage fast and actionable, we separate **questions** from **bugs**.

## Where to ask questions

If you are unsure how to do something (usage questions, “how do I…”, design discussion, troubleshooting help):

- **GitHub Discussions (recommended):** https://github.com/x07lang/x07/discussions

When asking, include:

- the exact command(s) you ran,
- the full machine-readable JSON report(s) from X07 tools (save stdout to a file),
- your OS / arch,
- toolchain versions (`x07 --version`, and `x07up --version` if you used the installer).

## Where to report bugs

**GitHub Issues are for actionable bugs and tracked work items** (bugs, confirmed regressions, and accepted feature requests).

- **Toolchain / stdlib / skills / examples bugs:** open an issue in **this** repo using the Bug Report form.
- **Docs site rendering / deployment bugs:** https://github.com/x07lang/x07-website/issues
- **Registry API bugs:** https://github.com/x07lang/x07-registry/issues
- **Registry UI (x07.io) bugs:** https://github.com/x07lang/x07-registry-web/issues

Before filing:

1) Update to the latest stable toolchain (or confirm the bug reproduces on the latest).
2) Capture structured reports (stdout) as artifacts.
3) Reduce to a minimal repro (smallest `.x07.json` + smallest inputs).

## Security issues

Please **do not** open a public issue.

See [SECURITY.md](SECURITY.md).

## Feature requests

If the change is large or affects core language/toolchain contracts, it may require an RFC.
Start with the Feature Request issue form, and follow the RFC guidance in `governance/RFC-REQUIREMENTS.md` when applicable.
