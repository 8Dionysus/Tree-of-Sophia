# TOS-D-0084 Edition Pages as Witness Data

## Index Metadata

- Decision ID: TOS-D-0084
- Original date: 2026-11-20
- Surface classes: bonsai, access/web
- ToS layers: source representation, derived projection
- Tree classes: none
- Guard families: source-first authority, rights custody, provenance
- Posture: accepted by explicit operator instruction

## Context

A reader checking a transcription wants the page of the first edition beside
its text (interface plan, H1). The Zarathustra stand's witnesses keep each
unit's text exactly as its source gives it, but no unit knows its page: the
Deutsches Textarchiv's XML marks page breaks (`<pb facs=…/>`) and Wikisource's
index of the 1913 Russian edition lists its scan pages, and neither was kept.
The facsimiles belong to their holders (Staatsbibliothek zu Berlin through the
DTA; the 1913 scan on Wikimedia Commons), each under its own statement.

## Decision

- Each witness gets a page map, kept as stand data beside its witness
  (`bonsai/<stand>/pages/<witness>.json`): for every unit, the page or pages it
  stands on, the page's label as printed, the holder's address of the page
  image, and the source the map was read from with its fixity.
- A facsimile is shown under TOS-D-0082 in the technical section of the
  chapter window, opened on purpose; the reading keeps the text in view.
- Until the image's rights are checked, the page is shown from the holder's
  own address and names the holder's statement. A stand copy (the layer store
  by SHA-256) is kept only after the statement is read and admits public
  redistribution; the check is recorded with the copy.

## Options Considered

- Copy every page image now: fast, but a copy whose rights were not read is a
  liability the stand cannot carry.
- Link only to the holder's viewer: no exact page beside the unit, and the
  reader leaves the stand to check one sentence.
- Show the facsimile beside every text: the apparatus would crowd the reading.

## Rationale

The page map is a fact about the witness and belongs with it; it lasts beyond
any one viewer. Showing the holder's image until rights are read keeps custody
with the holder; a copy follows only from a recorded check.

## Consequences

- Acquiring the maps needs the holders' hosts in the execution environment.
- A witness without a page map shows no facsimile, and says nothing about it
  outside the technical section.
- The rights check of each image source is recorded before any copy is made.

## Source Surfaces

- `bonsai/zarathustra/witnesses.json`
- `bonsai/zarathustra/witnesses/`
- `access/web/src/public-research/fragments.mjs`

## Validation

`node bonsai/lib/cli.mjs check` for the stand's data; the web suite for the
chapter window.
