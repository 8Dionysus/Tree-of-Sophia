# Exact native reader

`tos-reader` is a trusted-local read-only adapter for native AdmissionStore V2.
It requires an exact retained revision and either `--source-id` or the mutually
exclusive `--member-path`. It grants no public access or corpus admission.

It uses `tos-source-store::native_v2::V2ReadSession`, which authenticates the selected
current pointer and retained history, resolves either the typed identity or the canonical relative member path at
the requested revision, and verifies the object before it is staged. A missing
exact member path is a nonzero result with no selected bytes on stdout; it does
not create or infer a source identity. V2 also requires finite
caller-supplied pointer, segment, tree, object, cumulative I/O, state, and
deadline limits. Packed V2 objects additionally require `--max-work-units N`,
a positive finite cumulative work allowance shared by tree visits and packed
object reads. An omitted work allowance retains the existing unpacked V2
interface and refuses packed reads before exposing selected bytes. Its staging directory must be absolute, normalized, owned by
the current user, and private; the stage file is created relative to a held
directory descriptor. Verified bytes remain staged until the current-pointer
fence succeeds.

A failure returns no selected bytes unless stdout itself fails after verification.

Install with `cargo install --locked --path rust/crates/tos-reader --root
INSTALL_ROOT` and run `INSTALL_ROOT/bin/tos-reader --help` for the required
options. `tos-software-ci verify-reader-install --repo-root ABS` checks old and current
fixture revisions after an isolated install.

The reader requires Linux 5.6 or newer with `openat2`. It anchors traversal to
opened directory descriptors and refuses symlink traversal. Non-Linux builds
fail, and an older Linux kernel returns an unsupported-platform error; there
is no weaker path-open fallback. `tos-reader --capabilities` reports V2 as the
only supported store format. The isolated install check exercises both selectors
at current and retained revisions of a synthetic V2 fixture; it does not establish
source admission or production runtime acceptance.

This raw local reader has no rights, consent, publication or current-use
policy adapter. Its input store must be a trusted local snapshot. Do not
expose this binary as an HTTP, MCP or public corpus endpoint.
