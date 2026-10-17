/// Exact historical inputs independent of the current corpus's storage layout.
pub(crate) fn retained_history(repository: &std::path::Path) -> tempfile::TempDir {
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    use tos_foundation::{Digest256, JsonLimits};
    use tos_source_store::{CaptureRestoreLimits, ReadLimits, SoftwareCaptureSelectionV1};

    let capture =
        repository.join("rust/crates/tos-command/tests/fixtures/retained-history-v1/capture");
    let temporary = tempfile::tempdir().expect("private retained-history fixture parent");
    tos_source_store::restore_capture(
        &capture,
        &temporary.path().join("source"),
        &SoftwareCaptureSelectionV1 {
            source_git_commit: "989343513e7d1042cec1bb541f7d39fdfb4ad479".into(),
            source_git_tree: "edcd678c928f511f17d1bfd4d779c2aa5ddf0162".into(),
            capture_manifest_sha256: Digest256::from_hex(
                "8fc43282ad9b06f828559d4619026ded3c99dba616426b5aa01024b3226cb863",
            )
            .unwrap(),
        },
        CaptureRestoreLimits {
            metadata: ReadLimits {
                max_manifest_bytes: 131_072,
                max_manifest_entries: 256,
                max_selected_object_bytes: 1_048_576,
                json: JsonLimits::new(131_072, 64, 32_768, 4300).unwrap(),
            },
            max_archive_bytes: 262_144,
            max_decoded_bytes: 2_097_152,
            max_source_bytes: 2_097_152,
        },
        Instant::now() + Duration::from_secs(60),
        &AtomicBool::new(false),
    )
    .expect("restore pinned retained-history regression fixture");
    temporary
}
