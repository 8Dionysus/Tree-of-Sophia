# Native additive TextLayer derivation review

Review date: 2026-09-09. Baseline:
`725477fac15437c4686ee083b32674830931464a`.

## Live contract audit

Before this change, native `source_text_layer_commands.py` implemented only
bounded EPUB/XHTML structural extraction. Correction, normalization, OCR and
transcription enums in source-text-layer v1 were not discoverable executable
construction routes. `NativeLayerAssessmentSources` compared only that exact
extraction profile. Existing native unit construction could use a real layer,
but neither its bootstrap nor a schema enum supplied the missing transforms.

## Source and boundary review

- Yes: one independently protected additive grant chooses exactly one of
  correction, Unicode normalization, supplied transcription or supplied OCR
  recording. The original extraction grant/operation remains distinct.
- Yes: correction and normalization read one exact versioned whole predecessor,
  retain its record/content hashes and source anchors, and create a distinct
  identity with explicit successor version. No predecessor, raw File, source
  metadata, existing anchor, assessment or accepted use is rewritten.
- Yes: corrections have ordered explicit input/output spans, exact strings,
  digests, reasons, responsibility and proposed status. Unicode normalization
  executes the named form and actual Unicode version with an explicit whole-
  text operation. Even identical resulting bytes do not inherit source quality.
- Yes: supplied manual/model transcription and OCR preserve exact private
  UTF-8 input and a separate reported maker/method/version. The recording
  event is annotation, contains no model invocation, and expressly does not
  attest upstream provider execution, source-visible transcription, competence,
  layout or fidelity.
- Yes: both existing source rights and exact new-layer rights precede content
  I/O. Reading, derivation and supplied-result reading are independent current
  grants. External selectors, files, configuration and text remain inert data;
  no provider, shell, OCR engine, renderer, network or download is selected.
- Yes: existing no-replace private construction and retained recovery remain
  the only publication route. Expected configuration/dependencies and exact
  historical outputs are checked; expiry, input drift, changed dependencies,
  foreign state or torn evidence fail closed without erasure or overwrite.
- Yes: text is bounded at 128 KiB and edits at 128, with existing resolver,
  package, plan, lineage and cooperative-time ceilings. There is no quadratic
  diff or image/PDF decompression. Original File fixity is streamed separately.
- Yes: the native reader verifies exact predecessor version/source scope and
  independently replays the immediate explicit delta. It does not replay a
  historical provider or require today's Unicode version to authenticate an
  old execution. Unrecognized legacy method configurations remain opaque.
- Yes: all new layers are unreviewed. Predecessor uncertainty is not resolved;
  accepted uses, competence, review, promotion and publication are not copied.
  Source-near correction cannot erase a normalized predecessor's posture.
- Yes: discovery remains grant-free, outputs remain redacted, and documentation
  plus existing script/test inventories describe the real implemented route.
  No new type registry, runtime service or source-truth carrier was introduced.

Golden-kernel transfer, lived witness, calibration, counterpart mapping, canon
mirrors and durable decision intake are not applicable. No source, rights,
human work or real philosophical text was assessed by these synthetic checks.

## Verification and next owner

The focused native derivation, extraction, discovery, reader, unit/assessment
compatibility and script/test topology selection passed **177 tests and 1,025
subtests**. It includes a complete synthetic extraction -> correction ->
normalization -> first-segmentation chain, all four Unicode forms, supplied
human/model/OCR provenance, missing/changed input and rights boundaries,
independent edit replay, retained interruption recovery and explicit refusal
by the extraction-only assessment adapter. The source-witness foundation
validator also passed. An intermediate selection passed 90 tests and 210
subtests. No failure was waived.

The final narrow rerun after adding the reader's new transitive implementation
dependency to the existing TextUnit input pins passed **74 tests and 790
subtests**. It rechecked layer and unit transaction behavior plus script/test
topology. All checks ran through the owner-managed light resource route.

The source-visible assessment continuation is explicitly owned by
`native_text_layer_assessment.py` / `NativeLayerAssessmentSources._metadata`
and `NATIVE_TEXT_LAYER_ASSESSMENT.md`, followed by the existing assessment
journal's exact current purpose-scoped quality-basis route. Its present
extraction-only adapter refuses the new methods; old layer quality is not a
successor verdict. This bounded construction commit therefore does not claim
Foundation-wide assessment completion. The integration owner will run a
separate real source canary and integrate the method-specific comparison path.

No generated parity, broad release gate, CI, merge, deployment, restart, runtime
installation, provider execution or real corpus acceptance is claimed. The
integration owner retains post-union generated-companion and landing ownership.
