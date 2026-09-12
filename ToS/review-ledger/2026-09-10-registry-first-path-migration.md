# Registry first navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **cross-branch source-anchor preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-first-planting-2026-09-08/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/cross-branch-source-anchors-20260908.json` (SHA-256 `fe63957563adaed179b5a1d1764d82d467e7c67f56ffb0d007d44c9bc2b44155`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/execution/plant_cross_branch_sources.py` (SHA-256 `6fb48ae7166075cf194d9a3b37700d8a095567a6ea9d69c9e9c137686c6e7794`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/execution/report_cross_branch_sources.py` (SHA-256 `4dc5c6a169a19b06f1ed08bdce1f441a5d02a991b9576a1c95a0e1e08901d211`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/manifest.json` binds SHA-256 `c472fae47ceabe45c28cc80915ac69359beb4e801ae0f492ca87755588a273dc` and retains historical manifest SHA-256 `da1bdc754c3bb47ee8be84c0789550ca6657cadd62e9019e1a9fe0ad83a32c74`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
