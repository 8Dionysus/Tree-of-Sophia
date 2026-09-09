# Declared source profiles in the assessment command adapter

Date: 2026-09-07. Implementation parent: `913f85d34b0c7137416d7b9fb12f9d2884419a0c`.
Owner: Growth assessment command adapter; source semantics remain with the
existing source metadata/Claim profiles and assessment policy.

The v2 owner configuration can now select declared metadata and Claim records
without copying them into inline configuration or crawling a corpus. All
endpoints of a declared relation must be selected explicitly. Their native or
declared schema, ID family, basename and inherited type are checked before the
relation's concrete domain/range. Maker and assertion layer remain source-owned.
Consumed registry/schema bytes join the exact source/configuration snapshot
and are exposed as `source_contracts`. Current profile changes stale commands;
profile resolution does not grant competence or admission.

## Test-first observations and executed checks

The initial real Letter/three-Claim selection failed with unsupported identity
family. The shared profile adapter then read all seven explicitly selected
records, preserving full payloads and their unreviewed posture in 0.136 seconds.
The no-crawl guard rejects `Path.rglob` calls during this operation.

Two subsequent negative tests exposed genuine gaps: a Corpus envelope could
masquerade as a Letter, and a shared-stream Claim could bypass the declared
schema by using the old Claim-packet schema name. Native endpoint validation
and profile-first file dispatch now refuse both. Closure negatives also reject
missing endpoints, wrong domain/range, undeclared predicates/versions, changed
layers, accepted source flags and nonpublic data.

The command test binds all selected records and consumed contracts, proves
that changed metadata/Claim/Corpus schemas or relation registry invalidate the
old snapshot, rejects configured maker/layer substitution, and grants no
admission without assessment. A new synthetic Document subtype and its Work
relation are declared solely in fixture data; the same reader validates them
and preserves unknown instruction-like extension values. That fixture is not
a historical assertion. It passed in 0.259 seconds.

All 119 Growth tests passed in 34.404 seconds. This protects the existing
command, journal, form and revision behavior as well as the new source adapter;
it is not a quality measurement or competence claim about a real agent.

## Boundary review and remaining work

Source return, full field retention, source/profile separation, no executable
source prose, exact source-owned scope and no implicit authority: yes.
Existing source packages, assessment history and legacy judgments are not
rewritten. Unknown or incompatible selected records fail explicitly rather
than being discarded. Source and contracts share an 8 MiB unique-input budget;
profile readers also keep their own per-file bounds. Same-account hostile or
uncooperative concurrent editors remain outside this local trust boundary;
the caller still must keep a coherent source snapshot stable.

This does not implement native non-Corpus artifact assessment inputs, source
provenance/rights admission, real assessor calibration, model attestation,
multi-subject assessment transactions, or consumer integration of admission.
The next owner work is a source-visible, separately authorized non-self
assessment with measured competence. Reader rollback must preserve newly
written source Claims and any later assessment history. CI, merge, deployment,
D1/UI acceptance and the complete nine-profile foundation remain unproven.
