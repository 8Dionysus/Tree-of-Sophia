# Composite definition selection review — 2026-10-19

Reviewer: T1, the independently assigned temporal implementation/review agent.
This is my source-visible meaning judgment within the delegated review of the
two temporal entity definitions. It is not a human approval receipt or a native
assessment-policy admission record.

## Coverage and exact sources

Inspected source revision: `10796b67b229c4f9630c9e9e08965303589e8e13`.
The current selected inventory is 526 definitions: 162 entity types, 168 shared
properties and 196 relations. Its review coverage is composite:

- The [original topology selection review](2026-10-19-definition-topology-review.md)
  covers the original 524 definitions (160/168/196) and their migration and
  navigation choices. Its 2322 bytes remain unchanged, SHA-256
  `2b748fab6e7fd2bfa61f5fed5463b240f5d9b2348582592ec1d932abcb10b68f`.
  This note does not extend that historical review retroactively.
- This review accepts only the two additions identified below, including their
  authored meanings, candidate lifecycle, Identity placement and declared source
  profiles. It does not reassess the original 524 meanings.

All hashes in this note identify exact file bytes, not canonical JSON record
references or runtime admission evidence. Definition paths below are relative to
`ToS/doctrine/semantic-interchange/definitions/`.

| Definition | Authored path | Bytes | SHA-256 |
| --- | --- | --- | --- |
| `tos.entity.chronology-version` | `canonical/entities/tos.entity.thing/tos.entity.identity/tos.entity.chronology-version.json` | 1593 | `271a4a031baca6715e846cb5f46734a8dc373bbe7ac91cb3941c07ba3dbda2ea` |
| `tos.entity.temporal-crosswalk` | `canonical/entities/tos.entity.thing/tos.entity.identity/tos.entity.temporal-crosswalk.json` | 1550 | `d707f981ef3791fe7b49de7376811faf345aff55b12e9cdd662f7ea36afaa971` |

Both definitions have the sole semantic parent and navigation parent
`tos.entity.identity`, `object_role: identity`, `abstract: false`,
`lifecycle: candidate` and `supersedes_type_id: null`. The unchanged Identity
definition represents a referent that persists while descriptions and assertions
change. The two source-owned objects fit that distinction: a frozen selection
and a mapping proposal have addressable versioned identities, while their
contents make separate assertions about time and evidence. Their placement adds
no historical event identity, equivalence edge or automatic conversion rule.

## Meaning judgment

**ChronologyVersion — accept within the declared candidate source profile.**
The definition explicitly distinguishes a frozen selection of exact Claim
revisions, concrete selectors, source cut and policy definitions from historical
truth, completeness outside that selection, canon and current access. This is
the right boundary for reproducible knowledge selection: preserving selected
bytes does not authenticate every assertion or restore revoked rights. The
record schema's `ExplicitExactMembers` selection and separate system,
attributed-knowledge and publication selectors support that meaning without
collapsing the temporal roles. Neither the label nor Identity parent makes a
selection a universal chronology.

**TemporalCrosswalk — accept within the declared candidate source profile.**
The definition describes an explicitly versioned mapping proposal with axes,
applicability, immutable rules, predecessor and provenance. It expressly keeps
stored source policy distinct from consumer admission and access authority.
That distinction permits inspection and revision of a proposal without treating
storage as permission to convert arbitrary coordinates. The finite mapping
schema supports the declared bounded profile; it does not establish correctness
of individual rows, predecessor identity, applicability or current admission.
Those remain obligations of the exact codec, writer, consumer and source review.

I checked for contrary readings in the definitions, their Identity parent and
their declared schemas: in particular, frozen selection as historical truth,
stored crosswalk as accepted conversion, or navigation as a new semantic edge.
The explicit limitations above exclude those readings. No unresolved source
contradiction in these two definitions prevents their scoped selection. This
judgment remains corrigible and does not promote either lifecycle to stable.

## Profile and schema bindings reviewed

Both profiles declare `profile_version: 1`, reader `corpus-metadata-v1` and
`graph_layer: source-profile`. Each maps to both `source-claims` and
`source-navigation` with its own `source_kind_id` equal to its `record_type`.

| Field | ChronologyVersion | TemporalCrosswalk |
| --- | --- | --- |
| `record_type` / `source_kind_id` | `chronology-version` | `temporal-crosswalk` |
| `id_prefix` | `tos.chronology-version.` | `tos.temporal-crosswalk.` |
| `source_basename` | `chronology-version.json` | `temporal-crosswalk.json` |
| `catalog_filename` | `chronology-versions.jsonl` | `temporal-crosswalks.jsonl` |
| `schema_version` | `tos_chronology_version_record_v1` | `tos_temporal_crosswalk_record_v1` |
| `owner_ref` and `schema_ref` | `ToS/contracts/chronology-version-record.schema.json` | `ToS/contracts/temporal-crosswalk-record.schema.json` |

The chronology schema is 6384 bytes, SHA-256
`4918c0775d3aca4ceb7c11dc974d1c448f7d2ae6dbb5e588642f6d8d4800b975`.
The crosswalk schema is 4976 bytes, SHA-256
`1568c0346d12e9074a487283a6c2d5dffc3d06b8887bf62081877f21d7a0a5f5`.
Both profiles explicitly depend on
`ToS/contracts/corpus-record.schema.json` and
`ToS/contracts/source-metadata-record.schema.json`. Schema acceptance establishes
structure within those contracts, not source truth or permission.

## Selection and limits

The manifest's `selection_review_ref` may point to this composite coverage note.
Current authority for meaning remains with the exact authored definitions and
their owning doctrine/contracts; generated registries remain compatibility
carriers. No definition, schema, parent, baseline or registry bytes change in
this review closure.

The mechanical transition's `semantic_acceptance: false` remains correct and
unchanged: that receipt does not perform this meaning review. Existing topology
tests, capability gates and compatibility results likewise do not supply it.
This note records no new compilation, test, runtime or CI result. It does not
approve source instances, general calendar conversion, production grants,
rights, publication, canon or merge. Those retain their actual owner routes and
current checks independently of this bounded source judgment.
