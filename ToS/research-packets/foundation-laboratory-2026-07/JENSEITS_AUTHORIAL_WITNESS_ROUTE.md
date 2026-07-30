# Jenseits von Gut und Böse: Authorial Witness and Critical Route

Status: current research route, not admitted manuscript text, accepted German,
author-final reconstruction, translation authority, publication permission,
semantic truth, or a scheduled human task

Research date: 2026-07-30

Discovery receipt:
`ToS/source-witnesses/discovery/runs/jenseits-authorial-witness-route.2026-07-30.v1.json`

## Result

The first cross-work soil beyond the *Zarathustra* golden kernel now has an
exact upstream route:

```text
mixed notebook and dictated-sheet regions
  -> W I 3-W I 8 region-specific drafting and reworking
    -> cut, assembled, and repeatedly reordered print manuscript D 18
      -> partial proof/correction witness HAAB C 4615
        -> Naumann 1886 first print
          -> Nietzsche's later corrected hand copy HAAB C 4619
            -> 1894 Köselitz numbering intervention and later reversion
              -> Colli/Montinari critical text with stable eKGWB addresses
```

This is not a single linear text. D 18 already preserves several compositional
stages; the notebooks contain material for other works; correction copies can
carry readings later than the first print; and the critical text answers a
different responsibility question from an authorial manuscript or historical
edition.

The stable foundation is therefore:

- persistent Work, shelfmark, notebook, leaf, proof, edition, item, section,
  and version identities;
- provenance and responsibility for every material and digital layer;
- region-level membership claims rather than whole-notebook attribution;
- source-visible comparison between independently addressed witnesses;
- preserved disagreement, uncertainty, and representational asymmetry.

No source body, manuscript image, accepted reading, semantic annotation,
translation candidate, new Item, publication route, or recurring human queue
is created by this research.

## Research order

The route was checked in the required order:

1. current official archive, library, facsimile, critical-edition, rights, and
   independent first-print documentation;
2. established genetic scholarship and historical-critical commentary;
3. the freshest directly relevant chronology and textual-genetic method;
4. general web only as a final lead surface, with claims reopened at their
   originating records.

The freshness search ran through 2026-07-30. It found no 2026 philological
publication changing the current D 18 shelfmark, identity, or manuscript
family. That is a dated negative result, not a timeless assertion.

## 1. Official archive layer

### D 18: the print manuscript

The current
[Goethe- und Schiller-Archiv ORES record](https://ores.klassik-stiftung.de/ords/f?p=401:2:::::P2_ID:75093)
identifies:

| Field | Current value |
| --- | --- |
| archive identity | `GSA 71/26` |
| Mette siglum | `D 18` |
| DFGA siglum | `D-18` |
| archive class | `Nietzsche, Friedrich > Werke > Druckmanuskripte` |
| embodiment relation | print manuscript for Naumann, Leipzig 1886 |
| extent and hand | 108 leaves, mostly Nietzsche's hand and partly another hand |
| current digital route | [232-canvas IIIF manifest](https://ores.klassik-stiftung.de/ords/rest_api/iiif/digi_gsa/75093/manifest) |
| exact digital-object label | `Public Domain` |

The current
[Kalliope SRU result](https://kalliope-verbund.info/sru?version=1.2&operation=searchRetrieve&query=ead.unitid%3D%22GSA%2071%2F26%22&recordSchema=ead&maximumRecords=20)
preserves a parent record and nine child groups:

| Leaves | Catalogued group |
| --- | --- |
| 1-14 | preface and first main section |
| 15-26 | second main section |
| 27-36 | third main section |
| 37-44 | fourth main section |
| 45-55 | fifth main section |
| 56-64 | sixth main section |
| 65-76 | seventh main section |
| 77-87 | eighth main section |
| 88-108 | ninth main section |

This archival subdivision is useful navigation. It is not proof of an exact
content boundary for every later addition, the concluding poem, or every
reassembled leaf.

### Preparatory notebooks

The archive identifies a bounded W I family as “studies from the revaluation
period, partly used in *Jenseits*”:

| Mette | GSA | ORES | Archive extent | ORES canvases |
| --- | --- | --- | ---: | ---: |
| W I 3 | `71/151` | `75260` | 49 leaves | 144 |
| W I 4 | `71/152` | `75261` | 27 leaves | 52 |
| W I 5 | `71/153` | `75262` | 24 leaves | 50 |
| W I 6 | `71/154` | `75263` | 40 leaves | 84 |
| W I 7 | `71/155` | `75264` | 38 leaves | 84 |
| W I 8 | `71/156` | `75265` | 145 leaves | 292 |

Every checked manifest labels its exact digital object `Public Domain`.
That does not make an entire notebook a *Jenseits* witness. “Partly used” is a
region-level warning: a future claim needs an exact leaf or other bounded
address and a source for the attribution.

Two loose-leaf complexes require even stricter restraint:

- `Mp XV` (`GSA 71/231`, ORES `212281`) belongs to an earlier mixed period.
  The current archive title does not assign the whole complex to *Jenseits*.
- `Mp XVI` (`GSA 71/232`, ORES `212283`) is catalogued for later works, not
  *Jenseits*. Historical-critical commentary connects exact dictated leaves
  to the preface and §§42-43; those regions may be routed, but the whole
  complex may not.

### Archive and facsimile counts do not collapse

The same D 18 material currently exposes different representation counts:

| Layer | Current observation |
| --- | --- |
| ORES/Kalliope archive | 108 leaves, 232 IIIF canvases |
| DFGA API | 116 folio sheets, 224 image records, 240 logical identifiers, current pagination 1-105 |

This is a codicological and digital-representation asymmetry. ToS must not
invent a one-to-one mapping among leaf, sheet, canvas, image record, logical
identifier, and current manuscript page.

## 2. Official correction-copy layer

The Herzogin Anna Amalia Bibliothek preserves two distinct 1886 correction
witnesses under the same bibliographic work record:

| Witness | Official record | Scope | Digital route |
| --- | --- | --- | --- |
| Nietzsche's correction copy | `C 4619`, PPN `30939905X` | complete copy with Nietzsche provenance, marks, marginalia, and the label `Korrekturexemplar` | [285-canvas IIIF manifest](https://haab-digital.klassik-stiftung.de/viewer/api/v1/records/1216143412/manifest/) |
| partial correction proof | `C 4615`, PPN `30939905X` | title leaf, pp. III-VI, two leaves, pp. 35-48 and 65-271; sheets 1, 2, and 4 absent; Naumann stamp and Nietzsche corrections | [232-canvas IIIF manifest](https://haab-digital.klassik-stiftung.de/viewer/api/v1/records/1649470630/manifest/) |

The official
[electronic-reproduction record](https://opac.lbs-weimar.gbv.de/DB=2/XMLPRS=N/PPN?PPN=642420394)
binds those digital objects to their physical templates and exposes the URNs
`urn:nbn:de:gbv:32-1-10013792115` and
`urn:nbn:de:gbv:32-1-10025989559`.

The manifests expose the literal placeholder `iiif_license`, not a usable
rights declaration. Open viewing and IIIF availability are not publication
permission. Current
[HAAB use rules](https://www.klassik-stiftung.de/herzogin-anna-amalia-bibliothek/benutzung/benutzungsordnung/)
and
[copy/scan guidance](https://www.klassik-stiftung.de/herzogin-anna-amalia-bibliothek/benutzung/kopieren-scannen-drucken/)
retain a separate consent route for reproduction and publication. These
witnesses are therefore valuable for private source-visible research while
public derivative reuse remains unresolved for an exact intended use.

## 3. Official digital critical and independent print layers

### DFGA

The bounded same-day DFGA observation of `D-18` reported:

- 224 image records and 240 logical identifiers;
- 116 folio sheets, written on one side and partly assembled from smaller
  sheets;
- current pagination 1-105;
- a missing title page;
- almost entirely Nietzsche's hand, with aphorism 16 in Louise
  Röder-Wiederhold's hand;
- `isPublic: false`.

Later on 2026-07-30 the same Nietzsche Source endpoints timed out from this
machine. Both observations are retained. A transient response, a public flag,
or an application route is never treated as permission or as locally held
source material.

The current
[Nietzsche Source rights page](https://doc.nietzschesource.org/en/rights)
states CC BY-NC-ND 4.0 for Nietzsche Source content and separately describes
an agreement for non-commercial DFGA derivatives. Those scopes are not
silently combined with GSA or HAAB rules.

### eKGWB

Two bounded observations of the
[eKGWB JGB include](http://www.nietzschesource.org/resources/scripts/static_html_include.php?book=%23eKGWB%2FJGB)
were byte-identical:

- 624,405 bytes;
- SHA-256
  `143d6330c94bf00b135f887eeed5f20f6caee6a98174746227af255583ae8b20`;
- 302 unique stable content IDs: 296 numbered aphorisms, `65a`, `73a`,
  `237[a]`, title, preface, and concluding poem.

The response body was not retained. The same counter observed zero
correction-popup elements; that bounded observation must not be generalized
into a claim that the critical edition contains no corrections.

The IDs are comparison addresses, not admitted text, manuscript foliation,
proof of critical completeness, or authorization to publish a derivative.

### Independent first-print route

The current
[Zentralbibliothek Zürich e-rara record](https://www.e-rara.ch/zuz/content/titleinfo/20295083)
supplies an independent official-library route to the 1886 Naumann first
print:

- shelfmark `43.273`;
- DOI `10.3931/e-rara-73194`;
- VI + 266 pages;
- IIIF, OCR, and ALTO routes;
- Public Domain Mark and explicit citation guidance.

ToS already holds a fixity-verified local 1886 first-print package from a
different route. The e-rara object is registered as a remote alternative, not
downloaded, not turned into a duplicate Item, and not used to accept OCR.

## 4. Established genetic and commentary baseline

Beat Röllin's 2013 study,
[*Ein Fädchen um’s Druckmanuskript und fertig? Zur Werkgenese von Jenseits von Gut und Böse*](https://doi.org/10.1515/9783110298901),
provides the principal established genetic account:

- D 18 is largely complete except for the title page;
- its first recoverable arrangement had 308 aphorisms;
- sheets were cut, assembled, reordered, and renumbered repeatedly;
- late additions include §258 and the concluding poem;
- a final compositional change occurred around mid-July 1886.

Andreas Urs Sommer's 2016
[*Kommentar zu Nietzsches Jenseits von Gut und Böse*](https://doi.org/10.11588/diglit.69929)
adds the public historical-critical bridge:

- only six of the 308 aphorisms in the first recoverable D 18 state are not
  identifiable;
- D 18 is already a late, internally layered stage;
- typesetting and correction proceeded in June and July 1886, with Peter
  Gast/Köselitz involved;
- the 1894 Köselitz numbering intervention differs from the 1886 state, while
  later editions returned to the earlier numbering;
- Nietzsche's C 4619 copy carries later authorial corrections, including the
  `65a` and `73a` numbering;
- parts of the preface and the route to §§42-43 connect to exact dictated
  leaves in `Mp XVI`, not to the whole complex.

The 2023 bibliographic form of
[KGW IX 14, Nachbericht zur neunten Abteilung](https://doi.org/10.1515/9783111022284)
is the established manuscript-description, concordance, correction, and
cross-reference control. These sources do not make genesis linear or make
the first print automatically later than every authorial correction.

## 5. Freshest directly relevant work

Beat Röllin's 2024
[*Chronologie der Manuskripte 1885–89. Nachtrag zu KGW IX*](https://doi.org/10.1515/nietzstu-2024-0001)
is the current chronology control. This route uses it to test dates and
sequence, not to claim a changed D 18 identity without a passage-level
demonstration.

Axel Pichler's 2025 chapter,
[*Genese – Kontext – These*](https://doi.org/10.24894/978-3-7965-5300-4),
is the freshest direct methodological bridge to ToS. Its *Jenseits* §22
example compares:

```text
W I 7, GSA 71/155, leaves 44v-45r
  -> D 18, GSA 71/26, leaf 27r
    -> published JGB §22
```

The comparison tracks semantic change across rewriting while explicitly
separating textual genesis from philosophical validity. It demonstrates why
different wording stages may differ semantically and why a graph may not
collapse them into one proposition.

This is the strongest first content-bearing transfer experiment currently
identified for ToS. It is a candidate, not a started experiment.

## 6. First bounded A/B/C candidate

If and only if a concrete question justifies content-bearing work:

- **A — early region:** W I 7, leaves 44v-45r;
- **B — print manuscript:** D 18, leaf 27r;
- **C — publication/critical comparison:** 1886 print §22 and the stable
  eKGWB address;
- **conditional correction control:** C 4615 and C 4619 only if the same
  region is present and relevant.

Before any output:

1. resolve exact leaf, page, and section crosswalks;
2. record source-specific access and intended-use rights;
3. obtain independent diplomatic readings for the selected German regions;
4. require German-competence evidence for acceptance;
5. freeze the unassisted comparison before revealing commentary or a
   recognized interpretation;
6. preserve disagreements rather than forcing one answer;
7. measure quality, time, compute, and correction effort separately.

The present route performs none of those content-bearing steps. It only makes
their evidence boundary explicit.

## 7. What is admitted now

Admitted:

- one metadata-only route from mixed preparatory regions through D 18,
  correction witnesses, the 1886 print, and critical addresses;
- current official identifiers and access states;
- representation-specific counts without false one-to-one equivalence;
- distinct object-level and institutional rights evidence;
- established and fresh scholarly controls;
- one deferred, exact §22 A/B/C candidate.

Not admitted:

- any remote source body or facsimile as a local Item;
- any manuscript, proof, OCR, or eKGWB text;
- any author-final, preferred, or synthetic German reading;
- any whole-notebook *Jenseits* membership claim;
- any semantic proposition, concept, relation, or graph edge;
- any translation judgment;
- any public reuse permission;
- any Human Gold assignment or standing review backlog.

## Next action gate

Do not broaden this route by processing whole notebooks or whole works. Open a
content-bearing episode only when one exact philosophical or textual question
requires it. The first defensible candidate is §22 because the current fresh
scholarship supplies an exact three-stage route. Even then, source access,
rights, German competence, independent reading, and blind comparison must be
ready before a candidate output exists.
