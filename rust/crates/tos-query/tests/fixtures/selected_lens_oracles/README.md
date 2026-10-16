# Frozen selected-lens Python oracle packets

These compressed JSON packets are the exact `stdout.json` bytes captured from the successful R4 run of the historical Python engine. The R4 provenance, source commit/tree, native selected-lens product, installed consumer identity, raw-output hashes and compressed-fixture hashes are recorded in `manifest.json`. Tests decompress the packets and retain the existing exact canonical packet comparisons, scope/error assertions, stale-withdrawal checks and budget controls.

The test only applies two explicitly bounded compatibility adaptations already required by the native route: eight known contract availability strings and duplicate inline philosophy rows. The captured files themselves remain untouched.
