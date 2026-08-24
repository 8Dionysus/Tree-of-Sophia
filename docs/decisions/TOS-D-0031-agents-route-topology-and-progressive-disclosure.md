# TOS-D-0031 AGENTS route topology and progressive disclosure

## Index Metadata

- Decision ID: TOS-D-0031
- Original date: 2026-08-24
- Surface classes: agents/route-cards, docs/validation, scripts/validation, tests/topology, context-evaluation
- ToS layers: agents, docs, validation
- Tree classes: agent route, context boundary, generated read model, owner handoff
- Guard families: tracked-card parity, inheritance, currentness, context budget, owner boundary, progressive disclosure, completion
- Posture: accepted

## Context

The earlier agent-surface decision, TOS-D-0029, intentionally left the
AGENTS.md documentation direction separate. The current route-card validator
discovered 54 cards because its fixed roots excluded tracked `.github/AGENTS.md`;
Git tracked 55 exact-basename cards. It also had no machine-readable inheritance
map, generated card currentness, representative owner-route probes, or context
budget guard.

The existing cards are generally strong and carry source-first boundaries,
owner routes, local checks, and escalation posture. Rewriting them would erase
useful local law. The concrete context pressure was the 619-line,
5440-whitespace-token `scripts/AGENTS.md` command and script-family dump. Its
release route inherited 6680 tokens when combined with the root card, while
the other representative stacks ranged from 1418 to 2679 tokens. Command
authority already belonged to `docs/validation/validation_lanes.json`, and
script coverage already belonged to `SCRIPT_TOPOLOGY.md` and
`script_inventory.json`.

## Decision

Keep the root and strong nested route cards source-first and owner-specific.
Make the root card the universal map of identity, topology, owner routes,
cross-surface boundaries, verification, and completion. Let each nested card
retain only action-changing local owner, source authority, invariants, nearest
check, and escalation or return routes. Keep specialized reference knowledge in
the named on-demand source or validator.

Add `docs/validation/agents_route_inventory.json` as the authored route map.
It records exact tracked-card discovery, root-plus-nearest-ancestor
inheritance, the preserved `docs/AGENTS_ROOT_REFERENCE.md` audit surface,
influencing files, context budgets, and 15 representative task routes spanning
source, corpus, text, translation, semantics, graph, public entry, mechanics,
decisions, release, skills, eval, stats, KAG, and memo ports. Generate
`.agents/agents-route.current.json` as a subordinate hash/count read model.
Use `validate_nested_agents.py` for structural, inheritance, reference, and
currentness admission and `agents_route_harness.py` for deterministic route
shape, budget, boundary, validation, and completion probes. The harness
explicitly makes no behavioral, semantic, human, or owner-acceptance claim.

Compress `scripts/AGENTS.md` into a route card that points to these existing
command and inventory owners instead of reproducing every script procedure.
Keep the script card's source/derived, private-payload, human-review, and
downstream-owner boundaries. Add only the narrow local law pressure exposed by
the probes: translation anchor sets and bounded public-entry posture. Add the
route-card sequence to the validation and release lanes, with positive and
negative focused tests.

## Options Considered

- Rewrite every route card into one new universal template. Rejected because
  it would discard strong local source, mechanics, review, and port law without
  evidence of pressure.
- Fix only the 54-versus-55 count. Rejected because count parity would not
  detect broken inheritance, stale executables, missing task law, currentness
  drift, or context regressions.
- Add a prose map and no executable carrier. Rejected because route and budget
  guarantees need repeatable machine checks.
- Move every specialized procedure into the root card. Rejected because it
  would increase always-on context and make the root a hidden replacement for
  owner docs, skills, scripts, and validators.
- Add an authored route inventory, generated currentness, deterministic
  harness, focused controls, and a compact pressured card. Chosen because it
  closes the measured gaps while preserving strong surfaces and existing
  authority boundaries.

## Rationale

The route inventory makes the tracked/discovered distinction explicit: the
previous validator counted 54; the candidate counts 55 tracked and 55
discovered, including `.github/AGENTS.md`. The inheritance rule is inspectable
and does not treat `DESIGN.AGENTS.md` or the preserved root reference as cards.
The generated companion provides file hashes and currentness without becoming
source authority.

The representative task routes keep source lineage, provenance, rights,
uncertainty, competing readings, review, canon boundaries, and manual semantic
judgment in their owner scopes. Their selected validation is named as lane IDs
and exact paths. The route harness reports declared on-demand reads, while
behavioral extra reads remain explicitly unknown because no model run was
performed. This separates deterministic route mechanics from behavioral eval.

The probe result is bounded and evidence-bearing: 15/15 routes succeed before
and after the change; release/validation inherited context falls from 6680 to
2091 tokens (4589 fewer, 68.7%); the other 14 routes gain 61 root completion
tokens each; owner hops and completion coverage remain unchanged; boundary
deviation and missing task-specific law are zero in the candidate. These are
route-harness results, not claims about a model's actual reading behavior.

## Consequences

The root card now states a concrete completion contract and nested cards can be
checked against one explicit topology. Script procedure detail remains in the
script topology, lane manifest, inventory, validators, skills, and owner docs.
Contributors must update the route inventory when adding a tracked card or
representative task route and regenerate currentness before validation.

The route budget is a guard against accidental context growth, not a universal
token law for source or prose. A task override is explicit in the inventory;
semantic judgment, rights clearance, human review, runtime health, CI status,
and owner acceptance remain separate claims. The preserved root reference stays
available for audit and is not an active inheritance source.

## Source Surfaces

- `AGENTS.md`
- `DESIGN.AGENTS.md`
- `DESIGN.md`
- `docs/AGENTS_ROOT_REFERENCE.md`
- `.agents/README.md`
- `.agents/agent-surface.manifest.json`
- `docs/validation/agents_route_inventory.json`
- `docs/validation/validation_lanes.json`
- `docs/validation/SCRIPT_TOPOLOGY.md`
- `docs/validation/script_inventory.json`
- `docs/decisions/TOS-D-0029-agent-tool-owner-port-documentation-architecture.md`

## Validation

Run `python scripts/build_agents_route_currentness.py --check`,
`python scripts/validate_nested_agents.py`,
`python scripts/agents_route_harness.py --check`,
`python scripts/generate_decision_indexes.py --check`, and
`python scripts/validate_decision_records.py`.

Also run the affected source, public-entry, mechanics, port, agent-surface,
focused-test, and repository release lanes. The decision record is weaker than
the current route cards and owner surfaces it explains.
