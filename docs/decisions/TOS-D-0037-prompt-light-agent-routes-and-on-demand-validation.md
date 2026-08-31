# TOS-D-0037 Prompt-light agent routes and on-demand validation

## Index Metadata

- Decision ID: TOS-D-0037
- Original date: 2026-08-31
- Surface classes: agents/route-cards, docs/validation, docs/public-entry, scripts/validation, tests/topology
- ToS layers: agents, docs, validation, root, ports
- Tree classes: agent route, context boundary, validation route, owner handoff, generated read model
- Guard families: progressive disclosure, command authority, owner boundary, context budget, public navigation, generated parity
- Posture: accepted

## Context

`TOS-D-0008` correctly removed command inventories from the public root README,
but placed executable validation authority in `AGENTS.md`, local route cards,
and the release entrypoint. `TOS-D-0009` later made
`docs/validation/validation_lanes.json` the sole machine command authority, and
`TOS-D-0031` reduced one unusually large script card while preserving the rest
of the route-card corpus.

At the current owner head the tracked documentation corpus contains 55
`AGENTS.md`, one `DESIGN.AGENTS.md`, 213 `README.md`, and no `VALIDATION.md`.
Forty route cards still expose executable or copyable command text, and their
inherited corpus is 102,781 bytes. Several cards also prescribe broad README
reading before the touched source or route is known. This makes procedure text
prompt-visible even though exact internal command order already belongs to the
lane manifest.

The README corpus has a different job. Root and local README files carry public
orientation, human usage, provenance, branch maps, source explanations, and
standalone navigation. Moving that material into inherited cards would make
the agent context larger and would weaken the public entry surface.

## Decision

Keep every current root and local README unless a separate owner review proves
that its human or public function is obsolete. README is a human-facing entry,
explanation, or usage surface and is opened on demand; it is not required
agent context merely because it is nearby.

Make every active `AGENTS.md` a prompt-light inherited semantic route card.
The root card carries repository identity, source-first authority, global
boundaries, and completion meaning. A nested card adds only action-changing
local scope, owner, invariants, stronger-owner returns, and the name of the
on-demand validation route. Active cards do not carry executable fences,
copyable command batteries, ordinary branch/PR/merge procedure, or an
unconditional README inventory.

Add root and district `VALIDATION.md` surfaces as human, on-demand operation
maps. Internal ToS validation selects named lanes from
`docs/validation/validation_lanes.json`; exact ordered commands remain owned by
that manifest. `scripts/validation_lanes.py` may inspect or execute a selected
sequence without copying its body into an agent card. A district
`VALIDATION.md` may retain an exact external-owner or mutation-bearing builder
procedure when the command is not owned by the internal lane manifest, but it
must name the stronger owner and its claim limit.

Keep `DESIGN.AGENTS.md` and
`docs/validation/agents_route_inventory.json` as authored shape and topology
sources. Rebuild `.agents/agents-route.current.json` and the cross-corpus
documentation currentness carrier from those sources. Validators protect
tracked-card parity, source-first routing, command-free active cards,
conditional README loading, validation-route closure, structural residue, and
declared context budgets without freezing ordinary prose as doctrine.

## Supersession Note

This decision supersedes only the procedure-placement part of `TOS-D-0008`
that named `AGENTS.md` and local route cards as executable validation owners.
It preserves that record's public-README boundary. It refines the
preserve-strong-cards posture of `TOS-D-0031` where command bodies or broad
preload lists remained in active cards. It does not supersede the route
inventory, task harness, or context-budget law from that decision.

`TOS-D-0009` remains stronger for machine command authority: the lane manifest,
not Markdown, owns internal sequence membership and order.

## Options Considered

- Keep executable procedures in AGENTS and only shorten the root card.
  Rejected because nested tasks would continue inheriting unrelated procedure
  text and the command-authority split would remain ambiguous.
- Move procedures and human explanations into README. Rejected because README
  is public/human navigation and should not become an agent execution contract
  or mandatory preload.
- Put every command in one root validation document. Rejected because
  external ports, mutation-bearing builders, and district-specific procedures
  have distinct owners and failure routes.
- Use prompt-light AGENTS, retained human README, manifest-backed root and
  district VALIDATION maps, and source-first generated views. Chosen because
  it reduces inherited context while preserving human navigation and exact
  owner routes.

## Rationale

Prompt-visible context should carry semantic constraints that change action,
not procedure bodies that can be selected after the touched surface is known.
Keeping human explanation in README and executable detail in on-demand
validation routes separates audiences without deleting useful knowledge.
Keeping exact internal command sequences in the existing manifest prevents
Markdown drift from becoming a second machine authority.

The district split is intentionally bounded. It exists where a real local
procedure or external owner route already exists; it is not a mandate to add a
new validation file beside every card.

## Consequences

Agents inherit less text and must choose the touched owner before loading
human explanation or executable procedure. Human contributors retain root and
local entrypoints. External eval, memo, and stats ports keep their own explicit
owner-bound procedures rather than being absorbed into the ToS release lane.

Adding or changing a route card now requires source-first currentness rebuilds
and a guard against executable residue, empty procedure headings, stacked or
dangling lead-ins, unconditional README preloads, and broken validation links.
Generated currentness remains evidence of file and route parity only; it does
not prove model behavior, semantic correctness, owner acceptance, CI, release,
or runtime health.

## Source Surfaces

- `AGENTS.md`
- `DESIGN.AGENTS.md`
- `README.md`
- `CONTRIBUTING.md`
- `VALIDATION.md`
- `.agents/VALIDATION.md`
- `ToS/VALIDATION.md`
- `docs/VALIDATION.md`
- `evals/VALIDATION.md`
- `kag/VALIDATION.md`
- `manifests/VALIDATION.md`
- `mechanics/VALIDATION.md`
- `memo/VALIDATION.md`
- `scripts/VALIDATION.md`
- `stats/VALIDATION.md`
- `tests/VALIDATION.md`
- `docs/validation/validation_lanes.json`
- `docs/validation/agents_route_inventory.json`
- `docs/validation/documentation_family_map.json`
- `scripts/validation_lanes.py`
- `scripts/validate_nested_agents.py`
- `scripts/agents_route_harness.py`
- `tests/test_validation_lanes.py`
- `tests/test_nested_agents.py`
- `tests/test_docs_verify_routes.py`

## Validation

Rebuild the decision indexes, AGENTS currentness, and documentation-family
currentness through their canonical builders. Run the validation-authority,
route-docs, cross-corpus documentation, focused test, and release routes as
appropriate. The command evidence remains in the current lane manifest and
the on-demand `VALIDATION.md` owner maps.
