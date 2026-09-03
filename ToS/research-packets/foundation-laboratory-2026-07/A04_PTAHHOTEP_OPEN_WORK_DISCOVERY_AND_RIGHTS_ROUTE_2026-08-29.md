# A04 — Ptahhotep open-Work discovery and rights route

Date: 2026-08-29
Status: model-made research packet; `not_source_witness`, `not_canon`, `not_doctrine`, `not_legal_advice`
Active discovery receipt: `ToS/source-witnesses/discovery/runs/instruction-of-ptahhotep-open-work-route.2026-08-29.v2.json`
Historical discovery receipt: `ToS/source-witnesses/discovery/runs/instruction-of-ptahhotep-open-work-route.2026-08-29.v1.json`
Artifact receipts: `ToS/source-witnesses/discovery/runs/papyrus-prisse-artifact-route.2026-08-29.v1.json`; `ToS/source-witnesses/discovery/runs/museo-egizio-cgt-54014-artifact-route.2026-08-29.v1.json`

## Question and result

The reviewed open-Work queue selected *Instruction of Ptahhotep* as the first
eligible chronological target after the explicit A01 non-Work exclusion. The
run did not resolve one reusable Work payload. It did resolve a strong physical
witness route, one historical English translation route, several distinct
witnesses, and one exact CC0 image object. With explicit operator authorization,
that exact JPEG was retained as a File-backed visual representation of Museo
Egizio CGT 54014. The candidate terminates as `held_source_witness`; no
Work/Expression/Edition/Item identity, source text, translation, semantic
claim, graph fact, canon state, or publication authority was invented.

## Source-owned findings

- The [BnF Papyrus Prisse root](https://archivesetmanuscrits.bnf.fr/ark:/12148/cc12921q)
  owns the repository description for Egyptien 183-194. It reports a papyrus
  palimpsest, 15 x 701 cm, Twelfth Dynasty, probably from Dra Abu el-Nagga,
  donated by Prisse d'Avennes in 1842. Its hierarchy separates Egyptien 183
  (Kagemni), 184-185 (erased text), and Egyptien 186-194 (Ptahhotep).
- [Egyptien 186](https://archivesetmanuscrits.bnf.fr/ark:/12148/cc12921q/ca103)
  is one exact Ptahhotep unit, lines 1-73, with its own 2022 high-definition
  digitization route. It is not the whole Work and not the physical root object.
- [TLA version L1](https://thesaurus-linguae-aegyptiae.de/text/3W45TQYQ3RCX5DXZ7DIZ43JWNQ)
  supplies persistent scholarly identity and bibliography. Other TLA IDs expose
  distinct witnesses rather than interchangeable copies. TLA also states that
  its content is [not provided under a free license](https://thesaurus-linguae-aegyptiae.de/info/tla-development?lang=fr).
- [Trismegistos collection 270](https://www.trismegistos.org/collection/270)
  corroborates TM 755250 for Paris BnF 186-194 and keeps the Kagemni member
  (TM 755052) separate.
- [Project Gutenberg 30508](https://www.gutenberg.org/ebooks/30508) exposes the
  Battiscombe G. Gunn historical English translation route. Its own
  [license policy](https://www.gutenberg.org/policy/license) says “public domain
  in the USA” and requires users outside the USA to check local law. WorldCat
  separately reports a London, J. Murray, 1908 second impression under
  [OCLC 787861218](https://search.worldcat.org/title/The-instruction-of-Ptah-hotep-and-the-instruction-of-Ke%27gemni-%3A-the-oldest-books-in-the-world./oclc/787861218).

## Rights layers

BnF metadata has a positive [Etat Open Licence reuse route](https://www.bnf.fr/fr/reutiliser-les-donnees-de-la-bnf)
with attribution/date conditions. That does not automatically cover Gallica
scans, embedded ancient text, modern facsimiles, translations, or TLA content.
The exact Prisse image appears as public domain on Wikimedia Commons, but the
same BnF/Gallica representation carries separate reuse conditions; this run
preserves the conflict instead of choosing the most permissive badge.

For Mexico, the current official [Ley Federal del Derecho de Autor](https://www.diputados.gob.mx/LeyesBiblio/pdf/LFDA.pdf)
was checked at Articles 29, 78-79, and 152. They provide relevant term,
translation, public-domain, and moral-rights evidence, but this model pass does
not calculate transitional application to Gunn or issue a legal conclusion.

## Exact open-object acquisition and boundary record

General web search ran last and found a distinct Museo Egizio witness,
[CGT 54014](https://collezioni.museoegizio.it/en-GB/material/CGT_54014/), plus
an exact [CC0 1.0 Commons JPEG](https://commons.wikimedia.org/wiki/File:Papyrus_fragments_with_a_section_of_the_%27Instruction_of_Ptahhotep%27_on_the_recto_-_Museo_Egizio,_Turin_CGT_54014_p01.jpg)
(487,283 bytes; 1252 x 1982; Commons SHA-1
`7360a8cfe500c9377f29415adfb24aa22b0e5672`). This is positive rights evidence
for that image only; it does not make the Prisse scan, Gunn translation, TLA
content, or ancient Work identity equivalent or open.

The retained file is
`ToS/source-witnesses/artifacts/egyptian/deir-el-medina/museo-egizio-cgt-54014/representations/recto-photograph-p01/payload/cgt-54014-p01.jpg`:

- byte size: `487283`;
- SHA-256: `59b39f048b3dd80e74bda835be0bbb7be15987955d151fc5c21ba8ee97205696`;
- Commons SHA-1: `7360a8cfe500c9377f29415adfb24aa22b0e5672`;
- media and dimensions: `image/jpeg`, `1252 x 1982`.

The current Museo Egizio direct full JPEG is byte-distinct. It remains an
originating-provider lead and was not silently substituted for the exact
Commons bytes selected by the discovery record.

The exact storage command used the project target and artifact byte size:

```text
abyss-machine storage write-preflight --kind artifact --bytes 487283 --target /srv/AbyssOS/.worktrees/tos-open-work-loop-20260829/ToS/source-witnesses/artifacts/egyptian/deir-el-medina/museo-egizio-cgt-54014/payload/cgt-54014-p01.jpg --json
```

It returned `deny`: the target matched the protected read-only
`/srv/AbyssOS` project surface owned by `abyss_os_project`. Capacity was not the
blocking factor. That verdict is preserved as a deny for machine-owned host
storage automation; it is not rewritten into an allow. The operator's explicit
instruction authorized this bounded project-owned source acquisition, and the
repository now records the bytes through
`artifact-visual-representation.schema.json`. The host suggestion to use
`/srv/abyss-machine/storage` was not followed because a source witness belongs
to the ToS source tree, not a host cache.

An earlier preflight attempt used the invalid kind `source-witness`; the CLI
rejected it and listed `artifact` as the applicable kind. That failed attempt
is retained here so the next loop uses the typed storage kind directly.

## Authority and next route

The Papyrus Prisse record and A04 planting remain public metadata only. CGT
54014 is an exact physical-artifact metadata node related to the candidate,
discovery, File, visual representation, rights record, and acquisition event;
it has no fabricated philosophy-backlog planting. These relations do not
accept source text, create semantic relations, promote graph facts, form
canon, or establish human/legal acceptance.

The v1 timing zeros remain historical unknown sentinels. The active v2 run has
12 positive `time.perf_counter_ns` HTTP transport measurements in a separate
timing receipt. Those values cover request transport through the first 16 KiB,
not research, interpretation, rights review, or human effort.

The queue has been rebuilt and now names Pyramid Texts as the next eligible
candidate; it has not been executed. A human/legal review is still required
before treating the BnF/Gallica Prisse representation or Gunn translation as
redistributable across FR, MX, and US.
