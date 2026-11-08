//! Planting a media observation in the source home.
//!
//! The Tree's planting route has three owners in turn. `media.prepare-artifact`
//! writes the evidence an Artifact rests on: the observation as research, an
//! artifact-targeted discovery run, metadata-only rights and their discovery
//! provenance event; it returns the `native-artifact-create` request and the
//! derived part of its owner configuration. The native owner creates the
//! Artifact with its human forms under the operator's configuration. Then
//! `media.plant-representation` writes an image-service representation of that
//! Artifact with the operator's rights assessment and its provenance event.
//! Rights, Artifact identity and publication keep their owners: this module
//! writes the operator's decisions and the provider's statements, and decides
//! neither.

use crate::source_registry_acquisition::{append_jsonl, publish_json, validate_json};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tos_foundation::Digest256;

type Result<T> = std::result::Result<T, String>;

const ARTIFACTS: &str = "ToS/source-witnesses/artifacts";
const RUNS: &str = "ToS/source-witnesses/discovery/runs";
const PROVENANCE: &str = "ToS/source-witnesses/discovery/provenance.jsonl";
const OBSERVATIONS: &str = "ToS/research-packets/media-observations";
const AGENT: &str = "software:tos-native-owner-command/acquisition/media";
const CONTRACTS: &str = "https://tree-of-sophia.local/ToS/contracts";

pub(crate) fn prepare_artifact(request: &Value) -> Result<Value> {
    let root = root(request)?;
    let observation = observation(&root, request)?;
    let home = ArtifactHome::parse(text(request, "artifact_path")?)?;
    let owner = field(request, "owner")?;
    let maker_type = text(owner, "maker_type")?;
    if !["human", "model", "mixed", "software"].contains(&maker_type) {
        return Err("owner maker_type must be human, model, mixed or software".into());
    }
    let principal = nonempty_text(owner, "principal_id")?;
    let creation_event = nonempty_text(owner, "provenance_event_id")?;
    let purpose = nonempty_text(request, "purpose")?;
    let provider = text(&observation, "provider")?;
    let object = field(&observation, "object")?;
    let record_url = text(object, "record_url")?;
    let date = day(text(&observation, "observed_at")?)?;
    let slug = format!("{provider}-{}", home.identity);

    let research_ref = format!("{OBSERVATIONS}/{provider}/{}.{date}.json", home.identity);
    let rights_ref = format!("{}/rights.json", home.dir());
    let run_ref = format!("{RUNS}/media-{slug}.{date}.v1.json");
    let event_id = format!("tos.event.discovery.media-{slug}.{}", date.replace('-', ""));
    for reference in [&research_ref, &rights_ref, &run_ref, &home.witness()] {
        absent(&root, reference)?;
    }
    event_absent(&root, &event_id)?;
    event_absent(&root, creation_event)?;

    let now = utcnow()?;
    let rights = json!({
        "schema_version":"tos_rights_record_v1",
        "rights_id":format!("tos.rights.{}-public-metadata", home.dotted()),
        "scope_refs":[home.artifact_id()],
        "assessment_status":"copyright_not_evaluated",
        "rights_statement_uri":null,
        "license_uri":null,
        "jurisdictions_reviewed":[],
        "source_refs":[record_url],
        "permissions":["retain the provider's factual object identity, inventory number, dating, dimensions, custody, source links and layer boundaries"],
        "restrictions":["this metadata record licenses no image, inscription, museum prose or byte sequence; each visual representation carries its own rights record"],
        "visibility":"public_metadata_only",
        "redistribution_posture":"metadata_only",
        "derivative_posture":"local_research_only",
        "assessed_by":{"maker_type":maker_type,"agent_ref":principal},
        "assessed_at":now,
        "rationale":"The Artifact packet exposes factual catalogue metadata only; image rights are assessed per representation.",
        "review_status":"unreviewed",
        "review_refs":[],
        "access_request_ref":null,
        "record_version":1,
        "supersedes_rights_ref":null
    });
    validate_json(&root, &rights, "rights-record")?;

    let run = discovery_run(
        &observation,
        &home,
        &run_ref,
        &event_id,
        purpose,
        &research_ref,
    )?;
    validate_json(&root, &run, "material-discovery-record")?;

    let research = path(&root, &research_ref)?;
    write_new(&research, &observation)?;
    write_new(&path(&root, &rights_ref)?, &rights)?;
    write_new(&path(&root, &run_ref)?, &run)?;
    let outputs = [
        (&run_ref, "artifact-targeted-provider-discovery"),
        (&rights_ref, "artifact-metadata-only-rights"),
        (&research_ref, "provider-observation-research"),
    ]
    .iter()
    .map(|(reference, role)| digest_ref(&root, reference, role))
    .collect::<Result<Vec<_>>>()?;
    let started = text(&observation, "observed_at")?;
    let event = provenance(
        &event_id,
        "discovery",
        started,
        &now,
        vec![],
        outputs,
        "media-provider-observation-discovery",
        json!({"provider":provider,"requested_object":observation["requested_object"],"general_web_search_performed":false}),
        &rights_ref,
        vec![run_ref.clone()],
    );
    validate_json(&root, &event, "provenance-event")?;
    append_jsonl(&path(&root, PROVENANCE)?, &event)?;

    let bindings = json!({
        "rights_ref":digest_binding(&root, &rights_ref)?,
        "discovery_ref":digest_binding(&root, &run_ref)?,
        "research_ref":digest_binding(&root, &research_ref)?,
    });
    let record = artifact_record(
        &observation,
        &home,
        [&rights_ref, &run_ref, &research_ref],
        creation_event,
        maker_type,
        principal,
        &now,
    )?;
    validate_json(&root, &record, "artifact-source-witness-v2")?;
    let form_ids = [home.form_id("source-name"), home.form_id("source-note")];
    let forms = json!([
        {"form_id":form_ids[0],"field_id":"metadata.preferred-name"},
        {"form_id":form_ids[1],"field_id":"metadata.source-note"}
    ]);
    check_forms(&root, &record, &forms, &form_ids, principal)?;
    Ok(json!({
        "written":[research_ref,rights_ref,run_ref,PROVENANCE],
        "discovery_event_id":event_id,
        "artifact_create":{
            "request":{
                "schema_version":"tos_local_source_command_v1",
                "operation":"prepare-create",
                "record":record,
                "forms":forms,
                "source_bindings":bindings
            },
            "owner_configuration":{
                "schema_version":"tos_local_artifact_create_owner_v1",
                "principal_id":principal,
                "maker_type":maker_type,
                "source_path":home.witness(),
                "record_id":home.artifact_id(),
                "provenance_event_id":creation_event,
                "allowed_operations":["source.create"],
                "allowed_form_ids":form_ids,
                "source_bindings":bindings
            }
        }
    }))
}

pub(crate) fn plant_representation(request: &Value) -> Result<Value> {
    let root = root(request)?;
    let observation = observation(&root, request)?;
    let home = ArtifactHome::parse(text(request, "artifact_path")?)?;
    let view = slug_text(request, "view")?;
    let index = request
        .get("image")
        .map(|value| value.as_u64().ok_or("image must be an image index"))
        .transpose()?
        .unwrap_or(0);
    let image = observation["images"]
        .get(usize::try_from(index).map_err(|_| "image index")?)
        .ok_or("the observation has no image at that index")?;
    if image["form"] != "image_service" {
        return Err("only an image-service image plants without a payload; a file image needs the payload route".into());
    }
    let service = field(image, "image_service")?;
    let authorization = nonempty_text(request, "operator_authorization_ref")?;
    let maker = field(request, "maker")?;
    let maker_type = text(maker, "maker_type")?;
    let agent = nonempty_text(maker, "agent_ref")?;
    let decision = field(request, "rights")?;

    let witness_ref = home.witness();
    let witness: Value = serde_json::from_slice(
        &std::fs::read(path(&root, &witness_ref)?)
            .map_err(|_| format!("plant the Artifact first: {witness_ref} is absent"))?,
    )
    .map_err(|error| format!("{witness_ref}: {error}"))?;
    let artifact_id = home.artifact_id();
    if witness["artifact_id"] != artifact_id.as_str() {
        return Err(format!("{witness_ref} does not hold {artifact_id}"));
    }
    let discovery_ref = text(&witness, "discovery_ref")?.to_owned();
    let base = format!("{}/representations/{view}", home.dir());
    let representation_ref = format!("{base}/representation.json");
    let rights_ref = format!("{base}/rights.json");
    let representation_id = image_service_id(text(service, "service_id")?);
    let event_id = format!(
        "tos.event.acquisition.media-{}-{view}.{}",
        home.identity,
        day(text(&observation, "observed_at")?)?.replace('-', "")
    );
    for reference in [&representation_ref, &rights_ref] {
        absent(&root, reference)?;
    }
    event_absent(&root, &event_id)?;

    let posture = text(decision, "redistribution_posture")?;
    if !["authorized", "authorized_with_conditions"].contains(&posture) {
        return Err("an image-service representation needs an authorized redistribution posture from the rights assessment".into());
    }
    let now = utcnow()?;
    let statements: Vec<Value> = observation["rights"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|statement| statement["scope"] == "image")
        .filter_map(|statement| statement["stated_in"].as_str())
        .map(|url| Value::String(url.to_owned()))
        .collect();
    let mut source_refs: Vec<Value> = decision
        .get("source_refs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for reference in statements {
        if !source_refs.contains(&reference) {
            source_refs.push(reference);
        }
    }
    let rights = json!({
        "schema_version":"tos_rights_record_v1",
        "rights_id":format!("tos.rights.{}-{view}", home.dotted()),
        "scope_refs":[artifact_id,representation_id],
        "assessment_status":field(decision,"assessment_status")?,
        "rights_statement_uri":decision.get("rights_statement_uri").cloned().unwrap_or(Value::Null),
        "license_uri":decision.get("license_uri").cloned().unwrap_or(Value::Null),
        "jurisdictions_reviewed":field(decision,"jurisdictions_reviewed")?,
        "source_refs":source_refs,
        "permissions":field(decision,"permissions")?,
        "restrictions":field(decision,"restrictions")?,
        "visibility":"public_metadata_only",
        "redistribution_posture":posture,
        "derivative_posture":field(decision,"derivative_posture")?,
        "assessed_by":field(decision,"assessed_by")?,
        "assessed_at":now,
        "rationale":field(decision,"rationale")?,
        "review_status":"unreviewed",
        "review_refs":[],
        "access_request_ref":null,
        "record_version":1,
        "supersedes_rights_ref":null
    });
    validate_json(&root, &rights, "rights-record")?;

    let requests = observation["requests"]
        .as_array()
        .ok_or("observation requests")?;
    let object_request = requests
        .iter()
        .find(|request| request["role"] == "object-record")
        .ok_or("the observation has no object record")?;
    let fingerprint = &service["info_fingerprint"];
    let representation = json!({
        "$schema":format!("{CONTRACTS}/artifact-visual-representation-v2.schema.json"),
        "schema_version":"tos_artifact_visual_representation_v2",
        "representation_form":"image_service",
        "representation_id":representation_id,
        "artifact_id":artifact_id,
        "artifact_ref":witness_ref,
        "layer_kind":"photograph",
        "view_label":format!("{view}, {} image service", provider_name(text(&observation,"provider")?)),
        "provider_records":[
            {"provider":provider_name(text(&observation,"provider")?),"role":"originating-record","record_url":observation["object"]["record_url"],"api_url":object_request["final_url"],"checked_at":object_request["started_at"],"response_fingerprint":{"byte_size":object_request["byte_size"],"sha256":object_request["sha256"],"captured":false}},
            {"provider":format!("{} IIIF Image API",provider_name(text(&observation,"provider")?)),"role":"image-service-record","record_url":fingerprint["url"],"api_url":null,"checked_at":fingerprint["checked_at"],"response_fingerprint":{"byte_size":fingerprint["byte_size"],"sha256":fingerprint["sha256"],"captured":false}}
        ],
        "observation":{"service_info_url":fingerprint["url"],"observed_at":fingerprint["checked_at"],"transport":"https","technical_access_bypass_used":false,"operator_authorization_ref":authorization},
        "image_service":service,
        "rights_ref":rights_ref,
        "discovery_ref":discovery_ref,
        "provenance_event_ref":event_id,
        "layer_separation":{"visual_is_physical_artifact":false,"visual_is_catalog_record":false,"visual_is_transcription":false,"embedded_text_admitted":false,"same_bytes_as_originating_provider_asserted":false},
        "authority":{"evidence_status":"visual_source_witness","review_status":"unreviewed","visibility":"public_metadata_only","source_text_admitted":false,"semantic_authority":false,"graph_authority":false,"canon_authority":false,"publication_authority":false},
        "maker":{"maker_type":maker_type,"agent_ref":agent,"human_review_performed":false},
        "created_at":now,
        "record_version":1
    });
    validate_json(&root, &representation, "artifact-visual-representation-v2")?;

    write_new(&path(&root, &rights_ref)?, &rights)?;
    write_new(&path(&root, &representation_ref)?, &representation)?;
    let outputs = vec![
        digest_ref(
            &root,
            &representation_ref,
            "image-service-visual-representation",
        )?,
        digest_ref(&root, &rights_ref, "image-service-rights")?,
    ];
    let event = provenance(
        &event_id,
        "acquisition",
        text(fingerprint, "checked_at")?,
        &now,
        vec![digest_ref(&root, &witness_ref, "represented-artifact")?],
        outputs,
        "media-image-service-representation",
        json!({"service_id":service["service_id"],"source_payloads_acquired":0,"publication_authorized":false}),
        &rights_ref,
        vec![representation_ref.clone(), rights_ref.clone()],
    );
    validate_json(&root, &event, "provenance-event")?;
    append_jsonl(&path(&root, PROVENANCE)?, &event)?;
    Ok(json!({
        "written":[representation_ref,rights_ref,PROVENANCE],
        "representation_id":representation_id,
        "provenance_event_id":event_id
    }))
}

/// `<tradition>/<site>/<identity>` under the Artifact spine.
struct ArtifactHome {
    tradition: String,
    site: String,
    identity: String,
}

impl ArtifactHome {
    fn parse(value: &str) -> Result<Self> {
        let parts: Vec<&str> = value.split('/').collect();
        let slug = |part: &str| {
            !part.is_empty()
                && part.len() <= 96
                && part.split('-').all(|word| {
                    !word.is_empty()
                        && word
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                })
        };
        if parts.len() != 3 || !parts.iter().all(|part| slug(part)) {
            return Err("artifact_path is <tradition>/<site>/<identity> in lowercase slugs".into());
        }
        if parts.iter().any(|part| part.eq_ignore_ascii_case("cdli")) {
            return Err("an Artifact home is keyed by physical identity, not a provider".into());
        }
        Ok(Self {
            tradition: parts[0].into(),
            site: parts[1].into(),
            identity: parts[2].into(),
        })
    }
    fn dir(&self) -> String {
        format!(
            "{ARTIFACTS}/{}/{}/{}",
            self.tradition, self.site, self.identity
        )
    }
    fn witness(&self) -> String {
        format!("{}/artifact-witness.json", self.dir())
    }
    fn dotted(&self) -> String {
        format!("{}.{}.{}", self.tradition, self.site, self.identity)
    }
    fn artifact_id(&self) -> String {
        format!("tos.artifact.{}", self.dotted())
    }
    fn form_id(&self, role: &str) -> String {
        format!("tos.form.artifact.{}.{role}", self.dotted())
    }
}

fn discovery_run(
    observation: &Value,
    home: &ArtifactHome,
    run_ref: &str,
    event_id: &str,
    purpose: &str,
    research_ref: &str,
) -> Result<Value> {
    let provider = text(observation, "provider")?;
    let object = field(observation, "object")?;
    let record_url = text(object, "record_url")?;
    let requests = observation["requests"]
        .as_array()
        .ok_or("observation requests")?;
    let identifiers: Vec<Value> = object["identifiers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|identifier| json!({"scheme":identifier["scheme"],"value":identifier["value"]}))
        .collect();
    let title = object["titles"]
        .as_array()
        .and_then(|titles| titles.first())
        .and_then(|title| title["text"].as_str())
        .unwrap_or(text(observation, "requested_object")?);
    let rights_statement = observation["rights"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|statement| {
            format!(
                "{} {}",
                statement["provider_field"].as_str().unwrap_or(""),
                statement["value"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let rights_evidence = observation["rights"]
        .as_array()
        .and_then(|rights| rights.first())
        .and_then(|statement| statement["stated_in"].as_str())
        .unwrap_or(record_url);
    let has_service = observation["images"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|image| image["form"] == "image_service");
    let mut channels = Vec::new();
    for (index, request) in requests.iter().enumerate() {
        let sequence = index + 1;
        let role = text(request, "role")?;
        let url = text(request, "url")?;
        let result_id = format!(
            "tos-discovery-result.media-{provider}-{}-{sequence}",
            home.identity
        );
        let (channel_role, interface, machine) = match role {
            "image-service-info" => ("digital-object-record", "iiif", "iiif"),
            "digital-object-record" | "visual-item-record" => {
                ("digital-object-record", "api", "api")
            }
            _ => ("originating-record", "api", "api"),
        };
        channels.push(json!({
            "channel_id":format!("channel-media-{provider}-{}-{sequence}", home.identity),
            "sequence":sequence,
            "channel_type":"collection-aggregator",
            "role":channel_role,
            "source_name":provider_name(provider),
            "endpoint_url":url,
            "interface_type":interface,
            "interface_version":format!("{role} response observed {}", request["started_at"].as_str().unwrap_or("")),
            "exact_query":format!("GET {url}"),
            "queried_at":request["started_at"],
            "elapsed_seconds":request["elapsed_seconds"],
            "result_order_preserved":true,
            "results":[{
                "result_id":result_id,
                "rank":1,
                "title_as_displayed":title,
                "result_url":request["final_url"],
                "originating_record_url":record_url,
                "identifiers":identifiers,
                "available_formats":[request["media_type"].as_str().unwrap_or("JSON")],
                "declared_rights":{
                    "statement":if rights_statement.is_empty() { "The provider states no rights for this object or its images.".to_owned() } else { rights_statement.clone() },
                    "scope":if has_service || !observation["images"].as_array().is_none_or(Vec::is_empty) { "digital-object" } else { "metadata" },
                    "evidence_url":rights_evidence,
                    "tos_conclusion":"evidence-only-not-a-rights-conclusion"
                },
                "availability":"open-view",
                "machine_interface":machine,
                "decision":"select",
                "rationale":format!("The {role} response belongs to the observed object; its bytes are fingerprinted in the observation."),
                "acquisition":{"downloaded":false,"acquired_at":null,"byte_size":null,"sha256":null,"event_ref":null},
                "snapshot":{"state":"not-captured","format":null,"sha256":null,"reason":format!("The response's size and SHA-256 are retained in {research_ref}.")}
            }]
        }));
    }
    let selected: Vec<Value> = channels
        .iter()
        .map(|channel| channel["results"][0]["result_id"].clone())
        .collect();
    let comparison: Vec<Value> = channels
        .iter()
        .map(|channel| json!({
            "channel_id":channel["channel_id"],
            "completeness":"adequate",
            "metadata_precision":"strong",
            "rights_clarity":"adequate",
            "machine_interface_quality":"strong",
            "human_minutes":0,
            "machine_seconds":channel["elapsed_seconds"],
            "notes":"One response of the provider's own route for the observed object; the media bridge measured it."
        }))
        .collect();
    let ended = requests
        .iter()
        .filter_map(|request| request["started_at"].as_str())
        .max()
        .ok_or("observation requests have no time")?;
    Ok(json!({
        "$schema":format!("{CONTRACTS}/material-discovery-record.schema.json"),
        "schema_version":"tos_material_discovery_record_v1",
        "discovery_id":format!("tos.discovery.{}", run_ref.trim_start_matches(&format!("{RUNS}/")).trim_end_matches(".json")),
        "protocol_ref":"ToS/source-witnesses/discovery/DISCOVERY_PROTOCOL.md",
        "target":{
            "target_kind":"artifact",
            "known_tos_refs":[home.artifact_id()],
            "description":purpose,
            "required_properties":[
                "the provider's own object record and its exact inventory number",
                "the image route the provider serves, with its rights statement as stated",
                "no transcription, interpretation, rights decision or publication effect"
            ],
            "acceptable_substitutions":["a later record of the same provider for the same physical object"],
            "languages":[],
            "formats":["provider object record","IIIF Image API info.json"],
            "purpose_ref":research_ref
        },
        "channels":channels,
        "channel_comparison":comparison,
        "selected_result_ids":selected,
        "rejected_result_ids":[],
        "rights_inference_from_availability_prohibited":true,
        "general_web_search_is_last_resort":true,
        "technical_access_bypass_used":false,
        "maker":{"maker_type":"software","agent_ref":AGENT},
        "started_at":observation["observed_at"],
        "ended_at":ended,
        "status":"executed",
        "provenance_event_refs":[event_id],
        "record_version":1,
        "supersedes_discovery_ref":null
    }))
}

fn artifact_record(
    observation: &Value,
    home: &ArtifactHome,
    [rights_ref, run_ref, research_ref]: [&String; 3],
    creation_event: &str,
    maker_type: &str,
    principal: &str,
    now: &str,
) -> Result<Value> {
    let provider = text(observation, "provider")?;
    let object = field(observation, "object")?;
    let record_url = text(object, "record_url")?;
    let inventory = object["identifiers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|identifier| {
            matches!(
                identifier["scheme"].as_str(),
                Some("Rijksmuseum object number" | "Met object number")
            )
        })
        .and_then(|identifier| identifier["value"].as_str())
        .ok_or("the observation names no inventory number")?;
    let repository = object["repository"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| provider_name(provider).to_owned());
    let identifiers: Vec<Value> = object["identifiers"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|identifier| json!({"scheme":identifier["scheme"],"value":identifier["value"],"source_ref":record_url}))
        .collect();
    let materials: Vec<&str> = object["materials"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let dimension = |kind: &str| -> Option<Value> {
        object["dimensions"]
            .as_array()?
            .iter()
            .find(|dimension| {
                dimension["kind"].as_str().is_some_and(|label| {
                    label.eq_ignore_ascii_case(kind)
                        || label.to_lowercase().ends_with(&format!(" {kind}"))
                })
            })
            .and_then(|dimension| {
                let centimetres = dimension["unit"] == "cm"
                    || dimension["unit_uri"] == "http://vocab.getty.edu/aat/300379098";
                centimetres
                    .then(|| dimension["value"].as_str())
                    .flatten()
                    .and_then(millimetres)
            })
    };
    let statement = object["dating"]["statement"].as_str();
    let fingerprints: Vec<Value> = observation["requests"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|request| request["role"] != "image-service-info")
        .map(|request| json!({"surface":request["final_url"],"byte_size":request["byte_size"],"sha256":request["sha256"],"captured":false}))
        .collect();
    let checked_at = observation["requests"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|request| request["role"] == "object-record")
        .and_then(|request| request["started_at"].as_str())
        .ok_or("the observation has no object record")?;
    let classifications: Vec<Value> = object["classifications"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let visual_layers: Vec<Value> = observation["images"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|image| {
            json!({
                "layer_kind":"photograph",
                "availability":"linked",
                "content_stored":false,
                "source_ref":image["access_url"],
                "rights_note":"Each visual representation carries its own rights record."
            })
        })
        .collect();
    Ok(json!({
        "$schema":format!("{CONTRACTS}/artifact-source-witness-v2.schema.json"),
        "schema_version":"tos_artifact_source_witness_v2",
        "artifact_id":home.artifact_id(),
        "artifact_kind":"other",
        "path_identity":{
            "basis":"repository_identity",
            "provider_independent":true,
            "note":format!("The path follows {repository} inventory number {inventory}, not an image identifier, a provider URL or a title.")
        },
        "identifiers":identifiers,
        "physical_description":{
            "material":if materials.is_empty() { "not stated in the provider record".to_owned() } else { materials.join("; ") },
            "height_mm":dimension("height"),
            "width_mm":dimension("width"),
            "thickness_mm":null,
            "source_ref":record_url
        },
        "dating":{
            "period_label":statement.unwrap_or("not dated in the provider record"),
            "display_date":statement.unwrap_or("not dated in the provider record"),
            "certainty":if statement.is_some() { "reported" } else { "unknown" },
            "period_stage_separation_required":true,
            "source_ref":record_url
        },
        "find_context":{
            "site":null,
            "findspot":null,
            "excavation_numbers":[],
            "source_note":null,
            "documentation_limit":"The provider record states the object's making and custody; it states no find context.",
            "source_ref":record_url
        },
        "custody":{"current_repository":repository,"inventory_numbers":[inventory],"source_ref":record_url},
        "digital_catalog_record":{
            "provider":provider_name(provider),
            "record_id":inventory,
            "record_url":record_url,
            "api_url":object["api_url"],
            "checked_at":checked_at,
            "mutable":true,
            "response_fingerprints":fingerprints
        },
        "inscription_layer":{
            "availability":"unknown",
            "language_status":"unknown",
            "provider_language_code":null,
            "normalized_language_code":null,
            "script":null,
            "transliteration_stored":false,
            "translation_stored":false,
            "source_refs":[record_url]
        },
        "composite_layer":{
            "status":"absent",
            "external_refs":[],
            "relationship":"The object is one physical work; no scholarly composite gathers it.",
            "is_physical_artifact_identity":false
        },
        "visual_layers":visual_layers,
        "scholarly_classification":{
            "genre":classifications.first().and_then(Value::as_str).unwrap_or("object"),
            "corpus_labels":classifications,
            "classification_is_interpretive":true,
            "source_refs":[record_url]
        },
        "bibliography":[],
        "layer_separation":{
            "physical_artifact_is_catalog_record":false,
            "catalog_record_is_inscription":false,
            "inscription_is_composite":false,
            "visual_is_artifact":false,
            "sign_has_fixed_translation":false,
            "artifact_is_philosophy":false
        },
        "rights_ref":rights_ref,
        "discovery_ref":run_ref,
        "provenance_event_ref":creation_event,
        "research_ref":research_ref,
        "philosophy_planting_refs":[],
        "authority":{
            "evidence_status":"metadata_witness",
            "review_status":"unreviewed",
            "visibility":"public_metadata_only",
            "source_text_admitted":false,
            "semantic_authority":false,
            "graph_authority":false,
            "canon_authority":false,
            "publication_authority":false
        },
        "maker":{"maker_type":maker_type,"agent_ref":principal,"human_review_performed":false},
        "created_at":now,
        "record_version":1
    }))
}

/// The native form preparation the creation owner runs, applied here so the
/// returned request carries a ready name form.
fn check_forms(
    root: &Path,
    record: &Value,
    forms: &Value,
    form_ids: &[String; 2],
    principal: &str,
) -> Result<()> {
    use crate::source_command as cmd;
    let parse = |value: &Value| {
        cmd::parse(&serde_json::to_vec(value).map_err(|error| error.to_string())?)
            .map_err(|error| format!("{error:?}"))
    };
    let config = parse(&json!({"allowed_form_ids":form_ids,"principal_id":principal}))?;
    let record = parse(record)?;
    let selections = parse(forms)?;
    let selections = selections.as_array().ok_or("form selections")?;
    let mut schema_error = None;
    crate::source_creation::prepare_source_forms(
        &config,
        &record,
        selections,
        |_, root_schema, instance| {
            let value: Value = cmd::canonical(instance)
                .ok()
                .and_then(|raw| serde_json::from_slice(&raw).ok())
                .unwrap_or(Value::Null);
            let result = contract_uri(root, root_schema).and_then(|uri| {
                crate::source_registry_acquisition::validate_json_uri(
                    root,
                    &value,
                    root_schema,
                    &uri,
                )
            });
            if let Err(error) = result {
                schema_error = Some(error);
            }
            Ok(())
        },
    )
    .map_err(|error| format!("Artifact forms: {error:?}"))?;
    match schema_error {
        Some(error) => Err(format!("Artifact forms: {error}")),
        None => Ok(()),
    }
}

/// The `$id` a contract file declares.
fn contract_uri(root: &Path, contract: &str) -> Result<String> {
    let raw =
        std::fs::read(path(root, contract)?).map_err(|error| format!("{contract}: {error}"))?;
    let schema: Value =
        serde_json::from_slice(&raw).map_err(|error| format!("{contract}: {error}"))?;
    text(&schema, "$id").map(str::to_owned)
}

#[allow(clippy::too_many_arguments)]
fn provenance(
    event_id: &str,
    event_type: &str,
    started: &str,
    ended: &str,
    inputs: Vec<Value>,
    outputs: Vec<Value>,
    name: &str,
    configuration: Value,
    rights_ref: &str,
    receipts: Vec<String>,
) -> Value {
    json!({
        "schema_version":"tos_provenance_event_v1",
        "event_id":event_id,
        "event_type":event_type,
        "started_at":started,
        "ended_at":ended,
        "agent_refs":[AGENT],
        "inputs":inputs,
        "outputs":outputs,
        "method":{
            "maker_type":"software",
            "name":name,
            "version":"1",
            "artifact_digest":Digest256::of_bytes(include_bytes!("source_media_plant.rs")).to_hex(),
            "runtime":"rust-native",
            "configuration":configuration
        },
        "status":"completed",
        "warnings":[],
        "receipt_refs":receipts,
        "rights_basis_ref":rights_ref,
        "event_version":1,
        "supersedes_event_ref":null
    })
}

fn provider_name(provider: &str) -> &'static str {
    match provider {
        "rijksmuseum" => "Rijksmuseum",
        "met" => "The Metropolitan Museum of Art",
        _ => "media provider",
    }
}

/// A centimetre value as written, in millimetres.
fn millimetres(centimetres: &str) -> Option<Value> {
    let (whole, fraction) = centimetres.split_once('.').unwrap_or((centimetres, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let digits = format!("{whole}{fraction}");
    let shift = fraction.len();
    // Ten millimetres to the centimetre move the point one place right.
    let value = if shift == 0 {
        format!("{digits}0")
    } else if shift == 1 {
        digits
    } else {
        let point = digits.len() - (shift - 1);
        format!("{}.{}", &digits[..point], &digits[point..])
    };
    let value = value.trim_start_matches('0');
    serde_json::from_str::<Value>(if value.is_empty() || value.starts_with('.') {
        return None;
    } else {
        value
    })
    .ok()
    .filter(|number| number.as_f64().is_some_and(|mm| mm > 0.0))
}

fn image_service_id(service_id: &str) -> String {
    format!(
        "tos.image-service.sha256.{}",
        Digest256::of_bytes(service_id.as_bytes()).to_hex()
    )
}

fn observation(root: &Path, request: &Value) -> Result<Value> {
    let observation = field(request, "observation")?.clone();
    validate_json(root, &observation, "media-provider-observation")?;
    Ok(observation)
}

fn root(request: &Value) -> Result<PathBuf> {
    let root = PathBuf::from(text(request, "repo_root")?);
    if !root.is_absolute() || !root.join("ToS/contracts").is_dir() {
        return Err("repo_root must be the absolute root of a Tree of Sophia checkout".into());
    }
    Ok(root)
}

fn path(root: &Path, reference: &str) -> Result<PathBuf> {
    tos_foundation::RelativePath::parse(reference)
        .map_err(|_| format!("unsafe path: {reference}"))?;
    Ok(root.join(reference))
}

fn absent(root: &Path, reference: &str) -> Result<()> {
    if path(root, reference)?.exists() {
        return Err(format!(
            "{reference} exists; planting writes only new records"
        ));
    }
    Ok(())
}

fn event_absent(root: &Path, event_id: &str) -> Result<()> {
    let raw = std::fs::read(path(root, PROVENANCE)?).unwrap_or_default();
    let needle = format!("\"event_id\":\"{event_id}\"");
    if String::from_utf8_lossy(&raw).contains(&needle) {
        return Err(format!("{event_id} is already recorded"));
    }
    Ok(())
}

fn write_new(target: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    publish_json(target, value)
}

fn digest_ref(root: &Path, reference: &str, role: &str) -> Result<Value> {
    let raw =
        std::fs::read(path(root, reference)?).map_err(|error| format!("{reference}: {error}"))?;
    Ok(json!({"ref":reference,"role":role,"sha256":Digest256::of_bytes(&raw).to_hex()}))
}

fn digest_binding(root: &Path, reference: &str) -> Result<Value> {
    let raw =
        std::fs::read(path(root, reference)?).map_err(|error| format!("{reference}: {error}"))?;
    Ok(json!({"ref":reference,"sha256":Digest256::of_bytes(&raw).to_hex()}))
}

fn day(instant: &str) -> Result<String> {
    let day = instant.get(..10).ok_or("observation time")?;
    let ok = day.len() == 10
        && day.bytes().enumerate().all(|(index, b)| {
            if index == 4 || index == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    ok.then(|| day.to_owned())
        .ok_or_else(|| "observation time".into())
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value.get(key).ok_or_else(|| format!("{key} is missing"))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be text"))
}

fn nonempty_text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    text(value, key).and_then(|text| {
        (!text.trim().is_empty() && text.len() <= 4096)
            .then_some(text)
            .ok_or_else(|| format!("{key} must be 1..4096 characters"))
    })
}

fn slug_text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    let slug = text(value, key)?;
    let ok = !slug.is_empty()
        && slug.len() <= 96
        && slug.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        });
    ok.then_some(slug)
        .ok_or_else(|| format!("{key} must be a lowercase slug"))
}

fn utcnow() -> Result<String> {
    crate::source_serialization::instant()
        .map_err(|error| format!("native clock unavailable: {error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, Instant};
    use tos_validation::item_rules::{ItemLimits, ItemRefusal};
    use tos_validation::layer_family_rules::{LayerFamilyRules, LayerFamilySource};

    const HOME: &str = "dutch-painting/amsterdam/rijksmuseum-sk-c-5";

    fn workspace() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap()
    }

    /// A checkout holding the real contracts and nothing planted yet.
    fn scratch() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let contracts = dir.path().join("ToS/contracts");
        fs::create_dir_all(&contracts).unwrap();
        for entry in fs::read_dir(workspace().join("ToS/contracts")).unwrap() {
            let entry = entry.unwrap();
            if entry
                .file_name()
                .to_string_lossy()
                .ends_with(".schema.json")
            {
                fs::copy(entry.path(), contracts.join(entry.file_name())).unwrap();
            }
        }
        dir
    }

    fn example(name: &str) -> Value {
        serde_json::from_slice(
            &fs::read(workspace().join("ToS/contracts/examples").join(name)).unwrap(),
        )
        .unwrap()
    }

    fn prepare_request(root: &Path, observation: Value) -> Value {
        json!({
            "operation":"media.prepare-artifact","repo_root":root,"observation":observation,
            "artifact_path":HOME,
            "owner":{"maker_type":"model","principal_id":"agent:media-test","provenance_event_id":"tos.event.creation.artifact-rijksmuseum-sk-c-5"},
            "purpose":"The Night Watch as visual material for the Tree."
        })
    }

    fn plant_request(root: &Path, observation: Value, posture: &str) -> Value {
        json!({
            "operation":"media.plant-representation","repo_root":root,"observation":observation,
            "artifact_path":HOME,"view":"whole-iiif-service",
            "operator_authorization_ref":"operator-directive:media-test",
            "maker":{"maker_type":"model","agent_ref":"agent:media-test"},
            "rights":{
                "assessment_status":"public_domain_reviewed",
                "rights_statement_uri":"https://creativecommons.org/publicdomain/mark/1.0/",
                "license_uri":null,
                "jurisdictions_reviewed":["NL","US"],
                "source_refs":["https://creativecommons.org/publicdomain/mark/1.0/"],
                "permissions":["show the image through the provider's service at every detail it serves"],
                "restrictions":["the assessment covers this image service; other images of the object carry their own"],
                "redistribution_posture":posture,
                "derivative_posture":"allowed",
                "assessed_by":{"maker_type":"model","agent_ref":"agent:media-test"},
                "rationale":"The provider marks the image Public Domain."
            }
        })
    }

    fn sha(root: &Path, reference: &str) -> String {
        Digest256::of_bytes(&fs::read(root.join(reference)).unwrap()).to_hex()
    }

    /// The checkout as the foundation rules read it, with the real contracts.
    struct Checkout(PathBuf);
    impl LayerFamilySource for Checkout {
        fn current(
            &mut self,
            path: &str,
            _: usize,
            _: Instant,
        ) -> std::result::Result<Option<Vec<u8>>, ItemRefusal> {
            Ok(fs::read(self.0.join(path)).ok())
        }
        fn recorded(
            &mut self,
            path: &str,
            digest: &str,
            _: usize,
            _: Instant,
        ) -> std::result::Result<Option<Vec<u8>>, ItemRefusal> {
            Ok(fs::read(self.0.join(path))
                .ok()
                .filter(|raw| Digest256::of_bytes(raw).to_hex() == digest))
        }
        fn schema(
            &mut self,
            _: &str,
            raw: &[u8],
            contract: &str,
            _: Instant,
        ) -> std::result::Result<bool, ItemRefusal> {
            let name = contract
                .trim_start_matches("ToS/contracts/")
                .trim_end_matches(".schema.json");
            let value: Value = serde_json::from_slice(raw).map_err(|_| ItemRefusal::Budget)?;
            Ok(validate_json(&self.0, &value, name).is_ok())
        }
        fn generation(&self) -> String {
            "media-planting-test".into()
        }
        fn checkpoint(&self, _: Instant) -> std::result::Result<(), ItemRefusal> {
            Ok(())
        }
    }

    #[test]
    fn an_observation_plants_as_evidence_and_then_as_an_image_service() {
        let repo = scratch();
        let root = repo.path();
        let observation = example("media-provider-observation.v1.rijksmuseum.json");
        let prepared = prepare_artifact(&prepare_request(root, observation.clone())).unwrap();

        let create = &prepared["artifact_create"];
        let bindings = &create["request"]["source_bindings"];
        for field in ["rights_ref", "discovery_ref", "research_ref"] {
            let reference = bindings[field]["ref"].as_str().unwrap();
            assert_eq!(bindings[field]["sha256"], sha(root, reference), "{field}");
        }
        assert_eq!(create["owner_configuration"]["source_bindings"], *bindings);
        let record = &create["request"]["record"];
        assert_eq!(
            record["artifact_id"],
            "tos.artifact.dutch-painting.amsterdam.rijksmuseum-sk-c-5"
        );
        assert_eq!(record["custody"]["inventory_numbers"], json!(["SK-C-5"]));
        assert_eq!(record["physical_description"]["height_mm"], json!(3795));
        assert_eq!(record["physical_description"]["width_mm"], json!(4535));
        assert_eq!(
            record["maker"],
            json!({"maker_type":"model","agent_ref":"agent:media-test","human_review_performed":false})
        );
        assert_eq!(
            record["provenance_event_ref"],
            "tos.event.creation.artifact-rijksmuseum-sk-c-5"
        );
        assert_eq!(
            create["request"]["forms"][0]["field_id"],
            "metadata.preferred-name"
        );

        let provenance = fs::read_to_string(root.join(PROVENANCE)).unwrap();
        let event: Value = serde_json::from_str(provenance.lines().last().unwrap()).unwrap();
        assert_eq!(event["event_id"], prepared["discovery_event_id"]);
        for output in event["outputs"].as_array().unwrap() {
            assert_eq!(output["sha256"], sha(root, output["ref"].as_str().unwrap()));
        }
        let again = prepare_artifact(&prepare_request(root, observation.clone())).unwrap_err();
        assert!(again.contains("exists"), "{again}");

        let early = plant_representation(&plant_request(root, observation.clone(), "authorized"))
            .unwrap_err();
        assert!(early.contains("plant the Artifact first"), "{early}");
        // The native owner creates the Artifact from the returned request.
        let witness = root.join(format!("{ARTIFACTS}/{HOME}/artifact-witness.json"));
        publish_json(&witness, record).unwrap();

        let refused =
            plant_representation(&plant_request(root, observation.clone(), "metadata_only"))
                .unwrap_err();
        assert!(
            refused.contains("authorized redistribution posture"),
            "{refused}"
        );
        let planted =
            plant_representation(&plant_request(root, observation.clone(), "authorized")).unwrap();
        assert_eq!(
            planted["representation_id"],
            image_service_id("https://iiif.micr.io/PJEZO")
        );
        let representation_ref = planted["written"][0].as_str().unwrap();
        let representation: Value =
            serde_json::from_slice(&fs::read(root.join(representation_ref)).unwrap()).unwrap();
        assert_eq!(representation["discovery_ref"], record["discovery_ref"]);
        assert_eq!(representation["image_service"]["width_pixels"], 14645);

        // The foundation layer rules accept what was planted.
        let mut rules = LayerFamilyRules::new(ItemLimits {
            max_member_bytes: 2_097_152,
            max_total_bytes: 64_000_000,
            max_state_bytes: 64_000_000,
            max_issues: 1000,
            deadline: Instant::now() + Duration::from_secs(30),
        });
        let mut checkout = Checkout(root.to_path_buf());
        rules.inspect(&mut checkout, representation_ref).unwrap();
        rules
            .inspect(
                &mut checkout,
                &format!("{ARTIFACTS}/{HOME}/artifact-witness.json"),
            )
            .unwrap();
        let report = rules.finish();
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        // The Artifact's creation is verified against the native creation
        // event, which only the owner's run produces.
        let gaps: Vec<&str> = report
            .unsupported
            .iter()
            .map(|gap| gap.profile.as_str())
            .collect();
        assert_eq!(
            gaps,
            ["native-artifact-retained-creation-software-and-event-verification"]
        );

        let twice =
            plant_representation(&plant_request(root, observation, "authorized")).unwrap_err();
        assert!(twice.contains("exists"), "{twice}");
    }

    #[test]
    fn a_file_image_waits_for_the_payload_route() {
        let repo = scratch();
        let root = repo.path();
        let observation = example("media-provider-observation.v1.met.json");
        let error =
            plant_representation(&plant_request(root, observation, "authorized")).unwrap_err();
        assert!(error.contains("payload route"), "{error}");
    }

    #[test]
    fn artifact_homes_are_three_slugs_of_physical_identity() {
        assert!(ArtifactHome::parse(HOME).is_ok());
        for bad in [
            "dutch-painting/amsterdam",
            "Dutch/amsterdam/x",
            "a/b/c/d",
            "a/cdli/x",
            "a/b/../c",
            "a/b/x--y",
        ] {
            assert!(ArtifactHome::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn centimetres_become_millimetres_as_written() {
        assert_eq!(millimetres("379.5"), Some(json!(3795)));
        assert_eq!(millimetres("34.9"), Some(json!(349)));
        assert_eq!(millimetres("12"), Some(json!(120)));
        assert_eq!(millimetres("0.25"), Some(json!(2.5)));
        assert_eq!(millimetres("7.125"), Some(json!(71.25)));
        assert_eq!(millimetres("0"), None);
        assert_eq!(millimetres("1,5"), None);
    }
}
