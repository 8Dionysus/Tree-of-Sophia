---
name: aoa-sanitized-share
description: Turn raw technical material into a shareable public-safe artifact while keeping the raw source separate, placing the sanitized result in the canonical sharing surface, and preserving the lesson after redaction. Use when logs, configs, diagnostics, or reports may contain secrets, topology, or internal identifiers. Do not use when the material is already clearly public-safe or when the task is to perform the underlying operational change.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: risk
  aoa_status: canonical
  aoa_invocation_mode: explicit-only
  aoa_source_skill_path: skills/risk/aoa-sanitized-share/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0034,AOA-T-0002
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-sanitized-share

## Intent
Use this skill to turn potentially sensitive technical material into a shareable, reviewable, public-safe form, keeping the raw source distinct from the final public-safe output and placing that output where reviewers expect it.

## Trigger boundary
Use this skill when:
- logs, configs, diagnostics, reports, or examples may contain sensitive details
- a result needs to be shared publicly or with a broader audience
- raw material may reveal secrets, topology, internal identifiers, or unsafe context
- the output needs a canonical public-safe home rather than an ad hoc pasted summary

Do not use this skill when:
- the material is already clearly public-safe and minimal
- the task is to perform the underlying operational change rather than prepare a shareable surface
- the main task is deciding whether the underlying action should be allowed; use `aoa-approval-gate-check`
- the task is to preview or execute the operational change itself; use `aoa-dry-run-first` or `aoa-safe-infra-change`

## Inputs
- material to be shared
- sharing audience or context
- known sensitive surfaces
- acceptable level of abstraction

## Outputs
- sanitized shareable artifact, abstract summary, or recommendation not to share the raw material directly
- note on what was generalized or removed
- warning about any remaining ambiguity or sensitive edge
- canonical public-safe output location or reference

## Procedure
1. inspect the material for secrets, tokens, private paths, topology, internal identifiers, or unsafe operational detail
2. separate raw source material from the shareable surface before rewriting anything
3. remove, redact, or generalize sensitive details
4. preserve the technical lesson or signal without preserving the sensitive surface
5. place the sanitized output in the canonical public-safe location or repo surface
6. note what kind of sanitization was applied when that matters for interpretation
7. verify that the shared result remains useful without revealing what should stay private

## Contracts
- shareable output should not leak secrets or private infrastructure detail
- sanitization should preserve meaning where possible
- generalization should not silently change the core lesson beyond recognition
- uncertainty about sensitivity should lean toward caution
- raw material and shareable output should remain clearly separated
- the sanitized artifact should be discoverable from the expected public-safe surface

## Risks and anti-patterns
### Failure modes

- over-sanitizing until the artifact becomes meaningless
- under-sanitizing because a value looks harmless in isolation
- collapsing raw and shareable surfaces into one note or transcript

### Negative effects

- the shared artifact becomes hard to reuse or verify
- sensitivity leaks through topology, naming, or surrounding context even when tokens are removed
- reviewers cannot tell where the canonical public-safe version lives

### Misuse patterns

- sharing raw excerpts when a bounded summary would be safer
- treating a small harmless-looking field as proof that the full material is safe
- using the shareable surface as a substitute for the raw source or vice versa

### Detection signals

- the sanitized output still points too directly to private topology or naming
- a reviewer cannot tell what was generalized or removed
- the artifact no longer communicates the lesson it was meant to preserve
- the output has no clear canonical place for future reuse

### Mitigations

- generalize paths, hostnames, and private identifiers when needed
- name the sanitization level and the remaining uncertainty
- verify the shared result remains useful without preserving the sensitive surface
- keep a visible boundary between raw input, sanitized output, and published placement

## Verification
- confirm obvious sensitive surfaces were checked
- confirm the resulting artifact is still understandable
- confirm the sanitization level matches the intended audience
- confirm raw sensitive detail was not preserved by accident
- confirm remaining uncertainty is named rather than ignored
- confirm the sanitized output lives in the expected public-safe location or reference
- confirm the raw/shareable split is still obvious after editing

## Technique traceability
Manifest-backed techniques:
- AOA-T-0034 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/instruction/docs-boundary/public-safe-artifact-sanitization/TECHNIQUE.md` and sections: Intent, When to use, When not to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation
- AOA-T-0002 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/instruction/docs-boundary/source-of-truth-layout/TECHNIQUE.md` and sections: Intent, When to use, When not to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Future project overlays may add:
- local sanitization rules
- examples of sensitive surfaces
- public versus private sharing thresholds
- project-specific reporting conventions
