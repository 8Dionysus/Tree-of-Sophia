# Promotion outcomes

This reference names the allowed outcomes for `aoa-quest-harvest`.

## Allowed outcomes

1. `keep_or_open_quest`
   Use when repetition is still weak, ownership is unclear, or the evidence is not yet honest enough for promotion.

2. `promote_to_skill`
   Use when the repeated unit is a bounded leaf workflow with a stable trigger boundary, explicit inputs and outputs, and a reviewable verification path.

3. `promote_to_playbook`
   Use when the repeated unit is a multi-step route with handoffs, artifacts, and stop or re-entry logic.

4. `promote_to_orchestrator_surface`
   Use when the repeated unit is class law such as read order, boundary note, allowed surfaces, forbidden surfaces, or expected outputs.

5. `promote_to_proof_surface`
   Use when the repeated unit is specifically a proof pattern, verdict posture, rubric law, or another bounded evaluation surface owned by `aoa-evals`.

6. `promote_to_memo_surface`
   Use when the repeated unit is specifically a recurrence, recall, or writeback pattern owned by `aoa-memo`.

## Guardrail

Do not promote an orchestrator class into a skill just because an orchestrator repeatedly performs some work.

Promote only the repeatable leaf workflow, not the class itself.

Do not collapse proof and memo promotions into one shared outcome. They route to different owner layers for different reasons.
