# Registry tenth navigation path migration — 2026-09-10

## Scope and owner review

Reviewed the current preparation and execution navigation for **Plutarch Lives Greek preparation and execution** against parent `d5c034871722b041f9966d5857f12703994ed23e`. Current route names identify the source branch or corpus together with the language or version role; ordinal sequence labels remain historical provenance.

The source owner records this as a navigation migration in [the current migration record](../source-witnesses/discovery/registry-tenth-planting-2026-09-09/path-migration.current.json). The archive record at `ToS/source-witnesses/discovery/registry-tenth-planting-2026-09-09/legacy/path-migration.json` retains the exact prior preparation inputs, execution bytes and checkpoint receipts, including their original manifest digests, preparation commits and review references. No source record ID, Work/Expression/Edition/Item relation, payload byte, rights assessment, translation, semantic claim, publication state or canon state was rewritten.

The current migration record carries only active routes and stable bindings. Historical route names and raw bytes remain behind the excluded `legacy/` archive boundary. Current receipts bind the current manifest bytes and point back to the exact retained receipts. Their role is current navigation migration; they are not acquisition-preparation receipts. The acquisition owner's current-commit check therefore remains fail-closed for any later new transfer unless a fresh preparation checkpoint is produced by that owner.

## Current routes

- `source-preparation`: `ToS/philosophy/source-planting-preparation/plutarch-lives-greek-20260909.json` (SHA-256 `4674256ee9f9587fd3180d1e7d72a5296ef31fc9ffad2d23829b959e3d748f93`).
- `planting-execution`: `ToS/source-witnesses/discovery/registry-tenth-planting-2026-09-09/execution/plant_plutarch_lives_greek.py` (SHA-256 `78cf819ba389e28600dea34ee09877ad7f34a02678f25da855cd003a21b4fa6f`).
- `report-execution`: `ToS/source-witnesses/discovery/registry-tenth-planting-2026-09-09/execution/report_plutarch_lives_greek.py` (SHA-256 `f1dc1a48917b7a64128a1c240632d90c491173de4df5c41837101ecc66ec502d`).

## Verification

- Renamed preparation inputs retain their exact prior bytes and SHA-256 values; archived execution bytes retain their original digests, while current report scope prose follows the active route names.
- Archived preparation manifests, receipts and raw execution bytes were read back; stored SHA-256 and byte-size values match the retained archive.
- Current manifest, queue, readiness and metadata-form companions resolve the active routes; current receipts bind the current manifest digests.
- `ToS/source-witnesses/discovery/registry-tenth-planting-2026-09-09/manifest.json` binds SHA-256 `28e3a3acd6b938f2b5953fd05b85e71cd33a7933196c2217c2cc7d627cebf1b4` and retains historical manifest SHA-256 `9be466b5d0f44d9b1a3d2525b3486b7f7f70a9a7cb03ce0b0c91f7e14631e419`.
- Stable source identifiers, payload bytes, provenance and rights boundaries remain unchanged; this review records navigation only.

These are local mechanical and source-navigation checks. CI, merge, deployment, publication, semantic acceptance, translation-quality review and canon remain separate owner states.
