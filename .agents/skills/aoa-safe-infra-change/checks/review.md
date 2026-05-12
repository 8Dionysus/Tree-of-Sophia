# Review Checklist

## Purpose

Use this checklist when reviewing a bounded infrastructure or configuration change that claims to follow `aoa-safe-infra-change`.

## When it applies

- the change touches infrastructure, orchestration, runtime configuration, or operational surfaces
- the task has deployment, safety, or recovery implications
- the review needs to confirm that the change stayed explicit, bounded, and verifiable

## Review checklist

- [ ] The operational surface and the main risk are named before execution.
- [ ] The change remains small and reviewable rather than hiding broader churn.
- [ ] Verification is explicit and proportional to the operational risk.
- [ ] Rollback or recovery thinking is present before execution or recommendation.
- [ ] The final report names unresolved risk, deferred work, or recovery notes.

## Not a fit

- purely local code changes with no operational implications
- tasks where the main question is authority classification or preview-path selection rather than the infra change itself
