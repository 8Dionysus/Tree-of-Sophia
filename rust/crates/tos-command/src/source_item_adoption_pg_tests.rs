//! PG owner custody boundary regressions. These exercise retained/publication
//! bindings only; provider parsing, schema execution and real PG commit/replay
//! require the separate admitted integration run.
use super::*;
use serde_json::{Value, json};

struct Fixture {
    configuration: JsonValue,
    request: JsonValue,
    stage: Value,
    child: BTreeMap<String, Vec<u8>>,
}

fn source(value: &Value) -> JsonValue {
    super::super::foundation_value(value).unwrap()
}

fn raw(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn fixture(media_type: &str, profile: &str) -> Fixture {
    // Distinct layers deliberately carry the same observation. No provider
    // output is treated as bibliographic, rights or textual admission here.
    let configuration = json!({
        "item_id":"tos.item.pg-inventory-custody",
        "file_id":"tos.file.pg-inventory-custody",
        "payload_basename":"synthetic.bin",
        "original_basename":"synthetic.bin",
        "media_type":media_type,
        "byte_size":16,
        "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "inventory_event_id":"tos.event.pg-inventory-custody"
    });
    let observation = json!({
        "file_id":configuration["file_id"],
        "file_sha256":configuration["sha256"],
        "media_type":media_type,
        "profile":profile,
        "summary":{"resource_count":1}
    });
    let rights = json!({
        "rights_id":"tos.rights.pg-inventory-custody",
        "visibility":"local_only",
        "review_status":"unreviewed",
        "assessment_status":"not_assessed",
        "redistribution_posture":"not_authorized",
        "derivative_posture":"local_research_only",
        "permissions":[]
    });
    let request = json!({
        "inventory":observation,
        "inventory_limitation":null,
        "rights":rights
    });
    let stage = json!({
        "schema_version":"tos_item_deposit_stage_v1",
        "transaction_id":Digest256::of_bytes(b"synthetic PG custody transaction").to_prefixed(),
        "binding":{"configuration":configuration,"request":request},
        "observation":{"inventory":observation,"limitation":null},
        "state":"deposited",
        "created_at":"2026-10-23T12:00:01Z",
        "observation_interval":{
            "started_at":"2026-10-23T12:00:00Z",
            "ended_at":"2026-10-23T12:00:01Z"
        },
        "deposited_at":"2026-10-23T12:00:02Z",
        "recovery_authorization":null
    });
    let inventory = json!({
        "item_id":configuration["item_id"],
        "files":[observation],
        "provenance_event_ref":configuration["inventory_event_id"],
        "inventory_authority":"mechanical_metadata_only",
        "source_text_included":false
    });
    let child = BTreeMap::from([
        ("resource-inventory.json".into(), raw(&inventory)),
        ("source-create-request.json".into(), raw(&request)),
        ("rights.json".into(), raw(&rights)),
        (
            "item-deposit-receipt.json".into(),
            raw(&crate::source_item_deposit::public_receipt(&stage).unwrap()),
        ),
    ]);
    Fixture {
        configuration: source(&configuration),
        request: source(&request),
        stage,
        child,
    }
}

fn verify(fixture: &Fixture) -> SourceCommandResult<()> {
    published_inventory_binding(
        &fixture.configuration,
        &fixture.request,
        &fixture.child,
        &fixture.stage,
    )
}

#[test]
fn pg_inventory_publication_preserves_file_inventory_profiles() {
    for (media_type, profile) in [
        ("application/pdf", "pdf_pages_v1"),
        ("image/vnd.djvu", "djvu_pages_v1"),
        ("application/tei+xml", "tei_structure_v1"),
        ("text/plain", "plain_utf8_file_v1"),
    ] {
        let fixture = fixture(media_type, profile);
        verify(&fixture).unwrap();
        verify(&fixture).expect("reconstructing the same retained observation is repeatable");
        let deposit: Value =
            serde_json::from_slice(fixture.child.get("item-deposit-receipt.json").unwrap())
                .unwrap();
        assert_eq!(deposit["metadata_committed"], false);
        assert_eq!(deposit["grants_admission"], false);
    }
}

#[test]
fn pg_inventory_publication_accepts_equivalent_prepared_object_order() {
    let mut fixture = fixture("application/pdf", "pdf_pages_v1");
    let mut observation = cmd::field(&fixture.request, "inventory")
        .unwrap()
        .as_object()
        .unwrap()
        .to_vec();
    observation.reverse();
    cmd::set(
        &mut fixture.request,
        "inventory",
        JsonValue::Object(observation),
    )
    .unwrap();
    verify(&fixture).expect("canonical prepared request is independent of JSON member order");
}

#[test]
fn pg_inventory_recovery_rejects_retained_request_and_observation_substitution() {
    for pointer in [
        "/binding/request/inventory/profile",
        "/observation/inventory/file_id",
        "/observation/inventory/file_sha256",
        "/observation/inventory/media_type",
        "/observation/limitation",
    ] {
        let mut fixture = fixture("application/pdf", "pdf_pages_v1");
        *fixture.stage.pointer_mut(pointer).unwrap() = json!("substituted");
        assert!(matches!(
            retained_inventory_binding(&fixture.request, &fixture.stage),
            Err(SourceCommandError::Conflict(
                "PG Item retained inventory differs from prepared request"
            ))
        ));
    }
}

#[test]
fn pg_inventory_publication_rejects_consumer_rights_provenance_and_receipt_drift() {
    for (member, pointer, value) in [
        (
            "resource-inventory.json",
            "/files/0/profile",
            json!("substituted"),
        ),
        ("resource-inventory.json", "/files", json!([])),
        (
            "resource-inventory.json",
            "/item_id",
            json!("tos.item.other"),
        ),
        (
            "resource-inventory.json",
            "/provenance_event_ref",
            json!("tos.event.other"),
        ),
        (
            "resource-inventory.json",
            "/source_text_included",
            json!(true),
        ),
        (
            "resource-inventory.json",
            "/inventory_authority",
            json!("admission"),
        ),
        (
            "rights.json",
            "/redistribution_posture",
            json!("authorized"),
        ),
        (
            "source-create-request.json",
            "/inventory/profile",
            json!("substituted"),
        ),
        (
            "item-deposit-receipt.json",
            "/private_stage_digest",
            json!("substituted"),
        ),
    ] {
        let mut fixture = fixture("application/pdf", "pdf_pages_v1");
        let mut changed: Value =
            serde_json::from_slice(fixture.child.get(member).unwrap()).unwrap();
        *changed.pointer_mut(pointer).unwrap() = value;
        fixture.child.insert(member.into(), raw(&changed));
        assert!(
            matches!(
                verify(&fixture),
                Err(SourceCommandError::Conflict(
                    "PG Item published inventory differs from retained custody"
                ))
            ),
            "consumer mutation accepted: {member}{pointer}"
        );
    }
}

#[test]
fn pg_inventory_publication_requires_each_exact_custody_member() {
    for member in [
        "resource-inventory.json",
        "source-create-request.json",
        "rights.json",
        "item-deposit-receipt.json",
    ] {
        let mut fixture = fixture("application/pdf", "pdf_pages_v1");
        fixture.child.remove(member).unwrap();
        assert!(matches!(
            verify(&fixture),
            Err(SourceCommandError::Conflict(
                "PG Item inventory custody member absent"
            ))
        ));
    }
}

#[test]
fn pg_inventory_unavailable_retains_custody_without_publishing_metadata() {
    let mut fixture = fixture("application/octet-stream", "unavailable");
    let mut request = super::super::serde_value(&fixture.request).unwrap();
    request["inventory"] = Value::Null;
    request["inventory_limitation"] = json!("inventory-unavailable:InventoryBuildError");
    fixture.request = source(&request);
    fixture.stage["binding"]["request"] = request;
    fixture.stage["observation"]["inventory"] = Value::Null;
    fixture.stage["observation"]["limitation"] = json!("inventory-unavailable:InventoryBuildError");
    retained_inventory_binding(&fixture.request, &fixture.stage).unwrap();
    assert!(matches!(
        verify(&fixture),
        Err(SourceCommandError::Conflict(
            "PG Item published inventory differs from retained custody"
        ))
    ));
}
