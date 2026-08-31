# Agent-surface validation

Use this route after the affected model-facing card, skill, or owner port is
known. Exact internal commands remain in `docs/validation/validation_lanes.json`.

```bash
python scripts/validation_lanes.py --run agent_surface
python scripts/validation_lanes.py --run route_docs
```

These checks establish structural and currentness evidence only. They do not
prove model behavior, owner acceptance, or runtime activation.
