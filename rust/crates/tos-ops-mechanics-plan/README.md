# Mechanics-local command planning

`tos-ops-mechanics-plan` discovers package-local and part-local mechanics
tests, builders and validators. Its output schema is
`tos_mechanics_local_plan_v1`: `test_file_count` plus commands with `kind`,
`home` and `argv`. Commands appear in the current validation-lane order:
one unittest-discover command per test home, then each builder `--check`,
then each validator. Within each group, homes and script names sort by path.

The CLI requires `--repo-root ABSOLUTE_PATH` and accepts
`--python COMMAND` (default `python`). It emits one JSON line. The chosen
Python command is a host adapter in each planned argv; this crate does not
start child processes, decide whether the lane is required, or interpret
their results. The current `scripts/run_mechanics_local_tests.py` remains the
executing consumer until a bounded Rust executor and lane integration are
reviewed. An executor must define time, output and cancellation limits.

Discovery is limited to immediate `mechanics/*` packages and
`mechanics/*/parts/*` parts. It rejects symlinked encountered entries and
caps scanned entries and commands. No authored ToS material is read or
written. The fixture records the Python runner's ordering on a synthetic
package/part tree; real-tree oracle evidence is recorded in the OPS.3
execution packet.
