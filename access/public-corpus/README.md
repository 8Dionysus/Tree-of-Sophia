# Public research corpus

`selection.v1.json` selects the public source packets, versions, rights records,
text layers and proposed reading journeys delivered by the two public profiles.
It does not admit a source, translation, interpretation or canon claim. The
new curated records live with `ToS/source-witnesses/`; journey questions live
with `ToS/research-packets/public-prototypes-20261026/`.

Both profiles select the same 51 works: *Zarathustra*, the 49 books in the
retained Antiquity collection, and Xenophon’s *Memorabilia*. The selection
contains 138 reading versions, including 17 captured Russian Karpov translations
of Plato from Wikisource and the complete English Dakyns *Memorabilia*. The
39-chapter reading retains editorial footnotes; the source prints chapter XII
twice in Book III, and the second is numbered 13 for navigation only. The
anonymous 1889 Russian Seneca *De Beneficiis* witness contains only the manually
transcribed Book I chapter 6; its remaining chapters are not supplied by that
witness. Distinct reading representations of the same edition
are not additional historical editions.
Gutenberg's electronic editions and the pinned Perseus
repository revision are identified in `acquisition-index.v1.json` and the
individual source metadata snapshots. These are the selected electronic
representations, without a claim to critical-edition or transcription accuracy.
English and Greek sections can be compared by their supplied reference labels;
this is navigation between independently identified versions, not a general
alignment or assessment of translation equivalence. The selected Meno 70 packet
contains two source-bound model proposals for `διδακτὸν` / “can be
taught” and `ἀρετή` / “virtue”. They describe only those exact spans and remain
proposed after a bounded source-visible agent review for research use; formal
alignment admission remains pending. Editorial notes retained by the TEI extraction remain
visible in the reading text.

## Rebuild

The maintained native owner is `tos-access public-corpus-build`. With a binary
built from the selected checkout, choose a fresh absolute output directory:

```sh
tos-access public-corpus-build \
  --source-root /absolute/Tree-of-Sophia \
  --selection access/public-corpus/selection.v1.json \
  --output /absolute/fresh-public-corpus \
  --max-input-bytes 536870912 --max-output-bytes 268435456 --max-seconds 180
```

Only explicitly public, publication-authorized packets covered by the selected
digest-bound rights records pass. Source work identity, text-layer bytes,
Unicode code-point anchors, ordered units, journey targets and analysis layer
bindings are checked before output. All output is deterministic JSON, with
hash-bound document manifests and pages. Pages contain at most 40 units and
262144 bytes; one unit is capped at 65536 UTF-8 bytes. The exact surface index
does not fold case, spelling or Unicode, and does not infer lemmas. A bounded
page scan remains available when an index exceeds its cap.

Review the fresh output, then replace `access/web/public/public-corpus/` as a
whole and build the public Worker assets. Its pages, indexes, document
manifests and analyses live in R2 and stay out of git; `layers.json` here is
their digest manifest, and the snapshot's own `manifest.json`, `retained.json`,
`revisions/` and `mappings/` stay tracked. After replacing the snapshot:

```sh
node bonsai/lib/layers.mjs manifest access/public-corpus   # the new digests
node bonsai/lib/layers.mjs send access/public-corpus       # new files onto layers/transfer
```

A checkout brings the files with `node bonsai/lib/layers.mjs fetch
access/public-corpus` ([layers kept in R2](../../bonsai/README.md#layers-kept-in-r2-tosbonsailayersv1)). The exporter does not traverse the
private local corpus or discover an installed data root. It refuses symlink
source references and has finite byte/time budgets.

Raw acquisition and the initial proposed segmentation were one-off preparation
steps, recorded separately in each item's provenance. The public snapshot is
native-generated; the metadata inventories themselves do not claim native
source-catalog admission. That protected owner check needs its own issued
invocation and stage ticket.

`prepare_retained_corpus.py` imports the explicitly selected 96 saved reading
JSON files from the separate Antiquity worktree. It retains their exact bytes
as physical source items and records their upstream TEI or historical-page
provenance separately. Missing upstream TEI bytes are not represented as
retained files. Reading text, notes, headings and variant information are
carried into source-bound native units; large sections are split without
discarding text. The companion `retained-library-map.v1.json` contains only
navigation metadata. It connects the existing collection material IDs to
the native work, version and section IDs without renaming saved graph nodes.

Completeness here means all sections of the saved reading representations.
It does not certify transcription quality against scans or critical editions.
Historical Wikisource transclusions retain their recorded proofreading limits.

The Karpov translations identify the 1863–1879 edition, translator, license,
article oldids, per-page revisions and proofreading status. Exact parse API
responses and reading-layer bytes remain in local Item payload custody; the
repository carries their digests and source records. The 16 dialogue captures
and ten separately captured books of *Republic* retain their source renderings;
the latter have a Wikisource template error outside the text body. Captured
pages were not collated against scans, and the alignment is unreviewed. The
preparation scripts extract reading text and margin references without silently
correcting source errors; they do not independently verify historical rights
or semantic equivalence. *Republic* intake also requires the native inventory
owner to account for each retained response.

## Editorial marks in the reader

`editorial-annotations.v1.json` is bounded presentation metadata, generated
from the exact source and immutable reading-layer bytes by
`python3 access/public-corpus/prepare_editorial_annotations.py`. It does not
rebuild or change the public corpus. It records the source digest, version,
layer digest, unit, Unicode-code-point range and exact text for every displayed
mark. The reader applies a mark only when all these bindings still match.

The pinned Rackham TEI maps 696 source `<note>` elements to their own units;
an ambiguous note makes generation fail. The retained Athenian Constitution
HTML maps 56 editorial additions and 26 gap marks; the retained Greek Ethics
HTML maps 80 addition, deletion and gap spans. Three ambiguous Greek Ethics
spans were rejected and are not displayed as editorial marks. The retained
HTML is evidence of its own markup, not proof of an editor's identity, a full
critical apparatus, or a semantic judgment. Text and existing selectors stay
unchanged; quoted selected ranges identify any mapped editorial material.

The provisional source-visible comparison of one phrase in Adams's
*On the Sacred Disease* is separately recorded in
`ToS/research-packets/public-prototypes-20261026/reader-textual-variants.v1.json`.
The reader preserves and attributes the digital reading while showing the
different reading in the linked printed witness. This is not a complete
collation or a replacement text layer.

The software reader consumes the byte-identical derived companion
[`reader-textual-variants.v1.json`](reader-textual-variants.v1.json), together
with [`contained-reading-targets.v1.json`](contained-reading-targets.v1.json).
Their authored owners remain the same-named packets under
`ToS/research-packets/public-prototypes-20261026/`. Refresh only these bounded
metadata projections, then verify their byte parity:

```sh
python3 access/public-corpus/prepare_reader_companions.py --write
python3 access/public-corpus/prepare_reader_companions.py
```

This route does not rebuild texts or change references. Software builds consume
the derived files under Access and do not require the authored research-packet
checkout.

## Morphology and dictionaries

The two `morphology.*.v1.json` files contain retained, unranked tool candidates,
bound to exact text-layer hashes and analysis versions. German uses DWDSmor
Open 0.18.0; the Greek sample uses retained Morpheus responses. The public
reader makes no per-click request to the retired hosted Morpheus service.
Tool/software licensing and generated linguistic judgments remain distinct.

German preparation queried 11384 exact forms, with candidates for 6575 and
4809 explicitly unrecognized forms. Greek preparation queried 78 of 2467
distinct source forms; 77 returned candidates. These are preparation counts,
not accuracy or occurrence-coverage scores. Known historical spellings are
not silently modernized. Candidate lists do not select a contextually correct
lemma; truncation is explicit. Unrecognized, not processed, provider failure
and unsupported language have different results. No candidate is an etymology.

DWDS and Logeion search links use the exact selected form; candidate links use
the supplied lemma where one is known. They are navigation to identified
reference services; no dictionary entry or license to redistribute its contents
is implied. The bounded review
and known quality limitations are recorded in
[`MORPHOLOGY-REVIEW.md`](../../ToS/research-packets/public-prototypes-20261026/MORPHOLOGY-REVIEW.md).

## Bonsai witness delivery

`prepare_bonsai_witnesses.py` projects the retained DTA and Antonovsky units
from `bonsai/zarathustra/witnesses/*.json` without changing their text or IDs.
Its `tos_bonsai_public_text_packet_v1` and
`tos_bonsai_public_rights_projection_v1` are bounded delivery inputs to
`public-corpus-build`, **not** ToS source-text-unit packets or rights-owner
records. This distinction matters because the Antonovsky units retain their
stand-local `ru1913.*` IDs. The builder still checks the selected packet and
rights digests, public conditions, every Unicode-code-point anchor and its
exact text hash. The concatenated text is a delivery layer; it is not an
assertion that the four DTA first editions are one historical edition.
Their original identities, source links, attribution, rights and transcription
limits remain in `bonsai/zarathustra/witnesses.json`. The existing Gutenberg
versions and IDs are retained separately. DWDSmor candidates are reused only
for exactly equal forms, unranked and without a contextual correctness claim.
The bonsai's automatic sentence alignment is displayed as unreviewed and
only after target text agrees exactly with the selected reader version.
