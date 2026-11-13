# Synthetic V2 reader fixture

This fixture contains one member and identity at two retained revisions. It
exercises exact point reads, not source admission or artifact restoration. The
first root carries a synthetic snapshot artifact descriptor and the second a
synthetic compact commit descriptor; their source artifacts are not supplied.
Its construction follows the storage-boundary fixture in
`tos-source-store/src/native_v2/reader.rs`.

`tos-software-ci verify-reader-install` copies the tracked files into a private
temporary store and creates the empty custody directories before reading it.
