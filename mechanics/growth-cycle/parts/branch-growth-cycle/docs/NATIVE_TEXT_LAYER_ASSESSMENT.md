# Exact text-layer comparison and current quality use

This is a confidential source-owner extension of the existing assessment
journal, not a public graph reader or a second assessment engine. It uses
`assessment_journal.py --owner-config /absolute/protected-owner.json` and the
existing `tos_local_assessment_command_v1` request envelope. Source creation
remains with [native layer construction](NATIVE_TEXT_LAYER_CONSTRUCTION.md).

## Protected configuration v5

`tos_local_assessment_owner_v5` retains all v4 fields and adds exactly:

| Field | Contract |
| --- | --- |
| `native_text_layers` | At most eight exact selections described below. Each selected layer has its own entry in `subjects`. |
| `quality_dependencies` | Object keyed by configured target ID, with at most eight `{layer_id, use}` entries per target. No duplicate layers, extra groundings or self-dependencies. |

Each layer selection has `binding`, `origin_id`, `source_access` and
`payload_access`. The binding is `tos_native_text_layer_binding_v1`: exact
layer metadata plus its Work/Expression/Edition/Item source locators. It does
not require a predecessor TextUnit. The target's assessment reference is the
canonical digest of the unchanged raw layer, not the metadata file digest.

`source_access` has exactly `read_scope`, `access_allowed: true` and an
independently issued `authority_ref`. A `metadata_only` selection requires
`payload_access: null`. An `exact_owner_local` selection requires a separate
current payload grant with `read_scope: exact_acquired_file`,
`access_allowed: true`, `authority_ref`, timezone-aware `expires_at`, absolute
`payload_root` and exact `byte_size`. The acquired payload root must be
separate from both public metadata and private derivative roots. Grants are
checked before source I/O; every selected layer's exact source and layer rights
are checked before opening any selected original or representation bytes.
The retained construction configuration is source data, not reading authority:
its historical expiry does not replace or invalidate a separately issued
current reading grant.

Each layer subject has the ordinary exact `record`, `risk`, `languages`,
`maker_id`, `requested_use` and `access_allowed` fields. Its assertion layer
is `textual_observation`, language is the actual single source language, and
maker is the actual extraction maker. The four separate requested uses are:

- `text-layer:citation`
- `text-layer:linguistic-analysis`
- `text-layer:semantic-analysis`
- `text-layer:search-projection`

Policy v3 supplies `text-layer-quality` for low/moderate risk and
`text-layer-quality-high` for high risk. The latter requires two independent
reviewer groups. The existing engine still requires source-visible judgment,
current delegated authority, calibrated competence, independent execution
binding, exact evidence and counterevidence posture. No command or comparison
grants those qualifications. One configuration selects one use per layer;
separately scoped configurations may share its immutable journal.

## Compare, assess, inspect

`describe` returns exact target, policy, comparison and required-dependency
references, current readiness, supported operations and owner snapshot. It
does not return source text. The independently selected exact-read configuration
authorizes reading and rechecking bytes while computing that snapshot.

`read-layer-comparison` uses the ordinary non-describe request fields:
`schema_version`, `operation`, `subject_id`, `expected_subject` and
`expected_snapshot`. It returns the selected comparison record and payload,
including the bounded original UTF-8 XHTML member, selected expected text,
actual representation, selector, policy, maker and input fixity. Its entire
return is local-only and publication remains unauthorized. The caller must not
forward it to a public projection. Metadata-only selection cannot use this
operation or append a quality assessment.

`append` uses unchanged command/event/batch v1 fields. The quality reviewer
must cite the comparison as evidence; a matching extraction or a successfully
verified hash is not itself an assessment. The first adapter supports only
the declared structural XHTML extraction profile. It rejects unsupported
OCR/correction/normalization chains instead of treating derived-byte fixity as
original fidelity. Original and selected-output disagreement remains visible
in the comparison and prevents usable quality admission. It does not prevent
a qualified rejection, dispute, deferral or withdrawal based on that available
exact comparison. The adapter separates source availability from positive-use
eligibility; `describe` exposes both comparison `ready` and
`positive_use_allowed`. Metadata-only selection still cannot append any decision.

## Dependent claims and forms

The adapter derives the required layers from exact selected native grounding,
including the declared Claim source closure and human-form context. Protected
`quality_dependencies` must match that closure, not merely name a convenient
quality journal. Textual/forensic observations, bibliography and scholarly
reports require citation quality; linguistic and translation judgments require
linguistic-analysis quality; semantic interpretations require semantic-analysis
quality. A human form follows its subject's judgment layer. Search-only quality
does not qualify a semantic or citation route.

Before evaluating the dependent target, the adapter reads the current quality
admission under the same target/dependency journal locks. It derives a
`tos_native_text_layer_quality_basis_v1` record containing exact layer,
comparison, purpose, content scope, policy, active assessment references,
current status and inherited limits. `describe.required_admissions` exposes
its reference and permission, not confidential text. This record must occur in
dependent assessment evidence. A human form must bind its whole current basis
as context as well as its subject. Caller-supplied basis records cannot shadow
this runtime-owned result.

Withdrawal, revocation, scope expiry, changed comparison or replaced quality
evidence makes the affected dependent result unusable without deleting source
or history. A renewed positive quality event changes the exact basis; it does
not automatically resurrect a dependent assessment of another basis. Merely
appending an unrelated journal event does not change semantic evidence. Current
quality and target history are rechecked before returning; historical receipts
remain statements about their original commit, not current permission.

Closing the quality-use gate does not prevent an authorized negative review or
explicit withdrawal of a dependent judgment. These acts still require all
exact current source and quality-basis evidence and the ordinary supersession
authority. Every required original comparison must remain available; a
metadata-only dependency cannot qualify even a negative append. The current
quality condition gates positive use, not the ability to record why such use
must stop.

## Bounds and compatibility

Selections conservatively charge original `byte_size` per layer, up to 16 MiB
in total. The reader bounds metadata closure, original extraction and each
comparison (1 MiB); oversized comparisons fail closed and need a narrower
supported source route, never truncation. Locks use a shared bounded deadline
and stable order. They coordinate current reads and one target append, not an
atomic multi-target write transaction.

Configurations v1–v4 retain their existing meaning. No legacy layer's
`human_review_performed`, review status or source bytes are rewritten. Public
assessed graph builds and Sign issuance still reject confidential v4/v5
configurations before opening the private source context. This route grants
neither publication nor diplomatic fidelity, full translation verification or
canon. Source comparison tests and synthetic journal tests are separate from
actual language competence and real source-visible quality assessment.
