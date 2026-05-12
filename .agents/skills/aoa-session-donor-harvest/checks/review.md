# Review Checklist

## Purpose

Use this checklist when reviewing work that turns a reviewed session artifact
into a bounded `HARVEST_PACKET` with AoA owner-layer candidates and explicit
family handoff hints.

## When it applies

- a reviewed session, transcript, or compaction packet is being distilled after the fact
- the reviewer must check whether reusable units belong in `aoa-techniques`, `aoa-skills`, `aoa-playbooks`, `aoa-evals`, `aoa-memo`, `aoa-agents`, or should remain deferred
- the route risks collapsing source-owned meaning into derivative layers or vague "good ideas"
- the result may also need explicit automation scan, route-forks, diagnosis, progression, or
  quest-triage follow-through

## Review checklist

- [ ] The source artifact was reviewed and bounded before harvest.
- [ ] Checkpoint notes, closeout handoffs, and ledger hints were dispositioned
      before harvest as accepted, rejected, stale, cross-session,
      contaminated, or unresolved.
- [ ] No `candidate_ref` was minted from a checkpoint or handoff hint unless
      the reviewed artifact or receipt evidence confirmed the same reusable
      unit.
- [ ] Each kept candidate names one reusable unit rather than one topic cluster.
- [ ] Each candidate has one primary owner layer.
- [ ] Each accepted candidate minted `candidate_ref` only after reviewed donor harvest.
- [ ] Any carried `cluster_ref` stayed linked instead of being treated as final object identity.
- [ ] Stale, neighboring-session, or diagnostic residue stayed out of the
      current session's accepted donor candidates.
- [ ] The output names the nearest wrong target and rejects it explicitly.
- [ ] `aoa-routing` and `aoa-kag` were not treated as first authoring targets for source-owned meaning.
- [ ] `usefulness` was treated as a reuse signal, not as an owner layer.
- [ ] Weak or unclear candidates were allowed to remain `hold` instead of being forced into canon.
- [ ] The result names one concrete next artifact for each accepted candidate.
- [ ] Automation candidates, quest residue, route forks, diagnosis hints, repair follow-through, or progression follow-through were surfaced explicitly when they survived the harvest.
- [ ] Any `HARVEST_PACKET_RECEIPT` stayed evidence-linked, append-only, and smaller than the packet it summarizes.
- [ ] Any `CORE_SKILL_APPLICATION_RECEIPT` stayed generic, finish-only, and pointed back to the bounded detail receipt instead of duplicating packet meaning.

## Not a fit

- active sessions that still need execution or review
- raw session capture, transcript export, or local indexing work
- narrow final promotion triage for one repeated quest unit where `aoa-quest-harvest` is already the honest next step
