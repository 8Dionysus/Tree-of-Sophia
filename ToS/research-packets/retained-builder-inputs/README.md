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

The active route determines execution separately from retained input custody.
For Antonovsky v1, the script is a native CLI compatibility entry; its exact
`4c80683124592bc969e0db6ffe0d5696ec28d4079bc522725788aeff44abd8eb`
recipe is retained here as nonexecuted bytes. Native Build/Check require the
active original path to exist, then accept only the exact original bytes at
that path or this digest archive, with held-root no-follow reads and a 1 MiB
bound. The logical original path and recorded digest remain provenance data;
the installed native image and its execution receipt identify the actual
producer. Retention establishes input availability, not execution or assessment.
Current schema and output validation remain independent.

The twenty-one files are exact Git blobs from
`f56cec46de2315f0f6411dcce6cacbe7d1da918a`, selected by the existing transfer
source-visible review, structural and passage candidates, authored-source
bridges, selected-form recurrence, lexical research and synthetic provenance
laboratory receipts. Their original
paths and recorded digests determine their locations here.

The additional Antonovsky v1 blob is exact maintained source from
`0ee415f03553d745ff54e2d08ee447862e66c555`, 91,999 bytes. It is never imported
or executed by the native producer. The active Python module temporarily retains
helpers imported by the separately supported v2 builder; that dependency is not
retirement of the entire Python family.

The additional Antonovsky v2 blob is the exact 61,015-byte maintained source
`fa3cd89b6cad07aa2f63dad6cc84707902586b2b262db4a98128d5def9261a01`,
retained before its CLI entry changed to native execution. The historical
`f1c6bf35b382e1daa15f6f934107e130a781c7d4f0943b196e853dbdc4409570`
blob remains unchanged. The v2 native producer does not execute these archives
or hash its active Python entry as a generated manifest input; its static inputs
are the census, identity issuance and primary challenger. Research oracle
execution uses separately authenticated private inputs under its own custody;
archive availability is not permission to execute or a claim of independence.

The paragraph alignment recipe is additionally retained from immutable commit
`b0703ca53f3d5d8ac8d2a9b3bedd1fab64638389`, original path
`scripts/build_zarathustra_de_ru_paragraph_alignment_v1.py`, Git blob
`417f6f586434a701df264525f3a6c3b89fd63943`, SHA256
`0b3d29bc9a9d8b1aa6bbdbd06cfa68e7bcb46168f188d914243648d90344f329`.
Its archive is historical rendering-recipe source only: never execute or import
it as a maintained producer fallback. The active entry dispatches native `tos`.
Historical packet `software_ref` names the original logical rendering path;
changed facade bytes do not acquire that recipe identity. Native implementation
and selected executable identity belong to separate execution receipts.
