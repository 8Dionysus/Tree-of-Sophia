# Research Packet Authority Correction

## Index Metadata

- Decision ID: TOS-D-0006
- Original date: 2026-06-07
- Surface classes: source-home, domain-topology, scripts/validation, research-packet, docs/route-law
- ToS layers: docs, source-witnesses, research-packets, philosophy, canon, scripts
- Tree classes: source, concept, relation
- Guard families: source-first authority, validator restraint, owner boundary, metadata boundary
- Posture: accepted

## Context

The first philosophy-domain landing treated the Notion page that contained a
Deep Research GPT result as a source witness. That was wrong. Notion was only a
capture container, and the GPT result was only a non-authoritative research
scaffold.

The validator then made the mistake worse by requiring
`ToS/source-witnesses/notion/philosophy/witness.manifest.json`. A green check
therefore proved only that the repository matched a false route, not that the
route was true.

## Decision

Move the Deep Research philosophy packet out of `ToS/source-witnesses/` and into
`ToS/research-packets/deep-research/philosophy/`.

Research packets may preserve capture metadata and branch leads. They must not
be called source witnesses, authors, schools, works, doctrine, canon, or graph
authority. Claims that matter must later route to real source witnesses,
published works, editions, translations, or reviewed canon surfaces.

Revise `scripts/validate_philosophy_topology.py` so it checks boundary
invariants instead of creating ontology:

- reject AI/Notion packet placement under `ToS/source-witnesses/`;
- require research packets to declare `not_source_witness` and `not_canon`;
- keep capture titles out of repository path components;
- validate the already-owned philosophy branch contour.

## Options Considered

- Leave Notion under `source-witnesses/` but add warnings. This keeps the false
  authority in the path.
- Delete the packet entirely. This removes a useful scaffold before the
  philosophy tree has enough source-anchored replacements.
- Move it to a non-authoritative research-packet branch and correct the
  validators and decisions.

## Rationale

Validators in an agentic system should guard contracts, boundaries, and
regressions after the owner model is understood. They should not invent the
owner model and then present their own green result as truth.

The corrected route keeps useful research scaffolding available while making
its authority class explicit. `ToS/source-witnesses/` remains reserved for
source-facing witness material. `ToS/research-packets/` holds scaffolds that
must pass through branch review and source anchoring before they can influence
canon or graph surfaces.

## Consequences

`TOS-D-0003` and `TOS-D-0004` are corrected to describe the Deep Research
material as a research packet, not a Notion witness.

Future validators for ToS topology should prefer negative authority guards and
owner-route checks over exact path mandates unless the path is already owned by
the source surface itself.

## Source Surfaces

- `ToS/source-witnesses/`
- `ToS/research-packets/`
- `ToS/research-packets/deep-research/philosophy/research.manifest.json`
- `ToS/philosophy/AGENTS.md`
- `ToS/philosophy/philosophy.manifest.json`
- `scripts/validate_philosophy_topology.py`
- `scripts/validate_tos_source_home.py`
- `scripts/validate_nested_agents.py`

## Validation

Run:

```bash
python scripts/validate_philosophy_topology.py
python scripts/validate_tos_source_home.py
python scripts/validate_nested_agents.py
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
