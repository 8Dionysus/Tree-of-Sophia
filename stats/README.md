# Tree of Sophia local stats port

This directory exposes statistical questions whose domain meaning belongs to
the authored Tree of Sophia. It uses the shared `aoa-stats` grammar without
moving atlas meaning, source authority, graph review, or canon decisions into
the central stats organ.

## Current reference measurement

| Measurement | Question | Reference value |
| --- | --- | --- |
| `Tree-of-Sophia/table-i-prepared-dossier-route-ratio` | What fraction of current validated Table I atlas rows have an explicit unique prepared-dossier route into an existing philosophy branch? | `48 / 48` at evidence revision `159d57402dcfc79c65af93fae7ce1346798dc392` |

The population is a census of unique rows in the Table I atlas. The numerator
contains unique route-map entries that name a row in that population and an
existing philosophy branch. Table II and III rows, dossier contents, source
witnesses, graph projections, generated audits, and canon do not enter the
ratio. A valid Table I population with no routes is an observed zero; malformed,
empty, duplicate, unsupported, out-of-population, or missing-path input is
unknown.

## Evidence posture

The packet is a public reference snapshot of the owner-controlled atlas and
route map at a named source revision. It is not a live view, and its terminal
progress means only that the declared census was processed.

## Authority

The ratio reports route coverage only. It does not establish dossier quality,
source-witness adequacy, philosophical truth or value, branch maturity, graph
review readiness, canon status, release readiness, runtime state, or what the
tree should grow next.

## Surfaces

- `port.manifest.json` declares the owner-local question and measurement.
- `packets/table-i-prepared-dossier-route-ratio.reference.json` records the
  evidence-linked reference observation.
- the Table I rows and prepared-dossier route map own the counted identities;
- branch homes own their philosophical content and source posture;
- `aoa-stats` owns shared validation and cross-owner composition.
