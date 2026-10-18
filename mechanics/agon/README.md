# Agon Mechanic

<a id="active-route"></a>
<a id="historical-provenance"></a>

Agon is the ToS-local route for threshold pressure that reaches the philosophy
home without gaining canon-write authority.

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `active` |
| class | `head-fed/local` |
| trigger | Agon, threshold, candidate-only registry, canon-restraint, or handoff pressure touches ToS |
| input | threshold packets, candidate review scope, source-first restraint, owner handoff language |
| output | intake contract, canon-restraint route, candidate-only registry, landing handoff receipt |
| owner | `mechanics/agon/AGENTS.md`, the operation map below, and active part routes |
| stronger route | `ToS/` owns authored philosophy and canon; `Agents-of-Abyss` owns center Agon law |
| next route | active part first; ToS canon review only through ToS-owned authority |
| validation | `tos-ops-mechanics-plan --repo-root ROOT --execute`; `tos-ops-mechanics-plan --repo-root ROOT --mechanics-topology-validate` |

## Operations
Each Agon part owns a functioning operation route and its concrete inputs,
outputs and handoff.


| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Threshold Intake](parts/threshold-intake/README.md) | receive threshold packets as reviewable candidates | `ToS/` source and canon authority |
| [Canon Restraint](parts/canon-restraint/README.md) | keep threshold pressure from becoming direct canon mutation | `ToS/canon/` and source review |
| [Threshold Registry](parts/threshold-registry/README.md) | maintain candidate-only registry config, schema, example, and generated companion | generated surfaces remain downstream |
| [Landing Handoff](parts/landing-handoff/README.md) | account for the Agon landing handoff without turning it into doctrine | `Agents-of-Abyss` center Agon law |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts

- `threshold-intake`: [TOS_AGON_THRESHOLD_INTAKE](parts/threshold-intake/docs/TOS_AGON_THRESHOLD_INTAKE.md)
- `canon-restraint`: [TOS_CANON_RESTRAINT](parts/canon-restraint/docs/TOS_CANON_RESTRAINT.md), [TOS_REJECTION_AND_BRANCHING](parts/canon-restraint/docs/TOS_REJECTION_AND_BRANCHING.md), [TOS_THRESHOLD_REVIEW_BOUNDARY](parts/canon-restraint/docs/TOS_THRESHOLD_REVIEW_BOUNDARY.md)
- `landing-handoff`: [SOPHIAN_THRESHOLD_TOS_LANDING](parts/landing-handoff/docs/SOPHIAN_THRESHOLD_TOS_LANDING.md)

## History

This is the active bridge from current Agon routes to former-path accounting.
Former source material remains recoverable in immutable Git history.

## Legacy Route

Historical old-path accounting is preserved in [TOS-D-0043](../../docs/decisions/TOS-D-0043-spark-and-legacy-scaffolding-retirement.md).

## Distillation Rule

When former Agon material changes current behavior, update the owning active
part first, then this history route with an exact immutable history reference.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/agon/PROVENANCE.md) preserves the pre-compaction route.
Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions
Agon is active in ToS as threshold routing. It receives pressure from the
center or local review, keeps that pressure candidate-shaped, and returns it to
ToS source and canon routes without granting write authority.

### Current Contour

- Keep threshold intake, canon restraint, threshold registry, and landing
  handoff routed through active `parts/`.
- Keep threshold entries candidate-only until ToS source and canon surfaces
  review them.
- Keep Agon-to-ToS handoff language subordinate to the golden route and canon
  owner surfaces.
- Keep former Agon landing names in the history route below; no package-local `legacy/` is admitted.

### Next Work

- Clarify threshold classes only when new repeated ToS threshold pressure
  outgrows the current intake/restraint/registry/handoff parts.
- Keep candidate pressure routed to review before it touches canon, relation
  packs, or public compatibility.
- Keep Table I and later graph-review pressure candidate-shaped when a proposed
  node or relation tries to become authority too early.

### When Time Comes

Time has come when: repeated threshold pressure outgrows the current four
parts.

Move: add a narrower Agon part or refine the threshold registry.

Guardrail: every output remains candidate, review, or handoff posture until
the owning ToS source or canon surface accepts it.

### Out Of Scope

- Live arena protocol.
- Rank mutation.
- Automatic canonization.
- Proof verdicts, runtime activation, SDK behavior, memory truth, or KAG
  substrate authority.
