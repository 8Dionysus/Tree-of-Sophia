# Candidate Terminal Receipts

Each JSON file records one immutable terminal transition for one reviewed
open-work candidate. A later correction uses a higher `record_version` and a
new receipt ID; it does not rewrite or delete the earlier receipt.

The receipt binds the exact candidate digest, the generated queue snapshot
that selected it, and one protocol-native discovery run. It records what was
and was not acquired, the layered rights result, any source planting or
operational relation refs, and the next trigger.

The latest receipt for a candidate must also bind one exact external timing
receipt. Every channel must have a positive monotonic transport measurement;
historical receipts may retain their original unknown sentinels when a higher
version supersedes them.

A terminal receipt is execution evidence. It is not bibliographic, legal,
semantic, canon, publication, deployment, or human-acceptance authority.
