# Review Checklist

## Purpose

Use this checklist when reviewing work that turns one reviewed session into a
bounded `DIAGNOSIS_PACKET`.

## When it applies

- the route needs diagnosis before repair
- repeated friction, contradiction, proof debt, or boundary drift survived the
  reviewed session
- the reviewer must check whether symptoms, causes, and owner hints stayed
  separate and honest

## Review checklist

- [ ] The source artifact was reviewed and bounded before diagnosis.
- [ ] Each diagnosis cites concrete evidence refs.
- [ ] Symptoms and probable causes stay separate.
- [ ] Probable causes remain probabilistic when evidence is thin.
- [ ] Each meaningful symptom and probable cause carries an evidence posture such as reviewed symptom, reviewed inference, provisional hint, contested evidence, stale evidence, or unknown.
- [ ] Checkpoint, closeout, generated, or stale hints were not treated as settled diagnosis evidence.
- [ ] A likely owner layer is named without pretending the verdict is already final.
- [ ] Unknowns are preserved where evidence is incomplete.
- [ ] No hidden mutation or silent repair happened.
- [ ] Cross-layer issues are not lazily blamed on one convenient owner.
- [ ] Any `DIAGNOSIS_PACKET_RECEIPT` stayed evidence-linked and descriptive rather than blame-shaped.
- [ ] Any `CORE_SKILL_APPLICATION_RECEIPT` pointed to the detail diagnosis receipt and stayed generic finish telemetry only.

## Not a fit

- live or unreviewed sessions
- issues that are already fully diagnosed and only need repair execution
- single promotion verdict questions that belong to `aoa-quest-harvest`
