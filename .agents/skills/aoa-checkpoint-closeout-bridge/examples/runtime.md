# Runtime Example

## Scenario

A session accumulated checkpoint-led route and progression hints during several
reviewable commits. At reviewed closeout the operator wants one explicit bridge
that reuses those hints, rereads the full reviewed artifact, and then executes
the core session-growth chain in order.

## Why this skill fits

The route needs one orchestrator layer that stays honest about checkpoint
authority. Mid-session capture should remain provisional, while reviewed
closeout should explicitly drive donor harvest, progression lift, and quest
triage in sequence.

## Expected inputs

- one reviewed session artifact
- one optional local checkpoint note
- one optional reviewed surface closeout handoff
- any staged or published session receipts that sharpen the reread
- repo scope or touched owner hints if they help bound the closeout route

## Expected outputs

- one closeout context bundle
- one ordered reviewed-closeout execution chain
- accepted, rejected, and unresolved focus hints before the chain starts
- one status per downstream stage: `executed`, `skipped`, `deferred`, or
  `stopped`
- one execution report naming the emitted artifacts and receipts

## Boundary notes

- Do not use this skill to mint final verdicts from checkpoint notes alone.
- Do not hide it inside `aoa closeout run`.
- Do not force a downstream verdict when the reviewed evidence only supports a
  skipped or deferred stage.
- Do not refresh stats before the explicit closeout chain finishes.

## Verification notes

- Confirm checkpoint collection only updates provisional local ledger surfaces.
- Confirm the reviewed artifact is reread during donor harvest, progression
  lift, and quest harvest.
- Confirm the execution order stays donor -> progression -> quest.
- Confirm stage statuses are visible and evidence-linked.
- Confirm checkpoint note and closeout handoff act as shortlist hints rather
  than sole evidence.
- Confirm stats refresh remains downstream of the bridge.
