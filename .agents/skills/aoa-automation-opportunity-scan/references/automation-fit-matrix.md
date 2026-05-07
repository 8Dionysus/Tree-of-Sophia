# Automation Fit Matrix

Use this matrix to decide whether a process is actually ready for automation or
only desires automation.

| signal | what strong looks like | what weak looks like | likely consequence |
|---|---|---|---|
| frequency | repeats across sessions or review windows | happened once | weak candidates should usually stay manual or become quests |
| friction | repeated drag, context switching, checklist fatigue, or rote triage | negligible cost | low-friction work rarely justifies early automation |
| determinism | route mostly follows stable rules | route still depends on changing human judgment | weak determinism often means `not_now` or technique extraction first |
| input clarity | canonical sources and triggers are visible | hidden, unstable, or private inputs | unclear inputs block honest automation |
| output clarity | the end state is checkable | success is aesthetic or vague | unclear outputs weaken proof and health checks |
| proof surface | there is a bounded verification seam | results are hard to inspect | weak proof posture means higher false confidence |
| dry run | preview or simulation is possible | only live mutation exists | lack of preview raises risk and approval burden |
| reversibility | rollback marker is obvious | rollback is unclear or expensive | weak rollback pushes toward repair or checkpoint routes |
| secret coupling | little or no secret handling | secret-heavy or environment-bound | high coupling often disqualifies public seed-ready paths |
| approval sensitivity | authority is explicit | authority is hidden or shifting | unclear authority should force checkpoint or defer posture |

## Good first candidates

Good first candidates usually look like:

- recurring audits
- repeated hygiene checks
- bounded report generation
- repeated doc or drift reviews
- repeated preflight or postflight rituals
- repeated session closeout routes

## Bad first candidates

Bad first candidates usually look like:

- one-off creative synthesis
- unstable exploratory research loops
- secret-heavy ops without explicit boundaries
- routes with no rollback or no health check
- routes that silently reshape important system surfaces
