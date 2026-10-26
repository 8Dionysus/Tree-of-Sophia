# Public research corpus

`selection.v1.json` selects the public source packets, versions, rights records,
text layers and proposed reading journeys delivered by the two public profiles.
It does not admit a source, translation, interpretation or canon claim. The
new curated records live with `ToS/source-witnesses/`; journey questions live
with `ToS/research-packets/public-prototypes-20261026/`.

The initial selection contains complete German and English *Zarathustra*,
Greek and English *Meno*, Rackham's English *Nicomachean Ethics*, and Fowler's
English *Apology*. Gutenberg's electronic editions and the pinned Perseus
repository revision are identified in `acquisition-index.v1.json` and the
individual source metadata snapshots. These are the selected electronic
representations, without a claim to critical-edition or transcription accuracy.
English and Greek sections can be compared by their supplied reference labels;
this is navigation between independently identified versions, not word alignment
or an assessment of translation equivalence. Editorial notes retained by the
TEI extraction remain visible in the reading text.

## Rebuild

The maintained native owner is `tos-access public-corpus-build`. With a binary
built from the selected checkout, choose a fresh absolute output directory:

```sh
tos-access public-corpus-build \
  --source-root /absolute/Tree-of-Sophia \
  --selection access/public-corpus/selection.v1.json \
  --output /absolute/fresh-public-corpus \
  --max-input-bytes 67108864 --max-output-bytes 16777216 --max-seconds 120
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

DWDS and Logeion links open the external dictionary for the exact selected
form. They are navigation to an identified reference service; no dictionary
entry or license to redistribute its contents is implied. The bounded review
and known quality limitations are recorded in
[`MORPHOLOGY-REVIEW.md`](../../ToS/research-packets/public-prototypes-20261026/MORPHOLOGY-REVIEW.md).
