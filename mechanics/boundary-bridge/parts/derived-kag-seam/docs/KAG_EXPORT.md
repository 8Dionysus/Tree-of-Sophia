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

The standard installed `tos-ops-mechanics-plan` entry exposes the explicit
accepted-corpus export builder and verifier, owned by
`rust/crates/tos-ops-mechanics-plan/src/kag_corpus_export.rs`:

```text
tos-ops-mechanics-plan --repo-root REPO --kag-source-export-build --store STORE --revision REVISION --output EXPORT
tos-ops-mechanics-plan --repo-root REPO --kag-source-export-verify --kag-export EXPORT
```

The installed native builder reads the selected CorpusStore revision and calls
`derived_kag::build_payload` with the staged source root. This renderer has no
ambient repository or output paths and only returns deterministic payload data.
The old checkout-writing `--derived-kag-generate` and `--derived-kag-validate`
flags are compatibility controls, not accepted-corpus export or publication.

The native `tos-kag-release export-build` and `export-verify` operations own
this ToS export. They require only the selected ToS software and corpus. KAG
provider construction, Python execution, index validation and integration
status belong to the consuming repository. See [the independent export route](../../../../../kag/README.md).
