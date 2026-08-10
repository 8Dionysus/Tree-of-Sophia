# DTA Zarathustra Parts I-IV Layered Rights Assessment

Status: model-authored rights research; exact upstream route positively
identified; no legal advice, human rights review, source-text acceptance,
operator transfer approval, or publication decision

Research snapshot: 2026-08-02; United States foreign-work route corrected and
exact DTA license state rechecked 2026-08-10

## Question

What may Tree of Sophia say about the four exact DTA TEI P5 Items for the
first-publication parts of *Also sprach Zarathustra*, and what may a future
server do with them?

The answer must keep six different questions separate:

1. Is Nietzsche's underlying Work still protected in Germany or the United
   States?
2. Does the historical printed Edition add a still-active right?
3. What license covers DTA's annotated TEI serialization?
4. Does the distinct DTA pure-text download inherit the same posture?
5. Do DTA's displayed facsimiles inherit the text license?
6. Does a positive upstream answer authorize transfer of the operator's local
   ToS payload?

## Result

The four exact DTA annotated-TEI routes have positive current open-license
evidence. Each freshly generated TEI header states CC BY-SA 4.0, and the
current DTA terms state that DTA full texts have used CC BY-SA 4.0 since
2020-06-16. The OAI Dublin Core endpoints still emit the former CC BY-NC 3.0
label. That field is stale evidence which must remain visible, but the current
terms themselves explain the temporal transition and the exact current TEI
headers agree with the newer license.

The rights records can therefore move from `conflicting_evidence` to
`licensed` for the exact annotated TEI Items. This does **not** accept their
German, make them critical editions, or authorize publication of ToS's local
copies. The user has established a stricter operator rule: local research
payloads stay local. A future public route must reacquire a then-current,
explicitly licensed upstream object, give it its own fixity and provenance,
recheck the license, and receive human rights plus operator transfer approval.

The positive historical-layer result is unchanged, but its independent United
States basis is corrected. These are German publications, so the domestic
pre-1931 cutoff is context rather than a complete analysis. Under 17 U.S.C.
§104A, the Nietzsche Work and conservative historical-presentation terms had
already expired in Germany before the possible 1996 restoration date and fail
the restored-work source-country condition.

## Exact objects and current response evidence

| Part | DTA identifier | Local payload SHA-256 | Current TEI-header SHA-256 | Current OAI-DC SHA-256 |
| --- | --- | --- | --- | --- |
| I | `nietzsche_zarathustra01_1883` | `d3fa5f8af39d87c8f0ede7a0b52b26da0c57e73125d3b0bd2a836b80eae24a4b` | `22c342de8160500402ff09ae6c970cefbf016db805560144293b1d118a90e58b` | `371d74b552cc457206b1f019312fd16515a53da6cc62f61917e70d3a3b7f7e88` |
| II | `nietzsche_zarathustra02_1883` | `b78a09d74bcc1dd9610b81ee348387942a5df1f9e6c8a4340fb7f2221d8372cb` | `06569a88018d4f816ad2bd4a3c86cb90bf91d25dd90799a9a2306f077661bf91` | `1dc8d569928e3b3d9b731210119e2d9e8f3b241c7c16ed0d03af13548d5e993` |
| III | `nietzsche_zarathustra03_1884` | `722dbdab2a3d28ea1691dad4c9fbec62bb7f4caf2699212fd7eef40f1ac8f331` | `109509505cfcd433d0ab1ff4b3adf5f475b82f4c0d6fd339efe1a98dabc47dc6` | `7b064c9e8818f2c336ae2d48f3e0488c7afec18e50dcefb4616e82e731665afa` |
| IV | `nietzsche_zarathustra04_1891` | `ffc34ce6bea3f0b906c37f313bc66e5e58d9a8b0e69b53ea497a778c15dadd0d` | `ac53e18ed35d3a14de65e2f9d009814b4af45eb4c48da83e656210759f6bab51` | `40d0796d4c03c8df38d8b5200b517cd31d24442c05b560b1ad5098dcaa6955f2` |

The live headers are generated responses and contain a generation timestamp;
their response digests are observation fixity, not permanent object IDs. The
ToS local payload digests remain the immutable identity of the acquired Items.

The shared live DTA terms response was 17,139 bytes with SHA-256
`ea02c2b636b5b04c3c5d5c4641149053405daa08fc3ff4b9293c21c67c1f9029`.
The current responses were captured directly over HTTPS on 2026-08-02; no
facsimile or new source-text payload was downloaded.

A bounded live recheck on 2026-08-10 again found CC BY-SA 4.0 in all four
exact generated TEI headers. The current response digests were:

| Surface | Current response SHA-256 |
| --- | --- |
| DTA terms | `8496e2837066ead8e2d462a438763bc912e74b4be27f828a7e459fa626eab857` |
| Part I TEI header | `5e9aa38746a00b5156e8d75a6517facb6a6034505c89f083c4e5e72ec8706361` |
| Part II TEI header | `6af19d55138a2008d49ff9c3af8e90eb21647e1b44d5f6ab33009855a5809d5c` |
| Part III TEI header | `739f2329bac338800c9015fcfd6138fb59cf51c2a80cd0bc901048b30f8f9ca7` |
| Part IV TEI header | `8af486974b593e6ad89d9c8e3366e8639442d9361ec142e68986d4ea66520f0d` |

The generated-header digests changed with the generated response state; the
license expression did not. No response, facsimile, or source-text payload was
retained in Git.

## Classical and official documentation

### Exact DTA provider surfaces

The [DTA terms](https://www.deutschestextarchiv.de/doku/nutzungsbedingungen)
say:

- DTA full texts are CC BY-SA 4.0 unless otherwise marked;
- that license has applied since 2020-06-16 and replaced CC BY-NC 3.0;
- reuse should attribute `Deutsches Textarchiv` as creator of the electronic
  version;
- the distinct plain-text form, without XML/HTML annotation, is described as
  unrestricted public-domain text;
- displayed facsimiles are separate, normally limited to scientific/private
  noncommercial display, and third-party image publication requires
  coordination with the holding library;
- DTA does not warrant completeness, correctness, uninterrupted operation, or
  permanent availability.

Each exact live TEI header independently states that the annotated object is
distributed under
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). It also says
that OCR was checked by native speakers under DTA transcription rules. The
quality statement is external editorial evidence, not a ToS correctness
verdict.

The four exact OAI Dublin Core responses still state `Creative Commons
Attribution-NonCommercial 3.0 Unported License`. Because the current terms
date that license as the pre-2020 state and all four exact current headers use
BY-SA 4.0, ToS records the OAI values as stale metadata rather than silently
discarding them or letting them override the more specific current surfaces.

### License conditions

The [CC BY-SA 4.0 legal
code](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en) grants a
worldwide, royalty-free, non-exclusive and irrevocable right to reproduce,
share and adapt the licensed material. Shared material requires attribution,
a license reference, change indication where applicable, and ShareAlike for
adapted material. Downstream technological or legal restrictions may not
remove freedoms granted by the license.

The DTA terms also contain broad introductory language about noncommercial
use. ToS does not erase that wording. The exact license grant and current TEI
headers are the positive route, while the extra prose is retained as a reason
for a fresh human rights check before a real public deployment. This research
does not give legal advice.

### Historical Work and Edition layers

Germany's [UrhG section
64](https://www.gesetze-im-internet.de/urhg/__64.html) sets the ordinary term
at life plus seventy years, and [section
69](https://www.gesetze-im-internet.de/urhg/__69.html) calculates the term from
the end of the relevant calendar year. Nietzsche died in 1900, so the
ordinary German Work term ended no later than 1970-12-31.

[UrhG section 70](https://www.gesetze-im-internet.de/urhg/__70.html) gives a
qualifying modern scientific edition a twenty-five-year related right. The
four historical print witnesses are not silently classified as modern
scientific editions; even a conservative analogous publication-date check
would have ended in 1908, 1909, or 1916.

The U.S. Copyright Office's April 2026 revision of [Circular
15A](https://www.copyright.gov/circs/circ15a.pdf) states that works published
in the United States before 1931 are now public domain. These four Editions
were published in Germany, however, so that domestic line is not an
independent foreign-work conclusion. [17 U.S.C.
§104A](https://www.copyright.gov/title17/92chap1.html#104a) defines a restored
work to exclude a work already in the public domain in its source country
through term expiration. The Copyright Office's current
[URAA/GATT overview](https://www.copyright.gov/gatt.html) confirms that the
relevant restoration date for then-eligible countries was 1996-01-01.

Nietzsche's German Work term ended in 1970. Even the deliberately conservative
section-70 analogy for the historical presentation ended in 1908, 1909, or
1916. Both historical layers were therefore outside German protection before
possible United States restoration. This §104A source-country condition—not
the domestic pre-1931 shorthand—is the independent United States basis. It
does not turn DTA's later annotation into public domain; that exact digital
layer remains governed by CC BY-SA 4.0.

## Established scholarship and practice

Haaf, Geyken, and Wiegand's 2015 article on the [DTA Base
Format](https://doi.org/10.4000/jtei.1114) explains why a constrained TEI P5
subset, documentation, and consistent annotation are needed to integrate
historical print corpora. It supports treating the TEI structure as a real
curated digital layer rather than pretending it is only the old book text.

Kampkaspar's 2017 [RIDE review of
DTA](https://ride.i-d-e.de/issues/issue-6/deutsches-textarchiv/) treats DTA as
a high-quality reference corpus because of its accurate transcriptions and
documentation, while also noting its first-edition corpus purpose and lack of
historical-critical variant commentary. That makes DTA strong research soil,
not a Nietzsche critical edition or ToS gold by reputation alone.

The established distinction between old Work, historical Edition, digital
facsimile, transcription, and annotation is therefore both legal and
editorial. One blanket `open` or `closed` label would lose the information ToS
needs later.

## Fresh and currently relevant checks

The current snapshot adds four freshness signals:

1. All four DTA headers generated on 2026-08-02 still carry CC BY-SA 4.0.
2. All four OAI records still carry the pre-2020 BY-NC 3.0 field, proving the
   metadata drift is current rather than historical conjecture.
3. Current 17 U.S.C. §104A and the Copyright Office URAA/GATT overview retain
   the source-country-protection condition and 1996 restoration date; the
   April 2026 Circular 15A domestic cutoff is not substituted for that route.
4. Creative Commons' 2025 [cultural-heritage license
   guidance](https://creativecommons.org/2025/07/09/recommended-licenses-and-tools-for-cultural-heritage-content/)
   recommends separating public-domain originals, faithful reproductions,
   born-digital institutional contributions, and metadata rather than
   applying one indiscriminate license.

The 2026 WIPO cultural-heritage rights toolkit remains useful as a current
workflow signal: record object, layer, source, jurisdiction, uncertainty, and
reuse conditions explicitly. It is a draft toolkit, not binding law and not a
substitute for an object-specific decision.

## General web search, last

Only after the provider, law, license, and established-source checks, exact-ID
general searches were run. They returned ordinary citations to DTA and
TextGrid repository alternates. TextGrid exposes related DTA-derived objects
under its own CC-BY/CC0 metadata route, but those are separate repository
objects and licenses. They are not inherited into the four current ToS Items.

No credible exact-object source was found that contradicts the live DTA
header/terms transition. General search also returned many irrelevant Richard
Strauss and generic Zarathustra results, confirming why it belongs last.

## Layer decisions

| Layer | Rights result | ToS consequence |
| --- | --- | --- |
| Nietzsche Work | `public_domain_reviewed` in DE/US | reusable at the Work layer; no modern critical text is selected |
| 1883/1884/1891 historical presentation | `public_domain_reviewed` in DE/US | historical presentation term is long expired; no statement about modern facsimiles |
| exact DTA annotated TEI | `licensed`, CC BY-SA 4.0 | upstream sharing/adaptation route exists with attribution and ShareAlike; text remains unaccepted and local ToS bytes remain non-transferable |
| metadata embedded in the exact TEI | `licensed`, with factual-data boundary | public-safe facts may be cited; substantial database reuse and non-DTA authority claims still need their own review |
| separate DTA plain text | outside the current Item | promising unrestricted provider route, but it requires a new acquisition, fixity, provenance, quality, and rights record |
| DTA/holding-library facsimiles | outside the current Item | no image bytes were acquired; text licensing does not authorize image publication |

## Server consequence

The four existing server plans now record `open-licensed` rights evidence but
remain `metadata-only`, `blocked-rights`, and operator-unapproved. This is not
a contradiction:

- the upstream license is a rights fact;
- source quality and German correctness are separate;
- the local-payload nonpublication rule is stricter than the upstream license;
- human rights review and operator transfer approval are still absent.

No OCR, transcription, page image, snippet, embedding, alignment, translation,
or source-bearing annotation may be served from these local payloads. A later
public candidate must be independently reacquired from DTA, be fixed to a new
manifest and response revision, preserve attribution and ShareAlike
conditions, exclude the separately governed facsimiles, and pass the future
server protocol.

## What this does not prove

This assessment does not prove:

- German orthographic or grammatical correctness;
- exact agreement with the printed pages;
- a critical or author-final Nietzsche reading;
- equivalence among the four parts, the Naumann 1893 witness, or eKGWB;
- translation quality, etymology, semantics, signs, graph truth, or canon;
- that a current web response will remain unchanged;
- publication authority for the operator's local files.

The useful result is narrower and durable: ToS can distinguish a genuinely
open upstream structured-text route from its own intentionally local research
custody without flattening either side.
