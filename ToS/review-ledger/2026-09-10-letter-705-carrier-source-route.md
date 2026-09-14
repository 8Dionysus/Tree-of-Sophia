# Letter 705: bounded archival-carrier source route review

Date: 2026-09-10. Reviewer/preparer: `model:codex`; source-visible issuer review
was performed by the coordinating model in the same work cycle. No human
review, legal clearance, textual/semantic admission or canon decision is
claimed. The source records retain their initial unreviewed posture.

## Scope and exact source commits

Base: `5d4ac49f60895501e0db53dcef5f123d76ce58c7`, a descendant of the requested
integration baseline `d899af2e`. The isolated source branch is
`codex/tos-letter-705-inputs-20260910`.

| Commit | Source-owned change |
| --- | --- |
| `b7f570dd6cf61af7a5e33f681f27a6dcb1fe7c93` | Three independent Artifact inputs and one appended discovery observation event |
| `bcc57628aff4a4ea2b925b3465ed75dabc285352` | Native Artifact v2 metadata, two source-copy forms and immutable creation companions |
| `9486313dddf49126d50d94b617c514b7f81252d6` | Qualified `document_carried_by` Claim and its immutable creation companions |
| `e73eab6e9f7461851657f96684ed2a62ce9a5d98` | Separate source-copy statement form for that Claim |

Each commit received a non-automatic agent checkpoint review against its exact
parent. Those reviews record the local changes and evidence; they do not
replace the source issuer's decisions or assert integration, CI or deployment.
No other historical event, Letter, Agent, Work, interpretation or relation
profile was changed. In particular, this package does not implement the
separate document date/place contract work.

## Source-visible review and rights limits

The selected primary source is the [Goethe- und Schiller-Archiv correspondence
record 4752](https://ores.klassik-stiftung.de/ords/f?p=406:2:::::P2_ID:4752).
The coordinating source issuer independently read the current rendered catalog
fields and confirmed the sender and addressee, date, correspondence places,
one-leaf extent, shelfmark/leaf and printed-edition reference reported in the
[current observation note](../research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_ARCHIVE_METADATA_2026_09_10.md).
This confirmation has no reconstructed exact timestamp. The same issuer read
both that note and the unchanged [earlier source-reading note](../research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_SOURCE_READING_V1.md),
reviewed the complete carrier Claim draft against the declared relation's
definition/domain/range, and confirmed that the created Claim equals the
reviewed draft, including all qualifiers.

The preparer's two HTTP observations were narrower: one checked four markers,
the other measured the response and extracted the page title but no catalog
table fields. Their exact measured times, byte counts and SHA-256 values live
in the observation note and discovery event. These probes are not falsely
described as independently reading every catalog field. Both response bodies
were discarded from process memory, with `captured: false`; no catalog HTML,
image, letter text, transcription, translation or marginalia was retained or
republished. The two different hashes do not demonstrate a change in the
catalog facts, and no physical-source fixity is inferred.

The issuer accepted only the bounded ToS-authored factual citation and
metadata-only derivative. The [rights record](../source-witnesses/rights/nietzsche-letter-705-carrier-metadata.2026-09-10.json)
keeps `copyright_undetermined`, empty permissions, null license and rights
statement, no reviewed jurisdiction, `public_metadata_only` / `metadata_only`,
and `local_research_only` content-derivative posture. This is not a new
third-party license, content-publication permission or source-text admission.
Source availability and an HTTP digest are not rights decisions.

## Independent fixed input bindings

All three inputs existed before Artifact issuance and remain byte-identical.

| Input | Exact SHA-256 |
| --- | --- |
| [Rights](../source-witnesses/rights/nietzsche-letter-705-carrier-metadata.2026-09-10.json) | `20fef8ee6b9cf16f815ae39dc002b4b7ad33a419a188e18e25bcd632cf5d31f1` |
| [Discovery](../source-witnesses/discovery/runs/nietzsche-letter-705-carrier-metadata.2026-09-10.v1.json) | `9223ad53cef04c0902fd79e3b1c3d849fd6397070aefc040bdee75a3d37d5405` |
| [Research](../research-packets/foundation-laboratory-2026-07/JENSEITS_1886_LETTER_705_ARCHIVE_METADATA_2026_09_10.md) | `cd330647ba8f3b33ed78648b3010dbe5e4347879cbb165e346a52c48b865b76d` |

The new event in [discovery provenance](../source-witnesses/discovery/provenance.jsonl)
is `tos.event.discovery.nietzsche-letter-705-catalog-observation-20260910`.
The original journal prefix was preserved; exactly one line was appended.
That appended line including its newline has SHA-256
`e3bbc3105490195206c11f2df748c229eaa386ae8daebd0acf4306a00eb98b24`.
The event binds the actual input/output bytes and distinguishes observation
and later input preparation from acquisition and native source serialization.

## Native identities and exact relationships

The [Artifact source](../source-witnesses/artifacts/nietzsche-correspondence/weimar/gsa-71-bw-291-1-leaf-8/artifact-witness.json)
is `tos.artifact.nietzsche-correspondence.weimar.gsa-71-bw-291-1-leaf-8`,
version 1, canonical digest
`072e0e2981db0b8b739a96868d350d0b10cae9c4a715f050f62f4e8b40b23af6`.
Its physical identity follows the reported repository shelfmark, not digital
record 4752. Material, dimensions, physical paper date, script and language
are not invented. The actual inventory label is preserved as the name form;
the source-attributed description is preserved as the hover form, both with
the native schema's null language/script metadata.

The [Claim source](../source-witnesses/relations/nietzsche-letter-705-carrier/source-claims.jsonl)
is `tos.claim.nietzsche-letter-705.carrier`, version 1, canonical digest
`28f918b42e3a3c0665df570b703188a0e0c158c7be84d4d6056b832637cd5a8e`.
It relates existing `tos.letter.nietzsche-naumann-1886-705` to the new Artifact
through `document_carried_by`, with `bibliographic_assertion`, `reported` and
public-metadata-only posture. The intellectual document and the physical
carrier remain different identities. Autograph, delivery, reading, complete
preservation, custody continuity, textual equivalence and reuse rights are
explicitly outside the claim. Catalog letter dating is not reassigned to the
existing commissioning event.

The [Claim form](../source-witnesses/relations/nietzsche-letter-705-carrier/source-claims.25b689d525e37d151097c839d57239cc4638db9eaadc99c1d8bbb7102e2f68cf.human-forms.json)
is `tos.form.nietzsche-letter-705.carrier-statement`, version 1, canonical
digest `2a8800bbd8b4b1bb34e18230e913ef8f84853bc377c9c855887c20f6f82aca29`.
It copies `claim.statement` in Russian/Cyrillic and retains the complete exact
Claim as mandatory context. It is not a freeform summary, standalone reading
or assessment; admission remains null.

## Verification and review boundary

Performed on the actual bounded package:

- Rights, discovery and observation-event schema validation; unique event ID,
  exact one-line journal append and every observation-event input/output hash.
- Separately issued, current Artifact, Claim and form grants; no self-issued
  grant or expansion of an earlier grant's operation scope.
- Artifact prepare/create, exact replay, six-file byte stability, unchanged
  three input bindings, grant-independent native origin verification, and two
  ready source-copy forms.
- Claim prepare/create, exact replay, all five receipt file hashes/sizes,
  stored-Claim equality with the independently reviewed draft, and exact
  current raw/canonical endpoint and evidence bindings. Both endpoint records
  and both evidence notes remained unchanged.
- Form preparation against the exact Claim version/digest and absent form
  revision, apply/replay, exact statement text, full-Claim context equality,
  RU/Cyrl preservation and unchanged original five Claim files. Claim creation
  replay also passed after the form was added; Artifact origin still verified.
- Staged whitespace checks and manual source-layer/boundary review against
  the ToS review checklist. The published metadata, physical/document identity,
  observation, rights posture, source serialization and semantic admission
  remain separate. No independent reusable-test pressure beyond the existing
  native serializer/form contracts was found; no incident-only test was added.

Resource-admitted runs completed successfully without swap: Artifact
105.4 MiB peak, Claim 85.2 MiB peak, and form plus post-form replay/origin checks
30.8 MiB peak. These are observed local peaks, not performance guarantees.

The final `python3 scripts/validation_lanes.py --run source_home` lane passed:
source-home manifest/branch topology and the lived-witness route's structural
private boundary. The latter does not assess human authorship, consent,
memory, context or meaning. The resource-admitted lane used 31.9 MiB peak and
zero swap. No private source-content inspection is claimed.

Not performed: full source-witness-foundation/catalog parity, full corpus
generation, graph/KAG/runtime bundles, global registry/graph validation,
browser/UI acceptance, CI, merge, release or deployment. In particular, current
generated catalogs have not been refreshed by this source-only branch.
The individual serializer results do not establish those downstream outcomes.

Next owner: foundation integration applies the four exact source commits and
this review note, regenerates only required owned companions, and runs the
relevant source/graph/reader validation on the union. Further content use,
textual or semantic assessment and document date/place work remain separately
owned. No source byte needs rewriting to claim those later outcomes.
