# Source-First Decision Rationale Lane

## Index Metadata

- Decision ID: TOS-D-0001
- Original date: 2026-06-04
- Surface classes: docs/decisions, docs/route-law, scripts/validation
- ToS layers: docs, generated, scripts
- Tree classes: none
- Guard families: source-first authority, generated index parity, owner boundary
- Posture: accepted

## Context

`Tree-of-Sophia` already has strong current surfaces for source authority, bounded public entry, tree contracts, review posture, and downstream export. The rationale for why those surfaces exist has historically lived in route docs, review notes, roadmap entries, and neighboring federation memory.

That is workable for small movement, but it becomes fragile as ToS grows. A future contributor should be able to find durable rationale for route-law and boundary choices without mistaking that rationale for current source canon.

Sibling AoA repositories now use generated decision indexes to keep durable decisions discoverable. ToS should take the useful pattern, but not the sibling mechanics vocabulary. ToS decisions need to route by source-first layers, tree classes, and guard families.

## Decision

Adopt `docs/decisions/` as the durable ToS decision rationale lane.

Decision records use the canonical `TOS-D-####` ID pattern, full canonical-ID filenames, and an `## Index Metadata` block. Generated lookup indexes under `docs/decisions/indexes/` expose the records by number, date, surface class, ToS layer, tree class, and guard family.

Decision records explain why a route was chosen. They do not replace the current source, intake, tree, docs, examples, generated, or scripts surfaces they describe.

## Options Considered

- Keep rationale only in route docs and review notes. This preserves the current surface count, but future agents have to rediscover durable decisions across unrelated docs.
- Copy the AoA sibling decision lane literally. This gives symmetry by shape, but imports mechanics vocabulary that does not fit ToS source-first authority.
- Create a ToS-local decision lane with generated indexes and ToS-specific metadata. This preserves sibling discoverability while keeping the local owner language honest.

## Rationale

ToS needs durable rationale because its hardest choices are usually boundary choices: source versus interpretation, candidate versus canon, tree authority versus downstream export, and human-reviewed meaning versus generated convenience.

The selected lane keeps those choices legible without moving authority away from the owning surfaces. The generated indexes are deliberately cheap read models. They help an agent find the relevant decision, then route back to the source, tree, route, review, or export surface that owns current truth.

The metadata is ToS-specific:

- `ToS layers` names source, intake, tree, docs, examples, generated, or scripts pressure.
- `Tree classes` names source, concept, lineage, context, principle, event, state, support, analogy, synthesis, relation, or `none`.
- `Guard families` names the source-first, boundary, export, validation, provenance, or review guard that shaped the decision.

## Consequences

New durable route-law, boundary, validator, export, or source-discipline decisions should land as `TOS-D-####` notes when the rationale would otherwise be hard to reconstruct.

Existing review notes remain review notes. They do not need retroactive conversion.

If a decision changes, a new decision supersedes the old one. Existing IDs and filenames are not renumbered.

Generated indexes must stay derived from decision metadata and must not be hand-edited.

## Source Surfaces

- `README.md`
- `CHARTER.md`
- `BOUNDARIES.md`
- `ROADMAP.md`
- `docs/AGENTS.md`
- `scripts/AGENTS.md`

## Validation

Run:

```bash
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
```

For this first landing, also run the nested guidance validator and the repository test suite because the lane adds new route-law and script surfaces.
