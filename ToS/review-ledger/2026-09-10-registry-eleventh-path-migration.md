# Registry eleventh navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **Plutarch Lives English preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-eleventh-planting-2026-09-09/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-eleventh-planting-2026-09-09/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/plutarch-lives-english-20260909.json` (SHA-256 `8ebea5d499e4914b472b9effe162918451fc154a2dfeec0ede857c09fb817651`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-eleventh-planting-2026-09-09/execution/plant_plutarch_lives_english.py` (SHA-256 `4e673f20ca65ab883ef2263f223bc667084042a13bb624e2b6e0de2ffce9f4f8`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-eleventh-planting-2026-09-09/execution/report_plutarch_lives_english.py` (SHA-256 `0f99524ed5923f6fe2f58d6ebd4608d3b647b45c818e43f9f33e9dbe1d09b561`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-eleventh-planting-2026-09-09/manifest.json` binds SHA-256 `e0633d8c150fdad904fc6e52e8b36d183fb595ba323b0d8e0d448fde6e960214` and retains historical manifest SHA-256 `96f07e1fd54e9d69d595b79d5e47b71c84d1dc3b4cdecf26317559717d6d6758`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
