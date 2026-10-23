# Definition topology selection review — 2026-10-19

Scope: local source topology at baseline `4555e5c00e67fc0a064900080b1c4c68b9bcfd3e`.
The I/C implementation review accepts the 524 proposed canonical homes (160
entities, 168 shared properties, 196 relations) after independent twelve-case
CLI acceptance. Native owner tests passed 21/21 for the selected-source upgrade.
This selection changes navigation and assembly ownership, not semantic content.

`definitions/canonical/`, `definitions/shared/` and `definitions/manifest.json`
are the authored inputs. The two flat registry files in their parent directory
are generated compatibility carriers. Historical `definitions/baseline/` bytes
remain immutable evidence. Full parsed-value equality, including shared fields,
array ordering and all semantic parent edges, was checked at migration. Byte
identity is a separate observation: uniform generated formatting differs from
the historical mixed formatting; both original byte digests remain preserved.

Seven multiple-parent navigation choices are accepted for this migration using
the specific rationale in `definitions/README.md`: Place→Identity;
TemporalAssertion/ProvisionActivity→Assertion; ReceptionProcess/HistoricalForgetting
→HistoricalProcess; RediscoveryEpisode→HistoricalEvent; IntellectualLegacy
→HistoricalState. Every other semantic parent remains in the authored record.
Navigation parentage introduces no semantic edge or accepted interpretation.

The deterministic assembler validates unique homes, references, reachability,
cycles and complete pair generation. `check-compatibility` rejects either missing
file or any byte divergence from the deterministic pair. `check-migration`
separately verifies the initial historical parsed snapshot; future reviewed
semantic evolution is governed by the existing semantic registry transition
route rather than permanently forbidding new definitions.

Rollback selects both historical compatibility files and the old authored-source
selection in one Git revision; the additive baseline and review record remain
available. No production pointer, corpus import, main merge or release follows
from this local source review. Runtime data revisions and stale prepared readers
retain their existing fences and must be rebuilt explicitly by their owners.
