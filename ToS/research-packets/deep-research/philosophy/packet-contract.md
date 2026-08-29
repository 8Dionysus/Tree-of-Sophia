# Philosophy Research Packet Contract

## Role

This contract defines the route for current and future philosophy research packets.

A packet may carry capture metadata, table structure, dossier structure,
source leads, unresolved questions, and extraction notes. It does not carry
source-witness authority, canon authority, doctrine authority, or final graph
authority.

## Authority Posture

| Field | Value |
| --- | --- |
| source status | `not_source_witness` |
| canon status | `not_canon` |
| doctrine status | `not_doctrine` |
| graph status | pre-canon scaffold |
| next route | branch review, source anchoring, graph workbench, canon review |

Research packet claims become ToS material only after they are reviewed inside
the philosophy branch and anchored to source witnesses, editions,
translations, published scholarship, or reviewed canon surfaces.

## Packet Shape

Future packets may be split into:

| Surface | Use |
| --- | --- |
| `master-tables/` | table-level dispatch scaffolds, launch order, status fields, and row-shape notes |
| `dossiers/` | per-topic or per-branch research dossiers awaiting branch review |
| `dossiers/table-i-docx-intake.manifest.json` | fixity and capture-posture record for operator-local, untracked Table I DOCX bytes |
| `dossiers/table-i-docx-extraction-coverage.json` | explicit accounting of extracted, metadata-only, and deferred table rows |
| `dossiers/table-ii-docx-intake.manifest.json` | exact fixity, admission, and quarantine record for the supplied partial Table II packet |
| `dossiers/table-ii-docx-extraction-coverage.json` | explicit accounting of extracted, metadata-only, deferred, and identity-quarantined Table II rows |
| `pages/` | capture metadata from external workspaces or UI containers |
| `packet-contract.md` | this route contract |
| `extraction-map.md` | the route from packet fields into future branch and graph objects |

These surfaces are allowed to describe structure. Prepared corpus data moves
only through an explicit intake pass. A tracked digest or coverage record does
not turn the local DOCX bytes or their claims into source witnesses.

Partial packages preserve three separate states: admitted master-aligned
artifacts, supplied artifacts quarantined for an exact identity conflict, and
master rows for which no input was supplied. A filename never overrides a
conflicting master identity, and a quarantined artifact emits no semantic or
branch projection.

## Boundary

Capture containers, generated summaries, spreadsheet rows, DOCX files, and AI
research outputs stay below source authority. They can suggest branch leads;
they cannot name a source witness, work, school, doctrine, concept judgment, or
canonical edge by themselves.
