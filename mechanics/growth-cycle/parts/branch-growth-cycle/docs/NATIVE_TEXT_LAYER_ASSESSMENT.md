# Exact text-layer comparison and current quality use

This confidential source-owner extension uses the existing assessment journal
through `assessment_journal.py --owner-config /absolute/protected-owner.json`
and the `tos_local_assessment_command_v1` request envelope. Source creation
belongs to [native layer construction](NATIVE_TEXT_LAYER_CONSTRUCTION.md).

## Protected configuration v5

`tos_local_assessment_owner_v5` retains all v4 fields and adds exactly:

| Field | Contract |
| --- | --- |
| `native_text_layers` | At most eight exact selections described below. Each selected layer has its own entry in `subjects`. |
| `quality_dependencies` | Object keyed by configured target ID, with at most eight `{layer_id, use}` entries per target. No duplicate layers, extra groundings or self-dependencies. |

Each layer selection has `binding`, `origin_id`, `source_access` and
`payload_access`. The `tos_native_text_layer_binding_v1` binding selects exact
layer metadata and its Work/Expression/Edition/Item source locators directly.
The target's assessment reference uses the canonical digest of the unchanged
raw layer; raw metadata-file fixity has its own digest.

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
maker is the actual selected layer maker. The four separate requested uses are:

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
verified hash is not itself an assessment. The structural-extraction adapter
retains its original v1 comparison contract and exact XHTML profile.
Original and selected-output disagreement remains visible
in the comparison and prevents usable quality admission. It does not prevent
a qualified rejection, dispute, deferral or withdrawal based on that available
exact comparison. The adapter separates source availability from positive-use
eligibility; `describe` exposes both comparison `ready` and
`positive_use_allowed`. Metadata-only selection still cannot append any decision.

### Native derived-layer comparison

The same v5 selection can address a layer created under the independently
versioned `tos_local_text_layer_derive_owner_v1` profile: explicit correction,
Unicode normalization, or recording supplied manual/model transcription or
OCR text. The source view is still one exact acquired EPUB/XHTML member and
one supported structural anchor. An image/PDF or another anchor needs a
separately implemented source renderer; its File digest cannot substitute for
source visibility. This does not run an OCR or transcription provider.

`native-text-layer-derivation-comparison.schema.json` adds a comparison route
alongside the retained extraction schema. The returned
`tos_native_text_layer_derivation_comparison_v1` record contains the original
member and selected character data, complete oldest-to-newest layer records
and representations, exact policies/configuration digests, declared operations
and unverified supplied-producer metadata. Source and output can intentionally
differ. Every native delta is independently replayed, and
`source_text_equals_output` records byte comparison. `positive_use_allowed`
identifies evidence from which an authorized competent reviewer can reach a
positive or negative judgment. The assessment engine requires this exact
comparison in review evidence and applies current purpose, risk, authority and
competence policy.

Each predecessor's current rights are checked in the metadata pass before any
selection's content is opened. Retained construction grants, even expired or
revoked, remain source data; only independently selected current reading
authority authorizes this read. Grant validity and a shared cooperative time
budget are rechecked around every resolver file read, including recursive
lineage and snapshot reads, and between parsing and comparison-building steps.
An expired grant stops the next content read rather than waiting for the next
top-level layer. A valid predecessor assessment does not admit its successor;
the new layer needs its own exact comparison, assessment and scoped quality
basis. Withdrawal and subsequent reassessment follow the existing dependent
quality route below, without restoring obsolete dependent evidence.

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
dependent assessment evidence. The explicit assessed source-copy/freeform
consumer retains the whole current basis as owner-resolved context and an exact
dependency alongside the immutable authored subject/field bindings. It requires
current admission of the form itself; ordinary source-only readiness remains
`admission: null`. Changing the basis requires renewed form review but does not
rewrite unchanged wording. An already authored explicit basis ref still has to
match exactly and remains visibly stale after replacement, even if a new review
otherwise qualifies. Caller-supplied basis records cannot shadow this
runtime-owned result. See [assessed form materialization](../README.md#assessed-form-materialization).

A v5 Claim form additionally resolves current parent Claim admission under an
explicit exact same-use parent scope and the combined form/parent/layer locks.
The complete `subject_assessment` companion retains current state, limits and
historical withdrawals; its observed head/current result joins the owner
snapshot and is checked before return. Display distinguishes initial source
posture from current parent assessment. Qualified display remains possible
with an unreviewed or negatively assessed parent, with that state and its
restrictions included as mandatory context. Form admission covers the form's
selected use; Claim assessment and the layer quality basis retain their
separate evidence and use conditions.

Withdrawal, revocation, scope expiry, changed comparison or replaced quality
evidence makes the affected dependent result unusable while source and history
remain retained. A renewed positive quality event changes the exact basis and
requires a corresponding dependent assessment. Unrelated journal events leave
semantic evidence unchanged. Current quality and target history are rechecked
before returning; historical receipts retain their original commit scope.

An authorized negative review or explicit withdrawal remains available after
the quality-use gate closes. These acts require exact current source and
quality-basis evidence and ordinary supersession authority. Every required
original comparison must remain available, including for a negative append.
This preserves the evidence route for recording why positive use must stop.

## Bounds and compatibility

Selections conservatively charge original `byte_size` per layer, up to 16 MiB
in total. The reader bounds metadata closure, original extraction and each
comparison (1 MiB); oversized comparisons fail closed and need a narrower
supported source route, never truncation. A derived comparison contains at
most 16 lineage layers within the same aggregate content/metadata budgets.
One comparison construction or later snapshot call has a 30-second cooperative
budget; nested checks cannot renew that call's deadline. This is not a
preemptive timer for one filesystem or parser operation. Locks use a shared bounded deadline
and stable order. They coordinate current reads and one target append, not an
atomic multi-target write transaction.

Configurations v1–v4 retain their meaning, source bytes,
`human_review_performed` fields and review status. Public assessed graph
builds and Sign issuance reject confidential v4/v5 configurations before
opening the private source context. Publication, diplomatic fidelity, full
translation verification and canon follow their owner routes. Synthetic tests
cover comparison and journal mechanics; real source-visible quality assessment
requires applicable language competence.

The separate [image/OCR comparison v6](NATIVE_IMAGE_OCR_ASSESSMENT.md) accepts
one authenticated retained PDF-page raw OCR layer or one explicitly selected
operator-created synthetic PNG result. It leaves this v5 EPUB comparison
unchanged, separates local reading from assistant disclosure, and carries
method-specific limits into current layer admission and dependent quality.
