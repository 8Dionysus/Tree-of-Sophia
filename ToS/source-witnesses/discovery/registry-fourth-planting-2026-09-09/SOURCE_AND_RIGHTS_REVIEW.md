# Fourth planting: English translations alongside existing Greek versions

Reviewed by Codex for the Operator's next planting request, 2026-09-09 UTC.
This assessment concerns the exact supplied digital versions, their CTS Work
correspondence, translation role, source/rights metadata, local acquisition and
branch fit. It does not accept translation accuracy, ancient attribution,
textual readings, philosophical interpretation or canon.

## Selection and identity

The pinned `PerseusDL/canonical-greekLit` tree at
`341e309c821d5eca8c976bebca77c28b10bad58f` contains 68 English-version files for
59 of the 63 Greek Works acquired in the preceding two passes. Their complete
TEI header prefixes and exact CTS Work/translation declarations were retained
and read before complete source-file acquisition. The tree response is not
truncated. Transport receipts distinguish a bounded prefix read from complete
source custody.

This pass selects one English translation for each of those 59 existing Works:
35 Platonic-corpus works, 14 Plutarch works, seven Aristotelian-corpus works,
the two Epictetus works and Diogenes Laertius. Where alternatives are present,
the selected Plutarch files use the Loeb translations, and Epictetus uses
George Long. Nine other English versions remain metadata-only alternatives,
with their own future Expression/Edition review condition. Four prior Greek
Works have no English file in this exact repository view; this says nothing
about their availability elsewhere.

The English CTS declaration must explicitly name the same Work URN as the
existing ToS Work record and classify this version as a translation. That
supports one additional English Expression under the existing Work identity.
It does not establish that the translator used the particular Greek Edition
retained by ToS, so no exact translation-from or passage-alignment claim is
created. Greek source files and records remain present. Parallel reading
links expose both exact versions without claiming interchangeable wording.

Each existing Work's complete prior record is retained under `work-before/`
with SHA-256. Acquisition may append only the prepared `has_expression` claim
refs and increment the Work record version. Existing labels, responsibility,
source refs, identity and prior claim refs must remain unchanged. A changed
Work preimage stops acquisition rather than overwriting another operation.
New Expression, Edition and Item paths may not already exist.

## Translation and edition responsibility

The 35 Platonic files name Harold North Fowler, W. R. M. Lamb, R. G. Bury and
Paul Shorey, with distinct supplied volumes and printing statements from
1914 through 1935–37. The older headers retain the 1992 St. Olaf scanning
note and the 1996 Perseus publication statement. Contributor lists and
volume-renumbering notes remain literal source evidence. A Work's disputed
Platonic attribution is not settled by the translation or provider grouping.

Plutarch's selected translations include F. C. Babbitt, W. C. Helmbold, and
the Cherniss/Helmbold volume for the lunar-face work. Other selected works use
the Goodwin-edited *Morals*, retaining the separately named translators
including John Philips, R. Brown, E. Smith, Samuel White, William Baxter and
A. G. Supplied print statements include 1874/1878 and Loeb printings through
1957. The descriptions are preserved per version; Goodwin's editorial role
is not substituted for an individually named translator.

Aristotle's English versions name H. Rackham, Hugh Tredennick, G. C. Armstrong,
W. H. Fyfe and J. H. Freese, with the literal source volume and printing
statements retained. The containing printed volumes may collect several
works, while each acquired CTS file covers the named constituent Work. This
intake does not pretend to acquire all works in those printed volumes.
Pseudo-Aristotelian attribution and textual completeness questions remain
open where already recorded.

Epictetus uses George Long's supplied 1890 translation, separately from the
recording/editing responsibility of Arrian. Diogenes Laertius uses R. D.
Hicks's supplied 1925 English translation, covering the entire supplied
ten-book file while the A26 branch need names Books 6–10. Quotations and
embedded letters are not independently admitted by acquiring this container.

The exact per-version header statements are in `metadata-review-summary.json`
and the original retained XML headers. Ancillary Greek, Latin, French, German
or other language declarations describe material within these English
editions; they do not establish additional full translations.

### Economics: conflicting upstream language metadata

The CTS child for `tlg0086.tlg029.perseus-eng2` correctly identifies a
translation under the Economics Work URN, but its `xml:lang` is `grc`. This
conflicts with the named Armstrong English translation and the actual pinned
TEI source: both its `text` and translation `div` explicitly declare `eng`,
and the retained opening is visibly English prose. A bounded 16,384-byte
source opening was read and retained for this review, without acquiring the
complete file. `metadata-language-assessments.json` binds that observation
and the original contradictory CTS metadata. The English Expression follows
the actual source-language declaration and inspected text. The full acquired
file must also match the reviewed opening's digest. The incorrect CTS label
is preserved, not edited or hidden, and this finding does not accept the
translation's textual or philosophical accuracy.

## Registry and branch fit

A25 explicitly lists Greek texts and available English translations at its
Plato/Aristotle Work and corpus routes. A29's Plutarch and Epictetus corpus
leads likewise include English. Diogenes Laertius is grounded by the exact
English Hicks record **A26-R002**, not the Greek-only A26-R001. The original
research assertions remain reported, and each target retains its exact
normalized record and original cell locator.

The source anchors in A25/A26/A29 remain their current author/Work corpus
needs. These additional versions support reading alongside the already
retained Greek versions; they do not replace a source-language requirement
or exhaust a corpus-level lead. The A26-R061 Plutarch title/CTS conflict from
the third pass remains open and is not silently closed here.

## Rights and local scope

The pinned [repository README](https://github.com/PerseusDL/canonical-greekLit/blob/341e309c821d5eca8c976bebca77c28b10bad58f/README.md)
provides a default CC BY-SA 4.0 grant with an exception clause and warns that
header accuracy remains under review. Selected Plutarch, Aristotle, Hicks
and Long headers explicitly state CC BY-SA 4.0. The selected Plato headers
have no conflicting exception; their applicable basis is the repository grant.
The complete source descriptions, publication statements and notes were
checked per version. This assessment relies on that positive licence within
supplier authority, not the antiquity of the Work or presumed expiry of a
translator's copyright.

Retain headers, source attribution, named translator/editor credits and
licence notices. Future modifications must be identified, with applicable
share-alike conditions for shared adaptations; no source bytes are modified
in this operation. The repository's request to offer modifications back to
Perseus remains recorded. See the [CC BY-SA 4.0 legal code](https://creativecommons.org/licenses/by-sa/4.0/legalcode.en).

The intended operation is local preservation and research. Source text bytes
remain Git-ignored under exact Item `payload/` paths; metadata, rights,
fixity, provenance and branch routes remain tracked. Local acquisition does
not authorize public transfer of the source files or establish a universal
rights determination.

## Verification and admission limits

The preparation is frozen and checkpoint-reviewed before complete source
acquisition. Each file must match its pinned Git blob and size, reviewed
header prefix, exact English CTS translation identity, parsed TEI structure,
qualified source division addresses and nonempty Latin-letter text. The
letter count is a mechanical check, not independent language identification
or translation-quality assessment. SHA-256 and resource inventory are
recorded from the actual local file.

Run a complete first target through acquisition, guarded Work extension and
branch linking before the rest of the batch. Rebuild source catalogs,
registry coverage, branch projections and documentation after all selected
versions have verified local custody. Retain the old version routes and
validate their continued local availability. Mechanical green remains
separate from semantic review and remote publication.
