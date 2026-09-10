# Registry fifth navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **Roman philosophical Latin-edition preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-fifth-planting-2026-09-09/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-fifth-planting-2026-09-09/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/roman-philosophical-latin-editions-20260909.json` (SHA-256 `a76dcc332f7f3f10f52068f75156e9b4da53aa1ee8aff85731488c739607ac2e`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-fifth-planting-2026-09-09/execution/plant_roman_philosophical_latin.py` (SHA-256 `5756e3cb36510b26f60c0a25e610cb679d82fba5f932a20c9ed8b8a8f60e773e`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-fifth-planting-2026-09-09/execution/report_roman_philosophical_latin.py` (SHA-256 `5c8c5148aaaf3ca68a77c2c089fbecfdc17da8c77a728707ec8d99978a0314cc`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-fifth-planting-2026-09-09/manifest.json` binds SHA-256 `ec05400aff73c54b1fe0f8aa3e46311112a71321e82c02a6326c4995a2f6a7b4` and retains historical manifest SHA-256 `a1ef99827f95c5691a387b407d47fed732b3a8739df51c3c9f816f22d62c037b`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
