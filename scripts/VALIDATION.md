# Script validation

Choose the changed behavior and the operation that owns it. Select the relevant command sequences from
`docs/validation/validation_lanes.json` according to that changed behavior and
operation.

## Software

After building the native commands and installing browser dependencies, run:

```sh
tos-release-check --repo-root "$PWD"
```

This checks software contracts, builds browser assets, and runs program tests
on bounded fixtures. It needs no production corpus, KAG, stats, sibling
checkout, generated-currentness refresh or R2 credentials. Browser behavior
is checked by `software_browser` after the build; Worker tests use their own
software route. See [root validation](../VALIDATION.md) and the exact
[release procedure](../docs/RELEASING.md).

For a focused change, run the affected native owner test module or command.
The selected command sequence remains owned by
`docs/validation/validation_lanes.json`; an unknown path does not become an
implicit full-corpus operation.

## Corpus and integration

Select the exact corpus operation, source snapshot and owner validators.
Source identity, rights, provenance, review and reference closure remain
required for that admission. A software test does not accept corpus data.
A builder writes only its selected output; reproducible generated data is not
an automatic Git companion of every code edit.

Native source admission and generated-product checks use their explicit
owner commands and selected source/software inputs. Use
`tos-native-owner-command corpus-admit` for source admission and
`tos-native-owner-command corpus-build` with `mode: check` for bounded generated
projection parity. Projection parity does not replace source admission or
historical corpus assessment. KAG/stats validation belongs to their selected
integration artifacts under [D0062](../docs/decisions/TOS-D-0062-independent-software-corpus-and-integration-releases.md).

The `active_naming` route, when selected for naming work, uses
`tos-ops-mechanics-plan --repo-root ABS --active-naming-validate`. Its optional
`--feedback-cache ABS` accepts an external local SQLite cache on Linux; cached results do not replace release
or corpus-admission evidence. No inventory or documentation-currentness
rebuild is required merely because a script or test file was added.
