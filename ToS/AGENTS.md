# AGENTS.md

This file applies to the `ToS/` source home.

## Role

`ToS/` is the source-home organ for Tree of Sophia. It holds authored
philosophical meaning, witnesses, doctrine, the Zarathustra golden growth
kernel, research packets, candidate intake, canon, public compatibility,
derived exports, contracts, and review evidence as one tree-shaped home.

Root files remain the public front door and landing surface. `docs/decisions/`
keeps durable rationale.

## Operating Card

| Field | Route |
| --- | --- |
| role | source-home organ for Tree of Sophia |
| input | witness, doctrine, Zarathustra route, research packet, philosophy branch, candidate pass, canon object, compatibility mirror, export, contract, or review note |
| output | branch-shaped ToS surface with visible owner, source posture, and validation route |
| owner | `ToS/AGENTS.md` for home law; nearest nested `AGENTS.md` for branch law; `ToS/source_home.manifest.json` for branch inventory |
| next route | witness or research packet -> zarathustra, philosophy, or candidate intake -> canon -> public compatibility -> derived export |
| tools | branch manifest, schema, generator, validator, review checklist, or decision record when needed |
| check | source-home, philosophy-topology, and route-card validators |

## Read First

Use `ToS/README.md`, `ToS/source_home.manifest.json`,
`ToS/doctrine/KNOWLEDGE_MODEL.md`, `ToS/doctrine/NODE_CONTRACT.md`, and the
nearest nested `AGENTS.md` for the branch being touched.

## Branch Routes

| Branch | Owns |
| --- | --- |
| `doctrine/` | knowledge law, node contracts, templates, interpretation posture |
| `source-witnesses/` | source-facing witness material |
| `zarathustra/` | first golden growth kernel, private lived-witness route, and current bounded route orientation |
| `research-packets/` | non-authoritative research scaffolds |
| `philosophy/` | domain-shaped philosophical growth |
| `candidate-intake/` | provisional extraction and promotion residue |
| `canon/` | authored nodes, relations, and registries |
| `public-compatibility/` | public-safe mirrors and tiny-entry examples |
| `derived-exports/` | generated downstream read models |
| `contracts/` | schemas and structural contracts |
| `review-ledger/` | dated inspection evidence |

## Golden-Kernel Engineering Route

When a change belongs to the Zarathustra kernel, classify the artifact before
editing:

| Artifact | Owner |
| --- | --- |
| primary text, edition, translation, or aligned witness | `source-witnesses/` |
| voluntary first-person lived testimony | `zarathustra/lived-witness/` |
| annotation layer, semantic distinction, or promotion law | `doctrine/` |
| machine-checkable payload shape | `contracts/` |
| extracted observation or agent proposal | `candidate-intake/` |
| accept, reject, defer, ambiguity, or counter-reading receipt | `review-ledger/` |
| accepted authored node or relation | `canon/` |
| public mirror | `public-compatibility/` |
| graph, KAG, retrieval, or compact reader | `derived-exports/` |

Do not bypass an intermediate owner because a downstream representation is
easier to generate. A gold packet is not ready when only its accepted labels
are reproducible; rejected and unresolved decisions must remain inspectable.

## Corpus-Foundation Engineering Route

For corpus intake and addressing, read
`ToS/doctrine/CORPUS_FOUNDATION.md`, then
`ToS/source-witnesses/README.md` and its `AGENTS.md`.

Keep these boundaries visible:

| Surface | Owner |
| --- | --- |
| identity, anchor, text-unit, sign, assertion, and translation law | `doctrine/` |
| tracked catalogs, object records, payload route, fixity, provenance, rights, source-near text | `source-witnesses/` |
| machine-checkable record shapes | `contracts/` |
| extraction/OCR/LLM/embedding/graph experiment mechanics | `abyss-stack` |
| host models, caches, runtimes, reservations, and large temporary data | `/etc/abyss-machine` and `/srv/abyss-machine` owner routes |

Paths speak; IDs persist. Every derived layer cites the exact input digest and
transformation event. Graph and retrieval stores remain disposable readers.

## Boundary Routes

- Keep `ToS/` as one tree-shaped home with branch owner surfaces.
- Keep source witnesses, doctrine, canon, compatibility, and exports distinct.
- Keep `ToS/philosophy/` for domain growth; route provisional extraction to
  `ToS/candidate-intake/`.
- Route operational process to `mechanics/` and durable rationale to
  `docs/decisions/`.
- Route runtime, proof, memory, KAG substrate, federation, SDK, skill, and
  technique authority to owning AoA repositories or layers.

## Validation

Run the branch-local validator first. For source-home topology changes, use
`scripts/validate_tos_source_home.py`, `scripts/validate_lived_witness_route.py`,
`scripts/validate_philosophy_topology.py`, and
`scripts/validate_nested_agents.py`.
