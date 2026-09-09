# Native Expression responsibility attachment

`source_responsibility_commands.py`, dispatched through `source_commands.py`,
owns one separately delegated `translated_by` attachment from an existing
Expression to an existing Agent. It uses the canonical relation registry and
`tos_source_relation_claim_v1`; no second semantic store or new Claim schema is
introduced. Its source reader profile is not a flat Claim-create grant.

## Exact source change

The Expression keeps its identity, Work backlink, language, role, identity
posture, notes and all other fields. Its version advances once and
`responsibility_claim_refs` appends exactly one new Claim, preserving earlier
order. Only its record, forms and history are selected. All current source-copy
forms are explicitly rebound; old forms remain retained. Authored parent forms
that are not source copies require their own explicit rebind route.

The new qualified version-1 Claim and its statement form occupy a new standard
`ToS/source-witnesses/relations/<slug>/source-claims.jsonl` home. The existing
Expression's `has_expression` stream and immutable creation package are not
append targets. The Agent is an exact read dependency, never an output.

The Claim remains unreviewed public metadata, with no assessment or supersession
grant. `bibliographic_assertion` and `scholarly_report` use the canonical
registry's existing layers. Epistemic status and polarity remain explicit
authored values; the command does not convert them into positive certainty.
`qualifiers.statement`, `statement_language`, `statement_script` and
`attribution_scope` must all be explicit. The scope distinguishes provider
attribution from a historical printing or later electronic editing when those
limits matter. All other qualifiers and nested context remain intact.

Two Claims may name the same Expression, predicate and Agent. Competing reports
are not rejected by a person-uniqueness or endpoint-pair rule. Claim identities
remain unique and each current Claim must belong to the parent's reference
list. Other responsibility predicates need their actual owner type matrix and
explicit operation before this narrow command can grow into a family.

## Attribution evidence is not endpoint metadata

`evidence_refs` and `counterevidence_refs` cite the attribution's declared
evidence. They are checked against an exact protected allowlist. HTTP(S) URLs
remain addresses, not proof of fetching, source reading, reliability or
independent witnesses. Several pages from one provider do not become several
independent attestations through this operation.

The graph carries each external citation as a Claim-specific occurrence. Its
top-level path, line and canonical hash identify the local citing Claim; the
literal URL lives in `properties.evidence_ref` with `evidence_kind:
external_citation`, `resolved: false`, `remote_content_sha256: null` and
`observation_posture: address_only_not_observed`. Identity includes Claim ID
and literal URL, not Claim version, so another citing Claim is a separate
occurrence. The existing graph node schema retains its non-null local source
hash contract; no remote-content identity or first-class observed Link is made.

The Expression and Agent logical paths, exact record references and raw hashes
are separately bound by the receipt, parent before-package, Agent packet and
dependency snapshot. The endpoint metadata paths are not mandatory attribution
evidence: derived records created from the same report cannot manufacture
additional grounds for it. Explicit public metadata evidence paths are also
supported; anchor and other evidence carrier families require their owner route.

## Protected configuration and request

Configuration schema: `tos_local_expression_responsibility_owner_v1`. In
addition to `uid`, `principal_id`, `maker_type`, `source_root`, `authority_ref`,
`expires_at` and `allowed_operations`, it names:

- exact `expression_id` / `expression_source_path` and existing `agent_id` /
  `agent_source_path`;
- `predicate: translated_by`, new `claim_id`, separate `claim_source_path`,
  and unique `provenance_event_id`;
- bounded `allowed_expression_form_ids`, `allowed_claim_form_ids` and
  `allowed_evidence_refs`.

Requests use `tos_local_expression_responsibility_command_v1`:

1. `describe` returns the exact Expression source/revision/publication, schema
   and type handles, form-selectable source fields, plus `agent_record` and its
   explicit logical-path/canonical-reference/raw-byte binding.
2. `prepare-attach` takes `agent`, `claim`, `forms`, `claim_forms` and `reason`.
   `agent` is the complete current source Agent packet, checked against the
   exact catalog locator and source bytes; it does not create or revise Agent
   truth. `forms` and `claim_forms` are explicit `{form_id, field_id}` selections.
   Preparation makes no source write and returns the required bindings.
3. `expression.responsibility.attach` submits the same inputs, `command_id`,
   returned `prepared_fields` as `fields`, `source` as `expected_source`,
   `revision` as `expected_revision`, `owner_configuration` as
   `expected_configuration`, and `expected_dependencies` / `expected_publication`.
4. `expression.responsibility.recover` requires its current explicit grant,
   exact `transaction_id`, `decision: resume|rollback`, and
   `expected_configuration`. A recovery-only renewal cannot start a new Claim.

The command discovers selected records and existing translator Claims through
exact current catalog routes. Catalog generation is a separate post-publication
step before the next growth operation. No descendant enumeration is needed.

## Shared publication and retained reading

The adapter uses the same common-lock, selected-metadata publication and
current-authority recovery as
[native Work/Expression growth](NATIVE_WORK_EXPRESSION_GROWTH.md). Shared
catalog, form, dependency and lifecycle mechanics live in
`source_compound_commands.py`; each adapter owns its distinct scope, typed
delta and complete before/after reconstruction. Old Work/Expression receipt
serialization and historical replay remain unchanged.

The new home retains the exact request, runtime capture, version-2 serialization
provenance and `responsibility-attachment-receipt.json`. This is unsigned buffer
capture, not proof that the caller read a source. Prepared or orphaned evidence
does not establish a committed attachment. Current authority, expiry and exact
dependencies are rechecked during new publication and recovery; an external
third state is never overwritten by resume or rollback.

Historical verification requires the exact committed attachment plan, immutable
capture bytes and its transition in the current Expression lineage. A later
descriptive Agent correction must preserve the original raw bytes in continuous
owner history; selected corrections additionally require their committed
transaction. An arbitrary archive or matching ID cannot substitute for that
history. Ordinary separately delegated Claim correction remains available:
the existing Claim revision reader must reconstruct the continuous current
stream from the exact initial compound stream. The initial receipt never
pretends to certify corrected wording or later assessment.
The current generic Claim-writer URL branch is limited to descriptive correction
of such a verified native `translated_by` Claim. It explicitly labels the
proposed citation binding `candidate_claim`: its hash identifies the proposed
Claim declaration, not pre-existing source input or remote contents. Non-URL
writer bindings and older receipt reconstruction remain unchanged. Qualification
is checked again before publishing the Claim successor; completed attachment
replay reads that current verified successor while matching its original request.

The foundation validator joins unchanged legacy responsibility streams and only
verified native translator attachments, then checks exact parent/ref closure.
It does not turn the union into acceptance of the attribution. Source review,
translation judgment, equivalence, rights, publication and canon retain their
actual owners.

## Verification

`mechanics/growth-cycle/tests/test_source_responsibility_commands.py` exercises
the actual adapter and transaction engine on bounded synthetic metadata,
including current grants, competing assertions, process death/recovery,
historical raw-byte resolution and independent Claim corrections. It does not
write the corpus or treat green mechanics as source admission.
