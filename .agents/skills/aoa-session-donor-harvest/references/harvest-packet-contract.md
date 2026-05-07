# Harvest Packet Contract

Use this reference when `aoa-session-donor-harvest` needs a small but explicit
shape for the post-session packet it emits.

## Required fields

- `packet_version`
- `source_session_ref`
- `reviewed_artifacts`
- `candidates`
- `deferred_or_dropped`

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

Each accepted candidate should keep:

- `candidate_ref`
- optional `cluster_ref`
- `title`
- `kind`
- `summary`
- `owner_hypothesis`
- `owner_shape`
- `evidence_refs`
- `repeat_signal`
- `owner_repo`
- `chosen_next_surface`
- `nearest_wrong_target`
- `status_posture`
- optional `supersedes`, `merged_into`, and `drop_reason`
- optional `difficulty`, `risk`, `control_mode`, and `notes`

## Contract rule

The `HARVEST_PACKET` is a bounded post-session packet.
It may point at route forks, diagnosis, repair, progression, or quest follow-up,
but it must not silently replace those family seams.
