# Registry sixth navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **Roman philosophical English-translation preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-sixth-planting-2026-09-09/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-sixth-planting-2026-09-09/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/roman-philosophical-english-translations-20260909.json` (SHA-256 `b4f7025aaeefc65fa772bca4049fe2eee54c2d9b81e669a26d8c527db9c2051d`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-sixth-planting-2026-09-09/execution/plant_roman_philosophical_english.py` (SHA-256 `cbb6d6d82b40f5975ae39f543364b34b1675e18c44c32a7d44fe507046c6d802`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-sixth-planting-2026-09-09/execution/report_roman_philosophical_english.py` (SHA-256 `a4dbddc3e7e7b88d007d2003d803a3200613abbb7dfb6edeb46b5c3423b90af7`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-sixth-planting-2026-09-09/manifest.json` binds SHA-256 `a746bfde18ddf91dfaab373d761d44132fdf63538646f4658920659929de382a` and retains historical manifest SHA-256 `059cb8f3f289c3f1b65261c2bce826dd28e1fbc16cd8ec6475d8c58a1740b712`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
