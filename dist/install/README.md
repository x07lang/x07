# X07 installer (bootstrap)

This directory contains the bootstrap installers that:

1. Download and install `x07up` into `$ROOT/bin`
2. Run `x07up install` for a selected channel or pinned toolchain
3. (Default profile) Install the X07 agent kit (skills) into the user environment

These scripts are designed to be non-interactive and agent-friendly (`--yes`, `--quiet`, `--json`).

