# Review Checklist

## Purpose

Use this checklist when reviewing work that detects automation-ready routes
from reviewed evidence and packages them into a bounded
`AUTOMATION_OPPORTUNITY_PACKET`.

## When it applies

- a reviewed session or recurring project slice is being scanned for honest automation value
- the reviewer must check whether the route should stay manual, become a bounded skill, become a playbook automation seed candidate, or stay deferred
- the route may touch approval, rollback, or self-change posture
- the output must stay detection-shaped rather than scheduler-shaped

## Review checklist

- [ ] The candidate route is reviewed, real, and currently manual rather than speculative.
- [ ] Repeat signal and friction are evidenced rather than asserted from enthusiasm.
- [ ] Input clarity, output clarity, proof surface, reversibility, and approval sensitivity were assessed explicitly.
- [ ] Each candidate received an explicit `seed_ready` or `not_now` verdict.
- [ ] Each candidate named an `automation_mode_posture` that is no stronger than the evidence supports.
- [ ] The next artifact and likely owner layer were named.
- [ ] The nearest wrong target was rejected explicitly.
- [ ] `checkpoint_required` was raised for self-changing, approval-heavy, or important mutation routes.
- [ ] No schedule hint was presented as runtime authority.
- [ ] Any `AUTOMATION_CANDIDATE_RECEIPT` stayed evidence-linked, append-only, and detector-shaped.
- [ ] Any `CORE_SKILL_APPLICATION_RECEIPT` stayed finish-only, pointed to the detail receipt, and did not widen into scheduler authority.

## Not a fit

- one-off creative or exploratory work
- live scheduling, autonomous background execution, or secret-bearing ops
- routes that still need donor harvest or source-of-truth clarification first
