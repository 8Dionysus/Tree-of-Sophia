# tos-validation

`InProcessSchemaEngine` prepares an offline Draft 2020-12 registry from an
exact resource set and reuses compiled validators for selected root URIs and
fragments. Each engine binds one schema-set digest and format profile. Strict
and legacy input modes preserve their respective parsing rules. Reports carry
`tos_schema_engine_in_process_v1`, input and schema-set digests, bounded
ordered diagnostics and explicit refusal states. `Report::to_evidence_json`
and `Report::from_evidence_json` write and read this engine's evidence.

The source-cut adapter binds each selected contract digest and distinguishes a
source revision from a candidate identity. Callers using the existing worker
continue to use its historical execution evidence. Cancellation and deadline
checks run before and after synchronous validation.

The existing command admission spool keeps worker diagnostics in the
`sf_rows` SQLite payload within its selected index scope. Its v1 report codec
continues to read those rows. The in-process evidence codec is available to
new adapters for a separately versioned report. The command report codec
reads historical version 1 worker evidence and version 2 in-process reports
and envelopes. Command admission continues to write version 1 until caller
migration.
