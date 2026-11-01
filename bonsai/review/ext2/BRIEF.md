# Exact fragments of the ancient works the Antiquity stand cites outside the public corpus

## Why

The Antiquity stand of Tree of Sophia (a public research site) is being
rewritten so that it never retells a work in its own words: whatever a work
says is shown as an exact fragment, in the original language and in the
reader's language, with its edition and rights. Most works it cites are in the
Tree's public corpus; a few are not. Your job is to obtain the exact text of
the places listed below, in the original and in translations, from editions
whose rights allow putting them on the site, and to record the rights basis
carefully.

The rights rule of the owner: "Study the licence carefully: never place what is
forbidden, and never leave out what is allowed through an oversight or, worse,
through a too cautious and wrong reading." So: a public-domain text goes in
whole; a protected translation goes in only as a short quotation under the
quotation right (Russia: Civil Code art. 1274 p. 1 sub. 1; USA: 17 U.S.C. §107),
with attribution, and never from a pirated copy. A digital copy may add its own
licence (for example CC BY-SA 4.0 for Perseus and First1KGreek TEI files):
record it and its attribution.

## Places

The locators are the ones the stand uses (Bekker lines for Aristotle, letter
and section for Seneca, book and section for Sextus and Xenophon). A place is
the lines or sections the locator names; where a range starts or ends inside a
sentence, take the exact words of those lines in the original, and in a
translation the smallest run of whole sentences that renders them.

1. Aristotle, *De anima*: I.1 402a6-7; II.1 412a19-21, 412a22-23, 412a23-26,
   412a27-b1, 412b4-6, 412b17, 412b18-20, 412b18-22, 412b20-22, 412b22-25,
   412b27-413a1, 413a2-3, 413a3-7; II.2 413b11-13; II.3 414a29-31; III.4
   429a10-13, 429a13-15, 429a15-18, 429a18-20, 429a20-22, 429a29-b5,
   429b4-5. Greek: a public-domain edition with Bekker line numbering (for
   example the Perseus or First1KGreek TEI on GitHub, which mark Bekker pages
   and lines; name the edition the TEI transcribes). English: a public-domain
   translation (R. D. Hicks 1907; J. A. Smith 1931 only if you can establish it
   is public domain where the site is used — say what you found). Russian: a
   translation only as a short quotation if it is protected (П. С. Попов
   1937), and only if you can verify the exact wording against a legitimate
   copy; otherwise null.
2. Seneca, *Epistulae morales ad Lucilium*: 1.1; 9.8; 9.10; 9.20. Latin
   (Hense 1914 or the Latin of Gummere's Loeb 1917, public domain; Latin
   Library or Wikisource la copies are fine when they reproduce a named
   public-domain edition); English: Richard M. Gummere (1917, Loeb, public
   domain; Wikisource has it); Russian: С. А. Ошеров (1977) only as a short
   quotation, verified against a legitimate copy, otherwise null.
3. Sextus Empiricus, *Outlines of Pyrrhonism* (Πυρρώνειοι ὑποτυπώσεις): I.1,
   I.3 and I.1-3 (the opening three sections). Greek: Mutschmann (Teubner 1912,
   public domain) or another public-domain edition; English: Mary Mills
   Patrick (1899, public domain); Russian: Н. В. Брюлова-Шаскольская (1913) if
   public domain (find the translator's death year) and legitimately
   available, else null.
4. Xenophon, *Memorabilia*: 1.6.14 and 4.5.3. Greek: Marchant's OCT text as
   in Perseus (state its licence) or another public-domain edition; English:
   H. G. Dakyns (1897, public domain; Project Gutenberg) or E. C. Marchant
   (1923, Loeb; public domain in the USA, protected elsewhere until the
   translator's term ends — say which you use and why); Russian: С. И.
   Соболевский (1935) only as a short quotation, verified, else null.

The classifiers who found these places noted more of them may come; when the
coordinator sends you more locators, add them the same way.

## How to fetch

- The shell reaches Wikisource, Project Gutenberg and GitHub; other sites
  usually only through WebFetch. WebFetch passes pages through a small model
  that may alter wording, so never take exact text from WebFetch output; use it
  only to locate a source, then fetch the exact text from the shell (MediaWiki
  API `action=parse&prop=wikitext` or `action=query&prop=revisions`,
  Gutenberg `.txt`, GitHub raw XML).
- For Wikisource use the polite client
  `bonsai/review/sources/wiki.py`
  (one request at a time, honours Retry-After). Never put any personal e-mail
  address in a User-Agent or anywhere else.
- Keep every downloaded file under `ext2/sources/` (one new folder per site),
  read it with scripts kept outside that folder, and run Python with `-I`.
- Do not get around any bot check, login or paywall.

## Output

Write `ext2/antiquity-fragments.json`
(`bonsai/review/ext2/`):

```json
{"works": {
  "de-anima": {"author": {"ru": "Аристотель", "en": "Aristotle"}, "title": {"ru": "О душе", "en": "On the Soul"}, "original": "grc",
         "witnesses": {
           "grc": {"edition": "...", "translator": null, "year": 1907, "url": "exact URL fetched",
                  "license": "...", "attribution": "exact attribution the source asks for",
                  "rights": {"basis": "public-domain | public-domain+licence | short-quotation", "ru": "...", "us": "...", "eu": "...", "reasoning": "editor/translator death year, publication year, licence of the digital copy"}},
           "en": {...}, "ru": {...}}},
  "letters": {...}, "outlines": {...}, "memorabilia": {...}},
 "places": [
  {"work": "de-anima", "loc": "II.1 412b18-20", "label": {"ru": "II.1, 412b18–20", "en": "II.1, 412b18–20"},
   "texts": {"grc": "exact text", "en": "exact text", "ru": "exact text or null"},
   "source_refs": {"grc": "page/line in that edition", "en": "...", "ru": "..."},
   "note": ""}
 ]}
```

Use exactly these work keys (`de-anima`, `letters`, `outlines`, `memorabilia`)
and exactly the `loc` strings above (with a hyphen, as written).

- Texts are exact, as printed in that edition (keep its spelling and
  diacritics; normalize only hyphenation at line ends and whitespace).
  Paragraph breaks inside a place as "\n\n".
- `rights.basis` for every witness, with reasoning that names the death years
  and the law, and the licence of the digital copy you took the text from.

Report back in a few lines: which places have which witnesses, every witness
you could not obtain or that must stay out (and why), and every rights decision
that was not obvious.
