# Archive security corpus (M4)

This folder contains **small, deterministic adversarial fixtures** for safe archive processing.

Fixtures live under `tests/corpora/archive/fixtures/`:

- `zip_slip_evil.zip`: ZIP with an entry name containing `..` (`../evil.txt`).
- `malformed_cd_truncated.zip`: ZIP truncated in the end-of-central-directory area.
- `tar_dir_then_hello.tar`: TAR containing a directory entry (`dir/`) plus `dir/hello.txt`.
- `tar_slip_evil.tar`: TAR with an entry name containing `..` (`../evil.txt`).
- `tgz_ratio_limit.tgz`: TGZ that exceeds the configured inflate ratio cap.

The corpus test suite is `tests/corpora/archive/tests.json` (world: `solve-fs`).
