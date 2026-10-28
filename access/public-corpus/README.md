# Public research corpus

`selection.v1.json` selects the public source packets, versions, rights records,
text layers and proposed reading journeys delivered by the two public profiles.
It does not admit a source, translation, interpretation or canon claim. The
new curated records live with `ToS/source-witnesses/`; journey questions live
with `ToS/research-packets/public-prototypes-20261026/`.

Both profiles select the same 50 works: German and English *Zarathustra* and
all 49 books in the retained Antiquity collection, in 96 retained versions.
The six previously published reading layers remain available with their
original IDs and revisions. Four of those layers describe editions also in
the retained collection; they are distinct reading representations, not four
additional historical editions. The complete selection has 102 reading layers.
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
whole and build the public Worker assets. The exporter does not traverse the
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
