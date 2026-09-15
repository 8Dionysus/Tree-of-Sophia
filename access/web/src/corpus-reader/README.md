# Corpus reading in the real UI

This module prepares reading whole works independently of the delivery backend.
It is mounted by the Observatory and by the constructor's explicit `?live=1`
entry. The existing node/relation reader remains the contextual inspection view.
The constructor demo, its supplied fragments and its localStorage namespaces
are unchanged.

The assembled research entry is `/static/research.html`
(`research.mjs` → `mountLiveResearch`). The corpus fixture page remains a
development-only harness and is never the production provider.

`mountCorpusEntry` accepts a corpus provider and notebook. An absent provider
is an explicit unavailable state. It never substitutes demonstration text for
a missing source response, guesses an HTTP endpoint, or writes ToS sources.
The Observatory accepts the provider through `createObservatoryData({corpus})`
or directly as `data.corpus`; another host can call
`mountCorpusEntry({root, provider, graphNavigate})` directly.

## Ownership and identity

The [provider interface](DATA-CONTRACT.md) is a browser adapter seam, not a new
published backend API. Delivery still needs an owner implementation for catalog,
document versions, bounded text windows, exact resolution and version search.

Source meaning, identity and admission remain with
[`source-text-unit-packet-v1`](../../../../ToS/contracts/source-text-unit-packet-v1.schema.json),
[`source-anchor-v2`](../../../../ToS/contracts/source-anchor-v2.schema.json), and
[`translation-alignment-packet-v1`](../../../../ToS/contracts/translation-alignment-packet-v1.schema.json).
The browser stores weaker references to the work, expression, edition, item,
file, text layer and unit; text layer and file digests distinguish representation
states. Unicode code-point selectors locate personal selections within a unit.
The reference does not derive identity from a title or paragraph ordinal.

URL fragments carry document/version/unit/revision locators only. Notes,
quotations and source text stay out of URLs. The provider must resolve the
requested version exactly. A changed or unavailable target is not permission
to reattach the old note to different text.

## Reading and navigation

The reading mode supports a paged library, explicit versions, one or two text
panes, local text selection, notebook and appearance controls. Text is inserted
as text, including poetry and non-Latin scripts. The active source language is
independent of interface language. Showing two supplied versions makes no
alignment claim. Automatic parallel scrolling awaits source-supplied alignment.

Each pane loads at most forty text units. Moving between windows replaces the
rendered window, preserving ordinary browser selection and copy in the loaded
text. The provider bounds responses; a huge corpus is not a huge DOM tree.
Search identifies whether it covers the loaded text or the selected version.
Corpus search is available only when the provider advertises it.

Counts are evidence supplied by the provider. Missing or `null` totals and
unit counts remain unknown in the library, manifest and reader; they are never
rendered as zero. Metadata-only, restricted, expired and unavailable versions
remain visible with their status so a reader does not silently retarget to a
different text. An empty intermediate catalog, search or structure page may
carry a cursor; the user can request that next bounded page explicitly, while
the reader avoids automatic full scans. A failed next-window request leaves
the last readable page in place and exposes the delivery error beside it.
Text and metadata are bounded in UTF-8 bytes as well as characters (see
[`DATA-CONTRACT.md`](DATA-CONTRACT.md)); two-pane reading retains at most two
version windows and per-version positions independently.

Only an explicit `unit.context.graphTarget` can offer a transition to the graph.
The host remains mounted through reading so opening or closing the reader does
not recreate its canvas, camera, lens or scene. Reading resumes independently
of graph inspection. The constructor's scope disclosure distinguishes loaded
source records, visible objects and continuation; it never reports a local
scene count as the size of the whole corpus.

## Personal work

The [notebook](STORAGE.md) uses record-oriented IndexedDB with indexed note
pages. Text windows are disposable page state and are not notebook records.
Storage availability and failures must remain visible. The explicit memory
adapter is useful for tests and unavailable-storage sessions, not a claim of
persistence.

Imports merge non-conflicting records atomically, preserve existing local work
and reject conflicting note IDs. Existing demo or research-workspace exports
are not automatically migrated: they lack the exact corpus source addresses
needed to make their anchors equivalent. Their original packets remain usable
through their existing importers. Existing graph-session persistence remains
in its original namespace; this reader does not reinterpret graph notes as
text annotations. Server synchronization is a separate future
adapter and conflict contract.

## Verification route

Run the focused `src/corpus-reader` Vitest tests and the web typecheck. The
browser route `/static/fixtures/corpus-reader.html` mounts the same reader and
constructor sky with explicitly artificial data. It provides:

- a 10,000-work lazy catalog and a 100,000-unit first work;
- original/translation versions and Greek/Arabic scripts;
- network delay, unavailable delivery and revision changes;
- a real IndexedDB smoke test, including reopen, concurrent revision conflict,
  note pagination, separate reading slots, additive import and storage warnings;
- a 100-transition test which records rendered and retained text bounds.

The browser smoke uses only a unique test database and cleans it up after the
check. The interactive fixture notebook uses its own database. Synthetic data
and client-side fixture search are not measurements of a production backend.
The fixture is a development route; the default production entry does not
import its corpus or run its checks.

The first acceptance scenario is: find a work, choose an edition, read a deep
address, compare two supplied versions, select a passage and add a note, visit
its declared graph context, return to reading and reload. Check narrow-screen
access, independent positions, stale references and errors along this route.

## Remaining owner bindings

This preparation does not establish a deployed text service, corpus-wide search,
accepted translation alignment, remote notebook synchronization or complete
scan/OCR presentation. A source transport must preserve rights and visibility,
stable segmentation identity and exact representation state before exposing
real text. The optional `structure` operation supplies a paged table of contents with
exact section targets. The reader displays at most 24 entries at a time and
never infers source hierarchy from paragraph numbers. Real delivery still
needs to bind this operation to the owning text structure.

Native multispan whole-book delivery has a separate owner contract. Its four
operations are specified in
[`NATIVE-MULTISPAN-REQUIREMENTS.md`](NATIVE-MULTISPAN-REQUIREMENTS.md); this
frontend preparation does not implement that owner or flatten its packet
spans into `unit.text`.
