# Registry eighth navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **Plutarch Moralia Greek preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-eighth-planting-2026-09-09/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-eighth-planting-2026-09-09/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/plutarch-moralia-greek-20260909.json` (SHA-256 `1e0059f9e5841c522f569e6b66c73850b376d9ae431f720033c97389c81f4cd5`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-eighth-planting-2026-09-09/execution/plant_plutarch_moralia_greek.py` (SHA-256 `4bcc216ce22fcbbbeb5dd547ff6898f812cd1622a31a171b40b4991d4c4a9030`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-eighth-planting-2026-09-09/execution/report_plutarch_moralia_greek.py` (SHA-256 `f49cf9617963108cfe2f6bae686b45b2473a42488ecccc96b8d397da8e0442b5`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-eighth-planting-2026-09-09/manifest.json` binds SHA-256 `c176bc16a9677c4cef16ab3a012ac30f8e46fb0119904a682bb647c1269c93c7` and retains historical manifest SHA-256 `779603a6295102f6bef2106843db72999d490c0786a89ca6d93adad27b58d481`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
