# Security Policy

Thank you for helping to keep X07 and its ecosystem secure.

## Reporting a vulnerability

Please **do not** report security vulnerabilities via public GitHub issues, pull requests, or public discussions.

Preferred (if enabled): **GitHub Private Vulnerability Reporting**

1. Open the repository's **Security** tab.
2. Click **Advisories**.
3. Click **Report a vulnerability**.

Fallback: email **security@x07lang.org**.

When reporting, please include:

- Which repository is affected (toolchain / registry / website / registry UI).
- The exact version(s) affected (toolchain version, git commit, or release tag).
- A minimal reproduction (steps, PoC, or exploitability notes).
- Expected vs actual behavior.
- Impact assessment (confidentiality / integrity / availability).

If you are not sure whether an issue is a security problem, report it privately anyway.

## Supported versions

We provide security fixes for:

- The **latest stable release** of the X07 toolchain.
- The **previous stable release** (N-1).

Older versions may receive fixes at maintainer discretion, but should be considered unsupported.

## Disclosure timeline

We aim to follow coordinated disclosure best practices:

- **Acknowledgement:** within 72 hours.
- **Triage:** within 7 days.
- **Fix and release:** typically within 30 days (may vary by complexity/coordination).

If active exploitation is suspected, we will prioritize a fix and may accelerate the timeline.

## Credits

We will credit reporters in advisories and release notes unless you request otherwise.
