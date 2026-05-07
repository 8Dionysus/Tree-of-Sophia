# Decision Boundary Lenses

Use these lenses when a real decision exists, but its authority, evidence,
placement, or future effect could be confused with nearby surfaces. They are not
an ADR expansion checklist. Pick only the lenses that keep one decision
reviewable.

## How To Use

1. Name the decision in one sentence.
2. Select the few lenses that change where the note belongs or how future
   readers should interpret the decision.
3. For each selected lens, record the boundary: what the ADR decides, what it
   does not decide, and which stronger owner or evidence surface remains
   stronger than the note.
4. Drop any lens that only restates the same decision in different words.

## Lens Set

| Lens | Use When | ADR Question | Failure To Avoid |
|---|---|---|---|
| Decision object | The record may mix several choices or object classes. | Is this one architecture, workflow, tooling, policy, placement, or release decision? | One ADR becomes a bundle of unrelated moves. |
| Owner/source | The rationale touches more than one owner repository, layer, or canonical file. | Who owns the decision, and whose truth is only referenced? | A local ADR imports stronger owner law. |
| Placement | Several decision homes, review notes, route cards, or changelogs could receive the note. | Where will future reviewers look first, and what links back here? | The note is correct but undiscoverable. |
| Evidence state | The choice came from tests, evals, generated reports, receipts, or audit output. | What evidence informed the decision, and what claim does the evidence not prove? | Evidence is promoted into authority or verdict. |
| Workflow/process | The choice came from a task session, checkpoint, closeout, scenario, campaign, or roadmap pass. | Is this a reviewed durable decision or only a process-local hint? | Temporary execution context becomes canon. |
| Lifecycle/time | The note discusses current state, future direction, migration, deprecation, or release posture. | What is decided now, what is deferred, and what is only provenance? | A future trigger reads like an already-landed decision. |
| Portability/overlay | Public reusable core and project-local overlay both shape the choice. | What is portable decision meaning, and what remains local adaptation? | Local paths or ecosystem assumptions leak into portable canon. |
| Runtime/body | The decision affects docs, config, source code, deployment, service state, or observed runtime behavior. | Is the ADR authoring meaning, configuring behavior, or recording why a runtime-facing path was chosen? | Documentation pretends to operate the system. |
| Handoff/fan-out | The decision affects downstream generated surfaces, exports, adapters, or sibling consumers. | Which surfaces must be refreshed or informed, and which remain derived? | Derived outputs become hidden decision records. |
| Risk/approval | The choice affects destructive, public-share, privacy, security, or operational authority. | What approval, guardrail, dry run, or rollback expectation follows from the decision? | Risk is buried inside confident rationale. |
| Scale | The choice could be local, package-level, repo-wide, workspace-wide, or federation-facing. | What is the smallest scope this ADR actually governs? | A local decision is written as universal project law. |
| Lighter artifact | A commit message, PR summary, test, comment, runbook, incident note, or review note may be enough. | What would be lost if no ADR is written? | ADR clutter hides the decisions that matter. |

## Compact Decision-Lens Pass

Use this when a full ADR template would be too heavy but boundary clarity is
still needed.

| Field | Answer |
|---|---|
| Decision |  |
| Selected lenses |  |
| Owner of decision |  |
| Evidence or context only |  |
| Canonical placement |  |
| Does not decide |  |
| Handoff or refresh required |  |
| Future reader should know |  |

## Verification

- exactly one decision is being preserved
- selected lenses change authority, placement, interpretation, or follow-through
- evidence and generated surfaces remain context unless they are the actual
  owner-approved decision surface
- session, checkpoint, roadmap, or scenario language is not stronger than
  reviewed decision evidence
- the note says what it does not decide when a stronger owner or future route
  remains responsible
- a lighter artifact is rejected only when it would lose important rationale
