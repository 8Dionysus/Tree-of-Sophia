# Passport lane matrix v3

| Passport / posture | Honest lane |
| --- | --- |
| `d0_probe` or `d1_patch`, low risk, clear anchor, clear outputs | `codex_local_leaf` |
| bounded `d2_slice`, low risk, review helper role | `codex_local_reviewed` |
| high risk but narrowing child role and explicit reviewed closeout | `codex_local_reviewed` or explicit `remote_reviewed` |
| `d3+` | `split_required` |
| progression unlock required but missing | `human_gate` |
| self-agent checkpoint incomplete | `human_gate` |
| stress says `stop_before_mutation` | `human_gate` unless the child is only narrowing and non-mutating |
