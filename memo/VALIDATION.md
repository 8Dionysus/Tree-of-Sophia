# Local memo-port validation

Read `PORT.yaml` and the exact changed candidate, receipt or export. Review
its evidence refs, privacy posture, requested route and actual disposition.
Keep `write_candidate_only` distinct from reviewed handoff and durable admission.
The `validators` names in `PORT.yaml` identify consumer-side compatibility
checks; they are not commands executed by ToS.

The native ToS checks verify the declared port and agent routes:

```sh
tos-ops-mechanics-plan --repo-root "$PWD" --agent-surface-validate
tos-ops-mechanics-plan --repo-root "$PWD" --nested-agents-validate
```

These checks do not validate candidate schema, decide memory truth or rebuild
`INDEX.md` / `index.min.json`. Compare those snapshots with the changed packet
membership and use packets directly when they are stale. Report that freshness
explicitly; do not present a historical index as current. Route-card changes
also use `route_docs` in [root validation](../VALIDATION.md).

`aoa-memo` validates and indexes its received copy under its own contract before
reviewed admission. ToS does not fetch or execute that owner's checkout,
validator or builder. A receipt, export or green route check is not admission.
