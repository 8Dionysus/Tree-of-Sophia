# TOS-D-0083 Research Registers: Memory, Research Layer and Claims

## Index Metadata

- Decision ID: TOS-D-0083
- Original date: 2026-11-20
- Surface classes: bonsai, docs/route-law
- ToS layers: research packets, derived projection
- Tree classes: none
- Guard families: source-first authority, review separation, bounded downstream seams
- Posture: accepted by explicit operator instruction

## Context

The Tree's research registers are workbooks made by separate research passes,
batch by batch, outside the repository. They propose: identities, years, works,
places, communities, events and references, each with the sources a row cites,
its compiler's status and its gaps. They were not made to be reliable enough to
be carried into the Tree as they stand, and more of them will come, differing
in content and in form.

What the operator's mirror holds now (`MANIFEST_SHA256.csv` pins 1,200 files):

| Family | Workbooks | Sheet sets | Core sheets (in ≥90%) | Sheets in two workbooks or fewer |
| --- | --- | --- | --- | --- |
| Persons and works (Identity) | 191 | 22 | agents, works, related_objects, gaps | 844 |
| Places and communities (Context) | 185 | 15 | entities, relations, sources, gaps | 153 |
| Events | 55 | 1 | events, relations, entities, sources, gaps | 0 |
| References (Corpus and Rights) | 190 | 8 | registry, gaps | 67 |

Beside them lie three master tables (one row per research node: 190 nodes with
macro-region, windows of formation, fixation and canonization, languages and
centres) and the Deep Research basis. `VERSION_HISTORY.json` already keeps the
prior values of 83 T3 reference registers with crosswalks from old record IDs
to current ones.

How the repository uses registers today:

- The Deep Research philosophy packet lives in `ToS/research-packets/` as a
  non-authoritative scaffold, never a source witness (TOS-D-0006).
- The Aesthetics registers are kept whole in git under
  `bonsai/review/aesthetics/registry/` (4.2 MB, public repository); a reading
  enters the field only after a reader's verdict (TOS-D-0080).
- The Population keeps the Identity rows it grows from whole under
  `bonsai/review/population/registry/` and a parsed layer beside them, both in
  R2 (`layers.json`), published to the site's copy on merge (107 MB); its
  members show the registers' words at low footing, and the reading line's
  decisions with their sources (TOS-D-0082).

The question raised by the operator: keep registers as memory and source,
process them and keep the raw material hidden, or use them directly.

## Decision

Every register passes through three tiers, and only the third speaks for the
bonsai.

1. **Memory.** A workbook is kept unchanged, pinned by SHA-256, with the
   mirror's version history; a correction arrives as a new version, never as an
   edit. Memory lives in the operator's mirror and archive. Git holds a
   workbook's path, size, digest and form, as `provenance.json` does; the
   workbooks themselves are not published.
2. **Research layer.** A family's core sheets are its contract. An adapter per
   family and per version of its form reads them into two separate, versioned
   layers: the rows verbatim and a parsed layer. An adapter refuses a form it
   does not know instead of bending it; auxiliary sheets stay in memory until an
   adapter is written for them. A research layer is a research packet in the
   sense of TOS-D-0006: never a source witness, never canon.
3. **Claims.** What a bonsai takes from a register carries the register's
   words, the row's ID, the compiler's status and the sources the row cites, at
   low footing. A reading line checks it against sources and records a decision
   with its own source, at medium footing. Only a competent reader's verdict
   admits it to the Tree (`ToS/doctrine/KNOWLEDGE_ASSESSMENT.md`).

**Publication.** The research layer a bonsai grows from is published beside
the bonsai with the research-packet posture: the rows its claims rest on and
their parsed layer. The workbooks and their other sheets stay in memory. No
surface shows a register's value as a fact: a claim stands with its words, its
row and its footing, and the verdict stays `undecided` until a reader decides.

## Options Considered

- **A. Publish the research layer a bonsai grows from** (chosen). The rows
  a bonsai's claims rest on, and their parsed layer, are published beside the
  bonsai with the research-packet posture; the workbooks and all other sheets
  stay in memory. This is what Aesthetics (in git) and Population (in R2) do
  now, made one rule. A reader can open the whole row behind a claim, and CI can
  rebuild the bonsai from public inputs.
- **B. Publish claims only.** The research layer stays in memory with the
  workbooks; the public sees each claim with the register's words, row ID and
  cited sources, but not the rows. Raw material cannot be mistaken for data,
  and the public surface does not grow with every register. The cost: no one
  outside can check a claim against its whole row, CI needs private access to
  verify a bonsai, and the Aesthetics rows already in git history stay public
  regardless.
- **C. Publish whole workbooks.** All sheets of every family, in their varying
  forms. Most transparent, but the public surface takes on every form's drift,
  and auxiliary sheets (audits, change logs, crosswalks) read as content.
- **D. Memory only until read.** Nothing from a register is shown before a
  reading line has checked it. Safest, but the skeletons empty: the Population
  would show no names or years until the line reads them.

## Rationale

The registers' weakness is reliability, not secrecy: the fault to prevent is a
register value standing in the Tree, or on a stand, as if it were established.
The three tiers address that by authority, as TOS-D-0006 did for research
packets: memory keeps the record exact and recoverable, the research layer keeps
it legible across changing forms, and only claims, labelled with their words and
footing, face the reader. Option A keeps what is already public consistent with
what will be published, lets anyone trace a claim to its row, and lets CI verify
a bonsai without private credentials. Option B would have kept the raw research
out of public view, at the cost of that tracing, while the Aesthetics rows stay
public in git history in any case.

## Consequences

- **Population (PR #500).** Its `registry/` and `layer/` are declared a
  research packet; they are published to R2 and the site's copy on merge as
  before, and the stand links each batch's rows.
- **Aesthetics.** Already within the rule: its rows in git are its research
  layer.
- **Master tables.** They are registers of the research itself. A bonsai may
  publish their derived fields (node title, normalized macro-region, windows of
  time) as the structure of the research, not as claims about people; languages
  and centres wait for the places ring.
- **New families** (Context, Events, References) enter as memory, get an adapter
  when a bonsai needs them, and publish the research layer that bonsai grows
  from.
- **Not to infer:** a register's `verified` is its compiler's statement; a
  green adapter check proves only that a form was read as declared; a
  published research layer is no endorsement.

## Source Surfaces

- `bonsai/review/population/intake.py`, `parse.py`, `provenance.json`, `layers.json`
- `bonsai/review/aesthetics/`, `bonsai/CORE.md` ("Growth")
- `ToS/research-packets/AGENTS.md`
- `docs/decisions/TOS-D-0006-research-packet-authority-correction.md`
- `docs/decisions/TOS-D-0080-aesthetics-bonsai-and-the-core-of-a-field.md`
- `docs/decisions/TOS-D-0082-population-bonsai-and-the-core-of-a-population.md`

## Validation

Review the decision. Each family's adapter is checked by its own `--check`
(`intake.py`, `parse.py` for the Identity registers); `node bonsai/lib/layers.mjs
status` checks that a published research layer matches its manifest.
