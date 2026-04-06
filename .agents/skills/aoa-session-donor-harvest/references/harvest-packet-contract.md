# Harvest Packet Contract

Use this reference when `aoa-session-donor-harvest` needs a small but explicit
shape for the post-session packet it emits.

## Required fields

- `session_ref`
- `reviewed_artifacts`
- `extracts`

## Common optional fields

- `closure_state`
- `wins`
- `frictions`
- `deferrals`
- `quest_hooks`
- `chronicle_stub`
- `fork_cards`
- `diagnosis`
- `repair_candidates`
- `progression`

## Extract record expectations

Each extract should keep:

- `title`
- `kind`
- `summary`
- `evidence_refs`
- `repeat_signal`
- `owner_repo`
- `chosen_next_surface`
- `nearest_wrong_target`
- optional `difficulty`, `risk`, `control_mode`, and `notes`

## Contract rule

The `HARVEST_PACKET` is a bounded post-session packet.
It may point at route forks, diagnosis, repair, progression, or quest follow-up,
but it must not silently replace those family seams.
