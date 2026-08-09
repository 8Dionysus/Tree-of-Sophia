# Etymology Evidence Route Research

Status: ordered research complete; route selected; zero dictionary article or
source payload admitted; zero etymologies accepted

Research snapshot: 2026-08-08

Protocol-native receipt:
`ToS/source-witnesses/discovery/runs/zarathustra-etymology-evidence-route.2026-08-08.v1.json`

## Question

What is the smallest defensible route from an exact German occurrence in an
accepted *Zarathustra* witness to a sourced, contestable etymology finding that
can inform translation without being promoted into meaning, authorial intent,
sign, concept, or philosophical truth?

The answer is not one dictionary and not an LLM prompt. It is a layered
evidence route:

```text
accepted source region and exact surface form
  -> explicit lemma or homograph proposal
    -> exact historical dictionary article
      -> exact etymological article or historical page
        -> competing or corroborating source
          -> dated etymology finding with uncertainty
            -> translation consequence proposal
              -> separate review or unresolved state
```

Every arrow after the source occurrence remains interpretive. Stable source
identity and addressability make correction possible; they do not make the
interpretation immutable.

## Research order

The investigation followed the foundation order:

1. classical standards and current official documentation;
2. established and benchmark-setting research;
3. fresh 2025-2026 primary research and current service, version, and rights
   surfaces;
4. general web search last, as a lead-only negative control.

No dictionary article body, standard text, scan, OCR corpus, or model output
was copied into the repository during this work.

## I. Classical standards and official documentation

### TEI P5

The current official [TEI dictionary chapter](https://tei-c.org/release/doc/tei-p5-doc/en/html/DI.html)
and [`entry` reference](https://tei-c.org/release/doc/tei-p5-doc/en/html/ref-entry.html)
are version 4.12.0, updated 2026-07-28 at revision `113e933e2`. TEI supplies
structured dictionary entries and an `etym` element, but its documentation
states that etymologies vary so much that tags cannot practicably capture their
entire intellectual structure or every precise relation among mentioned words.

Consequence for ToS: retain source prose, exact citations, explicit claim
boundaries, and uncertainty. Do not force every etymology into one apparently
complete genealogy graph merely because the serialization can express a
relation.

### ISO LMF

[ISO 24613-1:2024](https://www.iso.org/standard/82014.html) is the current LMF
core, while [ISO 24613-3:2021](https://www.iso.org/standard/75410.html) is the
published Etymological extension, which also supports diachronic information.
The official lifecycle surface says
the Part 3 systematic review began 2026-01-15 and closed 2026-06-05, while the
page still identifies the 2021 publication as current. Its post-review outcome
must therefore be checked again before implementing an external compatibility
profile.

ISO's current page states that its publications and materials are protected
and that any use, including reproduction, requires written permission. ToS
records the standard identifiers and public metadata only. It neither ingests
the standard nor copies its model into an owner contract. ISO/LMF is
compatibility orientation, not ToS source truth.

### DWDS and Pfeifer EtymWB

The official [EtymWB resource page](https://www.dwds.de/d/wb-etymwb) states
that the digital resource is based on the second 1993 print edition, was
updated by Wolfgang Pfeifer until 2020, contains more than 23,000 lexemes with
over 9,000 main entries and over 14,000 subentries, and is represented in TEI
P5-conformant form. The official BBAW/CLARIN
[object record](https://clarin.bbaw.de/en/objects/dwds%3A3/) independently
identifies the 1993 basis and reports free article-level web access.

An exact article route is addressable by headword, for example
`https://www.dwds.de/wb/etymwb/Wort`; the modern control remains a distinct
route such as `https://www.dwds.de/wb/Wort`. The official
[citation guidance](https://www.dwds.de/d/zitieren) requires the headword,
Pfeifer resource identity, exact URL, and access date.

Current [DWDS terms](https://www.dwds.de/d/nutzungsbedingungen) permit normal
individual web consultation, but automated querying, crawling, parsing,
text/data mining, and other content use require express permission unless a
documented statutory exception applies. Therefore:

- EtymWB is a point-consultation and exact-citation reference baseline;
- no agent may treat it as an unrestricted bulk corpus or autonomous backend;
- no article body is archived by default;
- any automated use must first record the exact permission or legal basis;
- a citation is evidence for a bounded claim, not permission to ingest or
  publish the resource.

### 1DWB through Wörterbuchnetz

The current [Wörterbuchnetz](https://woerterbuchnetz.de/) interface exposes a
machine-readable dictionary inventory and local 1DWB lemma results. Its exact
lemma list route can resolve the DWB identifier before a human or bounded
algorithm selects the correct grammatical homograph. For `Wort`, the observed
neuter result is DWB lemma ID `W26937`, and the stable article route is
`https://woerterbuchnetz.de/DWB?lemid=W26937`.

Two negative controls matter:

- the convenience `entrypoint/lemma/Wort` route returned the fuzzy first hit
  `-wort` (`V04845`), so it cannot be used as exact identity resolution;
- `EtymWb` is configured by the official client as a remote DWDS resource and
  its Wörterbuchnetz API route failed; the correct EtymWB route is the exact
  DWDS headword URL, not a fabricated local API contract.

No public OpenAPI contract or reusable article-data license was found for the
live Wörterbuchnetz API. It is therefore a discovery aid, not a durable ToS
dependency or bulk-content permission. A DWB sense also remains historical
lexicographic evidence, not Nietzsche's intended sense.

### Open historical control: Kluge 1889

Wikimedia Commons exposes Friedrich Kluge's public-domain
[*Etymologisches Wörterbuch der deutschen Sprache*, fourth improved edition](https://commons.wikimedia.org/wiki/File:Etymologisches_W%C3%B6rterbuch_der_deutschen_Sprache_4._Auflage.djvu).
The source-visible title page identifies Friedrich Kluge, the fourth improved
edition, Karl J. Trübner, Strasbourg, and 1889. The current Commons metadata
reports 535 pages, 28,671,111 bytes, MIME `image/vnd.djvu`, SHA-1
`c4c9722f1d7842fc1e601032eed18d16ec5e0b81`, and a public-domain declaration.

This candidate is valuable because it is both historically close to the
first *Zarathustra* publications and potentially usable for later local,
source-visible OCR experiments. It is not modern etymological truth. Its
nineteenth-century scholarship, language, omissions, and possible errors must
be preserved as dated evidence and triangulated with Pfeifer, DWB/DTA, and
the exact Nietzsche occurrence.

The Commons description links Internet Archive identifier
`etymologischesw08kluggoog`, but the current Internet Archive metadata for
that identifier describes a different 2002 Kluge/Seebold manifestation. ToS
retains this mismatch as a provenance warning and does not use the contaminated
Internet Archive metadata to identify the Commons file.

## II. Established research

### Etymology modeling

Khan et al.,
[“Modelling Etymology in LMF/TEI: The Grande Dicionário Houaiss da Língua
Portuguesa Dictionary as a Use Case”](https://aclanthology.org/2020.lrec-1.388/),
connect the LMF diachrony/etymology extension with TEI serialization. This is
the strongest discovered established bridge for later interoperability. It
does not remove the need to preserve source-native wording, uncertainty, or
conflicting reconstructions.

### Diachronic representations and evaluation

Kutuzov et al.'s
[survey of diachronic word embeddings](https://aclanthology.org/C18-1117/)
shows that distributional change methods can expose temporal shifts while
terminology, practices, and evaluation remain uneven. The
[SemEval-2020 lexical semantic change task](https://aclanthology.org/2020.semeval-1.1/)
provides manually annotated German, English, Latin, and Swedish benchmarks and
makes evaluation central.

Consequence for ToS: embeddings, similarity, and induced change points can
propose where to inspect. They cannot author an etymology, establish a sense,
or convert recurrence into philosophical importance.

## III. Fresh 2025-2026 primary research

Hagen's 2025
[historical German DURel/LLM study](https://aclanthology.org/2025.latechclfl-1.16/)
finds that retrieval augmentation is not a universal accuracy improvement:
it helped some smaller models only marginally, harmed larger models in that
setup, and behaved differently across parts of speech. Any ToS RAG challenger
must therefore be tested as a fallible proposal route and stratified by POS.

Fittschen et al. 2026 show that
[models efficiently pretrained from scratch on modest historical corpora](https://aclanthology.org/2026.findings-eacl.241/)
can respect time divisions better and avoid some anachronistic contamination.
This is a future machine-fit candidate, not present etymology authority.

Tat et al. 2026 provide two interpretable lexical-semantic-change routes:
[dependency profiles](https://aclanthology.org/2026.lchange-1.8/) and
[frame semantics](https://aclanthology.org/2026.starsem-conference.5/).
They belong in later A/B/C semantic-change experiments because they can expose
observable contextual differences. Neither method belongs inside the source
etymology layer or establishes authorial intent.

## Selected A/B/C route

### A — open historical page witness

Use the Kluge 1889 Commons object only after a separate exact-object
acquisition receipt and SHA-256 fixity record. A future local trial may compare
page-image inspection, existing OCR if separately licensed, and bounded local
OCR. Output must return to a visible page region and retain the historical
date of the claim.

Current state: selected candidate, not downloaded, not OCR-tested, not content
admitted, and not accepted as an etymological authority.

### B — current scholarly point citation

Consult Pfeifer EtymWB manually or through a permission-backed route. Store
only ToS-authored findings, the exact headword URL, access date, resource
identity, locator, bounded quotation if needed and lawful, and the finding's
epistemic status. Do not store an article body by default.

Current state: selected citation-only candidate; automation blocked absent an
express permission or documented legal basis.

### C — historical sense and occurrence triangulation

Use 1DWB for the historical article and DTA/eKGWB for exact period occurrence
and Nietzsche return. The undocumented DWB API may discover a lemma ID but
must not become the sole address or identity authority. The modern DWDS
dictionary is a separate anachronism control.

Current state: selected discovery and citation route; no etymology follows
from a DWB/DTA frequency or sense alone.

### Computational challengers

RAG, diachronic embeddings, historical language models, dependency profiles,
frame-semantic profiles, and LLM-generated definitions remain proposal-only
methods. They may be measured for quality, cost, speed, source return, and
correction burden after the lexical evidence packet exists. A model answer
without a resolvable source locator is rejected, not “low confidence.”

## Per-finding admission minimum

The existing `tos_translation_pre_draft_analysis_v1` contract already supports
the required route. No schema change is justified before a real finding
exposes missing pressure. For each etymology finding, require:

1. exact accepted Nietzsche occurrence and source-region anchor;
2. recorded surface form, proposed lemma, grammatical category, and homograph
   decision;
3. exact source identity: work/resource, edition or version, headword or page,
   stable URL, and access date;
4. a direct citation with locator and a rights posture for any retained text;
5. claim scope separated into attested form, proposed derivation, historical
   sense, modern control, and possible translation consequence;
6. epistemic status and source-native uncertainty;
7. a competing or corroborating source, or an explicit unresolved state;
8. any LLM/software role named as proposal or transformation, never evidence;
9. no transition from etymology to authorial intent, sign, concept, or graph
   relation without a separately evidenced claim and review path.

No etymology is accepted from one source. “Original meaning” is not a
privileged interpretation of a later passage: ToS explicitly rejects the
etymological fallacy.

## Rights and publication boundary

- Public web visibility is not ingestion or redistribution permission.
- Private local research may use separately admitted local files while their
  bytes remain ignored and unpublished.
- Public-domain metadata about Kluge 1889 opens an acquisition candidate; the
  exact future object still needs fixity and item-level review.
- DWDS/Pfeifer supports ordinary point consultation and citation; bulk agent
  access is closed by default under the current terms.
- ISO metadata can orient compatibility, but protected standard content is not
  copied or supplied to an AI system.
- Only permitted derived artifacts may later move to a site. Private sources,
  article bodies, and local payload paths do not.

## Decision

The foundation now has a usable etymology route without pretending to have an
etymology corpus. The next laboratory action is not mass extraction. It is one
small, predeclared lexical packet tied to an accepted German source region,
run through A/B/C with manual source-return checks and retained disagreement.

This research does not select that word, create the packet, schedule human
work, admit content, or promote any semantic claim.
