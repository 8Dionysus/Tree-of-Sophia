# TOS-D-0081 Media Captions in the Reader's Language

## Index Metadata

- Decision ID: TOS-D-0081
- Original date: 2026-11-11
- Surface classes: bonsai, access/web
- ToS layers: derived projection
- Tree classes: none
- Guard families: source-first authority, bounded downstream seams
- Posture: accepted by explicit operator instruction

## Context

Each language of the Tree shows a text in its original and in the reader's
language only (`shownLanguages`, `bonsai/lib/locales.mjs`). The captions of the
stands' images, as TOS-D-0079 built them, showed the provider's words as they
are: on a Russian card the Met's and Wikidata's English titles ("Light
Prayer"), and on every card of the Antiquity stand the Rijksmuseum's Dutch
titles ("Hercules op de tweesprong") and its English statements of the makers,
which carry Dutch words of their own. In a walk of the live stands on 11
November 2026, these were the last places where an English or Russian page
showed a third language. The operator ruled that no page may mix in a
language that is neither the original nor the reader's.

## Decision

A stand keeps the Tree's own names of its media works in a module of its own,
`media-names.json` (`tos.bonsai.media-names.v1`): for each work, in each language
it names it in, its titles, its series and who made it, and the names of the
works' holders. A caption shows each of these in the reader's language alone,
the language `translationLanguage` gives: the Tree's name where it has one in
that language, else the provider's words where they are in it, and nothing
otherwise. A provider's record that tags no language is read in the language
its provider writes in (`PROVIDER_LANGUAGES`). The words written on the object
itself stand as written, as the original's own words. The Public Domain Mark
is named in the reader's words. The bonsai check asks every work to be named
in English and Russian; `media add` lists what a new work still needs and does
not refuse it for that.

## Options Considered

- Hide every provider word that is not in the reader's language: a Russian
  reader would see no title at all, because no provider writes Russian.
- Read more languages from the providers: Wikidata labels exist in many
  languages, but the Met, the Rijksmuseum and Berlin write one language each,
  and a label read later is not the observation the work was recorded from.
- Write the Tree's names into `media.json`: that module keeps the providers'
  words exactly as observed, and a new observation of a work replaces its
  entry.

## Rationale

The rule of one original and one reader's language is the Tree's, not a
provider's, so the Tree supplies the words it needs. The provider's record
stays as observed and is one link away on every caption. Keeping the names in
their own module keeps the observation separate from the stand's words, as
TOS-D-0079 requires, and lets a new observation replace a work without losing
its names.

## Consequences

A new medium needs its names in English and Russian before the check passes.
The other interface languages read the work in English, the language of their
copy, until the Tree names it in theirs. A caption names a work once, in one
language; the original title of a painting is not shown unless it is written
on the object.

## Source Surfaces

- `bonsai/lib/media.mjs`
- `bonsai/README.md`
- `bonsai/zarathustra/media-names.json`
- `bonsai/antiquity/media-names.json`
- `access/web/src/public-research/media-view.mjs`

## Validation

- `node bonsai/lib/cli.mjs check zarathustra` and `check antiquity`
- `access/web/src/public-research/bonsai-media.test.mjs`
