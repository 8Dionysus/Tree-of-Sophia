# Native Expression / Edition growth

`source_edition_commands.py` implements the separately delegated
`expression.edition.create` and `expression.edition.recover` operations through
the existing `source_commands.py` front door and shared
`source_compound_commands.py` publication lifecycle.

## Source boundary

This first operation selects one existing Expression under its exact read-only
Work and creates one provisional Edition plus a distinct `embodied_by` Claim.
Only the Expression's version and appended `embodiment_claim_refs` change.
All other fields, lists, source-copy form history and earlier compound receipts
remain retained. Work metadata, existing editions and descendants are not
rewritten. The new Claim lives inside the new Edition home, never in the
Expression's immutable `has_expression` source carrier.

An Edition identifies a published or edited manifestation, not an acquired
copy or downloaded file. The caller owns its explicit referent and evidence.
Nothing infers a historical printing from an ebook identifier, a publication
year from a download/update date, or translator responsibility for later
editorial changes. Metadata, source visibility and serialization do not accept
bibliographic identity, equivalence, rights, publication or canon.

The initial Edition is version 1, provisional, with `no_equivalence_claim`,
no supersession, one explicit `embodies_expression_refs` backlink, unverified
identity variants and empty publication/provision, responsibility and exemplar
claims. Its source-owned name, optional edition statement and qualified notes
remain supplied input. The new Claim is initial, positive, observed, unreviewed
public metadata, with no assessment admission. `observed` describes the declared
record link only. Its evidence is exactly the two linked metadata paths and
its qualified statement has explicit language and script.

One Expression is this operation's bounded write set, not global bibliographic
cardinality. Existing Editions may embody several Expressions or belong to a
collection; an Edition need not yet have an Item. Attaching an existing Edition
or creating an Item/File requires a separate owner route and grant. Do not
duplicate an Edition merely to fit this creation operation.

## Exact delegation and ABI

The protected configuration schema is `tos_local_expression_edition_owner_v1`.
It contains `uid`, `principal_id`, `maker_type`, `source_root`, `authority_ref`,
`expires_at`, `allowed_operations`, and these exact targets:

- Read-only `work_id` and `work_source_path`.
- Existing `expression_id` and `expression_source_path` under that Work.
- New `edition_id` and `edition_source_path`, exactly
  `<expression-home>/editions/<slug>/edition.json`.
- New `claim_id`, `provenance_event_id`, and distinct bounded
  `allowed_expression_form_ids`, `allowed_edition_form_ids`, `allowed_claim_form_ids`.

Requests use `tos_local_expression_edition_command_v1`:

1. `describe` returns the selected Expression, revision, publication token,
   source fields and existing Expression/Edition/Claim owner profile handles.
2. `prepare-create` accepts `record` (the Edition), `claim`, `forms` (the
   Expression), `edition_forms`, `claim_forms`, and `reason`. It writes no source
   and returns `prepared_expression`, `prepared_edition`, `prepared_fields`,
   forms and exact dependency/publication bindings.
3. `expression.edition.create` adds `command_id`, those prepared `fields`,
   `expected_source`, `expected_revision`, `expected_configuration`,
   `expected_dependencies`, and `expected_publication` to the same proposal.
4. `expression.edition.recover` names the pending `transaction_id`, explicit
   `decision` (`resume` or `rollback`), and current `expected_configuration`.
   A recovery-only renewal never authorizes a new creation.

All current Expression source-copy forms must be explicitly rebound. New
Edition name and Claim statement forms are explicit source copies; each
language comes from its source field. Authored forms require their own route.
Keep confidential owner paths, grants and credentials out of public inputs;
the public receipt retains only bounded scope, principal and authority refs.

## Retained evidence and recovery

Preparation binds current catalog identity, the Expression's unique verified
Work origin, and its existing Edition forward/backlink closure. The exact
catalog must bind the current publication token. The adapter reads selected
catalog routes, not arbitrary descendants. New/resumed publication rechecks
current account delegation, expiry, grammar, implementation and dependencies.

The parent three-file delta and new Edition's eight metadata/form/capture/
receipt files use the [selected publication protocol](SELECTED_METADATA_TRANSACTIONS.md).
The pending barrier closes participating reads. Recovery reconstructs all
original before/after bytes and refuses foreign or third-state changes.
Immutable predecessor archives preserve earlier source language and receipts.

`verify_compound` admits only an exact committed transaction, immutable Claim,
request/environment/provenance captures, the transition in current Expression
history and the original Edition bytes in current or verified retained history.
An orphan archive is not history. Exact completed retries remain valid after
catalog regeneration and later sibling growth, subject to current authority
and verified lineage. Unsigned runtime capture is not external execution proof.

Foundation closure joins verified native `has_expression` and `embodied_by`
Claims with unchanged legacy topology streams. The old batch counts and raw
digests retain their own historical scope; altered legacy Expression inputs
resolve only through their exact original path and committed retained bytes.
The read-only metadata version reader now resolves Edition records; this does
not enable generic Edition creation or revision grants. Earlier Work/Expression
and translator receipts remain independently verifiable after this append.

Synthetic command tests protect these boundaries without changing the corpus.
Catalog regeneration and wider downstream validation remain separate owner
steps; a successful command reports `grants_admission: false`. No Item, File,
payload acquisition, extraction, OCR, text-layer or rights transition is performed.
