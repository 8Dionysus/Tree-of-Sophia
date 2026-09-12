# Registry third navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **ancient Mediterranean source-edition preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-third-planting-2026-09-08/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-third-planting-2026-09-08/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/ancient-mediterranean-source-editions-20260908.json` (SHA-256 `f97d477b234e9f9dffeddd7309713d48173ecac9557b3c55b548e01582905c92`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-third-planting-2026-09-08/execution/plant_ancient_mediterranean_sources.py` (SHA-256 `690d430ea218ea6366acba0259efd24b7b2b9107ae2093a767c1834bec70c2fd`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-third-planting-2026-09-08/execution/report_ancient_mediterranean_sources.py` (SHA-256 `5512081b989fbd7a0dbe32bbae1fed2161e053a3ca8ccac5523896666e04944c`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-third-planting-2026-09-08/manifest.json` binds SHA-256 `5a7211ab7646d32ad64aec83eebcd19c0234af22a340ff12b2900dc8e990b173` and retains historical manifest SHA-256 `108555bc5f6bfb1c11062def9fef4b8702d7779c7c294377d0001e637e9b35f8`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
