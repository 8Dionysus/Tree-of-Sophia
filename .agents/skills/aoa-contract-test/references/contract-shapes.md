# Contract Shapes

Use this reference when the contract boundary is wider than a simple module or
service interface. It is not a checklist. Pick the smallest shape that names
one producer, one consumer, one stable expectation, and one honest validation
surface.

## How To Use

1. Name the producer and consumer.
2. Name the stable shape or behavior the consumer relies on.
3. Choose the narrowest contract shape below.
4. State what the contract does not prove.
5. Validate the shape with the smallest reviewable check.

## Shape Set

| Shape | Producer | Consumer | Validate | Do Not Claim |
|---|---|---|---|---|
| Module, service, or API | Function, package, service, or endpoint. | Caller, client, downstream service, or workflow. | Input/output shape, errors, status, side effects, compatibility smoke. | Internal implementation details are public contract. |
| CLI or tool report | Command, script, or local tool. | Automation, CI, another script, or operator workflow. | Flags, exit code, machine-readable output, report fields, failure mode. | Human log wording is stable unless explicitly documented. |
| Schema, manifest, or registry | Authored schema, config, registry, or manifest builder. | Validator, router, SDK, generated reader, or release check. | Required fields, version, enum, reference resolution, invalid fixture failure. | Current incidental field order or formatting is semantic truth. |
| Source to generated/export | Source-owned file, builder, or canonical bundle. | Generated catalog, export, adapter, or compact capsule consumer. | Source-to-output field mapping, freshness check, rebuild check, source ref. | Generated output owns the meaning it summarizes. |
| Practice object or handoff | Practice atom, topology, capsule, or practice-to-skill bridge. | Execution skill, routing, eval, retrieval substrate, or downstream practice consumer. | Stable ID, atom shape, topology fields, selected sections, handoff payload. | A practice object is a skill, eval, role, memory object, or scenario. |
| Skill bundle or export | Canonical skill bundle and export builder. | Runtime, SDK, router, pack profile, or installed skill surface. | Metadata, trigger boundary, support refs, export path, trigger eval. | Exported installed copy replaces authored bundle truth. |
| Eval proof or report | Eval bundle, runner, scorer, or verdict emitter. | Review gate, release support, metric summary, or regression reader. | Claim limit, fixture shape, scoring logic, verdict schema, report fields. | One eval proves total quality or intelligence. |
| Role contract or runtime seam | Profile, role contract, projection builder, runtime seam binding. | Scenario, SDK, runtime harness, or handoff consumer. | Role fields, authority limits, handoff payload, projection artifact. | Role text grants hidden runtime authority. |
| Memory recall or writeback | Memory object, recall contract, writeback envelope, or retrieval export. | Router, retrieval substrate, eval, agent, or closeout workflow. | Inspect/capsule/expand shape, source refs, provenance, retention limit. | Recall output is fresh proof or live memory authority by itself. |
| Scenario or reentry | Scenario route, review packet, stress lane, or reentry gate. | Agent, routing, metrics, runtime, or follow-through workflow. | Phase route, handoff, fallback, evidence expectation, reentry fields. | A recurring scenario is a single skill or live runtime ledger. |
| Routing or SDK typed seam | Router hint, owner dispatch seam, SDK facade, CLI report, typed loader. | Agent, script, CI, notebook, or downstream adapter. | Typed fields, owner refs, dispatch result, compatibility result, error report. | Routing or SDK owns the source meaning it points to. |
| Metrics receipt or summary ABI | Receipt publisher, shared envelope, event-kind registry, summary builder. | Metric summary, dashboard, closeout, routing, or review reader. | Envelope fields, event kind, supersession, active view, summary schema. | Summary counts are owner truth or final promotion verdict. |

## Compact Contract Pass

| Field | Answer |
|---|---|
| Producer |  |
| Consumer |  |
| Stable expectation |  |
| Validation surface |  |
| Out of contract |  |
| Downstream impact if broken |  |
| Source owner remains |  |

## Verification

- one producer and one consumer are named
- the stable shape or behavior is observable by the consumer
- the check fails reviewably when the contract is broken
- incidental output and internal implementation details stay out of contract
- derived, generated, or adapter surfaces remain weaker than source owners
