# Exact native reader

`tos-reader` is a trusted-local read-only adapter. It keeps V1 as the default
and accepts `--format v2` as an explicit selection for native AdmissionStore
V2. Both formats require an exact retained revision and one exact selector; V1 accepts only `--source-id`, while V2 accepts either
`--source-id` or the mutually exclusive `--member-path`. Neither format grants
public access or corpus admission.

The V1 path remains the `tos-source-store::CorpusReader` snapshot reader. V2
uses `tos-source-store::native_v2::V2ReadSession`, which authenticates the selected
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

The caller supplies all finite V1 read and JSON limits and a private staging
directory. In either format, a failure returns no selected bytes unless stdout
itself fails after verification.

Install with `cargo install --locked --path rust/crates/tos-reader --root
INSTALL_ROOT` and run `INSTALL_ROOT/bin/tos-reader --help` for the required
options. `tos-software-ci verify-reader-install --repo-root ABS` checks old and current
fixture revisions after an isolated install.

The reader requires Linux 5.6 or newer with `openat2`. It anchors traversal to
opened directory descriptors and refuses symlink traversal. Non-Linux builds
fail, and an older Linux kernel returns an unsupported-platform error; there
is no weaker path-open fallback. `tos-reader --capabilities` reports V1 as the
default and lists the explicit V2 format. The isolated install check exercises
the V1 fixture revisions; it does not claim installed V2 or native-runtime
verification.

This raw local reader has no rights, consent, publication or current-use
policy adapter. Its input store must be a trusted local snapshot. Do not
expose this binary as an HTTP, MCP or public corpus endpoint.
