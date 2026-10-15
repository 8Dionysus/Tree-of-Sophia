//! Exact native V2 descriptor codecs, independent of command execution.
use super::completion::NativeAdmissionCompletionProofV1;
use crate::{SourceMembershipV1, SourceMembershipV2};
use std::{io, mem::size_of};
use tos_foundation::{
    CanonicalProfile, CutPreparedSchemaExecutionBinding, CutPreparedSchemaProtocol, Digest256,
    Digest256Hasher, FormatProfile, JsonDocument, JsonLimits, JsonMode, JsonValue, SourceRevision,
    canonical_bytes_v1, parse_json_with_state_budget,
};
use tos_segment_store::{AuthenticatedTreeDescriptorV2, SegmentStore};
const ROOTSET_SCHEMA: &str = "tos-native-source-rootset-v2";
const LEGACY_REVISION_ROOT_SCHEMA: &str = "tos-native-source-revision-roots-v2";
const LEGACY_IDENTITY_PATHS_REVISION_ROOT_SCHEMA: &str =
    "tos-native-source-revision-roots-v2-identity-paths";
const TYPED_REVISION_ROOT_SCHEMA: &str = "tos-native-source-revision-roots-v2-records";
const PACKED_REVISION_ROOT_SCHEMA: &str = "tos-native-source-revision-roots-packed-v2";
const IDENTITY_PATHS_REVISION_ROOT_SCHEMA: &str =
    "tos-native-source-revision-roots-identity-paths-v2";
const PACKED_IDENTITY_PATHS_REVISION_ROOT_SCHEMA: &str =
    "tos-native-source-revision-roots-packed-identity-paths-v2";
const LEGACY_REVISION_ROOT_V3_SCHEMA: &str = "tos-native-source-revision-roots-legacy-v3";
const LEGACY_IDENTITY_PATHS_REVISION_ROOT_V3_SCHEMA: &str =
    "tos-native-source-revision-roots-legacy-identity-paths-v3";
const TYPED_REVISION_ROOT_V3_SCHEMA: &str = "tos-native-source-revision-roots-v3";
const PACKED_REVISION_ROOT_V3_SCHEMA: &str = "tos-native-source-revision-roots-packed-v3";
const IDENTITY_PATHS_REVISION_ROOT_V3_SCHEMA: &str =
    "tos-native-source-revision-roots-identity-paths-v3";
const PACKED_IDENTITY_PATHS_REVISION_ROOT_V3_SCHEMA: &str =
    "tos-native-source-revision-roots-packed-identity-paths-v3";
const TYPED_REVISION_ROOT_V4_SCHEMA: &str = "tos-native-source-revision-roots-v4";
const PACKED_REVISION_ROOT_V4_SCHEMA: &str = "tos-native-source-revision-roots-packed-v4";
const IDENTITY_PATHS_REVISION_ROOT_V4_SCHEMA: &str =
    "tos-native-source-revision-roots-identity-paths-v4";
const PACKED_IDENTITY_PATHS_REVISION_ROOT_V4_SCHEMA: &str =
    "tos-native-source-revision-roots-packed-identity-paths-v4";
const COMPACT_COMMIT_SCHEMA: &str = "tos-native-source-compact-commit-v2";
const PACKED_COMPACT_COMMIT_SCHEMA: &str = "tos-native-source-compact-commit-packed-v2";
const IDENTITY_PATHS_COMPACT_COMMIT_SCHEMA: &str =
    "tos-native-source-compact-commit-identity-paths-v2";
const PACKED_IDENTITY_PATHS_COMPACT_COMMIT_SCHEMA: &str =
    "tos-native-source-compact-commit-packed-identity-paths-v2";
const COMPACT_COMMIT_V3_SCHEMA: &str = "tos-native-source-compact-commit-v3";
const PACKED_COMPACT_COMMIT_V3_SCHEMA: &str = "tos-native-source-compact-commit-packed-v3";
const IDENTITY_PATHS_COMPACT_COMMIT_V3_SCHEMA: &str =
    "tos-native-source-compact-commit-identity-paths-v3";
const PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V3_SCHEMA: &str =
    "tos-native-source-compact-commit-packed-identity-paths-v3";
const COMPACT_COMMIT_V4_SCHEMA: &str = "tos-native-source-compact-commit-v4";
const PACKED_COMPACT_COMMIT_V4_SCHEMA: &str = "tos-native-source-compact-commit-packed-v4";
const IDENTITY_PATHS_COMPACT_COMMIT_V4_SCHEMA: &str =
    "tos-native-source-compact-commit-identity-paths-v4";
const PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V4_SCHEMA: &str =
    "tos-native-source-compact-commit-packed-identity-paths-v4";
const IDENTITY_PATHS_COMPACT_PREIMAGE_SCHEMA: &str =
    "tos-native-source-compact-commit-identity-paths-v2-preimage";
const PACKED_IDENTITY_PATHS_COMPACT_PREIMAGE_SCHEMA: &str =
    "tos-native-source-compact-commit-packed-identity-paths-v2-preimage";
const COMPACT_COMMIT_V3_PREIMAGE_SCHEMA: &str = "tos-native-source-compact-commit-v3-preimage";
const PACKED_COMPACT_COMMIT_V3_PREIMAGE_SCHEMA: &str =
    "tos-native-source-compact-commit-packed-v3-preimage";
const IDENTITY_PATHS_COMPACT_COMMIT_V3_PREIMAGE_SCHEMA: &str =
    "tos-native-source-compact-commit-identity-paths-v3-preimage";
const PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V3_PREIMAGE_SCHEMA: &str =
    "tos-native-source-compact-commit-packed-identity-paths-v3-preimage";
const COMPACT_COMMIT_V4_PREIMAGE_SCHEMA: &str = "tos-native-source-compact-commit-v4-preimage";
const PACKED_COMPACT_COMMIT_V4_PREIMAGE_SCHEMA: &str =
    "tos-native-source-compact-commit-packed-v4-preimage";
const IDENTITY_PATHS_COMPACT_COMMIT_V4_PREIMAGE_SCHEMA: &str =
    "tos-native-source-compact-commit-identity-paths-v4-preimage";
const PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V4_PREIMAGE_SCHEMA: &str =
    "tos-native-source-compact-commit-packed-identity-paths-v4-preimage";
const PACKED_COMPACT_REVISION_DOMAIN: &[u8] =
    b"tos-native-source-compact-commit-packed-revision-v2\0";
const COMPACT_COMMIT_REVISION_DOMAIN: &[u8] = b"tos-native-source-compact-commit-revision-v2\0";
const IDENTITY_PATHS_COMPACT_REVISION_DOMAIN: &[u8] =
    b"tos-native-source-compact-commit-identity-paths-revision-v2\0";
const PACKED_IDENTITY_PATHS_COMPACT_REVISION_DOMAIN: &[u8] =
    b"tos-native-source-compact-commit-packed-identity-paths-revision-v2\0";
const COMPACT_COMMIT_V3_REVISION_DOMAIN: &[u8] = b"tos-native-source-compact-commit-revision-v3\0";
const PACKED_COMPACT_COMMIT_V3_REVISION_DOMAIN: &[u8] =
    b"tos-native-source-compact-commit-packed-revision-v3\0";
const IDENTITY_PATHS_COMPACT_COMMIT_V3_REVISION_DOMAIN: &[u8] =
    b"tos-native-source-compact-commit-identity-paths-revision-v3\0";
const PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V3_REVISION_DOMAIN: &[u8] =
    b"tos-native-source-compact-commit-packed-identity-paths-revision-v3\0";
const COMPACT_COMMIT_V4_REVISION_DOMAIN: &[u8] = b"tos-native-source-compact-commit-revision-v4\0";
const PACKED_COMPACT_COMMIT_V4_REVISION_DOMAIN: &[u8] =
    b"tos-native-source-compact-commit-packed-revision-v4\0";
const IDENTITY_PATHS_COMPACT_COMMIT_V4_REVISION_DOMAIN: &[u8] =
    b"tos-native-source-compact-commit-identity-paths-revision-v4\0";
const PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V4_REVISION_DOMAIN: &[u8] =
    b"tos-native-source-compact-commit-packed-identity-paths-revision-v4\0";
pub const ROOTSET_MAX_BYTES: usize = 65_536;
const TREE_DESCRIPTOR_MAX_BYTES: usize = 12_288;
const ENCODE_WORKSPACE_FIXED_OVERHEAD: usize = 64 * 1024;
pub const MAX_COMPACT_COMMIT_V2_BYTES: usize = 65_536;
pub const MEMBERS_KIND: &[u8] = b"source-members-v2";
pub const IDENTITIES_KIND: &[u8] = b"source-identities-v2";
pub const DEPENDENCIES_KIND: &[u8] = b"source-dependencies-v2";
pub const RETIREMENTS_KIND: &[u8] = b"source-retirements-v2";
pub const OBJECT_EXTENTS_KIND: &[u8] = b"source-object-extents-v2";
pub const IDENTITY_PATHS_KIND: &[u8] = b"source-identity-paths-v2";
pub const HISTORY_KIND: &[u8] = b"source-history-v2";
pub const SOURCE_ADMISSION_V2_DOMAIN: &[u8] = b"tos-native-admission-source-v2";

fn validate_tree_binding(
    tree: &AuthenticatedTreeDescriptorV2,
    store_id: [u8; 16],
    domain_digest: Digest256,
    kind: &[u8],
    entries: u64,
) -> io::Result<()> {
    if tree.store_id != store_id
        || tree.domain_digest != domain_digest
        || tree.kind.as_slice() != kind
        || tree.entries != entries
    {
        return Err(invalid("source root tree binding differs"));
    }
    // Reuse the physical owner's descriptor shape and commitment checks.
    let raw = tree_bytes(tree)?;
    if AuthenticatedTreeDescriptorV2::decode(&raw, TREE_DESCRIPTOR_MAX_BYTES)
        .map_err(|_| invalid("source root tree commitment is invalid"))?
        != *tree
    {
        return Err(invalid("source root tree roundtrip differs"));
    }
    Ok(())
}

fn revision_descriptor_refs<'a>(
    members: &'a AuthenticatedTreeDescriptorV2,
    identities: &'a AuthenticatedTreeDescriptorV2,
    dependencies: &'a AuthenticatedTreeDescriptorV2,
    retirements: &'a AuthenticatedTreeDescriptorV2,
    objects: Option<&'a AuthenticatedTreeDescriptorV2>,
    identity_paths: Option<&'a AuthenticatedTreeDescriptorV2>,
) -> ([&'a AuthenticatedTreeDescriptorV2; 6], usize) {
    // Stack-only view: unused optional slots create no clone or allocation.
    (
        [
            members,
            identities,
            dependencies,
            retirements,
            objects.or(identity_paths).unwrap_or(retirements),
            identity_paths.unwrap_or(retirements),
        ],
        4 + usize::from(objects.is_some()) + usize::from(identity_paths.is_some()),
    )
}

fn append_storage_descriptors(
    mut value: serde_json::Value,
    objects: Option<&AuthenticatedTreeDescriptorV2>,
    identity_paths: Option<&AuthenticatedTreeDescriptorV2>,
) -> io::Result<serde_json::Value> {
    let fields = value
        .as_array_mut()
        .ok_or_else(|| invalid("storage tuple is not an array"))?;
    if let Some(objects) = objects {
        fields.push(
            serde_json::to_value(tree_bytes(objects)?)
                .map_err(|_| invalid("object tree descriptor JSON differs"))?,
        );
    }
    if let Some(identity_paths) = identity_paths {
        fields.push(
            serde_json::to_value(tree_bytes(identity_paths)?)
                .map_err(|_| invalid("identity tree descriptor JSON differs"))?,
        );
    }
    Ok(value)
}

#[cfg(test)]
mod rootset_descriptor_tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};
    use tos_segment_store::{AuthenticatedTreeEntryV1, AuthenticatedTreeLimitsV1, SegmentLimits};

    #[test]
    fn optional_storage_descriptors_preserve_packed_and_empty_wire_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let store = SegmentStore::initialize_empty(
            directory.path(),
            b"rootset-descriptor-test",
            SegmentLimits {
                max_segment_bytes: 1024 * 1024,
                max_frame_bytes: 4096,
                max_frames: 8,
                max_journal_bytes: 64 * 1024,
            },
        )
        .unwrap();
        let limits = AuthenticatedTreeLimitsV1 {
            max_key_bytes: 32,
            max_value_bytes: 76,
            max_kind_bytes: 64,
            max_node_bytes: 4096,
            max_children: 16,
            max_nodes: 128,
            max_total_bytes: 16 * 1024 * 1024,
            max_rows: 3,
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        let cancelled = AtomicBool::new(false);
        let objects = store
            .build_authenticated_tree_v2(
                OBJECT_EXTENTS_KIND,
                [Ok(AuthenticatedTreeEntryV1 {
                    key: b"object".to_vec(),
                    value: b"extent".to_vec(),
                })],
                limits,
                deadline,
                &cancelled,
            )
            .unwrap();
        let identities = store
            .build_authenticated_tree_v2(
                IDENTITY_PATHS_KIND,
                std::iter::empty(),
                limits,
                deadline,
                &cancelled,
            )
            .unwrap();
        for (object, identity) in [
            (None, None),
            (Some(&objects), None),
            (None, Some(&identities)),
            (Some(&objects), Some(&identities)),
        ] {
            let value = append_storage_descriptors(serde_json::json!(["prefix"]), object, identity)
                .unwrap();
            let wire = serde_json::to_vec(&value).unwrap();
            let (document, _) = parse_bounded_json(
                &wire,
                ROOTSET_MAX_BYTES,
                decode_workspace_upper_bound(wire.len()).unwrap(),
            )
            .unwrap();
            let fields = document.root().as_array().unwrap();
            let expected: Vec<_> = object.into_iter().chain(identity).collect();
            assert_eq!(fields.len(), 1 + expected.len());
            let (accounted, count) =
                revision_descriptor_refs(&objects, &objects, &objects, &objects, object, identity);
            assert_eq!(&accounted[4..count], expected.as_slice());
            for (index, (field, descriptor)) in fields[1..].iter().zip(expected).enumerate() {
                assert_eq!(tree(field).unwrap(), *descriptor);
                assert_eq!(
                    serde_json::from_value::<Vec<u8>>(value[index + 1].clone()).unwrap(),
                    tree_bytes(descriptor).unwrap(),
                );
            }
        }
    }
}

pub fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Conservative transient allowance for decoding one bounded wire row. The
/// foundation parser enforces its share before each allocation; the remaining
/// 64 raw-sized bytes cover canonical output, typed descriptors, and the
/// compact revision preimage while the parsed document is still live.
pub fn decode_workspace_upper_bound(raw_bytes: usize) -> io::Result<usize> {
    if raw_bytes == 0 || raw_bytes > ROOTSET_MAX_BYTES.max(MAX_COMPACT_COMMIT_V2_BYTES) {
        return Err(invalid("source V2 JSON byte profile exceeded"));
    }
    raw_bytes
        .checked_mul(256)
        .and_then(|bytes| bytes.checked_add(64 * 1024))
        .ok_or_else(|| invalid("source V2 JSON workspace bound overflow"))
}

fn parse_bounded_json(
    raw: &[u8],
    maximum_bytes: usize,
    workspace_bytes: usize,
) -> io::Result<(JsonDocument, JsonLimits)> {
    if raw.is_empty() || raw.len() > maximum_bytes {
        return Err(invalid("source V2 JSON byte profile exceeded"));
    }
    let required = decode_workspace_upper_bound(raw.len())?;
    if workspace_bytes < required {
        return Err(invalid(
            "source V2 JSON workspace reservation is insufficient",
        ));
    }
    let fixed = raw
        .len()
        .checked_mul(64)
        .and_then(|bytes| bytes.checked_add(16 * 1024))
        .ok_or_else(|| invalid("source V2 JSON workspace overflow"))?;
    let parser_workspace = workspace_bytes
        .checked_sub(fixed)
        .ok_or_else(|| invalid("source V2 JSON workspace preflight refused"))?;
    let limits = JsonLimits::new(raw.len(), 16, raw.len(), 4300)
        .map_err(|_| invalid("source V2 JSON limits are invalid"))?;
    let document =
        parse_json_with_state_budget(raw, JsonMode::PublishedStrict, limits, parser_workspace)
            .map_err(|_| invalid("source V2 JSON parse or workspace bound refused"))?;
    let canonical = canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|_| invalid("source V2 JSON canonical emission refused"))?;
    if canonical.as_slice() != raw {
        return Err(invalid("source V2 JSON encoding is not canonical"));
    }
    Ok((document, limits))
}

fn digest(value: &JsonValue) -> io::Result<Digest256> {
    let text = value
        .as_str()
        .ok_or_else(|| invalid("source rootset digest field is not text"))?;
    Digest256::from_hex(text).map_err(|_| invalid("source rootset digest encoding differs"))
}

fn optional_revision(value: &JsonValue) -> io::Result<Option<SourceRevision>> {
    if value.is_null() {
        Ok(None)
    } else {
        digest(value).map(|revision| Some(SourceRevision(revision)))
    }
}

fn number(value: &JsonValue) -> io::Result<u64> {
    value
        .as_u64()
        .ok_or_else(|| invalid("source rootset count is not unsigned"))
}

fn membership_v1_value(membership: Option<SourceMembershipV1>) -> serde_json::Value {
    membership.map_or(serde_json::Value::Null, |membership| {
        serde_json::json!([membership.count, membership.digest.to_hex()])
    })
}

fn membership_v1_from(value: &JsonValue) -> io::Result<Option<SourceMembershipV1>> {
    if value.is_null() {
        return Ok(None);
    }
    let fields = value
        .as_array()
        .filter(|fields| fields.len() == 2)
        .ok_or_else(|| invalid("source V1 membership tuple shape differs"))?;
    Ok(Some(SourceMembershipV1 {
        count: number(&fields[0])?,
        digest: digest(&fields[1])?,
    }))
}

fn membership_v2_value(membership: Option<SourceMembershipV2>) -> serde_json::Value {
    membership.map_or(serde_json::Value::Null, |membership| {
        serde_json::json!([
            membership.count,
            membership.source_bytes,
            membership.members_tree_commitment.to_hex(),
            membership.commitment.to_hex()
        ])
    })
}

fn membership_v2_from(value: &JsonValue) -> io::Result<Option<SourceMembershipV2>> {
    if value.is_null() {
        return Ok(None);
    }
    let fields = value
        .as_array()
        .filter(|fields| fields.len() == 4)
        .ok_or_else(|| invalid("source V2 membership tuple shape differs"))?;
    Ok(Some(SourceMembershipV2 {
        count: number(&fields[0])?,
        source_bytes: number(&fields[1])?,
        members_tree_commitment: digest(&fields[2])?,
        commitment: digest(&fields[3])?,
    }))
}

fn completion_proof_value(proof: Option<NativeAdmissionCompletionProofV1>) -> serde_json::Value {
    proof.map_or(serde_json::Value::Null, |proof| {
        let (protocol, caps) = match proof.prepared_schema().protocol {
            CutPreparedSchemaProtocol::LegacyScalar => ("legacy-scalar", serde_json::Value::Null),
            CutPreparedSchemaProtocol::DiagnosticsV2 { caps_sha256 } => {
                ("diagnostics-v2", serde_json::json!(caps_sha256.to_hex()))
            }
        };
        serde_json::json!([
            proof.validator_sha256().to_hex(),
            membership_v1_value(proof.membership_v1()),
            proof.source_bytes(),
            [
                proof.prepared_schema().schema_profile.id(),
                proof.prepared_schema().schema_set_sha256.to_hex(),
                proof.prepared_schema().worker_sha256.to_hex(),
                [protocol, caps]
            ],
            proof.identity_count(),
            proof.dependency_source_count(),
            proof.dependency_count()
        ])
    })
}

fn completion_proof_from(
    value: &JsonValue,
) -> io::Result<Option<NativeAdmissionCompletionProofV1>> {
    if value.is_null() {
        return Ok(None);
    }
    let fields = value
        .as_array()
        .filter(|fields| fields.len() == 7)
        .ok_or_else(|| invalid("native completion proof tuple shape differs"))?;
    let schema = fields[3]
        .as_array()
        .filter(|fields| fields.len() == 4)
        .ok_or_else(|| invalid("native completion schema tuple shape differs"))?;
    let schema_profile = match schema[0].as_str() {
        Some("tos.schema.format.legacy-python-observed-20260923") => {
            FormatProfile::LegacyPythonObserved20260923
        }
        Some("tos.schema.format.asserted-source-candidate-v1") => {
            FormatProfile::AssertedSourceCandidateV1
        }
        _ => return Err(invalid("native completion schema profile differs")),
    };
    let protocol_fields = schema[3]
        .as_array()
        .filter(|fields| fields.len() == 2)
        .ok_or_else(|| invalid("native completion protocol tuple shape differs"))?;
    let protocol = match (protocol_fields[0].as_str(), protocol_fields[1].is_null()) {
        (Some("legacy-scalar"), true) => CutPreparedSchemaProtocol::LegacyScalar,
        (Some("diagnostics-v2"), false) => CutPreparedSchemaProtocol::DiagnosticsV2 {
            caps_sha256: digest(&protocol_fields[1])?,
        },
        _ => return Err(invalid("native completion schema protocol differs")),
    };
    NativeAdmissionCompletionProofV1::from_authenticated_root_fields(
        digest(&fields[0])?,
        membership_v1_from(&fields[1])?,
        number(&fields[2])?,
        CutPreparedSchemaExecutionBinding {
            schema_profile,
            schema_set_sha256: digest(&schema[1])?,
            worker_sha256: digest(&schema[2])?,
            protocol,
        },
        number(&fields[4])?,
        number(&fields[5])?,
        number(&fields[6])?,
    )
    .map(Some)
}

fn tree_bytes(tree: &AuthenticatedTreeDescriptorV2) -> io::Result<Vec<u8>> {
    tree.encode(TREE_DESCRIPTOR_MAX_BYTES)
        .map_err(|_| invalid("source rootset tree descriptor exceeds profile"))
}

pub fn tree_retained_state_bytes(tree: &AuthenticatedTreeDescriptorV2) -> io::Result<usize> {
    let mut state = size_of::<AuthenticatedTreeDescriptorV2>()
        .checked_add(tree.kind.capacity())
        .ok_or_else(|| invalid("V2 tree descriptor state overflow"))?;
    if let Some(root) = &tree.root {
        state = state
            .checked_add(size_of::<tos_segment_store::AuthenticatedTreeNodeRefV1>())
            .and_then(|n| n.checked_add(root.min_key.capacity()))
            .and_then(|n| n.checked_add(root.max_key.capacity()))
            .ok_or_else(|| invalid("V2 tree descriptor state overflow"))?;
    }
    if tree.physical_root.is_some() {
        state = state
            .checked_add(size_of::<tos_segment_store::AuthenticatedTreeLocatorV2>())
            .ok_or_else(|| invalid("V2 tree locator state overflow"))?;
    }
    Ok(state)
}

pub fn segment_store_retained_state_bytes(segment: &SegmentStore) -> io::Result<usize> {
    size_of::<SegmentStore>()
        .checked_add(
            segment
                .retained_heap_state_bytes()
                .map_err(segment_io_error)?,
        )
        .ok_or_else(|| invalid("V2 segment store retained state overflow"))
}

fn descriptor_wire_len(tree: &AuthenticatedTreeDescriptorV2) -> io::Result<usize> {
    let semantic_len = 8usize
        .checked_add(2 + 2 + 16 + 32 + 2)
        .and_then(|n| n.checked_add(tree.kind.len()))
        .and_then(|n| n.checked_add(8 + 1 + 32))
        .and_then(|n| {
            tree.root.as_ref().map_or(Some(n), |root| {
                n.checked_add(32 + 8 + 4 + root.min_key.len() + 4 + root.max_key.len())
            })
        })
        .ok_or_else(|| invalid("V2 descriptor wire length overflow"))?;
    if tree.physical_root.is_some() {
        8usize
            .checked_add(2 + 2 + 4 + 92)
            .and_then(|n| n.checked_add(semantic_len))
            .ok_or_else(|| invalid("V2 packed descriptor wire length overflow"))
    } else {
        Ok(semantic_len)
    }
}

fn descriptor_json_workspace_upper_bound(
    trees: &[&AuthenticatedTreeDescriptorV2],
    retained_state_bytes: usize,
    additional_live_bytes: usize,
) -> io::Result<usize> {
    // A descriptor becomes a JSON byte array. Bound simultaneously retained
    // encoded descriptor bytes, Value elements (with 2x Vec capacity slack),
    // and compact output (three digits plus a separator per byte, again with
    // allocator-growth slack). The fixed term covers tuple/object nodes and
    // short schema/digest strings. All terms are computed before `wire_value`
    // constructs serde Values or the output Vec.
    let descriptor_bytes = trees.iter().try_fold(0usize, |total, tree| {
        total
            .checked_add(descriptor_wire_len(tree)?)
            .filter(|bytes| *bytes <= trees.len().saturating_mul(TREE_DESCRIPTOR_MAX_BYTES))
            .ok_or_else(|| invalid("V2 descriptor workspace overflow"))
    })?;
    let descriptor_value_bytes = descriptor_bytes
        .checked_mul(size_of::<serde_json::Value>())
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| invalid("V2 descriptor Value workspace overflow"))?;
    let descriptor_copy_bytes = descriptor_bytes
        .checked_mul(2)
        .ok_or_else(|| invalid("V2 descriptor copy workspace overflow"))?;
    let output_bytes = descriptor_bytes
        .checked_mul(4)
        .and_then(|n| n.checked_add(ENCODE_WORKSPACE_FIXED_OVERHEAD))
        .ok_or_else(|| invalid("V2 encoded output workspace overflow"))?;
    let output_capacity = output_bytes
        .checked_mul(2)
        .ok_or_else(|| invalid("V2 output capacity workspace overflow"))?;
    retained_state_bytes
        .checked_add(additional_live_bytes)
        .and_then(|n| n.checked_add(descriptor_value_bytes))
        .and_then(|n| n.checked_add(descriptor_copy_bytes))
        .and_then(|n| n.checked_add(output_capacity))
        .and_then(|n| n.checked_add(ENCODE_WORKSPACE_FIXED_OVERHEAD))
        .ok_or_else(|| invalid("V2 encoded workspace bound overflow"))
}

fn tree(value: &JsonValue) -> io::Result<AuthenticatedTreeDescriptorV2> {
    let fields = value
        .as_array()
        .filter(|fields| fields.len() <= TREE_DESCRIPTOR_MAX_BYTES)
        .ok_or_else(|| invalid("source rootset tree descriptor is not a bounded byte array"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(fields.len())
        .map_err(|_| invalid("source rootset tree descriptor allocation failed"))?;
    for field in fields {
        bytes.push(
            u8::try_from(number(field)?)
                .map_err(|_| invalid("source rootset tree descriptor byte is out of range"))?,
        );
    }
    AuthenticatedTreeDescriptorV2::decode(&bytes, TREE_DESCRIPTOR_MAX_BYTES)
        .map_err(|_| invalid("source rootset tree descriptor is invalid"))
}

/// The file format retained for one immutable source revision. The legacy
/// variant exists only to decode old rootsets and preserves their original
/// scalar wire field exactly; it is never emitted by a new writer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceRevisionArtifactV2 {
    LegacyManifestV1 { sha256: Digest256 },
    SnapshotV1 { sha256: Digest256, bytes: u64 },
    CompactCommitV2 { sha256: Digest256, bytes: u64 },
    CompactPackedV2 { sha256: Digest256, bytes: u64 },
}

impl SourceRevisionArtifactV2 {
    pub fn format(&self) -> &'static str {
        match self {
            Self::LegacyManifestV1 { .. } => "legacy-manifest-v1",
            Self::SnapshotV1 { .. } => "tos-corpus-snapshot-v1",
            Self::CompactCommitV2 { .. } => "tos-native-source-compact-commit-v2",
            Self::CompactPackedV2 { .. } => "tos-native-source-compact-commit-packed-v2",
        }
    }

    pub fn sha256(&self) -> Digest256 {
        match self {
            Self::LegacyManifestV1 { sha256 }
            | Self::SnapshotV1 { sha256, .. }
            | Self::CompactCommitV2 { sha256, .. }
            | Self::CompactPackedV2 { sha256, .. } => *sha256,
        }
    }

    pub fn bytes(&self) -> Option<u64> {
        match self {
            Self::LegacyManifestV1 { .. } => None,
            Self::SnapshotV1 { bytes, .. }
            | Self::CompactCommitV2 { bytes, .. }
            | Self::CompactPackedV2 { bytes, .. } => Some(*bytes),
        }
    }

    pub fn filename(&self) -> &'static str {
        match self {
            Self::LegacyManifestV1 { .. } | Self::SnapshotV1 { .. } => "snapshot.json",
            Self::CompactCommitV2 { .. } => "commit-v2.json",
            Self::CompactPackedV2 { .. } => "commit-packed-v2.json",
        }
    }

    fn wire_value(&self) -> serde_json::Value {
        match self {
            Self::LegacyManifestV1 { sha256 } => serde_json::json!(sha256.to_hex()),
            Self::SnapshotV1 { sha256, bytes } => {
                serde_json::json!(["snapshot-v1", sha256.to_hex(), bytes])
            }
            Self::CompactCommitV2 { sha256, bytes } => {
                serde_json::json!(["compact-commit-v2", sha256.to_hex(), bytes])
            }
            Self::CompactPackedV2 { sha256, bytes } => {
                serde_json::json!(["compact-packed-v2", sha256.to_hex(), bytes])
            }
        }
    }

    fn from_typed_wire(value: &JsonValue) -> io::Result<Self> {
        let fields = value
            .as_array()
            .filter(|fields| fields.len() == 3)
            .ok_or_else(|| invalid("source revision artifact tuple shape differs"))?;
        let format = fields[0]
            .as_str()
            .ok_or_else(|| invalid("source revision artifact format is not text"))?;
        let sha256 = digest(&fields[1])?;
        let bytes = number(&fields[2])?;
        if bytes == 0 {
            return Err(invalid("source revision artifact byte length is zero"));
        }
        match format {
            "snapshot-v1" => Ok(Self::SnapshotV1 { sha256, bytes }),
            "compact-commit-v2" if bytes <= MAX_COMPACT_COMMIT_V2_BYTES as u64 => {
                Ok(Self::CompactCommitV2 { sha256, bytes })
            }
            "compact-commit-v2" => Err(invalid("compact source commit exceeds its byte profile")),
            "compact-packed-v2" if bytes <= MAX_COMPACT_COMMIT_V2_BYTES as u64 => {
                Ok(Self::CompactPackedV2 { sha256, bytes })
            }
            "compact-packed-v2" => Err(invalid("packed compact commit exceeds its byte profile")),
            _ => Err(invalid("source revision artifact format is unsupported")),
        }
    }
}

/// One revision's current persistent roots. V1 membership remains its exact
/// historical digest; V2 tree commitments are separately named descriptors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRevisionRootsV2 {
    pub revision: SourceRevision,
    pub base_revision: Option<SourceRevision>,
    pub validator_sha256: Digest256,
    /// Compatibility alias for existing CMD receipts. It always equals the
    /// typed source artifact's digest, not necessarily a V1 manifest digest.
    pub manifest_sha256: Digest256,
    pub source_artifact: SourceRevisionArtifactV2,
    pub batch_sha256: Option<Digest256>,
    /// The V1 ordered-row digest is retained when available. New addressed
    /// successor writes may omit it and use the separately versioned V2
    /// authenticated-tree commitment instead.
    pub membership_v1: Option<SourceMembershipV1>,
    /// Present in V3 root tuples. Old roots decode with `None` and can derive
    /// a read-only equivalent from their already authenticated member root.
    pub membership_v2: Option<SourceMembershipV2>,
    /// Proof issued only by a genuine sealed full native IndexView, or by a
    /// bounded successor derived from that proof and an exact addressed delta.
    /// The optional field is committed by the history tuple and source revision.
    pub completion_proof: Option<NativeAdmissionCompletionProofV1>,
    pub source_bytes: u64,
    pub member_count: u64,
    pub identity_count: u64,
    pub dependency_source_count: u64,
    pub dependency_count: u64,
    pub retirement_count: u64,
    pub members: AuthenticatedTreeDescriptorV2,
    pub identities: AuthenticatedTreeDescriptorV2,
    pub dependencies: AuthenticatedTreeDescriptorV2,
    pub retirements: AuthenticatedTreeDescriptorV2,
    /// None preserves the exact historical digest-file interpretation. A
    /// packed root is authenticated by this revision, never a lookup cache.
    pub objects: Option<AuthenticatedTreeDescriptorV2>,
    /// Optional on legacy roots so their exact historical wire remains
    /// decodable. Newly written V2 roots carry the path-keyed reverse identity
    /// index used by bounded warm reads.
    pub identity_paths: Option<AuthenticatedTreeDescriptorV2>,
}

impl SourceRevisionRootsV2 {
    pub const MAX_ENCODED_BYTES: usize = ROOTSET_MAX_BYTES;

    /// Return the V2 membership commitment declared by this root, or derive
    /// the equivalent read-only value for a legacy root. The derivation is
    /// bound to the authenticated members descriptor and exact counts; it is
    /// never exposed as a V1 ordered-row digest.
    pub fn source_membership_v2(&self) -> Option<SourceMembershipV2> {
        self.membership_v2.or_else(|| {
            Some(SourceMembershipV2::from_members_tree(
                self.member_count,
                self.source_bytes,
                self.members.commitment,
            ))
        })
    }

    pub fn retained_state_bytes(&self) -> io::Result<usize> {
        {
            let (roots, count) = revision_descriptor_refs(
                &self.members,
                &self.identities,
                &self.dependencies,
                &self.retirements,
                self.objects.as_ref(),
                self.identity_paths.as_ref(),
            );
            roots[..count]
                .iter()
                .try_fold(size_of::<Self>(), |total, root| {
                    total
                        .checked_add(tree_retained_state_bytes(root)?)
                        .ok_or_else(|| invalid("revision descriptor state overflow"))
                })
        }
    }

    pub fn encode_state_upper_bound(&self, additional_live_bytes: usize) -> io::Result<usize> {
        let (roots, count) = revision_descriptor_refs(
            &self.members,
            &self.identities,
            &self.dependencies,
            &self.retirements,
            self.objects.as_ref(),
            self.identity_paths.as_ref(),
        );
        descriptor_json_workspace_upper_bound(
            &roots[..count],
            self.retained_state_bytes()?,
            additional_live_bytes,
        )
    }

    pub fn retained_state_upper_bound_for_value(max_value_bytes: usize) -> io::Result<usize> {
        max_value_bytes
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(size_of::<Self>() + 4096))
            .ok_or_else(|| invalid("V2 root result state overflow"))
    }

    pub fn validate_store_binding(
        &self,
        store_id: [u8; 16],
        domain_digest: Digest256,
    ) -> io::Result<()> {
        self.validate_store_content_binding(store_id, domain_digest)?;
        if self.base_revision == Some(self.revision) {
            return Err(invalid("source revision cannot be its own base"));
        }
        Ok(())
    }

    /// Validate all authenticated roots and logical content fields while a
    /// successor revision is still provisional. The source revision is
    /// intentionally not checked until the compact record derives its actual
    /// value from the batch and these roots.
    pub fn validate_store_content_binding(
        &self,
        store_id: [u8; 16],
        domain_digest: Digest256,
    ) -> io::Result<()> {
        let v1_count_matches = self
            .membership_v1
            .is_none_or(|membership| membership.count == self.member_count);
        let v2_matches = self.membership_v2.is_none_or(|membership| {
            membership.count == self.member_count
                && membership.source_bytes == self.source_bytes
                && membership.members_tree_commitment == self.members.commitment
                && membership
                    == SourceMembershipV2::from_members_tree(
                        self.member_count,
                        self.source_bytes,
                        self.members.commitment,
                    )
        });
        let proof_matches = self.completion_proof.is_none_or(|proof| {
            proof.validator_sha256() == self.validator_sha256
                && proof.source_bytes() == self.source_bytes
                && proof.identity_count() == self.identity_count
                && proof.dependency_source_count() == self.dependency_source_count
                && proof.dependency_count() == self.dependency_count
                && proof
                    .membership_v1()
                    .is_none_or(|membership| self.membership_v1 == Some(membership))
        });
        if self.manifest_sha256 != self.source_artifact.sha256()
            || !v1_count_matches
            || !v2_matches
            || !proof_matches
            || (self.membership_v1.is_none() && self.membership_v2.is_none())
            || self.dependency_source_count > self.dependency_count
            || (self.dependency_source_count == 0) != (self.dependency_count == 0)
        {
            return Err(invalid("source revision root logical counts differ"));
        }
        match (&self.source_artifact, self.base_revision, self.batch_sha256) {
            (SourceRevisionArtifactV2::LegacyManifestV1 { .. }, _, None) => (),
            (SourceRevisionArtifactV2::SnapshotV1 { bytes, .. }, None, Some(_)) if *bytes > 0 => (),
            (SourceRevisionArtifactV2::CompactCommitV2 { bytes, .. }, Some(_), Some(_))
                if self.objects.is_none()
                    && *bytes > 0
                    && *bytes <= MAX_COMPACT_COMMIT_V2_BYTES as u64 =>
            {
                ()
            }
            (SourceRevisionArtifactV2::CompactPackedV2 { bytes, .. }, Some(_), Some(_))
                if self.objects.is_some()
                    && *bytes > 0
                    && *bytes <= MAX_COMPACT_COMMIT_V2_BYTES as u64 =>
            {
                ()
            }
            _ => return Err(invalid("source revision artifact and batch binding differ")),
        }
        if let Some(objects) = &self.objects {
            if matches!(
                self.source_artifact,
                SourceRevisionArtifactV2::LegacyManifestV1 { .. }
            ) {
                return Err(invalid(
                    "legacy revision cannot select a packed object root",
                ));
            }
            // The extent index belongs to this revision's reachable source
            // closure. Historical revisions bind their own index; they do not
            // authorize an unrelated accumulating object catalogue here.
            let reachable_slots = self
                .retirement_count
                .checked_mul(2)
                .and_then(|slots| slots.checked_add(self.member_count))
                .ok_or_else(|| invalid("packed revision object count overflow"))?;
            if objects.entries > reachable_slots || (objects.entries == 0) != (reachable_slots == 0)
            {
                return Err(invalid("packed revision object closure count differs"));
            }
            validate_tree_binding(
                objects,
                store_id,
                domain_digest,
                OBJECT_EXTENTS_KIND,
                objects.entries,
            )?;
        }
        if let Some(identity_paths) = &self.identity_paths {
            validate_tree_binding(
                identity_paths,
                store_id,
                domain_digest,
                IDENTITY_PATHS_KIND,
                self.identity_count,
            )?;
        }
        for (root, kind, count) in [
            (&self.members, MEMBERS_KIND, self.member_count),
            (&self.identities, IDENTITIES_KIND, self.identity_count),
            (&self.dependencies, DEPENDENCIES_KIND, self.dependency_count),
            (&self.retirements, RETIREMENTS_KIND, self.retirement_count),
        ] {
            validate_tree_binding(root, store_id, domain_digest, kind, count)?;
        }
        Ok(())
    }

    /// Canonical tuple stored as an authenticated history-tree value. The
    /// caller still owns the shared state allowance for these bounded bytes.
    pub fn encode_with_state_limit(
        &self,
        max_state_bytes: usize,
        additional_live_bytes: usize,
    ) -> io::Result<Vec<u8>> {
        if self.encode_state_upper_bound(additional_live_bytes)? > max_state_bytes {
            return Err(invalid("source revision encoding exceeds reserved state"));
        }
        let raw = serde_json::to_vec(&self.wire_value()?)
            .map_err(|_| invalid("source revision root serialization failed"))?;
        if raw.is_empty() || raw.len() > ROOTSET_MAX_BYTES {
            return Err(invalid("source revision root byte profile exceeded"));
        }
        Ok(raw)
    }

    pub fn decode(raw: &[u8]) -> io::Result<Self> {
        let workspace = decode_workspace_upper_bound(raw.len())?;
        Self::decode_with_workspace(raw, workspace)
    }

    /// Decode with the caller's already reserved transient state allowance.
    /// The Foundation parser charges each allocation before it is made, even
    /// when malformed nested input will later fail the fixed tuple shape.
    pub fn decode_with_workspace(raw: &[u8], workspace: usize) -> io::Result<Self> {
        if raw.is_empty() || raw.len() > ROOTSET_MAX_BYTES {
            return Err(invalid("source revision root byte profile exceeded"));
        }
        let (document, _) = parse_bounded_json(raw, ROOTSET_MAX_BYTES, workspace)?;
        Self::from_wire(document.root())
    }

    fn wire_value(&self) -> io::Result<serde_json::Value> {
        self.validate_store_binding(self.members.store_id, self.members.domain_digest)?;
        let legacy = matches!(
            self.source_artifact,
            SourceRevisionArtifactV2::LegacyManifestV1 { .. }
        );
        if self.membership_v2.is_some() && self.completion_proof.is_some() {
            let value = serde_json::json!([
                match (self.objects.is_some(), self.identity_paths.is_some()) {
                    (true, true) => PACKED_IDENTITY_PATHS_REVISION_ROOT_V4_SCHEMA,
                    (true, false) => PACKED_REVISION_ROOT_V4_SCHEMA,
                    (false, true) => IDENTITY_PATHS_REVISION_ROOT_V4_SCHEMA,
                    (false, false) => TYPED_REVISION_ROOT_V4_SCHEMA,
                },
                self.revision.0.to_hex(),
                self.base_revision.map(|revision| revision.0.to_hex()),
                self.validator_sha256.to_hex(),
                self.source_artifact.wire_value(),
                self.batch_sha256
                    .ok_or_else(|| invalid("typed source revision lacks its batch digest"))?
                    .to_hex(),
                membership_v1_value(self.membership_v1),
                membership_v2_value(self.membership_v2),
                completion_proof_value(self.completion_proof),
                self.source_bytes,
                self.member_count,
                self.identity_count,
                self.dependency_source_count,
                self.dependency_count,
                self.retirement_count,
                tree_bytes(&self.members)?,
                tree_bytes(&self.identities)?,
                tree_bytes(&self.dependencies)?,
                tree_bytes(&self.retirements)?
            ]);
            return append_storage_descriptors(
                value,
                self.objects.as_ref(),
                self.identity_paths.as_ref(),
            );
        }
        if self.completion_proof.is_some() {
            return Err(invalid("native completion proof requires V2 membership"));
        }
        if self.membership_v2.is_some() {
            if legacy {
                let value = serde_json::json!([
                    if self.identity_paths.is_some() {
                        LEGACY_IDENTITY_PATHS_REVISION_ROOT_V3_SCHEMA
                    } else {
                        LEGACY_REVISION_ROOT_V3_SCHEMA
                    },
                    self.revision.0.to_hex(),
                    self.base_revision.map(|revision| revision.0.to_hex()),
                    self.validator_sha256.to_hex(),
                    self.manifest_sha256.to_hex(),
                    membership_v1_value(self.membership_v1),
                    membership_v2_value(self.membership_v2),
                    self.source_bytes,
                    self.member_count,
                    self.identity_count,
                    self.dependency_source_count,
                    self.dependency_count,
                    self.retirement_count,
                    tree_bytes(&self.members)?,
                    tree_bytes(&self.identities)?,
                    tree_bytes(&self.dependencies)?,
                    tree_bytes(&self.retirements)?
                ]);
                return append_storage_descriptors(value, None, self.identity_paths.as_ref());
            }
            return append_storage_descriptors(
                serde_json::json!([
                    match (self.objects.is_some(), self.identity_paths.is_some()) {
                        (true, true) => PACKED_IDENTITY_PATHS_REVISION_ROOT_V3_SCHEMA,
                        (true, false) => PACKED_REVISION_ROOT_V3_SCHEMA,
                        (false, true) => IDENTITY_PATHS_REVISION_ROOT_V3_SCHEMA,
                        (false, false) => TYPED_REVISION_ROOT_V3_SCHEMA,
                    },
                    self.revision.0.to_hex(),
                    self.base_revision.map(|revision| revision.0.to_hex()),
                    self.validator_sha256.to_hex(),
                    self.source_artifact.wire_value(),
                    self.batch_sha256
                        .ok_or_else(|| invalid("typed source revision lacks its batch digest"))?
                        .to_hex(),
                    membership_v1_value(self.membership_v1),
                    membership_v2_value(self.membership_v2),
                    self.source_bytes,
                    self.member_count,
                    self.identity_count,
                    self.dependency_source_count,
                    self.dependency_count,
                    self.retirement_count,
                    tree_bytes(&self.members)?,
                    tree_bytes(&self.identities)?,
                    tree_bytes(&self.dependencies)?,
                    tree_bytes(&self.retirements)?
                ]),
                self.objects.as_ref(),
                self.identity_paths.as_ref(),
            );
        }
        let membership_v1 = self
            .membership_v1
            .ok_or_else(|| invalid("legacy source root lacks its V1 membership digest"))?;
        if matches!(
            self.source_artifact,
            SourceRevisionArtifactV2::LegacyManifestV1 { .. }
        ) {
            let value = serde_json::json!([
                if self.identity_paths.is_some() {
                    LEGACY_IDENTITY_PATHS_REVISION_ROOT_SCHEMA
                } else {
                    LEGACY_REVISION_ROOT_SCHEMA
                },
                self.revision.0.to_hex(),
                self.base_revision.map(|revision| revision.0.to_hex()),
                self.validator_sha256.to_hex(),
                self.manifest_sha256.to_hex(),
                membership_v1.count,
                membership_v1.digest.to_hex(),
                self.source_bytes,
                self.member_count,
                self.identity_count,
                self.dependency_source_count,
                self.dependency_count,
                self.retirement_count,
                tree_bytes(&self.members)?,
                tree_bytes(&self.identities)?,
                tree_bytes(&self.dependencies)?,
                tree_bytes(&self.retirements)?
            ]);
            return append_storage_descriptors(value, None, self.identity_paths.as_ref());
        }
        append_storage_descriptors(
            serde_json::json!([
                match (self.objects.is_some(), self.identity_paths.is_some()) {
                    (true, true) => PACKED_IDENTITY_PATHS_REVISION_ROOT_SCHEMA,
                    (true, false) => PACKED_REVISION_ROOT_SCHEMA,
                    (false, true) => IDENTITY_PATHS_REVISION_ROOT_SCHEMA,
                    (false, false) => TYPED_REVISION_ROOT_SCHEMA,
                },
                self.revision.0.to_hex(),
                self.base_revision.map(|revision| revision.0.to_hex()),
                self.validator_sha256.to_hex(),
                self.source_artifact.wire_value(),
                self.batch_sha256
                    .ok_or_else(|| invalid("typed source revision lacks its batch digest"))?
                    .to_hex(),
                membership_v1.count,
                membership_v1.digest.to_hex(),
                self.source_bytes,
                self.member_count,
                self.identity_count,
                self.dependency_source_count,
                self.dependency_count,
                self.retirement_count,
                tree_bytes(&self.members)?,
                tree_bytes(&self.identities)?,
                tree_bytes(&self.dependencies)?,
                tree_bytes(&self.retirements)?
            ]),
            self.objects.as_ref(),
            self.identity_paths.as_ref(),
        )
    }

    fn from_wire(value: &JsonValue) -> io::Result<Self> {
        let fields = value
            .as_array()
            .ok_or_else(|| invalid("source revision root tuple shape differs"))?;
        let schema = fields.first().and_then(JsonValue::as_str);
        let legacy = schema == Some(LEGACY_REVISION_ROOT_SCHEMA) && fields.len() == 17;
        let legacy_identity_paths =
            schema == Some(LEGACY_IDENTITY_PATHS_REVISION_ROOT_SCHEMA) && fields.len() == 18;
        let typed = schema == Some(TYPED_REVISION_ROOT_SCHEMA) && fields.len() == 18;
        let packed = schema == Some(PACKED_REVISION_ROOT_SCHEMA) && fields.len() == 19;
        let typed_identity_paths =
            schema == Some(IDENTITY_PATHS_REVISION_ROOT_SCHEMA) && fields.len() == 19;
        let packed_identity_paths =
            schema == Some(PACKED_IDENTITY_PATHS_REVISION_ROOT_SCHEMA) && fields.len() == 20;
        let legacy_v3 = schema == Some(LEGACY_REVISION_ROOT_V3_SCHEMA) && fields.len() == 17;
        let legacy_identity_paths_v3 =
            schema == Some(LEGACY_IDENTITY_PATHS_REVISION_ROOT_V3_SCHEMA) && fields.len() == 18;
        let typed_v3 = schema == Some(TYPED_REVISION_ROOT_V3_SCHEMA) && fields.len() == 18;
        let packed_v3 = schema == Some(PACKED_REVISION_ROOT_V3_SCHEMA) && fields.len() == 19;
        let typed_identity_paths_v3 =
            schema == Some(IDENTITY_PATHS_REVISION_ROOT_V3_SCHEMA) && fields.len() == 19;
        let packed_identity_paths_v3 =
            schema == Some(PACKED_IDENTITY_PATHS_REVISION_ROOT_V3_SCHEMA) && fields.len() == 20;
        let typed_v4 = schema == Some(TYPED_REVISION_ROOT_V4_SCHEMA) && fields.len() == 19;
        let packed_v4 = schema == Some(PACKED_REVISION_ROOT_V4_SCHEMA) && fields.len() == 20;
        let typed_identity_paths_v4 =
            schema == Some(IDENTITY_PATHS_REVISION_ROOT_V4_SCHEMA) && fields.len() == 20;
        let packed_identity_paths_v4 =
            schema == Some(PACKED_IDENTITY_PATHS_REVISION_ROOT_V4_SCHEMA) && fields.len() == 21;
        if !legacy
            && !legacy_identity_paths
            && !typed
            && !packed
            && !typed_identity_paths
            && !packed_identity_paths
            && !legacy_v3
            && !legacy_identity_paths_v3
            && !typed_v3
            && !packed_v3
            && !typed_identity_paths_v3
            && !packed_identity_paths_v3
            && !typed_v4
            && !packed_v4
            && !typed_identity_paths_v4
            && !packed_identity_paths_v4
        {
            return Err(invalid("source revision root version or shape differs"));
        }
        let is_legacy = legacy || legacy_identity_paths || legacy_v3 || legacy_identity_paths_v3;
        let has_v3_membership = legacy_v3
            || legacy_identity_paths_v3
            || typed_v3
            || packed_v3
            || typed_identity_paths_v3
            || packed_identity_paths_v3
            || typed_v4
            || packed_v4
            || typed_identity_paths_v4
            || packed_identity_paths_v4;
        let has_v4_proof =
            typed_v4 || packed_v4 || typed_identity_paths_v4 || packed_identity_paths_v4;
        let has_objects = packed
            || packed_identity_paths
            || packed_v3
            || packed_identity_paths_v3
            || packed_v4
            || packed_identity_paths_v4;
        let has_identity_paths = legacy_identity_paths
            || typed_identity_paths
            || packed_identity_paths
            || legacy_identity_paths_v3
            || typed_identity_paths_v3
            || packed_identity_paths_v3
            || typed_identity_paths_v4
            || packed_identity_paths_v4;
        let (artifact, batch_sha256, membership_index) = if is_legacy {
            let sha256 = digest(&fields[4])?;
            (
                SourceRevisionArtifactV2::LegacyManifestV1 { sha256 },
                None,
                5,
            )
        } else {
            (
                SourceRevisionArtifactV2::from_typed_wire(&fields[4])?,
                Some(digest(&fields[5])?),
                6,
            )
        };
        let tree_index = membership_index + 8 + usize::from(has_v4_proof);
        let result = Self {
            revision: SourceRevision(digest(&fields[1])?),
            base_revision: optional_revision(&fields[2])?,
            validator_sha256: digest(&fields[3])?,
            manifest_sha256: artifact.sha256(),
            source_artifact: artifact,
            batch_sha256,
            membership_v1: if has_v3_membership {
                membership_v1_from(&fields[membership_index])?
            } else {
                Some(SourceMembershipV1 {
                    count: number(&fields[membership_index])?,
                    digest: digest(&fields[membership_index + 1])?,
                })
            },
            membership_v2: if has_v3_membership {
                membership_v2_from(&fields[membership_index + 1])?
            } else {
                None
            },
            completion_proof: if has_v4_proof {
                completion_proof_from(&fields[membership_index + 2])?
            } else {
                None
            },
            source_bytes: number(&fields[membership_index + 2 + usize::from(has_v4_proof)])?,
            member_count: number(&fields[membership_index + 3 + usize::from(has_v4_proof)])?,
            identity_count: number(&fields[membership_index + 4 + usize::from(has_v4_proof)])?,
            dependency_source_count: number(
                &fields[membership_index + 5 + usize::from(has_v4_proof)],
            )?,
            dependency_count: number(&fields[membership_index + 6 + usize::from(has_v4_proof)])?,
            retirement_count: number(&fields[membership_index + 7 + usize::from(has_v4_proof)])?,
            members: tree(&fields[tree_index])?,
            identities: tree(&fields[tree_index + 1])?,
            dependencies: tree(&fields[tree_index + 2])?,
            retirements: tree(&fields[tree_index + 3])?,
            objects: if has_objects {
                Some(tree(&fields[tree_index + 4])?)
            } else {
                None
            },
            identity_paths: if has_identity_paths {
                Some(tree(&fields[tree_index + 4 + usize::from(has_objects)])?)
            } else {
                None
            },
        };
        result.validate_store_binding(result.members.store_id, result.members.domain_digest)?;
        Ok(result)
    }
}

/// Compact successor metadata persisted beside the immutable COW roots.
/// Its revision is derived from the exact transaction identity and committed
/// roots; it contains no whole-corpus V1 manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactCommitV2 {
    pub revision: SourceRevision,
    pub base_revision: SourceRevision,
    pub validator_sha256: Digest256,
    pub batch_sha256: Digest256,
    pub membership_v1: Option<SourceMembershipV1>,
    pub membership_v2: Option<SourceMembershipV2>,
    pub completion_proof: Option<NativeAdmissionCompletionProofV1>,
    pub source_bytes: u64,
    pub member_count: u64,
    pub identity_count: u64,
    pub dependency_source_count: u64,
    pub dependency_count: u64,
    pub retirement_count: u64,
    pub members: AuthenticatedTreeDescriptorV2,
    pub identities: AuthenticatedTreeDescriptorV2,
    pub dependencies: AuthenticatedTreeDescriptorV2,
    pub retirements: AuthenticatedTreeDescriptorV2,
    /// None preserves the exact historical digest-file interpretation. A
    /// packed root is authenticated by this revision, never a lookup cache.
    pub objects: Option<AuthenticatedTreeDescriptorV2>,
    pub identity_paths: Option<AuthenticatedTreeDescriptorV2>,
}

impl CompactCommitV2 {
    pub fn seal_successor(
        mut roots: SourceRevisionRootsV2,
        batch_sha256: Digest256,
        max_state_bytes: usize,
        additional_live_bytes: usize,
    ) -> io::Result<(SourceRevisionRootsV2, Self, Vec<u8>)> {
        let base_revision = roots
            .base_revision
            .ok_or_else(|| invalid("compact source successor lacks its base revision"))?;
        let roots_retained = roots.retained_state_bytes()?;
        let (descriptor_array, descriptor_count) = revision_descriptor_refs(
            &roots.members,
            &roots.identities,
            &roots.dependencies,
            &roots.retirements,
            roots.objects.as_ref(),
            roots.identity_paths.as_ref(),
        );
        let descriptors = &descriptor_array[..descriptor_count];
        // Reserve the compact row workspace against the source descriptors
        // before cloning them into the record. The peak includes the original
        // roots and any retained caller state.
        let commit_retained =
            descriptors
                .iter()
                .try_fold(size_of::<Self>(), |total, descriptor| {
                    total
                        .checked_add(tree_retained_state_bytes(descriptor)?)
                        .ok_or_else(|| invalid("compact source commit state overflow"))
                })?;
        let encode_additional_live = roots_retained
            .checked_add(additional_live_bytes)
            .ok_or_else(|| invalid("compact successor retained state overflow"))?;
        if descriptor_json_workspace_upper_bound(
            &descriptors,
            commit_retained,
            encode_additional_live,
        )? > max_state_bytes
        {
            return Err(invalid("compact successor exceeds reserved state"));
        }
        let mut commit = Self {
            revision: roots.revision,
            base_revision,
            validator_sha256: roots.validator_sha256,
            batch_sha256,
            membership_v1: roots.membership_v1,
            membership_v2: roots.membership_v2,
            completion_proof: roots.completion_proof,
            source_bytes: roots.source_bytes,
            member_count: roots.member_count,
            identity_count: roots.identity_count,
            dependency_source_count: roots.dependency_source_count,
            dependency_count: roots.dependency_count,
            retirement_count: roots.retirement_count,
            members: roots.members.clone(),
            identities: roots.identities.clone(),
            dependencies: roots.dependencies.clone(),
            retirements: roots.retirements.clone(),
            objects: roots.objects.clone(),
            identity_paths: roots.identity_paths.clone(),
        };
        if commit.encode_state_upper_bound(encode_additional_live)? > max_state_bytes {
            return Err(invalid("compact successor exceeds reserved state"));
        }
        commit.revision = commit.derived_revision_precharged()?;
        let bytes = commit.encode_with_state_limit(max_state_bytes, encode_additional_live)?;
        let sha256 = Digest256::of_bytes(&bytes);
        roots.revision = commit.revision;
        roots.batch_sha256 = Some(batch_sha256);
        roots.manifest_sha256 = sha256;
        let record_bytes = u64::try_from(bytes.len())
            .map_err(|_| invalid("compact source commit length exceeds range"))?;
        roots.source_artifact = if roots.objects.is_some() {
            SourceRevisionArtifactV2::CompactPackedV2 {
                sha256,
                bytes: record_bytes,
            }
        } else {
            SourceRevisionArtifactV2::CompactCommitV2 {
                sha256,
                bytes: record_bytes,
            }
        };
        Ok((roots, commit, bytes))
    }

    fn retained_state_bytes(&self) -> io::Result<usize> {
        {
            let (roots, count) = revision_descriptor_refs(
                &self.members,
                &self.identities,
                &self.dependencies,
                &self.retirements,
                self.objects.as_ref(),
                self.identity_paths.as_ref(),
            );
            roots[..count]
                .iter()
                .try_fold(size_of::<Self>(), |total, root| {
                    total
                        .checked_add(tree_retained_state_bytes(root)?)
                        .ok_or_else(|| invalid("revision descriptor state overflow"))
                })
        }
    }

    fn encode_state_upper_bound(&self, additional_live_bytes: usize) -> io::Result<usize> {
        let (roots, count) = revision_descriptor_refs(
            &self.members,
            &self.identities,
            &self.dependencies,
            &self.retirements,
            self.objects.as_ref(),
            self.identity_paths.as_ref(),
        );
        descriptor_json_workspace_upper_bound(
            &roots[..count],
            self.retained_state_bytes()?,
            additional_live_bytes,
        )
    }

    fn root_fields(&self) -> io::Result<serde_json::Value> {
        if self.membership_v2.is_some() && self.completion_proof.is_some() {
            return append_storage_descriptors(
                serde_json::json!([
                    match (self.objects.is_some(), self.identity_paths.is_some()) {
                        (true, true) => PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V4_PREIMAGE_SCHEMA,
                        (true, false) => PACKED_COMPACT_COMMIT_V4_PREIMAGE_SCHEMA,
                        (false, true) => IDENTITY_PATHS_COMPACT_COMMIT_V4_PREIMAGE_SCHEMA,
                        (false, false) => COMPACT_COMMIT_V4_PREIMAGE_SCHEMA,
                    },
                    self.base_revision.0.to_hex(),
                    self.validator_sha256.to_hex(),
                    self.batch_sha256.to_hex(),
                    membership_v1_value(self.membership_v1),
                    membership_v2_value(self.membership_v2),
                    completion_proof_value(self.completion_proof),
                    self.source_bytes,
                    self.member_count,
                    self.identity_count,
                    self.dependency_source_count,
                    self.dependency_count,
                    self.retirement_count,
                    tree_bytes(&self.members)?,
                    tree_bytes(&self.identities)?,
                    tree_bytes(&self.dependencies)?,
                    tree_bytes(&self.retirements)?
                ]),
                self.objects.as_ref(),
                self.identity_paths.as_ref(),
            );
        }
        if self.membership_v2.is_some() {
            return append_storage_descriptors(
                serde_json::json!([
                    match (self.objects.is_some(), self.identity_paths.is_some()) {
                        (true, true) => PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V3_PREIMAGE_SCHEMA,
                        (true, false) => PACKED_COMPACT_COMMIT_V3_PREIMAGE_SCHEMA,
                        (false, true) => IDENTITY_PATHS_COMPACT_COMMIT_V3_PREIMAGE_SCHEMA,
                        (false, false) => COMPACT_COMMIT_V3_PREIMAGE_SCHEMA,
                    },
                    self.base_revision.0.to_hex(),
                    self.validator_sha256.to_hex(),
                    self.batch_sha256.to_hex(),
                    membership_v1_value(self.membership_v1),
                    membership_v2_value(self.membership_v2),
                    self.source_bytes,
                    self.member_count,
                    self.identity_count,
                    self.dependency_source_count,
                    self.dependency_count,
                    self.retirement_count,
                    tree_bytes(&self.members)?,
                    tree_bytes(&self.identities)?,
                    tree_bytes(&self.dependencies)?,
                    tree_bytes(&self.retirements)?
                ]),
                self.objects.as_ref(),
                self.identity_paths.as_ref(),
            );
        }
        let membership_v1 = self
            .membership_v1
            .ok_or_else(|| invalid("V2 compact commit lacks V1 membership"))?;
        append_storage_descriptors(
            serde_json::json!([
                match (self.objects.is_some(), self.identity_paths.is_some()) {
                    (true, true) => PACKED_IDENTITY_PATHS_COMPACT_PREIMAGE_SCHEMA,
                    (true, false) => "tos-native-source-compact-commit-packed-v2-preimage",
                    (false, true) => IDENTITY_PATHS_COMPACT_PREIMAGE_SCHEMA,
                    (false, false) => "tos-native-source-compact-commit-v2-preimage",
                },
                self.base_revision.0.to_hex(),
                self.validator_sha256.to_hex(),
                self.batch_sha256.to_hex(),
                membership_v1.count,
                membership_v1.digest.to_hex(),
                self.source_bytes,
                self.member_count,
                self.identity_count,
                self.dependency_source_count,
                self.dependency_count,
                self.retirement_count,
                tree_bytes(&self.members)?,
                tree_bytes(&self.identities)?,
                tree_bytes(&self.dependencies)?,
                tree_bytes(&self.retirements)?
            ]),
            self.objects.as_ref(),
            self.identity_paths.as_ref(),
        )
    }

    fn derived_revision_precharged(&self) -> io::Result<SourceRevision> {
        let v1_count_matches = self
            .membership_v1
            .is_none_or(|membership| membership.count == self.member_count);
        let v2_matches = self.membership_v2.is_none_or(|membership| {
            membership.count == self.member_count
                && membership.source_bytes == self.source_bytes
                && membership.members_tree_commitment == self.members.commitment
                && membership
                    == SourceMembershipV2::from_members_tree(
                        self.member_count,
                        self.source_bytes,
                        self.members.commitment,
                    )
        });
        let proof_matches = self.completion_proof.is_none_or(|proof| {
            proof.validator_sha256() == self.validator_sha256
                && proof.source_bytes() == self.source_bytes
                && proof.identity_count() == self.identity_count
                && proof.dependency_source_count() == self.dependency_source_count
                && proof.dependency_count() == self.dependency_count
                && proof
                    .membership_v1()
                    .is_none_or(|membership| self.membership_v1 == Some(membership))
        });
        if !v1_count_matches
            || !v2_matches
            || !proof_matches
            || (self.membership_v1.is_none() && self.membership_v2.is_none())
            || self.dependency_source_count > self.dependency_count
            || (self.dependency_source_count == 0) != (self.dependency_count == 0)
        {
            return Err(invalid("compact source commit logical counts differ"));
        }
        let preimage = serde_json::to_vec(&self.root_fields()?)
            .map_err(|_| invalid("compact source revision preimage failed"))?;
        let mut hasher = Digest256Hasher::new();
        let revision_domain = if self.completion_proof.is_some() {
            if self.membership_v2.is_none() {
                return Err(invalid("native completion proof requires V2 membership"));
            }
            match (self.objects.is_some(), self.identity_paths.is_some()) {
                (true, true) => PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V4_REVISION_DOMAIN,
                (true, false) => PACKED_COMPACT_COMMIT_V4_REVISION_DOMAIN,
                (false, true) => IDENTITY_PATHS_COMPACT_COMMIT_V4_REVISION_DOMAIN,
                (false, false) => COMPACT_COMMIT_V4_REVISION_DOMAIN,
            }
        } else {
            match (
                self.membership_v2.is_some(),
                self.objects.is_some(),
                self.identity_paths.is_some(),
            ) {
                (true, true, true) => PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V3_REVISION_DOMAIN,
                (true, true, false) => PACKED_COMPACT_COMMIT_V3_REVISION_DOMAIN,
                (true, false, true) => IDENTITY_PATHS_COMPACT_COMMIT_V3_REVISION_DOMAIN,
                (true, false, false) => COMPACT_COMMIT_V3_REVISION_DOMAIN,
                (false, true, true) => PACKED_IDENTITY_PATHS_COMPACT_REVISION_DOMAIN,
                (false, true, false) => PACKED_COMPACT_REVISION_DOMAIN,
                (false, false, true) => IDENTITY_PATHS_COMPACT_REVISION_DOMAIN,
                (false, false, false) => COMPACT_COMMIT_REVISION_DOMAIN,
            }
        };
        hasher.update(revision_domain);
        hasher.update(&preimage);
        Ok(SourceRevision(hasher.finalize()))
    }

    pub fn encode_with_state_limit(
        &self,
        max_state_bytes: usize,
        additional_live_bytes: usize,
    ) -> io::Result<Vec<u8>> {
        if self.encode_state_upper_bound(additional_live_bytes)? > max_state_bytes {
            return Err(invalid(
                "compact source commit encoding exceeds reserved state",
            ));
        }
        if self.derived_revision_precharged()? != self.revision {
            return Err(invalid("compact source revision derivation differs"));
        }
        let value = if self.membership_v2.is_some() && self.completion_proof.is_some() {
            append_storage_descriptors(
                serde_json::json!([
                    match (self.objects.is_some(), self.identity_paths.is_some()) {
                        (true, true) => PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V4_SCHEMA,
                        (true, false) => PACKED_COMPACT_COMMIT_V4_SCHEMA,
                        (false, true) => IDENTITY_PATHS_COMPACT_COMMIT_V4_SCHEMA,
                        (false, false) => COMPACT_COMMIT_V4_SCHEMA,
                    },
                    self.revision.0.to_hex(),
                    self.base_revision.0.to_hex(),
                    self.validator_sha256.to_hex(),
                    self.batch_sha256.to_hex(),
                    membership_v1_value(self.membership_v1),
                    membership_v2_value(self.membership_v2),
                    completion_proof_value(self.completion_proof),
                    self.source_bytes,
                    self.member_count,
                    self.identity_count,
                    self.dependency_source_count,
                    self.dependency_count,
                    self.retirement_count,
                    tree_bytes(&self.members)?,
                    tree_bytes(&self.identities)?,
                    tree_bytes(&self.dependencies)?,
                    tree_bytes(&self.retirements)?
                ]),
                self.objects.as_ref(),
                self.identity_paths.as_ref(),
            )?
        } else if self.membership_v2.is_some() {
            append_storage_descriptors(
                serde_json::json!([
                    match (self.objects.is_some(), self.identity_paths.is_some()) {
                        (true, true) => PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V3_SCHEMA,
                        (true, false) => PACKED_COMPACT_COMMIT_V3_SCHEMA,
                        (false, true) => IDENTITY_PATHS_COMPACT_COMMIT_V3_SCHEMA,
                        (false, false) => COMPACT_COMMIT_V3_SCHEMA,
                    },
                    self.revision.0.to_hex(),
                    self.base_revision.0.to_hex(),
                    self.validator_sha256.to_hex(),
                    self.batch_sha256.to_hex(),
                    membership_v1_value(self.membership_v1),
                    membership_v2_value(self.membership_v2),
                    self.source_bytes,
                    self.member_count,
                    self.identity_count,
                    self.dependency_source_count,
                    self.dependency_count,
                    self.retirement_count,
                    tree_bytes(&self.members)?,
                    tree_bytes(&self.identities)?,
                    tree_bytes(&self.dependencies)?,
                    tree_bytes(&self.retirements)?
                ]),
                self.objects.as_ref(),
                self.identity_paths.as_ref(),
            )?
        } else {
            let membership = self
                .membership_v1
                .ok_or_else(|| invalid("V2 compact commit lacks V1 membership"))?;
            append_storage_descriptors(
                serde_json::json!([
                    match (self.objects.is_some(), self.identity_paths.is_some()) {
                        (true, true) => PACKED_IDENTITY_PATHS_COMPACT_COMMIT_SCHEMA,
                        (true, false) => PACKED_COMPACT_COMMIT_SCHEMA,
                        (false, true) => IDENTITY_PATHS_COMPACT_COMMIT_SCHEMA,
                        (false, false) => COMPACT_COMMIT_SCHEMA,
                    },
                    self.revision.0.to_hex(),
                    self.base_revision.0.to_hex(),
                    self.validator_sha256.to_hex(),
                    self.batch_sha256.to_hex(),
                    membership.count,
                    membership.digest.to_hex(),
                    self.source_bytes,
                    self.member_count,
                    self.identity_count,
                    self.dependency_source_count,
                    self.dependency_count,
                    self.retirement_count,
                    tree_bytes(&self.members)?,
                    tree_bytes(&self.identities)?,
                    tree_bytes(&self.dependencies)?,
                    tree_bytes(&self.retirements)?
                ]),
                self.objects.as_ref(),
                self.identity_paths.as_ref(),
            )?
        };
        let raw = serde_json::to_vec(&value)
            .map_err(|_| invalid("compact source commit serialization failed"))?;
        if raw.is_empty() || raw.len() > MAX_COMPACT_COMMIT_V2_BYTES {
            return Err(invalid("compact source commit byte profile exceeded"));
        }
        Ok(raw)
    }

    pub fn decode(raw: &[u8]) -> io::Result<Self> {
        let workspace = decode_workspace_upper_bound(raw.len())?;
        Self::decode_with_workspace(raw, workspace)
    }

    pub fn decode_with_workspace(raw: &[u8], workspace: usize) -> io::Result<Self> {
        if raw.is_empty() || raw.len() > MAX_COMPACT_COMMIT_V2_BYTES {
            return Err(invalid("compact source commit byte profile exceeded"));
        }
        let (document, _) = parse_bounded_json(raw, MAX_COMPACT_COMMIT_V2_BYTES, workspace)?;
        let fields = document
            .root()
            .as_array()
            .ok_or_else(|| invalid("compact source commit tuple shape differs"))?;
        let schema = fields.first().and_then(JsonValue::as_str);
        let legacy = schema == Some(COMPACT_COMMIT_SCHEMA) && fields.len() == 17;
        let packed = schema == Some(PACKED_COMPACT_COMMIT_SCHEMA) && fields.len() == 18;
        let identity_paths =
            schema == Some(IDENTITY_PATHS_COMPACT_COMMIT_SCHEMA) && fields.len() == 18;
        let packed_identity_paths =
            schema == Some(PACKED_IDENTITY_PATHS_COMPACT_COMMIT_SCHEMA) && fields.len() == 19;
        let legacy_v3 = schema == Some(COMPACT_COMMIT_V3_SCHEMA) && fields.len() == 17;
        let packed_v3 = schema == Some(PACKED_COMPACT_COMMIT_V3_SCHEMA) && fields.len() == 18;
        let identity_paths_v3 =
            schema == Some(IDENTITY_PATHS_COMPACT_COMMIT_V3_SCHEMA) && fields.len() == 18;
        let packed_identity_paths_v3 =
            schema == Some(PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V3_SCHEMA) && fields.len() == 19;
        let legacy_v4 = schema == Some(COMPACT_COMMIT_V4_SCHEMA) && fields.len() == 18;
        let packed_v4 = schema == Some(PACKED_COMPACT_COMMIT_V4_SCHEMA) && fields.len() == 19;
        let identity_paths_v4 =
            schema == Some(IDENTITY_PATHS_COMPACT_COMMIT_V4_SCHEMA) && fields.len() == 19;
        let packed_identity_paths_v4 =
            schema == Some(PACKED_IDENTITY_PATHS_COMPACT_COMMIT_V4_SCHEMA) && fields.len() == 20;
        if !legacy
            && !packed
            && !identity_paths
            && !packed_identity_paths
            && !legacy_v3
            && !packed_v3
            && !identity_paths_v3
            && !packed_identity_paths_v3
            && !legacy_v4
            && !packed_v4
            && !identity_paths_v4
            && !packed_identity_paths_v4
        {
            return Err(invalid("compact source commit version differs"));
        }
        let has_v3_membership = legacy_v3
            || packed_v3
            || identity_paths_v3
            || packed_identity_paths_v3
            || legacy_v4
            || packed_v4
            || identity_paths_v4
            || packed_identity_paths_v4;
        let has_v4_proof = legacy_v4 || packed_v4 || identity_paths_v4 || packed_identity_paths_v4;
        let has_objects = packed
            || packed_identity_paths
            || packed_v3
            || packed_identity_paths_v3
            || packed_v4
            || packed_identity_paths_v4;
        let has_identity_paths = identity_paths
            || packed_identity_paths
            || identity_paths_v3
            || packed_identity_paths_v3
            || identity_paths_v4
            || packed_identity_paths_v4;
        let base_revision = optional_revision(&fields[2])?
            .ok_or_else(|| invalid("compact source commit base revision is absent"))?;
        let result = Self {
            revision: SourceRevision(digest(&fields[1])?),
            base_revision,
            validator_sha256: digest(&fields[3])?,
            batch_sha256: digest(&fields[4])?,
            membership_v1: if has_v3_membership {
                membership_v1_from(&fields[5])?
            } else {
                Some(SourceMembershipV1 {
                    count: number(&fields[5])?,
                    digest: digest(&fields[6])?,
                })
            },
            membership_v2: if has_v3_membership {
                membership_v2_from(&fields[6])?
            } else {
                None
            },
            completion_proof: if has_v4_proof {
                completion_proof_from(&fields[7])?
            } else {
                None
            },
            source_bytes: number(&fields[7 + usize::from(has_v4_proof)])?,
            member_count: number(&fields[8 + usize::from(has_v4_proof)])?,
            identity_count: number(&fields[9 + usize::from(has_v4_proof)])?,
            dependency_source_count: number(&fields[10 + usize::from(has_v4_proof)])?,
            dependency_count: number(&fields[11 + usize::from(has_v4_proof)])?,
            retirement_count: number(&fields[12 + usize::from(has_v4_proof)])?,
            members: tree(&fields[13 + usize::from(has_v4_proof)])?,
            identities: tree(&fields[14 + usize::from(has_v4_proof)])?,
            dependencies: tree(&fields[15 + usize::from(has_v4_proof)])?,
            retirements: tree(&fields[16 + usize::from(has_v4_proof)])?,
            objects: if has_objects {
                Some(tree(&fields[17 + usize::from(has_v4_proof)])?)
            } else {
                None
            },
            identity_paths: if has_identity_paths {
                Some(tree(
                    &fields[17 + usize::from(has_v4_proof) + usize::from(has_objects)],
                )?)
            } else {
                None
            },
        };
        if result.encode_state_upper_bound(raw.len())? > workspace {
            return Err(invalid(
                "compact source commit preimage exceeds decode workspace",
            ));
        }
        drop(document);
        if result.derived_revision_precharged()? != result.revision {
            return Err(invalid("compact source revision derivation differs"));
        }
        Ok(result)
    }

    pub fn matches_roots(&self, roots: &SourceRevisionRootsV2) -> bool {
        self.revision == roots.revision
            && Some(self.base_revision) == roots.base_revision
            && self.validator_sha256 == roots.validator_sha256
            && Some(self.batch_sha256) == roots.batch_sha256
            && self.membership_v1 == roots.membership_v1
            && self.completion_proof == roots.completion_proof
            && self.source_bytes == roots.source_bytes
            && self.member_count == roots.member_count
            && self.identity_count == roots.identity_count
            && self.dependency_source_count == roots.dependency_source_count
            && self.dependency_count == roots.dependency_count
            && self.retirement_count == roots.retirement_count
            && self.members == roots.members
            && self.identities == roots.identities
            && self.dependencies == roots.dependencies
            && self.retirements == roots.retirements
            && self.objects == roots.objects
            && self.identity_paths == roots.identity_paths
            && matches!(
                roots.source_artifact,
                SourceRevisionArtifactV2::CompactCommitV2 { .. }
                    | SourceRevisionArtifactV2::CompactPackedV2 { .. }
            )
    }
}

/// The one immutable selection object named by current.json. It binds all
/// current roots and the append-history root under a single CAS selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRootSetV2 {
    pub current: SourceRevisionRootsV2,
    pub history: AuthenticatedTreeDescriptorV2,
}

impl SourceRootSetV2 {
    pub fn retained_state_bytes(&self) -> io::Result<usize> {
        let state = size_of::<Self>()
            .checked_add(self.current.retained_state_bytes()?)
            .ok_or_else(|| invalid("V2 source rootset retained state overflow"))?;
        state
            .checked_add(tree_retained_state_bytes(&self.history)?)
            .ok_or_else(|| invalid("V2 source rootset retained state overflow"))
    }

    pub fn retained_state_upper_bound_for_value(max_value_bytes: usize) -> io::Result<usize> {
        SourceRevisionRootsV2::retained_state_upper_bound_for_value(max_value_bytes)?
            .checked_add(size_of::<Self>())
            .and_then(|bytes| bytes.checked_add(size_of::<AuthenticatedTreeDescriptorV2>()))
            .and_then(|bytes| bytes.checked_add(max_value_bytes.checked_mul(2)?))
            .and_then(|bytes| bytes.checked_add(TREE_DESCRIPTOR_MAX_BYTES + 4096))
            .ok_or_else(|| invalid("V2 source rootset state overflow"))
    }

    pub fn encode_state_upper_bound(&self, additional_live_bytes: usize) -> io::Result<usize> {
        let mut roots = [
            &self.current.members,
            &self.current.identities,
            &self.current.dependencies,
            &self.current.retirements,
            &self.history,
            &self.history,
            &self.history,
        ];
        let mut count = 4;
        if let Some(objects) = self.current.objects.as_ref() {
            roots[count] = objects;
            count += 1;
        }
        if let Some(identity_paths) = self.current.identity_paths.as_ref() {
            roots[count] = identity_paths;
            count += 1;
        }
        roots[count] = &self.history;
        count += 1;
        descriptor_json_workspace_upper_bound(
            &roots[..count],
            self.retained_state_bytes()?,
            additional_live_bytes,
        )
    }

    pub fn validate_store_binding(
        &self,
        store_id: [u8; 16],
        domain_digest: Digest256,
    ) -> io::Result<()> {
        self.current
            .validate_store_binding(store_id, domain_digest)?;
        if self.history.entries == 0 {
            return Err(invalid("source history root has no current revision"));
        }
        validate_tree_binding(
            &self.history,
            store_id,
            domain_digest,
            HISTORY_KIND,
            self.history.entries,
        )
    }

    /// Apply only to a row returned by the authenticated history reader. This
    /// comparison binds the selected current tuple; it does not prove that a
    /// caller-supplied byte slice occurs in that tree.
    pub fn verify_current_history_row(
        &self,
        key: &[u8],
        raw: &[u8],
        decode_workspace_bytes: usize,
    ) -> io::Result<()> {
        if key != self.current.revision.0.as_bytes()
            || SourceRevisionRootsV2::decode_with_workspace(raw, decode_workspace_bytes)?
                != self.current
        {
            return Err(invalid("source history current revision binding differs"));
        }
        Ok(())
    }

    pub fn encode_with_state_limit(
        &self,
        max_state_bytes: usize,
        additional_live_bytes: usize,
    ) -> io::Result<Vec<u8>> {
        if self.encode_state_upper_bound(additional_live_bytes)? > max_state_bytes {
            return Err(invalid("source rootset encoding exceeds reserved state"));
        }
        self.validate_store_binding(
            self.current.members.store_id,
            self.current.members.domain_digest,
        )?;
        let value = serde_json::json!([
            ROOTSET_SCHEMA,
            self.current.wire_value()?,
            tree_bytes(&self.history)?
        ]);
        let raw = serde_json::to_vec(&value)
            .map_err(|_| invalid("source rootset serialization failed"))?;
        if raw.is_empty() || raw.len() > ROOTSET_MAX_BYTES {
            return Err(invalid("source rootset byte profile exceeded"));
        }
        Ok(raw)
    }

    pub fn decode(raw: &[u8]) -> io::Result<Self> {
        let workspace = decode_workspace_upper_bound(raw.len())?;
        Self::decode_with_workspace(raw, workspace)
    }

    pub fn decode_with_workspace(raw: &[u8], workspace: usize) -> io::Result<Self> {
        if raw.is_empty() || raw.len() > ROOTSET_MAX_BYTES {
            return Err(invalid("source rootset byte profile exceeded"));
        }
        let (document, _) = parse_bounded_json(raw, ROOTSET_MAX_BYTES, workspace)?;
        let fields = document
            .root()
            .as_array()
            .filter(|fields| fields.len() == 3)
            .ok_or_else(|| invalid("source rootset tuple shape differs"))?;
        if fields[0].as_str() != Some(ROOTSET_SCHEMA) {
            return Err(invalid("source rootset version differs"));
        }
        let result = Self {
            current: SourceRevisionRootsV2::from_wire(&fields[1])?,
            history: tree(&fields[2])?,
        };
        result.validate_store_binding(
            result.current.members.store_id,
            result.current.members.domain_digest,
        )?;
        Ok(result)
    }

    pub fn digest(&self, max_state_bytes: usize) -> io::Result<Digest256> {
        self.encode_with_state_limit(max_state_bytes, 0)
            .map(|raw| Digest256::of_bytes(&raw))
    }
}

pub fn segment_io_error(error: tos_segment_store::SegmentError) -> io::Error {
    use tos_segment_store::SegmentErrorCode;
    let kind = match error.code {
        SegmentErrorCode::Io => error
            .source
            .as_ref()
            .map_or(io::ErrorKind::Other, io::Error::kind),
        SegmentErrorCode::UnsupportedPlatform | SegmentErrorCode::UnsupportedOversized => {
            io::ErrorKind::Unsupported
        }
        SegmentErrorCode::Cancelled => io::ErrorKind::Interrupted,
        SegmentErrorCode::DeadlineExceeded => io::ErrorKind::TimedOut,
        _ => io::ErrorKind::InvalidData,
    };
    io::Error::new(kind, error)
}
