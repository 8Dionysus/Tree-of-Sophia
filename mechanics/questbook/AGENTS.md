# AGENTS.md

This card applies to `mechanics/questbook/` and every nested Questbook path.

## Role

Questbook keeps public obligations and dispatch contracts checkable without
turning philosophical interpretation, authored knowledge, or source meaning
into a task list.

## Read Before Editing

1. root `AGENTS.md`
2. `mechanics/AGENTS.md`
3. root `QUESTBOOK.md`
4. `mechanics/questbook/README.md`
5. `mechanics/questbook/PARTS.md`
6. `mechanics/questbook/PROVENANCE.md`
7. the owning active part README

## Boundaries

- Root `QUESTBOOK.md` stays the compact public obligation index.
- Root `quests/` stays the public quest item store.
- This package owns obligation and dispatch compatibility only.
- Do not make quest vocabulary replace source, node, relation, lineage,
  context, synthesis, canon, or philosophy branch authority.

## Validation

```bash
python scripts/validate_questbook_surface.py
python scripts/validate_mechanics_topology.py
python scripts/validate_nested_agents.py
```
