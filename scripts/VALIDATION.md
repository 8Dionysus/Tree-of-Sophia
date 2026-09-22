# Script validation

Choose the changed behavior and the operation that owns it. Named command
sequences live in `docs/validation/validation_lanes.json`; a sequence is not an
obligation for unrelated changes.

## Software

After installing Python and browser dependencies, run:

```sh
python scripts/release_check.py
```

This checks software contracts, builds browser assets, and runs program tests
on bounded fixtures. It needs no production corpus, KAG, stats, sibling
checkout, generated-currentness refresh or R2 credentials. Browser behavior
is checked by `software_browser` after the build; Worker tests use their own
software route. See [root validation](../VALIDATION.md) and the exact
[release procedure](../docs/RELEASING.md).

For a focused change, run the relevant test directly. The old
`release_check.py --feedback --changed-path ...` interface is removed; there
is no automatic fallback from an unknown path into a whole-corpus audit.

For one explicitly reserved admission or build measurement, set
`TOS_CORPUS_TIMINGS=1`. The source validator and build worker then emit
`tos_corpus_stage_timing_v1` start/end JSON lines on stderr for source grammar
preflight, source materialization, catalog work, foundation/index validation
and build phases. The grammar preflight checks the base-plus-batch schema
overlay before unrelated source members are copied; the complete source
identity and semantic audit still follows.
The normal stdout JSON and admission/build authority are unchanged; unset the
variable for ordinary runs. Builds also report the separate fixity check for
historical retirement objects that are not copied into the private source view.

## Corpus and integration

Select the exact corpus operation, source snapshot and owner validators.
Source identity, rights, provenance, review and reference closure remain
required for that admission. A software test does not accept corpus data.
A builder writes only its selected output; reproducible generated data is not
an automatic Git companion of every code edit.

During the verified data migration, a deliberate historical snapshot audit
remains available through `release_check.py --integration-audit`. It is not
a software merge requirement. KAG/stats validation belongs to their selected
integration artifacts under [D0062](../docs/decisions/TOS-D-0062-independent-software-corpus-and-integration-releases.md).

The `active_naming` route, when selected for naming work, uses
`python scripts/validate_active_naming.py`. Its optional `--feedback-cache`
accepts an external local SQLite cache; cached results do not replace release
or corpus-admission evidence. No inventory or documentation-currentness
rebuild is required merely because a script or test file was added.
