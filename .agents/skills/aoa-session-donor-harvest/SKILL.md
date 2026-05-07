---
name: aoa-session-donor-harvest
description: Turn a reviewed session artifact into a bounded HARVEST_PACKET, route each reusable unit to the right AoA owner layer, and hand off to the next honest post-session skill when needed. Use when the source is a reviewed session transcript, compaction note, or recap and the real question is what should survive this session rather than merely what happened. Do not use for active or unreviewed sessions, raw session capture or indexing work, or when the only remaining object is already one repeated quest unit that just needs `aoa-quest-harvest` for the final promotion verdict.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/core/session-growth/aoa-session-donor-harvest/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0075,AOA-T-0076,AOA-T-0077
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-session-donor-harvest

## Intent
Use this skill to metabolize a reviewed session artifact into a bounded
`HARVEST_PACKET` that names reusable units, routes each one to the right AoA
owner layer, and drafts the next honest artifact without forcing promotion.

## Trigger boundary
Use this skill when:
- a session transcript, compaction note, review packet, or bounded recap exists and must be distilled into reusable donor units
- the work is post-session and reviewable rather than live execution
- candidate outputs may belong in technique canon, `aoa-skills`, `aoa-playbooks`, `aoa-evals`, `aoa-memo`, `aoa-agents`, or a hold/quest lane
- a repeated manual route may also need an explicit automation-readiness seam instead of staying vague donor residue
- the session may also have produced decision forks, diagnosis clues, repair candidates, progression evidence, or quest residue that should be made explicit rather than buried in recap prose
- checkpoint notes, closeout handoffs, or other lower-authority hints need to
  be accepted, rejected, or carried without becoming candidates on their own
- the question is not merely "what happened?" but "what reusable object, if any, emerged here?"

Do not use this skill when:
- the session is still active or unreviewed
- the task is raw session capture, transcript export, or local indexing; those are history techniques, not this skill
- the result is clearly one bounded repeated quest unit that only needs the narrower final promotion verdict from `aoa-quest-harvest`
- the material is only a progress log, emotional recap, or theme cloud with no bounded reusable unit
- the intended first destination is `aoa-routing` or `aoa-kag`; those are derivative layers and should not receive source-owned meaning first

## Inputs
- reviewed session artifact
- session goal and closure state
- candidate repeat signals or reuse signals
- touched repos or AoA layers
- optional checkpoint `cluster_ref` values, closeout-handoff hints, or other
  pre-harvest focus hints
- current-session evidence boundary, especially when hints came from a ledger
  that may also contain stale or neighboring-session residue
- explicit uncertainty and boundary risks
- desired output posture: classify-only, draft-stub, or patch-ready proposal

## Outputs
- one bounded `HARVEST_PACKET`
- one reviewed intake note that says which checkpoint or handoff hints were
  accepted, rejected, or carried as unresolved focus only
- named candidates, each with:
  - candidate ref minted only after reviewed harvest
  - source cluster ref when the reviewed input carried one
  - reusable unit name
  - unit kind: pattern, mechanic, utility, law, proof, recall, or route
  - owner shape: technique, skill, playbook, eval, memo, agent, or hold
  - owner hypothesis and owner repo recommendation
  - one chosen next artifact
  - one rejected nearest-wrong target
  - status posture plus any supersedes, merged-into, or drop-reason carry
  - evidence anchors from the session artifact
- one short list of items to defer, drop, or keep as quest residue
- one optional `automation_candidate` extract when a repeated manual route is
  stable enough to deserve explicit automation-readiness classification
- one optional handoff list to `aoa-automation-opportunity-scan`,
  `aoa-session-route-forks`,
  `aoa-session-self-diagnose`, `aoa-session-self-repair`,
  `aoa-session-progression-lift`, or `aoa-quest-harvest`
- one `HARVEST_PACKET_RECEIPT` using `references/stats-event-envelope.md` and
  `references/harvest-packet-receipt-schema.yaml`
- one `CORE_SKILL_APPLICATION_RECEIPT` using
  `references/core-skill-application-receipt-schema.yaml`

## Procedure
1. start from a reviewed session artifact rather than transient chat memory
2. inventory any checkpoint, closeout-handoff, or ledger hints as focus inputs;
   mark each hint as accepted, rejected, stale, cross-session, contaminated, or
   unresolved before it can influence a candidate
3. accept a hint only when the reviewed artifact or receipt evidence shows the
   same reusable unit; never mint `candidate_ref` from a hint alone
4. extract candidate reusable units, not topics; prefer explicit moves, laws, checklists, structures, routes, or proof patterns
5. split merged candidates until each unit has one honest owner shape
6. classify each kept candidate twice:
   - by reuse kind: pattern, mechanic, utility, law, proof, recall, or route
   - by owner shape: technique, skill, playbook, eval, memo, agent, or hold
7. mint `candidate_ref` only after the reviewed unit is bounded and the owner
   hypothesis plus nearest-wrong target are explicit
8. mark `automation_candidate` only when a repeated manual route is stable
   enough to name the current inputs, outputs, and risk posture, but the
   surviving question is still automation readiness rather than owner canon
9. reject theme-only repetition, aesthetic resonance, and broad "good idea" residue unless a bounded reusable unit exists
10. route reusable practice meaning to technique canon first
11. route bounded executable leaf workflows to `aoa-skills`
12. route multi-step recurring scenario methods to `aoa-playbooks`
13. route rubrics, verdict postures, and proof surfaces to `aoa-evals`
14. route recall, writeback, recurrence, and memory-support patterns to `aoa-memo`
15. route role law, orchestrator class law, handoff law, and actor-boundary rules to `aoa-agents`
16. keep `aoa-routing` and `aoa-kag` out of first-authoring unless the source-owned object already exists elsewhere and the session only discovered a derivative bridge update
17. preserve quest residue without forcing promotion when the reviewed session
    is still mixed, early, or weakly repeated
18. hand off to `aoa-automation-opportunity-scan` when the main surviving
    question is whether a repeated manual route is honestly automation-ready
19. hand off to `aoa-session-route-forks` when the main post-session need is
    explicit next-route choice rather than donor extraction itself
20. hand off to `aoa-session-self-diagnose` when the dominant surviving object
    is drift, contradiction, proof gap, or ownership confusion
21. hand off to `aoa-session-progression-lift` when the main surviving object is
    evidence-backed progression reflection rather than owner placement
22. when the candidate is a repeated reviewed quest unit and the remaining
    ambiguity is specifically the final promotion target among quest, skill,
    playbook, agent, eval, or memo, hand off to `aoa-quest-harvest`
23. draft the smallest next artifact for each accepted candidate, such as
    `TECHNIQUE.md`, `SKILL.md`, `PLAYBOOK.md`, `EVAL.md`, memory object seed,
    or agent/orchestrator surface note
24. keep `cluster_ref`, `owner_hypothesis`, `owner_shape`,
    `nearest_wrong_target`, `status_posture`, `evidence_refs`, `supersedes`,
    `merged_into`, and `drop_reason` on each accepted candidate when that carry
    exists or becomes explicit during reviewed harvest
25. emit one `HARVEST_PACKET_RECEIPT` when the packet is complete, using the
    shared event envelope and a bounded receipt payload instead of duplicating
    the full donor packet
26. when the finish path is complete, emit one
    `CORE_SKILL_APPLICATION_RECEIPT` that points back to the bounded detail
    receipt, keeps `application_stage=finish`, and stays generic enough to act
    as project-core kernel telemetry rather than a second donor packet
27. record one clear reason for the chosen owner and one clear reason against
    the nearest wrong owner

## Contracts
- invocation must remain explicit and post-session
- the skill harvests donor units; it does not treat session history as memory canon or instruction authority
- checkpoint notes, closeout handoffs, and ledgers are focus inputs only until
  reviewed evidence confirms or rejects them
- one candidate must map to one primary owner layer
- `candidate_ref` appears only after reviewed donor harvest
- `usefulness` is a reuse signal, not an owner layer by itself
- derivative layers do not become first authoring targets for source-owned meaning
- weak evidence may end in `hold` or `keep_or_open_quest`
- the `HARVEST_PACKET` may carry handoff hints, but it does not become hidden
  routing authority
- `automation_candidate` is only a detector hint; it is not schedule or
  mutation authority
- `HARVEST_PACKET_RECEIPT` stays subordinate to the packet and never replaces
  owner-layer or proof meaning
- `CORE_SKILL_APPLICATION_RECEIPT` stays subordinate to the packet and the
  detail receipt; it records one finished kernel-skill application and nothing
  more
- receipt corrections use `supersedes` rather than silent mutation
- drafting a next artifact is allowed; forcing promotion is not

## Risks and anti-patterns
- collapsing technique, skill, and playbook into one generic "good reuse" bucket
- mistaking session themes for reusable units
- routing authored meaning into `aoa-routing` or `aoa-kag` first
- turning agent class law into a skill just because an agent repeatedly used a workflow
- promoting memory residue as if it were proof or source meaning
- over-harvesting one session into too many thin objects
- treating a transcript package or session index as the same thing as donor harvest
- minting `candidate_ref` from checkpoint or handoff hints that were never
  confirmed in the reviewed artifact
- letting stale, cross-session, or diagnostic ledger residue enter the donor
  packet as if it belonged to the current reviewed session
- stuffing route-forks, diagnosis, repair, or progression meaning into vague
  donor notes instead of naming the next family seam explicitly
- turning donor harvest into a generic automation detector for every recurring
  annoyance instead of keeping automation readiness as its own seam
- letting receipt counts masquerade as proof, progression, or routing law

## Verification
- confirm the source artifact is reviewed and bounded
- confirm checkpoint, closeout-handoff, or ledger hints were dispositioned as
  accepted, rejected, stale, cross-session, contaminated, or unresolved before
  any candidate was minted from them
- confirm each kept candidate names one reusable unit rather than a topic
- confirm each accepted candidate has one primary owner layer
- confirm the nearest wrong target is rejected explicitly
- confirm `candidate_ref` was minted only for reviewed bounded units
- confirm every `candidate_ref` is supported by reviewed artifact or receipt
  evidence rather than hint presence alone
- confirm any surviving checkpoint `cluster_ref` stayed linked when available
- confirm no candidate routes source-owned meaning first into derivative layers
- confirm hold and defer outcomes remain available
- confirm the output names the next artifact rather than only abstract categories
- confirm any family handoff hint is explicit rather than smuggled into the
  packet as hidden policy
- confirm any `automation_candidate` names the current manual route and still
  stops short of automation authority
- confirm any emitted receipt stays append-only, evidence-linked, and smaller
  than the packet it summarizes
- confirm any emitted `CORE_SKILL_APPLICATION_RECEIPT` points to the matching
  detail receipt and does not widen beyond one finished kernel-skill run

## Technique traceability
Manifest-backed techniques:
- AOA-T-0075 from `8Dionysus/aoa-techniques` at `b4e5c1446469f142c60a85d0a7a4d9de7835ea65` using path `techniques/continuity/donor-harvest/session-donor-harvest/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Validation
- AOA-T-0076 from `8Dionysus/aoa-techniques` at `b4e5c1446469f142c60a85d0a7a4d9de7835ea65` using path `techniques/governance/decision-routing/owner-layer-triage/TECHNIQUE.md` and sections: Intent, When to use, Outputs, Core procedure, Risks, Validation
- AOA-T-0077 from `8Dionysus/aoa-techniques` at `b4e5c1446469f142c60a85d0a7a4d9de7835ea65` using path `techniques/continuity/donor-harvest/harvest-packet-contract/TECHNIQUE.md` and sections: Inputs, Outputs, Contracts, Validation

## Adaptation points
Project overlays may add:
- local session artifact entrypoints
- local review packet templates
- repo-relative destination paths for drafted artifacts
- local stop conditions for when the result must stay `hold`
- local naming rules for donor packs and quest IDs
- local family handoff preferences when automation scan, route-forks,
  diagnosis, repair, or progression surfaces exist
- local vocabulary for accepting, rejecting, or carrying checkpoint and
  closeout-handoff hints without making them authoring authority

This skill assumes the session artifact already exists. Adjacent history techniques such as session capture, transcript packaging, and local indexing remain separate neighbors rather than being reopened here.
