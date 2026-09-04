# Tree of Sophia

Tree of Sophia (ToS) is a source-grounded map of philosophy and world thought.
It organizes works, editions, files, passages, translations, concepts,
interpretations, contexts, and intellectual lineages into an authored,
reviewable tree. Systems reproduce this tree as graphs, search indexes, and
public routes.

**Live site:** [treeofsophia.com](https://treeofsophia.com/)

Source-backed branches cite their source. Scaffold branches remain explicit
about their provisional role until that evidence exists. Nodes and relations
record provenance, exact source routes, interpretation history, and review
status. ToS traces how ideas
descend, diverge, inherit, and return across languages, traditions, places,
and time while preserving uncertainty and competing readings.

## WebMCP Challenge — what was built

The source-first Tree predates the challenge; [`access/`](access/README.md)
contains the 2026 WebMCP submission completed in
[PR #189](https://github.com/8Dionysus/Tree-of-Sophia/pull/189).

- Codex discovers stable and selection-bound tools through `document.modelContext`.
- Human focus and agent actions share one revisioned page state and command core.
- Evidence Lens exposes provenance, competing readings, gaps, and bounded conclusions.
- Codex reroutes around disputed edges and updates the human-visible graph.
- Hypotheses and proposals stay traceable, local, noncanonical, and pending human review.
- No model API, API key, or separate MCP connection is required.

## Quick start

With Python 3.11+, run
`git clone https://github.com/8Dionysus/Tree-of-Sophia.git && cd Tree-of-Sophia`,
`python3 -m venv .venv`, `.venv/bin/python -m pip install -e 'access[mcp]'`,
`.venv/bin/tos verify --profile standalone`, and `.venv/bin/tos serve`.
Open [http://127.0.0.1:8080](http://127.0.0.1:8080); standalone archives and
native MCP are documented in [`access/README.md`](access/README.md).

## How ToS works

ToS uses inspectable source-to-review paths. For bibliographic works, the
primary identity path is:

`work -> expression -> edition -> item -> immutable file -> passage or region -> observation -> claim or interpretation -> human review -> canon or explicit deferral -> derived view`

Physical artifacts that do not naturally enter that ladder use the parallel
[`artifacts/` spine](ToS/source-witnesses/artifacts/README.md), and modern
documentary, critical, or synoptic reconstructions use the parallel
[`scholarly-composites/` spine](ToS/source-witnesses/scholarly-composites/README.md).
Those routes keep the object, its members, representations, readings, and
interpretations distinct; neither a composite coordinate nor an artifact ID
turns a reconstruction into an original, fixed reading, semantic fact, or
canon.

Every stage is identified and versioned. Witnesses ground material; doctrine
defines node, relation, naming, and evidence law; intake holds observations
and proposals; the review ledger records acceptance, rejection, ambiguity,
counter-readings, and rationale; canon holds reviewed authored knowledge;
exports derive graph, retrieval, KAG, and runtime views. Human review owns
textual and philosophical judgment, interpretation, and canon. Validators
cover mechanics. Authored ToS sources govern generated views.

## How ToS grows

ToS grows along two linked fronts:

- a world-philosophy corpus foundation that orders works, witnesses, editions,
  languages, time, place, traditions, transmission, and stable text addresses;
- *Thus Spoke Zarathustra* as the first golden growth kernel, a deeply worked
  branch testing the full path from source evidence through review to authored
  knowledge and derived views.

The corpus provides breadth. The kernel makes the cycle legible and reusable
and transfers source and review discipline. Each source keeps its ontology and
vocabulary; new works or traditions may extend the model.

## Current state

Current release: `v0.5.0`.

Foundation work tests heterogeneous witnesses, text representations,
segmentation, translation alignment, provenance, bibliographic and semantic
claims, and derived projections. Synthetic, private, model-made, and
unreviewed records retain their declared mechanical status. Text, identity,
translation, interpretation, rights posture, graph truth, publication
authority, and canon require matching source evidence and owner review.

## Explore

| Need | Route |
| --- | --- |
| Understand the architecture | [CHARTER](CHARTER.md) · [DESIGN](DESIGN.md) · [BOUNDARIES](BOUNDARIES.md) |
| Navigate model-facing tools and owner ports | [.agents/README](.agents/README.md) · [agent-surface map](.agents/agent-surface.manifest.json) · [agent-surface validation](.agents/VALIDATION.md) |
| Install standalone access | [access](access/README.md) · [runtime contract](access/contracts/runtime-manifest.v1.json) · [web actions](access/contracts/web-actions.v1.json) |
| Enter the philosophical tree | [ToS](ToS/README.md) · [philosophy](ToS/philosophy/) |
| Inspect the evidence foundation | [doctrine](ToS/doctrine/CORPUS_FOUNDATION.md) · [source witnesses](ToS/source-witnesses/README.md) |
| Follow the golden kernel | [contract](ToS/zarathustra/GOLDEN_GROWTH_KERNEL.md) · [public route](ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md) · [worked capsule](ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md) |
| Read generated views | [root map](ToS/derived-exports/root_entry_map.min.json) · [bounded KAG export](mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md) |
| Contribute and validate | [validation](VALIDATION.md) · [agent routes](AGENTS.md) · [mechanics](mechanics/README.md) · [cross-corpus map](docs/validation/README.md#cross-corpus-documentation) |
| Track direction and decisions | [ROADMAP](ROADMAP.md) · [CHANGELOG](CHANGELOG.md) · [records](docs/decisions/README.md) |

Meaning stays in authored sources, operations in `mechanics/`, and decisions
and projections with their named owners.

## License

Apache-2.0
