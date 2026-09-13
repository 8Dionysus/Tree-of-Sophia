# Local Source Storage Boundary

This route exists because source bytes are evidence, not a regenerable model
cache or a host-managed AI runtime.

## Current operator route

The operator-selected local corpus root is:

```text
/srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses/
```

Only the bytes beneath an item's `payload/` directory or an exact scholarly-
composite representation's `payload/` directory are ignored by Git. The latter
route is `scholarly-composites/<method>/<tradition>/<composition>/representations/<representation>/payload/<file>`.
Identity records, representation records, manifests, fixity, provenance, rights
posture, forensic reports, anchors, reviews, and the generated navigation
catalog remain tracked. No metadata subtree is covered by the payload ignore.

A materialized scholarly-composite File uses `local_gitignored_payload` and
`git_tracked=false`, matching Item custody without inventing a bibliographic
Item. Its record, rights and acquisition event keep the exact source URL,
byte size and SHA-256. Existing `not_materialized` records remain historical
declarations until an authorized acquisition or restoration records a new
materialization event; their old discovery and terminal receipts are not
rewritten.

An ignored File may be absent from another checkout even when its source
record declares prior materialization. The default foundation check permits
that clone state; `--require-local-payloads` requires the exact bytes locally.
When present, bytes must match their size, SHA-256 and File ID and must remain
untracked and ignored. The record alone is not current availability evidence.
Changed content needs a new File and provenance event; neither a URL refresh
nor a new download may overwrite the retained witness with different bytes.

## Explicit external payload roots

The metadata checkout and the physical payload directory are separate inputs.
`scripts/acquire_registry_sources.py` and
`scripts/validate_source_witness_foundation.py` accept
`--payload-source-root` when the selected directory mirrors the
`ToS/source-witnesses/` tree. The default remains the checkout's own source
root for read-only verification; acquisition requires the explicit flag and
refuses to write into a metadata checkout by default. An external root changes only byte location; it does not move Item,
File, rights, provenance, or review authority and it must never be inferred
from a cache or archive path.

`scripts/source_payload_custody.py` is the bounded transfer seam for an
explicit manifest or retained inventory. It rejects absolute and parent
references, symlink escapes, differing existing bytes, and payload paths
outside the Item `payload/` directory. A new payload is written to a
same-directory temporary file, read back by size and SHA-256, and published
with an exclusive no-clobber link. Every result is recorded per Item/File and
manifest, including source-invalid, missing, already-present, conflict, and
copied states. The receipt proves mechanical custody and fixity only; it does
not assess rights, semantic identity, canon, publication, or human approval.

An external-root check may use a different metadata checkout from the
physical root. Git ignore/tracking posture is evaluated against the stable
`ToS/source-witnesses/.../payload/...` reference, while bytes are opened from
the explicitly supplied directory. The root must be an existing non-symlink
directory, and each selected payload must remain a direct regular file. No
full-corpus scan or implicit archive restore is part of this route. The
operator owns and serializes writes to a selected root; untrusted concurrent
writers are outside this bounded route's guarantees. The exclusive
no-clobber publication protects an existing destination, while source and
directory changes during a transfer must be prevented by the owner or cause
the operation to be reviewed again.

`local_only` governs access to the source bytes; it does not erase their
research role. A local item may remain the exact witness behind extraction,
comparison, translation work, annotations, or later claims. Public-safe
metadata may name the work, expression, translator, edition, year, local item
ID, file digest, and provenance relation when the evidence and privacy posture
support those assertions. Publishing that provenance is not publication of
the payload.

A content-bearing derivative receives its own rights and visibility
assessment. It may cite the local witness while remaining local itself, or it
may be published only when its own scope is affirmatively permitted. The
source payload does not become publishable merely because a metadata record,
claim, or separately permitted derivative refers back to it.

## Host-policy interpretation

`abyss-machine` owns host caches, model downloads, runtimes, benchmark output,
and other regenerable machine artifacts under `/srv/abyss-machine/`. It does
not take ownership of project source evidence inside `/srv/AbyssOS/`.

The host storage preflight can therefore classify this project target as
protected or unknown to host automation even when capacity is sufficient. That
result means "do not let host automation mutate this project tree"; it is not a
rights decision, a fixity result, or permission to redirect operator-designated
source evidence into a cache. Before any material write, record the preflight,
inspect capacity, resolve the exact item path, and require explicit project
scope.

## What never belongs here

- downloaded model weights;
- mutable model, compiler, browser, or OCR caches;
- temporary page renders and bulk OCR scratch output;
- benchmark runs and laboratory workspaces;
- graph databases, vector indexes, or deploy-local service state.

Those route through the owning `abyss-stack` laboratory and the machine storage
policy. A reviewed derivative promoted back into ToS receives its own tracked
record and provenance event; the lab directory itself never becomes source
truth.

## Future server transfer

A future site or server imports an item by manifest, verifies every digest,
applies the rights and visibility record, writes a receipt, and preserves the
stable item/file identifiers. Repository checkout, graph export, or catalog
generation must never silently upload local payload bytes.

The operator policy for the present local corpus is stricter: these local
source payloads remain off the future public site. The site may expose
public-safe source identity and provenance and may publish separately
authorized materials, but it must not receive or serve the local source files.
