# Philosophy Domain Tree

## Index Metadata

- Decision ID: TOS-D-0003
- Original date: 2026-06-07
- Surface classes: source-home, domain-topology, scripts/validation, source-witness
- ToS layers: philosophy, source-witnesses, doctrine, canon, scripts
- Tree classes: context, source, concept, principle, lineage, event, state, support, analogy, synthesis, relation
- Guard families: domain topology, source-first authority, canon promotion, naming discipline, owner boundary
- Posture: accepted

## Context

The first ToS source-home pass created `ToS/` and moved the old root-level
active homes inside it. That made the home real, but it did not yet provide a
domain-shaped place for the large philosophical skeleton coming from the Tree
of Sophia Notion workspace.

The Notion material currently behaves as a broad skeleton of philosophy:
eras, regions, traditions, works, figures, concepts, source corpora, and
transmission routes. Treating that material as `candidate-intake/` would make
the main philosophical body look provisional. Treating it as a Notion mirror
would let UI labels control repository topology. Treating it as immediate
canon would erase review and source-anchor discipline.

## Decision

Adopt `ToS/philosophy/` as the domain-shaped growing philosophy tree.

This branch is the home for philosophical growth before typed canonical
promotion. Its first contour is:

- `ToS/philosophy/trunk/`
- `ToS/philosophy/eras/`
- `ToS/philosophy/threads/`
- `ToS/philosophy/graph-workbench/`

The Notion page that supplied the current skeleton is routed as a source
witness under `ToS/source-witnesses/notion/philosophy/`. The source page title
is metadata only; it must not become a repository path component.

## Rationale

The philosophy branch needs a convex, tree-shaped topology because future
growth will not stop at eras. Works, figures, commentarial lineages, source
corpora, concepts, and graph fragments will multiply inside each branch.

The selected shape lets each mature branch grow locally:

```text
eras/<era>/
  regions/<region>/
    traditions/<tradition>/
      works/
      figures/
      concepts/
      transmissions/
      sources/
      graph-workbench/
```

Cross-thread surfaces may help navigation across the whole tree, but they do
not own the objects. The home of a work, figure, or concept stays in the
branch where its witness and interpretation are grounded.

## Consequences

`ToS/candidate-intake/` remains available for provisional extraction, but it
is not the main route for the philosophy skeleton.

`ToS/canon/` remains the typed authored canon. `ToS/philosophy/` can prepare
local graph fragments and promotion ledgers, but canonical nodes and relation
packs still require review and validation before they move into canon.

`scripts/validate_philosophy_topology.py` enforces the first branch contour and
rejects source UI labels such as `zagotovki` as path components.

## Source Surfaces

- `ToS/philosophy/AGENTS.md`
- `ToS/philosophy/README.md`
- `ToS/philosophy/philosophy.manifest.json`
- `ToS/source-witnesses/notion/philosophy/AGENTS.md`
- `ToS/source-witnesses/notion/philosophy/witness.manifest.json`
- `scripts/validate_philosophy_topology.py`
- `scripts/validate_tos_source_home.py`
- `scripts/validate_nested_agents.py`
- `scripts/release_check.py`

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
