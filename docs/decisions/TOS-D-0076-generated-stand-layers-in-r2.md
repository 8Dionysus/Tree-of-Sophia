# TOS-D-0076 Generated Stand Layers in R2

## Index Metadata

- Decision ID: TOS-D-0076
- Original date: 2026-11-07
- Surface classes: bonsai, access/web, .github/workflows, docs/storage
- ToS layers: docs
- Tree classes: none
- Guard families: artifact integrity, provenance, bounded downstream seams
- Posture: accepted by explicit operator instruction

## Context

The Zarathustra word layer (`bonsai/review/philology`: tokens, spelling,
origins, renderings) is generated from the witnesses with Stanza and
Wiktionary. It weighs about 18 MB and grows with each regeneration and each
stand. Its producers need models and extracts that CI does not hold, so CI
cannot rebuild it. The root rule keeps generated outputs out of Git, and
`docs/STORAGE.md` designates `tos-public-layers` for redistributable generated
public layers, keyed `sha256/<hex>` under indefinite retention.

## Decision

A stand's generated layers live in R2. Git keeps `bonsai/<stand>/layers.json`
(`tos.bonsai.layers.v1`): the declared paths, each file's SHA-256 and size, and
the rights of each group. `bonsai/lib/layers.mjs` writes and checks the
manifest, fetches files from R2, the site's copy or the transfer branch, and
writes only bytes matching their digest.

Files enter R2 only from main, in the Publish Bonsai workflow, which reads each
object back after writing it. A producer without R2 access puts new files on
`layers/transfer`, a single commit of objects named `sha256/<hex>`; the
publication keeps there only what R2 still lacks. The published site carries
the layers at `/__research/layers/sha256/<hex>`, readable without credentials.
Software CI builds and tests the stand without the layers; the bonsai check
then verifies the manifest, and with the files present it checks them like any
module.

## Options Considered

- Keep the layers in Git, accepting history growth with every regeneration.
- Regenerate the layers in CI, which needs model and dictionary downloads
  outside the CI budget.
- Keep them in R2 behind a digest manifest, with a transfer branch for
  producers without R2 access and a site copy for credential-free reads.

The third option keeps Git to the manifest, keeps unreviewed bytes out of the
locked store until merge, and lets any checkout reach the files.

## Rationale

The digest manifest binds a commit to exact bytes as tightly as Git does.
Writing to the locked prefix only from main means a pull request never leaves
permanent objects behind. The transfer branch holds one commit, so files leave
its reach once R2 holds them.

## Consequences

The public corpus snapshot and the core history snapshots stay in Git under
this decision. The corpus belongs to the native software package and its
software CI posture; the history snapshots compress to a few megabytes and
carry the upgrade path of saved personal graphs. Each needs its own owner
route before it moves. Publication now requires the R2 secrets named in
`docs/STORAGE.md`.

## Source Surfaces

- `bonsai/lib/layers.mjs`
- `bonsai/zarathustra/layers.json`
- `bonsai/lib/graft.mjs`
- `.github/workflows/publish-bonsai.yml`
- `docs/STORAGE.md`
- `bonsai/README.md`

## Validation

Run the bonsai check and the stand tests with and without the layers present,
the layer store tests (S3 signing vectors, fetch, upload, transfer branch), and
build the public research stand in both states. The first publication from
main verifies R2 writes and the site copy.
