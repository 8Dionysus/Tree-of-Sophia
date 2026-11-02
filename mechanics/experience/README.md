# Experience Mechanic

<a id="active-route"></a>
<a id="historical-provenance"></a>

Experience routes ToS boundary dossiers for adoption, installation, service,
governance, candidate and pattern review, and write guards. The seven families
retain their own schemas, examples and receiving owners under
`mechanics/experience/parts/`. [Mechanics validation](../VALIDATION.md) owns the
checks and [ToS](../../ToS/AGENTS.md) owns authored meaning.

## Mechanic Card

| Field | Route |
| --- | --- |
| status | `active` |
| class | `head-fed/local` |
| trigger | Experience packets touch ToS source, public or review surfaces |
| input | dossiers, guards, review notes and boundary pressure |
| output | reviewable packets, schemas, examples and owner handoffs |
| owner | `mechanics/experience/AGENTS.md` and the operation map below |
| validation | focused Experience tests |

## Operations

| Part | Function | Receiving owner |
| --- | --- | --- |
| [Candidate review](#candidate-review) | candidate dossiers and intake decisions | ToS review and source owners |
| [Adoption boundary](#adoption-boundary) | adoption dossiers and runtime adoption guard | consuming runtime and local owners |
| [Governance boundary](#governance-boundary) | governance dossiers, precedents and review notes | governance owner |
| [Installation boundary](#installation-boundary) | installation dossiers and install-write guard | installation and runtime owners |
| [Service office boundary](#service-office-boundary) | service dossiers and office runtime guard | service and office owners |
| [Pattern review](#pattern-review) | pattern review notes | ToS source review and receiving owner |
| [Write guards](#write-guards) | cross-owner write guards | source, runtime and center owners |

## Shared boundary

ToS receives reviewable packets. Source, canon, installation, runtime, service,
office, governance, proof and memory changes follow their respective owner's
review and acceptance. Part-local schemas and examples retain their individual
contract and compatibility roles.

### Candidate review

<a id="aoa-experience-candidate-intake-boundary"></a>
<a id="tos-candidate-dossier-review"></a>

Candidate review turns cross-repo recurrence into reviewable derived patterns
and ToS dossiers, with explicit inputs, outputs, gates and owner-local landing
hints:

```text
owner-local signals -> cross-repo recurrence -> pattern candidate -> candidate-review gate -> shared pattern registry -> KAG promotion dossier -> ToS candidate dossier boundary -> owner-local adoption review
```

Inputs include owner-local receipts, bounded eval verdicts, runtime and service
incidents, memory retention outcomes and routing owner-landings. Outputs include
candidate records, landing hints, gate decisions, derived summaries and permitted
dossiers. Promotion requires `evidence_refs`, an owner route, a bounded verdict
and an authority check; durable adoption requires retention.

Source repositories own their sources; `ToS/canon/` owns canon admission.
The eval owner supplies proof. `aoa-stats` summarizes recurrence; source owners
and eval review assess meaning and patterns. `aoa-memo` retains bounded lessons
and lineage memories. `aoa-kag` receives derived candidates after evidence,
retention and owner consent gates. Assistant adoption follows release/version
routes; agonic adoption follows scar/retention routes. Codex may collect and
propose candidates; authorized review owners decide candidate-review and KAG
promotion. Candidate review stays in the existing center-aligned program with
owner-local landings. Arena, deployment, Codex, assistants and candidate review follow ToS
source review and canon admission for writes.

### Adoption boundary

<a id="no-runtime-adoption-from-tos"></a>
<a id="tos-adoption-boundary-dossier"></a>

`tos_adoption_boundary_dossier` and `no_runtime_adoption_from_tos` are
candidate-stage ToS surfaces. The lifecycle is `request -> readiness -> shadow -> decision ->
activation -> retention`. Adoption is an explicit consuming-owner decision:
local consent, compatibility, evidence and shadow proof, rollback or explicit
quarantine fallback, retention watch and kind-safe projection are required.
Candidate-review acceptance starts this separate review. Runtime writes and
adoption follow the consuming owner; KAG adoption into a source repository
requires that repository's consent. Routing owners carry routes while source
owners author meaning. Persistent assistant changes use the explicit release and
adoption route.

### Governance boundary

<a id="governance-dossier-intake-boundary"></a>
<a id="governance-precedent-dossier-boundary"></a>
<a id="no-direct-polis-governance-write"></a>
<a id="tos-governance-review-note"></a>

Candidate governance dossiers and precedents are bounded ToS review inputs.
They cite evidence, sealed decisions and replay-valid histories; the local
owner retains meaning and adoption authority.

<a id="governance-decision-refs"></a>

Governance packets cite evidence and decision refs. The flow is: receive a route
signal; validate authority and local consent; apply, hold, reject or escalate
under owner law; emit an audit receipt. Precedent informs local decisions through
consent. Governance and office packets follow ToS review and admission for
source or canon writes.

Codex may collect evidence and propose packets; authorized actors vote, certify,
veto, amend charters and submit ToS writes. Assistant agents may request review;
rechartering and promotion follow their owner routes. Agonic agents may contest,
appeal and receive scars through agonic routes; assistant releases and service
revisions follow their own routes. Councils produce reviewable
decisions; vetoes and emergency stays require explicit authority and expiry.

### Installation boundary

<a id="installation-dossier-boundary"></a>
<a id="no-direct-experience-install-write"></a>

The v1.0.0 installation dossier owns intake, review notes and a canon gate. Its
flow is:

```text
owner-local artifact
  -> validation
  -> operator review
  -> activation or denial
```

The first release pass covers landing, migration, smoke testing, operator
review, the first live assistant office, governed release, rollback drill,
replay audit and post-release watch.
A typed artifact must reach its owner, survive replay and fail closed when
authority is missing. Installation and release artifacts follow review and
lineage before becoming ToS nodes or canon. Runtime writes, arena dust and
assistant canonization follow their respective owners. Cross-layer release
authority lives in [Agents-of-Abyss](https://github.com/8Dionysus/Agents-of-Abyss/blob/main/mechanics/experience/parts/release-deployment/README.md).

### Pattern review

<a id="tos-pattern-review-note"></a>

The candidate-stage `tos_pattern_review_note` reviews long-lived patterns and may inspect candidate
review and adoption outputs. It uses the [adoption lifecycle and consent gates](#adoption-boundary),
including compatibility, shadow proof, rollback, retention watch and kind-safe
projection. The local owner separately decides adoption.

### Service office boundary

<a id="no-runtime-office-write"></a>
<a id="service-dossier-boundary"></a>

The v1.1 Live Office Expansion and Multi-Release Train receives service dossiers
for review. `notary.assistant` is the first receipt-bearing anchor;
`concierge.assistant`, `courier.assistant` and `monitor.assistant` join through
a governed train with compatibility checks, a handoff graph, smoke gates,
rollback, replay audit and operator go/no-go. Office runtime changes follow the
service and office owners, and ToS canon follows its review route. Live office
expansion law belongs to [Agents-of-Abyss](https://github.com/8Dionysus/Agents-of-Abyss/blob/main/mechanics/experience/parts/office-operations/README.md).

### Write guards

<a id="no-direct-arena-or-experience-write"></a>
<a id="no-direct-governance-runtime-write"></a>

Arena, assistant, Codex, cross-repo experience and runtime governance proposals
follow source-owner review, bounded evaluation and canon admission. Governance
dossiers may arrive through the KAG/eval boundary; runtime law remains
reviewable and meaning stays owner-local.

## History

Former Experience placement is recoverable in immutable Git history.
[TOS-D-0043](../../docs/decisions/TOS-D-0043-spark-and-legacy-scaffolding-retirement.md)
owns historical old-path accounting. [Prior provenance](https://github.com/8Dionysus/Tree-of-Sophia/blob/b54cbf8e30ff46aeddeec5c47a5e7aa1f370977d/mechanics/experience/PROVENANCE.md)
records the pre-compaction route.

## Legacy Route

Follow [TOS-D-0043](../../docs/decisions/TOS-D-0043-spark-and-legacy-scaffolding-retirement.md)
for former-path accounting.

## Distillation Rule

Update the owning active part when former Experience material changes current
behavior, then record an exact immutable history reference in its history route.

## Growth conditions

Refine, split or retire a boundary family when recurring payloads make its owner route ambiguous.
