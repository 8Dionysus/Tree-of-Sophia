# Candidate Terminal Receipts

Each JSON file records one immutable terminal transition for one reviewed
open-work candidate. A later correction uses a higher `record_version` and a
new receipt ID; it does not rewrite or delete the earlier receipt.

The receipt binds the exact candidate digest, the candidate-ledger SHA-256
prefix visible when the queue snapshot was frozen, the generated queue
snapshot that selected it, and one protocol-native discovery run. The ledger
binding lets replay retain later append-only queue growth without treating a
rewritten or inserted row as historical evidence.

Any independent provenance witness for that frozen snapshot must carry the
same candidate-ledger digest; a syntactically valid digest from another ledger
state is not sufficient.

It records what was and was not acquired, the layered rights result, any
source planting or operational relation refs, and the next trigger.

The latest receipt for a candidate must also bind one exact external timing
receipt by SHA-256. Every channel must have a positive monotonic transport
measurement, and its `measured_at` must not be later than terminal issuance;
historical receipts may retain their original unknown sentinels when a higher
version supersedes them.

A terminal receipt is execution evidence. It is not bibliographic, legal,
semantic, canon, publication, deployment, or human-acceptance authority.
