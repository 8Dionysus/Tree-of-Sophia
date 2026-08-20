# Source-Witness Catalog

This directory is the tracked, generated navigation index over authoritative
object and bibliographic claim records in the speaking `agents/`, `places/`,
`organizations/`, `works/`, and `collections/` trees.

| File | Record class |
| --- | --- |
| `agents.jsonl` | people |
| `places.jsonl` | provisional normalized places |
| `organizations.jsonl` | provisional historical organizations |
| `works.jsonl` | intellectual works |
| `expressions.jsonl` | language/textual responsibility states |
| `editions.jsonl` | published/edited manifestations |
| `collections.jsonl` | aggregate publications |
| `items.jsonl` | acquired physical/digital copies or containers |
| `claims.jsonl` | source-returnable membership, responsibility, chronology, publication, provision-activity, and Work/Expression/Edition/Item topology claims |
| `catalog.manifest.json` | counts, paths, digest, and generation boundary |

Object records own identity; source claim packets own bibliographic
assertions. These JSONL files are rebuildable indexes. Object entries include
their source record path and canonical digest. Claim entries additionally
retain the exact source JSONL line, canonical claim digest, subject, predicate,
object, evidence, maker, provenance event, and review posture. The projection
does not accept or promote any claim. Only claims already marked `public` or
`public_metadata_only` may enter this tracked projection; local, restricted,
or permission-pending material requires a separately reviewed public-safe
derivative rather than silent copying.

The current topology contribution is 64 separately addressable packets: 24
`has_expression`, 24 `embodied_by`, and 16 `exemplified_by`. Their presence in
this generated catalog proves exact projection only. It does not make the
declared identity ladder true, accept any text, or turn embodiment into
textual equivalence.

The current bounded projection contains 85 object records and 129 claim records
(214 entries total). The responsibility slice contains seven Work authorship,
ten Expression translation, and three Edition role claims; all remain
source-returnable, model-made, public-metadata-only, and unreviewed.
The 1913 Antonovsky responsibility claim is bound to a proposed whole-page
title-page anchor in its exact Item. The distinct 2007 Cultural Revolution
Expression adds its own translator claim and two proposed page anchors while
recording that the historical translation was checked and newly edited. The
shared Agent does not equate either Expression with the 1898, 1911, 1996, or
later witnesses.
The chronology slice adds seven Work-level first-publication profiles. It keeps
their temporal objects as claim-scoped literals and supplies named interval
boundaries rather than one identity year.
The provision surface contains three provisional Places, eight provisional
historical Organizations, and sixteen Edition-owned grouped statements.
Three claims belong to the exact 1883/1883/1884 first three parts of *Also
sprach Zarathustra*: each returns to its own DTA Edition record while resolving
the same Chemnitz Place and historical Schmeitzner Corporate Body, not the
affiliated Person. A fourth claim belongs only to the exact part-IV Edition and
reports `Naumann; Leipzig; 1891`; it reuses the Leipzig Place and historical
Naumann publisher Organization while keeping the printer, founder Person,
1885 private print, 1890 printing, and March-1892 delivery outside the
statement. Literal wording, normalized identity, activity role, and statement
date remain distinct; shared labels and one holding shelfmark do not create
Edition, Item, or textual equivalence. All identities and claims remain
model-made, unreviewed, or provisional.

The Antonovsky revision route adds three catalog-identified but unacquired
Editions. The Lithuanian National Library identifies the Saint Petersburg 1900
Edition and one remote holding while retaining `[s.n.]`; the Russian State
Library identifies the 1903 `2-е изд., испр.` and four remote holdings while
retaining `тип. Альтшулера` only as typography evidence; and the removed-or-
replaced RuNEB card identifies the Saint Petersburg 1907 third Edition while
retaining `тип. Ф. Вайсберга и П. Гершунина` only as typography evidence.
Each Expression owns one `embodied_by` packet, but none of these Editions owns
an Item or provision claim. No publisher, source text, rights, collation,
equivalence, new derivation, review, or semantic authority follows from
catalog projection.

The Antonovsky 1913 Edition adds the contrasting source-visible case: its
publication claim joins a cover imprint to title-page place/year, while a
separate manufacture claim preserves the following-page printer line. Both
return through exact page anchors to the same fixity-verified Item. The
provisional `Жизнь для всех` publishing imprint and brothers-Linnik printer
remain different Organizations; neither a Person substitution nor legal-entity
equivalence is asserted.

The consolidated Naumann 1893 Edition adds a second contrast: one exact title-
page literal names both `Druck` and `Verlag`. Two claims preserve those roles:
publication reaches the provisional GND-backed publisher Organization, while
manufacture reaches a separate provisional GND-backed printer Organization.
The exact `Zweite Auflage` wording remains an Edition statement, not a
universal Work ordinal, and the following-page historical translation-right
reservation creates no present rights conclusion.

The 1886 *Jenseits von Gut und Böse* Edition adds a source-independent repeat
of that role-separation invariant. Two new claims return to one exact page-3
`Druck und Verlag` literal and resolve publisher and printer through distinct
Organization routes; page 4's `Alle Rechte vorbehalten.` is separately
addressed as historical rights evidence. No Edition statement, release date,
textual equivalence, current rights decision, or human acceptance is inferred.

The 1892 second *Zur Genealogie der Moral* Edition adds a two-surface repeat:
publication returns to the page-5 publisher/year imprint, while manufacture
returns to the undated page-204 printer line. The same provisional publisher,
printer, and Leipzig identities are reused only through claim-originating
edges; the Edition year is not an exact printing-completion or release date.

Regeneration, parity checks, and source-foundation validation route through
`scripts/AGENTS.md`, which owns the catalog builder and evidence-spine
validator. A green result proves exact projection and source return, not the
truth of metadata or relations, rights, OCR, translation, review, or
semantics.
