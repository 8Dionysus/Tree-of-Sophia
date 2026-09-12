# First registry planting — 2026-09-08

This source-owned preparation freezes thirteen small source-language versions:
three Westminster Leningrad Codex books with OSHB annotation, two Egyptian
compositions represented by ORAEC, and eight Pāli works in SuttaCentral's
Mahāsaṅgīti-derived root edition. The registry supplies research leads; the
upstream records, exact version, and separate rights assessment supply the
acquisition basis.

`manifest.json` and `prepared-source-packages.jsonl` are preparation material.
They do not create a Work, an Item, local custody, accepted source text, or a
branch planting. Their future source paths are intentional. File SHA-256 and
actual acquisition times are established only from downloaded bytes.

`scripts/prepare_registry_sources.py` captures bounded upstream metadata and license evidence and
rebuilds these preparations. Source-language corpus bodies are excluded from
its fetch routes. The pins remain explicit; a new upstream version requires a
new reviewed preparation.

`scripts/acquire_registry_sources.py verify-preparation --manifest PATH` checks
the prepared package without downloading source payloads.
`scripts/acquire_registry_sources.py acquire --manifest PATH --preparation-receipt PATH` requires the
completed preparation checkpoint identified by the operator's ordered goal.
The receipt binds the manifest SHA-256, repository commit, runtime session,
checkpoint review, and passed checks. It records sequence completion rather
than granting authority. The operator already authorized the ordered work.

Acquisition preserves original bytes in exact Item `payload/` directories.
The Git blob digest, byte size, computed SHA-256, parsing, and target-specific
coverage controls are checked before a source package is installed. Existing
source records are never silently replaced. Metadata, layered rights,
provenance, and forensic observations remain tracked. Native XML and JSON
files are locally openable source formats; no translation or normalized text
is manufactured.

Bibliographic identity is provisional and reports this exact supplied version.
Ancient authorship, textual correctness, translation quality, complete physical
witness coverage, critical reconstruction, semantic admission, and canon
remain outside this operation. Modern provider responsibility is retained in
the version and rights evidence and does not become ancient authorship.

The associated branch preparation is
`ToS/philosophy/source-planting-preparation/cross-branch-source-anchors-20260908.json`.
Its source anchors and remaining controls govern later planting; this directory
does not amend those anchors or mark them planted.

The current preparation and execution routes use source branch, corpus, and language/version identity; the reviewed navigation migration and exact historical evidence are recorded in [the current migration record](path-migration.current.json).
