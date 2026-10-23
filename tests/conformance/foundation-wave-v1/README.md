# R/T3 independent acceptance

The oracle bytes are frozen copies from the authorized preparation ZIP. Their
digests are checked against its unchanged MANIFEST before preparation or execution.
`prepared/requirement-matrix.json` retains every exact original input/expected:
60 temporal and 12 topology cases. Preparation is not runtime acceptance.

Run `python3 harness.py` for fixture/reference checks. Run
`python3 endpoint_oracle.py` for an independent mathematical audit of the two
exact network fixtures and 13 Allen relations. Neither command invokes ToS.

After I/C approves an actual-operation adapter and resource slot, use
`python3 harness.py --adapter /absolute/path/to/adapter`. Each adapter invocation
receives one JSON object on stdin with operation, input, context, source_profile,
profiles, and fixture_registry; it returns one actual envelope on stdout. Case
IDs, expected answers, forbidden assertions, and reference_closure are never sent.
The harness stores exact requests, actual/stdout/stderr, expected, command, oracle
digest, and field differences. It requires all declared expected fields, exact
list contents/order, and exact scalar types; additional diagnostic fields are
allowed. It never treats an unimplemented or harness-error result as a pass.

Select one or more cases with `--case CASE_ID`. `--timeout` is a process safety
timeout, not a solver ResourceLimit result: a killed adapter is HarnessError.
`network_resource_limit` must return the declared Undetermined/ResourceLimit
envelope from the actual implementation. A host timeout cannot satisfy it.

Topology execution requires the T0 manifest/assembler interface. Each mutation
must be applied to a disposable candidate copy; preserve accepted output before
the interruption test and compare after it. Never mutate the baseline checkout.
Parsed object equality and serialization/byte equality are separate gates.

The recurrence-expansion case is post-v1 by the supplied specification. Synthetic
DST and crosswalk tests do not establish IANA/TZDB or historical-calendar support.
Type construction alone does not establish a source-to-consumer vertical slice.

Reviewed diagnostic correction: `chronology_mismatch_snapshot` changes
both S1→S2 and source-rev-42→source-rev-43 but expects only source_snapshot_ref;
`current_shorthand_mismatch` also changes both and expects snapshot/system_state.
No original oracle value has been edited. I/C approved exhaustive mismatch
dimensions; `oracle-corrections.json` records both reviewed replacements and
rationale before any implementation result. Evidence records both original and
reviewed comparisons. The matrix retains original cases and reviewed_expected.

For the native T1 seam, first compile the assigned `foundation_wave_adapter`
integration test with I/C resource admission, then use
`--adapter python3 /absolute/native_test_adapter.py /absolute/test-executable`.
The adapter invokes only the explicitly selected ignored transport test. The
`foundation_wave_context` and `foundation_wave_temporal` integration tests cover
typed interfaces independently; successful execution must be recorded separately.
