# Confidential native TextUnit creation: bounded review, 2026-09-08

Status: implemented local immutable creation and exact retry; not completed
private semantic growth, real linguistic assessment, publication or Foundation v1.

## Owner and operation

[Corpus Foundation](../doctrine/CORPUS_FOUNDATION.md#owner-local-source-contexts)
and [TOS-D-0055](../../docs/decisions/TOS-D-0055-explicit-confidential-source-contexts.md)
retain one knowledge grammar in separately governed source locations. The
[native contract](../contracts/source-text-unit-packet-v1.schema.json) is not
changed. The [source-owner command](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#confidential-native-textunit-creation)
adds an explicitly delegated constructor over an existing exact binding,
not a semantic Description subclass or arbitrary packet importer.

The request chooses bounded spans and explicit complementary gaps. Source
scope, layer, original file identity and rights refs are copied from verified
native evidence. New packet/scheme/segmentation/unit/anchor IDs come from the
protected delegation, independently of words, offsets, labels and ordering.
All new states remain method-proposed; historical review is not rewritten.
The native resolver reads the resulting private packet using its existing
binding grammar. Default public consumers still cannot read that namespace.

The complete owner configuration, request, runtime description, native-aware
provenance, packet and existing source-create receipt travel in one mode-0700
atomic directory. Files are 0600. Exact retry binds all stored bytes before
excluding its own package from collision discovery and rechecks current input
and rights dependencies. No request or configuration prose is executed. Source
reading is explicit and separate from write delegation and recorded use rights.

## Review findings and corrections

The independent helper implemented and tested the pure constructor; root
implemented the command adapter and inspected the entire constructor and tests.
Root found a normalization-scope mismatch: the first constructor checked the
whole representation instead of its declared text scope. The correction keeps
the whole-byte digest while checking normalization only inside the scope; a
regression preserves an NFD sequence outside an otherwise NFC scope.

Independent adapter review found that prefix-only inventory omitted existing
`part-N.source-text-unit.v1.json` packets and JSONL anchors. Their established
owner filename families now participate in bounded identity discovery. The
first live inventory probe also exposed existing multi-megabyte native JSON
packets; the explicit discovery budget now accommodates them without raising
the command/request or newly constructed packet budget. On the current public
metadata set, the read-only probe verified 232 files, 32,688,225 bytes in
0.584 seconds. This is one observed inventory pass, not a cold/warm corpus
benchmark or a claim of unlimited scale. No original payload was read.

The helper reproduced two rights-selection bypasses: a current nested decision
could mask a superseded parent record, and a direct exact allow could hide a
competing nested exact restriction from another record. Bound parent
lifecycle/denial/conflict is now checked before selection, and exact nested
decisions are collected from all bound records. Both negative controls require
refusal before the first representation read. Conditional use is not silently
cleared; this initial constructor accepts an existing unconditional local
research route and returns other rights pressure to its actual owner.

## Verification

Root final focused runs passed 20 pure-constructor tests (0.345 seconds), 22 command
tests (19.494 seconds), 10 script-topology tests (1.294 seconds) and 6
test-topology tests (0.052 seconds). The command tests include real CLI
prepare/create/retry, immediate native resolver consumption, unchanged CRLF/NFD
source bytes, nonzero scopes, explicit gaps, private modes, source/configuration
drift, corrupt replay and staging, occupied directories and metadata-only ID
collisions. A killed subprocess before rename leaves no authored target; its
retry succeeds without accepting or deleting the abandoned stage. Large legacy
native JSON is counted without relaxing the new-packet budget. Pure tests
include a 256-unit construction, scope-limited normalization and preservation
of an explicitly unknown method locale. These are synthetic fixtures, not
linguistic or rights findings. The final general source-command suite passed
39 tests (305.306 seconds), and source Claim commands passed 26 (235.069 seconds)
after integration of the new dispatch and private provenance branch.
Documentation tests passed 35 (1.725 seconds), nested route cards passed 56,
and native text-unit laboratory/source-home validators passed. The corpus
index was rebuilt and its parity and validation passed. Generated documentation
currentness required its normal owner rebuild, not a relaxed stale check.

The independent helper's final bounded adapter review reported no remaining
actionable issue after the rights and filename fixes. Root additionally
verified the exact staged bytes and private lock permissions before publication.
Passing validation does not authenticate an unsigned provenance event, prove
transcription fidelity or make the proposed segmentation correct.

## Boundaries and next owner

Review checklist: exact source return, layer/identity preservation, explicit
uncertainty and coverage, rights/visibility inheritance, independent operation
authority, byte-bound replay and no semantic promotion are covered. Canon,
translation acceptance, lived-witness consent and public tiny-entry changes are
not applicable. No real private source packet or durable private store was
created by this slice. Synthetic test and metadata-inventory scratch are not
authored corpus results.

Native revision and source-layer bootstrap, private Description/Occurrence/
Claim/forms, private assessment and real source-visible linguistic work remain
with ToS growth and assessment owners. The current public projection, UI,
Worker/D1, broader scaling, CI, merge, deployment and public runtime are not
proved by these tests. Removing this writer does not authorize deleting a
future private store, its source packets or its history.
