# TOS-D-0077 Public Corpus Snapshot in R2

## Index Metadata

- Decision ID: TOS-D-0077
- Original date: 2026-11-07
- Surface classes: access/public-corpus, access/web, bonsai, .github/workflows, docs/storage
- ToS layers: docs
- Tree classes: none
- Guard families: artifact integrity, provenance, bounded downstream seams
- Posture: accepted by explicit operator instruction

## Context

`tos-access public-corpus-build` writes the public corpus snapshot that the
stands read: about 151 MB of deterministic JSON in
`access/web/public/public-corpus/`, rebuilt eight times in its first week.
Its build needs reading-layer bytes held in item payload custody, so CI cannot
rebuild it from Git. [TOS-D-0076](TOS-D-0076-generated-stand-layers-in-r2.md)
moved the stands' word layers to R2 and kept the corpus in Git pending its own
route. Software CI builds and tests the access product with representative
fixtures and without production corpus.

## Decision

The snapshot's pages, exact-surface indexes, document manifests and analyses
live in R2 under the route of TOS-D-0076. `access/public-corpus/layers.json`
is their digest manifest; its `root` names the snapshot directory. The
snapshot's own `manifest.json`, `retained.json`, `revisions/` and `mappings/`
stay tracked: they bind the R2 files by digest and keep the snapshot's revision
history readable in Git. Publish Bonsai fetches the files, checks the stands
against them, writes what R2 lacks and copies them into the site's copy.

Without the files the stands read no corpus texts, the corpus-reading test
cases are skipped, and the bonsai check leaves corpus places unchecked; the
publication runs those cases and the full check before every deployment.

## Options Considered

- Keep the snapshot in Git, accepting about 44 MB compressed per rebuild of
  the current tree in every clone.
- Move the whole directory, including its manifest and retained revisions.
- Move the content-addressed bulk and keep the snapshot's manifests in Git.

The third option leaves Git with the snapshot's identity and history while the
bytes it names live in the locked store.

## Rationale

The snapshot's manifest already names each document manifest by digest, and
the layers manifest names every file; together they bind a commit to exact
bytes. Software CI then holds no production corpus, matching the access
product's fixture-only posture.

## Consequences

Producers of a new snapshot write its digests and send new files to
`layers/transfer` after replacing the directory. The custody record
`access/web/public/public-corpus/r2-manifest.v1.json` of the 2026-11-02 upload
stays as historical evidence of that upload. The core history snapshots stay
in Git.

## Source Surfaces

- `access/public-corpus/layers.json`
- `access/public-corpus/README.md`
- `bonsai/lib/layers.mjs`
- `bonsai/lib/graft.mjs`
- `.github/workflows/publish-bonsai.yml`
- `docs/STORAGE.md`

## Validation

Run the stand tests and the bonsai check with and without the corpus files,
build the public research stand in both states, and fetch the files into a
clean checkout from the transfer branch. The first publication after landing
verifies R2 writes and the site copy.
