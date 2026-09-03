# AGENTS.md

## Applies to

This card applies to `memo/`.

## Role

`memo/` is the Tree-of-Sophia local memory port. It holds tree-local memory
candidates, receipts, exports, and local notes before reviewed landing in
`aoa-memo`.

## Conditional route

Start with root `AGENTS.md` and this card. Open `CHARTER.md`, `BOUNDARIES.md`,
ToS doctrine, or `README.md` when the selected candidate question needs their
human orientation; open `PORT.yaml` and the exact candidate, receipt, or export
surface when the route is known. Consult `aoa-memo` operation contracts only
when a candidate should move centrally.

## Boundaries

Use this port for `write_candidate_only` work. Keep source-linked knowledge and
tree meaning in Tree-of-Sophia source surfaces; use this port for recall,
candidate memory, receipts, and reviewed handoff.

Use `PORT.yaml` for the local port contract and `INDEX.md` / `index.min.json`
as generated read models. Use `candidates/` for proposed memory, `receipts/`
for review or handoff traces, `exports/` for packets meant for `aoa-memo`, and
`local/` for tree-local memory that stays local for now.

## Candidate Route

When a reviewed signal warrants a tree-local candidate, use the
`aoa-memo` candidate-intake procedure named by [`memo/VALIDATION.md`](VALIDATION.md);
bind evidence refs and keep output local until reviewed. Validate the emitted
candidate through the same owner route before any reviewed handoff.

## Reviewed Landing Route

For a reviewed export, select the `memo/VALIDATION.md` landing route and keep
the `aoa-memo` intake, generated-index, and review boundaries there. A landing
plan is an access-plane check; durable memory still requires reviewed intake.

`landing-plan` is an access-plane check. Durable memory lands only in
`aoa-memo` through reviewed intake, generated read models, validators, and
review.

## Validation

Use [`memo/VALIDATION.md`](VALIDATION.md) after the local memo operation and
intended `aoa-memo` checkout are known. That route retains the external
procedure and generated-index side-effect boundary.

For repo-wide release posture, use the root `AGENTS.md` validation route.

## Closeout

Report candidate path, evidence refs, validation result, and whether the item
stayed local, was exported for reviewed intake, or was landed in `aoa-memo`.
