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
construction except where returned values legitimately retain their contents.
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

## Completion and selection

A complete output contains three mode-0600 files:

- `snapshot.sqlite`: committed `tos_local_prepared_read_model_v1` publication.
- `binding.json`: exact independent reader binding, written only after the
  post-publication source-state check.
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
access/tests/test_offline_prepare.py`. The fixture writes only five tiny real
core input carriers, invokes the executable in a subprocess, and compares
prepared catalog/node/lens/compressed-search reads to their source reference.
Test ownership is in `tests/test_inventory.json`; ordered validation authority
stays in `docs/validation/validation_lanes.json`.
