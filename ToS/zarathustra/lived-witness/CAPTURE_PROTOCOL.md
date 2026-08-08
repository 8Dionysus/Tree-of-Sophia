# Lived-witness capture protocol

Status: prepared route; no testimony captured, scheduled, tracked, indexed, or
published

## Result first

Capture one voluntary first-person record at a time. Keep the exact human voice
and the contextual metadata together in a local ignored packet. Ask only the
next useful question, show the exact body back to the author, and require an
explicit confirmation before the packet becomes `author-confirmed`.

The protocol is intentionally not a recurring Human Gold task. It opens only
when the author chooses to speak, and it may stop at any point without creating
debt.

## Capture sequence

1. Confirm that the author wants to begin and that the default is local-only.
2. Ask what the record concerns: the whole work, a part, a passage, a reading
   or recitation event, or a life context. A whole-work record needs no forced
   passage anchor.
3. Invite the author to speak or write in their own form. Do not impose the
   categories already present in ToS.
4. Preserve the first raw answer. If voice or dialogue is used, keep the raw
   artifact locally and record its digest.
5. Ask only optional clarifications needed for the author's intended meaning,
   temporal scope, or target. Unknown, relative, and withheld time are valid.
6. Record capture mode, AI role, prompts or instructions, and every
   transformation. Punctuation, transcription, redaction, summary, and
   stylistic editing are different operations.
7. Show the exact testimony body back to the author. No normalization or
   polished rewrite is silent.
8. On explicit confirmation, bind the reviewed body digest and record the
   confirmation event. Otherwise retain `draft`.
9. Review third-party exposure before any derivative leaves local storage.
10. Ask about non-local uses only if the author requests one. Record each use
    separately; a `yes` to quotation is not a `yes` to model training,
    semantic indexing, graph projection, external processing, or publication.

## AI posture

AI may:

- ask open and bounded questions;
- preserve the author's answer verbatim;
- transcribe a recording as an unconfirmed candidate;
- propose punctuation or structure while showing the changes;
- fill mechanical metadata from explicit answers;
- identify missing provenance or permission decisions;
- create a separate, visibly derived summary or interpretation.

AI may not:

- manufacture first-person experience;
- insert plausible memories or life details;
- treat an AI-edited narrative as confirmed without showing it to the author;
- merge its own interpretation into the testimony body;
- infer consent from participation or from another permission;
- promote the packet into semantics, graph truth, canon, publication, or
  training data.

## Time and context

The packet separates:

- the time or interval of the remembered experience, with exact, approximate,
  relative, unknown, or withheld precision;
- the recording time of this version;
- the author's own context terms and optional note;
- later revision, supersession, and withdrawal events.

Memory is valuable first-person evidence but not automatic proof that every
recalled external event happened exactly as narrated. That limitation does not
reduce the authorship or importance of the testimony.

## Targets and anchors

Every packet names the work. It may additionally target an Expression,
Edition, Item, part, passage, reading event, recitation event, or life context.
Use source-returnable passage anchors when the testimony actually concerns a
passage. Do not invent one merely to satisfy a graph shape.

## Storage and permissions

The initial packet and all raw artifacts live under:

`ToS/zarathustra/lived-witness/local-content/<packet-id>/`

Git ignores that content by a narrow rule. File mode should be `0600`; parent
directories should not grant group or world access. No network or external API
receives the body unless `external_service_processing` is separately granted.

The packet records distinct decisions for:

- local storage;
- tracked metadata in Git;
- quotation;
- lexical indexing;
- semantic indexing;
- graph projection;
- public release;
- model training;
- external service processing.

All except local storage begin `not-granted`. If a future tracked or public
derivative is desired, create a separate content-minimized artifact after
review; never move the private packet itself into Git as a shortcut.

The author's copyright remains `author-retained` by default. A scoped license
or public-domain dedication requires its own explicit record and does not
arise from local capture, author confirmation, or any single use permission.

## Revision, supersession, and withdrawal

Do not overwrite an author-confirmed body. Create a new packet version, link
it to the prior version, and preserve the reason supplied by the author.

Withdrawal closes every non-local permission for future ToS-controlled use.
The local history may remain for audit or may be erased only by a separate
explicit owner decision. If content has already been made public elsewhere,
ToS must state honestly that complete third-party erasure cannot be guaranteed.

## Promotion boundary

Lived witness may later explain attention, salience, selection, and an
interpretive path. Any such use requires a separate `claim-packet` whose own
maker, evidence, method, review, visibility, and alternatives remain visible.
The lived-witness packet alone establishes none of the following:

- source wording or textual state;
- bibliography or publication history;
- morphology, etymology, or translation quality;
- stable sign, concept, relation, or necessary meaning;
- canonical ToS judgment.

Generated search, graph, and KAG surfaces may see only an explicitly permitted
derivative. They never become the owner of the testimony.

Structural validation is insufficient for authorship, consent, memory,
context, or meaning. The author must inspect those realities directly.
