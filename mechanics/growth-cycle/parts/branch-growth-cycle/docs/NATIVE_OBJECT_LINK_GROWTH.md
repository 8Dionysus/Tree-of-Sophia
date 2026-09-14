# Native object-Link growth

`object.link.create` publishes one native Link and one qualified association
Claim in separate new source homes under one selected-metadata transaction.
The existing subject is read-only: Work, Expression, Edition, Collection, Item
or physical Artifact. An Artifact retains `artifact_id` and its native witness
schema; it is never recast as a CorpusRecord or an ArtifactLink entity.

The independent owner grant is `tos_local_object_link_create_owner_v1`.
It specifies the exact source subject ID/path/type, new Link and Claim IDs and
paths, one predicate, provenance event ID, URI, observation ref, disjoint Form
allowlists, evidence allowlist, account, maker, principal, authority and expiry.
It grants `object.link.create` and optionally `object.link.recover`. Existing
Corpus, flat Claim and descriptive-revision grants gain no creation power.

Commands use `tos_local_object_link_command_v1`:

- `describe` exposes the exact selected scope and supported source contracts.
- `prepare-create` takes `subject`, `link`, `claim`, `forms`, `claim_forms` and
  a bounded authored `reason`. It binds the current catalog, subject bytes,
  metadata evidence, grammar and implementation; it writes nothing.
- `object.link.create` adds `command_id`, `expected_configuration`,
  `expected_dependencies` and `expected_publication` from preparation.
- `object.link.recover` selects an exact pending `transaction_id`, an explicit
  `resume` or `rollback` decision and the current `expected_configuration`.
  Renewal does not rewrite the original maker or capture. Changed dependencies,
  a mismatched scope or foreign third-state bytes prevent either action.

New homes are exactly `ToS/source-witnesses/links/<home>/link.json` and
`ToS/source-witnesses/relations/<home>/source-claims.jsonl`. Each has its own
source-copy Forms. The Claim home retains immutable request, runtime,
serialization provenance and compound receipt. Publication touches no subject
file, revision, descendant, payload, catalog, rights or external provider.
Shared [selected transaction mechanics](SELECTED_METADATA_TRANSACTIONS.md)
own byte movement and reader barriers, not the truth of the association.

`tos_source_link_v1` stays unchanged. New association Claims use the additive
[`tos_object_link_claim_v2`](../../../../../ToS/contracts/object-link-claim-v2.schema.json)
contract on the existing shared SourceClaimRecord. The four exact reified
predicates are `described_by`, `metadata_at`, `downloadable_at` and
`rights_statement_at`. Their domain is the six subject types above and their
range is Link; the older direct source-navigation `link-access` projection
and `tos_object_link_claim_v1` remain intact.

The Claim carries `forensic_observation` as its layer and a caller-authored
qualified statement with language, script, link role and explicit
`availability_is_rights_conclusion: false`. A reported observation is not a
newly performed remote observation. Source URLs are addresses, never evidence
that this command fetched their contents. A rights-statement link does not
grant rights, and download availability does not imply redistribution permission.
No source content, philosophical assessment, identity equivalence, publication
authorization or canon admission follows from serialization.

Independent native Link revision changes only its allowed descriptive fields;
URI, observation, identity, association and original provenance remain fixed.
The exact current and historical Link versions use MetadataVersionReader.
The qualified Claim uses the ordinary Claim correction/Form/history route;
its native compound origin and qualification must remain verifiable. Old Form
and source versions stay historical, not current assessments. A forged receipt,
partial capture, missing creation baseline or uncommitted correction is not
accepted as a native origin.

After a successful creation, correction or rollback, rebuild source-owned
derived catalogs before preparing a new catalog-dependent operation. Historical
retry returns the exact original receipt without republishing it. This is a
bounded metadata writer, not an indexed or incremental graph engine.

Focused validation:
`mechanics/growth-cycle/tests/test_source_link_commands.py`, shared discovery,
Claim/metadata history tests, source-witness foundation and the explicit
semantic-registry transition baseline. All fixtures are synthetic; green
mechanics never constitutes a real source assessment or rights acceptance.
