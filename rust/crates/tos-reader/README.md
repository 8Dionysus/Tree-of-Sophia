# Exact native reader

`tos-reader` is a trusted-local read-only adapter. It keeps V1 as the default
and accepts `--format v2` as an explicit selection for native AdmissionStore
V2. Both formats require an exact retained revision and one exact selector; V1 accepts only `--source-id`, while V2 accepts either
`--source-id` or the mutually exclusive `--member-path`. Neither format grants
public access or corpus admission.

The V1 path remains the `tos-source-store::CorpusReader` snapshot reader. V2
uses the maintained CMD `V2ReadSession`, which authenticates the selected
current pointer and retained history, resolves either the typed identity or the canonical relative member path at
the requested revision, and verifies the object before it is staged. A missing
exact member path is a nonzero result with no selected bytes on stdout; it does
not create or infer a source identity. V2 also requires finite
caller-supplied pointer, segment, tree, object, cumulative I/O, state, and
deadline limits. Its staging directory must be absolute, normalized, owned by
the current user, and private; the stage file is created relative to a held
directory descriptor. Verified bytes remain staged until the current-pointer
fence succeeds.

The caller supplies all finite V1 read and JSON limits and a private staging
directory. In either format, a failure returns no selected bytes unless stdout
itself fails after verification.

Install with `cargo install --locked --path rust/crates/tos-reader --root
INSTALL_ROOT` and run `INSTALL_ROOT/bin/tos-reader --help` for the required
options. `scripts/verify_rust_reader_install.py` checks old and current
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

## Exact member-path callsite

For the authored E5 pair, the V2 member selector is the exact item member path
`ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/expressions/de-schmeitzner-1883-part-1/editions/chemnitz-schmeitzner-1883-part-1/items/dta-sbb-corrected-tei-p5/item.human-forms.json`. Invoke the normal bounded V2 command with `--store STORE --revision REVISION --member-path 'PATH' --format v2 --stage-dir PRIVATE_STAGE` and the same explicit finite V2 limit arguments required by `--help`; omit `--source-id`. The same exact call at the afc87 revision returns status 2 and no stdout because that member is absent there. The source-owned `item.json` continues to be selected separately by its stable `record_id`; path lookup does not mint identity.
