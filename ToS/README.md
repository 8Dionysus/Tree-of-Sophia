# ToS Source Home

`ToS/` is Tree of Sophia's authored source home. It keeps source witnesses,
knowledge law, philosophy growth, candidate work, review, canon, public
compatibility, contracts, and derived readers in one tree-shaped route.

Start with the smallest route below. The nearest `AGENTS.md` is the owner
handle; deeper documents, schemas, manifests, and validators are on-demand
detail. Home law is [`AGENTS.md`](AGENTS.md), and the branch inventory is
[`source_home.manifest.json`](source_home.manifest.json). Authored ToS surfaces
remain stronger than public, generated, graph, or runtime projections.

## Authority chain

```text
source witness -> addressed observation or proposal -> review outcome
  -> canon or explicit deferral -> public or derived projection
```

Keep the layers distinct: provenance, rights, uncertainty, competing readings,
review state, witness identity, and source lineage are evidence, not context
to summarize away. Research packets and candidate intake remain provisional;
validators prove mechanics, while source-visible human review owns textual,
translation, interpretive, rights, and canon judgments.

## Family reading map

| Family | Owns | First read / owner handle | Next detail or consumer |
| --- | --- | --- | --- |
| Source witnesses | Work/expression/edition/item/file identity, physical artifacts, scholarly composites, text layers, provenance, rights, and source-facing claims | [`source-witnesses/README.md`](source-witnesses/README.md) · [`source-witnesses/AGENTS.md`](source-witnesses/AGENTS.md) | [`doctrine/CORPUS_FOUNDATION.md`](doctrine/CORPUS_FOUNDATION.md) and the exact item, claim, or witness record |
| Doctrine | Knowledge model, node/relation law, interpretation posture, and authored route boundaries | [`doctrine/AGENTS.md`](doctrine/AGENTS.md) · [`doctrine/KNOWLEDGE_MODEL.md`](doctrine/KNOWLEDGE_MODEL.md) | [`doctrine/NODE_CONTRACT.md`](doctrine/NODE_CONTRACT.md) and the owning contract or review route |
| Contracts | Machine-checkable structure for source, translation, semantic, canon, compatibility, and export surfaces | [`contracts/AGENTS.md`](contracts/AGENTS.md) | The exact schema plus its example and validator; contracts do not decide meaning |
| Research packets | Non-authoritative AI-assisted or secondary research scaffolds and capture metadata | [`research-packets/AGENTS.md`](research-packets/AGENTS.md) | The nearest packet card, then [`deep-research/philosophy/AGENTS.md`](research-packets/deep-research/philosophy/AGENTS.md) or philosophy review; never source or canon authority |
| Candidate intake | Source-linked observations, proposals, uncertainty, and promotion residue before review | [`candidate-intake/AGENTS.md`](candidate-intake/AGENTS.md) | The pass-local pack, then philosophy or review/canon; never a public or derived authority |
| Review ledger | Dated inspection notes, outcomes, ambiguity, rejection, deferral, and migration evidence | [`review-ledger/AGENTS.md`](review-ledger/AGENTS.md) | The owning doctrine, source, canon, contract, or durable decision surface |
| Canon | Reviewed authored nodes, relation packs, and vocabulary registries | [`canon/AGENTS.md`](canon/AGENTS.md) · [`canon/relations/AGENTS.md`](canon/relations/AGENTS.md) | [`public-compatibility/`](public-compatibility/) or [`derived-exports/`](derived-exports/) only after authored change |
| Philosophy growth | Atlas pressure, eras, regions, traditions, domain branches, and pre-canon graph work | [`philosophy/README.md`](philosophy/README.md) · [`philosophy/AGENTS.md`](philosophy/AGENTS.md) | [`philosophy/philosophy.manifest.json`](philosophy/philosophy.manifest.json) and [`philosophy/graph-workbench/AGENTS.md`](philosophy/graph-workbench/AGENTS.md) |
| Zarathustra kernel | The first source-to-review growth cycle, lived-witness boundary, and bounded public route | [`zarathustra/AGENTS.md`](zarathustra/AGENTS.md) · [`zarathustra/GOLDEN_GROWTH_KERNEL.md`](zarathustra/GOLDEN_GROWTH_KERNEL.md) | [`zarathustra/public-entry/TINY_ENTRY_ROUTE.md`](zarathustra/public-entry/TINY_ENTRY_ROUTE.md) and the exact witness/canon route |
| Public compatibility | ToS-owned public-safe examples and tiny-entry mirrors subordinate to source and canon | [`public-compatibility/AGENTS.md`](public-compatibility/AGENTS.md) · [`public-compatibility/README.md`](public-compatibility/README.md) | The tiny-entry and mirror validators; do not widen a public payload from a projection |
| Derived exports | Generated graph, KAG, corpus, atlas, lexical, and compact read models | [`derived-exports/AGENTS.md`](derived-exports/AGENTS.md) · [`derived-exports/README.md`](derived-exports/README.md) | The named generator and validator, then the downstream consumer; exports never replace authored meaning |

## Task probes

Use the first-read route, then run the narrowest check named by the owner. A
green probe establishes structural or generated parity only; it does not
accept a source, translation, interpretation, claim, relation, rights status,
or canon object.

| Question | Read in this order | Narrow check |
| --- | --- | --- |
| Can I identify a source and preserve rights/provenance? | [`source-witnesses/README.md`](source-witnesses/README.md) -> [`CORPUS_FOUNDATION.md`](doctrine/CORPUS_FOUNDATION.md) -> exact witness | `python scripts/validate_source_witness_foundation.py` |
| Can I address text or compare a translation? | [`source-witnesses/AGENTS.md`](source-witnesses/AGENTS.md) -> [`contracts/AGENTS.md`](contracts/AGENTS.md) -> [`contracts/translation-alignment-packet-v1.schema.json`](contracts/translation-alignment-packet-v1.schema.json) -> the relevant research packet | `python scripts/validate_source_witness_foundation.py --translation-alignment-v1-lab-only` |
| Is an interpretation still provisional? | [`doctrine/KNOWLEDGE_MODEL.md`](doctrine/KNOWLEDGE_MODEL.md) -> [`candidate-intake/AGENTS.md`](candidate-intake/AGENTS.md) -> source anchor | `python scripts/validate_intake_pack.py` |
| What was accepted, rejected, deferred, or left contested? | [`review-ledger/AGENTS.md`](review-ledger/AGENTS.md) -> exact review -> [`canon/AGENTS.md`](canon/AGENTS.md) | `python scripts/validate_tree_node_contracts.py` and `python mechanics/relation-weaving/parts/graph-promotion/scripts/validate_tree_relation_pack.py` |
| How does a new corpus branch grow? | [`philosophy/README.md`](philosophy/README.md) -> atlas/branch manifest -> graph workbench -> candidate/review/canon | `python scripts/validate_philosophy_topology.py` |
| What may a person or downstream system consume? | [`zarathustra/public-entry/TINY_ENTRY_ROUTE.md`](zarathustra/public-entry/TINY_ENTRY_ROUTE.md) -> public compatibility -> derived export | `python scripts/validate_tiny_entry_route.py`, `python scripts/validate_root_entry_map.py`, and `python mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py` |

## Operating Card

| Field | Route |
| --- | --- |
| role | source-home entrypoint for ToS-authored philosophical work |
| input | witness, doctrine, Zarathustra route, research packet, philosophy branch, candidate pass, canon object, compatibility mirror, export, contract, or review note |
| output | branch-shaped ToS surface with a visible owner, source posture, and validation route |
| owner | `ToS/AGENTS.md` and `ToS/source_home.manifest.json` |
| next route | witness or research packet -> zarathustra, philosophy, or candidate intake -> canon -> public compatibility -> derived export |
| check | `python scripts/validate_tos_source_home.py` plus the owning branch validator |

## Boundary Routes

- `ToS/source-witnesses/` is the evidence spine; a file, composite, catalog
  row, or generated claim index is not by itself a reading, translation,
  semantic fact, or canon decision.
- `ToS/doctrine/`, `candidate-intake/`, `review-ledger/`, and `canon/` keep
  law, proposal, judgment, and authored meaning separate. Preserve competing
  readings and unresolved states instead of smoothing them into a summary.
- `ToS/public-compatibility/` and `ToS/derived-exports/` are downstream
  surfaces. Change their source-owned inputs or generator first; never use a
  projection as a shortcut back to authority.
- There is no top-level `public/` source home. The current public seams live
  under `zarathustra/public-entry/` and `public-compatibility/`, with derived
  companions under `derived-exports/`.

For operation, command, generated-reference, live-state, or execution-proof
questions, follow the named mechanic, script, MCP/KAG, runtime, or receipt
owner rather than expanding this authored entrypoint.
