# Native local prepared publication

The Python `tos_access.prepared_publication`, `prepared_catalog`, prepared
search, compact-lens and membership-index writer APIs are retired. The native
`tos prepare` producer remains the supported way to create a local prepared
snapshot; its explicit output and optional maintenance attachment are described
in [`OFFLINE_PREPARED_BOOTSTRAP.md`](OFFLINE_PREPARED_BOOTSTRAP.md).

The native product continues to read compatible selected prepared snapshots
under its declared profile and binding checks. Retirement of Python writers
does not relabel historical snapshots or grant them a new binding. See the
[native selected-snapshot contract](contracts/native-selected-snapshot-profile.v1.md)
for the read boundary and refusal behavior.
