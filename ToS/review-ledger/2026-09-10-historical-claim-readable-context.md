# Historical Claim context vocabulary review

Date: 2026-09-10. Reviewer: `model:codex`. Base:
`088538c2047277e25b0dc15722eb5dee1c3a1eb2`.

## Observed gap and bounded change

The actual local reader at source revision
`a9c4e82415da6b8e680bc3c70b67d5daa4bd43ca3476f4b43500d2dd3e38dc66`
returned the exact historical Claim
`tos.claim.jenseits-1886-commission.date`, but every one of its 15 raw Claim
fields was unclassified in readable context. The already supported assertion
context used known labels for the same fields. Its separate `claim_ref` was
also unclassified. Inspection returned HTTP 200 and 183,840 UTF-8 bytes,
including the five related relations; this is a bounded observation, not a
latency, scaling or compact-packet measurement.

The owner vocabulary now explicitly recognizes `tos_historical_claim_v1`,
whose actual schema delegates the common Claim fields to the same owner
contract as native Claims. Its presentation version advances from 1 to 2 and
the containing entity registry from 36 to 37. The existing finite field rules
remain unchanged. The new `claim_ref` label is governing, not technical:
identifying the Claim addressed by a context is not an endorsement, identity
merge or current admission. This rule also serves already returned assertion
contexts.

No source Claim, date, calendar, wording, HumanForm, assessment, rights record,
graph artifact, UI file or runtime state was changed. Unknown schemas do not
inherit field rules, and unknown extensions and enum values stay visible.
The full source record and its exact bindings remain available. The change
does not deduplicate raw and assertion contexts, create a short title or fill
a missing description. Those are separate outstanding human-form requirements.

## Verification and considered review

- `access/tests/test_readable_context.py` passed in the owner-admitted
  `tos-root-readable-context-20260910.service`; exit 0, 122.9 MiB peak,
  zero swap. Added durable coverage checks the real retained historical
  carrier without changing it, exact date/qualifier preservation, mechanical
  versus governing categories, and an explicit Claim-context reference.
  Synthetic unknown-extension and future-schema controls remain test data.
- `scripts/validate_semantic_registry_transition.py` passed with the exact
  baseline above in `tos-root-context-registry-20260910.service`; exit 0,
  42.5 MiB peak, zero swap.
- `git diff --check` passed. The source contracts, changed vocabulary,
  doctrine and test diff were manually reviewed against the ToS review
  checklist. Source traceability, layer distinction, context preservation,
  version evolution, plurality and uncertainty preservation: yes. New
  semantic admission, identity migration, source translation, canon,
  publication, lived-witness or cross-owner authority: not applicable.

The real UI reread on the old snapshot confirmed exact source-date wording,
not complete human-form acceptance. Its oversized navigation heading and
missing substantive description remain open. This source change must be
integrated and the affected read model refreshed before any runtime improvement
is claimed. Full corpus generation, Worker/D1 execution, browser acceptance,
CI, merge and deployment were not performed by this bounded change.

Next owner: foundation integration for the vocabulary dependency refresh and
actual reader reread; source HumanForm growth for substantive compact wording.
No new durable decision is needed: the existing finite, explicitly versioned
context vocabulary law is applied without changing its authority boundary.

## Catalogue-schema continuation

After the separate Document catalogue grammar was reviewed and integrated at
`43a9b45f1580b59dd596aaf79990b54115626d4a`, its exact
`tos_document_catalogue_claim_v1` selector was added to the same vocabulary.
Presentation 2 → 3 and entity registry 37 → 38 retain explicit invalidation.
No finite field rule is broadened. The qualified catalogue attribution and
null calendar/numbering remain source values, not inferred historical time.

The whole focused readable-context module passed again in
`tos-root-catalogue-context-20260910.service` (exit 0, 112.5 MiB peak, zero
swap). A synthetic new-profile context checks exact field-selection wording,
null calendar/numbering, a false dispatch qualification and unchanged input.
It is not source evidence. Registry transition against the exact parent above
passed in `tos-root-catalogue-context-registry-20260910.service` (exit 0,
41.9 MiB peak, zero swap). Manual source-boundary review finds no change to
admission, date meaning, language authority or unknown-field treatment.
Worker/D1 role support is being implemented separately; neither this selector
nor the local checks establish its execution or the full G5 requirement.
