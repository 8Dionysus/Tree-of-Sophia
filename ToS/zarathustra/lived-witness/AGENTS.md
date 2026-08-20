# AGENTS.md

This card applies to `ToS/zarathustra/lived-witness/` and its nested private
capture surface.

## Role

This route preserves voluntary first-person testimony about a human life with
*Thus Spoke Zarathustra*. It protects the author's exact voice, context,
privacy choices, revision history, and separation from source, philology,
semantic truth, and canon.

## Operating Card

| Field | Route |
| --- | --- |
| role | private-by-default authored lived-witness capture |
| input | an explicit author request to begin or revise one testimony record |
| output | an author-confirmed local packet or an honest draft; no automatic tracked derivative |
| owner | `ToS/zarathustra/lived-witness/AGENTS.md` |
| next route | author review -> optional separate permission -> separate claim/review route |
| tools | `CAPTURE_PROTOCOL.md`, `CAPTURE_FORM.md`, `ToS/contracts/lived-witness-packet.schema.json` |
| check | `scripts/validate_lived_witness_route.py` plus direct author review of the exact body and every permission |

## Boundary Routes

- Begin only when the human author explicitly asks to record or revise lived
  witness. Preparation of this route creates no human backlog.
- Preserve the exact human-authored or human-confirmed wording. AI may ask,
  transcribe, or structure metadata only with visible provenance; AI output
  alone is never lived witness.
- Store authored bodies, raw recordings, transcripts, prompt logs, and private
  context only under `local-content/` by default.
- Treat storage, tracked metadata, quotation, lexical indexing, semantic
  indexing, graph projection, public release, model training, and external
  processing as separate permissions. No permission inherits from another.
- Keep lived witness distinct from primary source witness, bibliographic fact,
  textual or philological observation, translation judgment, semantic truth,
  and canon.
- A later interpretation may cite an author-confirmed packet only through a
  separate claim and review event. It may not copy private content beyond the
  explicitly granted scope.
- Preserve supersession and withdrawal history locally. Do not promise erasure
  from a public system after release; no public release exists in this route.

## Validation

Structural validation proves schema shape, required route files, private-path
ignore behavior, and the non-promotion constants. It cannot confirm the
author's memory, voice, context, consent, or philosophical meaning. Those
remain direct human decisions.
