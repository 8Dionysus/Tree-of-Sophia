//! Exact, read-only nonmetadata source layers. Catalog membership, revision
//! binding and the final disclosure hold belong to the caller. These kernels
//! neither mint handles nor grant rights, perform assessment or write sources.
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, RelativePath, canonical_raw_bytes_v1,
};

/// A fixed protected source-root transport, held for the entire owner read.
/// Range reads verify both the declared file size and retained fd/path identity;
/// verify_current rechecks the selected publication and every observed member.
pub trait SourceLayerRead {
    fn source_root(&self) -> &Path;
    fn read(
        &mut self,
        reference: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>>;
    fn read_range(
        &mut self,
        reference: &str,
        offset: u64,
        length: usize,
        expected_file_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>>;
    fn verify_current(&mut self, deadline: Instant, cancelled: &AtomicBool) -> Result<()>;
}
#[derive(Clone, Debug)]
pub struct LayerRead {
    pub record: Value,
    pub provenance: Value,
}
/// Independent full prepared revision and fixed owner root are required even
/// when one selected catalog member supplies all of the payload bytes.
pub struct LayerContext<'a> {
    pub source_root: &'a Path,
    pub full_revision: &'a Value,
    pub catalog_namespace: &'a str,
    pub max_record_bytes: usize,
    pub deadline: Instant,
    pub cancelled: &'a AtomicBool,
}
fn guard(reader: &mut dyn SourceLayerRead, c: &LayerContext<'_>) -> Result<()> {
    if c.cancelled.load(Ordering::Relaxed) || Instant::now() >= c.deadline {
        return Err(Error::Conflict("source read interrupted"));
    }
    if !c.source_root.is_absolute()
        || reader.source_root() != c.source_root
        || !c.full_revision.is_object()
        || !c
            .full_revision
            .get("source_revision")
            .and_then(Value::as_str)
            .is_some_and(bare_digest)
        || c.max_record_bytes > 1024 * 1024
    {
        return Err(Error::Invalid("source layer owner binding"));
    }
    reader.verify_current(c.deadline, c.cancelled)
}
fn bare_digest(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("source layer string"))
}
fn number(v: &Value, k: &str) -> Result<u64> {
    v.get(k)
        .and_then(Value::as_u64)
        .ok_or(Error::Invalid("source layer integer"))
}
fn hash(v: &Value, newline: bool) -> Result<String> {
    let raw = serde_json::to_vec(v).map_err(|_| Error::Invalid("source layer JSON"))?;
    let mut canonical = canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits {
            max_bytes: 8 * 1024 * 1024,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| Error::Invalid("source layer canonical JSON"))?;
    if newline {
        canonical.push(b'\n');
    }
    Ok(Digest256::of_bytes(&canonical).to_hex())
}
fn slot_key(kind: &str, id: &str) -> Result<String> {
    // Array strings are the catalog owner's canonical typed key.
    let raw =
        serde_json::to_vec(&json!([kind, id])).map_err(|_| Error::Invalid("source slot key"))?;
    let bytes = canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .map_err(|_| Error::Invalid("source slot key"))?;
    String::from_utf8(bytes).map_err(|_| Error::Invalid("source slot key"))
}
fn source_path(reference: &str, kind: &str) -> Result<()> {
    RelativePath::parse(reference).map_err(|_| Error::Invalid("source slot path"))?;
    if !reference.starts_with("ToS/source-witnesses/") || reference.contains("/catalog/") {
        return Err(Error::Invalid("source slot owner path"));
    }
    let name = reference.rsplit('/').next().unwrap_or("");
    let valid = match kind {
        "claim" => matches!(
            name,
            "source-claims.jsonl"
                | "membership-claims.jsonl"
                | "responsibility-claims.jsonl"
                | "publication-claims.jsonl"
                | "provision-activity-claims.jsonl"
                | "work-chronology-claims.jsonl"
                | "work-expression-claims.jsonl"
                | "expression-edition-claims.jsonl"
                | "edition-item-claims.jsonl"
                | "expression-derivation-claims.jsonl"
                | "object-link-claims.jsonl"
                | "historical-claims.jsonl"
        ),
        "provenance_event" => name.contains("provenance") && name.ends_with(".jsonl"),
        "anchor" => name.contains("anchor") && name.ends_with(".jsonl"),
        _ => false,
    };
    if !valid {
        return Err(Error::Invalid("source slot producer path"));
    }
    Ok(())
}
fn source_provenance(slot: &Value, c: &LayerContext<'_>) -> Result<Value> {
    let mut out = slot
        .get("source")
        .and_then(Value::as_object)
        .cloned()
        .ok_or(Error::Invalid("source slot binding"))?;
    out.extend(json!({"schema_version":"tos_source_catalog_slot_address_v1",
        "catalog_namespace":c.catalog_namespace,"profile_id":"tos.source-catalog.current-jsonl-slots.v1",
        "source_slot_key":slot["source_slot_key"],"row_sha256":hash(slot,true)?,
        "kind":slot["kind"],"identity":slot["identity"]}).as_object().unwrap().clone());
    Ok(Value::Object(out))
}
/// Read exactly one authoritative catalog-addressed JSONL source slot.
/// expected_row_sha256 is the catalog row hash (canonical bytes with LF),
/// while content_revision is the canonical payload hash without LF.
pub fn read_source_slot(
    reader: &mut dyn SourceLayerRead,
    c: &LayerContext<'_>,
    slot: &Value,
    kind: &str,
    id: &str,
    expected_row_sha256: &str,
    content_revision: &str,
) -> Result<LayerRead> {
    guard(reader, c)?;
    let field = match kind {
        "claim" => "claim_id",
        "provenance_event" => "event_id",
        "anchor" => "anchor_id",
        _ => return Err(Error::Invalid("source slot kind")),
    };
    if id.is_empty()
        || id.len() > 4096
        || slot["kind"] != kind
        || slot["identity"] != id
        || slot["source_slot_key"] != slot_key(kind, id)?
        || hash(slot, true)? != expected_row_sha256
    {
        return Err(Error::Conflict("source slot catalog row differs"));
    }
    let binding = &slot["source"];
    let reference = text(binding, "source_ref")?;
    source_path(reference, kind)?;
    let offset = number(binding, "byte_offset")?;
    let size = number(binding, "row_bytes")?;
    let file_size = number(binding, "file_bytes")?;
    let line = number(binding, "source_line")?;
    let delimiter = match text(binding, "delimiter")? {
        "lf" => &b"\n"[..],
        "crlf" => &b"\r\n"[..],
        "cr" => &b"\r"[..],
        "eof" => &b""[..],
        _ => return Err(Error::Invalid("source slot delimiter")),
    };
    let end = offset
        .checked_add(size)
        .and_then(|n| n.checked_add(delimiter.len() as u64))
        .ok_or(Error::Invalid("source slot range"))?;
    if size > c.max_record_bytes as u64
        || size == 0
        || end > file_size
        || line == 0
        || (line == 1) != (offset == 0)
        || delimiter.is_empty() && end != file_size
    {
        return Err(Error::Invalid("source slot range/budget"));
    }
    let start = offset.saturating_sub(1);
    let prefix = (offset - start) as usize;
    let extra = usize::from(delimiter == b"\r" && end < file_size);
    let length = prefix
        .checked_add(size as usize)
        .and_then(|n| n.checked_add(delimiter.len() + extra))
        .ok_or(Error::Invalid("source slot range budget"))?;
    let chunk = reader.read_range(reference, start, length, file_size, c.deadline, c.cancelled)?;
    if chunk.len() != length {
        return Err(Error::Conflict("source slot short range"));
    }
    let raw = &chunk[prefix..prefix + size as usize];
    if prefix > 0
        && (!matches!(chunk[0], b'\r' | b'\n') || chunk[0] == b'\r' && raw.starts_with(b"\n"))
        || &chunk[prefix + size as usize..prefix + size as usize + delimiter.len()] != delimiter
        || extra == 1 && chunk.last() == Some(&b'\n')
        || Digest256::of_bytes(raw).to_hex() != text(binding, "raw_row_sha256")?
    {
        return Err(Error::Conflict("source slot exact bytes/boundary differs"));
    }
    // PublishedStrict rejects duplicate keys and nonfinite JSON numbers.
    crate::source_command::parse(raw)?;
    let payload: Value =
        serde_json::from_slice(raw).map_err(|_| Error::Invalid("source slot JSON"))?;
    let canonical = hash(&payload, false)?;
    if !payload.is_object()
        || payload[field] != id
        || canonical != text(binding, "canonical_sha256")?
        || format!("sha256:{canonical}") != content_revision
    {
        return Err(Error::Conflict("source slot content differs"));
    }
    // These are the source owner's visibility contracts, not rights grants.
    if kind == "claim"
        && !matches!(
            payload["visibility"].as_str(),
            Some("public" | "public_metadata_only")
        )
        || kind == "provenance_event"
            && payload["schema_version"] == "tos_provenance_event_v2"
            && !matches!(
                payload["rights_and_visibility"]["content_visibility"].as_str(),
                Some("tracked_public_metadata" | "public_content" | "public_synthetic")
            )
    {
        return Err(Error::Denied("source slot visibility"));
    }
    guard(reader, c)?;
    Ok(LayerRead {
        record: payload,
        provenance: json!({"source":source_provenance(slot,c)?,"catalog":null,
        "verification_scope":"current-source-slot","whole_file_rehashed":false,"historical_claim_verified":false}),
    })
}
/// Current Claim adapter: reproduces the existing owner entry renderer and
/// exact slot/ref closure. Retained historical Claims require their separate
/// history owner; this function cannot assert historical verification.
pub fn read_current_claim(
    reader: &mut dyn SourceLayerRead,
    c: &LayerContext<'_>,
    claim: &Value,
    slot: &Value,
    exact_ref: &Value,
    expected_row_sha256: &str,
) -> Result<LayerRead> {
    if hash(claim, true)? != expected_row_sha256
        || claim["claim_ref"] != *exact_ref
        || claim["claim_id"] != exact_ref["id"]
        || claim["source_slot_key"] != slot["source_slot_key"]
    {
        return Err(Error::Conflict("Claim catalog exact ref differs"));
    }
    let id = text(exact_ref, "id")?;
    let mut selected = read_source_slot(
        reader,
        c,
        slot,
        "claim",
        id,
        &hash(slot, true)?,
        text(exact_ref, "digest")?,
    )?;
    let binding = &slot["source"];
    let entry = tos_compiler::source_witness_catalog::render_catalog_claim(
        &selected.record,
        text(binding, "source_ref")?,
        number(binding, "source_line")?,
        claim["entry"]["source_schema_ref"].as_str(),
        c.max_record_bytes,
    )
    .map_err(|_| Error::Invalid("Claim catalog renderer"))?;
    if entry != claim["entry"]
        || selected.record["claim_version"] != exact_ref["version"]
        || selected.record["claim_version"].as_u64().is_none()
    {
        return Err(Error::Conflict("Claim source entry differs"));
    }
    selected.provenance["catalog"] = json!({"schema_version":"tos_source_claim_catalog_address_v1",
        "catalog_namespace":c.catalog_namespace,"profile_id":"tos.source-catalog.public-claims.v1",
        "claim_key":id,"row_sha256":expected_row_sha256,"source_slot_key":claim["source_slot_key"],"claim_ref":exact_ref});
    guard(reader, c)?;
    Ok(selected)
}
/// The row is an exact member selected from the authoritative authored-corpus
/// root, never a consumer-supplied replacement for membership.
pub fn read_authored_csv(
    reader: &mut dyn SourceLayerRead,
    c: &LayerContext<'_>,
    row: &Value,
    target: &Value,
    authored_root_sha256: &str,
) -> Result<LayerRead> {
    guard(reader, c)?;
    let keys = [
        "layer",
        "pack_id",
        "edge_id",
        "source_row",
        "source_file_sha256",
        "content_revision",
    ];
    if target
        .as_object()
        .is_none_or(|m| m.len() != keys.len() || keys.iter().any(|k| !m.contains_key(*k)))
        || !bare_digest(authored_root_sha256)
        || !bare_digest(text(target, "source_file_sha256")?)
    {
        return Err(Error::Invalid("authored CSV closed target"));
    }
    let pack = text(target, "pack_id")?;
    let edge = text(target, "edge_id")?;
    if pack.len() > 2048
        || edge.is_empty()
        || edge.len() > 2048
        || edge.contains('\0')
        || pack
            .split('/')
            .any(|p| p.is_empty() || p == "payload" || p.starts_with('.'))
    {
        return Err(Error::Invalid("authored CSV identity"));
    }
    let reference = format!("ToS/{pack}/edges.csv");
    RelativePath::parse(&reference).map_err(|_| Error::Invalid("authored CSV path"))?;
    let branch = if pack.starts_with("canon/relations/") {
        "canon"
    } else if pack.starts_with("candidate-intake/") {
        "candidate-intake"
    } else {
        return Err(Error::Invalid("authored CSV owner"));
    };
    let authority = if branch == "canon" {
        "canon"
    } else {
        "candidate_intake"
    };
    let key = slot_key(pack, edge)?;
    if row["target"] != *target
        || row["key"] != key
        || row["source_ref"] != reference
        || row["owner_branch"] != format!("ToS/{branch}")
        || row["authority_layer"] != authority
        || target["layer"] != "authored_csv_record"
        || row.as_object().map(|m| m.len()) != Some(6)
    {
        return Err(Error::Conflict("authored CSV addressed row differs"));
    }
    let record = &row["record"];
    if format!("sha256:{}", hash(record, false)?) != text(target, "content_revision")? {
        return Err(Error::Conflict("authored CSV record digest differs"));
    }
    if record
        .get("edge_id")
        .and_then(Value::as_str)
        .is_some_and(|v| !v.is_empty() && v != edge)
    {
        return Err(Error::Conflict("authored CSV edge identity differs"));
    }
    let raw = reader.read(&reference, 8 * 1024 * 1024, c.deadline, c.cancelled)?;
    if Digest256::of_bytes(&raw).to_hex() != text(target, "source_file_sha256")? {
        return Err(Error::Conflict("authored CSV file differs"));
    }
    let mut provenance = tos_compiler::knowledge_canon_source::read_exact_authored_csv_row(
        &raw,
        number(target, "source_row")?,
        record,
        c.max_record_bytes,
        c.deadline,
        c.cancelled,
    )
    .map_err(|_| Error::Invalid("authored CSV exact source binding"))?;
    let selected = provenance
        .as_object_mut()
        .ok_or(Error::Invalid("authored CSV provenance"))?
        .remove("record")
        .ok_or(Error::Invalid("authored CSV record"))?;
    provenance["source_ref"] = json!(reference);
    provenance["owner_branch"] = json!(format!("ToS/{branch}"));
    provenance["authority_layer"] = json!(authority);
    provenance["authored_root_sha256"] = json!(authored_root_sha256);
    guard(reader, c)?;
    Ok(LayerRead {
        record: selected,
        provenance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::atomic::AtomicBool};

    struct BytesReader {
        root: PathBuf,
        reference: String,
        bytes: Vec<u8>,
        reads: usize,
    }
    impl SourceLayerRead for BytesReader {
        fn source_root(&self) -> &Path {
            &self.root
        }
        fn read(
            &mut self,
            reference: &str,
            max_bytes: usize,
            _: Instant,
            _: &AtomicBool,
        ) -> Result<Vec<u8>> {
            if reference != self.reference || self.bytes.len() > max_bytes {
                return Err(Error::Invalid("test source read"));
            }
            self.reads += 1;
            Ok(self.bytes.clone())
        }
        fn read_range(
            &mut self,
            reference: &str,
            offset: u64,
            length: usize,
            expected_file_bytes: u64,
            _: Instant,
            _: &AtomicBool,
        ) -> Result<Vec<u8>> {
            if reference != self.reference || expected_file_bytes != self.bytes.len() as u64 {
                return Err(Error::Conflict("test source range binding"));
            }
            let start = usize::try_from(offset).map_err(|_| Error::Invalid("test range"))?;
            let end = start
                .checked_add(length)
                .filter(|end| *end <= self.bytes.len())
                .ok_or(Error::Invalid("test source range"))?;
            self.reads += 1;
            Ok(self.bytes[start..end].to_vec())
        }
        fn verify_current(&mut self, _: Instant, _: &AtomicBool) -> Result<()> {
            Ok(())
        }
    }

    fn slot_case(
        raw: &[u8],
        visibility: &str,
    ) -> (BytesReader, Value, Value, String, String, AtomicBool) {
        let id = "tos.claim.fixture";
        let reference = "ToS/source-witnesses/fixture/source-claims.jsonl".to_owned();
        let mut bytes = raw.to_vec();
        bytes.push(b'\n');
        let payload: Value = serde_json::from_slice(raw).unwrap();
        assert_eq!(payload["visibility"], visibility);
        let canonical_sha = hash(&payload, false).unwrap();
        let slot = json!({
            "kind":"claim",
            "identity":id,
            "source_slot_key":slot_key("claim",id).unwrap(),
            "source":{
                "source_ref":reference,
                "byte_offset":0,
                "row_bytes":raw.len(),
                "file_bytes":bytes.len(),
                "source_line":1,
                "delimiter":"lf",
                "raw_row_sha256":Digest256::of_bytes(raw).to_hex(),
                "canonical_sha256":canonical_sha
            }
        });
        let content_revision = format!("sha256:{canonical_sha}");
        let row_sha = hash(&slot, true).unwrap();
        let reader = BytesReader {
            root: PathBuf::from("/selected/source-root"),
            reference,
            bytes,
            reads: 0,
        };
        let revision = json!({"source_revision":"a".repeat(64)});
        let cancelled = AtomicBool::new(false);
        (reader, slot, revision, content_revision, row_sha, cancelled)
    }

    fn context<'a>(
        root: &'a Path,
        revision: &'a Value,
        cancelled: &'a AtomicBool,
    ) -> LayerContext<'a> {
        LayerContext {
            source_root: root,
            full_revision: revision,
            catalog_namespace: "fixture.catalog",
            max_record_bytes: 4096,
            deadline: Instant::now() + std::time::Duration::from_secs(5),
            cancelled,
        }
    }

    #[test]
    fn current_claim_source_slot_reads_exact_public_bytes_and_rejects_private_visibility() {
        let public = br#"{"claim_id":"tos.claim.fixture","visibility":"public"}"#;
        let (mut reader, slot, revision, content_revision, row_sha, cancelled) =
            slot_case(public, "public");
        let root = reader.root.clone();
        let public_context = context(&root, &revision, &cancelled);
        let result = read_source_slot(
            &mut reader,
            &public_context,
            &slot,
            "claim",
            "tos.claim.fixture",
            &row_sha,
            &content_revision,
        )
        .unwrap();
        assert_eq!(result.record["claim_id"], "tos.claim.fixture");
        assert_eq!(
            result.provenance["verification_scope"],
            "current-source-slot"
        );
        assert_eq!(reader.reads, 1);

        let private = br#"{"claim_id":"tos.claim.fixture","visibility":"private"}"#;
        let (mut reader, slot, revision, content_revision, row_sha, cancelled) =
            slot_case(private, "private");
        let root = reader.root.clone();
        let context = context(&root, &revision, &cancelled);
        assert!(matches!(
            read_source_slot(
                &mut reader,
                &context,
                &slot,
                "claim",
                "tos.claim.fixture",
                &row_sha,
                &content_revision,
            ),
            Err(Error::Denied("source slot visibility"))
        ));
    }

    #[test]
    fn current_claim_read_matches_exact_catalog_entry_to_source_slot_and_ref() {
        let public = br#"{"claim_id":"tos.claim.fixture","claim_type":"attestation","assertion_layer":"recorded","subject_ref":"tos.record.fixture","predicate":"was witnessed by","object":"tos.agent.fixture","evidence_refs":[],"maker":{},"provenance_event_ref":null,"epistemic_status":"asserted","review_status":"unreviewed","visibility":"public_metadata_only","claim_version":1,"schema_version":"tos_source_claim_v1"}"#;
        let (mut reader, slot, revision, content_revision, _slot_row_sha, cancelled) =
            slot_case(public, "public_metadata_only");
        let source_ref = slot["source"]["source_ref"].as_str().unwrap();
        let payload: Value = serde_json::from_slice(public).unwrap();
        let entry = tos_compiler::source_witness_catalog::render_catalog_claim(
            &payload, source_ref, 1, None, 4096,
        )
        .unwrap();
        let exact_ref = json!({
            "id":"tos.claim.fixture",
            "version":1,
            "digest":content_revision
        });
        let claim = json!({
            "claim_id":"tos.claim.fixture",
            "claim_ref":exact_ref,
            "source_slot_key":slot["source_slot_key"],
            "entry":entry
        });
        let claim_row_sha = hash(&claim, true).unwrap();
        let root = reader.root.clone();
        let context = context(&root, &revision, &cancelled);
        let result = read_current_claim(
            &mut reader,
            &context,
            &claim,
            &slot,
            &exact_ref,
            &claim_row_sha,
        )
        .unwrap();
        assert_eq!(result.record, payload);
        assert_eq!(result.provenance["catalog"]["claim_ref"], exact_ref);
        assert_eq!(reader.reads, 1);

        let mut wrong_ref = exact_ref;
        wrong_ref["version"] = json!(2);
        assert!(
            read_current_claim(
                &mut reader,
                &context,
                &claim,
                &slot,
                &wrong_ref,
                &claim_row_sha,
            )
            .is_err()
        );
    }

    #[test]
    fn current_claim_source_slot_rejects_duplicate_json_before_record_return() {
        let duplicate = br#"{"claim_id":"tos.claim.fixture","claim_id":"tos.claim.fixture","visibility":"public"}"#;
        let (mut reader, slot, revision, content_revision, row_sha, cancelled) =
            slot_case(duplicate, "public");
        let root = reader.root.clone();
        let context = context(&root, &revision, &cancelled);
        assert!(
            read_source_slot(
                &mut reader,
                &context,
                &slot,
                "claim",
                "tos.claim.fixture",
                &row_sha,
                &content_revision,
            )
            .is_err()
        );
    }
}
