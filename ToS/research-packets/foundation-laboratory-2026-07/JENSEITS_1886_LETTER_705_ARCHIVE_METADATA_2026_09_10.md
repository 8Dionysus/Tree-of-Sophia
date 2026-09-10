# Letter 705: bounded archival metadata observation, 10 September 2026

Status: unreviewed source-reading research input, prepared by `model:codex`.
This is a metadata-only proposal for issuer review, not Artifact creation,
Claim creation, source admission, palaeographic assessment, legal clearance or
publication authority. The earlier
[reading note](JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md) remains unchanged.

## Exact target and source attribution

The proposed physical identity is
`tos.artifact.nietzsche-correspondence.weimar.gsa-71-bw-291-1-leaf-8`.
Its identity basis is the holding repository's shelfmark and leaf, not the
digital provider's record number. It is not yet a created source object.

The originating [archive record 4752](https://ores.klassik-stiftung.de/ords/f?p=406:2:::::P2_ID:4752)
is the only primary catalog selected here. During this session the coordinating
model reported reading its rendered fields: Friedrich Nietzsche as sender,
Constantin Georg Naumann as addressee, 3 June 1886, Naumburg to Leipzig,
one leaf, holding location GSA 71/BW 291,1, leaf 8, and the printed reference
KGB 3.3, p. 193, no. 705. The catalog also reports a receipt mark and a comment
attributed to Naumann. These are attributed catalog facts, not my independent
inspection of the handwriting, physical sheet, receipt mark or delivery.
The current report agrees with the bounded archival identity in the earlier
reading note; agreement does not create another independent historical witness.

The recipient display associates the person with a Leipzig printing firm.
That display does not equate person, organization, correspondence role or
physical carrier. The archive describes a document: catalog authorship, date,
places and an attributed added mark remain reported evidence, not autograph,
dispatch, receipt, reading, agreement or printing-performance findings.
Material composition, dimensions and physical manufacture date have not been
established by this observation.

## Response observations and limits

This agent independently made two bounded, credential-free HTTP reads of the
exact public URL using Node.js `v22.23.1` fetch with a 25-second timeout and
SHA-256 over the returned response body in memory. No response body was written
to disk or retained as a snapshot. Both responses were HTTP 200 and
`text/html;charset=UTF-8`.

| Observation | Started at UTC | Completed at UTC | Bytes | SHA-256 |
| --- | --- | --- | ---: | --- |
| Initial probe | 2026-09-10T06:27:13.784Z | 2026-09-10T06:27:18.382Z | 20393 | `1fc8c2c2787f1fa0ec444bce2ff9ce0745f919bf38a3f40354780d66d3ab7360` |
| Exact-record refresh | 2026-09-10T06:39:06.389Z | 2026-09-10T06:39:10.555Z | 20421 | `c5d675782be8190b528f822d6439c730c7cce0938809636556d6ea934f3c7c2e` |

The first probe checked only the presence of four strings: the shelfmark
fragment, recipient surname, displayed date and record number. The refresh
returned the page title `Vollanzeige`; a narrow table-row extractor found no
matching rows. Neither probe is a full field extraction or a palaeographic
reading. Catalog content attribution above therefore remains explicitly tied
to the coordinating model's rendered reading, not falsely attributed to this
agent's extractor. The earlier web-tool open refused the URL as unsupported;
that tool refusal is not evidence of archive unavailability or an access ban.

Both fingerprints have `captured: false`. The refresh is the selected response
for a future Artifact's `digital_catalog_record`; its exact URL, time, byte
count and hash must stay together. Different hashes do not prove a metadata
change: dynamic response bodies were not retained or compared. The discovery
record's acquisition fields are null with `downloaded: false`, meaning no
retained corpus acquisition; the HTTP transfer itself is not denied.

The two image sides described in the earlier note were not opened in these
HTTP probes. No new image inspection, transcription, translation, text
equivalence, image identity or preservation result is claimed. No source
HTML, catalog prose, scan, letter text or marginalia is republished here.

## Rights and authoring boundary

The [rights input](../../source-witnesses/rights/nietzsche-letter-705-carrier-metadata.2026-09-10.json)
concerns only this new ToS-authored, bounded citation and factual metadata
derivative. The basis is the source owner's separation of public-safe
bibliographic/provenance metadata from content-bearing derivatives in
[CORPUS_FOUNDATION](../../doctrine/CORPUS_FOUNDATION.md#rights-and-visibility-inheritance),
plus the explicit delegation to prepare these inputs for issuer review.
It is not a finding that all archive metadata is unprotected or freely reusable.
No license, holder permission, jurisdiction-specific determination, source
public-domain status or content redistribution authority was established.
Permissions remain empty; copyright is undetermined, and content derivatives
remain local-research-only. Future source-content use needs its own exact-layer
decision. Public viewing and response measurement do not supply that decision.

## Bounded continuation

The [discovery input](../../source-witnesses/discovery/runs/nietzsche-letter-705-carrier-metadata.2026-09-10.v1.json)
and its observation event preserve this limited execution. The observation
event is not an acquisition, source-create receipt or authenticated upstream
model execution attestation. Inputs await independent issuer review.

The separately delegated Artifact route may later bind these exact three
input byte sequences. A distinct `document_carried_by` Claim can then connect
the existing `tos.letter.nietzsche-naumann-1886-705` to the new Artifact. No such
operation or grant is created here. Original Letter, Event, sender, addressee,
Work, claims and creation history are unchanged.

Current `historical_dating` and `historical_place` profiles have
historical-situation domains, not Letter domains. A letter's catalog date and
places cannot silently become the existing commissioning event's date and
place. A separate owner decision is needed for that independently addressable
document profile. Economic or lived context is not established by this
catalog observation; the earlier critical-text and scholarly readings retain
their own evidence and limits.
