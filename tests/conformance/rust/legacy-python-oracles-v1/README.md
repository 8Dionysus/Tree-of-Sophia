# Frozen legacy Python oracle evidence

These full JSON outputs were captured by the root owner from the bounded historical Python implementations using the exact source and selected-input hashes recorded in each `.provenance.json`. Native conformance tests read these files; they do not regenerate expected values.

`whole-authored-philosophy-graph.provenance.json` records the complete historical graph stream's SHA-256 and byte count. The raw 73,584,723-byte stream remains in the separate R4 evidence artifact, not this repository; the Rust test checks native stream bytes against its exact digest and the summary fixture's byte count and collection roots.

The catalog test preserves the full captured manifest and graph, asserting the historical `generated_by` identity before mapping that one field to the native owner identity `tos-native-owner-command source-catalog build`.
