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

The stands are to show media on their cards: a portrait beside each author's
name, and on the Zarathustra stand works that share a motif with a star, from
a curated media registry. A later printed or painted portrait shows how a later age
imagined the author, so its card has to say who made it and when. The images
come from museums' IIIF Image API services, which the Tree's media bridges
observe (TOS-D-0078); the Met serves files instead, with neither CORS nor a
resource policy. The stands' pages are cross-origin isolated and admit images
and connections from their own origin alone. In a check of 7–8 November 2026,
Chrome blocked a Met file on such a page under `require-corp`, and loaded the
Rijksmuseum's and the National Gallery of Art's IIIF images in CORS mode.

## Decision

A bonsai keeps its media in a module of its own, `media.json`
(`tos.bonsai.media.v1`), in three levels: the work its holder keeps, with the
provider's titles, makers, dating, size and record word for word; the image,
with its pixel size and the provider's rights statement, as one IIIF Image API
3 or 2 service or as the provider's file and its smaller copy; and the use, which
binds an image to a subject of the stand in a role (an author's portrait, a
response to the book, a shared motif, a material of the era, an object of
analysis, a comparison). Every entry comes from a provider observation, and one command
(`node bonsai/lib/cli.mjs media add`) attaches it to a card and names its Tree
Artifact and representation in the graft map. A stand shows an image under the
Public Domain Mark or CC0, or one its provider flags as free to use (the Met's
`isPublicDomain`, the National Gallery of Art's `openaccess`, the Saint Louis
Art Museum's `Rights`, a Wikimedia Commons file's public-domain status or, where
its structured data states none, its page's public-domain licence).

A provider's files reach the page as the stand's own copies. The bridge
fingerprints the file and its smaller copy (`"files": true`); the attach
command keeps them only when their bytes match, under `media/` in the stand's
layer store; R2 holds them under their digest (TOS-D-0076), the Publish Bonsai
workflow writes them from main, and the site serves them at
`/__research/layers/sha256/<hex>`, from its own origin.

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

A link's card shows images right under its sentence, as a strip of small
copies that open in the same viewer. A link of the core is a subject of the
media module like a star; where the module binds images to the link, the strip
shows those. Elsewhere it shows the images of the link's two ends, each group
under its end's name, an image both ends share once. A link folded into
another card shows none.

## Options Considered

- Put the images into the stand's manifest or the collection: they would load
  with the space and mix provider facts with the stand's own words.
- Proxy the images through the site Worker: the IIIF services already answer
  CORS requests and serve tiles at every detail. For the Met's files a proxy
  resolving the current file by object number would follow a moved address,
  but it would show whatever the provider serves that day; in the first
  observations two Met originals came back as other bytes ten minutes later.
  A copy under its digest shows the file that was observed.
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
medium, or with its files fingerprinted and copied. The first Zarathustra
slice held the registry's shared motifs with open images from the Met, the
Rijksmuseum and the National Gallery of Art, 32 stars. The second holds the
registry's partial correspondences and meaning comparisons under the role
`comparison`, from the images already on the stand and 31 more files of the
Met: 192 stars with images in all. A card shows the images and their captions
alone; the registry's reasons stay with the registry. Of the registry's 784
link selections, 697 join the images of the two ends and so read from the
stars themselves; the 87 made for a link's own meaning stand on the link where
their images are open, 60 links. The third slice brings the registry's works
whose holders keep closed terms: flat public-domain works through Wikimedia
Commons under the status Commons states for the file and the law that leaves a
faithful photograph of such a work free (Article 14 of the Digital Single
Market Directive in the EU, Bridgeman v. Corel in the US), Leighton's bronze
from the Saint Louis Art Museum and two paintings from the Staatliche Museen zu
Berlin under their own statements. A Commons caption cites the holder and its
number and links the status to the file's page. The stand then shows 76 works
on 267 stars and 87 links; every link the registry selected for its own meaning
has its images. A Commons file whose structured data names the photograph
copyrighted stays out, and the registry's works without an open copy (a
fragment of a triptych yet to be marked, drawings of the Louvre and Weimar,
prints of the Museum of Modern Art, a Munch drawing the Met keeps closed, a
painting of the Hungarian National Gallery) wait. Antiquity's cards carry art
of their own: a portrait for twelve of its eighteen authors (the Met's marble
head of Epicurus, the Nuremberg Chronicle's woodcuts of Xenophon and Plotinus,
Galle's Zeno, Marcus Aurelius among the emperors), the responses its works
received (David's Death of Socrates for the Phaedo, Testa's Symposium,
Saenredam's Cave for the Republic, Raphael's School of Athens for the Timaeus
and the Ethics Plato and Aristotle hold, the Tree of Porphyry, title pages of
Lucretius and Epictetus, the Choice of Hercules for the Memorabilia, Diogenes
with his lantern for the Lives), and motifs of its schools and concepts; it
keeps the copies of its file images in a layer store of its own. Plutarch,
Porphyry, Lucretius, Sextus Empiricus and Diogenes Laertius have no portrait
yet. The viewer writes no style of its own: its
one touch-screen rule lives in the stand's stylesheet. Stars stay stars; an
image belongs to the card.

## Source Surfaces

- `bonsai/lib/media.mjs`, `bonsai/lib/media-hosts.mjs`, `bonsai/lib/cli.mjs`
- `bonsai/antiquity/media.json`, `bonsai/antiquity/identities.json`
- `bonsai/zarathustra/media.json`, `bonsai/zarathustra/layers.json`
- `access/web/src/public-research/media-view.mjs`
- `access/web/src/public-research/public-space.mjs`
- `access/deploy/cloudflare-worker/src/public-prototypes.ts`

## Validation

`bonsai-media.test.mjs` covers the module's contract, the attach command's
entries and graft names, the refusals, the Antiquity portraits, the Zarathustra
slices, the Commons, Saint Louis and Berlin works with their keys and rights
links, and the caption. The Worker test checks the stands' Content-Security-Policy. A built
site served through the Worker entry was checked in headless Chrome with the
real policy: the portraits of Cicero, Seneca, Plato, Aristotle and Socrates on
both stands, in Russian, English, German and Chinese, light and dark, on a
desktop and a narrow screen, the viewer at full detail, Escape and focus
return, and no policy violations.
