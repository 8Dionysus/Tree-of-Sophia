# Offline prepared bootstrap

`python -m tos_access.prepare` explicitly assembles the selected source tree's
complete normalized graph/catalog and publishes a new local prepared SQLite
snapshot. This is **full bootstrap**, not source-incremental processing. It uses
the portable access package only: no Cloudflare builder import, edge SQL, D1,
deployment, service startup or automatic consumer switch.

```bash
PYTHONPATH=access/src python -m tos_access.prepare \
  --source-root /absolute/source/Tree-of-Sophia \
  --output-dir /absolute/existing-parent/new-publication
```

Both paths are required. The parent output directory must already exist; the
output path itself must not exist, including an empty directory or symlink.
The producer creates it with mode 0700. It never replaces existing output or
deletes an incomplete attempt. Select a fresh path to retry and review retained
partial output under its storage owner. Host resource admission, adequate RAM
for whole-source normalization and disk/journal capacity remain caller duties.

The five knowledge input paths are explicitly rooted in `--source-root` and
override ambient `TOS_*` carrier selection; ancillary paths are explicitly
rooted too. Core's explicit `knowledge_snapshot_once()` reads the five carriers
without populating or evicting process-global raw caches, uses the shared
graph/catalog builders, and does not retain canonical carrier bytes for a future
mutable CAS delta. It checks the complete path/mtime/size/inode/ctime tuple
before and after input reading, graph construction, and catalog construction;
source drift refuses this attempt. The producer checks that state again before
and after publication. Ambient normalization-cache context is disabled only
for this one-shot build and restored afterwards. Existing mutable snapshots and
their caches remain untouched. The receipt identifies the source
revision and normalization binding observed during the build. A successful
check does not prove future source currentness, rights, semantic acceptance,
canon or any external deployment. Source symlinks retain the core's normal
resolution semantics; this is explicit selection, not a filesystem sandbox.

The producer applies the same recursive portable path-value conversion as the
edge builder: the exact root becomes `Tree-of-Sophia`, root-prefixed strings
become relative, dictionary keys and non-string/list/dict values are unchanged.
No normalization cache is selected or written. Complete graph construction still
retains full-size source and normalized objects while building; this is not a
streaming source compiler. Local input references expire after graph/catalog
construction except where returned values legitimately retain their contents;
the optional maintenance handoff below retains only its exact header, registries
and saved-lens specifications, not a canonical source-transition baseline.
The producer converts each row
on demand in two repeatable publication passes, so path conversion does not
retain a second complete graph. Header and catalog are converted separately.
Publication uses
`PublicationLimits` defaults: 64 MiB SQLite file, 1 MiB compact row, 8 MiB
metadata, two million SQL mutations, plus the publisher's delta-only limits
(4096 changes / 16 MiB). These are mechanical refusal caps, not host memory
forecasts or write authority. For a larger corpus, explicitly select
`--max-bytes BYTES` and `--max-mutations COUNT` after obtaining resource/storage
admission. Both require positive integers and are recorded in the completion
receipt. Portable storage admission also bounds the byte cap to `2**40` and
the full-bootstrap mutation cap to `2**53 - 1`; invalid selections fail before
source normalization or output creation. These upper bounds are numeric storage
contracts, not recommended operating budgets. Addressed search deltas retain
their separate 20,000,000 ceiling; a bootstrap budget is not a delta allowance.
Raising these whole-publication caps does not raise the row/metadata
limits, reserve any space, switch readers, or make full normalization incremental.
The byte cap covers the SQLite file, not its transient journal or source memory;
reserve those separately. There is no automatic retry with larger caps and no
cache flag.

For explicit bulk search construction, add **both**
`--bulk-search-scratch-bytes BYTES` and
`--bulk-search-scratch-mutations COUNT`. They select the scratch-backed
initializer described in [local prepared publication](LOCAL_PREPARED_PUBLICATION.md);
the old buffered route remains the default. Scratch is exclusively created as
`.search-sort.sqlite` inside the new output directory, capped independently,
and disposed before success. Both scratch limits are validated before source
work/output creation. A process interruption can still leave a disposable
scratch candidate; it is never an admitted or resumable snapshot.

Reserve scratch in addition to the main file and journal/headroom. Scratch
mutations also count toward `--max-mutations`, including the publisher's exact
carrier/header overhead. The completion receipt records `search_bootstrap`
(`bulk` or `buffered`) and `search_scratch_limits` (null for buffered). These
physical-build choices do not alter the logical binding or source admission.
This option eliminates incremental posting maintenance during the initial
cohort load; full source normalization remains a separate measured stage.

## Optional maintenance attachment

`--attach-maintenance` explicitly adds the existing exact catalog and auxiliary
semantic indexes to this same snapshot. It works with either buffered or bulk
search. Without this flag, snapshot packet keys, output fields and the three-file
ABI are unchanged, and no maintenance indexes are constructed.

```bash
PYTHONPATH=access/src python -m tos_access.prepare \
  --source-root /absolute/source/Tree-of-Sophia \
  --output-dir /absolute/existing-parent/new-maintainable-publication \
  --attach-maintenance --maintenance-max-mutations 2000000
```

The opt-in snapshot call requests `knowledge_snapshot_once(include_catalog_inputs=True)`.
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

Focused regression: `PYTHONPATH=access/src:access/tests python -m unittest
test_offline_prepare test_offline_maintenance`. The fixture writes only five tiny real
core input carriers, invokes the executable in a subprocess, and compares
prepared catalog/node/lens/compressed-search reads to their source reference.
Attachment cases cover both search initializers, exact registry/lens handoff,
cache restoration, declared/effective caps, transaction and marker ordering,
rollback on kernel failures/interruptions/drift, and post-commit marker failure.
Test ownership is in `tests/test_inventory.json`; ordered validation authority
stays in `docs/validation/validation_lanes.json`.
