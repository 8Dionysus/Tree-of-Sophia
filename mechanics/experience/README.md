# Experience Mechanic

<a id="active-route"></a>
<a id="historical-provenance"></a>

Experience is the ToS-local route for boundary packets that touch adoption,
installation, service, governance, candidate review, pattern review, or write
guards without becoming runtime or canon authority.

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `active` |
| class | `head-fed/local` |
| trigger | Experience boundary payloads touch ToS source, public, or review surfaces |
| input | dossiers, guard packets, review notes, and no-runtime/no-office boundary pressure |
| output | reviewable boundary docs, schemas, examples, and stronger-owner handoffs |
| owner | `mechanics/experience/AGENTS.md`, the operation map below, and active part routes |
| stronger route | `ToS/` for authored meaning; owner repos for runtime, proof, memory, SDK, service, office, and governance activation |
| next route | owning Experience part first; stronger owner when activation is requested |
| validation | focused Experience tests plus `tos-ops-mechanics-plan --repo-root ROOT --mechanics-topology-validate` |

## Operations
Each part owns one boundary operation family.


| Part | Function | Stronger owner route |
| --- | --- | --- |
| [Candidate Review](parts/candidate-review/README.md) | candidate dossiers and intake decisions | `ToS/` review and source authority |
| [Adoption Boundary](parts/adoption-boundary/README.md) | adoption dossiers and no-runtime adoption guard | runtime and owner repos |
| [Governance Boundary](parts/governance-boundary/README.md) | governance dossiers, precedents, and review notes | governance owner surfaces |
| [Installation Boundary](parts/installation-boundary/README.md) | installation dossiers and install-write guard | installation and runtime owners |
| [Service Office Boundary](parts/service-office-boundary/README.md) | service dossiers and office runtime guard | service and office owners |
| [Pattern Review](parts/pattern-review/README.md) | pattern review notes | ToS source review and stronger owner handoff |
| [Write Guards](parts/write-guards/README.md) | generic no-direct-write guard surfaces | source, runtime, and center owners stay stronger |

The [local card](AGENTS.md) owns editing; [validation](../VALIDATION.md) owns checks.
Source or semantic questions return to [the source owner](../../ToS/AGENTS.md).

### Direct contracts

- `candidate-review`: [AOA_EXPERIENCE_CANDIDATE_INTAKE_BOUNDARY](parts/candidate-review/docs/AOA_EXPERIENCE_CANDIDATE_INTAKE_BOUNDARY.md), [TOS_CANDIDATE_DOSSIER_REVIEW](parts/candidate-review/docs/TOS_CANDIDATE_DOSSIER_REVIEW.md)
- `adoption-boundary`: [NO_RUNTIME_ADOPTION_FROM_TOS](parts/adoption-boundary/docs/NO_RUNTIME_ADOPTION_FROM_TOS.md), [TOS_ADOPTION_BOUNDARY_DOSSIER](parts/adoption-boundary/docs/TOS_ADOPTION_BOUNDARY_DOSSIER.md)
- `governance-boundary`: [GOVERNANCE_DOSSIER_INTAKE_BOUNDARY](parts/governance-boundary/docs/GOVERNANCE_DOSSIER_INTAKE_BOUNDARY.md), [GOVERNANCE_PRECEDENT_DOSSIER_BOUNDARY](parts/governance-boundary/docs/GOVERNANCE_PRECEDENT_DOSSIER_BOUNDARY.md), [NO_DIRECT_POLIS_GOVERNANCE_WRITE](parts/governance-boundary/docs/NO_DIRECT_POLIS_GOVERNANCE_WRITE.md), [TOS_GOVERNANCE_REVIEW_NOTE](parts/governance-boundary/docs/TOS_GOVERNANCE_REVIEW_NOTE.md)
- `installation-boundary`: [INSTALLATION_DOSSIER_BOUNDARY](parts/installation-boundary/docs/INSTALLATION_DOSSIER_BOUNDARY.md), [NO_DIRECT_EXPERIENCE_INSTALL_WRITE](parts/installation-boundary/docs/NO_DIRECT_EXPERIENCE_INSTALL_WRITE.md)
- `service-office-boundary`: [NO_RUNTIME_OFFICE_WRITE](parts/service-office-boundary/docs/NO_RUNTIME_OFFICE_WRITE.md), [SERVICE_DOSSIER_BOUNDARY](parts/service-office-boundary/docs/SERVICE_DOSSIER_BOUNDARY.md)
- `pattern-review`: [TOS_PATTERN_REVIEW_NOTE](parts/pattern-review/docs/TOS_PATTERN_REVIEW_NOTE.md)
- `write-guards`: [NO_DIRECT_ARENA_OR_EXPERIENCE_WRITE](parts/write-guards/docs/NO_DIRECT_ARENA_OR_EXPERIENCE_WRITE.md), [NO_DIRECT_GOVERNANCE_RUNTIME_WRITE](parts/write-guards/docs/NO_DIRECT_GOVERNANCE_RUNTIME_WRITE.md)

## Shared boundary

The seven packet families remain distinct because their dossier schemas,
examples and receiving owners differ. They share one operation boundary:
review-only ToS packets cannot activate installation, adoption, governance,
service, office, runtime, proof, memory or canon. Their actual receiving owner
must accept any further action. Local checks route through
[mechanics validation](../VALIDATION.md); packet payload bytes remain unchanged.

### adoption-boundary

Adoption Boundary: keep adoption packets reviewable without runtime adoption.

Input: adoption dossier and no-runtime adoption guard. Output: adoption boundary packet.
Next owner: runtime or owner repo only after explicit owner acceptance.
The payload remains in `mechanics/experience/parts/adoption-boundary/`: [NO_RUNTIME_ADOPTION_FROM_TOS](parts/adoption-boundary/docs/NO_RUNTIME_ADOPTION_FROM_TOS.md), [TOS_ADOPTION_BOUNDARY_DOSSIER](parts/adoption-boundary/docs/TOS_ADOPTION_BOUNDARY_DOSSIER.md).

### candidate-review

Candidate Review: hold Experience candidate dossiers as review-only ToS material.

Input: candidate dossier, intake decision, review scope. Output: candidate review packet, no direct canon or runtime effect.
Next owner: `ToS/` review or stronger owner handoff.
The payload remains in `mechanics/experience/parts/candidate-review/`: [AOA_EXPERIENCE_CANDIDATE_INTAKE_BOUNDARY](parts/candidate-review/docs/AOA_EXPERIENCE_CANDIDATE_INTAKE_BOUNDARY.md), [TOS_CANDIDATE_DOSSIER_REVIEW](parts/candidate-review/docs/TOS_CANDIDATE_DOSSIER_REVIEW.md).

### governance-boundary

Governance Boundary: keep governance dossiers and precedents in review posture.

Input: governance dossier, precedent, review note, governance write risk. Output: governance boundary packet.
Next owner: governance owner, not ToS canon mutation.
The payload remains in `mechanics/experience/parts/governance-boundary/`: [GOVERNANCE_DOSSIER_INTAKE_BOUNDARY](parts/governance-boundary/docs/GOVERNANCE_DOSSIER_INTAKE_BOUNDARY.md), [GOVERNANCE_PRECEDENT_DOSSIER_BOUNDARY](parts/governance-boundary/docs/GOVERNANCE_PRECEDENT_DOSSIER_BOUNDARY.md), [NO_DIRECT_POLIS_GOVERNANCE_WRITE](parts/governance-boundary/docs/NO_DIRECT_POLIS_GOVERNANCE_WRITE.md), [TOS_GOVERNANCE_REVIEW_NOTE](parts/governance-boundary/docs/TOS_GOVERNANCE_REVIEW_NOTE.md).

### installation-boundary

Installation Boundary: keep installation dossiers from becoming runtime or canon writes.

Input: installation dossier and install-write guard. Output: installation boundary packet.
Next owner: installation or runtime owner.
The payload remains in `mechanics/experience/parts/installation-boundary/`: [INSTALLATION_DOSSIER_BOUNDARY](parts/installation-boundary/docs/INSTALLATION_DOSSIER_BOUNDARY.md), [NO_DIRECT_EXPERIENCE_INSTALL_WRITE](parts/installation-boundary/docs/NO_DIRECT_EXPERIENCE_INSTALL_WRITE.md).

### pattern-review

Pattern Review: hold pattern review notes without turning patterns into canon.

Input: pattern proposal and source-review state. Output: review note and return route.
Next owner: `ToS/` review or stronger owner handoff.
The payload remains in `mechanics/experience/parts/pattern-review/`: [TOS_PATTERN_REVIEW_NOTE](parts/pattern-review/docs/TOS_PATTERN_REVIEW_NOTE.md).

### service-office-boundary

Service Office Boundary: keep service and office packets out of direct ToS writes.

Input: service dossier, office runtime guard. Output: service-office boundary packet.
Next owner: service, office, or runtime owner.
The payload remains in `mechanics/experience/parts/service-office-boundary/`: [NO_RUNTIME_OFFICE_WRITE](parts/service-office-boundary/docs/NO_RUNTIME_OFFICE_WRITE.md), [SERVICE_DOSSIER_BOUNDARY](parts/service-office-boundary/docs/SERVICE_DOSSIER_BOUNDARY.md).

### write-guards

Write Guards: prevent direct cross-owner writes into ToS canon or runtime routes.

Input: no-direct-write risk, center/runtime pressure. Output: guard contract and example.
Next owner: owning source or runtime layer.
The payload remains in `mechanics/experience/parts/write-guards/`: [NO_DIRECT_ARENA_OR_EXPERIENCE_WRITE](parts/write-guards/docs/NO_DIRECT_ARENA_OR_EXPERIENCE_WRITE.md), [NO_DIRECT_GOVERNANCE_RUNTIME_WRITE](parts/write-guards/docs/NO_DIRECT_GOVERNANCE_RUNTIME_WRITE.md).

## History

This is the active bridge from current Experience routes to former-path
accounting. Former source material remains recoverable in immutable Git history.

## Legacy Route

Historical old-path accounting is preserved in [TOS-D-0043](../../docs/decisions/TOS-D-0043-spark-and-legacy-scaffolding-retirement.md).

## Distillation Rule

When former Experience material changes current behavior, update the owning
active part first, then this history route with an exact immutable history reference.

[Exact prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/experience/PROVENANCE.md) preserves the pre-compaction route.
Former `PARTS.md`, `PROVENANCE.md` and `ROADMAP.md` remain narrow compatibility
entries for known local and unenumerated external readers. Their contents are
owned here; they are not required components of a new mechanic.

## Growth conditions
Experience is active as boundary routing only. It keeps external adoption,
installation, governance, service, pattern, and write pressure from turning ToS
into runtime, office, or federation authority.

### Current Contour

- Keep candidate review, adoption boundary, governance boundary, installation
  boundary, service-office boundary, pattern review, and write guards routed
  through active parts.
- Keep each boundary family tied to the stronger owner that would receive any
  activation, proof, service, office, or runtime claim.
- Keep ToS-owned meaning and canon outside Experience.
- Keep public graph and atlas review experience bounded to route clarity, not
  activation claims.

### Next Work

- Tighten a boundary family only when repeated payloads make its owner split or
  allowed output ambiguous.
- Route any activation claim to the stronger owner before widening local ToS
  mechanics.

### When Time Comes

Time has come when: a boundary family develops repeated payloads that make the
current owner route ambiguous.

Move: refine that part, add a narrower part, or retire a part whose pressure no
longer recurs.

Guardrail: Experience may clarify adoption or interaction pressure; it may not
activate runtime, office, proof, memory, or canon authority.

### Out Of Scope

- Live workspace runtime.
- Office installation or service dispatch.
- Proof verdicts, memory objects, SDK helpers, or AoA federation law.
- Direct ToS canon mutation.
