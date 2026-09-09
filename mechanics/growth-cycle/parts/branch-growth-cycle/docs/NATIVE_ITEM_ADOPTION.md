# Native local Item adoption

`source_item_commands.py` adds `item.adopt` and `item.adoption.recover` to the
existing `source_commands.py` front door. It adopts one **already obtained local
file**, creates one provisional acquired Item and distinct `exemplified_by`
Claim, and appends only the existing Edition's `exemplar_claim_refs` with one
version/form successor. It performs no URL fetch, OCR, model call, correction,
translation, source reading, rights admission or publication. Artifact and
scholarly Composite representation routes remain separate; a Work/Expression
directory ladder is not a global bibliographic requirement.

## Exact delegation

The protected owner configuration uses `tos_local_item_adoption_owner_v1`.
`--discover --handler native-item-adoption` exposes operation/request shapes
without selecting a grant. Configuration keys are exactly:

- Common metadata authority: `uid`, `principal_id`, `maker_type`, `source_root`,
  `authority_ref`, `expires_at`, `allowed_operations`, `schema_version`.
- Existing `edition_id`, `edition_source_path`; new `item_id`,
  `item_source_path` (`<edition-home>/items/<slug>/item.json`), `claim_id`,
  `provenance_event_id`; `allowed_edition_form_ids`, `allowed_item_form_ids`,
  `allowed_claim_form_ids` (distinct bounded form sets).
- Exact File: `file_id`, `payload_basename`, `original_basename`, `media_type`,
  `byte_size`, `sha256` (bare 64 hexadecimal digits).
- Separate source records: `rights_id`, `acquisition_event_id`,
  `inventory_event_id`. All three event IDs must differ.
- Separate local-copy authority: `input_path`, `payload_root`,
  `payload_authority_ref`, `payload_expires_at`, `recovery_root`.

All roots and input are canonical absolute, protected, owner-selected paths.
`source_root` is the metadata checkout. `payload_root` is the canonical
**source-witnesses directory**, not a repository root; no request or worktree
default may choose it. Destination is exactly
`payload_root / item-home-relative-to-ToS/source-witnesses / payload / payload_basename`.
The independent original input remains untouched. The existing Item manifest
v1 and legacy `payload/<basename>` grammar remain unchanged.

`recovery_root` is an existing account-owned mode-0700 directory outside the
metadata checkout, canonical source checkout/payload root and input. Its
mode-0700 `<transaction-sha256>/` companion contains a mode-0600
`item-deposit.json`. This is private continuation for the **same transaction
identity**, not another registry or authority. No absolute input/root paths,
copy-inode/ancestor pins, raw private grant or credentials enter the tracked source
transaction journal, public request, human forms or receipt. Recovery requires
the same explicit private companion; missing evidence is not reconstructed
from a same-hash destination.

Both metadata and payload authority expire. The Unix account must match the
grant. A grant is scoped transport permission, not copyright permission;
acquisition, retention, processing and public redistribution remain distinct.

## Request sequence

Requests use `tos_local_item_adoption_command_v1`.

1. `describe` reports the exact selected Edition/version and handler grammar.
2. `prepare-create` supplies `record` (new Item), `claim`, `rights`, `item_kind`,
   `forms` (all current Edition source-copy forms), `item_forms`, `claim_forms`,
   and `reason`. It reads/hash-checks the already obtained file and enumerates
   supported resources, but writes neither bytes nor metadata.
3. Keep that proposal unchanged, set `operation: item.adopt`, choose a stable
   `command_id`, and copy `prepared_fields` into `fields`, `source` into
   `expected_source`, `revision` into `expected_revision`,
   `owner_configuration` into `expected_configuration`. Copy returned
   `expected_dependencies`, `expected_publication`, `inventory`,
   `inventory_limitation`, and `fixity_verified_at` verbatim.
4. An explicit `item.adoption.recover` selects `transaction_id`, `decision`
   (`resume` or `rollback`) and the **current** `expected_configuration`.
   Exact-root/ID/input renewal may permit recovery but never new adoption.

The Item is version 1, `provisional`, with `no_equivalence_claim`, no
supersession, unverified identity variants, and exact `item_manifest_ref`.
The Claim is an initial positive, unreviewed, observed public-metadata
`bibliographic_assertion`; its evidence is exactly the Edition and Item record
paths. “Observed” denotes the declared record link, not accepted bibliography.
Standalone/generic Claim creation or correction cannot bypass the compound
handler, including when the separate payload root leaves a flat Item metadata
package. Changing this native topology Claim needs its compound owner route.

This first handler only serializes a conservative supplied local-only rights
record: exact Item/File scope, unreviewed, not assessed/evaluated/determined,
no permissions or layer clearances, `not_authorized` redistribution and
`local_research_only` derivatives. The supplied rationale/source references
remain attributed input, not a new legal conclusion by this command.

## Two stages, not cross-root atomicity

The byte stage hashes a stable no-symlink input descriptor and ancestor chain,
streams within the exact byte-size bound (hard ceiling 512 MiB), verifies the
original again, fsyncs and publishes with Linux `renameat2(RENAME_NOREPLACE)`.
The destination must be absent; equal hashes never authorize adopting a
foreign inode. A bound partial prefix may resume, while unbound/replaced
partials or changed ancestors fail closed. No byte or original is deleted by
rollback. A crash after file publication is recoverable through the retained
inode and fixity binding.

Only after a completed copy and available inventory does the adapter publish
the Edition delta plus Item metadata through the existing
[selected metadata protocol](SELECTED_METADATA_TRANSACTIONS.md). Its explicit
v2 path profile permits only exact Item siblings `fixity.sha256` and
`forensic-report.md` in addition to ordinary selected JSON/JSONL. Legacy v1
transaction grammar stays unchanged. The Item manifest, rights, provenance,
resource inventory, forms, request/runtime captures and compound receipt are
metadata; payload bytes never enter selected blobs. Colocated payload-bearing
ancestors are not metadata rollback directories.

`item-deposit-receipt.json` exposes only bounded source-safe File identity,
fixity, separate actual observation and copy intervals, original/configuration digest, private-stage digest
and optional renewed-configuration digest. It explicitly does not attest a
metadata commit. The compound transaction separately binds its exact bytes.
The initial observation interval is retained once and supplies the forensic
inventory event; replay/recovery verification does not rewrite that interval.
The private state may say `prepared`, `copying`, `deposited`, or
`rolled-back-retained`; partial/deposited-only states are not acquired Items.
Rollback of an already committed adoption is rejected without changing stage,
source or publication state. Reversing acquired metadata needs another owner
operation, not recovery.

The initial native inventory profile is bounded EPUB only: at most 2048
members, 1 MiB central-directory bytes, 16 MiB per expanded member and 64 MiB
total expansion, and 256 KiB serialized inventory; ordinary single-disk ZIP
using only STORED or DEFLATED compression, no duplicate/unsafe members or
encryption. Other codecs are rejected before decoder creation (an LZMA member
can otherwise request a large dictionary before expanded-byte checks).
Actual member streams are bounded before invoking the existing
`build_file_inventory` enumerator. It emits resource structure/counts and
one-way fingerprints, not source text. Other formats, unsupported ZIP profiles
and parse/budget failures return an explicit limitation, retain the copied
file and private continuation, and create **no acquired Item metadata**. The
next owner is the inventory profile route; no fake empty inventory is emitted.

Before mutation/recovery, current authority, exact metadata snapshot, grammar,
implementation and source dependencies are checked again. Item IDs, File
ownership and Edition backlinks are read from catalog-bound Item manifests
(bounded to 96 current Items), not inferred from a directory or a nonexistent
File catalog. Readers preserve exact Edition and older Expression/Edition
compound history; stable identity never implies bibliographic or textual truth.

The same Unix-account trust boundary remains: hashes and unsigned local
receipts detect mismatches, not malicious self-forgery by that account. This
command is not a filesystem-wide transaction, external execution proof,
current payload resolver, legacy migration, publication gate or source review.
Catalog/derived regeneration, wider validation, checkpoint review and landing
remain separate owner steps. Before source-foundation closure, create the
new Item's exact future-server plan under
`ToS/source-witnesses/server-import/plans/` and its provenance in the existing
`server-import/provenance.jsonl`, following `SERVER_IMPORT_PROTOCOL.md`.
The plan covers the committed manifest and rights digests; current local
Items remain metadata-only with payload transfer and content-bearing server
derivatives prohibited. This owner companion is not part of the adoption
transaction and must not alter its committed bytes or imply server upload.
