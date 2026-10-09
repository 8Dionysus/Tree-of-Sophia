# Offline prepared bootstrap

`tos prepare` is the native full source bootstrap for the existing installed
`PREFIX/bin/tos` product. It captures the five explicit knowledge carriers,
normalizes their existing families, and publishes a new local prepared SQLite
snapshot. No Python runtime, production child, fallback, D1 SQL/static export,
deployment, service startup, source admission or consumer switch occurs.

```bash
/absolute/prefix/bin/tos prepare \
  --source-root /absolute/source/Tree-of-Sophia \
  --output-dir /absolute/existing-parent/new-publication \
  --max-seconds 60
```

The output directory must be fresh in an existing parent. It is created with
mode 0700; the completed three-file ABI is `snapshot.sqlite`, `binding.json`,
`completed.json`. Binding and completion files are exclusively linked and
fsynced, with completion last. Failure retains an incomplete directory and
never retries, replaces output or grants completion. The existing publisher
owns SQLite rollback and disposable search scratch. Private computational
stage files are removed on success; they cannot mint a selected Stage receipt.

The source capture reuses the existing bounded retained native capture and
family normalizers. It selects exactly corpus, philosophy, bibliographic claims
and the two semantic registries from `--source-root`, independent of ambient
`TOS_*` source selectors. Its prepare profile does not require public release
contracts, evidence, audit or ledger files. Query vocabulary and schema code
companions are compiled into the installed executable. Philosophy v1/v2 and
the original carrier symlink resolution remain supported. The five selected
path/mtime/size/inode/ctime observations and captured digests are checked before
publication, after publication, and before/after an optional maintenance commit.
Source refs and portable root-relative values remain in the read model. The
receipt retains `source_state_checked: true` and
`ongoing_currentness_granted: false`: coherence at build time grants no future
currentness, rights, canon or semantic acceptance.

Publication defaults remain 64 MiB SQLite, 1 MiB compact row, 8 MiB metadata,
two million SQL mutations and the existing delta-only allowances. Explicit
`--max-bytes` and `--max-mutations` change only those publication allowances.
`--bulk-search-scratch-bytes` plus `--bulk-search-scratch-mutations` select the
existing exclusive scratch initializer; buffered is the default. Scratch has
its own byte/write cap and contributes to the publication mutation budget.
`--attach-maintenance` selects the separate existing catalog/semantic attachment
transaction; `--maintenance-max-mutations` requires it. The source capture and
normalization use the existing compiler profile (8 GiB per capture/stage/TEMP
carrier, 16 GiB cumulative work), not publication byte limits. These declared
caps are refusal envelopes, neither host admission nor measured RAM/disk fit.
The caller reserves source/image/output, journals, TEMP and scratch coexistence
and supplies a whole deadline before invoking the command. No full corpus or
capacity result is inferred from finite fixture execution.

The Python `tos_access.prepare` module and source-graph producer are retired.
Invoke the installed `tos-access prepare` command directly. The native producer
binds its own normalization processor and configuration; it does not claim
identity with the retired Python implementation.

## Optional maintenance attachment

`--attach-maintenance` explicitly adds the existing exact catalog and auxiliary
semantic indexes to this same snapshot. It works with either buffered or bulk
search. Without this flag, snapshot packet keys, output fields and the three-file
ABI are unchanged, and no maintenance indexes are constructed.

```bash
/absolute/prefix/bin/tos-access prepare \
  --max-seconds ADMITTED_SECONDS \
  --source-root /absolute/source/Tree-of-Sophia \
  --output-dir /absolute/existing-parent/new-maintainable-publication \
  --attach-maintenance --maintenance-max-mutations 2000000
```

The native command passes the selected header, original registry bytes and
portable lens specs directly to the native maintenance owner. The producer makes
the captured header and lenses portable, preserves the original registries and
their normalization-binding digests, and verifies that the portable rows plus
those registries reproduce the selected catalog and semantic report. Attachment
refuses if they do not; it never reconstructs stronger inputs from a catalog or
rewrites registry values to force success.

After base publication commits, a separate `BEGIN IMMEDIATE` transaction invokes
the existing `bootstrap_prepared_maintenance_transaction` kernel. It receives
the same on-demand portable row stream in an additional pass, not another full
normalized graph. Source state and selected publication binding are checked
before and after attachment; the kernel verifies exact catalog and semantic
report reproduction. This bootstrap only creates auxiliary maintenance state:
it does not change header, rows, catalog, search, epoch or independent reader
binding, verify a source transition, select a consumer, or grant semantic
acceptance. See [catalog index](EXACT_CATALOG_INDEX.md) and
[semantic index](SEMANTIC_INDEX.md) for the unchanged kernel contracts.

`--maintenance-max-mutations` requires the flag and defaults to two million. It
is a separate allowance for the attachment transaction's combined catalog and
semantic SQL mutations, with the same positive-integer/`2**53 - 1` ceiling as
publication. `--max-mutations` still caps base publication, including bulk
scratch writes. Their sum is a declared **upper bound**, not an observed actual
combined total: base publication and attachment use separate connections and
the publisher returns no mutation counter. The attachment receipt records its
own measured `sql_mutations`; semantic writes also keep their narrower owner cap.

All SQLite byte limits address the same whole file. Attachment uses the minimum
of publication `max_bytes`, catalog `max_index_bytes`, and semantic `max_bytes`;
it never interprets them as additive capacities. CLI owner defaults still
include semantic 32 MiB input accounting and 256 MiB whole-file limits, catalog
4 GiB whole-file limits, and publication 64 MiB whole-file limits. Raising only
the CLI publication cap does not lift semantic or catalog limits. Advanced
callers can supply complete owner profiles with `--publication-limits` and
`--maintenance-limits`; these remain refusal budgets, not full-corpus admission
or RAM forecasts.

Only opt-in completion adds `maintenance` to the existing receipt. This field
records attachment status, unchanged binding, catalog/report digests, declared
and effective limits, measured attachment writes, mutation-budget upper bound,
and explicit false publication-change/consumer-switch/source-transition/semantic-
acceptance claims. It is not a new source authority or delta acceptance receipt.

Any exception, interruption, budget refusal, source drift or binding drift
before attachment commit rolls back both auxiliary lanes. The already committed
base file remains an **incomplete, unselected** output without final JSON markers.
After attachment commits, the producer checks source state again and writes
`binding.json`, then `completed.json`. A later source check or JSON/link/fsync
failure cannot roll back committed SQL: it leaves an incomplete output, possibly
with auxiliary indexes and `binding.json`, but no valid completion marker. No
automatic cleanup, fallback to completed base-only output or partial resume is
performed. Completion and explicit selection retain the boundary below.

## Completion and selection

A complete output contains three mode-0600 files:

- `snapshot.sqlite`: committed `tos_local_prepared_read_model_v1` publication.
- `binding.json`: exact independent reader binding, written only after the
  post-publication source-state check and any requested maintenance commit/check.
- `completed.json`: last, atomically linked completion receipt with schema
  `tos_offline_prepared_bootstrap_receipt_v1`, source revision, normalization
  binding, output filenames, full binding, explicit declared limits, node/relation
  counts and final SQLite byte size. It is also printed to stdout on success.

Only a valid `completed.json` marks this output-directory ABI complete. Missing
marker means incomplete even if `snapshot.sqlite` or `binding.json` exists.
Intermediate JSON is written and fsynced in private `.partial` files, then
exclusively linked to its final name. A failure exits nonzero with a bounded
JSON error class on stderr, no success stdout, and no attempt to clean arbitrary
paths. Interruptions may leave partial files; no automatic recovery is implied.

Select the snapshot and binding from a complete output **explicitly**, using
the normal prepared reader/CLI selection documented in [README](README.md):

```bash
/absolute/prefix/bin/tos-access \
  --prepared-read-model /absolute/existing-parent/new-publication/snapshot.sqlite \
  --prepared-binding /absolute/existing-parent/new-publication/binding.json \
  knowledge catalog
```

Keep the independent binding with the selected snapshot. No receipt-reader
integration or implicit fallback is installed. Subsequent addressed storage
deltas and stale binding refusal belong to [local prepared publication](LOCAL_PREPARED_PUBLICATION.md).
The completed marker records bootstrap completion only; it must not be treated
as a refreshed binding after a later mutation.

The installed native command test, `rust/crates/tos-access/tests/native_prepare.rs`,
checks source revision, registry identity, completion receipt, private output,
binding, buffered/bulk publication, optional attachment, invalid-cap refusal
and occupied-output preservation. It exercises the real native CLI. Python
graph-builder and adapter tests were retired with those implementations. The
fixture uses only five tiny core input carriers; it makes no full-corpus
capacity claim or human semantic judgment.

Native prepare measures the executing ELF with a bounded streaming SHA256 read,
keeps the executing file open through normalization, and verifies the exact
file/path physical stamp again before completion. It streams the ELF once per
call; a second full hash is not required by this protected executing-image
custody profile. That processor binding is separate from source revision and the
registry/query configuration binding. The D1 publisher identity is unchanged.

An explicit `--source-limits` JSON profile may narrow input, capture, stage and
TEMP envelopes; absence preserves production defaults. Limits describe refusal
boundaries and do not reserve storage. The finite native consumer selects 8 MiB
publication/index/file caps and retains one already required successful output
with its separate fixture-origin receipt. This is a derivative read publication,
without source admission, ongoing currentness or an incremental source-state
baseline.
