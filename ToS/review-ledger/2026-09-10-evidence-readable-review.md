# Evidence reference navigation review

## Scope and source return

Reviewed against base `aa586ebc47761625052c49377d65539aed58bd27` the Evidence
carrier emitted by `scripts/source_witness_bibliographic_graph_common.py` and
its public bibliographic-graph schema. The observed Freedom source-reading
Evidence lacked a source title and therefore normalized to an opaque hash.

The owner builder now supplies a bounded readable display using catalog names,
exact selected public review/research headings, repository-filename fallbacks,
source-file slots, or declared citation addresses. The existing access normalizer preserves this
display without a new access or UI heuristic. The exact source-reading example
remains `ToS/review-ledger/2026-09-07-jgb-freedom-source-reading.md`; its identity,
source path and file digest remain unchanged.

The Freedom reference now carries the authored H1
`JGB 19 and 21: source-visible research reading, 2026-09-07`, not its filename.
The two real Jenseits 1886 historical-episode and Letter 705 research notes
likewise retain their exact original-language H1 and research-lead provenance.
Only immediate Markdown children of `ToS/review-ledger/` and
`ToS/research-packets/foundation-laboratory-2026-07/` selected by the existing
verified public Claim catalog are eligible. The citing Claim's exact digest,
reference and public visibility must agree with the existing catalog entry.
No new membership registry is introduced. Git tracking belongs to source
authoring checks, not runtime rebuild: a source snapshot without `.git` retains
the same result from the same public catalog and protected metadata bytes. This is a
public metadata navigation scope, not assessment or source-content admission.
The existing protected owner reader rejects symlinks at every component and
non-regular/unprotected files, bounds the record to 1 MiB, and detects changes
and replacement. The title and node file digest come from the same read. Only
a UTF-8 first-line H1 of at most 4096 bytes and 240 characters is selected;
control-bearing, invalid, absent or oversized headings retain a missing-title
fallback. No recursive discovery, body synopsis or heading search occurs.

Repository filenames and source slots are honest navigation fallbacks, not
authored document titles. Descriptions explain the reference's role and source
return, not what its content establishes. The display explicitly declares no
HumanForm authority and no supplied source summary. A catalog label is copied
without translation when it fits the bound; an overlong label falls back
without claiming a truncated source title. External citation descriptions put
the unobserved-content limit before the potentially truncated address.

## Boundary review and validation

The review checklist's source traceability, authored-versus-derived distinction,
node layering, stable identity, language authority, and ToS/access ownership
remain satisfied in this bounded change. No source assertion, semantic reading,
assessment, translation, rights decision, publication grant or canon changed.
No arbitrary document-heading extraction, source payload read or remote fetch
was introduced. Existing file fixity reads outside the two selected metadata
families remain existing behavior, not a broadened source-reading permission.

Focused Evidence/external-citation checks passed: nine tests plus 35 subtests.
The source-home validator and `git diff --check` also passed. The tests
cover the real Freedom reference through normalization, preserved source hashes
and IDs, metadata-only anchor/event slots, original-language catalog names,
display bounds, exact per-Claim external citation identity and existing unsafe
address/context refusals. Additional tests cover the two real research notes,
uncatalogued, nested, private/payload and escaped path refusal, symlink refusal,
invalid/overlong/missing H1, metadata byte limits and changed snapshots.

This is a source-owner navigation correction, not full HumanForm completion.
The master integration owner must regenerate the bibliographic projection and
dependent companions in the combined tree, validate that union, and repeat the
real consumer inspection. Full corpus parity, release/KAG regeneration, CI,
merge, deployment and whole-foundation acceptance are not claimed here.
