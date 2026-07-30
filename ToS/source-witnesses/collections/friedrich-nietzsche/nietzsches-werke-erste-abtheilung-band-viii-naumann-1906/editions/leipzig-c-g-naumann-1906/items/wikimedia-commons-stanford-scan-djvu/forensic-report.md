# Forensic Report — *Nietzsche's Werke*, Band VIII, Naumann 1906

Status: exact local payload identified and mechanically inventoried;
source-visible collection, edition, holding-copy, and *Der Antichrist*
boundaries verified by a model; text, German, rights, and bibliography remain
unaccepted by a human

## Item

- item:
  `tos.item.friedrich-nietzsche.nietzsches-werke-band-8.naumann-1906.wikimedia-commons-stanford-scan-djvu`
- local payload:
  `payload/nietzsches-werke-viii-1906-commons-stanford.djvu`
- size: 24,324,176 bytes
- SHA-256:
  `8f61aaecd55339fc3ba11eca24fbeef85e953d1a262200b36e59d5fbc545ca9d`
- Commons SHA-1:
  `decddcd95661f368fbf725ee855350b9cb3701d2`
- Commons SHA-1 and byte-size match: yes
- Git posture: ignored by the narrow item-payload rule

## Container inspection

libmagic 5.46 reports a DjVu multiple-page document. The exact payload has an
`AT&T FORM` root of type `DJVM`, a bundled `DIRM` directory with 523 strictly
increasing entries, 523 `DJVU` page forms, and 523 `INFO` chunks. Every page
reports 4034 × 5834 pixels at 600 DPI.

The repository's deterministic resource-inventory builder now parses this
bundled structure directly and emits:

- profile `djvu_pages_v1`;
- 523 ordered page resources;
- one page geometry;
- no source text, OCR strings, word counts, or content fingerprints.

`djvudump`, `djvused`, `ddjvu`, `djvutxt`, ExifTool, and an ImageMagick DjVu
decode delegate were unavailable. Their absence is recorded rather than
silently replaced with an OCR or content claim. Commons page thumbnails were
used for bounded visual review; the tracked inventory itself is produced only
from the exact local bytes.

## Source-visible identity

A model opened the actual page images rather than relying on the embedded OCR
or current Internet Archive description.

- Pages 1-2 show the Stanford University Libraries binding, bookplate, and
  barcode `36105025673729`.
- Page 5 states *Nietzsche's Werke*, *Erste Abtheilung*, *Band VIII*,
  Leipzig, C. G. Naumann Verlag, and 1906.
- Page 6 states `10. und 11. Tausend des Antichrist`, separately from the
  print-run statements for the other contents.
- Pages 10-12 carry the volume contents.
- Pages 517-519 are publisher advertisements; pages 520-523 carry rear
  binding and Stanford circulation furniture.

This identifies the scanned copy and publication but does not establish a
complete catalog for every member in the aggregate volume.

## *Der Antichrist* boundary

The contents and boundary pages were visually cross-checked:

- scan page 11 reports *Vorwort* at printed page 213 and *Erstes Buch: Der
  Antichrist* at printed page 215;
- scan page 228 begins *Vorwort* on printed page 213;
- scan page 229 ends the preface on printed page 214;
- scan page 230 is the internal *Der Antichrist* title leaf;
- scan page 231 is a blank/furniture leaf;
- scan page 232 begins numbered section 1 on printed page 215;
- scan pages 326-329 carry printed pages 311-314 and the terminal text after
  section 62;
- scan page 330 begins *Disposition und Entwürfe zum dritten Buch der
  Umwerthung aller Werthe*.

The proposed member range is therefore scan pages 228-329 inclusive. The
ranges 1-227 and 330-523 remain explicitly unrepresented by the partial map.
They are not mislabeled as non-work content and no missing Work identities are
manufactured merely to make the map appear complete.

## Provider-lineage conflict

Commons credits Internet Archive item `nietzscheswerke00nietgoog`. Its original
PDF file metadata retains Google Books ID `raDHLnqFdIwC`, while the visible
scan is the Stanford/Naumann 1906 volume. The current Internet Archive item
record instead describes Gustav Siewerth, Düsseldorf 1971, and reports an
unrelated Open Library identity.

The conflict is preserved as provider metadata contamination. The IA
description is not used to identify this collection, edition, physical copy,
or rights state. The exact Commons object has its own page ID, SHA-1, size,
local SHA-256, and source-visible evidence.

## Editorial boundary

The Heidelberg Academy's historical-critical commentary reports that *Der
Antichrist* first appeared in the Koegel 1895 volume with Köselitz corrections
and four suppressed passages. The selected 1906 text is therefore an
editorially non-neutral archive-edition witness. It is not equated with:

- Nietzsche's 1888 manuscript;
- the Koegel 1895 first printing;
- a Colli-Montinari or other critical edition;
- the TextGrid/Kolimo+ transcription;
- accepted German.

The no-view Cornell 1895 Google Books record remains a deferred bibliographic
and possible written-access-request lead. No access request was sent.

## Rights boundary

Commons reports `Public domain`, `Copyrighted=False`,
`AttributionRequired=false`, and public-domain categories for the exact
object. This is strong positive source evidence. No jurisdiction-specific
human legal review has been completed, so ToS remains
`copyright_undetermined`, `local_only`, and unknown for redistribution and
derivatives.

The operator-held DjVu is never uploaded to the future site. Any future public
payload must be selected or reacquired independently from an exact authorized
route after rights review. Metadata and provenance may remain tracked without
redistributing the source file.

## Authority ceiling

This intake establishes exact bytes, page order and geometry, source-visible
copy/publication identity, a partial collection-membership claim, proposed
page boundaries, and documented rights/editorial evidence. It does not accept
OCR, German, a critical text, translation, semantics, signs, concepts, graph
relations, canon, or public payload transfer.
