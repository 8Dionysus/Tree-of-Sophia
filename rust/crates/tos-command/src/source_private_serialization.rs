//! Provenance capture for the two existing private metadata families.
use super::*;

pub(crate) enum PrivateMetadataFamily {
    Profile,
    Claim,
}

pub(crate) fn capture_private_metadata(
    family: PrivateMetadataFamily,
    request: &JsonValue,
    event_id: &str,
    home: &str,
    files: &mut BTreeMap<String, Vec<u8>>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let configuration =
        files
            .get("source-create-owner-configuration.json")
            .ok_or(SourceCommandError::Invalid(
                "private metadata retained configuration",
            ))?;
    let binding = json!({"ref":format!("{home}/source-create-owner-configuration.json"),
        "sha256":Digest256::of_bytes(configuration).to_hex()});
    let procedure = match family {
        PrivateMetadataFamily::Profile => "owner-local-source-profile-metadata-serialization",
        PrivateMetadataFamily::Claim => "owner-local-source-claim-serialization",
    };
    capture_creation_with_procedure(
        request, event_id, home, files, software, components, procedure, None, deadline, cancelled,
    )?;
    let raw = files
        .get("source-create-provenance.jsonl")
        .ok_or(SourceCommandError::Invalid(
            "private metadata capture missing",
        ))?;
    let mut event: Value = serde_json::from_slice(raw)
        .map_err(|_| SourceCommandError::Invalid("private metadata capture JSON"))?;
    for group in ["inputs", "outputs", "byproducts"] {
        for entity in event["entities"][group]
            .as_array_mut()
            .ok_or(SourceCommandError::Invalid(
                "private metadata capture entities",
            ))?
        {
            entity["content_disclosure"] = json!("private_content");
        }
    }
    event["method"]["configuration_binding"] = binding;
    event["rights_and_visibility"]["intended_uses"] = json!(["local_research"]);
    event["rights_and_visibility"]["content_visibility"] = json!("local_only");
    files.insert("source-create-provenance.jsonl".into(), encoded(event)?);
    active(deadline, cancelled)
}
