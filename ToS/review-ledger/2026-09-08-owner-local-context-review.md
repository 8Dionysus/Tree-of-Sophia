# Confidential source transport: bounded review, 2026-09-08

Status: local context/native-reader integration, not the complete private
Occurrence/Claim/form workflow or real private source creation.

[Corpus Foundation](../doctrine/CORPUS_FOUNDATION.md#owner-local-source-contexts)
owns the source-location distinction;
[TOS-D-0055](../../docs/decisions/TOS-D-0055-explicit-confidential-source-contexts.md)
records why the current public corpus guard is preserved while confidential
source-bearing records need a disjoint owner-selected store. The
[context contract](../contracts/owner-local-source-context.schema.json) and
[mechanic API](../../mechanics/growth-cycle/parts/branch-growth-cycle/README.md#explicit-owner-local-source-transport)
define the actual interface. Context creation is not a grant of any operation.

The native v1 binding is unchanged. Its logical owner-local prefix is an
explicit singleton partition, not a guessed fallback or another ontology.
Context/schema bytes, physical root roles and identities enter opaque
currentness along with native dependencies. Default public paths retain their
old snapshot behavior and refuse the reserved namespace. Private schema copies
cannot replace the source-owned grammar. Private files and configuration need
0600; private directories need 0700, stricter than the public command reader's
other-user-write protection. Custom bounded readers retain their accounting
but cannot bypass pre/post confidentiality checks.

The existing public creation/form/revision/Claim dependency closures now pin
the new helper's implementation as well as the native resolver, because even
the default resolver uses its namespace guard. Native private metadata or a
private representation route imposes an owner-local disclosure ceiling;
underlying public source declarations do not authorize that packet's release.

An independent bounded helper authored the synthetic context suite and
reviewed implementation. It found that metadata-only public resolution could
advertise a private representation locator because the path check ran only on
byte reads. The regression first failed; the resolver now validates transport
addressability without opening content before computing availability. The
helper also verified the transitive implementation dependency pins. Its final
19 tests passed in 4.277 seconds with no remaining actionable finding in this
bounded review. Root read the complete tests and changed production code.

Root results: context 19 tests in 4.217 seconds; native binding 27 in 10.454;
native assessment 18 in 27.975; Occurrence integration 13 in 60.850. Script
topology passed 10 in 2.486 seconds; test topology 6 in 0.082. These fixtures
use synthetic CRLF/NFD text and preserve its bytes, identities and proposed
statuses; they are not rights or linguistic evidence. Decision indexes were
generated and their parity and record validation passed.
Source-foundation validation passed with present-byte fixity, as did source
home, bibliographic graph parity, cross-corpus documentation currentness,
35 documentation tests (3.665 seconds) and the 56-card nested-agent check.
The corpus index required its normal owner rebuild after the doctrine change;
it is a generated companion, not evidence of private source migration.

Review checklist: source return, single owner per ref, private/public and
native/description boundaries, preserved identity and uncertainty, separate
read permission/competence/authority and no historical promotion are yes.
Canon, translation, public tiny-entry, lived witness, counterpart, calibration
and compost changes are not applicable. No retained real private payload or
durable private store was opened or created by this slice; no source record,
Claim, human form or assessment was migrated or admitted.

Private native creation and source/Claim/form/assessment adapters are the next
ToS growth owners. Real source-visible evaluation, private reader consumption,
UI, Worker/D1, scaling, preservation/backup, CI, merge and deployment remain
separate evidence requirements. Removing this reader does not authorize
deleting a future private store or its history. The Unix account is trusted;
same-account hostile-code isolation and encryption are not claimed.
