# Source and rights preparation review

Assessor: `model:codex`; scope: thirteen exact versions for local acquisition
and later source planting. This records model source/rights assessment; no
human review, publication decision, textual acceptance, or ancient authorship
is claimed. The target identity and version remain provisional.

The original XLSX rows are research leads. Their assertions about rights or
coverage are not permissions. `manifest.json` binds each lead separately to an
immutable upstream commit and exact file list. The normalized source-registry
snapshot must be bound before acquisition. The metadata evidence directory
retains the fetched bodies, SHA-256, UTC times, URLs, and measured transport
duration. Corpus XML/JSON bodies are absent during preparation.

## MorphHB: Proverbs, Job, Ecclesiastes

The selected version is the Westminster Leningrad Codex digital text with
Open Scriptures Hebrew Bible lemma and morphology annotation in commit
`3d15126fb1ef74867fc1434be1942e837932691f`. The three exact paths are
`wlc/Prov.xml`, `wlc/Job.xml`, and `wlc/Eccl.xml`. Git tree metadata records
their byte sizes and blob SHA-1. SHA-256 and source structure require the bytes
and will be checked during acquisition.

The [pinned license](https://github.com/openscriptures/morphhb/blob/3d15126fb1ef74867fc1434be1942e837932691f/LICENSE.md)
separates the public-domain WLC basis from OSHB's CC BY 4.0 contribution.
The license allows reproduction and adaptation, including commercial use,
subject to attribution and the other
[CC BY 4.0 conditions](https://creativecommons.org/licenses/by/4.0/legalcode.en).
The exact project attribution is retained in the rights templates and the
upstream license snapshot. Modern annotation responsibility remains OSHB's;
it does not determine the ancient composition's author.

The [pinned README](https://github.com/openscriptures/morphhb/blob/3d15126fb1ef74867fc1434be1942e837932691f/README.md)
identifies the OSIS format and warns against Unicode NFC normalization.
Acquisition therefore preserves the original bytes and character ordering.
Chapter controls are 31, 42, and 12 respectively. The expected book must match
every chapter/verse address, and the text must be nonempty. Those checks prove
mechanical coverage within this supplied version, not philological accuracy.

A12's original branch anchor names Sefaria. MorphHB is an explicit alternative
edition for the same named works, not a Sefaria mirror or an equivalent text.

## ORAEC: two sections of EA 10684

The exact raw-data commit is
`b83a0ee5fae27a40d4c0a2a9a8c9c2973d45e9cd`; paths are `oraec1016.json` and
`oraec3091.json`. The official [1016 metadata](https://oraec.github.io/corpus/oraec1016.html)
and [3091 metadata](https://oraec.github.io/corpus/oraec3091.html) identify
respectively verso 2.5–3.11 and 3.11–4.6 of pChester Beatty IV / British Museum
EA 10684. They name Peter Dils as responsible and return to AED-TEI and the
January 2018 scholarly database export. Gardiner 1935 is reported bibliography,
not the asserted date of this digital edition.

The [repository README](https://github.com/oraec/corpus_raw_data/blob/b83a0ee5fae27a40d4c0a2a9a8c9c2973d45e9cd/README.md)
expressly covers `oraec1.json .. oraec13026.json` under CC BY-SA 4.0. Its complete
corpus contributor list is preserved. The exact metadata pages repeat this
license. Reproduction, local processing and adaptation are supported with
attribution and, when adapted licensed material is shared, the
[CC BY-SA 4.0 conditions](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en).
No photograph or separate museum license is imported into that conclusion.

ORAEC's [description of the corpus](https://oraec.github.io/2022/10/23/hooray-our-text-corpus-is-online.html)
distinguishes written form, hieroglyphic representation, German translation,
and grammatical/lexical annotation. Each unmodified JSON is therefore a mixed
scholarly bundle in one Edition and Item. Its Egyptian source-language
component and German translation/gloss component have different Expressions.
The acquisition report must identify observed JSON fields and coordinates for
these components before installing either source record. It must not call the
entire JSON an Egyptian-language expression, or present a German gloss as an
ancient reading.

The two composition identities are provisional and bounded to these supplied
sections. They are not two physical papyri, the complete papyrus, the whole
TLA corpus, or complete critical reconstructions. Existing textual gaps and
editorial signs remain in the original bytes.

## Pāli: eight root versions

The selected Bilara `published` snapshot is
`d6d54741b7f2ddfeca82f02c3f95eb3990b4e351`, limited to `root/pli/ms/`. The
prepared list covers Khuddakapāṭha (9 files), Dhammapada (26), Suttanipāta (73),
Udāna (80), Itivuttaka (112), DN 2, MN 56 and MN 9 (one file each).
Translations, comments, variants, references and HTML are outside this list.

The [pinned official root-edition metadata](https://github.com/suttacentral/sc-data/blob/36c4fddac3ef3c0cda0ffc07760a04d39b8c2eae/misc/root_edition.json)
identifies `ms` as the Mahāsaṅgīti edition, Dhamma Society Fund, Bangkok,
Roman script, 2010. It reports preservation and transmission of the XML by
Ven. Yuttadhammo, followed by SuttaCentral's changes to structure/markup and
occasionally punctuation. This is reported digital lineage, not an independent
proof of textual equivalence with every printed edition or an ancient date.

The [pinned official licensing statement](https://github.com/suttacentral/suttacentral/blob/5c7470b58bcc0bb50887d3a6ed54f4687703f176/client/localization/elements/licensing_en.json)
explicitly places Buddhist original-language texts in the public domain and
SuttaCentral's own contributions under CC0. These two layers are recorded
separately, using the [CC0 instrument](https://creativecommons.org/publicdomain/zero/1.0/legalcode.en).
Bilara's translation license is retained as neighboring evidence but is not
used to infer the root text's rights. Source credit is retained even where CC0
does not require it.

These positive provider statements and international license/waiver terms
support the intended exact-file local acquisition in Mexico. The assessment
does not claim an independent term calculation for all jurisdictions. The
rights records retain affirmative reuse permissions; the present operation's
payload visibility is local-only and does not publish the files.

The MN 9 registry row describes Sujato's translation and separately points to
the cognate root. The selected Pāli root is an explicit separate version and
an antecedent for the later commentarial branch. It is not a medieval
commentary. DN 2/MN 56 supply the canonical side of the branch's parallel-text
need; their Āgama parallels are not acquired. The five smaller collections do
not establish possession of the whole Sutta-piṭaka.

## Acquisition controls

The script must require the completed preparation receipt, freeze the exact
normalized registry snapshot and package digest, reject duplicate or unsafe
paths, and refuse replacement of existing source records or mismatching
payloads. It must compare size and Git blob SHA-1 before assigning File
SHA-256, parse XML/JSON without normalizing source characters, reject duplicate
JSON keys and nonstandard numeric values, and record coverage and limitations.
Each transfer needs actual timing and outcome, and interrupted transfers must
remain visible. Discovery and acquisition provenance use the existing owner
schemas. Bibliographic topology closes through the existing claim routes;
resource inventories, human forms and catalogs use their owning builders.

The first concrete canary is Proverbs. A successful canary must include the
real ignored local file, its source records and manifests, forensic opening,
rights/provenance closure and the affected validators before the remaining
twelve targets are materialized.
