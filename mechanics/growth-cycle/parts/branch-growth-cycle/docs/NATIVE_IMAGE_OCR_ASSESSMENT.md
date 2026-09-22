# Exact image/OCR comparison

The confidential assessment-owner v6 profile adds one method-specific image
comparison to the existing [quality journal](NATIVE_TEXT_LAYER_ASSESSMENT.md).
It does not broaden the EPUB-only v5 profile. `native_page_ocr_assessment.py`
is read-only: it invokes the fixed owner **receipt verifier**, never Tesseract,
a PDF renderer, a model, or a remote service. Authentication still belongs to
the exact clean `abyss-stack` adapter; runtime admission remains with
`abyss-machine`. Comparison availability is not a quality verdict.

## Independently selected profiles

`tos_local_assessment_owner_v6` has the same top-level fields as v5, with at
most one `native_text_layers` selection. In addition to the existing binding,
origin, source and original-payload grants, that selection requires:

| Field | Exact scope |
| --- | --- |
| `comparison_profile` | `tos_retained_page_ocr_image_comparison_v1` or `tos_operator_synthetic_png_ocr_image_comparison_v1`. |
| `image_access` | A separately issued current exact image grant, or `null` for metadata-only selection. |
| `disclosure_access` | `null` for historical retained pages. The synthetic PNG profile alone may carry an exact current operator-issued assistant-session disclosure grant. |

The image grant has `read_scope: exact_retained_page`, `access_allowed: true`,
nonempty `authority_ref`, timezone-aware future `expires_at`, absolute `path`,
`byte_size`, `sha256`, `page_number`, `source_file_ref`, `source_file_sha256`,
`width_pixels`, `height_pixels`, and `processing_boundary: local_only`.
Metadata-only selection requires both image and payload grants to be null,
and cannot disclose, read a comparison, or append any quality decision.

The retained-page profile accepts only a separately authenticated
`text-layer.record-owner-page-ocr` raw layer: original acquired PDF identity,
one-based whole-page anchor, independently pinned PNG, original frozen sample
plan, historical unsigned rendering receipt and new signed verification
capture. Original PDF, retained PNG, raw OCR and subsequent judgments remain
distinct. The original PDF is streamed for exact fixity up to 128 MiB; PNG
input is at most 10 MiB/12 million pixels, single RGB image, and raw OCR at
most 128 KiB. This route does not rerender or independently establish historical
page-render fidelity. Comparison limits preserve that uncertainty in current
layer admission and every derived quality basis.

The separate synthetic PNG profile accepts only an authenticated
`text-layer.record-owner-ocr` result over one complete acquired PNG and its
whole-image pixel anchor. Its scope is operator-created material: original
File identity and image digest refer to that PNG. Historical-source and
PDF-rendering claims use their corresponding routes.

## Local access is not model disclosure

The retained-page profile rejects **any** non-null disclosure grant before
context or source I/O. A positive public-domain assessment, local processing
permission, successful OCR, signer identity or exact hash cannot open that
boundary. Source-visible historical review may remain explicitly unreviewed
without blocking completion of the bounded local processing route.

For the synthetic profile, an optional protected `disclosure_access` has
`allowed: true`, nonempty `authority_ref`, future timezone-aware `expires_at`,
`read_scope: exact_source_image_and_ocr`,
`basis: operator_created_synthetic_source`,
`processing_boundary: current_assistant_session`, exact `source_file_ref`,
`source_file_sha256`, `image_sha256` and native `layer_record_sha256`.
This declaration is supplied by the independently selected trusted issuer,
never by source material or an assessment request. Software validates its
scope and expiry; it does not prove authorship or invent the operator's will.
No grant means local comparison only. Another source, changed bytes or an
expired disclosure grant requires a new explicit owner choice.

## Reading and judging

`describe` returns metadata and readiness only. `read-layer-comparison`
returns `tos_native_page_ocr_comparison_v1`: exact source/anchor/input binding,
private image locator, raw OCR, authenticated execution/capture identity,
fixity, disclosure posture and limits. It never embeds or transfers image bytes.
The local caller must honor the declared disclosure boundary before sending
the image or OCR to an assistant or other server. `deterministic_text_match`
is null and `source_visible_judgment` is `not_performed`: OCR-to-pixel quality
cannot be inferred from matching hashes or the existence of this packet.

The engine requires exact source-visible evidence, current assessment
authority, calibrated competence and execution binding. A read-only
configuration may omit authority, competence and execution profiles and
supplies comparison data only. Any actual review applies to the exact
image/raw layer and selected use; broader accuracy, diplomatic fidelity,
translation quality, historical-source clearance, publication and canon
require their own scoped evidence and authority.

Original rights, metadata, original File identity, retained image identity,
grants and disclosure scope are rechecked through the snapshot and at the
journal return/commit edge. No retry executes OCR or rendering. Public graph
and Sign consumers continue to reject confidential v4/v5/v6 configurations
before private context I/O. Tests use synthetic fixtures and cannot grant
real source access, model competence or historical acceptance.
