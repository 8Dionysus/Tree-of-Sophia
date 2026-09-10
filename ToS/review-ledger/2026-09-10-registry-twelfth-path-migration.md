# Registry twelfth navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **Cicero forensic-speeches Latin preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-twelfth-planting-2026-09-09/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-twelfth-planting-2026-09-09/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/cicero-forensic-speeches-latin-20260909.json` (SHA-256 `16e8692b7f3c5a4a43d18690e0981e7da718330ce803f562f785bbfaa2b4e550`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-twelfth-planting-2026-09-09/execution/plant_cicero_forensic_speeches_latin.py` (SHA-256 `09012d177bde75b008ea8d2e530f7e81de6b612d71c0aebf1e660ed29b75bf75`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-twelfth-planting-2026-09-09/execution/report_cicero_forensic_speeches_latin.py` (SHA-256 `ae9169d594ea7991e4df458bce23293d9e44eb5320dd4c014b3cc6eccd637fba`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-twelfth-planting-2026-09-09/manifest.json` binds SHA-256 `2f530281a86e2ecd8f3e3f53cc6f9543dc799b77e031e3020b2d7e64f3b9565a` and retains historical manifest SHA-256 `1ba1976957c5055cae383eaa5e7de2f867e4c77fa378eba261d2d10e0ea644f8`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
