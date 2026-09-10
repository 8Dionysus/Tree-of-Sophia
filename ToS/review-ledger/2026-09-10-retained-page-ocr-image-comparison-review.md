# Retained-page OCR and separate image-comparison source return

Review date: 2026-09-10 UTC. Reviewed ToS implementation:
`3ffabb9e422510ccfe2e5d44927c41980bab78d4`, following
`4b21bdaf51f5af855cd8003bd5bc593f531f7ddd`.

This is a public-safe return of already completed local canaries, not another
execution, textual assessment, release or acceptance event. It supplements
the [earlier recording review](2026-09-09-native-owner-ocr-recording-review.md)
with actual execution evidence and the separate
[image-comparison contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_IMAGE_OCR_ASSESSMENT.md).
Raw payloads, OCR strings, private locations, grants, signer material and
operator/session identifiers remain with their existing custodians. The
identities below are correlation handles, not public payload access or a
replacement for authenticated owner evidence.

## Exact owner chain

| Owner and role | Exact source reference |
| --- | --- |
| `abyss-stack`: retained-page producer, current verification capture and receipt verification | `commit:48653434bef6eddec274169c7e6793df21cbafc1`; `mechanics/inference-pilots/parts/tos-foundation-lab/retained_page_tesseract_ocr.py` |
| `abyss-stack`: previously completed synthetic-image OCR producer | `commit:ad94f1ac7b7a48e98e599cca1f9831825ff63235`; `mechanics/inference-pilots/parts/tos-foundation-lab/bounded_tesseract_ocr.py` |
| `abyss-machine`: artifact policy, exact registry/subject admission and host resource boundary | `commit:ec1bd3199f1fc508f34e088287d9520167b5d777`; `manifests/artifact_signature_policy.manifest.json` and `manifests/artifact_bundles/abyss_stack_tesseract_ocr.bundle.json` |
| `Tree-of-Sophia`: native receipt consumer, separate layer/unit construction and read-only image comparison | `commit:3ffabb9e422510ccfe2e5d44927c41980bab78d4`; `scripts/native_owner_ocr.py`, `scripts/native_text_binding.py`, and `mechanics/growth-cycle/parts/branch-growth-cycle/scripts/native_page_ocr_assessment.py` |

The historical execution used the retained-page producer above, not the
earlier synthetic producer. Native recording called the fixed owner verifier;
ToS did not execute OCR. Current installed policy/manifest bytes had been
verified against the named machine source. Existing signed synthetic evidence
was retained without rebinding it to the later producer. This note does not
change any source, policy, grant or signer.

## Artifact and consumer admission evidence

For the historical canary, artifact class was `ai_model_or_runtime_bundle`,
source was the exact `abyss-stack` commit above, access policy was
`host-local-ocr-runtime-canary-v1`, and consumer reference was
`Tree-of-Sophia:bounded-ocr-canary`.

| Retained identity | SHA-256 |
| --- | --- |
| Runtime archive | `0ad2512eed42477d9be68e0c9276958f4f27310ab93bbb06d742fa82cbdf77e3` |
| Release manifest | `5c9f696163016136c9fae4a1cce36bae7ded824c334f9fb168f9a0863745ea0d` |
| Materialized artifact subject aggregate | `5a34e8606b617d4cbc6504ef069a9f023436a2b3a94581191da8cd5dfefeb408` |
| Signed subject manifest | `10ca22f391ae1be3d7bbafdda5b772344a62c5dd31f396edb67dae07a8e04033` |
| Exact registry record | `dec4babe1fc8769fd0487922dc34ae14feadd4865a26779346eee33d53ad65a3` |
| Historical execution's retained gate | `6de2b1f12508b455c1ed99a182192ab1b6c228d9de263bd7f65ef85cc4a60827` |

All five required controls were present and verified: `abi_signature`, `sbom`,
`ml_bom`, `slsa_in_toto`, and `sigstore_cosign`. The exact source, registry,
subject and access bindings passed with lifecycle `manually-verified` and
trust-root mode `host_managed`. The gate returned `allow` for
`consumer_intent: runtime_canary`.

The separately checked production intent `runtime` returned
`manual_review_required`, `allow: false`, for the same registry record:
`production_consumer_requires_release_trust_root` and
`production_consumer_requires_release_lifecycle`. Canary admission is not a
release trust root, production admission, global runtime installation or
permission to process another source. No control was waived.

## Historical page: actual local OCR, still unreviewed

The bounded source was page 44, one-based, of the retained Naumann 1893
Zarathustra PDF. Original PDF and retained PNG remain separate identities.
The current signed capture verified their exact fixity and retained relation;
it did not run a renderer, supply the absent historical rendering signature,
or independently establish historical page-render fidelity.

| Historical evidence layer | SHA-256 |
| --- | --- |
| Original PDF File | `61c947e5aff76a64d82600cc52dcb25ff1b5862530d3a99c96824da885c1e6cf` |
| Retained page PNG | `8cde75dffb270dece07b0550c7d85d8f80b7e8037fd857a6031ccb79e9430a9a` |
| Authenticated owner execution receipt | `4c0452c64a4c7fcb15c195ebe89393f3278d1aed24d779107abd0ced12841969` |
| Raw OCR, 1,078 bytes / 1,050 code points | `1c86f3530773161e6e8338ed428a60a01196882e7ddb56c90bc600c4fe02f037` |
| Immutable native TextLayer record | `d1c0cee1a17105864f948c48e38e5cd3147d8a12e75227d564535ee3a51be83e` |
| Bounded TextUnit packet | `c3ba22722bef715aaab35eaacca2e42373946678abc07e22d649e24fe162ef6f` |
| Local-only v6 comparison record | `ac48069b4855c373f35cace4cfe6ee288721a81fb09666979ce0650e8ee4d857` |

Owner execution completed with frozen language `deu`. Retained owner replay
returned the same receipt without executing OCR again. Native metadata-only
and exact-content verification passed, and writer retries preserved the same
layer/unit packets. The TextUnit is `unit_kind: other`, covering this bounded
raw representation only; segmentation remains `proposed`, not full-document
coverage or accepted structure.

The local comparison was available, but source-visible judgment was
`not_performed`, `quality_can_use` was false, no assessment journal revision
was created, the layer remained `unreviewed`, and accepted uses stayed empty.
Neither historical image nor OCR text was disclosed to the assistant. The
separate historical-disclosure negative was refused before context/source
I/O. Public-domain metadata and local processing permission did not substitute
for model disclosure authority. This unresolved historical quality is an
explicit outcome, not a synthetic result or a hidden full-corpus acceptance.

## Synthetic image: observed, not calibrated or admitted

A separately retained operator-created PNG, its already authenticated OCR
layer and a current exact image/OCR disclosure grant exercised the distinct
synthetic v6 profile. The original source and old receipts were unchanged.

| Synthetic evidence layer | SHA-256 |
| --- | --- |
| Own PNG File/image | `e2c675120eb42ef0dd226935b57f8cb0aa7794a3f01b4e16c0377f16923549af` |
| Earlier authenticated owner execution receipt | `0bee9c07a7252597b138f2d6bc700dd92ec2d876913d615a4de51405e542913a` |
| Raw OCR, 49 bytes | `36736ef3e3c5515baf8656a1fa7efe033054b6d81b62a59beeddda8a4771bac4` |
| Native TextLayer record | `686cf2a34cdc99c4e74f142af9a3616ac5c9c23325b2db646b83e4d88e651e1a` |
| Separate v6 comparison record | `07639d32aa6bd41382a1dc18ac9ca503b675ab2365f46a8c620de406e0a6f412` |

At `2026-09-10T02:22:04Z`, the assistant actually viewed that own image and
compared its two visible lines with the raw OCR. The visible text matched.
This retained observation explicitly has
`calibrated_competence_attested: false`, no assessment-journal event, no
accepted uses and no historical source comparison. The comparison machinery
itself still reported judgment `not_performed` and `quality_can_use: false`.
Manual canary observation is not calibrated source assessment, a quality
admission, historical accuracy, translation judgment, publication or canon.

## Completed verification and narrow closeout

These are retained results from the implementation, not checks rerun for this
documentation-only return:

- 112 tests and 922 subtests passed across `test_native_text_layer_assessment`,
  `test_source_text_layer_commands`, `test_source_command_discovery`,
  `test_native_text_binding`, `test_script_topology` and `test_test_topology`.
- Four targeted quality-journal tests passed: exact comparison/quality append,
  quality-basis change, public adapter/invalid-grant refusal and unit use
  requiring current quality.
- All eight new `test_native_page_ocr_assessment` tests passed, including
  inherited comparison limits and the distinct private v6 route. These
  selections overlap; their counts are not one disjoint suite total.
- Mechanics topology, source-witness foundation and diff checks passed.
- The full seven-module battery reached its 600-second resource timeout. Its
  owned unit was stopped; no assertion result established full-suite success.
  This timeout is not a green gate or a waived failing assertion.

Manual review found the most exposed boundaries preserved: source versus
image versus OCR versus judgment; exact owner/source/receipt provenance;
rights-before-read; local access versus model disclosure; and observations
versus current authority, competence and scoped admission. Public graph and
Sign adapters still reject confidential v6; the original EPUB v5 contract is
not widened. Root's independent code review reported no blocking finding.
No historical textual or semantic approval is inferred from that code review.

This derivative leaves raw custody unchanged and omits source strings,
private topology, credentials and personal/session identifiers. Public hashes
still permit correlation with an independently possessed artifact; they do
not authorize retrieval or disclosure. Only this dated source return and its
generated documentation currentness companion are created at closeout; no
runtime processing, functional test, assessment or policy change is repeated.
The documentation-currentness check and source-home validator passed. The
cross-corpus documentation guard was attempted but is not green: it reported
stale AGENTS-route currentness, KAG budget candidate seal/file-count drift
and a dirty source epoch, plus KAG content-hash drift for
`access/web/tsconfig.json`. These broader generated-owner dependencies were
not repaired by this documentation-only return.
The integration owner retains combined generated parity, full gates and the
landing route. Local review is not CI, merge, deployment or publication.
