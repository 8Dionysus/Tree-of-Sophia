# TOS-D-0078 Media Bridges and Image-Service Representations

## Index Metadata

- Decision ID: TOS-D-0078
- Original date: 2026-11-07
- Surface classes: ToS/contracts, ToS/source-witnesses, rust/tos-command
- ToS layers: source-witnesses, contracts
- Tree classes: none
- Guard families: source-first authority, provenance, rights separation, bounded downstream seams
- Posture: accepted by explicit operator instruction

## Context

The Tree is to carry media from public collections: portraits of authors,
works of art beside the nodes they bear on, and later a tree of art grown from
paintings. The bonsai stands are the first readers. Museums publish their
images through the IIIF Image API at full detail: the Rijksmuseum serves the
Night Watch at 14,645 × 12,158 pixels in 1,024-pixel tiles with a
17.55-megapixel limit per request. The existing
`artifact-visual-representation` contract retains one exact JPEG in Git; it
holds the open-access photographs acquired so far and cannot hold an image of
that size, nor a service that delivers it region by region.

## Decision

A media bridge is a native acquisition family (`media`) that reads one object
from one provider and returns a `media-provider-observation` packet: every
response with its fingerprint, the catalogue facts and rights statements in the
provider's own terms, and the images the provider serves, each IIIF service
described by its `info.json`. It writes nothing. Each response keeps the
deployment profile's acquisition record limit, and each observation states its
own request budget. The first bridges read the Rijksmuseum Linked Art records
and IIIF services and the Met Collection API.

`artifact-visual-representation-v2` keeps the file form of v1 and adds an
image-service form. That form records the observed service, its full
dimensions, tiling and size limits and its exact `info.json` fingerprint; its
identity is the SHA-256 of the service address, and the image bytes stay with
the provider. v1 stays byte-identical, since provenance events and conformance
fixtures pin its digest.

## Options Considered

- Copy every image into Git, as v1 does: a full museum image runs to hundreds
  of megabytes, and the reader would still need tiles for deep zoom.
- Keep the images in R2 and tile them ourselves: the right route for files a
  provider serves without a tile service, and for images the Tree must hold
  itself; it needs its own storage decision.
- Record the provider's IIIF service as the representation and keep a bridge
  that observes it: the reader opens the image at full detail from the source,
  and the record keeps exactly what was seen.

The third option comes first. The second remains open for file-only providers
such as the Met, whose image route serves files without an `info.json`.

## Rationale

A bridge separates reading a provider from admitting anything into the Tree.
Its packet keeps the provider's statements, rights included, as evidence with
their fingerprints, so the later acquisition, rights record and review work
from what was actually served. An image service is an access route that the
representation describes at an observed moment; its fingerprint shows when the
provider changes it.

## Consequences

An observation is planted along the Artifact route: `media.prepare-artifact`
writes the observation as research, an artifact-targeted discovery run,
metadata-only rights and their provenance event, and returns the
`native-artifact-create` request; the native owner creates the Artifact and its
human forms under the operator's configuration; `media.plant-representation`
writes the image-service representation with the operator's rights assessment
and its provenance event. Both foundation validators read the v2 form: an image-service representation is
identified by the digest of its service address, its rights cover the Artifact
and the service at `public_metadata_only` with an authorized redistribution
posture, and its provenance event closes over the record and its rights. A bonsai stand binds its stars to the Tree's
artifact and representation identities through its graft map and reads them;
it authors no media. New providers join the family as bridges with their own
hosts. An observation decides no rights: the provider's statement and the
Tree's rights decision stay distinct.

## Source Surfaces

- `ToS/contracts/artifact-visual-representation-v2.schema.json`
- `ToS/contracts/media-provider-observation.schema.json`
- `ToS/contracts/examples/`
- `ToS/contracts/CORPUS_CONTRACTS.md`
- `ToS/source-witnesses/README.md`
- `rust/crates/tos-command/src/source_media_bridge.rs`

## Validation

The `source_media_bridge` tests read fixtures shaped after the live records:
the Rijksmuseum route from object number to IIIF service and rights, refusals
outside a provider's hosts, partial search matches, records of another object,
the request budget, IIIF 2 and 3 limits, and the contract examples. Live runs of the native command
against the Rijksmuseum (SK-C-5, SK-A-4050) and the Met (438417, 436535, 1)
produced conforming observations.
