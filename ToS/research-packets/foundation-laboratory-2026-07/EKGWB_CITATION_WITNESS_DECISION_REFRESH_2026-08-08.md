# eKGWB citation-witness decision refresh

Status: model-authored, review-ready research assessment; no human decision,
source admission, accepted German, translation lane, or publication authority

Research snapshot: 2026-08-08

## Decision being prepared

Should Tree of Sophia admit the eKGWB locator `Za-I-Vorrede-1` and its
local-only observed reading as an edition-typed **citation witness** for
private, non-commercial AI-only and AI+human translation experiments?

This is deliberately narrower than accepting the German text as correct. It
does not ask the current operator to judge German orthography, grammar,
semantics, etymology, or translation fidelity. It also does not authorize
source publication, derivative publication, commercial use, or a human-only
translation lane.

## I. Current official and classical owner surfaces

The current [eKGWB documentation](https://doc.nietzschesource.org/en/ekgwb)
identifies the digital edition as the German reference edition based on the
Colli-Montinari print edition, edited digitally by Paolo D'Iorio and published
by Nietzsche Source. It describes word-by-word collation, approximately 6,600
integrated philological corrections, XML-TEI encoding, and stable siglum-based
addresses.

The current [Nietzsche Source rights
page](https://doc.nietzschesource.org/en/rights) declares CC BY-NC-ND 4.0 and
encourages scholarly consultation and exact citation. The license's legal
code permits non-commercial production and reproduction of private adapted
material but prohibits sharing adapted material. The owner page also asserts
a public-domain basis for the Colli-Montinari scientific text. ToS continues
to keep the underlying Nietzsche text, scientific-edition layer, digital
editorial layer, and website presentation separate rather than flattening
them into one rights object.

Fresh direct page opening from the research browser returned HTTP 502 on
2026-08-08. Search-cached owner pages were crawled four to six months earlier.
This refresh therefore confirms the currently indexed official posture but
does not pretend to be a live authenticated rights capture.

## II. Established scholarly and institutional evidence

Paolo D'Iorio's editor account,
[Nietzsche Source: Buscar, verificar,
citar](https://doi.org/10.1590/2316-82422024v4503pd), identifies eKGWB as the
digital critical edition based on the text established by Giorgio Colli and
Mazzino Montinari. The article was published in *Cadernos Nietzsche* 45(3),
2024, with online publication recorded in January 2025.

The current [ITEM Sources infrastructure
page](https://www.item.ens.fr/sources) independently describes eKGWB as the
reference critical-edition pilot, explains its stable addresses, and cites
the 2010 and 2024 editor accounts. The same institutional page explains that
the corrected digital text integrates philological corrections from the
critical apparatus.

The [Catalog of Digital Scholarly
Editions](https://www.digitale-edition.de/e461), version 4.045 with last change
2026-06-08 and freshly indexed during this review, records the edition as
edited by Paolo D'Iorio, published by Nietzsche Source at the École Normale
Supérieure in Paris, beginning in 2009. This is the freshest independent
bibliographic corroboration found. It is metadata evidence, not a passage
payload or rights grant.

## III. Exact object, preservation, and comparison evidence

The ordered discovery record
`ekgwb-za-i-vorrede-1-institutional-corroboration.2026-07-30.v3.json` preserves
the exact current owner URL, an independent Arquivo.pt WARC lineage, and the
bounded negative search for a publisher repository or direct TEI deposit.

The exact `Za-I-Vorrede-1` target block has SHA-256
`f58a13c189bcf22db19fcbca2345e660a771656c89010c67d55eb4a00c4cc398`
in both the owner-hosted object and the 2023 institutional archive replay.
Arquivo.pt supplies capture time, collection, WARC filename, offset, and
archive digest. Its terms explicitly do not validate the archived content or
publisher origin.

The separate machine triangulation compares only local, ignored source
payloads and records text-free results. After source-aware DTA dehyphenation,
the eKGWB candidate and the DTA TEI witness agree across 12 paragraphs and 261
normalized tokens with sequence SHA-256
`6d3deb76b489989f2a8ed3782f3ae9c12914a0ab6baea1b9f21432bcc8749e16`.
The Naumann automatic EPUB retains one OCR replacement plus page furniture.

These facts strongly corroborate the observed reading. They do not prove TLS
authentication, publisher signature, German linguistic correctness, or
philological finality.

## IV. Fresh source-route result

The 2026-08-08 check found no new official Git repository, direct publisher
TEI deposit, Zenodo record, DataCite object, or authenticated exact-passage
route that supersedes the July discovery. The current scholarly-edition
catalog and live ITEM page reinforce bibliographic identity; they do not close
transport authentication.

The existing access request remains useful if ToS later needs a
publisher-authenticated copy, permission for a shared derivative, or direct
institutional clarification. It is still draft-not-sent and requires explicit
human approval before any message is sent.

## Recommendation

Admit with limits.

The evidence is sufficient to treat the exact locator and local-only observed
reading as a citation witness for bounded private, non-commercial AI-only and
AI+human translation experiments, provided the following remain explicit:

- transport is `publisher-host-observed-over-unauthenticated-http`, not
  publisher-authenticated;
- institutional preservation corroborates bytes but does not validate origin;
- the reading is edition-typed evidence, not accepted German;
- the current operator is not asked to make German-language claims;
- source and adapted payloads remain local-only;
- recognized translations remain sealed until each experimental lane freezes;
- no human-only lane, public release, semantic promotion, graph truth, or
  canon promotion is opened by this decision.

Rejecting the admission is also valid. In that case the existing access
request or a different critical edition remains the next source route.

## One human question

Do you admit this eKGWB unit **with exactly the limits above** as a
citation-only witness for private AI-only and AI+human translation
experiments?

The answer is one of: `admit with limits`, `reject`, or `defer`.

## Authority boundary

This packet is a model-authored research synthesis and recommendation. It is
not legal advice, human bibliographic review, human rights review, source
admission, accepted German, translation evidence, semantic evidence, graph
truth, or canon authority.
