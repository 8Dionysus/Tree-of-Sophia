# Retained builder inputs

This directory preserves exact public Python source bytes referenced by recorded
research receipts. A source at `scripts/<name>.py` is retained at
`<name>/<sha256>.py`; the digest names the complete original byte sequence.
Preserve those bytes, including their historical wording.

The source-witness, lexical and transfer-readiness validators read an archive
only for an explicit original script path and recorded digest. The active
script must still exist. Resolution
checks every archive ancestor for symlinks, enforces a 1 MiB bound, and verifies
the full digest. A mismatch or absent archive leaves the recorded input
unavailable. This route reads source bytes without executing them.

Current execution uses the active script under `scripts/`. Retention establishes
availability of a receipt's stated input; execution authentication and assessment
retain their own evidence requirements. Current schema and output validation
remain independent.

The twenty-one files are exact Git blobs from
`f56cec46de2315f0f6411dcce6cacbe7d1da918a`, selected by the existing transfer
source-visible review, structural and passage candidates, authored-source
bridges, selected-form recurrence, lexical research and synthetic provenance
laboratory receipts. Their original
paths and recorded digests determine their locations here.
