//! Independent, synthetic STO.2 format and receipt conformance.
//! OPS registers this Linux-only integration target after the segment crate lands.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use serde_json::Value;
use tempfile::TempDir;
use tos_foundation::Digest256;
use tos_segment_store::{FrameInput, OwnerBinding, SegmentErrorCode, SegmentLimits, SegmentStore};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("segment-v1")
}

fn fixture() -> Value {
    serde_json::from_slice(&fs::read(fixture_dir().join("fixture.json")).unwrap()).unwrap()
}

fn hex_bytes(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn limits() -> SegmentLimits {
    SegmentLimits {
        max_segment_bytes: 1024,
        max_frame_bytes: 64,
        max_frames: 4,
        max_journal_bytes: 8192,
    }
}

fn empty_root() -> (TempDir, PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("store");
    fs::create_dir(&root).unwrap();
    (temporary, root)
}

fn binding(slot: u32) -> OwnerBinding {
    OwnerBinding {
        profile_id: b"ass-synthetic".to_vec(),
        profile_version: b"v1".to_vec(),
        subject_key: format!("opaque-ref-{slot}").into_bytes(),
        member_slot: slot,
    }
}

#[test]
fn independent_two_frame_bytes_recover_and_abort_fence() {
    let expected = fixture();
    let domain = hex_bytes(expected["custody_domain_hex"].as_str().unwrap());
    let rows = expected["frames"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let first = hex_bytes(rows[0]["payload_hex"].as_str().unwrap());
    let second = hex_bytes(rows[1]["payload_hex"].as_str().unwrap());
    let (_temporary, root) = empty_root();
    let store = SegmentStore::initialize_empty(&root, &domain, limits()).unwrap();
    let mut first_reader = Cursor::new(first.clone());
    let mut second_reader = Cursor::new(second.clone());
    let mut inputs = [
        FrameInput {
            binding: binding(0),
            declared_size: first.len() as u64,
            declared_sha256: Digest256::from_hex(rows[0]["sha256"].as_str().unwrap()).unwrap(),
            reader: &mut first_reader,
        },
        FrameInput {
            binding: binding(1),
            declared_size: second.len() as u64,
            declared_sha256: Digest256::from_hex(rows[1]["sha256"].as_str().unwrap()).unwrap(),
            reader: &mut second_reader,
        },
    ];
    let receipts = store
        .seal_segment(b"ass-prepare-two-frame", &mut inputs)
        .unwrap();
    assert_eq!(receipts.len(), 2);
    assert_ne!(receipts[0].receipt_id(), receipts[1].receipt_id());
    let expected_segment =
        Digest256::from_hex(expected["segment_sha256"].as_str().unwrap()).unwrap();
    let exact_file = fs::read(root.join("segments").join(expected_segment.to_hex())).unwrap();
    assert_eq!(
        exact_file,
        fs::read(fixture_dir().join("two-frame.bin")).unwrap()
    );
    assert_eq!(receipts[0].segment_digest(), expected_segment);
    assert_eq!(
        receipts[0].segment_size(),
        expected["segment_size"].as_u64().unwrap()
    );
    for (receipt, row, expected_bytes) in [
        (&receipts[0], &rows[0], &first),
        (&receipts[1], &rows[1], &second),
    ] {
        assert_eq!(
            receipt.coordinate().header_offset,
            row["frame_header_offset"].as_u64().unwrap()
        );
        assert_eq!(
            receipt.coordinate().size_bytes,
            row["length"].as_u64().unwrap()
        );
        assert_eq!(
            receipt.coordinate().sha256.to_hex(),
            row["sha256"].as_str().unwrap()
        );
        store.verify_receipt(receipt).unwrap();
        let mut unpublished = Vec::new();
        store.read_selected(receipt, 64, &mut unpublished).unwrap();
        assert_eq!(&unpublished, expected_bytes);
    }
    let reopened = SegmentStore::open_existing(&root, limits()).unwrap();
    let recovered = reopened.recover_sealed(receipts[0].pin_id()).unwrap();
    assert_eq!(recovered.len(), 2);
    assert_eq!(recovered[0].receipt_id(), receipts[0].receipt_id());
    reopened.verify_receipt(&recovered[0]).unwrap();
    assert_eq!(
        reopened
            .abort_uncommitted(recovered[0].pin_id(), b"ass-prepare-two-frame", 1)
            .unwrap(),
        2,
    );
    assert_eq!(
        reopened.verify_receipt(&recovered[0]).unwrap_err().code,
        SegmentErrorCode::InvalidReceipt
    );
    assert_eq!(
        reopened
            .recover_sealed(recovered[0].pin_id())
            .unwrap_err()
            .code,
        SegmentErrorCode::InvalidReceipt
    );
}

#[test]
fn selected_frame_damage_does_not_hide_from_whole_receipt_check() {
    let expected = fixture();
    let rows = expected["frames"].as_array().unwrap();
    let domain = hex_bytes(expected["custody_domain_hex"].as_str().unwrap());
    let first = hex_bytes(rows[0]["payload_hex"].as_str().unwrap());
    let second = hex_bytes(rows[1]["payload_hex"].as_str().unwrap());
    let (_temporary, root) = empty_root();
    let store = SegmentStore::initialize_empty(&root, &domain, limits()).unwrap();
    let mut first_reader = Cursor::new(first);
    let mut second_reader = Cursor::new(second.clone());
    let mut inputs = [
        FrameInput {
            binding: binding(0),
            declared_size: 4,
            declared_sha256: Digest256::from_hex(rows[0]["sha256"].as_str().unwrap()).unwrap(),
            reader: &mut first_reader,
        },
        FrameInput {
            binding: binding(1),
            declared_size: 4,
            declared_sha256: Digest256::from_hex(rows[1]["sha256"].as_str().unwrap()).unwrap(),
            reader: &mut second_reader,
        },
    ];
    let receipts = store.seal_segment(b"ass-damage", &mut inputs).unwrap();
    let path = root
        .join("segments")
        .join(receipts[0].segment_digest().to_hex());
    let mut damaged = fs::read(&path).unwrap();
    assert_eq!(damaged.len(), 144);
    damaged[88] ^= 1; // first payload, not its envelope or the second frame
    fs::write(&path, damaged).unwrap();
    assert_eq!(
        store.verify_receipt(&receipts[0]).unwrap_err().code,
        SegmentErrorCode::CorruptBytes
    );
    assert_eq!(
        store.recover_sealed(receipts[0].pin_id()).unwrap_err().code,
        SegmentErrorCode::CorruptBytes
    );
    let mut unpublished = Vec::new();
    assert_eq!(
        store
            .read_selected(&receipts[0], 64, &mut unpublished)
            .unwrap_err()
            .code,
        SegmentErrorCode::CorruptBytes
    );
    assert!(
        unpublished.is_empty(),
        "corrupt selected bytes reached the sink"
    );
    store
        .read_selected(&receipts[1], 64, &mut unpublished)
        .unwrap();
    assert_eq!(unpublished, second); // selected frame checks do not hash unrelated frames
}

#[test]
fn identical_bytes_keep_distinct_owner_bindings_without_a_rights_decision() {
    let expected = fixture();
    let domain = hex_bytes(expected["custody_domain_hex"].as_str().unwrap());
    let payload = hex_bytes(expected["frames"][0]["payload_hex"].as_str().unwrap());
    let digest = Digest256::from_hex(expected["frames"][0]["sha256"].as_str().unwrap()).unwrap();
    let (_temporary, root) = empty_root();
    let store = SegmentStore::initialize_empty(&root, &domain, limits()).unwrap();
    let mut first_reader = Cursor::new(payload.clone());
    let mut second_reader = Cursor::new(payload.clone());
    let mut inputs = [
        FrameInput {
            binding: binding(0),
            declared_size: payload.len() as u64,
            declared_sha256: digest,
            reader: &mut first_reader,
        },
        FrameInput {
            binding: binding(1),
            declared_size: payload.len() as u64,
            declared_sha256: digest,
            reader: &mut second_reader,
        },
    ];
    let receipts = store.seal_segment(b"ass-same-bytes", &mut inputs).unwrap();
    assert_eq!(receipts.len(), 2);
    assert_eq!(
        receipts[0].coordinate().sha256,
        receipts[1].coordinate().sha256
    );
    assert_ne!(receipts[0].receipt_id(), receipts[1].receipt_id());
    assert_ne!(receipts[0].binding(), receipts[1].binding());
    for receipt in &receipts {
        store.verify_receipt(receipt).unwrap();
        let mut unpublished = Vec::new();
        store.read_selected(receipt, 64, &mut unpublished).unwrap();
        assert_eq!(unpublished, payload);
    }
}

#[test]
fn maximum_declared_domain_reopens_and_oversized_frame_preflights() {
    let (_temporary, root) = empty_root();
    let domain = vec![b'D'; u16::MAX as usize];
    let store = SegmentStore::initialize_empty(&root, &domain, limits()).unwrap();
    assert_eq!(store.domain_digest(), Digest256::of_bytes(&domain));
    let reopened = SegmentStore::open_existing(&root, limits()).unwrap();
    assert_eq!(reopened.store_id(), store.store_id());
    let mut one = Cursor::new([1u8]);
    let mut inputs = [FrameInput {
        binding: binding(0),
        declared_size: limits().max_frame_bytes + 1,
        declared_sha256: Digest256::of_bytes(&[1]),
        reader: &mut one,
    }];
    assert_eq!(
        reopened
            .seal_segment(b"ass-too-large", &mut inputs)
            .unwrap_err()
            .code,
        SegmentErrorCode::UnsupportedOversized
    );
    assert_eq!(fs::read_dir(root.join("pins")).unwrap().count(), 0);
}
