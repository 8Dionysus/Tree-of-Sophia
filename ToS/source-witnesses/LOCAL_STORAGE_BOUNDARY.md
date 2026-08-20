# Local Source Storage Boundary

This route exists because source bytes are evidence, not a regenerable model
cache or a host-managed AI runtime.

## Current operator route

The operator-selected local corpus root is:

```text
/srv/AbyssOS/Tree-of-Sophia/ToS/source-witnesses/
```

Only the bytes beneath an item's `payload/` directory are ignored by Git.
Identity records, manifests, fixity, provenance, rights posture, forensic
reports, anchors, reviews, and the generated navigation catalog remain tracked.

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
