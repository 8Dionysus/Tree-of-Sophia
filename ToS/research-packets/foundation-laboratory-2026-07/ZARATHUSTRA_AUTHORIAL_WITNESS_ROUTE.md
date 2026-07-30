# Also sprach Zarathustra: Authorial Witness and Critical Route

Status: current research route, not admitted manuscript text, author-final
reconstruction, accepted German, publication permission, translation
authority, or semantic truth

Research date: 2026-07-30

Discovery receipt:
`ToS/source-witnesses/discovery/runs/zarathustra-authorial-witness-route.2026-07-30.v1.json`

## Result

The golden kernel now has an exact route behind the four historical German
publication witnesses already registered in ToS. That route is neither one
immutable original nor one continuous manuscript:

```text
1881 / Fröhliche Wissenschaft germs
  -> mixed notebooks, loose leaves, plans, drafts, and fair-copy stages
    -> print manuscripts for parts I-III, no longer preserved
    -> surviving authorial print manuscript D 17 for part IV
      -> 1883-1884 first prints of parts I-III and 1885 private print of IV
        -> Peter Gast's corrected private-print copy plus Nietzsche leaf
          -> later public and collected editions
            -> Colli/Montinari critical text with stable eKGWB addresses
```

This is a responsibility-aware route, not a claim that every arrow is direct
or that the latest layer contains the final authorial reading. The current
critical text, historical first prints, D 17, Peter Gast's corrected copy,
and individual notebook regions answer different questions.

The stable foundation is therefore:

- persistent Work, witness, shelfmark, notebook, leaf, page, section, and
  version identities;
- provenance for every material and digital layer;
- region-level responsibility and chronology claims;
- explicit uncertainty and conflict;
- source-return from every derived observation.

It is not an agent-produced synthetic text. The corrected Peter Gast copy is
especially important: it contains Nietzsche-origin corrections that the
holding institution reports did not enter the first public part-IV edition
or later editions. “Published later” and “last authorial intention” are not
interchangeable predicates.

## Research order

The route was checked in the required order:

1. current official archive, digital-facsimile, critical-edition, rights, and
   publisher documentation;
2. established critical edition and historical-critical commentary;
3. the freshest directly relevant philology, conference, review, and
   translation surfaces;
4. general web only as a final lead surface, with selected claims reopened at
   an originating record.

No manuscript image, DFGA body, eKGWB body, restricted commentary body, or
forthcoming translation was added to ToS. No access request, photo order,
review-copy request, or purchase was sent.

## 1. Official archive layer

The current archival owner is the
[Goethe- und Schiller-Archiv ORES database](https://ores.klassik-stiftung.de/ords/f?p=401:910).
Its records expose one surviving print manuscript for the four-part work:

| Witness | Current archive identity | Contribution | Digital state |
| --- | --- | --- | --- |
| Nietzsche print manuscript for part IV | [GSA 71/25, ORES 75091](https://ores.klassik-stiftung.de/ords/f?p=401:2:::::P2_ID:75091), Mette D 17, DFGA `D-17` | authorial print-ready manuscript for the 1885 private print, not a manuscript for parts I-III | [270-canvas IIIF manifest](https://ores.klassik-stiftung.de/ords/rest_api/iiif/digi_gsa/75091/manifest) |
| Peter Gast private-print working copy | [GSA 71/25a, ORES 471226](https://ores.klassik-stiftung.de/ords/f?p=401:2:::::P2_ID:471226), E 37 (D 17) | separate 1885 print witness in Köselitz/Gast's hand-use lineage; must not be collapsed into D 17 | [156-canvas IIIF manifest](https://ores.klassik-stiftung.de/ords/rest_api/iiif/digi_gsa/471226/manifest) |

The current critical-edition sigla state that the print manuscripts for
parts I-III are not preserved. The archive search and the 2024 commentary
expose no competing current D-manuscript identity for those parts. ToS should
say “not preserved according to the current critical route,” not infer where,
when, or why they disappeared.

### Notebook and loose-leaf families

The archive does preserve abundant compositional material. The following is a
route inventory, not a claim that every page belongs to *Zarathustra*:

| Stage or pressure | GSA / ORES / Mette route | Current archive description and limit |
| --- | --- | --- |
| early germs and part I | `71/198` / `75383` / N V 8; `71/199` / `75384` / N V 9; `71/200` / `75385` / N VI 1 | *Fröhliche Wissenschaft* and part-I material coexist; work membership must be region-level |
| parts I-II | `71/201` / `75386` / N VI 2; `71/202` / `75387` / N VI 3; `71/203` / `75388` / N VI 4 | part-II preparation across multiple notebooks, not one source text |
| transition to parts III-IV | `71/204` / `75389` / N VI 5; `71/205` / `75390` / N VI 6; `71/206` / `75391` / N VI 7; `71/207` / `75392` / N VI 8 | plans and drafts overlap; N VI 8 was also reused in 1888 |
| part IV preparation | `71/208` / `75393` / N VI 9 | direct part-IV preparation before D 17 |
| plans and “Heilige Gelächter” | `71/136` / `75235` / Z I 2; `71/138` / `75238` / Z I 4 | includes plans, studies, and material for adjacent works; Z I 4 is important to part II |
| parts III-IV and later reuse | `71/139` / `75239` / Z II 1 through `71/148` / `75250` / Z II 10 | draft family for parts III-IV; several books later contain *Ecce Homo*, *Nietzsche contra Wagner*, or revaluation material |
| fair-copy and later-work overlap | `71/149` / `75258` / W I 1; `71/150` / `75259` / W I 2 | fair-copy stages for parts III-IV also lead toward later work |
| loose-leaf complex | `71/231` / `212281` / Mp XV 1-3 | period material distributed across three DFGA subdivisions |

The exact checked IIIF manifests declare their digitized objects `Public
Domain`. This is positive object-level evidence, not a complete ToS
publication decision. The current
[GSA usage route](https://www.klassik-stiftung.de/goethe-und-schiller-archiv/benutzung/)
and
[copy, digitization, and publication-permission route](https://www.klassik-stiftung.de/goethe-und-schiller-archiv/benutzung/kopien-digitalisate-publikationsgenehmigung/)
separately govern uses of reproductions beyond personal use. Public payload
reuse remains closed until one exact intended use is reviewed against both
layers.

### Exact archive/facsimile asymmetry

`Z II 2` ([GSA 71/140, ORES 75240](https://ores.klassik-stiftung.de/ords/f?p=401:2:::::P2_ID:75240))
currently exposes zero ORES canvases, while the DFGA `Z-II-2` API responds
with 20 image records and 56 logical identifiers. This is not a contradiction
to erase. It records two digital representations with different current
coverage and access states.

## 2. Official digital facsimile and critical-edition layers

### DFGA

The
[Digitale Faksimile Gesamtausgabe](http://www.nietzschesource.org/DFGA/D-17)
gives manuscript-level address routes without making every notebook a
*Zarathustra* object.

| Family | Current API observation | Foundation use |
| --- | --- | --- |
| `D-17` | public flag true; 240 image records / 300 logical identifiers | authorial print manuscript for part IV; DFGA describes 135 quarto leaves and Nietzsche's authorial restriction to friends |
| `Z-I-2`, `Z-I-4` | 64 / 188 and 123 / 365; current public flags false | early plans and part-II preparation |
| `Z-II-1` … `Z-II-10` | 541 image records / 1,583 logical identifiers across ten books; mixed public flags | part-III/IV draft family and later reuse |
| `W-I-1`, `W-I-2` | 85 / 251 and 87 / 257; public flags false | fair-copy stages and later-work overlap |
| `N-V-8`, `N-V-9`, `N-VI-1` … `N-VI-9` | 887 image records / 2,617 logical identifiers across eleven books; public flags true | early germs through part-IV preparation |

ORES and DFGA counts are representation-specific. For D 17, 270 ORES IIIF
canvases, 240 DFGA image records, and 300 DFGA logical identifiers must never
be asserted as a one-to-one page mapping without a separate collation.

The DFGA API and application currently respond only over HTTP from this
machine; HTTPS attempts failed. A responding API or `isPublic` flag is not
permission. No DFGA response body or image was retained.

The current
[Nietzsche Source rights page](https://doc.nietzschesource.org/en/rights)
states CC BY-NC-ND 4.0 for Nietzsche Source content while separately
describing a DFGA agreement permitting non-commercial derivatives. Those
scopes and the GSA reproduction rules are not yet reconciled for a public ToS
derivative. Private research routing is useful; public reuse remains blocked.

### eKGWB

The
[eKGWB documentation](https://doc.nietzschesource.org/en/ekgwb)
identifies the Colli/Montinari digital critical edition and stable siglum
addresses. Two bounded observations of each part returned byte-identical
responses within the part:

| Part | Bytes | SHA-256 of transient response | Stable content IDs | Correction popups |
| --- | ---: | --- | ---: | ---: |
| `Za-I` | 180,138 | `3ad9b3f601eccf45446c71f2758d1aa5fd70fb01cc67c81453526f1b7eeb7061` | 35 | 5 |
| `Za-II` | 172,712 | `5d68ed6c8c360aba975d6ce610eff235f2a47317e4b4772c0016af9b137273f4` | 24 | 5 |
| `Za-III` | 237,239 | `b490c0c8d85d7dc9bb72e278c3f4c94b2db6ad7b28b9afdf7a9a2496360c08e1` | 62 | 14 |
| `Za-IV` | 262,163 | `18956c8e7c03ebf4c08919cdf192139c68e64a3d2babffff68410253caf56d61` | 60 | 23 |

The route now exposes 181 stable critical content addresses and 47 correction
popups across the whole work. These are critical comparison addresses, not
manuscript leaves, admitted source text, publisher-authenticated transport,
or proof that the critical reading contains every late authorial correction.
No response body was retained.

## 3. Established scholarly baseline

Katharina Grätz's 2024 historical-critical commentary supplies the current
whole-work bridge:

- [volume 4/1, parts I-II](https://doi.org/10.1515/9783110293319);
- [volume 4/2, parts III-IV](https://doi.org/10.1515/9783110293333).

The public publisher previews, not the restricted full bodies, were
consulted. They place the actual four-part composition from November 1882 to
February 1885, distinguish the 1883, 1884, and 1885 publication stages, expose
the open fifth-part horizon, and identify D 17 as the surviving part-IV print
manuscript. They do not turn every preceding note into part of the work.

The KSA 14 sigla provide the direct established statement that the print
manuscripts for `Za I-III` are not preserved while `D 17` is the authorial
print manuscript for `Za IV`. This corrects two shortcuts:

```text
all four published parts have one surviving authorial manuscript
all notebook material from 1882-1885 belongs to Zarathustra
```

Both are false.

The Stanford Complete Works
[preliminary note to volume 14](https://www.sup.org/books/theory-and-philosophy/unpublished-fragments-period-thus-spoke-zarathustra-summer-1882-2)
adds a useful corpus boundary: thirty-six manuscripts from July 1882 to
autumn 1885 comprise twenty-one large notebooks, twelve pocket notebooks, and
three loose-leaf binders. Approximately half of their material is described
as not literarily related to *Zarathustra*. Future attribution must therefore
be region- and claim-scoped, never notebook-wide by default.

## 4. A distinct late-correction witness

The official-cultural acquisition account
[Buch mit Biss](https://www.kulturstiftung.de/buch-mit-biss/)
documents an ensemble now routed as GSA 71/25a:

- one of the few surviving 1885 private-print copies;
- Peter Gast's working annotations;
- an accompanying Nietzsche manuscript leaf with previously unknown
  corrections;
- small wording changes and a six-line extension to `Das Honig-Opfer`;
- a report that the changes did not enter the first public part-IV edition or
  later editions.

This ensemble is not a replacement for D 17 or the critical edition. It is a
separate correction witness that can test an exact region. Before any
content-bearing collation, ToS needs item-level rights review, exact leaf/page
addressing, and a question-specific source-visible A/B/C plan.

## 5. Current and freshest relevant work

### 2025 manuscript continuity

The 28 June 2025 Nietzsche Documentation Center conference
[*Zarathustra als Erzieher*](https://www.nietzsche-gesellschaft.de/assets/Programm-ZA.pdf)
uses the exact DFGA address `D-17,5vet6` on its program cover. This is current
evidence that D 17 remains an active manuscript surface in specialist work,
not evidence that the conference supersedes archive or critical-edition
authority.

A 2025 *Nietzsche-Studien*
[review of Grätz's commentary](https://doi.org/10.1515/nietzstu-2025-0022)
emphasizes the work-in-progress character of the four-part composition,
especially part IV. It supports an open genesis model; it does not create a
new witness identity.

The freshness search through 2026-07-30 found no 2026 philological work that
changes the current shelfmark, D 17 identity, or notebook family. “Nothing
superseded the route” is a dated search result, not a timeless claim.

### Forthcoming August 2026 translation

Stanford University Press currently lists
[*Thus Spoke Zarathustra*, Complete Works volume 7](https://www.sup.org/books/theory-and-philosophy/thus-spoke-zarathustra),
translated with an afterword by Paul S. Loeb and David F. Tinsley, for August
2026. The publisher record gives 424 pages, ISBNs `9780804728799` and
`9781503647282`, cross-references to notebook variants, a translation-issues
afterword, and a key-term glossary.

On the research date the volume is forthcoming. It is therefore registered
only as a deferred translation-reference candidate:

- no content has been inspected or acquired;
- publisher description is not passage-level quality evidence;
- “new” does not mean “recognized” or “best”;
- no review-copy request has been sent;
- refresh is due after actual publication and manifestation verification.

This is the principal 2026 development directly relevant to the translation
laboratory. It does not change the manuscript route.

## Exact distinctions preserved

### Print manuscript versus print

`D 17` is the authorial print-ready manuscript for part IV. `E 37` is the
private print. GSA 71/25a is a particular corrected private-print ensemble.
The 1891/1892 public part-IV witness is another publication state.

### Notebook versus work region

A notebook can contain preparation for several works and later reuse. A
notebook identity is stable; whole-notebook *Zarathustra* membership often is
not.

### Archive canvas versus DFGA address

ORES canvases, DFGA image records, DFGA logical identifiers, manuscript
leaves, printed pages, and eKGWB section IDs are different address spaces.
Mappings between them require explicit evidence.

### Access versus permission

`Public Domain` in an exact ORES manifest, an HTTP response, and a DFGA
`isPublic` flag are not interchangeable rights grants. GSA reproduction rules
and Nietzsche Source terms remain separate.

### Published sequence versus last authorial correction

The Peter Gast/Nietzsche correction ensemble breaks the assumption that a
later public edition necessarily includes every available authorial change.
Claims about “final text” must be region-specific and source-cited.

## Consequences for ToS

1. Keep `tos.work.friedrich-nietzsche.also-sprach-zarathustra` as the stable
   work identity and add this route as a source reference.
2. Do not create a ToS `Item` for a remote manuscript or print without
   custody, local fixity-bound payload, rights evidence, and an acquisition
   event.
3. When manuscript work begins, mint distinct remote-witness identities
   before any content-bearing derivative; never overload historical
   publication Items.
4. Attribute draft membership by exact region and source claim. Mixed
   notebook metadata is a discovery bound, not a work-membership assertion.
5. Keep DTA historical first prints, D 17, GSA 71/25a, and eKGWB as separate
   comparison lanes.
6. Open human review only for a concrete promotion question, not for all
   manuscripts, pages, or 181 critical addresses.
7. Keep the Stanford 2026 translation sealed until publication,
   manifestation, rights, and passage-level suitability are independently
   checked.

## First bounded content-bearing experiment, when authorized

The highest-value first soil is one region touched by the GSA 71/25a
corrections, because it can test whether the existing historical and critical
routes omit a late authorial layer.

```text
A: D 17 manuscript region
B: 1885 private-print / Peter Gast corrected-copy region
C: eKGWB critical reading plus the registered historical public-print region
```

Prerequisites:

- exact leaf/page/section crosswalk;
- source-visible image or authorized reproduction access;
- item-level rights decision for the intended private laboratory use;
- independent diplomatic readings before comparator reveal;
- German-competent review only for the selected region;
- preservation of disagreements and non-equivalence.

Until then, this packet promotes route mechanics only. It admits zero
manuscript text, German readings, translation output, signs, concepts,
relations, or graph edges.
