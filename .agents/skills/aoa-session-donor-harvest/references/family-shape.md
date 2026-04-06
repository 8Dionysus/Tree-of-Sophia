# Family Shape Around `aoa-session-donor-harvest`

Recommended relation graph:

- `aoa-session-donor-harvest`
  - authors a bounded `HARVEST_PACKET`
  - may hand off to:
    - `aoa-session-route-forks`
    - `aoa-session-self-diagnose`
    - `aoa-session-self-repair`
    - `aoa-session-progression-lift`
    - `aoa-quest-harvest`

- `aoa-session-self-diagnose`
  - may hand off to `aoa-session-self-repair`

- `aoa-session-self-repair`
  - may emit repair quests or owner-repo deltas

This keeps the donor-harvest nucleus strong while avoiding one giant
bag-of-everything skill.
