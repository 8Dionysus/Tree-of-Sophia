# Decision Records Index

This directory is the durable decision surface for `Tree-of-Sophia`.

Use it when a future contributor needs the rationale for a route, topology, owner boundary, validator route, export posture, source-discipline rule, or tree-layer split.

Ordinary implementation notes, generated output, runtime logs, private evidence, proof verdicts, review notes, and one-off planning thoughts route to their owning surfaces instead.

## Operating Card

| Field | Route |
| --- | --- |
| role | durable ToS decision rationale entrypoint and index chooser |
| input | changed source-first route, owner boundary, rejected option, validation guard, export posture, or tree-layer pressure |
| output | canonical decision note, generated lookup indexes, and route back to the source surface |
| owner | `docs/decisions/AGENTS.md` for lane law; decision notes for rationale; generated indexes for lookup only |
| next route | source surface first, then nearest route card, `CHARTER.md`, `BOUNDARIES.md`, `ROADMAP.md`, generated lookup indexes, or the affected sibling owner |
| validation | `python scripts/generate_decision_indexes.py --check` and `python scripts/validate_decision_records.py`, plus the owning validator for the changed surface |

## Authority

Decision notes explain why a route was chosen.

They are weaker than the source surface they describe:

- source authority stays in `ToS/source-witnesses/`;
- golden Zarathustra route surfaces stay in `ToS/zarathustra/`;
- non-authoritative research scaffolds stay in `ToS/research-packets/`;
- candidate structure stays in `ToS/candidate-intake/`;
- canonical authored nodes and relations stay in `ToS/canon/`;
- public compatibility stays in `ToS/public-compatibility/`;
- derived export stays in `ToS/derived-exports/`;
- route law, current doctrine, review posture, boundaries, and roadmap stay in their owning surfaces;
- neighboring repositories keep stronger truth for federation, runtime, SDK control-plane helpers, KAG substrate semantics, skills, techniques, and playbooks.

Generated decision indexes are weaker than the decision notes. They exist to make lookup cheaper for agents, not to carry decision rationale.

## Index Shape

Each decision owns:

- a canonical `Decision ID: TOS-D-####`;
- a full canonical-ID filename, for example `TOS-D-0001-*.md`;
- an `## Index Metadata` block naming original date, surface classes, ToS layers, tree classes, guard families, and posture.

The lookup indexes under [indexes](indexes/README.md) are generated from that metadata:

- [Decisions by canonical ID and number](indexes/by-number.md)
- [Decisions by date](indexes/by-date.md)
- [Decisions by surface class](indexes/by-surface.md)
- [Decisions by ToS layer](indexes/by-tos-layer.md)
- [Decisions by tree class](indexes/by-tree-class.md)
- [Decisions by validation or guard family](indexes/by-guard.md)

Regenerate the read models after decision metadata changes:

```bash
python scripts/generate_decision_indexes.py
```

Check generated parity before closeout:

```bash
python scripts/generate_decision_indexes.py --check
```

## Lookup Route

Do not hand-maintain a "latest decision" roster in this README. That list drifts as soon as a new decision lands.

Use the generated indexes instead:

- [by number](indexes/by-number.md) for the complete canonical ledger;
- [by date](indexes/by-date.md) for recent landings;
- [by surface](indexes/by-surface.md), [by ToS layer](indexes/by-tos-layer.md), and [by tree class](indexes/by-tree-class.md) for owner-pressure lookup;
- [by guard](indexes/by-guard.md) for validation, route-law, or boundary pressure.

The first decision records why this lane exists without turning it into source canon or runtime authority.

## Addressing

Full canonical-ID decision paths are the active source files:

- `docs/decisions/TOS-D-0001-*.md`
- `docs/decisions/TOS-D-0002-*.md`
- `docs/decisions/TOS-D-####-*.md`

Canonical IDs remain stable handles. Previous path names belong to git, PR, or release history, not to a compatibility lookup layer.

## Naming

Use the full canonical decision ID as the filename prefix:

`TOS-D-0001-short-decision-slug.md`

Prefer short titles that name the route, not the whole debate.

## Template

Start from [TEMPLATE.md](TEMPLATE.md) for new decisions. Keep notes concise, but include enough context, options, rationale, consequences, source surfaces, and validation for a future agent to avoid repeating the same route question.
