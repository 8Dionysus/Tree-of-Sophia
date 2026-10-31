# The fragments pass: records and tools

The stands stopped retelling the works they read (PR #303). This folder keeps
what that pass rests on: the agents' briefs, their records, and the tools that
turned the records into the stands' modules. These are review events, not
derived data. The modules they produced live in `bonsai/zarathustra/`,
`bonsai/antiquity/` and `access/web/constructor/`.

## Records

| Folder | What it holds |
| --- | --- |
| `frag/` | Zarathustra. Every authored sentence classed against the German first editions and the other originals: retelling, navigation, interpretation, link, question, method, context, term. Each sentence names the places it rests on (`out/*.jsonl`); `BRIEF.md` is the instruction. |
| `align/` | The Zarathustra anchors converted to DTA witness units (`anchors.dta.json`). |
| `rewrite/` | Zarathustra. The mechanical rewrite and merge, the edit pass on the own words that remained (`edit-out/`, `EDIT-BRIEF.md`) and the links between places (`links.json`). `original.json` is the content before the pass. |
| `anti/` | Antiquity. The same steps: classification (`out/`, `BRIEF.md`), edit pass (`edit-out/`, `EDIT-BRIEF.md`) and links (`rewrite/links.json`). |
| `ext/`, `ext2/` | The places of other works, as gathered and read against the page images (`external-fragments.json`, `antiquity-fragments.json`). The builders turn them into each stand's `fragments.json`. |
| `verify/` | An independent check of a seeded sample (`report.json`). It led to citing whole sentences of the original (`snap_cites.mjs`). |
| `catalogs/` | The Spanish, German and Chinese translations of the new wording, by round (`translated/`, `translated2/`). |

A record's paths name places in the stands' modules (`collection.passages[12].body`,
`NODE_CONTENT.demo:gateway.body`) as they stood when the record was made.

## Tools and their order

Each tool finds this folder and the repository root by itself; `TOS_REVIEW` and
`TOS_REPO` override them. The steps for each stand are:

1. **Classify** (agents with the brief).
2. **Rewrite**: `rewrite.py`, `rewrite_antiquity.py`. A retold sentence gives way to the places it retold, and the own words stay as they were written.
3. **Edit** (agents, checked by `check_edits.py`). The own words are made to read right after their fragments.
4. **Merge**: `merge.py`, `merge_antiquity.py`, then `write_modules.mjs` for Zarathustra.
5. **Type the objects**: `objects.py`, `objects_antiquity.py`.
6. **Gather places of other works and build them**: `zarathustra_fragments.py`, `antiquity_fragments.py`, `whole_sentences.py`.
7. **Cite whole sentences**: `snap_cites.mjs`, then `--objects`.
8. **Align**: `node bonsai/lib/cli.mjs align-corpus <stand> --write`.
9. **Catalogs**: `catalogs.py plan2|apply2`.

`frag/zc.py` reads the public corpus for the agents. The downloads the gatherers
read (scans, OCR, TEI files) are not kept here. Their URLs are in each witness of
`fragments.json`, and a tool that needs one names the folder it expects.
