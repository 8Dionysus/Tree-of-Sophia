# TextGrid transfer-source challenger refresh — 2026-08-09

Status: completed negative/deferred source-discovery result; no corpus Item,
accepted text, alignment, human task, or experiment admission created

## Question

Can the current public TextGrid/Kolimo+ Nietzsche objects strengthen the
German side of the frozen thirty-five-route golden-kernel transfer plan as:

1. an exact historical edition or Item witness;
2. an independently useful comparison transcription; and
3. a rights-clear candidate for local or future public use?

The question is deliberately narrower than “is TextGrid useful?” TextGrid is
a durable, citable, machine-readable repository. The issue is which evidential
role these exact objects can honestly hold in Tree of Sophia.

The protocol-native receipt is
`ToS/source-witnesses/discovery/runs/textgrid-transfer-source-challenger.2026-08-09.v1.json`.

## Decision

Do **not** acquire or register the probed TextGrid objects as transfer-source
Items and do not substitute either text for the exact 1886 *Jenseits*, 1892
*Genealogie*, or 1906 *Antichrist* witnesses already used by the frozen plan.

Retain TextGrid/Kolimo+ as a **deferred normalized-corpus challenger** only.
A later episode may use one exact object for a predeclared OCR-discrepancy or
normalization question, after object/layer rights reconciliation and method
freeze. It may not count as:

- the historical Item or exact Edition used by the transfer route;
- a critical edition or author-final text;
- accepted German, German competence, or diplomatic transcription;
- a source-target passage, translation alignment, target gold, sign, concept,
  semantic claim, graph truth, or canon effect.

No later episode is scheduled by this refresh.

## Evidence order

### 1. Classical and official documentation

The current TextGrid Repository documentation describes a published object as
a data object plus a metadata object and documents public `READ` and
`READMETADATA` routes. Revision URIs and Handle PIDs address particular
revisions. The repository download documentation exposes TEI/plain-text and
aggregate export routes. These are strong transport, fixity, and citation
properties; they do not themselves establish historical-edition identity or
extend one object's license to another object or layer.

The existing ToS witness ladder remains the controlling comparison:
Work -> Expression -> Edition -> Item -> File. An open API response is access
evidence, not permission or bibliographic equivalence.

### 2. Established historical and critical controls

The exact 1886 *Jenseits* print is already independently source-visible through
the Zentralbibliothek Zürich e-rara route and a fixity-bound local research
Item. The 1892 second *Genealogie* Item and 1906 Naumann aggregate containing
*Der Antichrist* likewise already have exact, source-visible local routes.
Their authorial/print/critical stage distinctions are documented in:

- `JENSEITS_AUTHORIAL_WITNESS_ROUTE.md`;
- `GENEALOGIE_AUTHORIAL_WITNESS_ROUTE.md`;
- `ANTICHRIST_AUTHORIAL_WITNESS_ROUTE.md`.

The eKGWB remains the critical-edition comparison route with stable scholarly
addresses. Its critical layer is not interchangeable with a Kolimo+ corpus
transcription or a historical scan.

### 3. Freshest current object-level inspection

At 2026-08-09T01:57:55--01:57:56-06:00, the public TG-crud endpoints returned
the following exact responses. Bodies were held only in a mode-0700 host
temporary directory for fixity and bounded TEI-header inspection, then
deleted. They were not copied into `ToS/`, registered as Items, indexed, or
retained as snapshots.

| Object | Returned bytes | SHA-256 | What the object says | Transfer consequence |
| --- | ---: | --- | --- | --- |
| `textgrid:4k1kd.0`, `hdl:21.11113/4k1kd.0` | 213,431 | `34b5b9d2e4770c03f6be735148a9828a5c53e05383d5000fcd9d6ce0416c39b2` | *Der Antichrist* transcription supplied by Project Gutenberg DE/Hille & Partner; source description names Alfred Kröner, Leipzig, 1922, *Nietzsches Werke Taschen-Ausgabe*, vol. X | not the existing 1906 Item, the 1895 first print, manuscript, or critical text |
| metadata for `textgrid:4k1kd.0` | 1,703 | `ec940a6c33c652c908c23676c6e5c6e99c43c0a26726d408d1a64d453d831e9a` | public revision metadata | transport/fixity evidence only |
| `textgrid:4k212.0`, `hdl:21.11113/4k212.0` | 336,306 | `30122935647a3ea581e004c048d5c41e4b2ea5bb37d410b9a627a65c6f1347ad` | source path is `/nietzsch/genealog/genealog.xml`; the title collapses *Jenseits* and *Genealogie*, while source metadata combines Deutscher Taschenbuch Verlag/de Gruyter with the date 1887 | not a separate *Jenseits* text, not the local 1892 Item, and not a sufficiently coherent exact 1887 Edition identity |
| metadata for `textgrid:4k212.0` | 1,766 | `9de6ef3513947bc26b4ebdc89d90c46ae5eff821f2de9620fe2e44efb3868ea1` | public revision metadata | transport/fixity evidence only |
| aggregate `textgrid:4k1p6.0` | 446 | `566f38c68ce7cfe67a9a545796c02231e8145ad166503aacf240f39e82f5a0bc` | aggregate membership data | not the contained transcription |
| metadata for aggregate `textgrid:4k1p6.0` | 1,787 | `1d4502552efc26f38323c225ab8a25b912143841a26c995af0cda567697a8a66` | aggregate reports CC BY 4.0 | license evidence scoped to the object/layer that states it; no automatic propagation to `4k212.0` |

Both TEI text headers say `Vollständige digitalisierte Ausgabe`, but also state
that normalization was silent, long-s was normalized, page breaks were
marked, and line breaks were not. “Complete” therefore describes capture
scope under that editorial policy; it does not mean diplomatic fidelity,
critical authority, or identity with a different historical witness.

The current author-filtered TextGrid result surface exposes the malformed
*Genealogie/Jenseits* object and the *Antichrist* object. The apparent
*Jenseits* title is not evidence of a separate Nietzsche *Jenseits* object:
the direct text object's source path and body identity are *Genealogie*.

### 4. General web last

Only after the repository documentation, exact object endpoints, and existing
historical/critical controls were checked, a bounded exact-domain search was
used to test the remaining *Jenseits* ambiguity. It returned the same
TextGrid author listing and no stronger separately identified Nietzsche
*Jenseits* text object. No ordinary-web result displaced an originating
repository, library, or critical-edition record.

## Identity reconciliation

| Frozen transfer work | Exact current ToS source route | TextGrid/Kolimo+ object | Identity result |
| --- | --- | --- | --- |
| *Jenseits von Gut und Böse* | Naumann 1886 first-print Item; separate e-rara and eKGWB controls | no separately established object; `4k212.0` is *Genealogie* despite the collapsed title | reject as a *Jenseits* witness |
| *Zur Genealogie der Moral* | Naumann 1892 second-edition Item | normalized `4k212.0` with incoherent source bibliography | defer as a comparison corpus only |
| *Der Antichrist* | pages 228--329 in the 1906 Naumann aggregate, with separate navigation evidence | normalized `4k1kd.0` based on a 1922 Kröner volume | defer as a comparison corpus only |

The two deferred objects do not improve the exact source-boundary identity of
any of the thirty-five prepared routes. Their value is methodological
independence, not witness proximity.

## Rights reconciliation

The repository layer and transcription layer carry different statements:

- aggregate metadata reports CC BY 4.0;
- each inspected TEI transcription attributes its supplied text to Project
  Gutenberg DE/Hille & Partner and states `nicht-kommerzielle Nutzung frei`,
  linking the Gutenberg information route;
- repository availability and public API access establish access, not a
  single global license conclusion.

This is `conflicting_evidence` at the cross-layer level until exact object,
licensor, covered material, and intended use are reconciled. ToS therefore
does not propagate the aggregate CC BY label to the nested transcription and
does not turn the noncommercial statement into unrestricted redistribution.
No new rights record is created because no TextGrid object is being admitted
as an Item or scheduled for use.

## When the deferred challenger may reopen

A later run may reopen one object only when all of these are frozen first:

1. one exact discrepancy question, such as whether a named OCR route differs
   from the normalized corpus at a bounded already-admitted passage;
2. the exact TextGrid revision URI, Handle PID, response digest, and source
   bibliography;
3. the exact role: diagnostic comparator, never historical/critical truth;
4. object- and layer-specific rights for local processing and any output;
5. German competence appropriate to the intended judgment;
6. a comparison that returns to the exact historical page and preserves null
   or harmful results;
7. a separate decision before any byte retention, Item registration, human
   task, or public derivative.

Until then, acquisition would add storage and rights surface without closing
a current evidential gap.

## Zero-effect boundary

This refresh creates:

- one tracked research packet;
- one schema-valid discovery receipt;
- one provenance event;
- one explicit deferred challenger class.

It creates zero corpus Items, retained source payloads, accepted source or
target passages, alignments, eligible transfer units, gold units, semantic
objects, graph effects, human tasks, messages, publication permissions, or
server imports. The frozen 35-route / 32-intersection / 3-nonintersection /
20-page readiness state is unchanged.

## Current sources

- TextGrid Repository documentation: <https://doc.textgridlab.org/services/resolver.html>
- TG-crud current API documentation: <https://doc.textgridlab.org/submodules/tg-crud/tgcrud-webapp/docs/>
- TextGrid download documentation: <https://www.textgridrep.org/docs/download>
- current TextGrid Nietzsche result surface: <https://www.textgridrep.org/search?filter=edition.agents.author.value%3ANietzsche%2C+Friedrich&filter=edition.language%3Adeu&limit=50&mode=list&order=asc%3Aissued&query=>
- eKGWB documentation: <https://doc.nietzschesource.org/de/ekgwb>
- e-rara 1886 *Jenseits* record: <https://www.e-rara.ch/zuz/content/titleinfo/20295083>
