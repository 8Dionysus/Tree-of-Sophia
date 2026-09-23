# Exact native reader

`tos-reader` is a trusted-local read-only adapter over `tos-source-store`. It
requires an exact retained corpus revision and source ID. The caller supplies
all finite read and JSON limits and an absolute private staging directory.
The selected object is checked against the snapshot and staged before any
bytes reach stdout. A failure returns no selected bytes unless stdout itself
fails after successful verification.

Install with `cargo install --locked --path rust/crates/tos-reader --root
INSTALL_ROOT` and run `INSTALL_ROOT/bin/tos-reader --help` for the required
options. `scripts/verify_rust_reader_install.py` checks old and current
fixture revisions after an isolated install.

The reader requires Linux 5.6 or newer with `openat2`. It anchors traversal to
opened directory descriptors and refuses symlink traversal. Non-Linux builds
fail, and an older Linux kernel returns an unsupported-platform error; there
is no weaker path-open fallback. `tos-reader --capabilities` prints the
versioned platform and store-format contract. The isolated install check
exercises the actual host's `openat2` path with both retained fixture revisions.

This raw local reader has no rights, consent, publication or current-use
policy adapter. Its input store must be a trusted local snapshot. Do not
expose this binary as an HTTP, MCP or public corpus endpoint.
