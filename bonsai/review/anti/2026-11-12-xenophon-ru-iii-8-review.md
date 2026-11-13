# Xenophon, Memorabilia III.8.4–7: Russian witness review

The four Russian passages added to the stand are an independent transcription
of G. A. Yanchevetsky's translation in the second edition, Mitau, 1879,
pp. 89–90, from the [Presidential Library's copy](https://www.prlib.ru/item/1091561).
The complete scanned book is retained locally for research; this change does
not redistribute its images or claim that they are licensed for reuse.
Yanchevetsky died in 1903 and the translation was published in 1879: its
wording is public domain under the stated Russian, US and EU term bases.

Source-visible review compared every added passage against the two page images,
including punctuation and pre-reform orthography. Page-break and line-wrap
hyphenation are joined; margin numbers are omitted. Image SHA-256 values:

- printed p. 89, image 97: `92ab89e13ecc09b3267c947544bd63647b3139ab5b5442cbfd32f34ec3e89f28`;
- printed p. 90, image 98: `62718685fab30cd439f06de2aea4d8cfebef6c1712176b49b0826deca0983a19`.

The Greek text already held in `fragments.json` and the four transcribed
sections were supplied as separately numbered units to `alignWitnesses` in
`bonsai/lib/align.mjs`. The bounded alignment returned one-to-one unit groups
for III.8.4, III.8.5, III.8.6 and III.8.7. Each resulting whole passage was
also compared with the original for the named section; this is a review of
these four sections, not an alignment or review of the complete book. The
sentence aligner does not separately pair the initial Russian sentence in
III.8.7; it corresponds to the opening of the Greek sentence and is retained
in the complete section. The candidate transcription and alignment receipts
are retained locally with SHA-256 respectively
`b1effe2fc196cc9563aaee82f95dac174dbbbe021c17fa802a05b23f7f60c2d2` and
`af7d7c97518a5ca695c2e421bacb90b914658becf3f5ff40812a4d2d424b7105`.

The 1879 text in III.8.5 has the apparent typographical error «въ отношеніи
въ одному»; it is retained as printed. In III.8.6 the Greek and Russian have
a golden shield, but Dakyns's English has “spear of gold”, as verified in
his [1897 printed edition](https://archive.org/details/WorksOfXenophonV.3Pt.1),
p. 107. Neither historical translation is silently corrected to the other.

This is a bounded stand witness, not a source-owner admission, a reviewed
complete Russian text, or a claim of complete coverage. Other Russian places
remain unavailable until separately located, transcribed and reviewed.
