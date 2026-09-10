# Registry seventh navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **Plutarch/Epictetus English-1874 preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-seventh-planting-2026-09-09/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-seventh-planting-2026-09-09/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/plutarch-epictetus-english-1874-20260909.json` (SHA-256 `156dd2c32a3f4bc266bcd9bc505976045c58f42756686428c604bf62d28e1bb6`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-seventh-planting-2026-09-09/execution/plant_plutarch_epictetus_english_1874.py` (SHA-256 `7d2bd6cdd7fe49716075ac994b6fc4d3a24d209c4af28937bad7306d4589a1b2`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-seventh-planting-2026-09-09/execution/report_plutarch_epictetus_english_1874.py` (SHA-256 `2b08990af2dcf205ec76394f10256c877d4126259a2b90f0f21fe0084b25e9c6`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-seventh-planting-2026-09-09/manifest.json` binds SHA-256 `a552ff93dcb80482cde1b278ae73e7de12ac27a4961912c4d361249b80d1d8a5` and retains historical manifest SHA-256 `5f0b95e6f94c4ccc772769d56eacea7af2b5b3c03ddffb52be8aeacca62cf6f1`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
