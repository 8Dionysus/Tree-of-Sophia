# Local eval-port validation

ToS owns local intake, cases, fixtures, suites, reports and their source refs.
Review the changed packet against `PORT.yaml`: its question, exact evidence,
known limits and public-safe contents. Check each referenced source or record
why it is unavailable. Local evidence does not create a central verdict.

The native ToS checks verify the declared port and agent routes:

```sh
tos-ops-mechanics-plan --repo-root "$PWD" --agent-surface-validate
tos-ops-mechanics-plan --repo-root "$PWD" --nested-agents-validate
```

These are structural checks, not a packet-schema validator or a runnable eval.
Select a suite's existing executable check only when that suite supplies one.
Route-card changes also use `route_docs` in [root validation](../VALIDATION.md).

Hand the selected packet and evidence refs to `aoa-evals` when central proof
review is needed. That consumer owns compatibility, scoring and admission in
its own environment. ToS does not fetch or execute its checkout or validator.
