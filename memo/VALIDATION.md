# Local memo-port validation

Read `PORT.yaml` and the exact changed candidate, receipt or export. Review
its evidence refs, privacy posture, requested route and actual disposition.
Keep `write_candidate_only` distinct from reviewed handoff and durable admission.
The `validators` names in `PORT.yaml` identify consumer-side compatibility
checks; they are not commands executed by ToS.

Review does not rebuild
`INDEX.md` / `index.min.json`. Compare those snapshots with the changed packet
membership and use packets directly when they are stale. Report that freshness
explicitly; do not present a historical index as current.

`aoa-memo` validates and indexes its received copy under its own contract before
reviewed admission. ToS does not fetch or execute that owner's checkout,
validator or builder. A receipt, export or green route check is not admission.
