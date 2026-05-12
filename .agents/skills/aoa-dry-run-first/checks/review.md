# Review Checklist

## Purpose

Use this checklist when reviewing work that claims to prefer a preview, simulation, or inspect-only path before real execution.

## When it applies

- the task can be previewed before it changes a live or meaningful surface
- a mistaken execution would cost more than a bounded preview step
- the review needs to confirm that preview discipline stayed separate from real execution

## Review checklist

- [ ] A real preview, simulation, or inspect-only path was considered before execution.
- [ ] The review names what the preview covers and what it does not prove.
- [ ] Preview output is not presented as proof of total safety.
- [ ] The preview step did not silently perform the real action.
- [ ] The recommended next step matches the confidence created by the preview.

## Not a fit

- tasks where the central question is whether execution is authorized at all
- tasks that are purely analytical and never approach execution
