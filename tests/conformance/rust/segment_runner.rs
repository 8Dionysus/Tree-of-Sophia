//! Independent, synthetic STO.2 format and receipt conformance.
//! OPS registers this Linux-only integration target after the segment crate lands.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

use serde_json::Value;
use tempfile::TempDir;
use tos_foundation::Digest256;
use tos_segment_store::{
    FrameInput, OwnerBinding, PlacementV1, SegmentErrorCode, SegmentLimits, SegmentStore,
};

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

#[test]
fn placement_wire_requires_cold_pin_and_exact_physical_member() {
    let (_temporary, root) = empty_root();
    let store = SegmentStore::initialize_empty(&root, b"ass-private-domain", limits()).unwrap();
    let bytes = b"same bytes, different owner".to_vec();
    let mut first = Cursor::new(bytes.clone());
    let mut second = Cursor::new(bytes.clone());
    let mut inputs = [
        FrameInput {
            binding: binding(0),
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&bytes),
            reader: &mut first,
        },
        FrameInput {
            binding: binding(1),
            declared_size: bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(&bytes),
            reader: &mut second,
        },
    ];
    let receipts = store.seal_segment(b"ass-placement", &mut inputs).unwrap();
    let wire = receipts[1].placement().encode();
    assert_eq!(wire.len(), 208);
    let cold = SegmentStore::open_existing(&root, limits()).unwrap();
    let recovered = cold
        .recover_placement(&PlacementV1::decode(&wire).unwrap())
        .unwrap();
    assert_eq!(recovered.binding(), &binding(1));
    assert_ne!(recovered.receipt_id(), receipts[0].receipt_id());

    for bad in [
        wire[..207].to_vec(),
        [wire.as_slice(), &[0]].concat(),
        {
            let mut v = wire.to_vec();
            v[8] ^= 1; // wire version
            v
        },
    ] {
        assert_eq!(
            PlacementV1::decode(&bad).unwrap_err().code,
            SegmentErrorCode::InvalidFormat
        );
    }
    for offset in [84usize, 116, 156, 160, 176] {
        let mut altered = wire;
        altered[offset] ^= 1; // receipt, segment, frame index, coordinate, member digest
        let decoded = PlacementV1::decode(&altered).unwrap();
        assert_eq!(
            cold.recover_placement(&decoded).unwrap_err().code,
            SegmentErrorCode::InvalidReceipt,
            "altered placement byte {offset} acquired a receipt"
        );
    }

    let segment = root
        .join("segments")
        .join(receipts[1].segment_digest().to_hex());
    let mut damaged = fs::read(&segment).unwrap();
    damaged[receipts[1].coordinate().header_offset as usize + 40] ^= 1;
    fs::write(&segment, damaged).unwrap();
    assert_eq!(
        cold.recover_placement(&PlacementV1::decode(&wire).unwrap())
            .unwrap_err()
            .code,
        SegmentErrorCode::CorruptBytes
    );
    let mut unpublished = Vec::new();
    assert_eq!(
        cold.read_selected(&recovered, 64, &mut unpublished)
            .unwrap_err()
            .code,
        SegmentErrorCode::CorruptBytes
    );
    assert!(unpublished.is_empty());
}

#[test]
fn placement_wire_is_exact_little_endian_v1() {
    // Independently assembled with Python struct.pack, not the Rust encoder.
    let wire = hex_bytes(concat!(
        "544f53504c43563101000000",
        "000102030405060708090a0b0c0d0e0f",
        "101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f",
        "303132333435363738393a3b3c3d3e3f",
        "0700000000000000",
        "404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f",
        "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f",
        "c8000000000000000200000064000000000000001400000000000000",
        "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f",
    ));
    assert_eq!(wire.len(), PlacementV1::ENCODED_BYTES);
    assert_eq!(
        Digest256::of_bytes(&wire).to_hex(),
        "42478dde48195a3b9c4e6292c63ca2a7d7e5a2a942b83456d6cd0e2437c18fc7"
    );
    let placement = PlacementV1::decode(&wire).unwrap();
    assert_eq!(placement.store_id(), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    assert_eq!(placement.fence_epoch(), 7);
    assert_eq!(placement.segment_size(), 200);
    assert_eq!(placement.frame_index(), 2);
    assert_eq!(placement.coordinate().header_offset, 100);
    assert_eq!(placement.coordinate().size_bytes, 20);
    assert_eq!(placement.encode().as_slice(), wire.as_slice());
}
