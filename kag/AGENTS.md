# AGENTS.md

## Applies to

This card applies to `kag/` and every nested path.

## Role

`kag/` owns the template and explicit publication route for independent
Tree-of-Sophia KAG releases. Published provider homes live outside the software
checkout. Authored ToS meaning remains in `ToS/`; bounded portable records give
consumers exact handles back to that source.

## Operating Card

| Field | Route |
| --- | --- |
| input | exact accepted corpus revision and bounded source-return export |
| output | immutable external provider, source-return manifest and downstream status |
| owner | `kag/AGENTS.md`, `kag/README.md`, `kag/provider-template.json` |
| next route | explicit export -> selected aoa-kag producer and readers -> local integration release |
| validation | local provider and export checks for the selected KAG artifact; independent of software merge |

TOS-D-0062 supersedes the universal source-currentness merge obligation in
TOS-D-0044. Build KAG from an explicitly selected immutable ToS source/data
revision, record that revision and the provider revision, and validate source
refs, hashes, shards and parity before publishing that KAG artifact. A stale
integration remains visibly stale; it does not become current because software
CI passed. No regeneration is required for an unrelated software PR.

## Source Routes

- `ToS/derived-exports/kag_export.min.json`
- `ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json`
- `ToS/derived-exports/README.md`
- `mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md`

## Validation

Use [`kag/VALIDATION.md`](VALIDATION.md) for the selected integration.
`local_kag_provider` blocks publication of an invalid or falsely current KAG
artifact, not standalone software merge or release. Source exports remain
owned by their source builders and review routes.

## Closeout

Report corpus/export/integration identities, source-return verification and
consumer validation. Keep current software, selected corpus, successful KAG
publication and actual consuming runtime as separate states.
