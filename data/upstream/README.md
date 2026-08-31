# Upstream snapshots

This directory is populated by `scripts/refresh-data.sh` and checked into the
repository so that every build is reproducible. Do not edit the CSV snapshots
or `metadata.json` manually.

All three CSV files are downloaded from the same immutable commit in the CDS
`gcorg-resolver` repository. That repository refreshes its concordance and
organization information from the Government of Canada Open Government dataset
before regenerating its curated aliases.

`metadata.json` records the shared source commit, immutable raw URLs, the
original authoritative dataset URL, and each file's SHA-256 digest.

The compiler verifies all three SHA-256 digests before producing the API's
static lookup table.
