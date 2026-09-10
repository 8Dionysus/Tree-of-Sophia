# Registry fourteenth navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation navigation for the early Buddhist SN 1–11 branch in the fourteenth registry planting against parent `b65c9f816f0d66f61a357f6b1bdc7b6f38a778c2`. The two current preparation routes now identify their corpus range and language/version role. The historical engine copy is retained under the route's `legacy/` archive because it records an earlier execution shape rather than a current runner.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-fourteenth-planting-2026-09-09/path-migration.current.json). The archive record retains the exact prior preparation files and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current receipts bind the current manifest bytes for the migration route and point back to the exact historical receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Verification

- Both renamed preparation files retain their exact prior bytes and SHA-256.
- Archived preparation manifests and receipts were read back and their stored SHA-256 values match the retained bytes; original preparation commits and checkpoint review references are unchanged.
- Current manifest, queue, readiness, and metadata-form companions resolve the new routes; current receipts bind the new manifest digests.
- The 542 ignored payloads are present in the delivery worktree and match the two current manifests by byte size and Git blob SHA-1.
- The corpus index, registry coverage, documentation currentness, source foundation, philosophy topology, and focused acquisition/naming tests are the required local validation surfaces for final closeout.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
