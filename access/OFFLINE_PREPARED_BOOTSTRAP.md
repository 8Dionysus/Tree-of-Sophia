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

`python -m tos_access.prepare` and imported `prepare()` are compatibility
adapters to the same installed native command, through explicit
`--native-executable`/`TOS_PREPARED_EXECUTOR` or installed `tos` on PATH.
They do not assemble a Python graph. The importable `reference_prepare()` and
its source helpers remain the distinct oracle until final retirement. The
existing Python-specific reference tests retain their separate scope; native
command evidence must traverse the installed native route. The native producer
has its own normalization binding; it never copies the Python processor
identity to disguise a different executable.

## Optional maintenance attachment

`--attach-maintenance` explicitly adds the existing exact catalog and auxiliary
semantic indexes to this same snapshot. It works with either buffered or bulk
search. Without this flag, snapshot packet keys, output fields and the three-file
ABI are unchanged, and no maintenance indexes are constructed.

```bash
PYTHONPATH=access/src python -m tos_access.prepare \
  --native-executable /absolute/native-prefix/bin/tos --max-seconds ADMITTED_SECONDS \
  --source-root /absolute/source/Tree-of-Sophia \
  --output-dir /absolute/existing-parent/new-maintainable-publication \
  --attach-maintenance --maintenance-max-mutations 2000000
```

The retained Python oracle requests `knowledge_snapshot_once(include_catalog_inputs=True)`.
The native command carries the same captured header, original registry bytes and
portable lens specs directly to the existing native maintenance owner.
It returns copy-isolated `CatalogInputs` from the actual registries and saved-lens
carriers used by that coherent graph/catalog build, while normalization caching
is still disabled and before the final source-state check. The producer makes
the captured header and lenses portable, but preserves the original registries
and their normalization-binding digests. It never reconstructs stronger inputs
from a catalog. If portable rows/header/lenses plus those original registries
cannot exactly reproduce the selected catalog and semantic report, attachment
refuses; registry values are not rewritten or rebound to force success.

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
the CLI publication cap does not lift semantic or catalog limits. These defaults
are refusal budgets, not full-corpus admission or RAM forecasts. Advanced callers
can explicitly supply every owner limit, for example:

```python
from dataclasses import replace
from tos_access.catalog_index import CatalogLimits
from tos_access.semantic_index import SemanticIndexLimits
from tos_access.prepare import MaintenanceAttachmentLimits, prepare
from tos_access.prepared_publication import PublicationLimits

maintenance = MaintenanceAttachmentLimits(
    max_mutations=2_000_000,
    catalog_limits=replace(CatalogLimits(), max_index_bytes=128 * 1024 * 1024),
    semantic_limits=replace(SemanticIndexLimits(), max_bytes=128 * 1024 * 1024),
)
receipt = prepare(source_root, fresh_output_dir,
    limits=PublicationLimits(max_bytes=128 * 1024 * 1024), maintenance=maintenance)
```

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

```python
import json
from pathlib import Path
from tos_access.published_read_model import PublishedKnowledgeReadModel

output = Path('/absolute/existing-parent/new-publication')
completed = json.loads((output / 'completed.json').read_text())
assert completed['schema'] == 'tos_offline_prepared_bootstrap_receipt_v1'
assert completed['status'] == 'completed'
binding = json.loads((output / 'binding.json').read_text())
assert binding == completed['binding']
reader = PublishedKnowledgeReadModel(output / 'snapshot.sqlite', binding)
```

Keep the independent binding with the selected snapshot. No receipt-reader
integration or implicit fallback is installed. Subsequent addressed storage
deltas and stale binding refusal belong to [local prepared publication](LOCAL_PREPARED_PUBLICATION.md).
The completed marker records bootstrap completion only; it must not be treated
as a refreshed binding after a later mutation.

The existing real CLI cases may select a protected native command directly with
`TOS_NATIVE_SOURCE_PREPARE_EXECUTABLE`; the ordinary Python adapter selection
remains `TOS_PREPARED_EXECUTOR` plus `TOS_PREPARED_MAX_SECONDS`. This selected
bridge preserves the source-revision, registry identity, complete row/read,
exclusive output, buffered/bulk, attachment and binding assertions. Only the
native processor/configuration identity is treated as distinct from the Python
oracle. Python-specific injected source/marker/kernel failures call
`reference_prepare()` explicitly; a green reference case is not native evidence.
The finite selected consumer is four existing methods (real command reads,
output collision, buffered/bulk binding, opt-in maintenance buffered/bulk),
not a claim that all reference failure injection crossed the native process.

The fixture writes only five tiny real
core input carriers, invokes the executable in a subprocess, and compares
prepared catalog/node/lens/compressed-search reads to their source reference.
Attachment cases cover both search initializers, exact registry/lens handoff,
cache restoration, declared/effective caps, transaction and marker ordering,
rollback on kernel failures/interruptions/drift, and post-commit marker failure.
Test ownership is in `tests/test_inventory.json`; ordered validation authority
stays in `docs/validation/validation_lanes.json`.
