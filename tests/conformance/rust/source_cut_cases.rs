//! Source carrier coverage over the existing independent two-revision fixture.
//! No source admission, current rights, or billion-record profile is asserted.
use super::*;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_source_store::{CutReadLimits, SourcePresenceV1};

fn source_fixture() -> (TempDir, PathBuf, SourceRevision, SourceRevision) {
    let (temporary, root) = working_store();
    let old = "21e26ca0a4ccca2e99eb67a561a061894c852c6cd5b02006e4592673773419e1";
    let current = "dd5782f4226f3a0000deae0040fae06c3d12cd609769da8564a8e035fc18c971";
    let mut previous = None;
    let mut revisions = Vec::new();
    for original in [old, current] {
        let mut manifest: Value = serde_json::from_slice(
            &fs::read(root.join("revisions").join(original).join("snapshot.json")).unwrap(),
        )
        .unwrap();
        let rename = |path: &str| format!("ToS/source-witnesses/fixture/{path}");
        for entry in manifest["files"].as_array_mut().unwrap() {
            entry["path"] = Value::String(rename(entry["path"].as_str().unwrap()));
        }
        for path in manifest["identities"].as_object_mut().unwrap().values_mut() {
            *path = Value::String(rename(path.as_str().unwrap()));
        }
        let dependencies = manifest["dependencies"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(path, refs)| (rename(path), refs.clone()))
            .collect();
        manifest["dependencies"] = Value::Object(dependencies);
        manifest["base_revision"] = previous.map(Value::String).unwrap_or(Value::Null);
        manifest.as_object_mut().unwrap().remove("revision");
        let digest = Digest256::of_bytes(&canonical_json(&manifest));
        manifest["revision"] = Value::String(digest.to_hex());
        let dir = root.join("revisions").join(digest.to_hex());
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("snapshot.json"), canonical_json(&manifest)).unwrap();
        previous = Some(digest.to_hex());
        revisions.push(SourceRevision(digest));
    }
    (temporary, root, revisions[1], revisions[0])
}

#[test]
fn source_cut_preserves_current_retained_and_complete_eof() {
    let (_temporary, root, current, retained) = source_fixture();
    let reader = CorpusReader::open_existing(&root, read_limits()).unwrap();
    let budget = CutReadLimits {
        max_revisions: 2,
        max_members: 3,
        max_total_bytes: 33,
        max_member_bytes: 20,
    };
    let cancel = AtomicBool::new(false);
    let deadline = || Instant::now() + Duration::from_secs(10);
    let cut = reader
        .open_source_cut(current, budget, deadline(), &cancel)
        .unwrap();
    assert_eq!(
        cut.revisions()
            .map(SnapshotRevision::from)
            .collect::<Vec<_>>(),
        vec![SnapshotRevision(current), SnapshotRevision(retained)]
    );
    let alpha = RelativePath::parse("ToS/source-witnesses/fixture/records/alpha.txt").unwrap();
    let directory = RelativePath::parse("ToS/source-witnesses/fixture/records").unwrap();
    assert_eq!(cut.presence(current, &alpha), Some(SourcePresenceV1::File));
    assert_eq!(
        cut.presence(current, &directory),
        Some(SourcePresenceV1::MaterializedDirectory)
    );
    assert_eq!(
        cut.presence(
            retained,
            &RelativePath::parse("ToS/source-witnesses/fixture/records/beta.txt").unwrap()
        ),
        None
    );
    for (revision, expected_count) in [(current, 2), (retained, 1)] {
        let mut stream = cut.stream(revision).unwrap();
        let expected = stream.expectation();
        assert_eq!(expected.count, expected_count);
        assert_eq!(stream.coverage(), None);
        let first = stream.next_member(deadline(), &cancel).unwrap().unwrap();
        assert_eq!(first.path, alpha);
        assert_eq!(first.revision, revision);
        assert_eq!(first.stable_ids, vec!["tos.work.synthetic.alpha"]);
        let metadata = cut
            .revisions()
            .find(|s| s.revision() == revision)
            .unwrap()
            .member(&alpha)
            .unwrap();
        assert_eq!(Digest256::of_bytes(&first.raw), metadata.sha256);
        assert_eq!(first.raw.len() as u64, metadata.size_bytes);
        assert_eq!(stream.coverage(), None);
        if expected_count == 2 {
            assert!(stream.next_member(deadline(), &cancel).unwrap().is_some());
        }
        assert_eq!(stream.coverage(), None, "last row is not EOF");
        assert!(stream.next_member(deadline(), &cancel).unwrap().is_none());
        assert_eq!(stream.coverage(), Some(expected));
    }
    let mut too_short = budget;
    too_short.max_revisions = 1;
    assert_eq!(
        reader
            .open_source_cut(current, too_short, deadline(), &cancel)
            .unwrap_err()
            .code,
        StoreErrorCode::BudgetExceeded
    );
    let mut expired = cut.stream(current).unwrap();
    assert!(expired.next_member(Instant::now(), &cancel).is_err());
    assert!(
        expired.next_member(deadline(), &cancel).is_err(),
        "failure must remain terminal"
    );
    assert!(expired.coverage().is_none());
    let old_digest = cut
        .revisions()
        .last()
        .unwrap()
        .member(&alpha)
        .unwrap()
        .sha256;
    fs::write(root.join("objects").join(old_digest.to_hex()), b"wrong").unwrap();
    let mut old_stream = cut.stream(retained).unwrap();
    assert!(old_stream.next_member(deadline(), &cancel).is_err());
    assert!(old_stream.coverage().is_none());
    assert!(old_stream.next_member(deadline(), &cancel).is_err());
    assert!(
        cut.read_member(current, &alpha, 20, deadline(), &cancel)
            .is_ok(),
        "a retained failure must never fall through to current bytes"
    );
    fs::remove_file(
        root.join("revisions")
            .join(retained.0.to_hex())
            .join("snapshot.json"),
    )
    .unwrap();
    assert_eq!(
        reader
            .open_source_cut(current, budget, deadline(), &cancel)
            .unwrap_err()
            .code,
        StoreErrorCode::MissingRevision
    );
    // The original generic carrier fixture remains valid, but cannot pretend
    // to be a source-owner ToS namespace through its synthetic stable IDs.
    let generic = CorpusReader::open_existing(&fixture_store(), read_limits()).unwrap();
    assert_eq!(
        generic
            .open_source_cut(
                revision("dd5782f4226f3a0000deae0040fae06c3d12cd609769da8564a8e035fc18c971"),
                budget,
                deadline(),
                &cancel
            )
            .unwrap_err()
            .code,
        StoreErrorCode::InvalidMemberIndex
    );
}

#[derive(Debug, PartialEq)]
struct SnapshotRevision(SourceRevision);
impl From<&tos_source_store::Snapshot> for SnapshotRevision {
    fn from(snapshot: &tos_source_store::Snapshot) -> Self {
        Self(snapshot.revision())
    }
}
