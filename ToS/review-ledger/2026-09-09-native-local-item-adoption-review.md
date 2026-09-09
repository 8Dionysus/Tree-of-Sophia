# Native local Item adoption review

Date: 2026-09-09 UTC. Scope: source work after
`983b4eee3a60fa69de52c8bbb00f82eafaf8b4a8`, not Foundation v1 acceptance.
The Operator's Foundation direction and the
[local source boundary](../source-witnesses/LOCAL_STORAGE_BOUNDARY.md) authorize
this bounded acquisition and source growth, not public payload transfer.

## Referent and bounded source observation

One EPUB 3 container was obtained through the Project Gutenberg mirror route
for [catalog item 4363](https://www.gutenberg.org/ebooks/4363), following its
[mirroring guidance](https://www.gutenberg.org/help/mirroring.html).
Only one exact file was transferred. Its retained size is 238,143 bytes and
SHA-256 is
`9c894ce0ab9e1861b8326be82e95657e9d81d3a1ab3720a66b0649d156d019e1`.
Local ZIP integrity checks passed for all 14 members.

The OPF, opening provider header/transcriber note and embedded license were
inspected. They identify Nietzsche, Helen Zimmern and the electronic
`Beyond Good and Evil` manifestation. Four distinct observations remain:

- provider release date: 2003-08-01;
- header's last-update date: 2019-01-09;
- OPF generated-container modification: 2026-09-02T14:33:23Z;
- this local network acquisition: 2026-09-09T06:23:42–06:23:44Z.

None establishes the translation's composition date, exact printed edition,
historical publication date or textual fidelity. The broad Complete Works
1909–1913 reference and stated editorial adaptations do not identify a
particular physical copy. The whole philosophical text was not read or
compared here; three regions of one provider are not three independent sources.
Unsigned mirror transport plus a local digest proves retained-byte fixity,
not upstream cryptographic authenticity.

The [Item](../source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/en-zimmern-gutenberg-4363/editions/project-gutenberg-4363/items/local-epub3-20260909/item.json)
is one provisional local digital container, not the Zimmern translation class
or a printed original. Its acquired manifest uses `derived_publication` and
retains the separate File identity. The
[exemplified_by Claim](../source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/en-zimmern-gutenberg-4363/editions/project-gutenberg-4363/items/local-epub3-20260909/source-claims.jsonl)
observes the declared Edition–Item metadata link, remains unreviewed, and does
not assert equivalence or accept historical truth.

## Rights, privacy and future server boundary

The [rights record](../source-witnesses/works/friedrich-nietzsche/jenseits-von-gut-und-boese/expressions/en-zimmern-gutenberg-4363/editions/project-gutenberg-4363/items/local-epub3-20260909/rights.json)
retains the attributed United States provider statement while leaving
jurisdictions unreviewed, copyright undetermined, payload visibility
`local_only`, redistribution `not_authorized` and derivatives
`local_research_only`. The [provider license](https://www.gutenberg.org/policy/license.html)
is not a new general clearance by this agent.

The [future-server plan](../source-witnesses/server-import/plans/jenseits-gutenberg-4363-local-epub3-20260909.server-import.json)
and exact provenance bind the committed manifest and rights digests.
Server status is `blocked-rights`, access is `metadata-only`, transfer is
false and content-bearing server derivatives are prohibited. Graph/search
conditions cover reviewed public-safe metadata only. No source payload was
uploaded, no server import executed, and no publication or human approval was
fabricated. Takedown fields express future importer obligations, not verified
deployed behavior.

The public package and selected transaction journal contain no absolute
source/input/recovery paths, raw private grant, local command line or private
continuation content. Private evidence was preserved separately; only bounded
identity, fixity, intervals and their digest bindings were retained here.
The `aoa-knowledge-stewardship` sanitized-share route shaped this
source-safe derivative; it supplies neither rights nor publication authority.

## Real execution and history

The discovered `native-item-adoption` handler provides `item.adopt` and
`item.adoption.recover`. An exact protected expiring configuration selects
metadata, source-file and recovery scopes separately. The
[operation contract](../../mechanics/growth-cycle/parts/branch-growth-cycle/docs/NATIVE_ITEM_ADOPTION.md)
describes bounded copying, inventory, publication and recovery.

The actual operation completed in 4.126 seconds, peak 30.1 MiB, zero swap.
Transaction:
`sha256:fd5e00f8798907518f8034815581b09faf4a54dada844c55cd3cf19bc8ba5813`.
Edition v1 advanced to v2 with its new exemplar backlink and three rebound
forms; Item v1 and a distinct Claim v1 were created. The byte receipt records
observation at 07:21:59.653590–07:21:59.683435Z and copy at
07:21:59.684913–07:21:59.751683Z. These are neither the earlier network
acquisition nor the metadata serialization event.

A fresh readback verified the original and canonical local file hashes,
four independent compound receipts, 15 unchanged predecessor/legacy files,
Expression v1/v2/v3, Edition v1/v2, and Item v1 through exact canonical and
original-path/raw-digest references. All current forms retained their source
context. After catalog regeneration the exact request replayed without
changing publication or private continuation. The canary passed in
3.365 seconds, peak 28.5 MiB, zero swap. The actual rollback/recovery failure
paths were exercised on isolated fixtures, not destructively against this Item.

## Engineering review and checks

Independent source-choice and proposal reviews found no blocking identity,
rights-overclaim or privacy issue. The implementation review found and closed
three concrete defects: allocation through unsupported ZIP codecs before
expanded-byte bounds; conflation of observation and copy intervals; and generic
Claim correction bypassing the compound owner for a flat Item package.
Final focused Item/copy coverage passed 18 methods and 16 subtests in
74.53 seconds. Related earlier batches passed 26 transaction methods with
48 subtests, and 70 history/Claim/discovery/copy methods with 217 subtests;
these earlier batches preceded the final interval refinement and are not
represented as a complete rerun of the final tree.

The separate reviewer then inspected the actual acquired package: 18 selected
outputs, 17 receipt bindings, 20 journal blobs, all 14 ZIP member digests and
43 public files. Exact private continuation/configuration bindings and distinct
original/canonical single-link files matched; only the declared Edition delta
changed. The reviewer reread the bounded source regions and server plan, finding
no remaining actionable package, privacy or rights-overclaim defect. This is
their direct read/hash review, not a repeat of the root transport canary or
heavy validators. A documentation qualifier was tightened to distinguish
private copy-inode pins from the existing public metadata-parent guard facts.

The final root topology/inventory batch passed 33 methods and 782 subtests in
1.27 seconds. Semantic registry evolution passed against exact baseline
`983b4eee3a60fa69de52c8bbb00f82eafaf8b4a8`; the relation registry advances to
version 33 without silently changing the legacy Item or transaction profile.

Root source-foundation validation, bibliographic/philosophy graph regeneration
and both graph validators passed in 61.167 seconds, peak 420.4 MiB, zero swap.
The initial closure check correctly found the missing per-Item server plan;
the first drafted provenance then failed for a missing event version.
Both source companions were completed without modifying acquired metadata.

The new Item and its relation were read through core, loopback HTTP and native
MCP tool invocation with five identical cursor pages per transport. Repeated
cursors were stable; relation depth-zero kept both endpoints and the selected
link. Eight available node kinds, including the exact new Item, passed
depth-zero focus; the Item's two Russian source-copy forms retained context.
The graph cold build took 16.755 seconds; the complete unit took 23.297 seconds,
peak 1.2 GiB, zero swap. This is one measured snapshot, not a scaling budget,
external MCP handshake, D1 result or browser acceptance. No exact public
`tos.entity.occurrence` existed in that snapshot; it remains an explicit gap.

## Boundary checklist and remaining owner work

Yes: source return, stable distinct identities, immutable original bytes,
versioned lineage, separate observation/copy/serialization, conservative rights,
bounded authority, private continuation, language/source context, plurality,
and source authority over derived graphs are retained.
Not applicable: canon, counterpart, compost, calibration, lived witness,
gold promotion and philosophical interpretation acceptance.
The review does not grant current semantic admission.

Reproducible focused commands:

```sh
python -m pytest mechanics/growth-cycle/tests/test_source_item_commands.py
python -m pytest mechanics/growth-cycle/tests/test_source_item_deposit.py
python -m pytest mechanics/growth-cycle/tests/test_source_metadata_transactions.py
python scripts/build_source_witness_catalog.py --check
python scripts/validate_source_witness_foundation.py
python scripts/build_source_witness_bibliographic_graph.py --check
python scripts/validate_source_witness_bibliographic_graph.py
```

The concrete source grow/read path is closed locally; unsupported inventory
formats, broader compound corrections, native text-layer construction, complete
human command construction and remaining Foundation profiles stay with their
source/access owners. CI, merge, release and deployment require separately
observed integration evidence. This is not Foundation completion.
