# AGENTS.md

This card applies to `.agents/`.

## Role

This model-facing directory maps optional profile bindings and ToS-local owner ports through
`.agents/agent-surface.manifest.json`. `.agents/README.md` is the human guide.
The current contributor route has no repository-local shared-skill projection.

## Boundary Routes

Keep profile declaration, installation, model visibility and actual invocation
as separate states. Shared packages retain their owning homes and profile-owned discovery; a ToS-local
procedure needs explicit admission before adding a local skill projection.
Authored source surfaces stay in `ToS/`, operation law in `mechanics/`, and proof authority, memory,
runtime, SDK, KAG and playbook authority with their corresponding owners.

## Validation

Review the manifest, owner references and public-safety boundaries by hand.
A declaration and its installation or activation are separate states.
