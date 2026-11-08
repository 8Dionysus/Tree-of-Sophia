# TOS-D-0079 Media on Stand Cards

## Index Metadata

- Decision ID: TOS-D-0079
- Original date: 2026-11-07
- Surface classes: bonsai, access/web, access/deploy/cloudflare-worker
- ToS layers: derived projection
- Tree classes: none
- Guard families: source-first authority, rights separation, bounded downstream seams, public delivery security
- Posture: accepted by explicit operator instruction

## Context

The stands are to show media on their cards, first a portrait beside each
author's name. A later printed or painted portrait shows how a later age
imagined the author, so its card has to say who made it and when. The images
come from museums' IIIF Image API services, which the Tree's media bridges
observe (TOS-D-0078). The stands' pages are cross-origin isolated and admit
images and connections from their own origin alone.

## Decision

A bonsai keeps its media in a module of its own, `media.json`
(`tos.bonsai.media.v1`), in three levels: the work its holder keeps, with the
provider's titles, makers, dating, size and record word for word; the image,
one IIIF Image API 3 service with its pixel size and the provider's rights
statement; and the use, which binds an image to a subject of the stand in a
role (an author's portrait, a material of the era, an object of analysis, a
comparison). Every entry comes from a provider observation, and one command
(`node bonsai/lib/cli.mjs media add`) attaches it to a card and names its Tree
Artifact and representation in the graft map. A stand shows an image under the
Public Domain Mark or CC0.

The hosts of the image services are one list, `bonsai/lib/media-hosts.mjs`.
The bonsai check refuses a medium on any other host, and the site Worker adds
exactly these hosts to `img-src` and `connect-src` of the stands' pages.

A card loads the media module when it first opens, never with the space. It
shows each image whole, in its own proportions, as a copy fitted into 480
pixels (960 on dense screens) requested from the service in CORS mode, with
its caption: the role and the dating, then the provider's title, the
inscription on the object, the series, the makers and the size, and links to
the holder's record and the rights statement. The image opens in a modal
dialog over the stand, where OpenSeadragon reads the service's `info.json` and
its tiles down to full detail; the viewer is a separate chunk loaded on first
use, and closing it returns focus to the image on the card.

## Options Considered

- Put the images into the stand's manifest or the collection: they would load
  with the space and mix provider facts with the stand's own words.
- Proxy the images through the site Worker: the services already answer CORS
  requests and serve tiles at every detail; a proxy would add a route without
  adding a fact.
- Admit any `https:` image source in the CSP: one list of named hosts keeps the
  pages' policy as narrow as the media they show.

## Rationale

The provider's words stay the provider's: a caption is built from the facts
the observation recorded, and the stand adds only what the image is for on
its card. A dated caption says plainly that a portrait of 1566 is an image of
its own time. Loading media on demand keeps the space as fast as before, and a
single list of hosts ties what the check admits to what the page may load.

## Consequences

A new provider joins with its bridge, its host in the list and its first
medium. A file-only provider such as the Met needs a small preview from its
bridge before a card can show it. The viewer writes no style of its own: its
one touch-screen rule lives in the stand's stylesheet. Stars stay stars; an
image belongs to the card.

## Source Surfaces

- `bonsai/lib/media.mjs`, `bonsai/lib/media-hosts.mjs`, `bonsai/lib/cli.mjs`
- `bonsai/antiquity/media.json`, `bonsai/antiquity/identities.json`
- `access/web/src/public-research/media-view.mjs`
- `access/web/src/public-research/public-space.mjs`
- `access/deploy/cloudflare-worker/src/public-prototypes.ts`

## Validation

`bonsai-media.test.mjs` covers the module's contract, the attach command's
entries and graft names, the refusals, the Antiquity portraits and the
caption. The Worker test checks the stands' Content-Security-Policy. A built
site served through the Worker entry was checked in headless Chrome with the
real policy: the portraits of Cicero, Seneca, Plato, Aristotle and Socrates on
both stands, in Russian, English, German and Chinese, light and dark, on a
desktop and a narrow screen, the viewer at full detail, Escape and focus
return, and no policy violations.
