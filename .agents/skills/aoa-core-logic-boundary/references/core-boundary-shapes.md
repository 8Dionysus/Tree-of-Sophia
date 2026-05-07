# Core Boundary Shapes

Use this reference when the core-versus-glue question is wider than one code
module. It is not a checklist. Pick the smallest shape that names one reusable
center, one edge context, one stop-line, and one verification surface.

## How To Use

1. Name the already-understood context or owner surface.
2. Name the reusable rule, mapping, decision, or behavior candidate.
3. Name the glue, projection, rendering, adapter, runtime, or delivery detail.
4. Choose the narrowest shape below.
5. Verify that future changes can update the center without dragging edge
   detail, and edge detail without silently changing the center.

## Shape Set

| Shape | Reusable Center | Edge Or Glue | Do Not Promote | Verify |
|---|---|---|---|---|
| Code module or service slice | Stable rule, calculation, policy, state transition, or selection logic. | Request parsing, retries, logging, persistence, transport, scheduling, environment paths. | Delivery mechanics as domain truth. | Core tests stay small; edge behavior remains covered separately. |
| Skill bundle | Portable workflow, trigger boundary, contracts, risks, verification posture. | Installed export, profile metadata, pack selection, local overlay wording, runtime discovery. | Exported copy or router hint as authored skill truth. | Authored bundle remains source; generated/export refresh is deterministic. |
| Practice object | Atomic move, invariant posture, reusable proof or execution pattern. | Origin notes, examples, promotion notes, topology metadata, skill bridge wording. | Practice pattern as skill, scenario, eval, role, or runtime law. | Core move is stable; support notes stay explanatory. |
| Eval bundle | Scoring rule, proof contract, claim limit, verdict semantics. | Fixture loading, report layout, markdown rendering, runner plumbing. | Polished report text as proof logic. | A changed score rule is distinguishable from report rendering churn. |
| Role contract | Role contract, authority limits, handoff posture, collaboration mode. | Prompt projection, model-tier routing, runtime binding, UI labels. | Runtime projection as hidden role authority. | Role source remains reviewable apart from projection glue. |
| Memory, recall, or provenance surface | Recall rule, writeback envelope, provenance expectation, retention boundary. | Storage adapter, vector index, cache, search ranking, retrieval export plumbing. | Retrieved text or adapter cache as live memory authority. | Provenance and retention rules survive storage changes. |
| Scenario or playbook | Recurring route shape, phase order, fallback rule, evidence expectation. | Real-run notes, execution logs, scheduler hooks, orchestration scripts. | Scenario recipe as hidden runtime engine. | Scenario can be reviewed without replaying local runtime behavior. |
| Routing or SDK seam | Typed loader rule, owner dispatch decision, compatibility posture, facade contract. | CLI flags, report formatting, local path discovery, adapter wrappers. | Router or SDK as owner of the meaning it points to. | Typed behavior stays stable while presentation or local wrappers change. |
| Metrics or receipt surface | Event envelope, receipt semantics, supersession rule, active-view contract. | Dashboard rendering, summary prose, aggregation display, export packaging. | Counts or charts as final owner verdict. | Envelope and verdict limits remain clear under report changes. |
| Generated or export builder | Source-to-output mapping rule, freshness check, source reference preservation. | Generated artifact bytes, compact formatting, sort order, install path. | Generated output as source-owned meaning. | Rebuild proves mapping; source remains stronger than output. |
| Mechanics or process docs | Local mechanic contract, part boundary, roadmap rule, landing/provenance relation. | Legacy notes, logs, status prose, migration staging, folder ceremony. | Roadmap or provenance notes as active mechanic contract. | Active contract points to nearby docs without dragging legacy language. |
| Session or workflow surface | Stable phase order, stop condition, review gate, handoff requirement. | Checkpoint note, shell command, transcript detail, one-session local evidence. | A single session trace as universal workflow law. | The reusable phase survives while local evidence stays evidence. |

## Compact Core Boundary Pass

| Field | Answer |
|---|---|
| Context already understood |  |
| Reusable center |  |
| Edge or glue |  |
| Source owner |  |
| Derived or consumer surfaces |  |
| Stop-line |  |
| Verification surface |  |
| Future update rule |  |

## Verification

- the center is stable enough to reuse, test, or review independently
- edge detail is still allowed to vary without changing the center
- source-owned meaning stays stronger than generated, exported, adapter, or
  presentation surfaces
- the split reduces future review confusion rather than only renaming folders
- if the source owner or context is still unclear, stop and use
  `aoa-bounded-context-map` first
- if the concrete dependency seam is already clear, stop and use
  `aoa-port-adapter-refactor`
