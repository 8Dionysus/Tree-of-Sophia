# Authority Surface Shapes

Use this reference when source-of-truth ambiguity is wider than ordinary docs.
It is not a checklist. Pick the smallest shape that names one concern, one
authoritative source, weaker companion surfaces, and one verification path.

## How To Use

1. Name the concern readers are confused about.
2. Name every surface that appears to carry authority for that concern.
3. Choose the narrowest authority shape below.
4. State which surface is authoritative and what the others are allowed to do.
5. Verify that future readers can follow the route without hidden local context.

## Shape Set

| Shape | Authoritative For | Weaker Surfaces | Common Failure | Verify |
|---|---|---|---|---|
| Overview to canonical detail | Orientation, start route, public entry, or quick status. | README, MANIFEST, START_HERE, index docs, route cards. | Overview silently becomes the real manual. | Overview names canonical homes and stays short. |
| Source, config, schema, or manifest | Runtime behavior, allowed values, validation shape, registry identity. | Generated docs, examples, reports, compact catalogs, installed copies. | Generated summary overrides source-owned fields. | Source path is named; generated surfaces rebuild from it. |
| Operations or runbook | How to run, deploy, recover, rotate, or inspect a live surface. | README snippets, issue notes, incident history, one-off terminal logs. | Historical incident text becomes current run guidance. | Runbook owns current commands; history links outward. |
| Generated, exported, compact, or installed | Derived transport, routing aid, release artifact, compatibility view. | Catalogs, exports, installed skill copies, compact indexes, SDK reports. | Derived surface is treated as authoring truth. | Builder/source owner is named; no-drift or rebuild check passes. |
| Legacy, provenance, or history | Lineage, receipts, preserved raw material, audit trail. | Active contracts, roadmap, tutorial docs, generated indexes. | Raw old wording becomes active instruction. | Active path links to provenance without copying legacy law into it. |
| Decision, ADR, or review note | Why a choice was made, reviewed gate result, rationale and alternatives. | README, roadmap, changelog, code comment, generated report. | Rationale is hidden in status prose or mistaken for current procedure. | Decision surface is linked where future maintainers need the why. |
| Public entrypoint with project overlay | Portable public meaning plus route to local owner context. | Local runtime paths, ecosystem-only names, downstream mirrors. | Public surface requires private deployment knowledge to make sense. | Public text stands alone and links owner context as extra depth. |
| Sibling or downstream owner context | Which repo, organ, or layer owns meaning outside the local surface. | Local summaries, bridge notes, generated cross-repo indexes. | Nearby local file captures authority that belongs to another owner. | Owner route is explicit; local file stays a pointer or consumer. |
| Status, roadmap, or changelog | Current direction, planned movement, shipped changes, or open follow-up. | README counters, old plans, release reports, checkpoint notes. | Status documents accumulate history that belongs elsewhere. | Current status is short; history and detailed plans have named homes. |

## Compact Authority Pass

| Field | Answer |
|---|---|
| Concern |  |
| Candidate authority surfaces |  |
| Shape selected |  |
| Authoritative source |  |
| Weaker companions |  |
| Consumer/downstream surfaces |  |
| Conflict or stale route |  |
| Verification path |  |

## Verification

- the authoritative surface is named in plain language
- weaker surfaces are allowed to orient, summarize, transport, or preserve
  evidence without becoming source truth
- generated, exported, compact, installed, and runtime receipt surfaces remain
  subordinate to authored sources
- public surfaces remain understandable without hidden deployment knowledge
- sibling or downstream owner context is linked without being swallowed locally
