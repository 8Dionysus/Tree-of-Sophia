# Registry second navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **classical Athenian Plato-works preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-second-planting-2026-09-08/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-second-planting-2026-09-08/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/classical-athenian-plato-works-20260908.json` (SHA-256 `507314ee8996c014d71b1c1be40c2d6405970f5b11b8403730a1b334414e3fa4`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-second-planting-2026-09-08/execution/plant_classical_athenian_plato.py` (SHA-256 `eafcadd23aa8e2c2d48ae301d9e9f4cfd6b5841231dc0df8893a84f2ca770a91`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-second-planting-2026-09-08/execution/report_classical_athenian_plato.py` (SHA-256 `19b27657d3b9808948b227eb1dc31eef3ad06677e0eda23feec56c7aba5af9a7`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-second-planting-2026-09-08/manifest.json` binds SHA-256 `614eac954e725643c1fde35b15a10f4c18242bf802b64594780ee2527507ac09` and retains historical manifest SHA-256 `dc4658e96a632039a6c8560515dade38642838e7e958bda92c5c278bb9b8b6be`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
