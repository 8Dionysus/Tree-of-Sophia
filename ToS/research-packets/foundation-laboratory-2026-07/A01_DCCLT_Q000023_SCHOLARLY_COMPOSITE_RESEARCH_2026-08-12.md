# A01: DCCLT Q000023 Scholarly Composite Research

Date: 2026-08-12
Authoring posture: model-authored, review-ready, no human or philological
judgment claimed
Scope: exact scholarly-composite identity, member relation, provenance,
rights, and acquisition-method preparation; no source text copied into Git

## Result

The A01 backlog row `DCCLT link from P000015 [24]` resolves to a modern
documentary composite, `Q000023`, *Archaic Vessels and Garments*. CDLI
represents the same composite as record `P471693` and currently reports 98
witnesses. Its exact API relation `31` identifies the already planted physical
artifact `P000015` as witness 91 and the artifact ATF repeatedly maps its
lines to `Q000023` editorial coordinates.

This object is neither the physical tablet nor an ancient complete original.
It is a scholarly reconstruction and comparison surface assembled from
surviving exemplars. It therefore receives a separate
`tos.composite.*` identity under `source-witnesses/scholarly-composites/`, not
an artifact identity and not a forced Work/Expression/Edition/Item identity.

The planting fulfills exactly backlog line 12. It creates no accepted source
text, sign reading, translation, semantics, philosophy judgment, graph fact,
or canon state.

## 1. Official and classical documentation

### Exact current DCCLT and CDLI records

The exact [DCCLT Q000023 composite](https://oracc.museum.upenn.edu/dcclt/Q000023)
labels the object *Archaic Vessels and Garments*, `ATU 3, 123-134`, Archaic,
Lexical, and Thematic Word Lists. Its line table is a modern composite
representation with shared editorial coordinates, not a tablet surface.

The exact [DCCLT P000015 member page](https://oracc.museum.upenn.edu/dcclt/P000015)
places `W 12256,i + k + l + o` under *Archaic Vessels and Garments* and exposes
the individual witness's surface and column structure. That page does not by
itself state the complete membership set.

The current [CDLI P471693 JSON record](https://cdli.earth/artifacts/471693/json)
closes the relation more strongly:

- the composite identity is `Q000023` / `P471693`;
- its witness array contains 98 current members;
- artifact `15`, `P000015`, occurs as witness 91 through relation `31`;
- its member designation is `CDLI Lexical 000023, ex. 091`;
- the composite carries its own modern ATF inscription, distinct from every
  member inscription.

The reciprocal [CDLI P000015 JSON record](https://cdli.earth/artifacts/15/json)
contains the same composite relation and explicit `>>Q000023` line mappings.
This is sufficient for a source-returnable member observation without copying
the ATF body into the tracked repository.

### What a composite is

ORACC's [ATF composite conventions](https://oracc.museum.upenn.edu/ns/xtf/1.0/composite.html)
state that Q-identifiers belong to composites and that composites are
organized around documentary structure rather than physical-object
structure. ORACC's [score documentation](https://oracc.museum.upenn.edu/doc/help/editinginatf/scores/index.html)
likewise distinguishes reconstructed composite lines from individual
exemplar lines.

The current [CDLI lexical-composites page](https://cdli.earth/artifacts/composites/Lexical)
describes composites as scholarly re-created complete versions based on
surviving witnesses. For `Q000023` it currently displays 98 witnesses and 97
tagged witnesses. A complete-looking editorial sequence is therefore an
explicit modern scholarly object, not a recovered pristine original.

### Classical publication route

The exact artifact and composite records return to Englund and Nissen,
*Die lexikalischen Listen der archaischen Texte aus Uruk*,
[ATU 3](https://cdli.earth/publications/1785922), 1993. P000015 is cited at
plate 66, while the composite identifies the relevant treatment at pages
123-134. The publication lineage anchors the editorial tradition; no ATU
pages or plates were acquired in this slice, and their publication rights are
not inferred from DCCLT's later license.

## 2. Established top scholarship

[Veldhuis, *History of the Cuneiform Lexical Tradition*](https://researchportal.helsinki.fi/en/publications/history-of-the-cuneiform-lexical-tradition/),
2014, is the established large-scale account of lexical lists as a long-lived
scribal and scholarly tradition. It supports treating a lexical composition
as an intellectual and transmission object whose history is not reducible to
one surviving tablet.

Veldhuis's 2018
[DCCLT project report](https://escholarship.org/content/qt65j403m1/qt65j403m1.pdf)
is the direct digital-method control. It describes DCCLT's open ORACC data,
downloadable JSON, and the linking of composites with individual exemplars.
It also warns against treating ancient lexical translations as automatically
correct. For ToS, composite membership and line coordinates may therefore be
stable evidence while readings, glosses, translations, and semantic identity
remain separately reviewable claims.

These sources establish the durable modeling rule:

```text
physical witness
  != witness transliteration
  != documentary composite
  != composite line coordinate
  != sign reading
  != translation
  != semantic interpretation
```

## 3. Fresh and current relevance check

The freshest exact evidence is operational rather than merely publication-
dated:

- the DCCLT public bulk archive was last modified on 26 June 2026 and the
  live DCCLT glossary exposes a `2026-06-26` build version;
- the live 12 August 2026 CDLI composite API exposes 98 members and includes
  P000015;
- the live DCCLT Q-page exposes only 36 source links and does not show
  P000015 in that page list;
- the live DCCLT P-page still classifies P000015 under *Archaic Vessels and
  Garments*.

The 36-versus-98 observation is preserved as a representation-coverage
difference. The DCCLT page does not claim that its visible `Sources` list is
complete, so absence there is not promoted into a negative membership fact.
The full CDLI API relation and reciprocal member ATF are the stronger exact
evidence for the bounded P000015 relation.

The EU's current
[ScriPTS project report](https://cordis.europa.eu/project/id/101063660/reporting)
is the freshest directly relevant methodological control found after the
official and established pass. Its work on hundreds of Syllabary B fragments
uses material, archaeological, palaeographic, spatial, and scribal-community
variation and contributes updated transliterations to DCCLT. It concerns a
later lexical tradition, not Q000023, but demonstrates why composites must
retain variants and communities of transmission rather than collapse all
witnesses into one timeless canonical string.

Fresh 2025-2026 cuneiform OCR, sign-classification, language-model, and neural
translation papers were screened but not selected for this planting. They may
be challengers in a later `abyss-stack` content experiment; none is better
evidence for the identity or P000015 membership of this exact composite than
the live DCCLT/CDLI records.

## Rights and publication boundary

The exact [DCCLT project home](https://oracc.museum.upenn.edu/dcclt/) carries
both a rights metadata statement and footer declaring DCCLT content under
[CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/). ORACC's
[reuse guidance](https://oracc.museum.upenn.edu/doc/help/visitingoracc/reusingoracc/index.html)
requires project-specific attribution and warns that individual images, PDFs,
and downloads may have different third-party rights.

The result is positive but layer-specific:

- DCCLT-authored metadata, scholarly editing, and transliteration have a
  future redistribution and adaptation route with attribution and
  share-alike compliance;
- linked CDLI or holding-institution photographs, line art, publication
  plates, PDFs, and other third-party objects do not inherit that grant;
- this first tracked packet remains metadata-only and contains no composite
  or member text;
- a future public text derivative must freeze the exact source version,
  contributors, attribution, modifications, and share-alike posture before
  publication.

Open licensing is not hidden behind generic caution, but it also does not
substitute for fixity, provenance, source quality, or review.

## Machine-access observations

Current response fingerprints were computed in owner-local temporary storage;
no response body enters Git.

| Surface | Bytes | SHA-256 |
| --- | ---: | --- |
| DCCLT project home and license | 6,966 | `1c1890175cb103ebcb441638e193196dc57df0c43a72ddf3de81222f654e3fe3` |
| DCCLT Q000023 HTML | 68,210 | `89c1d97e859e91da7abbd7a9168c1c7b6ec5c2d6e4258291ac7b4b9a013062b4` |
| DCCLT P000015 HTML | 49,547 | `464e98e8287b30c866f8eb8bad68f9452396becd0e6b3672df462af98facafd7` |
| CDLI P000015 JSON | 8,584 | `1bb9739a321be00ff171a2382058dede3e371cd06f04f9136c57e2baa19d9a67` |
| CDLI P471693 JSON | 166,018 | `d2e9ee1e9334651b827ffbba9c49952cf87a6146149905f3450ea78b912bf4aa` |
| CDLI lexical-composites page | 36,608 | `cbc2b451e1f7d7da783dc5703b7c21aed6c44a3a051c4cac025612a55f90360c` |

The live ORACC server omitted its InCommon intermediate certificate during
this check. Verification was restored without `-k`: the leaf's AIA
intermediate was downloaded, converted, verified against the system trust
store, and supplied as an additional CA chain. No access control or TLS check
was bypassed.

The documented per-object `manifest.json`, `metadata.json`, and
`corpusjson/{Q000023,P000015}.json` endpoints returned HTTP 200 with empty
bodies on both HTTP/2 and HTTP/1.1. The official 77,284,743-byte DCCLT bulk ZIP
remained remote. These are current operational facts, not reasons to weaken
verification.

## A/B/C acquisition laboratory prepared

| Condition | Method | Current state | Quality question | Cost and speed pressure |
| --- | --- | --- | --- | --- |
| A | exact live HTML plus CDLI JSON relation | executed for metadata only | Can identity, membership, license, and visible representation differences be resolved without retaining content? | lowest transfer and storage; seconds; mutable live source |
| B | selective extraction of exact members from the official DCCLT bulk ZIP using HTTP ranges or a bounded archive fetch | prepared, not run | Can exact ATF/JSON bytes be frozen with less transfer while preserving archive-member integrity? | medium implementation complexity; low retained bytes; needs ZIP range/index proof |
| C | full versioned DCCLT ZIP cached under the host storage policy, fixity-bound and queried locally | prepared, not run | Does full-corpus closure improve reproducibility and membership analysis enough to justify transfer and storage? | 77 MB transfer plus cache; fastest repeated local queries; needs host admission |

Condition A proves the identity layer only. B and C, when run, must be compared
by exact returned bytes, coverage, failure cases, elapsed time, storage,
network transfer, and direct manual inspection—not by a green validator alone.
No model or LLM is needed for acquisition identity. Later OCR, sign, lexical,
or semantic challengers route through the `abyss-stack` laboratory.

## Remaining gates

- Composite source bytes: not acquired or admitted.
- P000015 member text: not acquired or admitted.
- Attribution roster and exact content-version publication packet: not built.
- Philological review: not performed or scheduled.
- Sign identity, reading, gloss, translation, etymology, or semantics: not
  performed.
- Physical-member coverage beyond the exact P000015 relation: not imported.
- Graph and canon: unchanged.
- Human task: none.

The durable result is a second foundation layer: a composite can now be
identified, licensed, compared across representations, and linked to one
physical witness without being mistaken for that witness or for ancient
semantic truth.
