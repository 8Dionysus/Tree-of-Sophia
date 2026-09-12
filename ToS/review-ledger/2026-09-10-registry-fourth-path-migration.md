# Registry fourth navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **ancient Mediterranean English-translation preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-fourth-planting-2026-09-09/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-fourth-planting-2026-09-09/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/ancient-mediterranean-english-translations-20260909.json` (SHA-256 `270a689202f79c621abbe51760413766e7da43a22bde19f04580af780e95203d`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-fourth-planting-2026-09-09/execution/plant_ancient_mediterranean_translations.py` (SHA-256 `8311a285a19d2aeb596809b27c33a56397c25b27aea573f8544cb285c71be8fd`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-fourth-planting-2026-09-09/execution/report_ancient_mediterranean_translations.py` (SHA-256 `af41b5fe21c0c2d7c49d64714e79c8a9f25859ffc6b8191f49dec54db1dd9c01`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-fourth-planting-2026-09-09/manifest.json` binds SHA-256 `4ab10dedf97a1ecfcb4505c70bf0e557383736288cf53137b8a2c8fb3dde2d52` and retains historical manifest SHA-256 `3784d04765816700e1c3c0da8e8c3c0bce5a935aec71ef7b402930fd1f351c23`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
