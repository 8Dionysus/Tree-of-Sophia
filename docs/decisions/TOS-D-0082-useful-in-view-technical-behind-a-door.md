# TOS-D-0082 Useful in View, Technical Behind a Door

## Index Metadata

- Decision ID: TOS-D-0082
- Original date: 2026-11-20
- Surface classes: bonsai, access/web
- ToS layers: derived projection
- Tree classes: none
- Guard families: source-first authority, bounded downstream seams
- Posture: accepted by explicit operator instruction

## Context

The interface plan for solo work with agents (`access/web/INTERFACE_PLAN.md`)
needed three answers before its highlight categories (A2) and held material (A5)
could be built, and a fourth surfaced with the pages of first editions (H1):

- `bonsai/CORE.md` said that size, brightness and position add nothing to a
  claim's weight, and listed "significance carried by size or brightness" as a
  refusal of the **show** operation. Read literally, it forbade any visual
  channel that shows a recorded property, such as a verdict or a category the
  reader turned on.
- `bonsai/zarathustra/held.json` holds what review withheld from the map. It
  matters to anyone checking the stand's work and is noise to a reader.
- Rights statements, provenance, addresses of places and regions, build and
  journal records, facsimiles for checking a transcription: each is important,
  and each stands in the way of reading when it is shown everywhere.

On 20 November 2026 the operator ruled: the Tree must be the Tree and hold
only what is of use; what is less useful but important is kept in technical
sections and shown only to those who go there on purpose.

## Decision

1. **Channels.** A view may give size, brightness or the weight of a line to a
   recorded property of a claim (its category, verdict, attention, or a layer
   the reader turned on) when the view names in its legend what that channel
   shows; one channel shows one property at a time. A channel that shows
   nothing recorded, or that the legend does not name, is refused by **show**.
2. **Held material.** What review withheld appears only in the research layer,
   with the reason review gave, never in the reading layer.
3. **Useful in view, technical behind a door.** What serves reading,
   understanding or the next step stands in view. What is important but
   serves checking (rights and their sources, provenance and fixity, addresses
   of places and regions, journal and build records, facsimiles beside a
   transcription) stands in a technical section the reader opens on purpose,
   and stays there whole.

## Options Considered

- Keep the literal rule: no channel for recorded properties. Categories and
  verdicts would then be told only in words, and a crowded map would hide them.
- Let channels carry weight freely: a reader could not tell a recorded verdict
  from emphasis the view invented.
- Show every technical record where it belongs to a subject: the stand would
  read like a database; the reader's work would drown in apparatus.

## Rationale

The rule keeps what CORE.md protected (a view never invents significance) and
admits what a reader needs (a recorded property seen at a glance), because the
legend names what each channel shows. Technical sections keep the apparatus
complete for anyone checking the work while the Tree stays a place of reading
and thought; nothing is deleted or hidden from someone who asks for it.

## Consequences

- `bonsai/CORE.md` states the channel rule and the technical-section rule; the
  **show** refusals follow them.
- The stands' surfaces move apparatus into technical sections as they are
  touched: addresses, rights links, journal records and reviewer details open
  on demand.
- Held material and facsimiles are built under these rules (A5, H1).
- A future contributor must not read this as licence to drop apparatus: a
  technical section holds all of it.

## Source Surfaces

- `bonsai/CORE.md`
- `access/web/INTERFACE_PLAN.md`
- `bonsai/zarathustra/held.json`

## Validation

`node bonsai/lib/cli.mjs check` for the bonsai; the web suite (`npm test` in
`access/web`) for the stand surfaces that follow the rules.
