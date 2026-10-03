//! Maintained exact-source handle wire contract. Handles select; they grant no use.
use crate::source_claim_publication_bytes as bytes;
use crate::source_command::{SourceCommandError, SourceCommandResult};
use serde_json::{Value, json};
use tos_foundation::Digest256;
pub(crate) const HANDLE_BYTES: usize = 16384;
pub(crate) const REQUEST_BYTES: usize = 65536;
pub(crate) const RECORD_BYTES: usize = 1048576;
pub(crate) const RESPONSE_BYTES: usize = 2097152;
pub(crate) const ISSUER: &str = "Tree-of-Sophia/source-witnesses";
pub(crate) const AUTHORED_ISSUER: &str = "Tree-of-Sophia/authored-corpus";
fn invalid() -> SourceCommandError {
    SourceCommandError::Invalid("exact source read wire contract")
}
pub(crate) fn exact(value: &Value, keys: &[&str]) -> SourceCommandResult<()> {
    let object = value.as_object().ok_or_else(invalid)?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn text<'a>(value: &'a Value, key: &str) -> SourceCommandResult<&'a str> {
    value.get(key).and_then(Value::as_str).ok_or_else(invalid)
}
fn bare(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn sha(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(bare)
}
fn identity(value: &str, claim: Option<bool>) -> bool {
    let Some(body) = value.strip_prefix("tos.") else {
        return false;
    };
    if claim.is_some_and(|c| body.starts_with("claim.") != c) {
        return false;
    }
    !body.is_empty()
        && body.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
        })
}
fn kind(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'-')
}
pub(crate) fn canonical(value: &Value, cap: usize) -> SourceCommandResult<Vec<u8>> {
    // Handle digests and selector keys use the maintained compact UTF-8 JSON
    // without LF. Projection row digests deliberately keep their separate LF.
    let framed_cap = cap.checked_add(1).ok_or(SourceCommandError::Unsupported(
        "source response byte budget",
    ))?;
    let mut raw = bytes::canonical(value, framed_cap)
        .map_err(|_| SourceCommandError::Unsupported("source response byte budget"))?;
    if raw.pop() != Some(b'\n') || raw.len() > cap {
        return Err(SourceCommandError::Invalid("source canonical framing"));
    }
    Ok(raw)
}
pub(crate) fn reference(value: &Value, claim: bool) -> SourceCommandResult<()> {
    exact(value, &["id", "version", "digest"])?;
    if !identity(text(value, "id")?, Some(claim))
        || !sha(text(value, "digest")?)
        || !value["version"]
            .as_u64()
            .is_some_and(|n| n > 0 && n <= 9_007_199_254_740_991)
    {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn epoch(value: &Value) -> SourceCommandResult<()> {
    exact(
        value,
        &[
            "source_revision",
            "catalog_root_sha256",
            "catalog_namespace",
            "source_publication",
        ],
    )?;
    let namespace = text(value, "catalog_namespace")?;
    if !bare(text(value, "source_revision")?)
        || !bare(text(value, "catalog_root_sha256")?)
        || namespace.is_empty()
        || namespace.len() > 128
        || !namespace.as_bytes()[0].is_ascii_lowercase()
        || !namespace
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
    {
        return Err(invalid());
    }
    let publication = &value["source_publication"];
    exact(publication, &["protocol", "token", "generation"])?;
    if text(publication, "protocol")? != "tos_selected_source_metadata_v1"
        || !publication["generation"]
            .as_u64()
            .is_some_and(|n| n <= 9_007_199_254_740_991)
        || !publication["token"].as_str().is_some_and(sha)
    {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn target(value: &Value) -> SourceCommandResult<()> {
    match text(value, "layer")? {
        "metadata_record" => {
            exact(
                value,
                &["layer", "record_type", "record_ref", "content_revision"],
            )?;
            if !kind(text(value, "record_type")?) {
                return Err(invalid());
            }
            reference(&value["record_ref"], false)?;
            if value["content_revision"] != value["record_ref"]["digest"] {
                return Err(invalid());
            }
        }
        "claim_record" => {
            exact(value, &["layer", "record_ref", "content_revision"])?;
            reference(&value["record_ref"], true)?;
            if value["content_revision"] != value["record_ref"]["digest"] {
                return Err(invalid());
            }
        }
        "source_slot" => {
            exact(
                value,
                &[
                    "layer",
                    "slot_kind",
                    "identity",
                    "row_sha256",
                    "content_revision",
                ],
            )?;
            if !["claim", "provenance_event", "anchor"].contains(&text(value, "slot_kind")?)
                || !identity(text(value, "identity")?, None)
                || !bare(text(value, "row_sha256")?)
                || !sha(text(value, "content_revision")?)
                || (value["slot_kind"] == "claim"
                    && !identity(text(value, "identity")?, Some(true)))
            {
                return Err(invalid());
            }
        }
        "authored_csv_record" => {
            exact(
                value,
                &[
                    "layer",
                    "pack_id",
                    "edge_id",
                    "source_row",
                    "source_file_sha256",
                    "content_revision",
                ],
            )?;
            let pack = text(value, "pack_id")?;
            let edge = text(value, "edge_id")?;
            if pack.len() > 2048
                || !(pack.starts_with("canon/relations/") || pack.starts_with("candidate-intake/"))
                || pack.split('/').any(|part| {
                    part.is_empty()
                        || matches!(part, "." | ".." | "payload")
                        || part.starts_with('.')
                })
                || pack.contains(['\\', '\0'])
                || edge.is_empty()
                || edge.len() > 2048
                || edge.contains('\0')
                || !bare(text(value, "source_file_sha256")?)
                || !sha(text(value, "content_revision")?)
                || !value["source_row"]
                    .as_u64()
                    .is_some_and(|n| n > 0 && n <= 9_007_199_254_740_991)
            {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    }
    Ok(())
}
pub(crate) fn access(scope: &str, visibility: &str) -> SourceCommandResult<Value> {
    if ![
        "public-metadata-record",
        "public-claim-record",
        "public-source-slot-metadata",
        "public-authored-csv-record",
    ]
    .contains(&scope)
        || !["public", "public_metadata_only"].contains(&visibility)
    {
        return Err(SourceCommandError::Denied(
            "source owner public metadata visibility",
        ));
    }
    Ok(
        json!({"scope":scope,"visibility":visibility,"visibility_verified":true,"rights_revalidated":false,"rights_scope":"metadata-disclosure-only","authority":"source-owner-public-metadata-contract"}),
    )
}
pub(crate) fn make_handle(
    epoch_value: &Value,
    target_value: &Value,
    access_value: &Value,
) -> SourceCommandResult<Value> {
    epoch(epoch_value)?;
    target(target_value)?;
    let mut value = json!({"schema_version":"tos_source_read_handle_v1","issuer":if target_value["layer"]=="authored_csv_record"{AUTHORED_ISSUER}else{ISSUER},"epoch":epoch_value,"target":target_value,"access":access_value});
    let digest = Digest256::of_bytes(&canonical(&value, HANDLE_BYTES)?).to_prefixed();
    value["handle_digest"] = Value::String(digest);
    validate_handle(&value)?;
    Ok(value)
}
pub(crate) fn validate_handle(value: &Value) -> SourceCommandResult<()> {
    canonical(value, HANDLE_BYTES)?;
    exact(
        value,
        &[
            "schema_version",
            "issuer",
            "epoch",
            "target",
            "access",
            "handle_digest",
        ],
    )?;
    epoch(&value["epoch"])?;
    target(&value["target"])?;
    let expected_issuer = if value["target"]["layer"] == "authored_csv_record" {
        AUTHORED_ISSUER
    } else {
        ISSUER
    };
    if text(value, "schema_version")? != "tos_source_read_handle_v1"
        || text(value, "issuer")? != expected_issuer
    {
        return Err(invalid());
    }
    let selected_scope = match text(&value["target"], "layer")? {
        "metadata_record" => "public-metadata-record",
        "claim_record" => "public-claim-record",
        "source_slot" => "public-source-slot-metadata",
        _ => "public-authored-csv-record",
    };
    let expected = access(selected_scope, text(&value["access"], "visibility")?)?;
    if value["access"] != expected {
        return Err(invalid());
    }
    let mut unsigned = value.clone();
    unsigned
        .as_object_mut()
        .ok_or_else(invalid)?
        .remove("handle_digest");
    if text(value, "handle_digest")?
        != Digest256::of_bytes(&canonical(&unsigned, HANDLE_BYTES)?).to_prefixed()
    {
        return Err(invalid());
    }
    Ok(())
}
