# Operational Route Cards

## Index Metadata

- Decision ID: TOS-D-0004
- Original date: 2026-06-07
- Surface classes: docs/route-law, source-home, scripts/validation, agent-instructions
- ToS layers: docs, doctrine, source-witnesses, research-packets, philosophy, candidate-intake, canon, public-compatibility, derived-exports, contracts, scripts
- Tree classes: source, concept, principle, lineage, event, state, support, analogy, synthesis, relation
- Guard families: instruction topology, owner boundary, source-first authority, validation route, metadata boundary
- Posture: accepted

## Context

The first `ToS/` source-home landing created nested owner surfaces and route
cards. Some of those cards preserved local correction pressure as broad
negative sections. That shape made future agents attend to prior mistakes
instead of the operational route.

External agent guidance points in the same direction: keep agent systems simple
and composable, make tool and route boundaries explicit, state what to do
instead of only saying what to avoid, and preserve verification as a first-class
part of the loop.

## Reference Inputs

- Anthropic, "Building effective agents"
- OpenAI, "A practical guide to building agents"
- OpenAI Help Center, "Best practices for prompt engineering with the OpenAI API"
- OpenAI, "Unrolling the Codex agent loop"
- Anthropic, "Best practices for Claude Code"
- Google ADK, Context, State, and Memory docs
- Model Context Protocol, server concepts and roots docs
- Jason Liu, "Codex-maxxing"

## Decision

Current ToS route cards should use an operating-card shape:

- role
- input
- output
- owner
- next route
- tools
- check

Boundary text should route material to the correct owner surface. Negative
warnings are reserved for compact machine checks or genuinely dangerous
boundary failures; they are not the default shape for agent-facing doctrine.

## Options Considered

- Keep the negative sections as stop lines. This preserved fresh correction
  pressure, but it made local errors look like permanent doctrine.
- Delete all boundary language. This reduced noise, but it would leave future
  agents without a route for source, intake, canon, export, and sibling-owner
  material.
- Convert current route cards to operating cards and boundary routes. This
  keeps the useful boundary information while making the next action explicit.

## Rationale

Tree of Sophia is expected to grow into large branch and graph surfaces. Future
agents need a routing map more than a list of anxieties. The operating-card
shape gives them the minimum durable information needed to act:

- what the surface is for
- what enters it
- what leaves it
- who owns it
- where material goes next
- which tools and checks close the loop

The same principle applies to AI-generated research packet material. Capture
page titles and temporary UI labels remain useful metadata, but repository
topology names the philosophical branch identity and does not treat the packet
as source authority.

## Consequences

`ToS/README.md` and current nested route cards now prefer `Operating Card` and
`Boundary Routes` sections.

`scripts/validate_nested_agents.py` rejects the old broad negative section
headers in current nested route cards.

`scripts/validate_tos_source_home.py` checks the same route-card shape in the
ToS home README.

`scripts/validate_philosophy_topology.py` now checks metadata-only capture
labels from non-authoritative research packet metadata, rejects the old
AI/Notion-as-source-witness route, and avoids hard-coding prior bad path names
as topology.

The `ToS/*/AGENTS.md` branch cards now use the same compact route shape: local
role, operating card, boundary routes, and validation route. Repeated read-first
lists, inherited prohibitions, and branch inventories route to the source home,
nearest owner card, or validator instead of being copied into each branch.

## Source Surfaces

- `ToS/README.md`
- `ToS/AGENTS.md`
- `ToS/philosophy/AGENTS.md`
- `ToS/philosophy/README.md`
- `ToS/philosophy/philosophy.manifest.json`
- `ToS/research-packets/AGENTS.md`
- `ToS/research-packets/deep-research/philosophy/AGENTS.md`
- `ToS/candidate-intake/AGENTS.md`
- `ToS/canon/AGENTS.md`
- `ToS/contracts/AGENTS.md`
- `ToS/derived-exports/AGENTS.md`
- `ToS/doctrine/AGENTS.md`
- `ToS/public-compatibility/AGENTS.md`
- `ToS/source-witnesses/AGENTS.md`
- `ToS/zarathustra/AGENTS.md`
- `ToS/research-packets/deep-research/philosophy/research.manifest.json`
- `ToS/research-packets/deep-research/philosophy/pages/2b6778a4-3758-80ee-bbe5-dc8387858f3c/capture.meta.json`
- `scripts/validate_nested_agents.py`
- `scripts/validate_tos_source_home.py`
- `scripts/validate_philosophy_topology.py`

## Validation

Run:

```bash
python scripts/validate_tos_source_home.py
python scripts/validate_philosophy_topology.py
python scripts/validate_nested_agents.py
python scripts/generate_decision_indexes.py
python scripts/generate_decision_indexes.py --check
python scripts/validate_decision_records.py
python scripts/release_check.py
```
