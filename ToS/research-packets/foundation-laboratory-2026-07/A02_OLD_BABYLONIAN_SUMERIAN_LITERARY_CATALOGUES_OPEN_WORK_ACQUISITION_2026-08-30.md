# Old Babylonian Sumerian literary catalogues: open-work acquisition

Date: 2026-08-30
Candidate: `open-work-candidate.old-babylonian-sumerian-literary-catalogue-witnesses`
Posture: acquired file source witness; unresolved work-like corpus boundary; unreviewed

## Outcome

The candidate is not admitted as one ancient Work, one original catalogue, a
closed corpus, a universal curriculum, or a canon. The existing
`tos.composite.sumerian.old-babylonian-literary-catalogue-witnesses` remains a
provider-independent modern synoptic object coordinating materially distinct
tablets and versioned editorial mappings.

One exact useful object is now held locally: the ETCSL `c.0.2.02.xml`
composite transliteration, whose own header identifies “OB catalogue in the
Louvre (L)”, the physical source as `AO 5393 (TCL 15 28)`, and the named ETCSL
editorial and technical responsibilities. The File is a modern editorial
representation of the inscription on a physical tablet. It is neither the
tablet, a photograph, an ancient original, nor an accepted source-text layer.

## Exact acquired object

| Field | Value |
| --- | --- |
| Originating record | [Oxford Text Archive, ota:2518](https://ota.bodleian.ox.ac.uk/repository/xmlui/handle/20.500.12024/2518?show=full) |
| Download | official `etcsl.zip`, sequence 11 |
| Container SHA-256 | `d1a35b396399216deaeb483d5954ae603662e73c4e77f23e39f2e7b58466962b` |
| Container bytes | 4,910,212 |
| Selected member | `etcsl/transliterations/c.0.2.02.xml` |
| File SHA-256 | `2979f2e86cc8a869c252fd84c81a28c0d8786780f51f02e36063835d3177410d` |
| File bytes | 25,199 |
| Local representation | `tos.composite-representation.sumerian.old-babylonian-literary-catalogue-ota-2518-c0202` |
| Local File | `tos.file.sha256.2979f2e86cc8a869c252fd84c81a28c0d8786780f51f02e36063835d3177410d` |

The official ZIP was acquired through the in-app browser after the operator's
explicit download authorization. No access control, login, paywall, crawler
workaround, or technical bypass was used. The ZIP is fixed as the transport
container and was not copied into the corpus; only the exact selected XML
member is retained under the ignored representation `payload/` path.

The host storage preflight returned `deny` because `/srv/AbyssOS` is protected
from machine-owned storage automation, while reporting sufficient capacity.
Per `LOCAL_STORAGE_BOUNDARY.md`, this does not redirect operator-designated
source evidence into `/srv/abyss-machine`; the explicit project route owns the
small local-only payload.

## Rights by layer

The OTA full record states that the revised ETCSL deposit is distributed by
the University of Oxford under
[CC BY-NC-SA 3.0 Unported](https://creativecommons.org/licenses/by-nc-sa/3.0/).
The positive conclusion is limited to the exact OTA deposit member and its
embedded modern editorial content.

| Layer | Posture |
| --- | --- |
| Exact `c.0.2.02.xml` transliteration | licensed CC BY-NC-SA 3.0; attribution, NonCommercial, and ShareAlike conditions |
| Embedded ETCSL notes and composition mappings | same exact-file license; still a distinct modern commentary layer |
| Provider-independent ToS composite metadata | public metadata-only; no provider database license inferred |
| Live ETCSL HTML | not silently covered by the byte-distinct OTA File record |
| CDLI HTML, JSON, ATF, line art | custom and layer-specific terms; not acquired under this receipt |
| Penn, Louvre, or CDLI photographs | not authorized by the XML license and not acquired |
| Physical AO 5393 tablet | not a copyrightable File and not replaced by the XML |
| De Gruyter article body | subscription route; no access bypass and no body acquired |

The local payload policy is stricter than the license: the XML remains
`local_only`. The license permits conditional reuse; it does not perform
philological review, accept incipit identifications, establish curriculum,
create semantic edges, promote canon, or authorize publication by ToS.

## Discovery reconciliation

The current run preserves ten ordered channels from authority and holding
records through scholarly projects, the OTA repository, aggregation,
identifier registry, open library, and general web last. Each channel has a
positive machine timing produced with `python.time.perf_counter_ns`; the
measurement covers only transport through the first 16 KiB, not research,
interpretation, rights review, or human time.

Useful source-returnable controls include:

- [ETCSL ancient literary catalogues](https://etcsl.orinst.ox.ac.uk/catalogue/catalogue0.htm), for dated project numbering and coverage;
- [CDLI P345372](https://cdli.earth/artifacts/345372), for the current AO 5393 artifact and composite relation;
- [Penn Museum object 521562](https://collections.penn.museum/collections/object/521562), for a separate Nippur holding route and unresolved original/cast distinction;
- [Delnero 2010](https://doi.org/10.1515/za.2010.003), as an interpretation control that argues the Nippur and Louvre lists are inventories rather than curricular sequences;
- the existing 2026-08-23 source-planting research, which remains the fuller dossier for coverage divergence and Decad evidence.

## Operational relations

The terminal receipt binds the frozen candidate and queue snapshot to the
measured discovery run, timing receipt, provider-independent composite,
physical artifact, exact representation, File, rights record, acquisition
event, existing source planting, and this research packet. These are
operational evidence relations only.

The XML header's `AO 5393` source description supports an evidence-bearing
representation-to-artifact relation. It does not prove that the editorial
composite exhausts the inscription, that every incipit mapping is correct, or
that the tablet belongs to one stable ancient corpus. Those judgments remain
for human philological and semantic review.

## Remaining decisions

- Human legal/publication review may confirm the attribution string and any
  intended external redistribution under CC BY-NC-SA 3.0.
- Human philological review may assess the ETCSL readings, gaps, line
  segmentation, and composition mappings against current editions.
- The Nippur original-versus-cast identity remains unresolved.
- Other catalogue members may become later candidates only when the frontier
  review identifies a distinct useful work-like pressure; this receipt does
  not mechanically mint thirteen Works from thirteen XML members.

No new architectural decision was required. TOS-D-0039 already owns the
reviewed candidate loop, and TOS-D-0040 already owns file-backed scholarly
composite representations.
