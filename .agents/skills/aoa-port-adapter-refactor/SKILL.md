---
name: aoa-port-adapter-refactor
description: Refactor code toward clearer ports and adapters so domain or application logic is less entangled with infrastructure details. Use when concrete dependencies leak into core logic, tests are hard because external concerns bleed inward, or a module needs a clearer seam before further change. Do not use for tiny local fixes or when there is no meaningful boundary to clarify.
license: Apache-2.0
compatibility: Designed for Codex or similar coding agents with repository file access and an interactive shell. Network access is optional and only needed when repository validation or referenced workflows require it.
metadata:
  aoa_scope: core
  aoa_status: evaluated
  aoa_invocation_mode: explicit-preferred
  aoa_source_skill_path: skills/core/engineering/aoa-port-adapter-refactor/SKILL.md
  aoa_source_repo: 8Dionysus/aoa-skills
  aoa_technique_dependencies: AOA-T-0016,AOA-T-0015
  aoa_portable_profile: codex-facing-wave-3
---

# aoa-port-adapter-refactor

## Intent
Use this skill to separate reusable domain, application, builder, or workflow logic from concrete dependencies and runtime details by introducing or clarifying ports and adapters.

## Trigger boundary
Use this skill when:
- business or domain logic is tightly coupled to a concrete dependency
- tests are hard to write because external concerns leak into the core logic
- the same dependency pattern is beginning to repeat across modules
- a module needs a clearer seam before further change
- a builder, CLI, SDK facade, generated/export writer, storage layer, filesystem path, environment lookup, clock, network client, subprocess, or provider API leaks into reusable logic
- the boundary is already named, and the next honest move is a narrow dependency seam rather than another context map

Do not use this skill when:
- the task is a tiny local fix with no architectural pressure
- there is no meaningful boundary to clarify yet
- the code would become more ceremonial than useful after extraction
- the main problem is deciding whether logic belongs in the core or at the edge; use `aoa-core-logic-boundary` first
- the main problem is clarifying repository docs or source-of-truth ownership; use `aoa-source-of-truth-check` first
- the main problem is validating a stable consumer-visible interface after the seam is clear; use `aoa-contract-test`
- the dependency is only incidental setup or test fixture detail, not a real source of coupling

## Inputs
- target module, builder, tool, workflow, or slice
- concrete dependency or runtime concern that currently leaks into reusable logic
- desired scope of refactor
- validation expectations
- behavior the reusable center actually needs from the dependency
- callers, consumers, or generated/export surfaces affected by the seam

## Outputs
- clearer boundary between logic and infrastructure
- proposed or implemented port shape
- proposed or implemented adapter shape
- notes on what remains inside the reusable center and what the adapter owns
- verification summary

## Procedure
1. identify the concrete dependency that is making change or testing harder
2. confirm the broader context and core-versus-edge boundary are already clear enough to make a seam; otherwise route to `aoa-bounded-context-map` or `aoa-core-logic-boundary`
3. when the dependency is not a simple service client, choose the smallest useful shape from `references/adapter-seam-shapes.md`
4. isolate the reusable logic from the infrastructure-specific behavior
5. define a narrow port around what the reusable center actually needs
6. move infrastructure-specific behavior behind an adapter or equivalent seam
7. keep the refactor bounded and avoid unrelated cleanup
8. verify that the new boundary improves clarity and does not silently change behavior

## Contracts
- the extracted boundary should reduce coupling, not add decorative abstraction
- the port should stay narrow and purpose-shaped
- the port should describe the reusable center's need, not mirror the provider API
- adapters should own translation, retries, transport, local paths, credentials, process calls, or runtime discovery when those details are the dependency pressure
- the refactor should not widen into a hidden rewrite
- the final result should remain understandable to another human or agent
- generated, exported, or runtime-facing consumers should keep their explicit contract checks separate from the adapter refactor

## Risks and anti-patterns
- introducing empty abstractions with no real boundary value
- extracting a port that mirrors an overgrown concrete dependency instead of narrowing it
- using the refactor as a pretext for unrelated architectural churn
- making tests more indirect without improving clarity
- wrapping every helper in an interface because ports are fashionable
- hiding source-of-truth or core-logic ambiguity under a new adapter name
- treating adapter introduction as proof that downstream contracts are validated

## Verification
- confirm the concrete dependency pressure was real
- confirm the new boundary is narrower and clearer than before
- confirm behavior did not drift silently
- confirm the refactor stayed inside the declared scope
- confirm the adapter owns the concrete dependency details and the reusable center depends only on the narrow port
- confirm any generated/export or consumer-visible behavior still has its own contract validation when needed

## Technique traceability
Manifest-backed techniques:
- AOA-T-0016 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/proof/skill-support/bounded-context-map/TECHNIQUE.md` and sections: Intent, When to use, Outputs, Core procedure, Contracts, Validation
- AOA-T-0015 from `8Dionysus/aoa-techniques` at `cd276f040d55d490bd015b8698c7a5d594b9f875` using path `techniques/proof/skill-support/contract-test-design/TECHNIQUE.md` and sections: Intent, When to use, Inputs, Outputs, Core procedure, Contracts, Risks, Validation

## Adaptation points
Future project overlays may add:
- local architecture conventions
- preferred adapter locations
- local test commands
- local adapter seam examples from `references/adapter-seam-shapes.md`
- repository-specific review rules
