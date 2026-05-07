---
name: aoa-property-invariants
description: Express stable system or domain truths as invariant-oriented tests or checks rather than only fixed examples. Use when correctness depends on behavior that should hold across many inputs or states, such as monotonicity, idempotency, conservation, or structural rules. Do not use when no meaningful invariant is known yet or when the real task is auditing existing coverage instead of writing the properties.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: canonical
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/engineering/aoa-property-invariants/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0017,AOA-T-0007
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-property-invariants

## Intent
Use invariant-oriented thinking to test broad behavior and stable artifact relationships through properties rather than only through a short list of fixed examples.

## Trigger boundary
Use this skill when:
- a rule should hold across many inputs or states
- examples alone feel too narrow
- the system has conservation, monotonicity, idempotency, or structural invariants
- safety or correctness depends on broad input coverage
- a stable relationship should hold across artifacts, transformations, generated/export outputs, workflow phases, route choices, or provenance chains
- correctness depends on repeatability, round-trip behavior, source-ref preservation, lifecycle ordering, uniqueness, freshness bounds, or authorization limits

Do not use this skill when:
- the behavior is mostly about presentation details or narrow snapshots
- the main problem is checking whether existing checks really cover the invariant; use `aoa-invariant-coverage-audit` first
- the main problem is a boundary contract rather than a stable invariant; use `aoa-contract-test`
- no meaningful invariant is yet understood
- the proposed property would only restate one example with random inputs around it
- the input generator or artifact family cannot be bounded enough for review

## Inputs
- target rule or domain truth
- current examples or tests
- input space or generators
- known edge cases
- artifact families, transformations, lifecycle states, or generated/export surfaces when the invariant is not simple data behavior
- invalid cases, exclusion rules, and claim limits for the property

## Outputs
- explicit invariants
- property-oriented tests or checks
- notes on generator assumptions
- verification summary
- notes on what the property proves and what remains outside its claim

## Procedure
1. identify what must remain true across many cases
2. separate strong invariants from weak hopes or vague expectations
3. when the invariant is not a simple input/output behavior rule, choose the smallest useful shape from `references/invariant-shapes.md`
4. define the valid input space, artifact family, lifecycle range, or transformation surface, including exclusions
5. express those invariants as property-oriented tests or repeated checks
6. keep the generators or inputs bounded and reviewable
7. keep a few concrete examples when they explain failures better than generated data alone
8. run the checks and report what properties now constrain behavior and what they do not prove

## Contracts
- each property should express a real invariant, not a wish
- the test should broaden coverage beyond a small handpicked set
- generator assumptions should remain understandable
- failures should point back to the protected rule rather than only to generated data
- generated/export, workflow, provenance, lifecycle, and structural invariants should stay tied to the source truth they protect
- a property should name its claim limit so broad input coverage does not pretend to prove total quality

## Risks and anti-patterns
- writing weak properties that prove almost nothing
- confusing random data with meaningful coverage
- letting overly broad generators or harness complexity hide the protected domain truth
- using polished invariant language while the actual stable truth remains underspecified
- using property checks where a small set of concrete examples would be clearer
- using invariant language to avoid naming the actual rule or boundary that matters
- turning every stable example into a property when the variation space is not meaningful
- treating generated data volume as evidence when the generator misses the important states
- letting project-specific artifact names dominate the portable invariant shape

## Verification
- confirm the property expresses a meaningful invariant
- confirm the invariant broadens coverage beyond fixed examples
- confirm the report names which stable truth was violated or protected, not only that generated inputs failed
- confirm the result is understandable to another human or agent
- confirm the input generator or artifact set is bounded and reviewable
- confirm the claim limit is visible when the property does not cover all states, artifacts, or consumers

## Technique traceability
Manifest-backed techniques:
- AOA-T-0017 from `8Dionysus/aoa-techniques` at `fbead87e01b82df6c56e3d92a074cd7515131847` using path `techniques/proof/skill-support/property-invariants/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0007 from `8Dionysus/aoa-techniques` at `fbead87e01b82df6c56e3d92a074cd7515131847` using path `techniques/proof/evaluation-chain/signal-first-gate-promotion/TECHNIQUE.md` and sections: summary, Validation

## Adaptation points
Project overlays should add:
- local generator tools
- domain-specific invariants
- local artifact, generated/export, lifecycle, workflow, or provenance invariant examples from `references/invariant-shapes.md`
- rules for when property-based checks are mandatory, optional, or out of scope
