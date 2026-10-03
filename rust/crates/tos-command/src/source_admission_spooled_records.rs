//! Same-connection transport for the streamed Records/Item owner facts.
//!
//! This module contains storage codecs and cursors only. The validation
//! kernels retain all predicates and mint their report only after their own
//! EOF/currentness checks.

use super::{CandidateFenceSource, IndexSink, bounded_text, invalid, sql};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    io,
    num::NonZeroUsize,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};
use tos_foundation::Digest256;
use tos_validation::{
    FormatProfile,
    executor::{
        ExceptionalSchemaUsage, SchemaDiagnosticUnit, SchemaDiagnosticsCheckpoint,
        schema_diagnostics as schema_diag,
    },
    item_rules::{ItemIssue, ItemRefusal},
    record_biblio_cut::{BiblioCurrentRecord, SourceCutSchemaDiagnostic, SourceCutSchemaVerdict},
    record_rules::{PathReferenceCheck, RecordObservation},
    source_foundation_records::{
        SourceFoundationCurrentRecordPathLookup, SourceFoundationCurrentRecordsPage,
        SourceFoundationEventInsertion, SourceFoundationFileDescriptorLookup,
        SourceFoundationGlobalIdFact, SourceFoundationItemEditionLookup,
        SourceFoundationItemRecordSelection, SourceFoundationItemSelectionLookup,
        SourceFoundationLinkUriFact, SourceFoundationRecordFact,
        SourceFoundationRecordFactCollection, SourceFoundationRecordFactPage,
        SourceFoundationRecordIdCarrier, SourceFoundationRecordObservation,
        SourceFoundationRecordPathReference, SourceFoundationRecordSchemaDiagnostic,
        SourceFoundationRecordsCollection, SourceFoundationRecordsCursor,
        SourceFoundationRecordsCursorPage, SourceFoundationRecordsIssue,
        SourceFoundationRecordsIssueFamily, SourceFoundationRecordsLookup,
        SourceFoundationRecordsOwnerIssue, SourceFoundationRecordsPageBudget,
        SourceFoundationRecordsSchemaCheck, SourceFoundationRecordsSchemaFamily,
        SourceFoundationRecordsStore, SourceFoundationRecordsStoredFact,
        SourceFoundationTypedIdRefFact, SourceFoundationUriOwnerLookup,
    },
};

const STORE_CODEC_VERSION: u64 = 1;
const FACT_CODEC_VERSION: u64 = 1;

fn refusal(_: impl std::fmt::Display) -> ItemRefusal {
    ItemRefusal::Source("source-foundation bounded index operation refused".into())
}

fn checked_add(a: usize, b: usize) -> io::Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| invalid("source-foundation row state overflow"))
}

fn value_state(value: &Value) -> io::Result<usize> {
    fn visit(value: &Value, total: &mut usize) -> io::Result<()> {
        match value {
            Value::Null | Value::Bool(_) => {
                *total = checked_add(*total, 32)?;
            }
            Value::Number(number) => {
                // arbitrary_precision stores the original numeric lexeme.
                // as_str borrows it without allocating a formatted copy.
                *total = checked_add(*total, checked_add(number.as_str().len(), 48)?)?;
            }
            Value::String(text) => {
                *total = checked_add(*total, checked_add(text.len(), 48)?)?;
            }
            Value::Array(rows) => {
                *total = checked_add(
                    *total,
                    checked_add(
                        32,
                        rows.len()
                            .checked_mul(8)
                            .ok_or_else(|| invalid("source-foundation value state overflow"))?,
                    )?,
                )?;
                for row in rows {
                    visit(row, total)?;
                }
            }
            Value::Object(rows) => {
                *total = checked_add(
                    *total,
                    checked_add(
                        48,
                        rows.len()
                            .checked_mul(16)
                            .ok_or_else(|| invalid("source-foundation value state overflow"))?,
                    )?,
                )?;
                for (key, row) in rows {
                    *total = checked_add(*total, checked_add(key.len(), 32)?)?;
                    visit(row, total)?;
                }
            }
        }
        Ok(())
    }
    let mut total = 0usize;
    visit(value, &mut total)?;
    Ok(total)
}

fn string_fields(fields: &[&str]) -> io::Result<usize> {
    fields.iter().try_fold(256usize, |total, field| {
        checked_add(
            total,
            checked_add(
                field
                    .len()
                    .checked_mul(2)
                    .ok_or_else(|| invalid("source-foundation text state overflow"))?,
                64,
            )?,
        )
    })
}

fn preflight_row_clone(sink: &IndexSink<'_>, source_state: usize) -> io::Result<()> {
    // Source estimates include nested values and strings. Allow temporary
    // wrapper trees while json! moves nested values into the stored row.
    sink.candidate.tick()?;
    let peak = source_state
        .checked_mul(4)
        .and_then(|n| n.checked_add(8192))
        .ok_or_else(|| invalid("source-foundation clone state overflow"))?;
    sink.candidate.check_state(peak)
}

fn encode_json(
    sink: &IndexSink<'_>,
    source_state: usize,
    value: &Value,
    aux: &[u8],
) -> io::Result<(Vec<u8>, usize)> {
    let value_bytes = value_state(value)?;
    // Every string byte needs at most six bytes in serde JSON (\u00XX).
    // value_state also charges nodes/keys, so this covers punctuation and
    // scalars. Reserve and charge the conservative wire capacity before writing.
    let wire_capacity = value_bytes
        .checked_mul(6)
        .ok_or_else(|| invalid("source-foundation wire state overflow"))?;
    let precharge = source_state
        .checked_add(value_bytes)
        .and_then(|n| n.checked_add(aux.len()))
        .and_then(|n| n.checked_add(wire_capacity))
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(|| invalid("source-foundation encoder state overflow"))?;
    sink.candidate.check_state(precharge)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(wire_capacity)
        .map_err(|_| invalid("source-foundation wire allocation refused"))?;
    // Allocators may round a reservation upward; charge actual capacity before
    // the serializer can write into it.
    let reserved_capacity = bytes.capacity();
    let actual_peak = checked_add(precharge - wire_capacity, reserved_capacity)?;
    sink.candidate.check_state(actual_peak)?;
    struct BoundedWire<'a> {
        bytes: &'a mut Vec<u8>,
        limit: usize,
    }
    impl io::Write for BoundedWire<'_> {
        fn write(&mut self, chunk: &[u8]) -> io::Result<usize> {
            let next = self
                .bytes
                .len()
                .checked_add(chunk.len())
                .ok_or_else(|| invalid("source-foundation wire length overflow"))?;
            // Refuse before extending: even an underestimated codec bound
            // cannot let serde trigger a Vec growth allocation.
            if next > self.limit || next > self.bytes.capacity() {
                return Err(invalid("source-foundation wire capacity exceeded"));
            }
            self.bytes.extend_from_slice(chunk);
            Ok(chunk.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(
        &mut BoundedWire {
            bytes: &mut bytes,
            limit: wire_capacity,
        },
        value,
    )
    .map_err(|_| invalid("source-foundation row JSON encoding failed"))?;
    let stored = source_state
        .checked_add(value_bytes)
        .and_then(|n| n.checked_add(bytes.len()))
        .and_then(|n| n.checked_add(aux.len()))
        .and_then(|n| n.checked_add(2048))
        .ok_or_else(|| invalid("source-foundation stored row state overflow"))?;
    if stored > sink.row_limit {
        return Err(invalid(
            "source-foundation stored row exceeds selected state envelope",
        ));
    }
    sink.candidate.check_state(actual_peak.max(stored))?;
    Ok((bytes, stored))
}

fn decode_value(bytes: &[u8], max_state: usize) -> io::Result<Value> {
    if bytes.len() > max_state {
        return Err(invalid(
            "source-foundation encoded row exceeds selected state envelope",
        ));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("source-foundation row JSON decoding failed"))?;
    let decoded = value_state(&value)?;
    if decoded
        .checked_add(bytes.len())
        .is_none_or(|state| state > max_state)
    {
        return Err(invalid(
            "source-foundation decoded row exceeds selected state envelope",
        ));
    }
    Ok(value)
}

fn field<'a>(value: &'a Value, name: &str) -> io::Result<&'a Value> {
    value
        .get(name)
        .ok_or_else(|| invalid("source-foundation stored row field missing"))
}

fn text<'a>(value: &'a Value, name: &str) -> io::Result<&'a str> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| invalid("source-foundation stored row text invalid"))
}

fn owned_text(value: &Value, name: &str) -> io::Result<String> {
    Ok(text(value, name)?.to_owned())
}

fn uint(value: &Value, name: &str) -> io::Result<u64> {
    field(value, name)?
        .as_u64()
        .ok_or_else(|| invalid("source-foundation stored row integer invalid"))
}

fn usize_value(value: &Value, name: &str) -> io::Result<usize> {
    usize::try_from(uint(value, name)?)
        .map_err(|_| invalid("source-foundation stored row integer overflow"))
}

fn bool_value(value: &Value, name: &str) -> io::Result<bool> {
    field(value, name)?
        .as_bool()
        .ok_or_else(|| invalid("source-foundation stored row boolean invalid"))
}

fn digest(value: &Value, name: &str) -> io::Result<Digest256> {
    Digest256::from_hex(text(value, name)?)
        .map_err(|_| invalid("source-foundation stored digest invalid"))
}

fn profile_id(profile: FormatProfile) -> &'static str {
    profile.id()
}

fn parse_profile(id: &str) -> io::Result<FormatProfile> {
    match id {
        "tos.schema.format.legacy-python-observed-20260923" => {
            Ok(FormatProfile::LegacyPythonObserved20260923)
        }
        "tos.schema.format.asserted-source-candidate-v1" => {
            Ok(FormatProfile::AssertedSourceCandidateV1)
        }
        _ => Err(invalid("source-foundation stored format profile invalid")),
    }
}

fn issue_family(family: SourceFoundationRecordsIssueFamily) -> &'static str {
    match family {
        SourceFoundationRecordsIssueFamily::Record => "record",
        SourceFoundationRecordsIssueFamily::Item => "item",
    }
}

fn parse_issue_family(value: &str) -> io::Result<SourceFoundationRecordsIssueFamily> {
    match value {
        "record" => Ok(SourceFoundationRecordsIssueFamily::Record),
        "item" => Ok(SourceFoundationRecordsIssueFamily::Item),
        _ => Err(invalid("source-foundation issue family invalid")),
    }
}

fn owner_issue(issue: Option<SourceFoundationRecordsOwnerIssue>) -> Value {
    match issue {
        None => Value::Null,
        Some(SourceFoundationRecordsOwnerIssue::JsonRootMustBeObject) => {
            json!("json_root_must_be_object")
        }
        Some(SourceFoundationRecordsOwnerIssue::JsonlRecordMustBeObject) => {
            json!("jsonl_record_must_be_object")
        }
        Some(SourceFoundationRecordsOwnerIssue::InvalidJson) => json!("invalid_json"),
        Some(SourceFoundationRecordsOwnerIssue::InvalidJsonl) => json!("invalid_jsonl"),
        Some(SourceFoundationRecordsOwnerIssue::BlankJsonlLine) => json!("blank_jsonl_line"),
        Some(SourceFoundationRecordsOwnerIssue::InvalidJsonlUtf8) => json!("invalid_jsonl_utf8"),
    }
}

fn parse_owner_issue(value: &Value) -> io::Result<Option<SourceFoundationRecordsOwnerIssue>> {
    if value.is_null() {
        return Ok(None);
    }
    match value
        .as_str()
        .ok_or_else(|| invalid("source-foundation owner issue invalid"))?
    {
        "json_root_must_be_object" => Ok(Some(
            SourceFoundationRecordsOwnerIssue::JsonRootMustBeObject,
        )),
        "jsonl_record_must_be_object" => Ok(Some(
            SourceFoundationRecordsOwnerIssue::JsonlRecordMustBeObject,
        )),
        "invalid_json" => Ok(Some(SourceFoundationRecordsOwnerIssue::InvalidJson)),
        "invalid_jsonl" => Ok(Some(SourceFoundationRecordsOwnerIssue::InvalidJsonl)),
        "blank_jsonl_line" => Ok(Some(SourceFoundationRecordsOwnerIssue::BlankJsonlLine)),
        "invalid_jsonl_utf8" => Ok(Some(SourceFoundationRecordsOwnerIssue::InvalidJsonlUtf8)),
        _ => Err(invalid("source-foundation owner issue invalid")),
    }
}

fn observation_value(row: &RecordObservation) -> Value {
    match row {
        RecordObservation::ExactPath { path, raw_sha256 } => {
            json!({"v":1,"tag":"exact_path","path":path,"raw_sha256":raw_sha256})
        }
        RecordObservation::Registry {
            path,
            version,
            raw_sha256,
        } => json!({"v":1,"tag":"registry","path":path,"version":version,"raw_sha256":raw_sha256}),
        RecordObservation::Schema {
            path,
            uri,
            raw_sha256,
        } => json!({"v":1,"tag":"schema","path":path,"uri":uri,"raw_sha256":raw_sha256}),
        RecordObservation::Profile {
            path,
            kind,
            profile_version,
            schema_version,
        } => {
            json!({"v":1,"tag":"profile","path":path,"kind":kind,"profile_version":profile_version,"schema_version":schema_version})
        }
        RecordObservation::Reference {
            from_path,
            target_path,
            check,
        } => {
            json!({"v":1,"tag":"reference","from_path":from_path,"target_path":target_path,"check":match check { PathReferenceCheck::RepoExistsIfToS => "repo_exists_if_tos", PathReferenceCheck::FileIfToS => "file_if_tos" }})
        }
        RecordObservation::RecordIdReference {
            from_path,
            target_id,
            expected_kind,
        } => {
            json!({"v":1,"tag":"record_id_reference","from_path":from_path,"target_id":target_id,"expected_kind":expected_kind})
        }
        RecordObservation::LinkUriOwner { uri, id, path } => {
            json!({"v":1,"tag":"link_uri_owner","uri":uri,"id":id,"path":path})
        }
        RecordObservation::IdOwner {
            id,
            kind,
            path,
            version,
            raw_sha256,
        } => {
            json!({"v":1,"tag":"id_owner","id":id,"kind":kind,"path":path,"version":version,"raw_sha256":raw_sha256})
        }
        RecordObservation::IdKindOwner { kind, id, path } => {
            json!({"v":1,"tag":"id_kind_owner","kind":kind,"id":id,"path":path})
        }
        RecordObservation::NativeReservation {
            id,
            packet_path,
            raw_sha256,
        } => {
            json!({"v":1,"tag":"native_reservation","id":id,"packet_path":packet_path,"raw_sha256":raw_sha256})
        }
        RecordObservation::Issue { path, code } => {
            json!({"v":1,"tag":"issue","path":path,"code":code})
        }
    }
}

fn static_record_code(code: &str) -> io::Result<&'static str> {
    Ok(match code {
        "native_record_id_collision" => "native_record_id_collision",
        "duplicate_record_id" => "duplicate_record_id",
        "duplicate_link_uri" => "duplicate_link_uri",
        "record_reference_missing" => "record_reference_missing",
        "global_fact_count" => "global_fact_count",
        "global_fact_bytes" => "global_fact_bytes",
        "global_fact_budget" => "global_fact_budget",
        "record_fact_budget_zero" => "record_fact_budget_zero",
        "schema_total_bytes" => "schema_total_bytes",
        "schema_resource_budget" => "schema_resource_budget",
        "native_record_byte_budget" => "native_record_byte_budget",
        "native_identity_byte_budget" => "native_identity_byte_budget",
        "native_identity_inventory_budget" => "native_identity_inventory_budget",
        "native_record_schema" => "native_record_schema",
        "native_schema_version" => "native_schema_version",
        "native_entities" => "native_entities",
        "native_entity_id" => "native_entity_id",
        "native_record_type" => "native_record_type",
        "native_record_id" => "native_record_id",
        "native_record_version" => "native_record_version",
        "record_byte_budget" => "record_byte_budget",
        "record_fact_budget" => "record_fact_budget",
        "compiled_route_budget" => "compiled_route_budget",
        "reference_type" => "reference_type",
        "record_id_reference_type" => "record_id_reference_type",
        "record_schema_version" => "record_schema_version",
        "record_schema" => "record_schema",
        "record_type" => "record_type",
        "record_profile_or_visibility" => "record_profile_or_visibility",
        "record_identity_metadata" => "record_identity_metadata",
        "record_version" => "record_version",
        "schema_budget" => "schema_budget",
        "record_path" => "record_path",
        "record_json" => "record_json",
        "unsupported_record_schema_version" => "unsupported_record_schema_version",
        "bounded_schema_evidence_mismatch" => "bounded_schema_evidence_mismatch",
        "id_facts_unsorted" => "id_facts_unsorted",
        "record_reference_wrong_kind" => "record_reference_wrong_kind",
        "link_uri_facts_unsorted" => "link_uri_facts_unsorted",
        _ => return Err(invalid("unknown static record issue code in stored row")),
    })
}

// Decode the maintained Item owner's static issue vocabulary without
// allocating leaked strings or changing its issue type. Unknown rows refuse.
fn static_item_code(code: &str) -> io::Result<&'static str> {
    Ok(match code {
        "acquisition-event-not-local" => "acquisition-event-not-local",
        "blank-jsonl-line" => "blank-jsonl-line",
        "duplicate-event-id" => "duplicate-event-id",
        "duplicate-resource-id" => "duplicate-resource-id",
        "duplicate-rights-layer-id" => "duplicate-rights-layer-id",
        "file-id-sha256" => "file-id-sha256",
        "file-identity-conflict" => "file-identity-conflict",
        "fixity-manifest-drift" => "fixity-manifest-drift",
        "invalid-json" => "invalid-json",
        "invalid-jsonl" => "invalid-jsonl",
        "invalid-jsonl-utf8" => "invalid-jsonl-utf8",
        "inventory-event-not-local" => "inventory-event-not-local",
        "inventory-file-identity" => "inventory-file-identity",
        "inventory-item-id" => "inventory-item-id",
        "inventory-provenance-output" => "inventory-provenance-output",
        "item-without-manifest" => "item-without-manifest",
        "manifest-item-record-id" => "manifest-item-record-id",
        "missing-companion" => "missing-companion",
        "missing-or-wrong-record-kind" => "missing-or-wrong-record-kind",
        "object-required" => "object-required",
        "payload-byte-size" => "payload-byte-size",
        "payload-path-string" => "payload-path-string",
        "payload-sha256" => "payload-sha256",
        "payload-source-inclusion" => "payload-source-inclusion",
        "required-payload-unavailable" => "required-payload-unavailable",
        "resource-count" => "resource-count",
        "rights-file-scope" => "rights-file-scope",
        "rights-item-scope" => "rights-item-scope",
        "rights-manifest-visibility" => "rights-manifest-visibility",
        "schema" => "schema",
        "unresolved-source-ref" => "unresolved-source-ref",
        _ => return Err(invalid("unknown static Item issue code in stored row")),
    })
}

fn observation_from_value(value: &Value) -> io::Result<RecordObservation> {
    if uint(value, "v")? != 1 {
        return Err(invalid("unsupported RecordObservation codec"));
    }
    Ok(match text(value, "tag")? {
        "exact_path" => RecordObservation::ExactPath {
            path: owned_text(value, "path")?,
            raw_sha256: owned_text(value, "raw_sha256")?,
        },
        "registry" => RecordObservation::Registry {
            path: owned_text(value, "path")?,
            version: owned_text(value, "version")?,
            raw_sha256: owned_text(value, "raw_sha256")?,
        },
        "schema" => RecordObservation::Schema {
            path: owned_text(value, "path")?,
            uri: owned_text(value, "uri")?,
            raw_sha256: owned_text(value, "raw_sha256")?,
        },
        "profile" => RecordObservation::Profile {
            path: owned_text(value, "path")?,
            kind: owned_text(value, "kind")?,
            profile_version: uint(value, "profile_version")?,
            schema_version: owned_text(value, "schema_version")?,
        },
        "reference" => RecordObservation::Reference {
            from_path: owned_text(value, "from_path")?,
            target_path: owned_text(value, "target_path")?,
            check: match text(value, "check")? {
                "repo_exists_if_tos" => PathReferenceCheck::RepoExistsIfToS,
                "file_if_tos" => PathReferenceCheck::FileIfToS,
                _ => return Err(invalid("RecordObservation path check invalid")),
            },
        },
        "record_id_reference" => RecordObservation::RecordIdReference {
            from_path: owned_text(value, "from_path")?,
            target_id: owned_text(value, "target_id")?,
            expected_kind: match text(value, "expected_kind")? {
                "work" => "work",
                "expression" => "expression",
                "collection" => "collection",
                _ => return Err(invalid("RecordObservation expected kind invalid")),
            },
        },
        "link_uri_owner" => RecordObservation::LinkUriOwner {
            uri: owned_text(value, "uri")?,
            id: owned_text(value, "id")?,
            path: owned_text(value, "path")?,
        },
        "id_owner" => RecordObservation::IdOwner {
            id: owned_text(value, "id")?,
            kind: owned_text(value, "kind")?,
            path: owned_text(value, "path")?,
            version: uint(value, "version")?,
            raw_sha256: owned_text(value, "raw_sha256")?,
        },
        "id_kind_owner" => RecordObservation::IdKindOwner {
            kind: owned_text(value, "kind")?,
            id: owned_text(value, "id")?,
            path: owned_text(value, "path")?,
        },
        "native_reservation" => RecordObservation::NativeReservation {
            id: owned_text(value, "id")?,
            packet_path: owned_text(value, "packet_path")?,
            raw_sha256: owned_text(value, "raw_sha256")?,
        },
        "issue" => RecordObservation::Issue {
            path: owned_text(value, "path")?,
            code: static_record_code(text(value, "code")?)?,
        },
        _ => return Err(invalid("RecordObservation tag invalid")),
    })
}

fn biblio_record_value(record: &BiblioCurrentRecord) -> Value {
    json!({"v":STORE_CODEC_VERSION,"path":record.path,"kind":record.kind,"value":record.value})
}

fn biblio_record_from_value(value: &Value) -> io::Result<BiblioCurrentRecord> {
    if uint(value, "v")? != STORE_CODEC_VERSION {
        return Err(invalid("Biblio record codec version invalid"));
    }
    Ok(BiblioCurrentRecord {
        path: owned_text(value, "path")?,
        kind: owned_text(value, "kind")?,
        value: field(value, "value")?.clone(),
    })
}

fn item_selection_value(row: &SourceFoundationItemRecordSelection) -> Value {
    json!({"v":STORE_CODEC_VERSION,"record_id":row.record_id,"path":row.path,"value":row.value})
}

fn item_selection_from_value(value: &Value) -> io::Result<SourceFoundationItemRecordSelection> {
    if uint(value, "v")? != STORE_CODEC_VERSION {
        return Err(invalid("Item selection codec version invalid"));
    }
    Ok(SourceFoundationItemRecordSelection {
        record_id: owned_text(value, "record_id")?,
        path: owned_text(value, "path")?,
        value: field(value, "value")?.clone(),
    })
}

fn schema_check_value(row: &SourceFoundationRecordsSchemaCheck) -> (Value, Vec<u8>) {
    let family = match row.family {
        SourceFoundationRecordsSchemaFamily::Record => "record",
        SourceFoundationRecordsSchemaFamily::Item => "item",
    };
    let raw = row.legacy_raw_instance.clone().unwrap_or_default();
    (
        json!({
            "v":STORE_CODEC_VERSION,
            "family":family,
            "before_issue":row.before_issue,
            "location":row.location,
            "contract":row.contract,
            "decoded_instance":row.decoded_instance,
            "has_legacy_raw_instance":row.legacy_raw_instance.is_some(),
            "owner_issue":owner_issue(row.owner_issue)
        }),
        raw,
    )
}

fn schema_check_from_value(
    value: &Value,
    raw: &[u8],
) -> io::Result<SourceFoundationRecordsSchemaCheck> {
    if uint(value, "v")? != STORE_CODEC_VERSION {
        return Err(invalid("schema check codec version invalid"));
    }
    let has_raw = bool_value(value, "has_legacy_raw_instance")?;
    if !has_raw && !raw.is_empty() {
        return Err(invalid("schema check auxiliary bytes are unexpected"));
    }
    Ok(SourceFoundationRecordsSchemaCheck {
        family: match text(value, "family")? {
            "record" => SourceFoundationRecordsSchemaFamily::Record,
            "item" => SourceFoundationRecordsSchemaFamily::Item,
            _ => return Err(invalid("schema check family invalid")),
        },
        before_issue: usize_value(value, "before_issue")?,
        location: owned_text(value, "location")?,
        contract: owned_text(value, "contract")?,
        decoded_instance: if field(value, "decoded_instance")?.is_null() {
            None
        } else {
            Some(field(value, "decoded_instance")?.clone())
        },
        legacy_raw_instance: has_raw.then(|| raw.to_vec()),
        owner_issue: parse_owner_issue(field(value, "owner_issue")?)?,
    })
}

fn issue_value(row: &SourceFoundationRecordsIssue) -> Value {
    json!({"v":STORE_CODEC_VERSION,"family":issue_family(row.family),"location":row.location,"message":row.message})
}

fn issue_from_value(value: &Value) -> io::Result<SourceFoundationRecordsIssue> {
    if uint(value, "v")? != STORE_CODEC_VERSION {
        return Err(invalid("owner issue codec version invalid"));
    }
    Ok(SourceFoundationRecordsIssue {
        family: parse_issue_family(text(value, "family")?)?,
        location: owned_text(value, "location")?,
        message: owned_text(value, "message")?,
    })
}

fn fact_value(fact: &SourceFoundationRecordFact) -> Value {
    match fact {
        SourceFoundationRecordFact::Observation(row) => {
            json!({"v":FACT_CODEC_VERSION,"tag":"observation","ordinal":row.ordinal,"observation":observation_value(&row.observation)})
        }
        SourceFoundationRecordFact::GlobalId(row) => {
            json!({"v":FACT_CODEC_VERSION,"tag":"global_id","ordinal":row.ordinal,"id":row.id,"kind":row.kind,"path":row.path,"carrier":match row.carrier { SourceFoundationRecordIdCarrier::Standalone=>"standalone",SourceFoundationRecordIdCarrier::NativePacket=>"native_packet" }})
        }
        SourceFoundationRecordFact::LinkUri(row) => {
            json!({"v":FACT_CODEC_VERSION,"tag":"link_uri","ordinal":row.ordinal,"uri":row.uri,"id":row.id,"path":row.path})
        }
        SourceFoundationRecordFact::TypedIdRef(row) => {
            json!({"v":FACT_CODEC_VERSION,"tag":"typed_id_ref","ordinal":row.ordinal,"target_id":row.target_id,"expected_kind":row.expected_kind,"from_path":row.from_path})
        }
        SourceFoundationRecordFact::PathReference(row) => {
            json!({"v":FACT_CODEC_VERSION,"tag":"path_reference","ordinal":row.ordinal,"from_path":row.from_path,"target_path":row.target_path,"check":match row.check { PathReferenceCheck::RepoExistsIfToS=>"repo_exists_if_tos",PathReferenceCheck::FileIfToS=>"file_if_tos" }})
        }
    }
}

fn fact_from_value(
    value: &Value,
    collection: SourceFoundationRecordFactCollection,
) -> io::Result<SourceFoundationRecordFact> {
    if uint(value, "v")? != FACT_CODEC_VERSION {
        return Err(invalid("record fact codec version invalid"));
    }
    let ordinal = uint(value, "ordinal")?;
    let expected_tag = match collection {
        SourceFoundationRecordFactCollection::Observations => "observation",
        SourceFoundationRecordFactCollection::GlobalIdFacts => "global_id",
        SourceFoundationRecordFactCollection::LinkUriFacts => "link_uri",
        SourceFoundationRecordFactCollection::TypedIdRefFacts => "typed_id_ref",
        SourceFoundationRecordFactCollection::PathReferenceFacts => "path_reference",
    };
    if text(value, "tag")? != expected_tag {
        return Err(invalid("record fact collection does not match stored row"));
    }
    let fields = value.get("fields").unwrap_or(value);
    Ok(match collection {
        SourceFoundationRecordFactCollection::Observations => {
            SourceFoundationRecordFact::Observation(SourceFoundationRecordObservation {
                ordinal,
                observation: observation_from_value(field(fields, "observation")?)?,
            })
        }
        SourceFoundationRecordFactCollection::GlobalIdFacts => {
            SourceFoundationRecordFact::GlobalId(SourceFoundationGlobalIdFact {
                ordinal,
                id: owned_text(fields, "id")?,
                kind: owned_text(fields, "kind")?,
                path: owned_text(fields, "path")?,
                carrier: match text(fields, "carrier")? {
                    "standalone" => SourceFoundationRecordIdCarrier::Standalone,
                    "native_packet" => SourceFoundationRecordIdCarrier::NativePacket,
                    _ => return Err(invalid("record ID carrier invalid")),
                },
            })
        }
        SourceFoundationRecordFactCollection::LinkUriFacts => {
            SourceFoundationRecordFact::LinkUri(SourceFoundationLinkUriFact {
                ordinal,
                uri: owned_text(fields, "uri")?,
                id: owned_text(fields, "id")?,
                path: owned_text(fields, "path")?,
            })
        }
        SourceFoundationRecordFactCollection::TypedIdRefFacts => {
            SourceFoundationRecordFact::TypedIdRef(SourceFoundationTypedIdRefFact {
                ordinal,
                target_id: owned_text(fields, "target_id")?,
                expected_kind: owned_text(fields, "expected_kind")?,
                from_path: owned_text(fields, "from_path")?,
            })
        }
        SourceFoundationRecordFactCollection::PathReferenceFacts => {
            SourceFoundationRecordFact::PathReference(SourceFoundationRecordPathReference {
                ordinal,
                from_path: owned_text(fields, "from_path")?,
                target_path: owned_text(fields, "target_path")?,
                check: match text(fields, "check")? {
                    "repo_exists_if_tos" => PathReferenceCheck::RepoExistsIfToS,
                    "file_if_tos" => PathReferenceCheck::FileIfToS,
                    _ => return Err(invalid("record path-reference check invalid")),
                },
            })
        }
    })
}

#[derive(Clone, Copy)]
enum WriteLaw {
    Append,
    First,
    Replace,
}

const fn collection_id(collection: SourceFoundationRecordsCollection) -> i64 {
    match collection {
        SourceFoundationRecordsCollection::CurrentRecords => 0,
        SourceFoundationRecordsCollection::UsedDeclaredProfileKinds => 1,
        SourceFoundationRecordsCollection::ItemRecordSelections => 2,
        SourceFoundationRecordsCollection::RecordSchemaPositions => 3,
        SourceFoundationRecordsCollection::FileDescriptors => 4,
        SourceFoundationRecordsCollection::ItemFileMemberships => 5,
        SourceFoundationRecordsCollection::LinkUriOwners => 6,
        SourceFoundationRecordsCollection::RightsIds => 7,
        SourceFoundationRecordsCollection::ItemEditions => 8,
        SourceFoundationRecordsCollection::SourceEventInsertions => 9,
        SourceFoundationRecordsCollection::OrderedIssues => 10,
        SourceFoundationRecordsCollection::SchemaChecks => 11,
        SourceFoundationRecordsCollection::ItemIssues => 12,
        SourceFoundationRecordsCollection::ManifestItemIds => 13,
        SourceFoundationRecordsCollection::RecordObservations => 14,
        SourceFoundationRecordsCollection::RecordSchemaDiagnostics => 15,
    }
}

const fn fact_collection_id(collection: SourceFoundationRecordFactCollection) -> i64 {
    match collection {
        SourceFoundationRecordFactCollection::Observations => 0,
        SourceFoundationRecordFactCollection::GlobalIdFacts => 1,
        SourceFoundationRecordFactCollection::LinkUriFacts => 2,
        SourceFoundationRecordFactCollection::TypedIdRefFacts => 3,
        SourceFoundationRecordFactCollection::PathReferenceFacts => 4,
    }
}

fn sorted_collection(collection: SourceFoundationRecordsCollection) -> bool {
    matches!(
        collection,
        SourceFoundationRecordsCollection::UsedDeclaredProfileKinds
            | SourceFoundationRecordsCollection::FileDescriptors
            | SourceFoundationRecordsCollection::ItemFileMemberships
            | SourceFoundationRecordsCollection::LinkUriOwners
            | SourceFoundationRecordsCollection::RightsIds
    )
}

fn sorted_fact_collection(collection: SourceFoundationRecordFactCollection) -> bool {
    matches!(
        collection,
        SourceFoundationRecordFactCollection::GlobalIdFacts
            | SourceFoundationRecordFactCollection::LinkUriFacts
            | SourceFoundationRecordFactCollection::TypedIdRefFacts
    )
}

struct Cursor<'a> {
    tag: u8,
    mode: u8,
    seq: i64,
    key1: &'a str,
    key2: &'a str,
}

fn make_cursor(
    tag: u8,
    mode: u8,
    seq: i64,
    key1: &str,
    key2: &str,
    max_bytes: usize,
) -> io::Result<SourceFoundationRecordsCursor> {
    let len = 15usize
        .checked_add(key1.len())
        .and_then(|n| n.checked_add(key2.len()))
        .ok_or_else(|| invalid("source-foundation cursor size overflow"))?;
    if len > max_bytes {
        return Err(invalid(
            "source-foundation cursor exceeds selected envelope",
        ));
    }
    let _precharge = checked_add(64, len)?;
    let mut bytes = Vec::with_capacity(len);
    bytes.push(1);
    bytes.push(tag);
    bytes.push(mode);
    bytes.extend_from_slice(&seq.to_be_bytes());
    bytes.extend_from_slice(
        &(u16::try_from(key1.len())
            .map_err(|_| invalid("source-foundation cursor key too long"))?)
        .to_be_bytes(),
    );
    bytes.extend_from_slice(key1.as_bytes());
    bytes.extend_from_slice(
        &(u16::try_from(key2.len())
            .map_err(|_| invalid("source-foundation cursor key too long"))?)
        .to_be_bytes(),
    );
    bytes.extend_from_slice(key2.as_bytes());
    Ok(SourceFoundationRecordsCursor::from_bytes(bytes))
}

fn parse_cursor<'a>(
    cursor: Option<&'a SourceFoundationRecordsCursor>,
    tag: u8,
    mode: u8,
    max_bytes: usize,
) -> io::Result<Option<Cursor<'a>>> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let raw = cursor.as_bytes();
    if raw.len() > max_bytes || raw.len() < 15 || raw[0] != 1 || raw[1] != tag || raw[2] != mode {
        return Err(invalid("source-foundation cursor binding or codec invalid"));
    }
    let seq = i64::from_be_bytes(
        raw[3..11]
            .try_into()
            .map_err(|_| invalid("source-foundation cursor ordinal invalid"))?,
    );
    let first_len = u16::from_be_bytes(
        raw[11..13]
            .try_into()
            .map_err(|_| invalid("source-foundation cursor key invalid"))?,
    ) as usize;
    let first_end = 13usize
        .checked_add(first_len)
        .ok_or_else(|| invalid("source-foundation cursor overflow"))?;
    let second_len_end = first_end
        .checked_add(2)
        .ok_or_else(|| invalid("source-foundation cursor overflow"))?;
    if second_len_end > raw.len() {
        return Err(invalid("source-foundation cursor truncated"));
    }
    let key1 = std::str::from_utf8(&raw[13..first_end])
        .map_err(|_| invalid("source-foundation cursor key is not UTF-8"))?;
    let second_len = u16::from_be_bytes(
        raw[first_end..second_len_end]
            .try_into()
            .map_err(|_| invalid("source-foundation cursor key invalid"))?,
    ) as usize;
    let second_end = second_len_end
        .checked_add(second_len)
        .ok_or_else(|| invalid("source-foundation cursor overflow"))?;
    if second_end != raw.len() {
        return Err(invalid("source-foundation cursor trailing bytes"));
    }
    let key2 = std::str::from_utf8(&raw[second_len_end..second_end])
        .map_err(|_| invalid("source-foundation cursor key is not UTF-8"))?;
    Ok(Some(Cursor {
        tag,
        mode,
        seq,
        key1,
        key2,
    }))
}

fn check_operation(
    sink: &IndexSink<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), ItemRefusal> {
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source(
            "source-foundation index operation cancelled".into(),
        ));
    }
    if !sink.candidate.matches_invocation(deadline, cancelled) {
        return Err(ItemRefusal::Source(
            "source-foundation index invocation identity changed".into(),
        ));
    }
    sink.candidate.tick().map_err(refusal)
}

fn observation_state(row: &RecordObservation) -> io::Result<usize> {
    let text_len = match row {
        RecordObservation::ExactPath { path, raw_sha256 } => string_fields(&[path, raw_sha256])?,
        RecordObservation::Registry {
            path,
            version,
            raw_sha256,
        } => string_fields(&[path, version, raw_sha256])?,
        RecordObservation::Schema {
            path,
            uri,
            raw_sha256,
        } => string_fields(&[path, uri, raw_sha256])?,
        RecordObservation::Profile {
            path,
            kind,
            schema_version,
            ..
        } => string_fields(&[path, kind, schema_version])?,
        RecordObservation::Reference {
            from_path,
            target_path,
            ..
        } => string_fields(&[from_path, target_path])?,
        RecordObservation::RecordIdReference {
            from_path,
            target_id,
            expected_kind,
        } => string_fields(&[from_path, target_id, expected_kind])?,
        RecordObservation::LinkUriOwner { uri, id, path } => string_fields(&[uri, id, path])?,
        RecordObservation::IdOwner {
            id,
            kind,
            path,
            raw_sha256,
            ..
        } => string_fields(&[id, kind, path, raw_sha256])?,
        RecordObservation::IdKindOwner { kind, id, path } => string_fields(&[kind, id, path])?,
        RecordObservation::NativeReservation {
            id,
            packet_path,
            raw_sha256,
        } => string_fields(&[id, packet_path, raw_sha256])?,
        RecordObservation::Issue { path, code } => string_fields(&[path, code])?,
    };
    checked_add(std::mem::size_of::<RecordObservation>(), text_len)
}

impl IndexSink<'_> {
    fn append_stored_row(
        &mut self,
        collection: SourceFoundationRecordsCollection,
        key1: &str,
        key2: &str,
        value: &Value,
        aux: &[u8],
        source_state: usize,
        law: WriteLaw,
        deadline: Option<Instant>,
        cancelled: Option<&AtomicBool>,
    ) -> io::Result<i64> {
        if let (Some(deadline), Some(cancelled)) = (deadline, cancelled) {
            check_operation(self, deadline, cancelled)
                .map_err(|_| invalid("source-foundation index operation expired"))?;
        } else {
            self.candidate.tick()?;
        }
        let id = collection_id(collection) as usize;
        let seq = self.records_collection_ordinals[id];
        let next = seq
            .checked_add(1)
            .ok_or_else(|| invalid("source-foundation collection ordinal overflow"))?;
        let seq_i64 = i64::try_from(seq)
            .map_err(|_| invalid("source-foundation collection ordinal exceeds SQLite range"))?;
        let (payload, state_bytes) = encode_json(self, source_state, value, aux)?;
        let state_i64 = i64::try_from(state_bytes)
            .map_err(|_| invalid("source-foundation row state exceeds SQLite range"))?;
        self.candidate.tick()?;
        match law {
            WriteLaw::Append => {
                self.db.execute("INSERT INTO sf_rows(collection,seq,key1,key2,payload,aux,state_bytes) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![collection_id(collection),seq_i64,key1,key2,payload,aux,state_i64]).map_err(sql)?;
            }
            WriteLaw::First => {
                self.db.execute("INSERT OR IGNORE INTO sf_rows(collection,seq,key1,key2,payload,aux,state_bytes) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![collection_id(collection),seq_i64,key1,key2,payload,aux,state_i64]).map_err(sql)?;
            }
            WriteLaw::Replace => {
                self.db.execute("INSERT INTO sf_rows(collection,seq,key1,key2,payload,aux,state_bytes) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT DO UPDATE SET payload=excluded.payload,aux=excluded.aux,state_bytes=excluded.state_bytes",params![collection_id(collection),seq_i64,key1,key2,payload,aux,state_i64]).map_err(sql)?;
            }
        }
        self.candidate.tick()?;
        self.records_collection_ordinals[id] = next;
        Ok(seq_i64)
    }

    fn append_fact(
        &mut self,
        collection: SourceFoundationRecordFactCollection,
        ordinal: u64,
        key: &str,
        value: &Value,
        source_state: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        check_operation(self, deadline, cancelled)?;
        let (payload, state_bytes) =
            encode_json(self, source_state, value, &[]).map_err(refusal)?;
        let ordinal_i64 = i64::try_from(ordinal).map_err(|_| ItemRefusal::Budget)?;
        let state_i64 = i64::try_from(state_bytes).map_err(|_| ItemRefusal::Budget)?;
        self.candidate.tick().map_err(refusal)?;
        self.db.execute("INSERT INTO sf_facts(collection,ordinal,key1,payload,state_bytes) VALUES(?1,?2,?3,?4,?5)",params![fact_collection_id(collection),ordinal_i64,key,payload,state_i64]).map_err(refusal)?;
        self.candidate.tick().map_err(refusal)?;
        Ok(())
    }
}

fn stored_fact_from_value(
    collection: SourceFoundationRecordsCollection,
    value: &Value,
    aux: &[u8],
) -> io::Result<SourceFoundationRecordsStoredFact> {
    if uint(value, "v")? != STORE_CODEC_VERSION {
        return Err(invalid("stored Records row codec version invalid"));
    }
    Ok(match collection {
        SourceFoundationRecordsCollection::CurrentRecords => {
            SourceFoundationRecordsStoredFact::CurrentRecord {
                record_id: owned_text(value, "record_id")?,
                record: biblio_record_from_value(field(value, "record")?)?,
            }
        }
        SourceFoundationRecordsCollection::UsedDeclaredProfileKinds => {
            SourceFoundationRecordsStoredFact::UsedDeclaredProfileKind(owned_text(value, "value")?)
        }
        SourceFoundationRecordsCollection::ItemRecordSelections => {
            SourceFoundationRecordsStoredFact::ItemRecordSelection(item_selection_from_value(
                field(value, "selection")?,
            )?)
        }
        SourceFoundationRecordsCollection::RecordSchemaPositions => {
            SourceFoundationRecordsStoredFact::RecordSchemaPosition {
                path: owned_text(value, "path")?,
                before_issue: usize_value(value, "before_issue")?,
            }
        }
        SourceFoundationRecordsCollection::FileDescriptors => {
            SourceFoundationRecordsStoredFact::FileDescriptor {
                file_id: owned_text(value, "file_id")?,
                sha256: field(value, "sha256")?.clone(),
                byte_size: field(value, "byte_size")?.clone(),
                media_type: field(value, "media_type")?.clone(),
            }
        }
        SourceFoundationRecordsCollection::ItemFileMemberships => {
            SourceFoundationRecordsStoredFact::ItemFileMembership {
                file_id: owned_text(value, "file_id")?,
                item_id: owned_text(value, "item_id")?,
            }
        }
        SourceFoundationRecordsCollection::LinkUriOwners => {
            SourceFoundationRecordsStoredFact::LinkUriOwner {
                uri: owned_text(value, "uri")?,
                record_id: owned_text(value, "record_id")?,
            }
        }
        SourceFoundationRecordsCollection::RightsIds => {
            SourceFoundationRecordsStoredFact::RightsId(owned_text(value, "value")?)
        }
        SourceFoundationRecordsCollection::ItemEditions => {
            SourceFoundationRecordsStoredFact::ItemEdition {
                item_id: owned_text(value, "item_id")?,
                embodiment_ref: owned_text(value, "embodiment_ref")?,
            }
        }
        SourceFoundationRecordsCollection::SourceEventInsertions => {
            SourceFoundationRecordsStoredFact::SourceEventInsertion((
                owned_text(value, "event_id")?,
                field(value, "event")?.clone(),
            ))
        }
        SourceFoundationRecordsCollection::OrderedIssues => {
            SourceFoundationRecordsStoredFact::OrderedIssue(issue_from_value(value)?)
        }
        SourceFoundationRecordsCollection::SchemaChecks => {
            SourceFoundationRecordsStoredFact::SchemaCheck(schema_check_from_value(value, aux)?)
        }
        SourceFoundationRecordsCollection::ItemIssues => {
            SourceFoundationRecordsStoredFact::ItemIssue(ItemIssue {
                path: owned_text(value, "path")?,
                code: static_item_code(text(value, "code")?)?,
            })
        }
        SourceFoundationRecordsCollection::ManifestItemIds => {
            SourceFoundationRecordsStoredFact::ManifestItemId(owned_text(value, "value")?)
        }
        SourceFoundationRecordsCollection::RecordObservations => {
            SourceFoundationRecordsStoredFact::RecordObservation(
                SourceFoundationRecordObservation {
                    ordinal: uint(value, "ordinal")?,
                    observation: observation_from_value(field(value, "observation")?)?,
                },
            )
        }
        SourceFoundationRecordsCollection::RecordSchemaDiagnostics => {
            SourceFoundationRecordsStoredFact::RecordSchemaDiagnostic(
                SourceFoundationRecordSchemaDiagnostic {
                    ordinal: uint(value, "ordinal")?,
                    diagnostic: diagnostic_from_value(field(value, "diagnostic")?)?,
                },
            )
        }
    })
}

fn input_state_for_value(value: &Value, strings: &[&str]) -> io::Result<usize> {
    let data = strings
        .iter()
        .try_fold(512usize, |n, s| checked_add(n, s.len()))?;
    checked_add(data, value_state(value)?)
}

fn table_preflight(
    db: &rusqlite::Connection,
    collection: i64,
    key1: &str,
    key2: &str,
    max_state: usize,
    candidate: &dyn CandidateFenceSource,
) -> io::Result<Option<(usize, usize, usize)>> {
    let row = db.query_row("SELECT length(payload),length(aux),state_bytes FROM sf_rows WHERE collection=?1 AND key1=?2 COLLATE BINARY AND key2=?3 COLLATE BINARY",params![collection,key1,key2],|row| {
        let payload: i64 = row.get(0)?;
        let aux: i64 = row.get(1)?;
        let state: i64 = row.get(2)?;
        Ok((payload,aux,state))
    }).optional().map_err(sql)?;
    let Some((payload, aux, state)) = row else {
        return Ok(None);
    };
    let payload =
        usize::try_from(payload).map_err(|_| invalid("stored row payload length invalid"))?;
    let aux = usize::try_from(aux).map_err(|_| invalid("stored row auxiliary length invalid"))?;
    let state = usize::try_from(state).map_err(|_| invalid("stored row state invalid"))?;
    if state > max_state
        || payload
            .checked_add(aux)
            .and_then(|n| n.checked_add(1024))
            .is_none_or(|n| n > max_state)
    {
        return Err(invalid("stored row exceeds point-read envelope"));
    }
    candidate.check_state(state.max(payload.saturating_add(aux).saturating_add(1024)))?;
    Ok(Some((payload, aux, state)))
}

fn table_read_payload(
    db: &rusqlite::Connection,
    collection: i64,
    key1: &str,
    key2: &str,
    expected: (usize, usize, usize),
) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let (payload,aux): (Vec<u8>,Vec<u8>) = db.query_row("SELECT payload,aux FROM sf_rows WHERE collection=?1 AND key1=?2 COLLATE BINARY AND key2=?3 COLLATE BINARY",params![collection,key1,key2],|row| Ok((row.get(0)?,row.get(1)?))).map_err(sql)?;
    if payload.len() != expected.0 || aux.len() != expected.1 || expected.2 == 0 {
        return Err(invalid("stored row changed during bounded point read"));
    }
    Ok((payload, aux))
}

fn cursor_tag_rows(collection: SourceFoundationRecordsCollection) -> u8 {
    collection_id(collection) as u8
}
fn cursor_tag_facts(collection: SourceFoundationRecordFactCollection) -> u8 {
    0x80 | fact_collection_id(collection) as u8
}

fn collection_from_id(id: i64) -> io::Result<SourceFoundationRecordsCollection> {
    Ok(match id {
        0 => SourceFoundationRecordsCollection::CurrentRecords,
        1 => SourceFoundationRecordsCollection::UsedDeclaredProfileKinds,
        2 => SourceFoundationRecordsCollection::ItemRecordSelections,
        3 => SourceFoundationRecordsCollection::RecordSchemaPositions,
        4 => SourceFoundationRecordsCollection::FileDescriptors,
        5 => SourceFoundationRecordsCollection::ItemFileMemberships,
        6 => SourceFoundationRecordsCollection::LinkUriOwners,
        7 => SourceFoundationRecordsCollection::RightsIds,
        8 => SourceFoundationRecordsCollection::ItemEditions,
        9 => SourceFoundationRecordsCollection::SourceEventInsertions,
        10 => SourceFoundationRecordsCollection::OrderedIssues,
        11 => SourceFoundationRecordsCollection::SchemaChecks,
        12 => SourceFoundationRecordsCollection::ItemIssues,
        13 => SourceFoundationRecordsCollection::ManifestItemIds,
        14 => SourceFoundationRecordsCollection::RecordObservations,
        15 => SourceFoundationRecordsCollection::RecordSchemaDiagnostics,
        _ => return Err(invalid("source-foundation collection cursor invalid")),
    })
}

#[derive(Clone, Copy)]
struct PageMeta {
    seq: i64,
    key1_bytes: usize,
    key2_bytes: usize,
    payload_bytes: usize,
    aux_bytes: usize,
    state_bytes: usize,
}

fn sqlite_usize(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<usize> {
    let raw: i64 = row.get(column)?;
    usize::try_from(raw).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn rows_meta(
    sink: &IndexSink<'_>,
    collection: SourceFoundationRecordsCollection,
    after: Option<&Cursor<'_>>,
    limit: usize,
    budget: SourceFoundationRecordsPageBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<PageMeta>, ItemRefusal> {
    check_operation(sink, deadline, cancelled)?;
    let sorted = sorted_collection(collection);
    let query = match (sorted, after) {
        (false, _) => {
            "SELECT seq,length(CAST(key1 AS BLOB)),length(CAST(key2 AS BLOB)),length(payload),length(aux),state_bytes FROM sf_rows WHERE collection=?1 AND seq>?2 ORDER BY seq LIMIT ?3"
        }
        (true, None) => {
            "SELECT seq,length(CAST(key1 AS BLOB)),length(CAST(key2 AS BLOB)),length(payload),length(aux),state_bytes FROM sf_rows WHERE collection=?1 ORDER BY key1 COLLATE BINARY,key2 COLLATE BINARY LIMIT ?2"
        }
        (true, Some(_)) => {
            "SELECT seq,length(CAST(key1 AS BLOB)),length(CAST(key2 AS BLOB)),length(payload),length(aux),state_bytes FROM sf_rows WHERE collection=?1 AND (key1 COLLATE BINARY,key2 COLLATE BINARY)>(?2 COLLATE BINARY,?3 COLLATE BINARY) ORDER BY key1 COLLATE BINARY,key2 COLLATE BINARY LIMIT ?4"
        }
    };
    let capacity = limit.min(budget.max_state_bytes.get() / std::mem::size_of::<PageMeta>().max(1));
    if capacity == 0 {
        return Err(ItemRefusal::Budget);
    }
    sink.candidate
        .check_state(
            capacity
                .checked_mul(std::mem::size_of::<PageMeta>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .map_err(refusal)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| ItemRefusal::Budget)?;
    let mut statement = sink.db.prepare(query).map_err(refusal)?;
    let limit_i64 = i64::try_from(limit).map_err(|_| ItemRefusal::Budget)?;
    let mut rows = match (sorted, after) {
        (false, _) => statement.query(params![
            collection_id(collection),
            after.map_or(-1, |cursor| cursor.seq),
            limit_i64
        ]),
        (true, None) => statement.query(params![collection_id(collection), limit_i64]),
        (true, Some(cursor)) => statement.query(params![
            collection_id(collection),
            cursor.key1,
            cursor.key2,
            limit_i64
        ]),
    }
    .map_err(refusal)?;
    while let Some(row) = rows.next().map_err(refusal)? {
        check_operation(sink, deadline, cancelled)?;
        if result.len() >= capacity {
            return Err(ItemRefusal::Budget);
        }
        result.push(PageMeta {
            seq: row.get(0).map_err(refusal)?,
            key1_bytes: sqlite_usize(row, 1).map_err(refusal)?,
            key2_bytes: sqlite_usize(row, 2).map_err(refusal)?,
            payload_bytes: sqlite_usize(row, 3).map_err(refusal)?,
            aux_bytes: sqlite_usize(row, 4).map_err(refusal)?,
            state_bytes: sqlite_usize(row, 5).map_err(refusal)?,
        });
    }
    drop(rows);
    drop(statement);
    check_operation(sink, deadline, cancelled)?;
    Ok(result)
}

fn fact_rows_meta(
    sink: &IndexSink<'_>,
    collection: SourceFoundationRecordFactCollection,
    after: Option<&Cursor<'_>>,
    limit: usize,
    budget: SourceFoundationRecordsPageBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<PageMeta>, ItemRefusal> {
    check_operation(sink, deadline, cancelled)?;
    let sorted = sorted_fact_collection(collection);
    let query = match (sorted, after) {
        (false, _) => {
            "SELECT ordinal,length(CAST(key1 AS BLOB)),length(payload),state_bytes FROM sf_facts WHERE collection=?1 AND ordinal>?2 ORDER BY ordinal LIMIT ?3"
        }
        (true, None) => {
            "SELECT ordinal,length(CAST(key1 AS BLOB)),length(payload),state_bytes FROM sf_facts WHERE collection=?1 ORDER BY key1 COLLATE BINARY,ordinal LIMIT ?2"
        }
        (true, Some(_)) => {
            "SELECT ordinal,length(CAST(key1 AS BLOB)),length(payload),state_bytes FROM sf_facts WHERE collection=?1 AND (key1 COLLATE BINARY,ordinal)>(?2 COLLATE BINARY,?3) ORDER BY key1 COLLATE BINARY,ordinal LIMIT ?4"
        }
    };
    let capacity = limit.min(budget.max_state_bytes.get() / std::mem::size_of::<PageMeta>().max(1));
    if capacity == 0 {
        return Err(ItemRefusal::Budget);
    }
    sink.candidate
        .check_state(
            capacity
                .checked_mul(std::mem::size_of::<PageMeta>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .map_err(refusal)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| ItemRefusal::Budget)?;
    let mut statement = sink.db.prepare(query).map_err(refusal)?;
    let limit_i64 = i64::try_from(limit).map_err(|_| ItemRefusal::Budget)?;
    let mut rows = match (sorted, after) {
        (false, _) => statement.query(params![
            fact_collection_id(collection),
            after.map_or(-1, |cursor| cursor.seq),
            limit_i64
        ]),
        (true, None) => statement.query(params![fact_collection_id(collection), limit_i64]),
        (true, Some(cursor)) => statement.query(params![
            fact_collection_id(collection),
            cursor.key1,
            cursor.seq,
            limit_i64
        ]),
    }
    .map_err(refusal)?;
    while let Some(row) = rows.next().map_err(refusal)? {
        check_operation(sink, deadline, cancelled)?;
        if result.len() >= capacity {
            return Err(ItemRefusal::Budget);
        }
        result.push(PageMeta {
            seq: row.get(0).map_err(refusal)?,
            key1_bytes: sqlite_usize(row, 1).map_err(refusal)?,
            key2_bytes: 0,
            payload_bytes: sqlite_usize(row, 2).map_err(refusal)?,
            aux_bytes: 0,
            state_bytes: sqlite_usize(row, 3).map_err(refusal)?,
        });
    }
    drop(rows);
    drop(statement);
    check_operation(sink, deadline, cancelled)?;
    Ok(result)
}

fn page_cost(
    metas: &[PageMeta],
    capacity: usize,
    metadata_capacity: usize,
    input_cursor: usize,
    output_cursor: usize,
    include_aux: bool,
) -> io::Result<usize> {
    let mut bytes = std::mem::size_of::<SourceFoundationRecordsCursorPage>();
    bytes = checked_add(
        bytes,
        capacity
            .checked_mul(std::mem::size_of::<SourceFoundationRecordsStoredFact>())
            .ok_or_else(|| invalid("page capacity state overflow"))?,
    )?;
    bytes = checked_add(
        bytes,
        metadata_capacity
            .checked_mul(std::mem::size_of::<PageMeta>())
            .ok_or_else(|| invalid("page metadata state overflow"))?,
    )?;
    bytes = checked_add(bytes, input_cursor)?;
    bytes = checked_add(bytes, output_cursor)?;
    for meta in metas {
        bytes = checked_add(bytes, meta.state_bytes)?;
        if include_aux {
            bytes = checked_add(bytes, meta.aux_bytes)?;
        }
    }
    Ok(bytes)
}

fn fact_page_cost(
    metas: &[PageMeta],
    capacity: usize,
    metadata_capacity: usize,
    input_cursor: usize,
    output_cursor: usize,
) -> io::Result<usize> {
    let mut bytes = std::mem::size_of::<SourceFoundationRecordFactPage>();
    bytes = checked_add(
        bytes,
        capacity
            .checked_mul(std::mem::size_of::<SourceFoundationRecordFact>())
            .ok_or_else(|| invalid("fact page capacity state overflow"))?,
    )?;
    bytes = checked_add(
        bytes,
        metadata_capacity
            .checked_mul(std::mem::size_of::<PageMeta>())
            .ok_or_else(|| invalid("fact page metadata state overflow"))?,
    )?;
    bytes = checked_add(bytes, input_cursor)?;
    bytes = checked_add(bytes, output_cursor)?;
    for meta in metas {
        bytes = checked_add(bytes, meta.state_bytes)?;
    }
    Ok(bytes)
}

fn output_cursor_len(meta: &PageMeta, sorted: bool) -> io::Result<usize> {
    let key_bytes = if sorted {
        checked_add(meta.key1_bytes, meta.key2_bytes)?
    } else {
        0
    };
    checked_add(15, key_bytes)
}

fn stored_fact_cursor_keys<'a>(
    collection: SourceFoundationRecordsCollection,
    row: &'a SourceFoundationRecordsStoredFact,
) -> io::Result<(&'a str, &'a str)> {
    Ok(match (collection, row) {
        (
            SourceFoundationRecordsCollection::CurrentRecords,
            SourceFoundationRecordsStoredFact::CurrentRecord { record_id, .. },
        ) => (record_id, ""),
        (
            SourceFoundationRecordsCollection::UsedDeclaredProfileKinds,
            SourceFoundationRecordsStoredFact::UsedDeclaredProfileKind(key),
        ) => (key, ""),
        (
            SourceFoundationRecordsCollection::ItemRecordSelections,
            SourceFoundationRecordsStoredFact::ItemRecordSelection(row),
        ) => (row.record_id.as_str(), ""),
        (
            SourceFoundationRecordsCollection::FileDescriptors,
            SourceFoundationRecordsStoredFact::FileDescriptor { file_id, .. },
        ) => (file_id, ""),
        (
            SourceFoundationRecordsCollection::ItemFileMemberships,
            SourceFoundationRecordsStoredFact::ItemFileMembership { file_id, item_id },
        ) => (file_id, item_id),
        (
            SourceFoundationRecordsCollection::LinkUriOwners,
            SourceFoundationRecordsStoredFact::LinkUriOwner { uri, .. },
        ) => (uri, ""),
        (
            SourceFoundationRecordsCollection::RightsIds,
            SourceFoundationRecordsStoredFact::RightsId(key),
        ) => (key, ""),
        _ => {
            return Err(invalid(
                "source-foundation cursor key row does not match collection",
            ));
        }
    })
}

fn fact_cursor_key<'a>(
    collection: SourceFoundationRecordFactCollection,
    row: &'a SourceFoundationRecordFact,
) -> io::Result<&'a str> {
    Ok(match (collection, row) {
        (
            SourceFoundationRecordFactCollection::GlobalIdFacts,
            SourceFoundationRecordFact::GlobalId(row),
        ) => &row.id,
        (
            SourceFoundationRecordFactCollection::LinkUriFacts,
            SourceFoundationRecordFact::LinkUri(row),
        ) => &row.uri,
        (
            SourceFoundationRecordFactCollection::TypedIdRefFacts,
            SourceFoundationRecordFact::TypedIdRef(row),
        ) => &row.target_id,
        (
            SourceFoundationRecordFactCollection::Observations,
            SourceFoundationRecordFact::Observation(_),
        ) => "",
        (
            SourceFoundationRecordFactCollection::PathReferenceFacts,
            SourceFoundationRecordFact::PathReference(_),
        ) => "",
        _ => return Err(invalid("Biblio cursor key row does not match collection")),
    })
}

fn read_row_for_page(
    sink: &IndexSink<'_>,
    collection: SourceFoundationRecordsCollection,
    seq: i64,
    expected: PageMeta,
    max_state: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(String, String, Vec<u8>, Vec<u8>), ItemRefusal> {
    check_operation(sink, deadline, cancelled)?;
    let state = expected.state_bytes;
    if state > max_state {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation row page decode state",
            used: Some(state as u64),
            limit: Some(max_state as u64),
        });
    }
    sink.candidate.check_state(state).map_err(refusal)?;
    let row = sink
        .db
        .query_row(
            "SELECT key1,key2,payload,aux FROM sf_rows WHERE collection=?1 AND seq=?2",
            params![collection_id(collection), seq],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            },
        )
        .map_err(refusal)?;
    if row.0.len() != expected.key1_bytes
        || row.1.len() != expected.key2_bytes
        || row.2.len() != expected.payload_bytes
        || row.3.len() != expected.aux_bytes
    {
        return Err(ItemRefusal::Source(
            "source-foundation row changed during page read".into(),
        ));
    }
    check_operation(sink, deadline, cancelled)?;
    Ok(row)
}

fn read_fact_for_page(
    sink: &IndexSink<'_>,
    collection: SourceFoundationRecordFactCollection,
    ordinal: i64,
    expected: PageMeta,
    max_state: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(String, Vec<u8>), ItemRefusal> {
    check_operation(sink, deadline, cancelled)?;
    let state = expected.state_bytes;
    if state > max_state {
        return Err(ItemRefusal::BudgetCheck {
            check: "Biblio fact page decode state",
            used: Some(state as u64),
            limit: Some(max_state as u64),
        });
    }
    sink.candidate.check_state(state).map_err(refusal)?;
    let row = sink
        .db
        .query_row(
            "SELECT key1,payload FROM sf_facts WHERE collection=?1 AND ordinal=?2",
            params![fact_collection_id(collection), ordinal],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?)),
        )
        .map_err(refusal)?;
    if row.0.len() != expected.key1_bytes || row.1.len() != expected.payload_bytes {
        return Err(ItemRefusal::Source(
            "Biblio fact changed during page read".into(),
        ));
    }
    check_operation(sink, deadline, cancelled)?;
    Ok(row)
}

fn page_row_capacity(
    budget: SourceFoundationRecordsPageBudget,
    input_cursor: usize,
    fact: bool,
) -> Result<usize, ItemRefusal> {
    let page_header = if fact {
        std::mem::size_of::<SourceFoundationRecordFactPage>()
    } else {
        std::mem::size_of::<SourceFoundationRecordsCursorPage>()
    };
    let row_size = if fact {
        std::mem::size_of::<SourceFoundationRecordFact>()
    } else {
        std::mem::size_of::<SourceFoundationRecordsStoredFact>()
    };
    let meta_size = std::mem::size_of::<PageMeta>();
    let fixed = page_header
        .checked_add(input_cursor)
        .and_then(|n| n.checked_add(budget.max_cursor_bytes.get()))
        .and_then(|n| n.checked_add(meta_size.checked_mul(2)?))
        .ok_or(ItemRefusal::Budget)?;
    let per = row_size
        .checked_add(meta_size)
        .and_then(|n| n.checked_add(1))
        .ok_or(ItemRefusal::Budget)?;
    let available = budget
        .max_state_bytes
        .get()
        .checked_sub(fixed)
        .ok_or(ItemRefusal::Budget)?;
    let by_state = available / per;
    let rows = budget.max_rows.get().min(by_state);
    if rows == 0 {
        return Err(ItemRefusal::Budget);
    }
    Ok(rows)
}

fn report_page(
    sink: &IndexSink<'_>,
    collection: SourceFoundationRecordsCollection,
    after: Option<&SourceFoundationRecordsCursor>,
    budget: SourceFoundationRecordsPageBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<SourceFoundationRecordsCursorPage, ItemRefusal> {
    if after.is_some_and(|cursor| cursor.as_bytes().len() > budget.max_cursor_bytes.get()) {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation stored cursor input bytes",
            used: after
                .map(|c| Some(c.as_bytes().len() as u64))
                .unwrap_or(None),
            limit: Some(budget.max_cursor_bytes.get() as u64),
        });
    }
    check_operation(sink, deadline, cancelled)?;
    let id = cursor_tag_rows(collection);
    let mode = if sorted_collection(collection) { 3 } else { 1 };
    let parsed = parse_cursor(after, id, mode, budget.max_cursor_bytes.get()).map_err(refusal)?;
    let input_bytes = after.map_or(0, |cursor| cursor.as_bytes().len());
    let capacity = page_row_capacity(budget, input_bytes, false)?;
    let meta_capacity = capacity.checked_add(1).ok_or(ItemRefusal::Budget)?;
    let meta = rows_meta(
        sink,
        collection,
        parsed.as_ref(),
        meta_capacity,
        budget,
        deadline,
        cancelled,
    )?;
    let mut take = meta.len().min(capacity);
    let mut more = meta.len() > take;
    let mut cursor_len = if more {
        output_cursor_len(
            meta.get(take.saturating_sub(1))
                .ok_or(ItemRefusal::Budget)?,
            sorted_collection(collection),
        )
        .map_err(refusal)?
    } else {
        0
    };
    if cursor_len > budget.max_cursor_bytes.get() {
        return Err(ItemRefusal::Budget);
    }
    while take > 0
        && page_cost(
            &meta[..take],
            capacity,
            meta_capacity,
            input_bytes,
            cursor_len,
            true,
        )
        .map_err(refusal)?
            > budget.max_state_bytes.get()
    {
        take -= 1;
        more = meta.len() > take;
        if take == 0 {
            return Err(ItemRefusal::Budget);
        }
        cursor_len = if more {
            output_cursor_len(&meta[take - 1], sorted_collection(collection)).map_err(refusal)?
        } else {
            0
        };
        if cursor_len > budget.max_cursor_bytes.get() {
            return Err(ItemRefusal::Budget);
        }
    }
    if more && take == 0 {
        return Err(ItemRefusal::Budget);
    }
    let charged = page_cost(
        &meta[..take],
        capacity,
        meta_capacity,
        input_bytes,
        cursor_len,
        true,
    )
    .map_err(refusal)?;
    if charged > budget.max_state_bytes.get() {
        return Err(ItemRefusal::Budget);
    }
    sink.candidate.check_state(charged).map_err(refusal)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| ItemRefusal::Budget)?;
    for row in &meta[..take] {
        check_operation(sink, deadline, cancelled)?;
        let (key1, key2, payload, aux) = read_row_for_page(
            sink,
            collection,
            row.seq,
            *row,
            budget.max_state_bytes.get(),
            deadline,
            cancelled,
        )?;
        let value = decode_value(&payload, budget.max_state_bytes.get()).map_err(refusal)?;
        let fact = stored_fact_from_value(collection, &value, &aux).map_err(refusal)?;
        output.push(fact);
    }
    let next_cursor = if more {
        let last = &meta[take - 1];
        let (key1, key2) = if sorted_collection(collection) {
            stored_fact_cursor_keys(collection, output.last().ok_or(ItemRefusal::Budget)?)
                .map_err(refusal)?
        } else {
            ("", "")
        };
        Some(
            make_cursor(
                id,
                mode,
                last.seq,
                key1,
                key2,
                budget.max_cursor_bytes.get(),
            )
            .map_err(refusal)?,
        )
    } else {
        None
    };
    check_operation(sink, deadline, cancelled)?;
    let minimum = std::mem::size_of::<SourceFoundationRecordsCursorPage>()
        .checked_add(
            output
                .len()
                .checked_mul(std::mem::size_of::<SourceFoundationRecordsStoredFact>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .and_then(|n| n.checked_add(next_cursor.as_ref().map_or(0, |c| c.as_bytes().len())))
        .ok_or(ItemRefusal::Budget)?;
    if charged < minimum
        || next_cursor
            .as_ref()
            .is_some_and(|cursor| cursor.as_bytes().len() > budget.max_cursor_bytes.get())
    {
        return Err(ItemRefusal::Budget);
    }
    Ok(SourceFoundationRecordsCursorPage {
        rows: output,
        next_cursor,
        charged_state_bytes: charged,
    })
}

struct CurrentRecordIdMeta {
    id: String,
    stored_state_bytes: usize,
    decode_state_bytes: usize,
}

fn current_records_by_id_page(
    sink: &IndexSink<'_>,
    after_id: Option<&str>,
    budget: SourceFoundationRecordsPageBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<SourceFoundationCurrentRecordsPage, ItemRefusal> {
    if after_id.is_some_and(|id| id.len() > budget.max_cursor_bytes.get()) {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation current Record ID cursor bytes",
            used: after_id.map(|id| id.len() as u64),
            limit: Some(budget.max_cursor_bytes.get() as u64),
        });
    }
    check_operation(sink, deadline, cancelled)?;
    let input_cursor_bytes = after_id.map_or(0, str::len);
    let input_cursor_state = after_id.map_or(Ok(0), |id| {
        id.len()
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or(ItemRefusal::Budget)
    })?;
    let capacity = page_row_capacity(budget, input_cursor_bytes, false)?;
    let metadata_capacity = capacity.checked_add(1).ok_or(ItemRefusal::Budget)?;
    let metadata_headers = metadata_capacity
        .checked_mul(std::mem::size_of::<CurrentRecordIdMeta>())
        .and_then(|bytes| bytes.checked_add(input_cursor_state))
        .ok_or(ItemRefusal::Budget)?;
    if metadata_headers > budget.max_state_bytes.get() {
        return Err(ItemRefusal::Budget);
    }
    sink.candidate
        .check_state(metadata_headers)
        .map_err(refusal)?;
    let mut metadata = Vec::new();
    metadata
        .try_reserve_exact(metadata_capacity)
        .map_err(|_| ItemRefusal::Budget)?;
    let metadata_slot_capacity = metadata.capacity();
    if metadata_slot_capacity < metadata_capacity {
        return Err(ItemRefusal::Budget);
    }
    let allocated_metadata_headers = metadata_slot_capacity
        .checked_mul(std::mem::size_of::<CurrentRecordIdMeta>())
        .and_then(|bytes| bytes.checked_add(input_cursor_state))
        .ok_or(ItemRefusal::Budget)?;
    if allocated_metadata_headers > budget.max_state_bytes.get() {
        return Err(ItemRefusal::Budget);
    }
    sink.candidate
        .check_state(allocated_metadata_headers)
        .map_err(refusal)?;
    let limit = i64::try_from(metadata_capacity).map_err(|_| ItemRefusal::Budget)?;
    let mut statement = sink
        .db
        .prepare(if after_id.is_some() {
            "SELECT key1,state_bytes,length(payload),length(aux) FROM sf_rows WHERE collection=?1 AND key1>?2 COLLATE BINARY ORDER BY key1 COLLATE BINARY LIMIT ?3"
        } else {
            "SELECT key1,state_bytes,length(payload),length(aux) FROM sf_rows WHERE collection=?1 ORDER BY key1 COLLATE BINARY LIMIT ?2"
        })
        .map_err(refusal)?;
    let mut rows = if let Some(after_id) = after_id {
        statement.query(params![
            collection_id(SourceFoundationRecordsCollection::CurrentRecords),
            after_id,
            limit
        ])
    } else {
        statement.query(params![
            collection_id(SourceFoundationRecordsCollection::CurrentRecords),
            limit
        ])
    }
    .map_err(refusal)?;
    let mut metadata_state = allocated_metadata_headers;
    while let Some(row) = rows.next().map_err(refusal)? {
        check_operation(sink, deadline, cancelled)?;
        if metadata.len() >= metadata_capacity {
            return Err(ItemRefusal::Budget);
        }
        let raw_id_len = match row.get_ref(0).map_err(refusal)? {
            rusqlite::types::ValueRef::Text(raw) => raw.len(),
            _ => {
                return Err(ItemRefusal::Source(
                    "stored current Record ID is invalid".into(),
                ));
            }
        };
        let id_state = raw_id_len
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or(ItemRefusal::Budget)?;
        let next_metadata_state = metadata_state
            .checked_add(id_state)
            .ok_or(ItemRefusal::Budget)?;
        if next_metadata_state > budget.max_state_bytes.get() {
            return Err(ItemRefusal::Budget);
        }
        sink.candidate
            .check_state(next_metadata_state)
            .map_err(refusal)?;
        let id = bounded_text(
            row,
            0,
            budget
                .max_state_bytes
                .get()
                .checked_sub(metadata_state)
                .ok_or(ItemRefusal::Budget)?,
        )
        .map_err(refusal)?;
        if metadata
            .last()
            .is_some_and(|previous: &CurrentRecordIdMeta| previous.id >= id)
        {
            return Err(ItemRefusal::Source(
                "stored current Record ID index order changed".into(),
            ));
        }
        let stored_state_bytes = usize::try_from(row.get::<_, i64>(1).map_err(refusal)?)
            .map_err(|_| ItemRefusal::Budget)?;
        if stored_state_bytes == 0 || stored_state_bytes > budget.max_state_bytes.get() {
            return Err(ItemRefusal::Budget);
        }
        let payload_bytes = usize::try_from(row.get::<_, i64>(2).map_err(refusal)?)
            .map_err(|_| ItemRefusal::Budget)?;
        let aux_bytes = usize::try_from(row.get::<_, i64>(3).map_err(refusal)?)
            .map_err(|_| ItemRefusal::Budget)?;
        let decode_state_bytes = payload_bytes
            .checked_add(aux_bytes)
            .and_then(|bytes| bytes.checked_add(1024))
            .ok_or(ItemRefusal::Budget)?
            .max(stored_state_bytes);
        if decode_state_bytes > budget.max_state_bytes.get() {
            return Err(ItemRefusal::Budget);
        }
        metadata.push(CurrentRecordIdMeta {
            id,
            stored_state_bytes,
            decode_state_bytes,
        });
        metadata_state = next_metadata_state;
    }
    drop(rows);
    drop(statement);
    check_operation(sink, deadline, cancelled)?;

    let mut take = metadata.len().min(capacity);
    let mut has_more = metadata.len() > take;
    let mut output_cursor_state = if has_more {
        let id = &metadata[take - 1].id;
        id.len()
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or(ItemRefusal::Budget)?
    } else {
        0
    };
    let page_state = |take: usize,
                      cursor_state: usize,
                      output_slots: usize,
                      metadata_slots: usize|
     -> Result<usize, ItemRefusal> {
        let mut state = std::mem::size_of::<SourceFoundationCurrentRecordsPage>()
            .checked_add(
                output_slots
                    .checked_mul(std::mem::size_of::<(String, BiblioCurrentRecord)>())
                    .ok_or(ItemRefusal::Budget)?,
            )
            .and_then(|bytes| {
                metadata_slots
                    .checked_mul(std::mem::size_of::<CurrentRecordIdMeta>())
                    .and_then(|metadata_bytes| bytes.checked_add(metadata_bytes))
            })
            .and_then(|bytes| bytes.checked_add(input_cursor_state))
            .and_then(|bytes| bytes.checked_add(cursor_state))
            .ok_or(ItemRefusal::Budget)?;
        for row in &metadata {
            state = state
                .checked_add(
                    row.id
                        .len()
                        .checked_mul(16)
                        .and_then(|bytes| bytes.checked_add(2048))
                        .ok_or(ItemRefusal::Budget)?,
                )
                .ok_or(ItemRefusal::Budget)?;
        }
        for row in metadata.iter().take(take) {
            state = state
                .checked_add(row.stored_state_bytes)
                .and_then(|bytes| {
                    row.id
                        .len()
                        .checked_mul(16)
                        .and_then(|id_bytes| id_bytes.checked_add(2048))
                        .and_then(|id_state| bytes.checked_add(id_state))
                })
                .ok_or(ItemRefusal::Budget)?;
        }
        let decode_workspace = metadata
            .iter()
            .take(take)
            .map(|row| row.decode_state_bytes)
            .max()
            .unwrap_or(0);
        state = state
            .checked_add(decode_workspace)
            .ok_or(ItemRefusal::Budget)?;
        Ok(state)
    };
    while take > 0
        && page_state(take, output_cursor_state, capacity, metadata_slot_capacity)?
            > budget.max_state_bytes.get()
    {
        take -= 1;
        has_more = metadata.len() > take;
        output_cursor_state = if has_more && take > 0 {
            metadata[take - 1]
                .id
                .len()
                .checked_mul(16)
                .and_then(|bytes| bytes.checked_add(2048))
                .ok_or(ItemRefusal::Budget)?
        } else {
            0
        };
    }
    if take == 0 && !metadata.is_empty() {
        return Err(ItemRefusal::Budget);
    }
    has_more = metadata.len() > take;
    output_cursor_state = if has_more {
        metadata[take - 1]
            .id
            .len()
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or(ItemRefusal::Budget)?
    } else {
        0
    };
    if has_more && metadata[take - 1].id.len() > budget.max_cursor_bytes.get() {
        return Err(ItemRefusal::Budget);
    }
    let precharged_state_bytes =
        page_state(take, output_cursor_state, capacity, metadata_slot_capacity)?;
    if precharged_state_bytes > budget.max_state_bytes.get() {
        return Err(ItemRefusal::Budget);
    }
    sink.candidate
        .check_state(precharged_state_bytes)
        .map_err(refusal)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| ItemRefusal::Budget)?;
    let output_slot_capacity = output.capacity();
    if output_slot_capacity < capacity {
        return Err(ItemRefusal::Budget);
    }
    let charged_state_bytes = page_state(
        take,
        output_cursor_state,
        output_slot_capacity,
        metadata_slot_capacity,
    )?;
    if charged_state_bytes > budget.max_state_bytes.get() {
        return Err(ItemRefusal::Budget);
    }
    sink.candidate
        .check_state(charged_state_bytes)
        .map_err(refusal)?;
    for row in metadata.iter().take(take) {
        check_operation(sink, deadline, cancelled)?;
        let (record, stored_state_bytes, _) = decode_current_record_checked(
            sink,
            &row.id,
            budget
                .max_state_bytes
                .get()
                .checked_sub(input_cursor_state)
                .ok_or(ItemRefusal::Budget)?,
            0,
            deadline,
            cancelled,
        )?
        .ok_or_else(|| ItemRefusal::Source("current Record page row disappeared".into()))?;
        if stored_state_bytes != row.stored_state_bytes {
            return Err(ItemRefusal::Source(
                "current Record page state changed during read".into(),
            ));
        }
        output.push((row.id.clone(), record));
    }
    let next_after_id = if has_more {
        Some(metadata[take - 1].id.clone())
    } else {
        None
    };
    check_operation(sink, deadline, cancelled)?;
    Ok(SourceFoundationCurrentRecordsPage {
        rows: output,
        next_after_id,
        charged_state_bytes,
    })
}

fn decode_current_record_checked(
    sink: &IndexSink<'_>,
    id: &str,
    max_state: usize,
    base_state: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Option<(BiblioCurrentRecord, usize, usize)>, ItemRefusal> {
    check_operation(sink, deadline, cancelled)?;
    let meta = table_preflight(
        &sink.db,
        collection_id(SourceFoundationRecordsCollection::CurrentRecords),
        id,
        "",
        max_state,
        sink.candidate,
    )
    .map_err(refusal)?;
    check_operation(sink, deadline, cancelled)?;
    let Some(meta) = meta else {
        return Ok(None);
    };
    let decode_state_bytes = meta
        .0
        .checked_add(meta.1)
        .and_then(|bytes| bytes.checked_add(1024))
        .ok_or(ItemRefusal::Budget)?
        .max(meta.2);
    if decode_state_bytes > max_state {
        return Err(ItemRefusal::Budget);
    }
    sink.candidate
        .check_state(
            base_state
                .checked_add(decode_state_bytes)
                .ok_or(ItemRefusal::Budget)?,
        )
        .map_err(refusal)?;
    let (payload, aux) = table_read_payload(
        &sink.db,
        collection_id(SourceFoundationRecordsCollection::CurrentRecords),
        id,
        "",
        meta,
    )
    .map_err(refusal)?;
    check_operation(sink, deadline, cancelled)?;
    if !aux.is_empty() {
        return Err(ItemRefusal::Source(
            "current Record row auxiliary data changed".into(),
        ));
    }
    let value = decode_value(&payload, max_state).map_err(refusal)?;
    if text(&value, "record_id").map_err(refusal)? != id {
        return Err(ItemRefusal::Source(
            "current Record row ID differs from its index key".into(),
        ));
    }
    let record =
        biblio_record_from_value(field(&value, "record").map_err(refusal)?).map_err(refusal)?;
    check_operation(sink, deadline, cancelled)?;
    Ok(Some((record, meta.2, decode_state_bytes)))
}

fn fact_page(
    sink: &IndexSink<'_>,
    collection: SourceFoundationRecordFactCollection,
    after: Option<&SourceFoundationRecordsCursor>,
    budget: SourceFoundationRecordsPageBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<SourceFoundationRecordFactPage, ItemRefusal> {
    if after.is_some_and(|cursor| cursor.as_bytes().len() > budget.max_cursor_bytes.get()) {
        return Err(ItemRefusal::BudgetCheck {
            check: "source-foundation Biblio cursor input bytes",
            used: after
                .map(|c| Some(c.as_bytes().len() as u64))
                .unwrap_or(None),
            limit: Some(budget.max_cursor_bytes.get() as u64),
        });
    }
    check_operation(sink, deadline, cancelled)?;
    let id = cursor_tag_facts(collection);
    let mode = if sorted_fact_collection(collection) {
        4
    } else {
        2
    };
    let parsed = parse_cursor(after, id, mode, budget.max_cursor_bytes.get()).map_err(refusal)?;
    let input_bytes = after.map_or(0, |cursor| cursor.as_bytes().len());
    let capacity = page_row_capacity(budget, input_bytes, true)?;
    let meta_capacity = capacity.checked_add(1).ok_or(ItemRefusal::Budget)?;
    let meta = fact_rows_meta(
        sink,
        collection,
        parsed.as_ref(),
        meta_capacity,
        budget,
        deadline,
        cancelled,
    )?;
    let mut take = meta.len().min(capacity);
    let mut more = meta.len() > take;
    let mut cursor_len = if more {
        output_cursor_len(
            meta.get(take.saturating_sub(1))
                .ok_or(ItemRefusal::Budget)?,
            sorted_fact_collection(collection),
        )
        .map_err(refusal)?
    } else {
        0
    };
    if cursor_len > budget.max_cursor_bytes.get() {
        return Err(ItemRefusal::Budget);
    }
    while take > 0
        && fact_page_cost(
            &meta[..take],
            capacity,
            meta_capacity,
            input_bytes,
            cursor_len,
        )
        .map_err(refusal)?
            > budget.max_state_bytes.get()
    {
        take -= 1;
        more = meta.len() > take;
        if take == 0 {
            return Err(ItemRefusal::Budget);
        }
        cursor_len = if more {
            output_cursor_len(&meta[take - 1], sorted_fact_collection(collection))
                .map_err(refusal)?
        } else {
            0
        };
        if cursor_len > budget.max_cursor_bytes.get() {
            return Err(ItemRefusal::Budget);
        }
    }
    if more && take == 0 {
        return Err(ItemRefusal::Budget);
    }
    let charged = fact_page_cost(
        &meta[..take],
        capacity,
        meta_capacity,
        input_bytes,
        cursor_len,
    )
    .map_err(refusal)?;
    if charged > budget.max_state_bytes.get() {
        return Err(ItemRefusal::Budget);
    }
    sink.candidate.check_state(charged).map_err(refusal)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| ItemRefusal::Budget)?;
    for row in &meta[..take] {
        let (key, payload) = read_fact_for_page(
            sink,
            collection,
            row.seq,
            *row,
            budget.max_state_bytes.get(),
            deadline,
            cancelled,
        )?;
        let value = decode_value(&payload, budget.max_state_bytes.get()).map_err(refusal)?;
        let fact = fact_from_value(&value, collection).map_err(refusal)?;
        output.push(fact);
    }
    let next_cursor = if more {
        let last = &meta[take - 1];
        let key = if sorted_fact_collection(collection) {
            fact_cursor_key(collection, output.last().ok_or(ItemRefusal::Budget)?)
                .map_err(refusal)?
        } else {
            ""
        };
        Some(
            make_cursor(id, mode, last.seq, key, "", budget.max_cursor_bytes.get())
                .map_err(refusal)?,
        )
    } else {
        None
    };
    check_operation(sink, deadline, cancelled)?;
    if next_cursor
        .as_ref()
        .is_some_and(|cursor| cursor.as_bytes().len() > budget.max_cursor_bytes.get())
    {
        return Err(ItemRefusal::Budget);
    }
    Ok(SourceFoundationRecordFactPage {
        rows: output,
        next_cursor,
        charged_state_bytes: charged,
    })
}

impl SourceFoundationRecordsStore for IndexSink<'_> {
    fn current_record_first(
        &mut self,
        id: &str,
        record: &BiblioCurrentRecord,
    ) -> Result<(), ItemRefusal> {
        let input = string_fields(&[id, &record.path, &record.kind])
            .and_then(|n| checked_add(n, value_state(&record.value)?))
            .map_err(refusal)?;
        preflight_row_clone(self, input).map_err(refusal)?;
        self.candidate.tick().map_err(refusal)?;
        let already_retained = self
            .db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sf_rows WHERE collection=?1 AND key1=?2 COLLATE BINARY AND key2='')",
                params![collection_id(SourceFoundationRecordsCollection::CurrentRecords), id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(refusal)?;
        self.candidate.tick().map_err(refusal)?;
        let value =
            json!({"v":STORE_CODEC_VERSION,"record_id":id,"record":biblio_record_value(record)});
        self.append_stored_row(
            SourceFoundationRecordsCollection::CurrentRecords,
            id,
            "",
            &value,
            &[],
            input,
            WriteLaw::First,
            None,
            None,
        )
        .map_err(refusal)?;
        drop(value);
        if !already_retained {
            preflight_row_clone(self, string_fields(&[id, &record.path]).map_err(refusal)?)
                .map_err(refusal)?;
            self.candidate.tick().map_err(refusal)?;
            self.db
                .execute(
                    "INSERT INTO sf_current_paths(path,record_id) VALUES(?1,?2) ON CONFLICT(path) DO UPDATE SET record_id=excluded.record_id WHERE excluded.record_id COLLATE BINARY < sf_current_paths.record_id COLLATE BINARY",
                    params![record.path, id],
                )
                .map_err(refusal)?;
            self.candidate.tick().map_err(refusal)?;
        }
        Ok(())
    }

    fn used_profile_kind(&mut self, kind: &str) -> Result<(), ItemRefusal> {
        preflight_row_clone(self, string_fields(&[kind]).map_err(refusal)?).map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"value":kind});
        self.append_stored_row(
            SourceFoundationRecordsCollection::UsedDeclaredProfileKinds,
            kind,
            "",
            &value,
            &[],
            string_fields(&[kind]).map_err(refusal)?,
            WriteLaw::First,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn item_record_selection(
        &mut self,
        row: &SourceFoundationItemRecordSelection,
    ) -> Result<(), ItemRefusal> {
        let state = string_fields(&[&row.record_id, &row.path])
            .and_then(|n| checked_add(n, value_state(&row.value)?))
            .map_err(refusal)?;
        preflight_row_clone(self, state).map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"selection":item_selection_value(row)});
        self.append_stored_row(
            SourceFoundationRecordsCollection::ItemRecordSelections,
            &row.record_id,
            "",
            &value,
            &[],
            state,
            WriteLaw::Replace,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn item_edition(&mut self, item_id: &str, embodiment_ref: &str) -> Result<(), ItemRefusal> {
        preflight_row_clone(
            self,
            string_fields(&[item_id, embodiment_ref]).map_err(refusal)?,
        )
        .map_err(refusal)?;
        let value =
            json!({"v":STORE_CODEC_VERSION,"item_id":item_id,"embodiment_ref":embodiment_ref});
        self.append_stored_row(
            SourceFoundationRecordsCollection::ItemEditions,
            item_id,
            "",
            &value,
            &[],
            string_fields(&[item_id, embodiment_ref]).map_err(refusal)?,
            WriteLaw::Replace,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn rights_id(&mut self, id: &str) -> Result<(), ItemRefusal> {
        preflight_row_clone(self, string_fields(&[id]).map_err(refusal)?).map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"value":id});
        self.append_stored_row(
            SourceFoundationRecordsCollection::RightsIds,
            id,
            "",
            &value,
            &[],
            string_fields(&[id]).map_err(refusal)?,
            WriteLaw::First,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn source_event_insertion(
        &mut self,
        row: &SourceFoundationEventInsertion,
    ) -> Result<(), ItemRefusal> {
        let state = string_fields(&[&row.0])
            .and_then(|n| checked_add(n, value_state(&row.1)?))
            .map_err(refusal)?;
        preflight_row_clone(self, state).map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"event_id":row.0,"event":row.1});
        self.append_stored_row(
            SourceFoundationRecordsCollection::SourceEventInsertions,
            &row.0,
            "",
            &value,
            &[],
            state,
            WriteLaw::Append,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn file_descriptor_first(
        &mut self,
        file_id: &str,
        sha256: &Value,
        byte_size: &Value,
        media_type: &Value,
    ) -> Result<(), ItemRefusal> {
        let state = [sha256, byte_size, media_type]
            .into_iter()
            .try_fold(string_fields(&[file_id]).map_err(refusal)?, |n, v| {
                checked_add(n, value_state(v).map_err(refusal)?).map_err(refusal)
            })?;
        preflight_row_clone(self, state).map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"file_id":file_id,"sha256":sha256,"byte_size":byte_size,"media_type":media_type});
        self.append_stored_row(
            SourceFoundationRecordsCollection::FileDescriptors,
            file_id,
            "",
            &value,
            &[],
            state,
            WriteLaw::First,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn item_file_membership(&mut self, file_id: &str, item_id: &str) -> Result<(), ItemRefusal> {
        preflight_row_clone(self, string_fields(&[file_id, item_id]).map_err(refusal)?)
            .map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"file_id":file_id,"item_id":item_id});
        self.append_stored_row(
            SourceFoundationRecordsCollection::ItemFileMemberships,
            file_id,
            item_id,
            &value,
            &[],
            string_fields(&[file_id, item_id]).map_err(refusal)?,
            WriteLaw::First,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn record_schema_position(
        &mut self,
        path: &str,
        before_issue: usize,
    ) -> Result<(), ItemRefusal> {
        preflight_row_clone(self, string_fields(&[path]).map_err(refusal)?).map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"path":path,"before_issue":before_issue});
        self.append_stored_row(
            SourceFoundationRecordsCollection::RecordSchemaPositions,
            path,
            "",
            &value,
            &[],
            string_fields(&[path]).map_err(refusal)?,
            WriteLaw::Append,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn ordered_issue(&mut self, row: &SourceFoundationRecordsIssue) -> Result<(), ItemRefusal> {
        preflight_row_clone(
            self,
            string_fields(&[&row.location, &row.message]).map_err(refusal)?,
        )
        .map_err(refusal)?;
        let value = issue_value(row);
        self.append_stored_row(
            SourceFoundationRecordsCollection::OrderedIssues,
            &row.location,
            "",
            &value,
            &[],
            string_fields(&[&row.location, &row.message]).map_err(refusal)?,
            WriteLaw::Append,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn schema_check(
        &mut self,
        row: &SourceFoundationRecordsSchemaCheck,
    ) -> Result<(), ItemRefusal> {
        let mut state = string_fields(&[&row.location, &row.contract]).map_err(refusal)?;
        if let Some(decoded) = &row.decoded_instance {
            state = checked_add(state, value_state(decoded).map_err(refusal)?).map_err(refusal)?;
        }
        state = checked_add(state, row.legacy_raw_instance.as_ref().map_or(0, Vec::len))
            .map_err(refusal)?;
        preflight_row_clone(self, state).map_err(refusal)?;
        let (value, raw) = schema_check_value(row);
        self.append_stored_row(
            SourceFoundationRecordsCollection::SchemaChecks,
            &row.location,
            "",
            &value,
            &raw,
            state,
            WriteLaw::Append,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn item_issue(&mut self, row: &ItemIssue) -> Result<(), ItemRefusal> {
        preflight_row_clone(
            self,
            string_fields(&[&row.path, &row.code]).map_err(refusal)?,
        )
        .map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"path":row.path,"code":row.code});
        self.append_stored_row(
            SourceFoundationRecordsCollection::ItemIssues,
            &row.path,
            "",
            &value,
            &[],
            string_fields(&[&row.path, &row.code]).map_err(refusal)?,
            WriteLaw::Append,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn manifest_item_id(&mut self, id: &str) -> Result<(), ItemRefusal> {
        preflight_row_clone(self, string_fields(&[id]).map_err(refusal)?).map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"value":id});
        self.append_stored_row(
            SourceFoundationRecordsCollection::ManifestItemIds,
            id,
            "",
            &value,
            &[],
            string_fields(&[id]).map_err(refusal)?,
            WriteLaw::Append,
            None,
            None,
        )
        .map_err(refusal)?;
        Ok(())
    }

    fn record_observation(
        &mut self,
        ordinal: u64,
        row: &RecordObservation,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        check_operation(self, deadline, cancelled)?;
        if ordinal != self.record_observation_count {
            return Err(ItemRefusal::Source(
                "Biblio observation ordinal is not contiguous".into(),
            ));
        }
        let state = observation_state(row).map_err(refusal)?;
        preflight_row_clone(self, state).map_err(refusal)?;
        let value =
            json!({"v":STORE_CODEC_VERSION,"ordinal":ordinal,"observation":observation_value(row)});
        let stored_seq = self
            .append_stored_row(
                SourceFoundationRecordsCollection::RecordObservations,
                "",
                "",
                &value,
                &[],
                state,
                WriteLaw::Append,
                Some(deadline),
                Some(cancelled),
            )
            .map_err(refusal)?;
        if u64::try_from(stored_seq).ok() != Some(ordinal) {
            return Err(ItemRefusal::Source(
                "Biblio observation storage order changed".into(),
            ));
        }
        drop(value);
        preflight_row_clone(self, state).map_err(refusal)?;
        let fact = |tag: &str, fields: Value| json!({"v":FACT_CODEC_VERSION,"tag":tag,"ordinal":ordinal,"fields":fields});
        match row {
            RecordObservation::IdOwner { id, kind, path, .. } => {
                let payload = fact(
                    "global_id",
                    json!({"id":id,"kind":kind,"path":path,"carrier":"standalone"}),
                );
                self.append_fact(
                    SourceFoundationRecordFactCollection::GlobalIdFacts,
                    ordinal,
                    id,
                    &payload,
                    state,
                    deadline,
                    cancelled,
                )?;
            }
            RecordObservation::NativeReservation {
                id, packet_path, ..
            } => {
                let kind = id.split('.').nth(1).unwrap_or("");
                let payload = fact(
                    "global_id",
                    json!({"id":id,"kind":kind,"path":packet_path,"carrier":"native_packet"}),
                );
                self.append_fact(
                    SourceFoundationRecordFactCollection::GlobalIdFacts,
                    ordinal,
                    id,
                    &payload,
                    state,
                    deadline,
                    cancelled,
                )?;
            }
            RecordObservation::LinkUriOwner { uri, id, path } => {
                let payload = fact("link_uri", json!({"uri":uri,"id":id,"path":path}));
                self.append_fact(
                    SourceFoundationRecordFactCollection::LinkUriFacts,
                    ordinal,
                    uri,
                    &payload,
                    state,
                    deadline,
                    cancelled,
                )?;
            }
            RecordObservation::RecordIdReference {
                from_path,
                target_id,
                expected_kind,
            } => {
                let payload = fact(
                    "typed_id_ref",
                    json!({"target_id":target_id,"expected_kind":expected_kind,"from_path":from_path}),
                );
                self.append_fact(
                    SourceFoundationRecordFactCollection::TypedIdRefFacts,
                    ordinal,
                    target_id,
                    &payload,
                    state,
                    deadline,
                    cancelled,
                )?;
            }
            RecordObservation::Reference {
                from_path,
                target_path,
                check,
            } => {
                let payload = fact(
                    "path_reference",
                    json!({"from_path":from_path,"target_path":target_path,"check":match check { PathReferenceCheck::RepoExistsIfToS=>"repo_exists_if_tos",PathReferenceCheck::FileIfToS=>"file_if_tos" }}),
                );
                self.append_fact(
                    SourceFoundationRecordFactCollection::PathReferenceFacts,
                    ordinal,
                    "",
                    &payload,
                    state,
                    deadline,
                    cancelled,
                )?;
            }
            _ => {}
        }
        self.append_fact(
            SourceFoundationRecordFactCollection::Observations,
            ordinal,
            "",
            &fact("observation", json!({"observation":observation_value(row)})),
            state,
            deadline,
            cancelled,
        )?;
        self.record_observation_count = self
            .record_observation_count
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn record_schema_diagnostic(
        &mut self,
        ordinal: u64,
        row: &SourceCutSchemaDiagnostic,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal> {
        check_operation(self, deadline, cancelled)?;
        if ordinal != self.record_schema_diagnostic_count {
            return Err(ItemRefusal::Source(
                "Biblio schema diagnostic ordinal is not contiguous".into(),
            ));
        }
        let state = row
            .accounted_state_bytes
            .max(std::mem::size_of::<SourceCutSchemaDiagnostic>());
        preflight_row_clone(self, state).map_err(refusal)?;
        let value =
            json!({"v":STORE_CODEC_VERSION,"ordinal":ordinal,"diagnostic":diagnostic_value(row)});
        let seq = self
            .append_stored_row(
                SourceFoundationRecordsCollection::RecordSchemaDiagnostics,
                "",
                "",
                &value,
                &[],
                state,
                WriteLaw::Append,
                Some(deadline),
                Some(cancelled),
            )
            .map_err(refusal)?;
        if u64::try_from(seq).ok() != Some(ordinal) {
            return Err(ItemRefusal::Source(
                "Biblio diagnostic storage order changed".into(),
            ));
        }
        self.record_schema_diagnostic_count = self
            .record_schema_diagnostic_count
            .checked_add(1)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }

    fn record_fact_page(
        &self,
        collection: SourceFoundationRecordFactCollection,
        after: Option<&SourceFoundationRecordsCursor>,
        budget: SourceFoundationRecordsPageBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationRecordFactPage, ItemRefusal> {
        fact_page(self, collection, after, budget, deadline, cancelled)
    }

    fn lookup_current_record(
        &self,
        id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationRecordsLookup>, ItemRefusal> {
        check_operation(self, deadline, cancelled)?;
        let found =
            decode_current_record_checked(self, id, max_state_bytes.get(), 0, deadline, cancelled)?;
        check_operation(self, deadline, cancelled)?;
        Ok(found.map(
            |(record, _, charged_state_bytes)| SourceFoundationRecordsLookup {
                record,
                charged_state_bytes,
            },
        ))
    }

    fn lookup_current_record_by_path(
        &self,
        path: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationCurrentRecordPathLookup>, ItemRefusal> {
        let max_state = max_state_bytes.get();
        let path_state = string_fields(&[path]).map_err(refusal)?;
        if path_state >= max_state {
            return Err(ItemRefusal::Budget);
        }
        check_operation(self, deadline, cancelled)?;
        self.candidate.check_state(path_state).map_err(refusal)?;
        let id_allowance = max_state
            .checked_sub(path_state)
            .ok_or(ItemRefusal::Budget)?;
        let id_len = self
            .db
            .query_row(
                "SELECT length(CAST(record_id AS BLOB)) FROM sf_current_paths WHERE path=?1 COLLATE BINARY",
                [path],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        let Some(id_len) = id_len else {
            return Ok(None);
        };
        let id_len = usize::try_from(id_len).map_err(|_| ItemRefusal::Budget)?;
        let id_state = id_len
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(2048))
            .ok_or(ItemRefusal::Budget)?;
        let lookup_state = path_state
            .checked_add(id_state)
            .ok_or(ItemRefusal::Budget)?;
        if lookup_state >= max_state {
            return Err(ItemRefusal::Budget);
        }
        self.candidate.check_state(lookup_state).map_err(refusal)?;
        let id = self
            .db
            .query_row(
                "SELECT record_id FROM sf_current_paths WHERE path=?1 COLLATE BINARY",
                [path],
                |row| bounded_text(row, 0, id_allowance),
            )
            .optional()
            .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        let Some(id) = id else {
            return Err(ItemRefusal::Source(
                "current path index row disappeared during lookup".into(),
            ));
        };
        if id.len() != id_len {
            return Err(ItemRefusal::Source(
                "current path index target changed during lookup".into(),
            ));
        }
        let record_allowance = max_state
            .checked_sub(lookup_state)
            .ok_or(ItemRefusal::Budget)?;
        let (record, _, decode_state_bytes) = decode_current_record_checked(
            self,
            &id,
            record_allowance,
            lookup_state,
            deadline,
            cancelled,
        )?
        .ok_or_else(|| ItemRefusal::Source("current path index target is missing".into()))?;
        if record.path != path {
            return Err(ItemRefusal::Source(
                "current path index target differs from the retained Record".into(),
            ));
        }
        let charged_state_bytes = lookup_state
            .checked_add(decode_state_bytes)
            .ok_or(ItemRefusal::Budget)?;
        if charged_state_bytes > max_state {
            return Err(ItemRefusal::Budget);
        }
        self.candidate
            .check_state(charged_state_bytes)
            .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        Ok(Some(SourceFoundationCurrentRecordPathLookup {
            record,
            charged_state_bytes,
        }))
    }

    fn lookup_item_edition(
        &self,
        item_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationItemEditionLookup>, ItemRefusal> {
        let max_state = max_state_bytes.get();
        let key_state = string_fields(&[item_id]).map_err(refusal)?;
        if key_state >= max_state {
            return Err(ItemRefusal::Budget);
        }
        check_operation(self, deadline, cancelled)?;
        self.candidate.check_state(key_state).map_err(refusal)?;
        let row_allowance = max_state
            .checked_sub(key_state)
            .ok_or(ItemRefusal::Budget)?;
        let meta = table_preflight(
            &self.db,
            collection_id(SourceFoundationRecordsCollection::ItemEditions),
            item_id,
            "",
            row_allowance,
            self.candidate,
        )
        .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        let Some(meta) = meta else {
            return Ok(None);
        };
        self.candidate
            .check_state(key_state.checked_add(meta.2).ok_or(ItemRefusal::Budget)?)
            .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        let (payload, aux) = table_read_payload(
            &self.db,
            collection_id(SourceFoundationRecordsCollection::ItemEditions),
            item_id,
            "",
            meta,
        )
        .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        if !aux.is_empty() {
            return Err(ItemRefusal::Source(
                "Item edition row auxiliary data changed".into(),
            ));
        }
        let value = decode_value(&payload, row_allowance).map_err(refusal)?;
        if text(&value, "item_id").map_err(refusal)? != item_id {
            return Err(ItemRefusal::Source(
                "Item edition row key differs from its index key".into(),
            ));
        }
        let embodiment_ref = owned_text(&value, "embodiment_ref").map_err(refusal)?;
        let charged_state_bytes = key_state.checked_add(meta.2).ok_or(ItemRefusal::Budget)?;
        if charged_state_bytes > max_state {
            return Err(ItemRefusal::Budget);
        }
        self.candidate
            .check_state(charged_state_bytes)
            .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        Ok(Some(SourceFoundationItemEditionLookup {
            embodiment_ref,
            charged_state_bytes,
        }))
    }

    fn current_records_by_id_page(
        &self,
        after_id: Option<&str>,
        budget: SourceFoundationRecordsPageBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationCurrentRecordsPage, ItemRefusal> {
        current_records_by_id_page(self, after_id, budget, deadline, cancelled)
    }

    fn link_uri_owner_first(
        &mut self,
        uri: &str,
        record_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationUriOwnerLookup>, ItemRefusal> {
        check_operation(self, deadline, cancelled)?;
        let collection = collection_id(SourceFoundationRecordsCollection::LinkUriOwners);
        let prior = table_preflight(
            &self.db,
            collection,
            uri,
            "",
            max_state_bytes.get(),
            self.candidate,
        )
        .map_err(refusal)?;
        if let Some(meta) = prior {
            let (payload, _) =
                table_read_payload(&self.db, collection, uri, "", meta).map_err(refusal)?;
            let value = decode_value(&payload, max_state_bytes.get()).map_err(refusal)?;
            check_operation(self, deadline, cancelled)?;
            return Ok(Some(SourceFoundationUriOwnerLookup {
                record_id: owned_text(&value, "record_id").map_err(refusal)?,
                charged_state_bytes: meta.2,
            }));
        }
        preflight_row_clone(self, string_fields(&[uri, record_id]).map_err(refusal)?)
            .map_err(refusal)?;
        let value = json!({"v":STORE_CODEC_VERSION,"uri":uri,"record_id":record_id});
        self.append_stored_row(
            SourceFoundationRecordsCollection::LinkUriOwners,
            uri,
            "",
            &value,
            &[],
            string_fields(&[uri, record_id]).map_err(refusal)?,
            WriteLaw::First,
            Some(deadline),
            Some(cancelled),
        )
        .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        Ok(None)
    }

    fn lookup_item_record_selection(
        &self,
        item_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationItemSelectionLookup>, ItemRefusal> {
        check_operation(self, deadline, cancelled)?;
        let collection = collection_id(SourceFoundationRecordsCollection::ItemRecordSelections);
        let Some(meta) = table_preflight(
            &self.db,
            collection,
            item_id,
            "",
            max_state_bytes.get(),
            self.candidate,
        )
        .map_err(refusal)?
        else {
            return Ok(None);
        };
        let (payload, aux) =
            table_read_payload(&self.db, collection, item_id, "", meta).map_err(refusal)?;
        let value = decode_value(&payload, max_state_bytes.get()).map_err(refusal)?;
        let selection = item_selection_from_value(field(&value, "selection").map_err(refusal)?)
            .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        Ok(Some(SourceFoundationItemSelectionLookup {
            selection,
            charged_state_bytes: meta.2,
        }))
    }

    fn lookup_file_descriptor(
        &self,
        file_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationFileDescriptorLookup>, ItemRefusal> {
        check_operation(self, deadline, cancelled)?;
        let collection = collection_id(SourceFoundationRecordsCollection::FileDescriptors);
        let Some(meta) = table_preflight(
            &self.db,
            collection,
            file_id,
            "",
            max_state_bytes.get(),
            self.candidate,
        )
        .map_err(refusal)?
        else {
            return Ok(None);
        };
        let (payload, _) =
            table_read_payload(&self.db, collection, file_id, "", meta).map_err(refusal)?;
        let value = decode_value(&payload, max_state_bytes.get()).map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        Ok(Some(SourceFoundationFileDescriptorLookup {
            file_id: owned_text(&value, "file_id").map_err(refusal)?,
            sha256: field(&value, "sha256").map_err(refusal)?.clone(),
            byte_size: field(&value, "byte_size").map_err(refusal)?.clone(),
            media_type: field(&value, "media_type").map_err(refusal)?.clone(),
            charged_state_bytes: meta.2,
        }))
    }

    fn contains_item_file_membership(
        &self,
        file_id: &str,
        item_id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        check_operation(self, deadline, cancelled)?;
        let contains=self.db.query_row("SELECT EXISTS(SELECT 1 FROM sf_rows WHERE collection=?1 AND key1=?2 COLLATE BINARY AND key2=?3 COLLATE BINARY)",params![collection_id(SourceFoundationRecordsCollection::ItemFileMemberships),file_id,item_id],|row|row.get::<_,bool>(0)).map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        Ok(contains)
    }

    fn contains_rights_id(
        &self,
        id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        check_operation(self, deadline, cancelled)?;
        let contains = self
            .db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sf_rows WHERE collection=?1 AND key1=?2 COLLATE BINARY AND key2='')",
                params![collection_id(SourceFoundationRecordsCollection::RightsIds), id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(refusal)?;
        check_operation(self, deadline, cancelled)?;
        Ok(contains)
    }

    fn page(
        &self,
        collection: SourceFoundationRecordsCollection,
        after: Option<&SourceFoundationRecordsCursor>,
        budget: SourceFoundationRecordsPageBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationRecordsCursorPage, ItemRefusal> {
        report_page(self, collection, after, budget, deadline, cancelled)
    }
}

fn reason_code(reason: schema_diag::Reason) -> &'static str {
    reason.code()
}

fn parse_reason(code: &str) -> io::Result<schema_diag::Reason> {
    use schema_diag::Reason as R;
    Ok(match code {
        "additional_items" => R::AdditionalItems,
        "additional_properties" => R::AdditionalProperties,
        "any_of" => R::AnyOf,
        "pattern" => R::Pattern,
        "constant" => R::Constant,
        "contains" => R::Contains,
        "content_encoding" => R::ContentEncoding,
        "content_media_type" => R::ContentMediaType,
        "custom" => R::CustomKeyword,
        "enum" => R::Enum,
        "exclusive_maximum" => R::ExclusiveMaximum,
        "exclusive_minimum" => R::ExclusiveMinimum,
        "false_schema" => R::FalseSchema,
        "format" => R::Format,
        "maximum_items" => R::MaximumItems,
        "maximum" => R::Maximum,
        "maximum_length" => R::MaximumLength,
        "maximum_properties" => R::MaximumProperties,
        "minimum_items" => R::MinimumItems,
        "minimum" => R::Minimum,
        "minimum_length" => R::MinimumLength,
        "minimum_properties" => R::MinimumProperties,
        "multiple_of" => R::MultipleOf,
        "not" => R::Not,
        "one_of" => R::OneOf,
        "property_names" => R::PropertyNames,
        "required" => R::Required,
        "type" => R::Type,
        "unevaluated_items" => R::UnevaluatedItems,
        "unevaluated_properties" => R::UnevaluatedProperties,
        "unique_items" => R::UniqueItems,
        "backtrack_limit" => R::BacktrackLimit,
        "regex_engine_failure" => R::RegexEngineFailure,
        "reference_failure" => R::ReferenceFailure,
        "unknown_validation_failure" => R::UnknownValidationFailure,
        _ => return Err(invalid("schema diagnostic reason invalid")),
    })
}

fn path_segment_value(segment: &schema_diag::PathSegment) -> Value {
    match segment {
        schema_diag::PathSegment::Property(value) => json!(["property", value]),
        schema_diag::PathSegment::Index(value) => json!(["index", value]),
    }
}

fn path_segment_from_value(value: &Value) -> io::Result<schema_diag::PathSegment> {
    let pair = value
        .as_array()
        .filter(|row| row.len() == 2)
        .ok_or_else(|| invalid("schema diagnostic path segment invalid"))?;
    match pair[0]
        .as_str()
        .ok_or_else(|| invalid("schema diagnostic path segment kind invalid"))?
    {
        "property" => Ok(schema_diag::PathSegment::Property(
            pair[1]
                .as_str()
                .ok_or_else(|| invalid("schema diagnostic path property invalid"))?
                .to_owned(),
        )),
        "index" => Ok(schema_diag::PathSegment::Index(
            pair[1]
                .as_u64()
                .ok_or_else(|| invalid("schema diagnostic path index invalid"))?,
        )),
        _ => Err(invalid("schema diagnostic path segment kind invalid")),
    }
}

fn report_value(report: &schema_diag::Report) -> Value {
    json!({
        "v":1,
        "protocol_version":report.protocol_version,
        "worker_sha256":report.worker_sha256.to_hex(),
        "request_sha256":report.request_sha256.to_hex(),
        "unit_sha256":report.unit_sha256.to_hex(),
        "schema_set_sha256":report.schema_set_sha256.to_hex(),
        "caps":{
            "max_issues_per_unit":report.caps.max_issues_per_unit,
            "max_report_bytes_per_unit":report.caps.max_report_bytes_per_unit,
            "max_path_segments":report.caps.max_path_segments,
            "max_path_bytes":report.caps.max_path_bytes
        },
        "status":report.status as u8,
        "failure":report.failure as u8,
        "total_issue_count":report.total_issue_count,
        "truncated":report.truncated,
        "issues_sha256":report.issues_sha256.to_hex(),
        "report_sha256":report.report_sha256.to_hex(),
        "issues":report.issues.iter().map(|issue| json!({
            "instance_path":issue.instance_path.iter().map(path_segment_value).collect::<Vec<_>>(),
            "schema_keyword":issue.schema_keyword,
            "reason":reason_code(issue.reason),
            "schema_path":issue.schema_path.iter().map(path_segment_value).collect::<Vec<_>>(),
            "compatibility_text":match issue.compatibility_text {
                None => Value::Null,
                Some(schema_diag::CompatibilityText::NullIsNotObject) => json!("null_is_not_object"),
                Some(schema_diag::CompatibilityText::NullIsNotArray) => json!("null_is_not_array"),
            }
        })).collect::<Vec<_>>()
    })
}

fn status_from_u64(value: u64) -> io::Result<schema_diag::Status> {
    use schema_diag::Status as S;
    match value {
        0 => Ok(S::Valid),
        1 => Ok(S::Invalid),
        2 => Ok(S::Truncated),
        3 => Ok(S::InputRejected),
        4 => Ok(S::Indeterminate),
        _ => Err(invalid("schema diagnostic status invalid")),
    }
}

fn failure_from_u64(value: u64) -> io::Result<schema_diag::Failure> {
    use schema_diag::Failure as F;
    match value {
        0 => Ok(F::None),
        1 => Ok(F::InvalidJson),
        2 => Ok(F::InputBudget),
        3 => Ok(F::ValidatorRuntime),
        4 => Ok(F::UnsupportedInputSemantics),
        _ => Err(invalid("schema diagnostic failure invalid")),
    }
}

fn report_from_value(value: &Value) -> io::Result<schema_diag::Report> {
    if uint(value, "v")? != 1 {
        return Err(invalid("schema diagnostics report codec version invalid"));
    }
    let caps = field(value, "caps")?;
    let caps = schema_diag::Caps {
        max_issues_per_unit: u32::try_from(uint(caps, "max_issues_per_unit")?)
            .map_err(|_| invalid("schema diagnostic cap overflow"))?,
        max_report_bytes_per_unit: u32::try_from(uint(caps, "max_report_bytes_per_unit")?)
            .map_err(|_| invalid("schema diagnostic cap overflow"))?,
        max_path_segments: u16::try_from(uint(caps, "max_path_segments")?)
            .map_err(|_| invalid("schema diagnostic cap overflow"))?,
        max_path_bytes: u32::try_from(uint(caps, "max_path_bytes")?)
            .map_err(|_| invalid("schema diagnostic cap overflow"))?,
    };
    let issues = field(value, "issues")?
        .as_array()
        .ok_or_else(|| invalid("schema diagnostic issues invalid"))?
        .iter()
        .map(|row| {
            let compatibility_text = match field(row, "compatibility_text")? {
                value if value.is_null() => None,
                value => Some(
                    match value
                        .as_str()
                        .ok_or_else(|| invalid("schema diagnostic compatibility text invalid"))?
                    {
                        "null_is_not_object" => schema_diag::CompatibilityText::NullIsNotObject,
                        "null_is_not_array" => schema_diag::CompatibilityText::NullIsNotArray,
                        _ => return Err(invalid("schema diagnostic compatibility text invalid")),
                    },
                ),
            };
            let path = |name: &str| -> io::Result<Vec<schema_diag::PathSegment>> {
                field(row, name)?
                    .as_array()
                    .ok_or_else(|| invalid("schema diagnostic path invalid"))?
                    .iter()
                    .map(path_segment_from_value)
                    .collect()
            };
            Ok(schema_diag::Issue {
                instance_path: path("instance_path")?,
                schema_keyword: owned_text(row, "schema_keyword")?,
                reason: parse_reason(text(row, "reason")?)?,
                schema_path: path("schema_path")?,
                compatibility_text,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let report = schema_diag::Report {
        protocol_version: u16::try_from(uint(value, "protocol_version")?)
            .map_err(|_| invalid("schema diagnostics protocol overflow"))?,
        worker_sha256: digest(value, "worker_sha256")?,
        request_sha256: digest(value, "request_sha256")?,
        unit_sha256: digest(value, "unit_sha256")?,
        schema_set_sha256: digest(value, "schema_set_sha256")?,
        caps,
        status: status_from_u64(uint(value, "status")?)?,
        failure: failure_from_u64(uint(value, "failure")?)?,
        total_issue_count: uint(value, "total_issue_count")?,
        truncated: bool_value(value, "truncated")?,
        issues_sha256: digest(value, "issues_sha256")?,
        report_sha256: digest(value, "report_sha256")?,
        issues,
    };
    if !report.is_well_formed() {
        return Err(invalid(
            "stored schema diagnostics report is not well formed",
        ));
    }
    Ok(report)
}

fn exceptional_value(value: Option<ExceptionalSchemaUsage>) -> Value {
    value
        .map(|value| {
            json!({
                "schema_scan_work":value.schema_scan_work,
                "schema_scan_bytes":value.schema_scan_bytes,
                "pattern_compile_count":value.pattern_compile_count,
                "pattern_bytes":value.pattern_bytes,
                "evaluation_work":value.evaluation_work,
                "evaluation_bytes":value.evaluation_bytes,
                "reference_steps":value.reference_steps,
                "regex_checks":value.regex_checks,
                "regex_bytes":value.regex_bytes,
            })
        })
        .unwrap_or(Value::Null)
}

fn exceptional_from_value(value: &Value) -> io::Result<Option<ExceptionalSchemaUsage>> {
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(ExceptionalSchemaUsage {
        schema_scan_work: uint(value, "schema_scan_work")?,
        schema_scan_bytes: uint(value, "schema_scan_bytes")?,
        pattern_compile_count: uint(value, "pattern_compile_count")?,
        pattern_bytes: uint(value, "pattern_bytes")?,
        evaluation_work: uint(value, "evaluation_work")?,
        evaluation_bytes: uint(value, "evaluation_bytes")?,
        reference_steps: uint(value, "reference_steps")?,
        regex_checks: uint(value, "regex_checks")?,
        regex_bytes: uint(value, "regex_bytes")?,
    }))
}

fn checkpoint_value(row: &SchemaDiagnosticsCheckpoint) -> Value {
    json!({
        "worker_sha256":row.worker_sha256.to_hex(),
        "request_sha256":row.request_sha256.to_hex(),
        "profile":profile_id(row.profile),
        "schema_set_sha256":row.schema_set_sha256.to_hex(),
        "ordered_manifest_sha256":row.ordered_manifest_sha256.to_hex(),
        "caps_sha256":row.caps_sha256.to_hex(),
        "completed_count":row.completed_count,
        "result_stream_sha256":row.result_stream_sha256.to_hex(),
        "worker_request_bytes":row.worker_request_bytes,
        "worker_response_bytes":row.worker_response_bytes,
        "worker_cpu_micros":row.worker_cpu_micros,
        "exceptional_remaining":exceptional_value(row.exceptional_remaining),
        "exceptional_usage":exceptional_value(row.exceptional_usage),
    })
}

fn checkpoint_from_value(value: &Value) -> io::Result<SchemaDiagnosticsCheckpoint> {
    let optional_u64 = |name: &str| -> io::Result<Option<u64>> {
        let item = field(value, name)?;
        if item.is_null() {
            Ok(None)
        } else {
            item.as_u64()
                .map(Some)
                .ok_or_else(|| invalid("schema diagnostic checkpoint integer invalid"))
        }
    };
    Ok(SchemaDiagnosticsCheckpoint {
        worker_sha256: digest(value, "worker_sha256")?,
        request_sha256: digest(value, "request_sha256")?,
        profile: parse_profile(text(value, "profile")?)?,
        schema_set_sha256: digest(value, "schema_set_sha256")?,
        ordered_manifest_sha256: digest(value, "ordered_manifest_sha256")?,
        caps_sha256: digest(value, "caps_sha256")?,
        completed_count: uint(value, "completed_count")?,
        result_stream_sha256: digest(value, "result_stream_sha256")?,
        worker_request_bytes: uint(value, "worker_request_bytes")?,
        worker_response_bytes: uint(value, "worker_response_bytes")?,
        worker_cpu_micros: optional_u64("worker_cpu_micros")?,
        exceptional_remaining: exceptional_from_value(field(value, "exceptional_remaining")?)?,
        exceptional_usage: exceptional_from_value(field(value, "exceptional_usage")?)?,
    })
}

fn diagnostic_value(row: &SourceCutSchemaDiagnostic) -> Value {
    let unit = &row.unit;
    let verdict = &row.verdict;
    json!({
        "v":1,
        "evaluation_ordinal":row.evaluation_ordinal,
        "path":row.path,
        "before_issue":row.before_issue,
        "verdict":{
            "instance_sha256":verdict.instance_sha256.to_hex(),
            "schema_set_digest":verdict.schema_set_digest.to_hex(),
            "format_profile":profile_id(verdict.format_profile),
            "root_uri":verdict.root_uri,
            "worker_protocol_id":verdict.worker_protocol_id,
            "worker_binary_digest":verdict.worker_binary_digest.to_hex(),
            "valid":verdict.valid
        },
        "checkpoint":checkpoint_value(&row.checkpoint),
        "unit":{
            "ordinal":unit.ordinal,
            "member_id":unit.member_id,
            "relative_path":unit.relative_path,
            "root_uri":unit.root_uri,
            "raw_sha256":unit.raw_sha256.to_hex(),
            "unit_sha256":unit.unit_sha256.to_hex(),
            "report":report_value(&unit.report)
        },
        "schema_resource_bytes":row.schema_resource_bytes,
        "schema_resource_buffer_bytes":row.schema_resource_buffer_bytes,
        "input_instance_bytes":row.input_instance_bytes,
        "input_instance_buffer_bytes":row.input_instance_buffer_bytes,
        "input_metadata_bytes":row.input_metadata_bytes,
        "request_bytes":row.request_bytes,
        "request_buffer_bytes":row.request_buffer_bytes,
        "response_bytes":row.response_bytes,
        "response_buffer_bytes":row.response_buffer_bytes,
        "worker_cpu_micros":row.worker_cpu_micros,
        "retained_state_bytes":row.retained_state_bytes,
        "accounted_state_bytes":row.accounted_state_bytes
    })
}

fn diagnostic_from_value(value: &Value) -> io::Result<SourceCutSchemaDiagnostic> {
    if uint(value, "v")? != 1 {
        return Err(invalid("schema diagnostic codec version invalid"));
    }
    let verdict = field(value, "verdict")?;
    let unit = field(value, "unit")?;
    let report = report_from_value(field(unit, "report")?)?;
    let unit = SchemaDiagnosticUnit {
        ordinal: uint(unit, "ordinal")?,
        member_id: owned_text(unit, "member_id")?,
        relative_path: owned_text(unit, "relative_path")?,
        root_uri: owned_text(unit, "root_uri")?,
        raw_sha256: digest(unit, "raw_sha256")?,
        unit_sha256: digest(unit, "unit_sha256")?,
        report,
    };
    let before_issue = usize_value(value, "before_issue")?;
    Ok(SourceCutSchemaDiagnostic {
        evaluation_ordinal: uint(value, "evaluation_ordinal")?,
        path: owned_text(value, "path")?,
        before_issue,
        verdict: SourceCutSchemaVerdict {
            instance_sha256: digest(verdict, "instance_sha256")?,
            schema_set_digest: digest(verdict, "schema_set_digest")?,
            format_profile: parse_profile(text(verdict, "format_profile")?)?,
            root_uri: owned_text(verdict, "root_uri")?,
            worker_protocol_id: owned_text(verdict, "worker_protocol_id")?,
            worker_binary_digest: digest(verdict, "worker_binary_digest")?,
            valid: bool_value(verdict, "valid")?,
        },
        checkpoint: checkpoint_from_value(field(value, "checkpoint")?)?,
        unit,
        schema_resource_bytes: usize_value(value, "schema_resource_bytes")?,
        schema_resource_buffer_bytes: usize_value(value, "schema_resource_buffer_bytes")?,
        input_instance_bytes: usize_value(value, "input_instance_bytes")?,
        input_instance_buffer_bytes: usize_value(value, "input_instance_buffer_bytes")?,
        input_metadata_bytes: usize_value(value, "input_metadata_bytes")?,
        request_bytes: usize_value(value, "request_bytes")?,
        request_buffer_bytes: usize_value(value, "request_buffer_bytes")?,
        response_bytes: usize_value(value, "response_bytes")?,
        response_buffer_bytes: usize_value(value, "response_buffer_bytes")?,
        worker_cpu_micros: uint(value, "worker_cpu_micros")?,
        retained_state_bytes: usize_value(value, "retained_state_bytes")?,
        accounted_state_bytes: usize_value(value, "accounted_state_bytes")?,
    })
}
