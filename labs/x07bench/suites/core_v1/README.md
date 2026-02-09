# core_v1

Seed benchmark suite for `x07 bench`.

Run locally:

```sh
x07 bench list --suite labs/x07bench/suites/core_v1/suite.json --format text
x07 bench validate --suite labs/x07bench/suites/core_v1/suite.json --artifact-dir target/x07bench --format text
x07 bench eval --suite labs/x07bench/suites/core_v1/suite.json --predictions labs/x07bench/suites/core_v1/predictions.oracle.jsonl --artifact-dir target/x07bench --format text
```

Baselines (committed):

- `baselines/oracle.report.json`
- `baselines/oracle.score.json`

Regenerate baselines:

```sh
x07 bench eval --suite labs/x07bench/suites/core_v1/suite.json \
  --predictions labs/x07bench/suites/core_v1/predictions.oracle.jsonl \
  --artifact-dir target/x07bench --format json --out labs/x07bench/suites/core_v1/baselines/oracle.report.json
python3 labs/x07bench/scripts/score_report.py \
  --in labs/x07bench/suites/core_v1/baselines/oracle.report.json \
  > labs/x07bench/suites/core_v1/baselines/oracle.score.json
```

Each instance includes:

- `issue.md`
- `repo/` broken snapshot
- `oracle.patchset.json` expected fix
