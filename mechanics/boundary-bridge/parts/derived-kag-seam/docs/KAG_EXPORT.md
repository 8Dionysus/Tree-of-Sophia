# ToS KAG Export

This document records the current source-owned tiny KAG export posture for Tree
of Sophia.

The export is deliberately narrow. It exposes one bounded source-node capsule
for downstream KAG consumers while keeping ToS-authored authority in its owning
surfaces.

## Current pilot

The pilot stays on the Zarathustra prologue route. The bounded export contains
six selected source files and one generated capsule:

- canonical source node: `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- source-facing documentation: `ToS/derived-exports/README.md`
- public compatibility concept and source nodes:
  `ToS/public-compatibility/concept_node.example.json` and
  `ToS/public-compatibility/source_node.example.json`
- supporting route surfaces:
  `ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md` and
  `ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md`
- generated capsule: `ToS/derived-exports/kag_export.min.json`

The selected CorpusStore revision is an explicit input to the builder. The
export binds the complete listed source closure and the generated capsule to
that revision.

## Core rule

The export is a source-owned guide surface. It may expose a bounded question,
summaries, interpretation-layer handles, and current route refs for downstream
consumption. Authored ToS authority remains in the canonical tree node and its
supporting ToS surfaces; the public entry remains a compatibility mirror.

## Tooling

`scripts/build_kag_export.py` is the explicit export builder and verifier:

```text
python scripts/build_kag_export.py build --store STORE --revision REVISION --output EXPORT
python scripts/build_kag_export.py verify EXPORT
```

The builder reads the selected CorpusStore revision and calls the pure renderer
`mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py`
with the staged source root. The renderer has no ambient repository or output
paths and only returns deterministic payload data.

`scripts/publish_kag_release.py` owns the local handoff to an explicitly
selected downstream KAG consumer and its release status:

```text
python scripts/publish_kag_release.py build --store STORE --revision REVISION --kag-root KAG_ROOT --release-root RELEASE_ROOT
python scripts/publish_kag_release.py status --release-root RELEASE_ROOT --expected-revision REVISION
```

The selected KAG owner supplies the actual consumer and records consumer
status/lag in its `kag/README.md` route. This ToS seam does not claim corpus
admission, public deployment, or the consumer's semantic validation.

## Current verification

For an already built export, run `python scripts/build_kag_export.py verify
EXPORT`. Verification checks the exact source membership, bytes, manifest
identity, source return, capsule structure, and bounded relation targets.

For a published downstream result, use the `status` command above. The release
publisher rechecks the private export copy, the selected consumer's returned
source identity, and the immutable release membership before reporting status.
The `public_entry` sequence in `docs/validation/validation_lanes.json` owns
broader route assurance.

## Regeneration

After the source owner has selected an accepted CorpusStore revision, build a
fresh export with the explicit `build` command above. No in-checkout Git parity
step or software merge gate is part of this export route; the selected revision
and its verified source objects are the input boundary.

If the downstream owner needs a release, invoke `scripts/publish_kag_release.py
build` with explicit store, revision, KAG root, and release root paths. Keep
consumer semantics, status, lag, admission, and deployment with their actual
owners.
