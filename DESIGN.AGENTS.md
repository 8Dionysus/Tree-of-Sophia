# ToS Agent-Surface Design

An `AGENTS.md` is inherited routing guidance. It identifies the local owner,
action-changing invariants and the next source or verification route. Detailed
procedures stay with the named owner and load on demand.

## Card shape

A substantial card answers: what lives here, what input arrives, what may be
produced, which source owns it, where to move next and how to select verification.
Use prose or a table as appropriate. A small nested card needs only its local
delta; do not repeat its parent's identity, boundaries or workflow.

| Surface | Responsibility |
| --- | --- |
| root `AGENTS.md` | repository identity, owner map and cross-surface boundaries |
| `ToS/AGENTS.md` | authored source-home and branch routes |
| branch cards | local source, canon, review, contract or public-entry invariant |
| mechanics cards | package/part operation ownership |
| rust/scripts/tests cards | native implementation, deterministic builder and check ownership |
| docs cards | documentation and durable rationale ownership |

[DESIGN.md](DESIGN.md) owns system architecture. The
[model-facing guide](.agents/README.md) and its
[manifest](.agents/agent-surface.manifest.json) own optional profile bindings
and owner-port handoff. Human README files explain usage and provenance; they
are read when relevant rather than preloaded as universal instructions.

## Progressive loading

Read root and nearest card, then the exact source or contract. Choose the
smallest coherent change and its nearest `VALIDATION.md` route. Internal
command membership and order live in
[validation_lanes.json](docs/validation/validation_lanes.json).

Specialized knowledge belongs in the owning source, contract, decision,
README or skill. Keep executable commands out of active cards. Use concrete
owner paths and durable names rather than temporary session/UI labels.
Negative rules belong only where they protect an actual local boundary.

For golden-kernel work, expose the artifact class, source address, layer,
authorship, review posture, uncertainty and lineage. The nearest owner defines
promotion and regeneration. A deterministic or accepted output alone is not a
gold slice; unresolved examples and transfer failure remain inspectable.

## Currentness and verification

[agents_route_inventory.json](docs/validation/agents_route_inventory.json)
is the authored route map. `.agents/agents-route.current.json` records
tracked/discovered parity, inheritance, hashes and context budgets. The native
`route_harness.rs` evaluates declared task paths. These checks establish
structure; model behavior, semantic assessment and time-to-owner need their
own evidence.

[AGENTS_ROOT_REFERENCE.md](docs/AGENTS_ROOT_REFERENCE.md) locates historical
guidance. Active inheritance starts at the current root card. When slimming a
card, retain a unique operative rule at its smallest owner or link its exact
source; historical prose must not become a competing current authority.

Closeout names the changed owners, checks, manual review, skipped coverage,
residual risk and next route. Generated evidence stays subordinate to the
source and review routes it exposes.
