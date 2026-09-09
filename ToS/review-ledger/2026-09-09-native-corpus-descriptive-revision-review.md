# Native Corpus descriptive correction — 2026-09-09

## Owner and scope

The explicit `tos_local_corpus_revision_owner_v3` route extends selected-file
descriptive correction to Edition, Collection and Item. Agent, Place,
Organization, Work and Expression remain supported. Older v1/v2 grants are
unchanged; discovery names the difference. Only `preferred_label`, `notes`,
`field_languages` and `source_refs` belong to this route. Structural links,
Collection membership, Item custody/manifest, rights, payloads and identity
status do not. This is not Collection creation or admission of its members.

The existing recoverable selected-file publication protocol retains exact old
source/form/history bytes, expected versions and dependencies, idempotent
commands and explicit recovery. The current metadata-version reader now also
resolves native Collection history from the owned catalog. No descendant
enumeration, Item manifest read or payload transformation is required by this
descriptive correction.

## Actual source judgment

The complete current `collection.json` for
`tos.collection.friedrich-nietzsche.works-in-two-volumes-volume-2-mysl-1996`
was inspected at version 3, canonical digest
`sha256:3c54b5e74076dee7bae4bb061798e56e5186960f37cc8ec6a3e8e8d4dea2dc9b`.
Its preferred label is Russian written in Cyrillic; the complete notes are
English written in Latin script. Neither field has an explicit language
declaration, and both adjacent source-copy forms currently have null
language/script. This limited agent judgment concerns the visible wording,
not the historical truth of the bibliographic statements.

The delegated correction declares `preferred_label: ru/Cyrl` and
`notes: en/Latn`, preserves both strings exactly and rebinds both existing
form IDs. It preserves all seven membership refs, ISBN and NCID statements,
source refs, `verified` historical identity posture, and
`no_equivalence_claim`. Those prior declarations are not newly verified or
silently re-admitted by this language correction. No translation or license
decision is made, and this correction command reads no Item or payload.

The original version-3 record, original version-1 forms and their September 7
growth-history receipt must remain returnable. Source-only form readiness
continues to carry `admission: null`; it is not substantive acceptance.

## Validation status

Before executing the real correction, selected revisions, source revision
compatibility and discovery passed 59 tests plus 114 subtests on stable
implementation files. An earlier concurrent-edit run had one dependency
snapshot conflict; the unchanged test passed on the frozen rerun. This does
not establish a measured race diagnosis or a source-content error.

## Actual command and return

The ordinary source command applied this correction at 20:25 UTC. It created
Collection version 4, canonical digest
`sha256:f14299bd238043c88f48b33e7116a12724d0e1285d5bb0861c69c78e1e3ace80`,
and version 2 of each unchanged form ID. Only `field_languages` and the record
version changed. Both original strings, all membership refs, identifiers,
source refs and historical status fields were compared unchanged.

The exact CLI repeat returned `replayed: true`, the same receipt and revision,
without a second transition. `inspect-version` returned the version-3 source
and exact archived companion. Both original forms and their full growth
history were compared with the retained predecessor and remain present.
The existing catalog builder regenerated `collections.jsonl` and the selected
publication manifest; the independent `MetadataVersionReader` then returned
the old ref as `available/historical` and new ref as `available/current`.
Its exact-ref enumeration and final currentness check passed. Both current
forms remain source-only `ready` with `admission: null` and explicit ru/Cyrl
or en/Latn provenance.

The apply/repeat processes took 0.61/0.29 seconds respectively; peak process
RSS was approximately 39/38 MiB. These are single small-source command
observations, not cold-cache, p95 or corpus-scale measurements. The source
foundation validator passed after catalog regeneration; its separate normal
optional-byte fixity checks are not the correction command's read footprint.
CI, merge and publication remain separate from this local source action.
