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
