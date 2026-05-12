# Runtime Example

## Scenario

A reviewed session repeatedly mixed technique language with skill language,
lost proof boundaries during recap, and kept reopening the same ownership
confusion in follow-up steps.

## Why this skill fits

The next honest move is not immediate repair or promotion.
It is a bounded read-only diagnosis pass that separates symptoms from probable
causes and names the likely owner layers involved.

## Expected inputs

- reviewed session artifact or harvest packet
- observed frictions, contradictions, or failures
- touched owner layers and repos
- any earlier related evidence if available
- checkpoint, closeout, generated, or earlier-session hints marked as hints

## Expected outputs

- one `DIAGNOSIS_PACKET`
- named drift types, symptoms, probable causes, and repair shapes
- evidence posture for each meaningful symptom and probable cause
- owner hints and explicit unknowns
- optional handoff to `aoa-session-self-repair`
- one `DIAGNOSIS_PACKET_RECEIPT` that records diagnosis types, confidence
  band, and evidence-linked owner hints
- one `CORE_SKILL_APPLICATION_RECEIPT` that records the finished
  `aoa-session-self-diagnose` run and points back to the detail receipt

## Boundary notes

- Do not use this skill for live-session debugging.
- Do not perform the repair inside the diagnosis pass.
- Do not turn one anecdote into structural certainty.

## Verification notes

- Confirm the diagnosis cites evidence.
- Confirm symptoms and causes are separated.
- Confirm likely causes do not outrun their evidence posture.
- Confirm unknowns stay visible where evidence is weak.
- Confirm the finish receipt stays smaller than the diagnosis packet and does
  not read like a final repair verdict.
- Confirm the generic core receipt points back to the diagnosis receipt and
  does not claim repair or proof authority.
