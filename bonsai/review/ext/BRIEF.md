# Exact fragments of the works the Zarathustra stand leans on

## Why

The Zarathustra stand of Tree of Sophia (a public research site) is being
rewritten so that it never retells a work in its own words: whatever a work
says is shown as an exact fragment, in the original language and in the
reader's language, with its edition and rights. The stand leans on several
works besides Zarathustra. Your job is to obtain the exact text of the places
listed below, in the original and in translations, from editions whose rights
allow putting them on the site, and to record the rights basis carefully.

The rights rule of the owner: "Study the licence carefully: never place what is
forbidden, and never leave out what is allowed through an oversight or, worse,
through a too cautious and wrong reading." So: a public-domain text goes in
whole; a copyrighted work goes in only as a short quotation under the quotation
right (Russia: Civil Code art. 1274 p. 1 sub. 1, quotation in the original and
in translation for scientific, critical, polemical or informational purposes in
a volume justified by the purpose, with author and source named; France/EU:
CPI L122-5 3° a, courtes citations; USA: 17 U.S.C. §107 fair use), with
attribution, and never from a pirated copy.

## Places

1. Nietzsche, *Die fröhliche Wissenschaft*, Book IV: §276 ("Zum neuen
   Jahre") and §341 ("Das grösste Schwergewicht"), whole aphorisms. German:
   the first edition (Chemnitz: Schmeitzner, 1882) from the Deutsches
   Textarchiv (deutschestextarchiv.de, CC BY-SA 4.0; use its TEI or plain-text
   download, the way the DTA offers it, not a scraper that gets around a bot
   check). English: Thomas Common's translation, *The Joyful Wisdom* (1910),
   Project Gutenberg. Russian: a public-domain translation if one exists
   (translator died before 1943), else note what exists and whether it is
   protected.
2. Spinoza, *Ethica* (Opera posthuma, 1677): I Def. 7; I P25 corollary; I P34;
   III P4; III P5; III P6 with demonstration; III P7 with demonstration; III P8
   with demonstration; III P9 with scholium. Latin original; English (Elwes
   1883, Project Gutenberg #3800, or White); Russian (Н. А. Иванцов, if on
   Wikisource or another legitimate source).
3. Leibniz, *Monadologie* (1714), §§53, 54, 55. French original; English (Latta
   1898); Russian (a pre-1943 translator, e.g. the 1890 or 1908 translations, if
   legitimately available).
4. Kierkegaard (Constantin Constantius), *Gjentagelsen* (1843): the opening
   paragraph of Part One (from "Da Eleaterne negtede Bevægelsen" to "...at han
   har glemt Noget" and on to the end of that paragraph). Danish original (1843 or
   the Samlede Værker 1901-06, Project Runeberg, or SKS if its terms allow);
   Russian: П. Г. Ганзен's translation «Повторение» (1894), if legitimately
   available; English: only a short quotation from a translation, if you can
   verify it exactly against a reliable edition.
5. Heraclitus, Diels-Kranz 22 B12 (Arius Didymus in Eusebius, Praep. ev.
   XV.20.2), B30 (Clement, Stromata V.104-105), B91 (Plutarch, De E 18, 392b).
   Greek (Diels 1903 or the source texts, e.g. First1KGreek or Perseus on
   GitHub, CC BY-SA 4.0); English (Burnet, *Early Greek Philosophy*, 1920,
   public domain; give Burnet's own fragment numbers); Russian (if only a
   protected translation exists, e.g. Маковельский 1914, a short quotation with
   attribution).
6. Camus, *Le Mythe de Sisyphe* (1942), final chapter "Le mythe de Sisyphe":
   the paragraphs beginning "C'est pendant ce retour, cette pause, que Sisyphe
   m'intéresse", "Si ce mythe est tragique, c'est que son héros est conscient",
   "Si la descente ainsi se fait certains jours dans la douleur", "Toute la joie
   silencieuse de Sisyphe est là" and the last paragraph ("Je laisse Sisyphe au
   bas de la montagne" ... "Il faut imaginer Sisyphe heureux."). Protected in
   France, the EU, Russia and the USA; public domain in Canada (Camus died
   1960), where "Les classiques des sciences sociales" (UQAC) publishes it. We
   may place short quotations only: for each paragraph, the one to three
   sentences that carry it, exact. Translations: short quotations from
   published translations (English: Justin O'Brien, 1955; Russian: the
   translation in print, e.g. А. М. Руткевич) only if you can verify the exact
   wording against a reliable edition; otherwise leave the translation null.
7. Deleuze, *Nietzsche et la philosophie* (PUF, 1962), ch. II, end of §5 and
   §6; *Différence et répétition* (PUF, 1968), Introduction (the passages on
   Kierkegaard and Nietzsche, the four propositions, the two corrections of
   Zarathustra to the dwarf and to the animals, the eternal return as
   selective). Protected everywhere. Exact short quotations only if verifiable
   against a reliable edition (never from a pirated PDF); otherwise give the
   exact locator (chapter, section, page of the French edition and of the
   English translation) and no text. Also find the publishers' pages: PUF for
   both originals, Columbia University Press for Tomlinson's *Nietzsche and
   Philosophy* and Patton's *Difference and Repetition*; the stand now links to
   copies on blogs.law.columbia.edu, which must be replaced.

## How to fetch

- The shell reaches Wikisource, Project Gutenberg, GitHub and the DTA; other
  sites usually only through WebFetch. WebFetch passes pages through a small
  model that may alter wording, so never take exact text from WebFetch output;
  use it only to locate a source, then fetch the exact text from the shell
  (MediaWiki API `action=parse&prop=wikitext` or `action=query&prop=revisions`,
  Gutenberg `.txt`, DTA TEI or TXT, GitHub raw XML).
- For Wikisource use the polite client
  `bonsai/review/sources/wiki.py`
  (one request at a time, honours Retry-After). Never put any personal e-mail
  address in a User-Agent or anywhere else.
- Keep every downloaded file under `ext/sources/` (one new folder per site), read
  it with scripts kept outside that folder, and run Python with `-I`.
- Do not get around any bot check, login or paywall.

## Output

Write `ext/external-fragments.json`
(`bonsai/review/ext/`):

```json
{"works": {
  "gs": {"author": {"ru": "...", "en": "..."}, "title": {"ru": "...", "en": "...", "de": "..."},
         "witnesses": {
           "de": {"edition": "...", "translator": null, "year": 1882, "url": "exact URL fetched",
                  "license": "CC BY-SA 4.0", "attribution": "exact attribution the source asks for",
                  "rights": {"basis": "public-domain | public-domain+licence | short-quotation", "ru": "...", "us": "...", "eu": "...", "reasoning": "author/translator death year, publication year, licence of the digital copy"},
                  "publisher_page": null},
           "en": {...}, "ru": {...}}},
  ...},
 "places": [
  {"id": "gs.341", "work": "gs", "loc": "341", "label": {"ru": "§341 «Величайшая тяжесть»", "en": "§341 “The Greatest Weight”"},
   "texts": {"de": "exact text", "en": "exact text", "ru": "exact text or null"},
   "source_refs": {"de": "page or section in that edition", "en": "...", "ru": "..."},
   "note": ""}
 ]}
```

- Texts are exact, as printed in that edition (keep its spelling, e.g.
  "grösste", "Nothwendige"; keep line breaks of verse; normalize only
  hyphenation at line ends and whitespace). Paragraph breaks inside a place as
  "\n\n".
- One place per locator (`gs.276`, `gs.341`, `spinoza.1d7`, `spinoza.3p6`
  (proposition with demonstration), `leibniz.53`, `kierkegaard.opening`,
  `heraclitus.b12`, `camus.return` ..., `deleuze.np.2.5` ...). A short quotation
  is its own place with the exact sentences.
- `rights.basis` for every witness, with reasoning that names the death years
  and the law, and the licence of the digital copy you took the text from.

Report back in a few lines: which places have which witnesses, every witness
you could not obtain or that must stay out (and why), and every rights decision
that was not obvious.
