# ToS Agent-Surface Design

## Role

This document names how agent-facing guidance should look inside
`Tree-of-Sophia`.

An agent card is a local route surface: role, input, output, owner, next route,
and verification.

## Thesis

Agents work better in ToS when each card tells them where they are in the tree,
what authority they are touching, and where to move next without flattening
source, branch, doctrine, mechanics, and generated companions into one pile.

## Card Shape

Use this shape when a substantial local `AGENTS.md` is needed:

| Field | Meaning |
| --- | --- |
| role | what this surface owns |
| input | what kind of pressure arrives here |
| output | what this surface may produce |
| owner | the source, branch, mechanic, or route that keeps authority |
| next route | where the agent goes after reading this card |
| verification | the smallest useful check or review surface |

Add local details only where they change action.

## Agent Topology

| Surface | Function |
| --- | --- |
| root `AGENTS.md` | repository route law and cross-surface orientation |
| `ToS/AGENTS.md` | source-home route law for philosophical growth |
| branch cards | local branch, doctrine, canon, review, or public-route guidance |
| mechanics cards | operation-package and part-local machine law |
| scripts/tests cards | deterministic check ownership |
| docs cards | decision and documentation route law |

The package-level model-facing route and owner-port map lives in
[.agents/README](.agents/README.md), with its authored machine companion at
[.agents/agent-surface.manifest.json](.agents/agent-surface.manifest.json).
Those surfaces describe progressive loading and handoff; they do not replace
the nearer card or a stronger AoA owner.

The nearer card owns local risk. The root card owns repository identity and the
route back to stronger owners.

## Movement

Good agent movement in ToS:

1. read root and nearest local card;
2. name the owner surface before editing;
3. distinguish authored source, doctrine, canon, review, mechanic, generated,
   and export surfaces;
4. make the smallest coherent change;
5. run the narrowest relevant check or manual review route;
6. report changed surfaces, validation, skipped checks, residual risk, and the
   next route.

## Golden-Kernel Work Cards

Agent guidance for a Zarathustra gold slice must be operational rather than
merely inspirational. The nearest card should let the agent determine:

| Field | Required answer |
| --- | --- |
| artifact class | witness, doctrine, contract, candidate, review receipt, canon object, mirror, or derived output |
| authority input | exact source and stable address from which work begins |
| output posture | observation, proposal, accepted, rejected, deferred, ambiguous, or derived |
| authorship | what a human authored, what an agent proposed, and what remains unreviewed |
| owner handoff | the next concrete owner path and the condition for entering it |
| regeneration | which public, graph, KAG, or compact companions move only after authored input changes |
| transfer probe | how unseen material may reject the vocabulary or expose a missing contract distinction |

Cards must not call a payload gold merely because it is deterministic or
accepted. Source anchors, layer posture, review rationale, uncertainty,
negative or unresolved examples, and version lineage are part of the
engineering route.

## Writing Law

- prefer route cards over essays;
- name owners with concrete paths;
- keep commands in route-law or validation surfaces, not everywhere;
- avoid old session labels as durable topology;
- write what to do and where to go next;
- keep negative rules only when a real boundary cannot be expressed as a
  positive route.

## Bad Signals

- long inherited prohibition lists;
- generic wisdom without owner paths;
- validation commands copied into every surface;
- route names from temporary UI or session labels;
- generated files, scripts, or validators made stronger than source-owned
  meaning.

## Relationship to DESIGN

[DESIGN](DESIGN.md) names the form of the tree. This document names the form of
the agent surfaces that help agents move through the tree.

When they conflict, preserve source authority and route to the nearest owner.
