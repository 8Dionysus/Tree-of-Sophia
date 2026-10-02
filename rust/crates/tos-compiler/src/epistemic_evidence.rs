//! Public Evidence Lens projection. Source, review, rights and canon retain authority.
//! Only requested identities are retained while selected graph collections stream.
use crate::{
    Error, PublicCaptureLimits, Result,
    d1_public_capture::{PublicCapture, compact, json, source_digest},
    safe_open,
};
use serde_json::{Value, json as value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Component, Path},
    sync::atomic::AtomicBool,
    time::Instant,
};
use tos_foundation::Digest256;
use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};

pub const SOURCE_REF: &str = "ToS/philosophy/graph-workbench/views/evidence-lens-scenes.v1.json";
pub const SCHEMA_REF: &str = "ToS/contracts/epistemic-evidence-projection.schema.json";
pub const PROJECTION_REF: &str = "ToS/derived-exports/epistemic_evidence_projection.min.json";
pub const CANON_REF: &str =
    "ToS/canon/relations/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/edges.csv";
const CAP: usize = 1024 * 1024;

fn err(message: impl Into<String>) -> Error {
    Error::Source(message.into())
}
fn decode(raw: &[u8]) -> Result<Value> {
    let parsed = json(raw, CAP)?;
    serde_json::from_slice(&compact(&parsed, CAP)?).map_err(|e| err(e.to_string()))
}
fn array<'a>(v: &'a Value, field: &str) -> Result<&'a [Value]> {
    match v.get(field) {
        None => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        _ => Err(err(format!("Evidence Lens {field} must be an array"))),
    }
}
fn string(v: &Value) -> Result<&str> {
    v.as_str().ok_or(Error::Invalid(
        "Evidence Lens identity/ref must be a string",
    ))
}
fn guard(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(Error::Budget("Evidence Lens deadline"))
    } else {
        Ok(())
    }
}
fn reference_path(root: &Path, reference: &str) -> Result<std::path::PathBuf> {
    let p = Path::new(reference);
    if reference.is_empty()
        || p.is_absolute()
        || reference.contains("/payload/")
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(err(format!(
            "unsafe Evidence Lens route ref: {reference:?}"
        )));
    }
    Ok(root.join(p))
}
fn read(root: &Path, reference: &str, deadline: Instant) -> Result<Vec<u8>> {
    guard(deadline)?;
    let mut file = safe_open::open_regular(&reference_path(root, reference)?, CAP as u64)?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)?;
    guard(deadline)?;
    Ok(raw)
}
/// Routes contribute only byte identity; never retain or parse their body.
fn route_digest(
    root: &Path,
    reference: &str,
    capture: &PublicCapture,
    limits: PublicCaptureLimits,
    deadline: Instant,
) -> Result<Digest256> {
    guard(deadline)?;
    let mut file =
        safe_open::open_regular(&reference_path(root, reference)?, limits.max_input_bytes)?;
    let (digest, _) = source_digest(&mut file, limits.max_input_bytes, |bytes| {
        capture.charge_work(bytes as u64)
    })?;
    guard(deadline)?;
    Ok(digest)
}
fn rendered(v: &Value, deadline: Instant) -> Result<Vec<u8>> {
    guard(deadline)?;
    let raw = serde_json::to_vec(v).map_err(|e| err(e.to_string()))?;
    let mut result = compact(&json(&raw, CAP)?, CAP)?;
    result.push(b'\n');
    guard(deadline)?;
    Ok(result)
}

pub fn validate_payload(root: &Path, payload: &Value, deadline: Instant) -> Result<()> {
    guard(deadline)?;
    let raw = read(root, SCHEMA_REF, deadline)?;
    let schema = decode(&raw)?;
    let uri = string(&schema["$id"])?;
    let backend = SchemaBackendProbe::new(
        [SchemaResource {
            uri: uri.to_owned(),
            raw,
        }],
        FormatProfile::LegacyPythonObserved20260923,
    )
    .map_err(|e| err(format!("Evidence Lens schema: {e:?}")))?;
    if !backend
        .is_valid_raw(uri, &rendered(payload, deadline)?)
        .map_err(|e| err(format!("Evidence Lens schema: {e:?}")))?
    {
        return Err(Error::Invalid("Evidence Lens schema violation"));
    }
    guard(deadline)?;
    let mut ids = BTreeSet::new();
    for scene in array(payload, "scenes")? {
        if scene["posture"] == "contested-pre-canon" && scene["conclusion"]["can_conclude"] == true
        {
            return Err(Error::Invalid(
                "contested pre-canon scenes cannot be conclusive",
            ));
        }
        if scene["conclusion"]["claim_evidence_closed"] == true {
            return Err(Error::Invalid(
                "v1 Evidence Lens scenes must not assert claim/evidence closure",
            ));
        }
        for id in array(scene, "selection_ids")? {
            if !ids.insert(string(id)?) {
                return Err(Error::Invalid("selection IDs must bind to one scene only"));
            }
        }
    }
    guard(deadline)
}

/// Captures selected collections to caller-owned fresh staging, then rechecks all
/// opened inputs before returning bytes. Never writes a source or public output.
pub fn build(
    root: &Path,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: Instant,
) -> Result<Vec<u8>> {
    let source_raw = read(root, SOURCE_REF, deadline)?;
    let source = decode(&source_raw)?;
    guard(deadline)?;
    let scenes = array(&source, "scenes")?;
    let mut requested: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for scene in scenes {
        if !scene.is_object() {
            return Err(Error::Invalid("Evidence Lens scenes must be objects"));
        }
        for selection in array(scene, "selections")? {
            if !selection.is_object() {
                return Err(Error::Invalid("Evidence Lens selections must be objects"));
            }
            let key = (
                string(&selection["mode"])?.to_owned(),
                string(&selection["view_id"])?.to_owned(),
            );
            for id in array(selection, "item_ids")? {
                requested
                    .entry(key.clone())
                    .or_default()
                    .insert(string(id)?.to_owned());
            }
            requested.entry(key).or_default();
        }
    }
    let capture = PublicCapture::create_evidence(root, staging, limits, deadline)?;
    let mut found: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    capture.visit_rows("evidence-philosophy", "views", |_, raw| {
        let view = decode(raw)?;
        let Some(id) = view["view_id"].as_str() else {
            return Ok(());
        };
        let key = ("philosophy".into(), id.to_owned());
        let Some(wanted) = requested.get(&key) else {
            return Ok(());
        };
        // Later duplicate view IDs replace the earlier membership, matching donor.
        let mut matched = BTreeSet::new();
        for field in ["node_ids", "edge_ids"] {
            for item in array(&view, field)? {
                if let Some(id) = item.as_str() {
                    if wanted.contains(id) {
                        matched.insert(id.to_owned());
                    }
                }
            }
        }
        for field in ["nodes", "edges"] {
            for item in array(&view, field)? {
                let identity = if crate::source_philosophy_support::truth(&item["node_id"]) {
                    &item["node_id"]
                } else {
                    &item["edge_id"]
                };
                let id = identity.as_str();
                if let Some(id) = id {
                    if wanted.contains(id) {
                        matched.insert(id.to_owned());
                    }
                }
            }
        }
        found.insert(key, matched);
        Ok(())
    })?;
    let corpus_key = ("corpus".into(), "route-graph".into());
    found.insert(corpus_key.clone(), BTreeSet::new());
    for collection in ["nodes", "relation_edges"] {
        capture.visit_rows("evidence-corpus", collection, |_, raw| {
            let row = decode(raw)?;
            if collection == "relation_edges" && row["owner_branch"] != "ToS/canon" {
                return Ok(());
            }
            let field = if collection == "nodes" {
                "node_id"
            } else {
                "edge_id"
            };
            if let (Some(id), Some(wanted)) = (row[field].as_str(), requested.get(&corpus_key)) {
                if wanted.contains(id) {
                    found.get_mut(&corpus_key).unwrap().insert(id.to_owned());
                }
            }
            Ok(())
        })?;
    }
    for (key, wanted) in &requested {
        let ids = found
            .get(key)
            .ok_or_else(|| err(format!("unknown Evidence Lens view: {}/{}", key.0, key.1)))?;
        let missing: Vec<_> = wanted.difference(ids).collect();
        if !missing.is_empty() {
            return Err(err(format!(
                "Evidence Lens selection IDs not in {key:?}: {missing:?}"
            )));
        }
    }
    let canon_raw = read(root, CANON_REF, deadline)?;
    let mut header = Vec::new();
    let mut anchors = BTreeMap::new();
    let l = crate::knowledge_canon_source::CanonSourceLimits {
        max_manifest_members: 1,
        max_selected_members: 1,
        max_nodes: 1,
        max_packs: 1,
        max_edges: 65536,
        max_source_bytes: CAP,
        max_raw_row_bytes: CAP,
        max_csv_fields: 1024,
        max_csv_record_bytes: CAP,
        max_forms: 1,
        max_forms_output_bytes: 1,
        max_page_rows: 1,
        max_page_bytes: 1,
        max_work_bytes: limits.max_work_bytes,
    };
    crate::knowledge_canon_source::csv_records(
        &canon_raw,
        l,
        deadline,
        &AtomicBool::new(false),
        |cells| {
            if cells.is_empty() {
                return Ok(());
            }
            if header.is_empty() {
                header = cells;
                return Ok(());
            }
            let row: BTreeMap<_, _> = header.iter().cloned().zip(cells).collect();
            let id = row
                .get("edge_id")
                .ok_or(Error::Invalid("canonical CSV edge_id header"))?;
            anchors.insert(id.clone(), row);
            Ok(())
        },
    )?;
    let mut opened = BTreeMap::from([
        (SOURCE_REF.to_owned(), Digest256::of_bytes(&source_raw)),
        (CANON_REF.to_owned(), Digest256::of_bytes(&canon_raw)),
        (
            SCHEMA_REF.to_owned(),
            Digest256::of_bytes(&read(root, SCHEMA_REF, deadline)?),
        ),
    ]);
    capture.charge_work((source_raw.len() + canon_raw.len()) as u64)?;
    let mut output = Vec::new();
    for raw_scene in scenes {
        let mut scene = raw_scene.as_object().unwrap().clone();
        scene.remove("anchor_edge_ids");
        let mut selection_ids = Vec::new();
        let mut seen = BTreeSet::new();
        for selection in array(raw_scene, "selections")? {
            for id in array(selection, "item_ids")? {
                if seen.insert(string(id)?.to_owned()) {
                    selection_ids.push(id.clone());
                }
            }
        }
        let mut routes = Vec::new();
        let mut refs = BTreeSet::from([SOURCE_REF.to_owned()]);
        for raw_route in array(raw_scene, "routes")? {
            let mut route = raw_route
                .as_object()
                .ok_or(Error::Invalid("Evidence Lens routes must be objects"))?
                .clone();
            let reference = string(&raw_route["ref"])?;
            let digest = route_digest(root, reference, &capture, limits, deadline)?;
            route.insert("exists".into(), Value::Bool(true));
            route.insert("sha256".into(), Value::String(digest.to_hex()));
            if let Some(old) = opened.insert(reference.to_owned(), digest) {
                if old != digest {
                    return Err(Error::Invalid("Evidence Lens source changed during build"));
                }
            }
            refs.insert(reference.to_owned());
            routes.push(Value::Object(route));
        }
        let mut source_anchors = Vec::new();
        for id in array(raw_scene, "anchor_edge_ids")? {
            let edge = crate::source_philosophy_support::string(id);
            let row = anchors
                .get(&edge)
                .ok_or_else(|| err(format!("unknown canonical anchor edge: {edge}")))?;
            let segments: Vec<_> = row
                .get("anchor_segment_ids")
                .map(String::as_str)
                .unwrap_or("")
                .split('|')
                .filter(|v| !v.is_empty())
                .collect();
            if segments.is_empty() {
                return Err(err(format!(
                    "canonical edge has no segment anchors: {edge}"
                )));
            }
            source_anchors.push(value!({"edge_id":edge,"anchor_segment_ids":segments,"witness_scope":row.get("witness_scope").map(String::as_str).unwrap_or(""),"relation_ref":CANON_REF}));
        }
        scene.insert("selection_ids".into(), Value::Array(selection_ids));
        scene.insert("source_anchors".into(), Value::Array(source_anchors));
        scene.insert("routes".into(), Value::Array(routes));
        scene.insert("source_refs".into(), value!(refs));
        output.push(Value::Object(scene));
    }
    let payload = value!({"schema_version":"tos_epistemic_evidence_projection_v1","owner_repo":"Tree-of-Sophia","surface_kind":"derived_public_evidence_navigation","source_definition_ref":SOURCE_REF,"source_definition_sha256":opened[SOURCE_REF].to_hex(),"scenes":output,"authority_boundary":{"is_source":false,"is_canon":false,"is_semantic_truth":false,"is_rights_clearance":false,"note":"This projection joins explicit owner routes for inspection. The referenced source, review, canon, and rights surfaces retain authority."}});
    validate_payload(root, &payload, deadline)?;
    capture.verify_inputs(limits)?;
    for (reference, digest) in opened {
        if route_digest(root, &reference, &capture, limits, deadline)? != digest {
            return Err(Error::Invalid("Evidence Lens source changed during build"));
        }
    }
    guard(deadline)?;
    rendered(&payload, deadline)
}

/// Read-only parity check shared by maintained check and validator commands.
pub fn check(
    root: &Path,
    staging: &Path,
    limits: PublicCaptureLimits,
    deadline: Instant,
) -> Result<()> {
    let expected = build(root, staging, limits, deadline)?;
    if read(root, PROJECTION_REF, deadline)? != expected {
        return Err(err(format!("{PROJECTION_REF} is out of date")));
    }
    guard(deadline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn put(root: &Path, reference: &str, raw: &[u8]) {
        let path = root.join(reference);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, raw).unwrap();
    }
    fn limits() -> PublicCaptureLimits {
        PublicCaptureLimits {
            max_input_bytes: 16 * 1024 * 1024,
            max_rows: 1000,
            max_staging_bytes: 16 * 1024 * 1024,
            max_work_bytes: 64 * 1024 * 1024,
            max_sql_vm_steps: 1_000_000,
            sqlite_cache_kib: 256,
        }
    }
    fn fixture(root: &Path) -> Value {
        put(
            root,
            SCHEMA_REF,
            include_bytes!("../../../../ToS/contracts/epistemic-evidence-projection.schema.json"),
        );
        put(
            root,
            CANON_REF,
            b"edge_id,anchor_segment_ids,witness_scope\r\ne,seg-a|seg-b,\"scope, quoted\"\r\n",
        );
        let mut witness = "Источник Ω\n".as_bytes().to_vec();
        witness.resize(1_598_518, b'x');
        put(root, "docs/witness.md", &witness);
        let corpus = value!({"schema_version":"tos_corpus_index_v1","nodes":[{"node_id":"n"}],"relation_edges":[{"edge_id":"e","owner_branch":"ToS/canon"}],"source_navigation":"x".repeat(3*1024*1024)});
        put(
            root,
            "ToS/derived-exports/tos_corpus_index.min.json",
            &serde_json::to_vec(&corpus).unwrap(),
        );
        put(root,"ToS/derived-exports/philosophy_graph_projection.min.json",br#"{"schema_version":"tos_philosophy_graph_projection_v2","views":[{"view_id":"v","node_ids":["p"]}]}"#);
        value!({"scenes":[{"scene_id":"one","selections":[{"mode":"corpus","view_id":"route-graph","item_ids":["n","e"]},{"mode":"philosophy","view_id":"v","item_ids":["p"]}],"posture":"contested-pre-canon","finding":"open","finding_ru":"Открыто Ω","conclusion":{"can_conclude":false,"canon_membership":false,"claim_evidence_closed":false,"allowed":["inspect"],"not_allowed":["closure"]},"anchor_edge_ids":["e"],"routes":[{"route_kind":"witness","ref":"docs/witness.md","status":"open"}],"gaps":[],"gaps_ru":[]}]})
    }
    fn write_source(root: &Path, source: &Value) {
        put(root, SOURCE_REF, &serde_json::to_vec(source).unwrap());
    }
    fn candidate(root: &Path, name: &str) -> Result<Vec<u8>> {
        build(
            root,
            &root.join(name),
            limits(),
            Instant::now() + std::time::Duration::from_secs(20),
        )
    }
    #[test]
    fn scene_authority_identity_anchor_and_route_fences() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut source = fixture(root);
        write_source(root, &source);
        let raw = candidate(root, "positive.sqlite").unwrap();
        let payload: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(
            payload["scenes"][0]["selection_ids"],
            value!(["n", "e", "p"])
        );
        assert_eq!(
            payload["scenes"][0]["source_anchors"][0]["anchor_segment_ids"],
            value!(["seg-a", "seg-b"])
        );
        assert_eq!(
            payload["scenes"][0]["source_anchors"][0]["witness_scope"],
            "scope, quoted"
        );
        assert_eq!(payload["scenes"][0]["finding_ru"], "Открыто Ω");
        assert_eq!(payload["authority_boundary"]["is_source"], false);
        put(root, PROJECTION_REF, &raw);
        check(
            root,
            &root.join("check.sqlite"),
            limits(),
            Instant::now() + std::time::Duration::from_secs(20),
        )
        .unwrap();
        source["scenes"][0]["conclusion"]["can_conclude"] = Value::Bool(true);
        write_source(root, &source);
        assert!(candidate(root, "conclusive.sqlite").is_err());
        source["scenes"][0]["conclusion"]["can_conclude"] = Value::Bool(false);
        source["scenes"][0]["routes"][0]["ref"] =
            value!("ToS/source-witnesses/items/x/payload/private.txt");
        write_source(root, &source);
        assert!(candidate(root, "private.sqlite").is_err());
        source["scenes"][0]["routes"][0]["ref"] = value!("../outside.txt");
        write_source(root, &source);
        assert!(candidate(root, "escape.sqlite").is_err());
        source["scenes"][0]["routes"][0]["ref"] = value!("docs/witness.md");
        source["scenes"][0]["selections"][0]["item_ids"] = value!(["absent"]);
        write_source(root, &source);
        assert!(candidate(root, "unknown.sqlite").is_err());
        assert!(build(root, &root.join("expired.sqlite"), limits(), Instant::now()).is_err());
        assert!(!root.join("expired.sqlite").exists());
    }
}
