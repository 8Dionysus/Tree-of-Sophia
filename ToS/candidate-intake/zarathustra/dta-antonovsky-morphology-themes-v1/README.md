# Zarathustra DE/RU morphology and theme candidates v1

This intake pass is the first content-facing layer above the complete technical
DE/RU lexical and paragraph-alignment foundation. It proposes overlapping form
families, bilingual recurrence neighborhoods, and typed relations. It does not
author a lemma, lexeme, sense, sign, concept, semantic relation, graph edge, or
canon object.

The tracked JSONL files withhold exact German and Russian strings. Readable
forms, provisional display hints, competing analyses, and agent-audit examples
remain in the ignored mode-`0600` private analysis named by `manifest.v1.json`.
Opaque candidate IDs are issued independently of mutable forms, stems, labels,
translations, and cluster hints; the issuance bindings are mechanical closure
keys, not identities or linguistic judgments.

Candidate status has a narrow meaning:

- `proposed`: the declared mechanical gates passed;
- `ambiguous`: a useful but competing or cross-alignment challenger;
- `deferred`: the family or neighborhood is too broad or weak for positive use.

`proposed` still means unreviewed candidate. All rows have empty review refs,
`accepted: false`, and no graph or canon effect.

`independent-agent-audit.v1.json` records two whole-corpus challenger runs. Its
German casing controls and Russian dictionary/quality controls were integrated
before the final build; the audit is explicitly not a human linguistic or
semantic review.

The implementation and focused validation are owned by
`scripts/build_zarathustra_morphology_theme_candidates_v1.py` and
`tests/test_zarathustra_morphology_theme_candidates_v1.py`; execute them
through the [ToS validation routes](../../../VALIDATION.md).

## Native technical input profiles

The native `tos zarathustra-morphology-theme-candidates-v1 --source-root PRIVATE_CARRIER --scratch-bytes RESERVED_REMAINING_BYTES --plan-ref ROOT_RELATIVE --build` route selects an explicit technical input profile; omitting `--plan-ref` retains the authenticated v1 plan. Custom profiles require a current-user-owned mode-`0700` carrier root and mode-`0600` plan. Use a separate private carrier and preserve the old plan, outputs and issuance in their original custody. The scratch value is an explicitly reserved remaining quota after the carrier baseline, retained outputs and metadata; it does not grant storage. The selector grants no source, semantic, rights or canon admission.

A profile requires a distinct `plan_id`, `status: proposed-technical-input-profile-successor`, `frozen_at: null`, and `input_profile_lineage` with `profile_version >= 2`, `supersedes_plan_ref` and `supersedes_plan_sha256` matching the authentic v1 plan. Only input `sha256` values may change; input membership, refs and all semantic fields, methods, thresholds, output declarations and authority remain exact. Unknown changes fail closed. Selected plan ref and digest are emitted in provenance and manifest. Full bindings must match the retained v1 issuance, including in preview; custom profiles reject `--issue-identities`. Outputs retain the existing filenames inside the selected carrier. Technical profile proposal and boundary review precede scoped mechanical admission; execution does not perform that admission.

The previous private Parallel analysis and coverage receipt must match the digest and byte size declared by the authenticated selected Parallel manifest. The private analysis requires mode `0600`; coverage requires `0644`. Each is checked and parsed through one retained file descriptor. Missing or stale unmanifested analysis is refused.

The local `0700`/`0600` privacy guard does not prove distinct custody. The controller and explicit admission must fence the selected carrier from original source and historical output custody.
