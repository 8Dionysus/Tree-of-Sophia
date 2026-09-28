//! Finite static companions of the disposable public v9 build. Every graph
//! packet is assembled from the captured row cursor; no full projection is
//! reconstructed in process memory.

use crate::{
    Error, Result,
    d1_public_capture::{MAX_ROW_BYTES, PublicCapture},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tos_foundation::{Digest256, python_casefold_unicode16_v1};

const FILE_CAP: u64 = 25 * 1024 * 1024 - 1;
const CORPUS_LIMITS: [usize; 4] = [1, 100, 700, 1000];
const PHILOSOPHY_LIMITS: [usize; 2] = [1, 1000];
const HEADERS: &str = "/*\n  Content-Security-Policy: default-src 'self'; base-uri 'none'; connect-src 'self'; font-src 'self'; form-action 'self'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self'; worker-src 'self'\n  Permissions-Policy: tools=(self), accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=()\n  Cross-Origin-Opener-Policy: same-origin\n  Cross-Origin-Embedder-Policy: require-corp\n  Cross-Origin-Resource-Policy: same-origin\n  Origin-Agent-Cluster: ?1\n  Referrer-Policy: no-referrer\n  X-Content-Type-Options: nosniff\n  X-Frame-Options: DENY\n\n/static/*\n  Cache-Control: public, max-age=31536000, immutable\n\n/__edge/*\n  Cache-Control: public, max-age=31536000, immutable\n";

pub(crate) struct StaticSummary {
    pub(crate) default_corpus_view: String,
    pub(crate) default_philosophy_view: String,
    pub(crate) bytes: u64,
    web_dist: PathBuf,
    web_inputs: Vec<(PathBuf, Digest256, u64)>,
}

impl StaticSummary {
    pub(crate) fn verify_web_inputs(&self, capture: &PublicCapture) -> Result<()> {
        let current = web_paths(&self.web_dist)?;
        if current.len() != self.web_inputs.len()
            || current
                .iter()
                .zip(&self.web_inputs)
                .any(|((path, _), (expected, _, _))| path != expected)
        {
            return Err(Error::Invalid("public D1 web asset membership changed"));
        }
        for (path, expected, len) in &self.web_inputs {
            let raw = bounded_file(path, FILE_CAP, capture)?;
            if raw.len() as u64 != *len || Digest256::of_bytes(&raw) != *expected {
                return Err(Error::Invalid("public D1 web asset changed"));
            }
        }
        Ok(())
    }
}

struct Writer<'a> {
    root: &'a Path,
    capture: &'a PublicCapture,
    max: u64,
    bytes: u64,
}
impl Writer<'_> {
    fn put(&mut self, relative: &str, raw: &[u8]) -> Result<()> {
        let target = self.root.join(relative);
        if relative.is_empty()
            || relative
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || raw.len() as u64 >= 25 * 1024 * 1024
        {
            return Err(Error::Budget("public D1 static path/file"));
        }
        self.bytes = self
            .bytes
            .checked_add(raw.len() as u64)
            .filter(|size| *size <= self.max)
            .ok_or(Error::Budget("public D1 static output bytes"))?;
        self.capture.charge_work(raw.len() as u64)?;
        fs::create_dir_all(
            target
                .parent()
                .ok_or(Error::Invalid("public D1 static parent"))?,
        )?;
        let mut file = File::create(target)?;
        file.write_all(raw)?;
        file.sync_all()?;
        Ok(())
    }
    fn json(&mut self, relative: &str, value: &Value) -> Result<()> {
        let mut raw = serde_json::to_vec(value).map_err(|e| Error::Source(e.to_string()))?;
        raw.push(b'\n');
        self.put(relative, &raw)
    }
}

fn bounded_file(path: &Path, cap: u64, capture: &PublicCapture) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > cap {
        return Err(Error::Invalid("public D1 unsafe/oversized static input"));
    }
    let mut raw = Vec::with_capacity(metadata.len() as usize);
    File::open(path)?.take(cap + 1).read_to_end(&mut raw)?;
    capture.charge_work(raw.len() as u64)?;
    if raw.len() as u64 != metadata.len() {
        return Err(Error::Invalid("public D1 static input changed"));
    }
    Ok(raw)
}

fn portable(value: &mut Value, root: &str) {
    match value {
        Value::String(text) => {
            if text == root {
                *text = "Tree-of-Sophia".to_owned();
            } else if let Some(suffix) = text
                .strip_prefix(root)
                .and_then(|rest| rest.strip_prefix('/'))
            {
                *text = suffix.to_owned();
            }
        }
        Value::Array(items) => {
            for item in items {
                portable(item, root);
            }
        }
        Value::Object(fields) => {
            for item in fields.values_mut() {
                portable(item, root);
            }
        }
        _ => {}
    }
}

fn rows(
    capture: &PublicCapture,
    role: &str,
    collection: &str,
    root: &str,
    mut use_row: impl FnMut(Value) -> Result<()>,
) -> Result<u64> {
    capture.visit_rows(role, collection, |_, raw| {
        if raw.len() > MAX_ROW_BYTES {
            return Err(Error::Budget("public D1 static row bytes"));
        }
        capture.charge_work(raw.len() as u64)?;
        let mut value: Value =
            serde_json::from_slice(raw).map_err(|e| Error::Source(e.to_string()))?;
        portable(&mut value, root);
        use_row(value)
    })
}

fn strings(value: &Value, field: &str) -> BTreeSet<String> {
    value[field]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
fn ordered_strings(value: &Value, field: &str) -> Vec<String> {
    value[field]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}
fn id<'a>(value: &'a Value, field: &str) -> &'a str {
    value[field].as_str().unwrap_or("")
}
fn array(value: &Value, field: &str) -> Vec<Value> {
    value[field].as_array().cloned().unwrap_or_default()
}
fn supported(view: &Value) -> bool {
    matches!(
        id(view, "view_id"),
        "corpus-topology" | "route-graph" | "promotion-flow"
    )
}

fn web_paths(dist: &Path) -> Result<Vec<(PathBuf, String)>> {
    if !dist.join("index.html").is_file() || dist.is_symlink() {
        return Err(Error::Invalid("public D1 web dist missing/unsafe"));
    }
    let mut paths = Vec::new();
    let mut pending = vec![(dist.clone(), String::new())];
    while let Some((directory, prefix)) = pending.pop() {
        let metadata = fs::symlink_metadata(&directory)?;
        if !metadata.file_type().is_dir() {
            return Err(Error::Invalid("public D1 web directory"));
        }
        let mut entries = fs::read_dir(&directory)?
            .map(|entry| entry.map(|item| item.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        for path in entries.into_iter().rev() {
            let name = path
                .file_name()
                .and_then(|part| part.to_str())
                .ok_or(Error::Invalid("public D1 web filename"))?;
            if name == "." || name == ".." {
                return Err(Error::Invalid("public D1 web filename"));
            }
            let relative = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}/{name}")
            };
            let kind = fs::symlink_metadata(&path)?.file_type();
            if kind.is_dir() {
                pending.push((path, relative));
            } else if kind.is_file() {
                paths.push((path, relative));
            } else {
                return Err(Error::Invalid("public D1 web file type"));
            }
            if paths.len() + pending.len() > 65_536 {
                return Err(Error::Budget("public D1 web entries"));
            }
        }
    }
    paths.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(paths)
}

fn web_assets(writer: &mut Writer<'_>, root: &Path) -> Result<Vec<(PathBuf, Digest256, u64)>> {
    for part in ["access", "access/web", "access/web/dist"] {
        if !fs::symlink_metadata(root.join(part))?.file_type().is_dir() {
            return Err(Error::Invalid("public D1 web ancestor"));
        }
    }
    let dist = root.join("access/web/dist");
    let paths = web_paths(&dist)?;
    let mut manifest = Vec::with_capacity(paths.len());
    let mut index = None;
    for (path, relative) in paths {
        let raw = bounded_file(&path, FILE_CAP, writer.capture)?;
        let digest = Digest256::of_bytes(&raw);
        manifest.push((path, digest, raw.len() as u64));
        if relative == "index.html" {
            index = Some(raw);
            continue;
        }
        let target = if let Some(suffix) = relative.strip_prefix("assets/") {
            format!("static/assets/{suffix}")
        } else {
            relative
        };
        writer.put(&target, &raw)?;
    }
    let mut index = String::from_utf8(index.ok_or(Error::Invalid("public D1 web index"))?)
        .map_err(|_| Error::Invalid("public D1 web index UTF-8"))?;
    for (path, digest, _) in &manifest {
        let relative = path
            .strip_prefix(&dist)
            .map_err(|_| Error::Invalid("public D1 web path"))?;
        let relative = relative
            .to_str()
            .ok_or(Error::Invalid("public D1 web path UTF-8"))?;
        if let Some(asset) = relative.strip_prefix("assets/") {
            let public = format!("/static/assets/{asset}");
            writer.capture.charge_work(index.len() as u64)?;
            if index.contains(&public) {
                index = index.replace(&public, &format!("{public}?v={}", &digest.to_hex()[..16]));
                writer.capture.charge_work(index.len() as u64)?;
            }
        }
    }
    writer.put("index.html", index.as_bytes())?;
    Ok(manifest)
}

fn corpus(writer: &mut Writer<'_>, header: &Value, root: &str) -> Result<String> {
    let mut views = Vec::new();
    rows(writer.capture, "corpus", "graph_views", root, |view| {
        if supported(&view) {
            views.push(view);
        }
        if views.len() > 3 {
            return Err(Error::Invalid("public D1 corpus view count"));
        }
        Ok(())
    })?;
    let status = json!({"schema":"tos_corpus_mcp_status_v1","index_exists":true,
        "tos_root":"Tree-of-Sophia","index_path":"ToS/derived-exports/tos_corpus_index.min.json",
        "owner_repo":header["owner_repo"],"surface_kind":header["surface_kind"],
        "counts":header["counts"],"graph_views":views.iter().map(|v|v["view_id"].clone()).collect::<Vec<_>>(),
        "authority_order":header["authority_order"],
        "runtime_projection_boundary":header["runtime_projection_boundary"]});
    let mut branches = Vec::new();
    rows(writer.capture, "corpus", "branches", root, |row| {
        if branches.len() >= 1000 {
            return Err(Error::Budget("public D1 static corpus branches"));
        }
        branches.push(row);
        Ok(())
    })?;
    let summary = json!({"schema":"tos_corpus_mcp_summary_v1","status":&status,
        "counts":header["counts"],"branches":&branches,"graph_views":&views,
        "runtime_projection_boundary":header["runtime_projection_boundary"],
        "authority_order":header["authority_order"]});
    writer.json("__edge/corpus/status.json", &status)?;
    writer.json("__edge/corpus/summary.json", &summary)?;
    for view in &views {
        let view_id = id(view, "view_id");
        for limit in CORPUS_LIMITS {
            let packet = corpus_view(writer.capture, header, view, &branches, root, limit)?;
            writer.json(
                &format!("__edge/corpus/graph-views/{view_id}/{limit}.json"),
                &packet,
            )?;
        }
    }
    Ok(views
        .first()
        .map(|v| id(v, "view_id").to_owned())
        .unwrap_or_default())
}

fn corpus_view(
    capture: &PublicCapture,
    header: &Value,
    view: &Value,
    branches: &[Value],
    root: &str,
    limit: usize,
) -> Result<Value> {
    let view_id = id(view, "view_id");
    let mut items = Vec::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    if view_id == "corpus-topology" {
        let root_id = format!("view:{view_id}");
        items = branches.iter().take(limit).cloned().collect();
        nodes.push(json!({"node_id":root_id.clone(),"label":view["title"],"node_type":"corpus-root","source_ref":view["entry_surface"]}));
        for branch in branches.iter().take(limit) {
            let branch_id = id(branch, "id");
            if branch_id.is_empty() {
                continue;
            }
            let source = branch
                .get("owner_surface")
                .filter(|v| !v.is_null())
                .unwrap_or(&branch["path"]);
            let mut item = branch.clone();
            let map = item
                .as_object_mut()
                .ok_or(Error::Invalid("public D1 branch"))?;
            map.insert("node_id".into(), json!(branch_id));
            map.insert("label".into(), json!(branch_id));
            map.insert("node_type".into(), json!("corpus-branch"));
            map.insert("source_ref".into(), source.clone());
            nodes.push(item);
            edges.push(
                json!({"edge_id":format!("corpus-edge:{root_id}:{branch_id}"),"from_id":root_id.clone(),
                "to_id":branch_id,"predicate_id":"contains","source_ref":source}),
            );
        }
    } else {
        let mut packs = BTreeMap::<String, String>::new();
        let mut canonical = BTreeSet::new();
        rows(capture, "corpus", "relation_packs", root, |pack| {
            let key = id(&pack, "pack_id");
            if !key.is_empty() {
                packs.insert(key.to_owned(), id(&pack, "path").to_owned());
                if id(&pack, "owner_branch") == "ToS/canon" {
                    canonical.insert(key.to_owned());
                }
            }
            if packs.len() > 8192 {
                return Err(Error::Budget("public D1 corpus pack paths"));
            }
            Ok(())
        })?;
        rows(capture, "corpus", "relation_edges", root, |mut edge| {
            if edges.len() >= limit {
                return Ok(());
            }
            let selected = if view_id == "route-graph" {
                canonical.contains(id(&edge, "pack_id"))
            } else {
                id(&edge, "owner_branch") == "ToS/candidate-intake"
            };
            if selected {
                if edge["source_ref"].as_str().is_none_or(str::is_empty) {
                    if let Some(path) = packs.get(id(&edge, "pack_id")).filter(|s| !s.is_empty()) {
                        edge["source_ref"] = json!(path);
                    }
                }
                edges.push(edge);
            }
            Ok(())
        })?;
        let endpoints = edges
            .iter()
            .flat_map(|e| [id(e, "from_id"), id(e, "to_id")])
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let mut indexed = BTreeMap::new();
        rows(capture, "corpus", "nodes", root, |node| {
            let key = id(&node, "node_id");
            if endpoints.contains(key) {
                indexed.insert(key.to_owned(), node);
            }
            Ok(())
        })?;
        if view_id == "route-graph" {
            nodes.extend(indexed.into_values());
        } else {
            let mut refs = BTreeMap::<String, BTreeSet<String>>::new();
            for edge in &edges {
                let source = id(edge, "source_ref");
                for endpoint in [id(edge, "from_id"), id(edge, "to_id")] {
                    if endpoints.contains(endpoint) && !source.is_empty() {
                        refs.entry(endpoint.to_owned())
                            .or_default()
                            .insert(source.to_owned());
                    }
                }
            }
            for endpoint in endpoints {
                nodes.push(indexed.remove(&endpoint).unwrap_or_else(||json!({
                "node_id":endpoint,"label":endpoint,"node_type":"candidate-endpoint",
                "authority_layer":"candidate_intake","owner_branch":"ToS/candidate-intake",
                "source_refs":refs.remove(&endpoint).unwrap_or_default().into_iter().collect::<Vec<_>>()
            })));
            }
        }
        items = edges.clone();
    }
    Ok(
        json!({"schema":"tos_corpus_mcp_graph_view_v1","view":view,"item_count":items.len(),
        "items":items,"node_count":nodes.len(),"edge_count":edges.len(),"nodes":nodes,"edges":edges,
        "counts":header["counts"],"runtime_projection_boundary":header["runtime_projection_boundary"]}),
    )
}

fn knowledge_contracts(capture: &PublicCapture, _root: &Path) -> Result<Value> {
    const CONTRACTS: [(&str, &str); 13] = [
        ("api", "access/contracts/knowledge-api.v1.json"),
        (
            "knowledge_graph",
            "access/contracts/knowledge-graph.v1.schema.json",
        ),
        (
            "knowledge_search_indexed",
            "access/contracts/knowledge-search-indexed.v2.schema.json",
        ),
        (
            "readable_context",
            "access/contracts/readable-context.v1.schema.json",
        ),
        ("lens_spec", "access/contracts/lens-spec.v1.schema.json"),
        ("lens_result", "access/contracts/lens-result.v1.schema.json"),
        (
            "temporal_comparison_request",
            "access/contracts/temporal-comparison-request.v1.schema.json",
        ),
        (
            "temporal_comparison_result",
            "access/contracts/temporal-comparison-result.v1.schema.json",
        ),
        ("source_read", "access/contracts/source-read.v1.schema.json"),
        (
            "entity_type_registry_schema",
            "ToS/contracts/semantic-entity-type-registry.schema.json",
        ),
        (
            "relation_type_registry_schema",
            "ToS/contracts/semantic-relation-type-registry.schema.json",
        ),
        (
            "entity_type_registry",
            "ToS/doctrine/semantic-interchange/entity-types.v1.json",
        ),
        (
            "relation_type_registry",
            "ToS/doctrine/semantic-interchange/relation-types.v1.json",
        ),
    ];
    let mut contracts = serde_json::Map::new();
    let mut refs = Vec::new();
    for (name, path) in CONTRACTS {
        let raw = capture
            .read_input(path, 4 * 1024 * 1024)?
            .ok_or(Error::Invalid("public D1 contract absent"))?;
        let value: Value =
            serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))?;
        contracts.insert(name.to_owned(), value);
        refs.push(path);
    }
    Ok(
        json!({"schema":"tos_knowledge_contract_bundle_v1","contracts":contracts,"source_refs":refs,
        "authority_boundary":{"is_source":false,"writes_to_tree":false,
            "source_owner":"Tree-of-Sophia/access/contracts",
            "note":"This packet transports versioned access contracts; it does not author ToS meaning."}}),
    )
}

fn exploration_contracts(capture: &PublicCapture, _root: &Path) -> Result<Value> {
    let mut result = serde_json::Map::new();
    for (field, path) in [
        (
            "request",
            "access/contracts/exploration-request.v1.schema.json",
        ),
        (
            "result",
            "access/contracts/exploration-result.v1.schema.json",
        ),
        (
            "request_v2",
            "access/contracts/exploration-request.v2.schema.json",
        ),
        (
            "result_v2",
            "access/contracts/exploration-result.v2.schema.json",
        ),
    ] {
        let raw = capture
            .read_input(path, 4 * 1024 * 1024)?
            .ok_or(Error::Invalid("public D1 exploration contract absent"))?;
        result.insert(
            field.into(),
            serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))?,
        );
    }
    Ok(Value::Object(result))
}

fn source_gaps(capture: &PublicCapture, root: &str) -> Result<Value> {
    let mut gaps = Vec::new();
    for path in capture.public_ledger_labels().into_iter().take(100) {
        let raw = capture
            .read_input(path, 256_000)?
            .ok_or(Error::Invalid("public D1 source gap absent"))?;
        let mut record: Value =
            serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))?;
        portable(&mut record, root);
        if id(&record, "schema_version") != "tos_access_request_v1"
            || record["personal_or_confidential_data_committed"] == true
        {
            return Err(Error::Invalid("public D1 unsafe source gap"));
        }
        let material = &record["material"];
        let response = &record["response"];
        let request_id = record["request_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                path.rsplit('/')
                    .next()
                    .unwrap_or("")
                    .trim_end_matches(".access-request.json")
                    .to_owned()
            });
        let title = material["title"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(&request_id);
        let from = material["tos_refs"]
            .as_array()
            .and_then(|refs| refs.first())
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| "tos.subject.friedrich-nietzsche".into());
        let mut source_refs = vec![path.to_owned()];
        for (object, field) in [
            (material, "discovery_refs"),
            (&record, "rights_record_refs"),
            (response, "safe_evidence_refs"),
        ] {
            for item in object[field]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if !source_refs.iter().any(|existing| existing == item) {
                    source_refs.push(item.to_owned());
                }
            }
        }
        let access = record["access_status"].as_str().unwrap_or("unknown");
        let request = record["request_status"].as_str().unwrap_or("unknown");
        let state = response["state"].as_str().unwrap_or("none");
        let from_label = material["edition_or_resource"].as_str().unwrap_or(&from);
        let gap = json!({"edge_id":format!("cluster-relation:source-gap:{request_id}"),
            "from_id":from,"to_id":request_id,"from_label":from_label,"to_label":title,
            "predicate_id":"source_access_gap","source_refs":source_refs,"access_status":access,
            "request_status":request,"response_state":state,"request_sent":record["sent_at"].as_str().is_some_and(|s|!s.is_empty()),
            "authority_posture":"source_witness_public_ledger","review_posture":request,
            "canon_status":"not_applicable","confidence":"recorded_status",
            "properties":{"public_summary_en":format!("ToS records {title} as {access}; request status is {request} and response state is {state}."),
                "research_purpose":record["research_purpose"].as_str().unwrap_or("")}});
        let key = python_casefold_unicode16_v1(title, 256_000, 1_000_000, 1_000_000)
            .map_err(|e| Error::Source(e.to_string()))?;
        capture.charge_work(key.len() as u64)?;
        gaps.push((key, gap));
    }
    gaps.sort_by(|a, b| a.0.cmp(&b.0));
    let count = gaps.len();
    let gaps = gaps.into_iter().map(|(_, gap)| gap).collect::<Vec<_>>();
    Ok(
        json!({"schema":"tos_source_gap_search_v1","query":"","result_count":count,"gaps":gaps,
        "authority_note":"These are recorded source-access gaps in a bounded public runtime set; this is not a corpus-completeness or legal conclusion. No request is sent and no source or canon is changed."}),
    )
}

fn legacy_navigation(capture: &PublicCapture, root: &str) -> Result<Value> {
    let mut header = capture.header_object("corpus", "source_navigation", 2 * 1024 * 1024)?;
    portable(&mut header, root);
    if id(&header, "schema_version") != "tos_source_navigation_v1" {
        return Err(Error::Invalid("public D1 navigation schema"));
    }
    let mut nodes = Vec::new();
    let mut ids = BTreeSet::new();
    rows(capture, "corpus", "source_navigation/nodes", root, |node| {
        if node["properties"]["packet_id"]
            .as_str()
            .is_none_or(str::is_empty)
        {
            let key = id(&node, "node_id");
            if !key.is_empty() {
                ids.insert(key.to_owned());
            }
            nodes.push(node);
        }
        if nodes.len() > 100_000 {
            return Err(Error::Budget("public D1 static navigation nodes"));
        }
        Ok(())
    })?;
    let mut edges = Vec::new();
    rows(capture, "corpus", "source_navigation/edges", root, |edge| {
        if ids.contains(id(&edge, "from_id")) && ids.contains(id(&edge, "to_id")) {
            edges.push(edge);
        }
        if edges.len() > 100_000 {
            return Err(Error::Budget("public D1 static navigation edges"));
        }
        Ok(())
    })?;
    let node_count = nodes.len();
    let edge_count = edges.len();
    header["nodes"] = json!(nodes);
    header["edges"] = json!(edges);
    header["counts"]["nodes"] = json!(node_count);
    header["counts"]["edges"] = json!(edge_count);
    Ok(header)
}

fn phi_selected(
    capture: &PublicCapture,
    view: &Value,
    root: &str,
    limit: usize,
) -> Result<(Vec<Value>, Vec<Value>, usize, usize)> {
    let inline_nodes = array(view, "nodes");
    let inline_edges = array(view, "edges");
    if !inline_nodes.is_empty() || !inline_edges.is_empty() {
        let node_count = inline_nodes.len();
        let edge_count = inline_edges.len();
        return Ok((
            inline_nodes.into_iter().take(limit).collect(),
            inline_edges.into_iter().take(limit).collect(),
            node_count,
            edge_count,
        ));
    }
    let node_ids = strings(view, "node_ids");
    let edge_ids = strings(view, "edge_ids");
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut node_count = 0usize;
    let mut edge_count = 0usize;
    rows(capture, "philosophy", "nodes", root, |node| {
        if node_ids.contains(id(&node, "node_id")) {
            node_count += 1;
            if nodes.len() < limit {
                nodes.push(node);
            }
        }
        Ok(())
    })?;
    rows(capture, "philosophy", "edges", root, |edge| {
        if edge_ids.contains(id(&edge, "edge_id")) {
            edge_count += 1;
            if edges.len() < limit {
                edges.push(edge);
            }
        }
        Ok(())
    })?;
    Ok((nodes, edges, node_count, edge_count))
}

fn phi_review(
    capture: &PublicCapture,
    view_id: &str,
    root: &str,
    boundary: &Value,
) -> Result<Value> {
    let mut found = None;
    rows(capture, "philosophy", "review_packets", root, |packet| {
        if id(&packet, "view_id") == view_id {
            if found.replace(packet).is_some() {
                return Err(Error::Invalid("public D1 duplicate review packet"));
            }
        }
        Ok(())
    })?;
    Ok(json!({"schema":"tos_philosophy_mcp_review_packet_v1",
        "packet":found.ok_or(Error::Invalid("public D1 review packet absent"))?,
        "runtime_projection_boundary":boundary,
        "authority_note":"Tree-of-Sophia owns review packet semantics; MCP serves the compact access packet."}))
}

fn phi_clusters(
    capture: &PublicCapture,
    view_id: &str,
    root: &str,
    limit: usize,
) -> Result<(Vec<Value>, usize, BTreeSet<String>)> {
    let mut selected = Vec::new();
    let mut count = 0usize;
    let mut kinds = BTreeSet::new();
    rows(capture, "philosophy", "clusters", root, |cluster| {
        if !strings(&cluster, "view_ids").contains(view_id) {
            return Ok(());
        }
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("public D1 clusters"))?;
        let kind = id(&cluster, "cluster_kind");
        if !kind.is_empty() {
            kinds.insert(kind.to_owned());
        }
        selected.push(cluster);
        selected.sort_by(|a, b| {
            (id(a, "cluster_kind"), id(a, "label")).cmp(&(id(b, "cluster_kind"), id(b, "label")))
        });
        if selected.len() > limit {
            selected.pop();
        }
        Ok(())
    })?;
    Ok((selected, count, kinds))
}

fn bounded_graph(nodes: Vec<Value>, edges: Vec<Value>, limit: usize) -> (Vec<Value>, Vec<Value>) {
    let by_id = nodes
        .iter()
        .filter_map(|node| {
            let key = id(node, "node_id");
            (!key.is_empty()).then_some((key.to_owned(), node))
        })
        .collect::<BTreeMap<_, _>>();
    let mut selected = BTreeSet::new();
    let mut output_edges = Vec::new();
    for edge in &edges {
        let left = id(edge, "from_id");
        let right = id(edge, "to_id");
        if !by_id.contains_key(left) || !by_id.contains_key(right) {
            continue;
        }
        let additions = usize::from(!selected.contains(left))
            + usize::from(left != right && !selected.contains(right));
        if selected.len() + additions > limit {
            continue;
        }
        selected.insert(left.to_owned());
        selected.insert(right.to_owned());
        output_edges.push(edge.clone());
        if output_edges.len() >= limit {
            break;
        }
    }
    for node in &nodes {
        if selected.len() >= limit {
            break;
        }
        let key = id(node, "node_id");
        if !key.is_empty() {
            selected.insert(key.to_owned());
        }
    }
    (
        nodes
            .into_iter()
            .filter(|node| selected.contains(id(node, "node_id")))
            .collect(),
        output_edges,
    )
}

fn bounded_clusters(clusters: Vec<Value>, nodes: &[Value], edges: &[Value]) -> Vec<Value> {
    let node_ids = nodes
        .iter()
        .map(|n| id(n, "node_id"))
        .collect::<BTreeSet<_>>();
    let edge_ids = edges
        .iter()
        .map(|e| id(e, "edge_id"))
        .collect::<BTreeSet<_>>();
    clusters
        .into_iter()
        .filter_map(|mut cluster| {
            let member_nodes = ordered_strings(&cluster, "member_node_ids");
            let member_edges = ordered_strings(&cluster, "member_edge_ids");
            let matched_nodes = member_nodes
                .iter()
                .filter(|n| node_ids.contains(n.as_str()))
                .collect::<Vec<_>>();
            let matched_edges = member_edges
                .iter()
                .filter(|e| edge_ids.contains(e.as_str()))
                .collect::<Vec<_>>();
            if matched_nodes.is_empty() && matched_edges.is_empty() {
                return None;
            }
            cluster["member_node_ids"] = json!(matched_nodes);
            cluster["member_edge_ids"] = json!(matched_edges);
            cluster["available_member_node_count"] = json!(member_nodes.len());
            cluster["available_member_edge_count"] = json!(member_edges.len());
            if cluster["properties"]["member_count"].is_number() {
                cluster["properties"]["member_count"] = json!(matched_nodes.len());
            }
            if cluster["properties"]["edge_count"].is_number() {
                cluster["properties"]["edge_count"] = json!(matched_edges.len());
            }
            Some(cluster)
        })
        .collect()
}

fn unique(rows: &[Value], field: &str) -> Vec<String> {
    rows.iter()
        .map(|row| id(row, field))
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn phi_view_kinds(
    capture: &PublicCapture,
    view: &Value,
    root: &str,
) -> Result<(Vec<String>, Vec<String>)> {
    if !array(view, "nodes").is_empty() || !array(view, "edges").is_empty() {
        return Ok((
            unique(&array(view, "nodes"), "node_type"),
            unique(&array(view, "edges"), "predicate_id"),
        ));
    }
    let node_ids = strings(view, "node_ids");
    let edge_ids = strings(view, "edge_ids");
    let mut kinds = BTreeSet::new();
    let mut predicates = BTreeSet::new();
    rows(capture, "philosophy", "nodes", root, |node| {
        if node_ids.contains(id(&node, "node_id")) {
            let kind = id(&node, "node_type");
            if !kind.is_empty() {
                kinds.insert(kind.to_owned());
            }
        }
        Ok(())
    })?;
    rows(capture, "philosophy", "edges", root, |edge| {
        if edge_ids.contains(id(&edge, "edge_id")) {
            let predicate = id(&edge, "predicate_id");
            if !predicate.is_empty() {
                predicates.insert(predicate.to_owned());
            }
        }
        Ok(())
    })?;
    Ok((
        kinds.into_iter().collect(),
        predicates.into_iter().collect(),
    ))
}

fn phi_view_packet(
    capture: &PublicCapture,
    view: &Value,
    root: &str,
    boundary: &Value,
    review: &Value,
    limit: usize,
) -> Result<Value> {
    let view_id = id(view, "view_id");
    let (nodes, edges, node_count, edge_count) =
        if !array(view, "nodes").is_empty() || !array(view, "edges").is_empty() {
            let all_nodes = array(view, "nodes");
            let all_edges = array(view, "edges");
            let node_count = all_nodes.len();
            let edge_count = all_edges.len();
            let (nodes, edges) = bounded_graph(all_nodes, all_edges, limit);
            (nodes, edges, node_count, edge_count)
        } else {
            let node_ids = strings(view, "node_ids");
            let edge_ids = strings(view, "edge_ids");
            let mut present = BTreeSet::new();
            let mut first_nodes = Vec::new();
            let mut node_count = 0usize;
            rows(capture, "philosophy", "nodes", root, |node| {
                let key = id(&node, "node_id");
                if node_ids.contains(key) {
                    present.insert(key.to_owned());
                    node_count += 1;
                    if first_nodes.len() < limit {
                        first_nodes.push(key.to_owned());
                    }
                }
                Ok(())
            })?;
            let mut selected = BTreeSet::new();
            let mut edges = Vec::new();
            let mut edge_count = 0usize;
            rows(capture, "philosophy", "edges", root, |edge| {
                if edge_ids.contains(id(&edge, "edge_id")) {
                    edge_count += 1;
                    let left = id(&edge, "from_id");
                    let right = id(&edge, "to_id");
                    if !present.contains(left) || !present.contains(right) || edges.len() >= limit {
                        return Ok(());
                    }
                    let additions = usize::from(!selected.contains(left))
                        + usize::from(left != right && !selected.contains(right));
                    if selected.len() + additions <= limit {
                        selected.insert(left.to_owned());
                        selected.insert(right.to_owned());
                        edges.push(edge);
                    }
                }
                Ok(())
            })?;
            for key in first_nodes {
                if selected.len() >= limit {
                    break;
                }
                selected.insert(key);
            }
            let mut nodes = Vec::new();
            rows(capture, "philosophy", "nodes", root, |node| {
                if selected.contains(id(&node, "node_id")) {
                    nodes.push(node);
                }
                Ok(())
            })?;
            (nodes, edges, node_count, edge_count)
        };
    let (clusters, _, _) = phi_clusters(capture, view_id, root, 1000)?;
    let clusters = bounded_clusters(clusters, &nodes, &edges)
        .into_iter()
        .take(limit)
        .collect::<Vec<_>>();
    let mut bounded_view = view.clone();
    if let Some(object) = bounded_view.as_object_mut() {
        object.remove("nodes");
        object.remove("edges");
    }
    bounded_view["node_ids"] = json!(
        nodes
            .iter()
            .map(|n| id(n, "node_id"))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    );
    bounded_view["edge_ids"] = json!(
        edges
            .iter()
            .map(|e| id(e, "edge_id"))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    );
    Ok(
        json!({"schema":"tos_philosophy_mcp_view_v1","view":bounded_view,
        "node_count":nodes.len(),"edge_count":edges.len(),"available_node_count":node_count,
        "available_edge_count":edge_count,"limit":limit,"nodes":nodes,"edges":edges,
        "clusters":clusters,"review_packet":review,
        "source_refs":view["source_refs"],"runtime_projection_boundary":boundary}),
    )
}

fn philosophy(writer: &mut Writer<'_>, root: &str) -> Result<String> {
    let capture = writer.capture;
    let mut header = capture.header_object("philosophy", "", 2 * 1024 * 1024)?;
    portable(&mut header, root);
    let boundary = header["runtime_projection_boundary"].clone();
    let mut views = Vec::new();
    rows(capture, "philosophy", "views", root, |view| {
        if views.len() >= 1000 {
            return Err(Error::Budget("public D1 philosophy views"));
        }
        views.push(view);
        Ok(())
    })?;
    let mut layers = Vec::new();
    rows(capture, "philosophy", "graph_layers", root, |layer| {
        if layers.len() >= 1000 {
            return Err(Error::Budget("public D1 philosophy layers"));
        }
        layers.push(layer);
        Ok(())
    })?;
    let view_ids = views
        .iter()
        .map(|v| v["view_id"].clone())
        .collect::<Vec<_>>();
    let layer_ids = layers
        .iter()
        .map(|l| l["layer_id"].clone())
        .collect::<Vec<_>>();
    let status = json!({"schema":"tos_philosophy_mcp_status_v1","projection_exists":true,
        "tos_root":"Tree-of-Sophia","projection_path":"ToS/derived-exports/philosophy_graph_projection.min.json",
        "owner_repo":header["owner_repo"],"surface_kind":header["surface_kind"],
        "counts":header["counts"],"views":view_ids,"graph_layers":layer_ids,
        "visibility_model":header["visibility_model"],"snapshot_review":header["snapshot_review"],
        "runtime_projection_boundary":boundary,
        "authority_note":"Tree-of-Sophia owns philosophy meaning; this MCP packet is a Tree-of-Sophia standalone access aid."});
    writer.json("__edge/philosophy/status.json", &status)?;
    writer.json(
        "__edge/philosophy/layers.json",
        &json!({"schema":"tos_philosophy_mcp_layers_v1",
        "graph_layers":layers,"layer_counts":header["layer_counts"],
        "visibility_model":header["visibility_model"],"runtime_projection_boundary":boundary}),
    )?;
    writer.json("__edge/philosophy/snapshot.json",&json!({"schema":"tos_philosophy_mcp_snapshot_v1",
        "snapshot_review":header["snapshot_review"],"runtime_projection_boundary":boundary,
        "authority_note":"Tree-of-Sophia owns snapshot semantics; MCP serves fingerprints for review and diff routing."}))?;
    let audit_path =
        "ToS/philosophy/graph-workbench/review-packets/table-i-post-planting-audit.json";
    let audit = capture.read_input(audit_path, 4 * 1024 * 1024)?;
    writer.json("__edge/philosophy/audit.json",&json!({"schema":"tos_philosophy_mcp_audit_v1",
        "audit_exists":audit.is_some(),"audit_path":audit_path,
        "audit":audit.map(|raw|serde_json::from_slice::<Value>(&raw).map_err(|e|Error::Source(e.to_string())))
            .transpose()?.unwrap_or_else(||json!({})),
        "authority_note":"Tree-of-Sophia owns the audit; MCP serves it as an access packet."}))?;
    let unresolved = header["unresolved_review_surfaces"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    writer.json(
        "__edge/philosophy/unresolved/all.json",
        &json!({"schema":"tos_philosophy_mcp_unresolved_v1",
        "view_id":null,"unresolved_count":unresolved.len(),"unresolved":unresolved,
        "runtime_projection_boundary":boundary}),
    )?;
    let mut summaries = Vec::new();
    let mut contracts = Vec::new();
    let mut all_node_kinds = BTreeSet::new();
    let mut all_predicates = BTreeSet::new();
    let mut all_cluster_kinds = BTreeSet::new();
    rows(capture, "philosophy", "nodes", root, |row| {
        let value = id(&row, "node_type");
        if !value.is_empty() {
            all_node_kinds.insert(value.to_owned());
        }
        Ok(())
    })?;
    rows(capture, "philosophy", "edges", root, |row| {
        let value = id(&row, "predicate_id");
        if !value.is_empty() {
            all_predicates.insert(value.to_owned());
        }
        Ok(())
    })?;
    rows(capture, "philosophy", "clusters", root, |row| {
        let value = id(&row, "cluster_kind");
        if !value.is_empty() {
            all_cluster_kinds.insert(value.to_owned());
        }
        Ok(())
    })?;
    for view in &views {
        let view_id = id(view, "view_id");
        if view_id.is_empty() || view_id.contains('/') || view_id == "." || view_id == ".." {
            return Err(Error::Invalid("public D1 philosophy view ID"));
        }
        let (_, _, node_count, edge_count) = phi_selected(capture, view, root, 1)?;
        let (node_kinds, edge_predicates) = phi_view_kinds(capture, view, root)?;
        let (clusters, cluster_count, cluster_kinds) = phi_clusters(capture, view_id, root, 1000)?;
        summaries.push(json!({"view_id":view["view_id"],"title":view["title"],
            "layout_hint":view["layout_hint"],"graph_layers":view["graph_layers"],
            "node_count":node_count,"edge_count":edge_count,"cluster_count":cluster_count,
            "review_intent":view["review_intent"],"collapse_rule":view["collapse_rule"],
            "source_ref":view["source_ref"],"route_card":view["route_card"]}));
        contracts.push(
            json!({"schema":"tos_philosophy_mcp_view_contract_v1","view_id":view["view_id"],
            "route_card":view["route_card"],"layout_hint":view["layout_hint"],
            "graph_layers":view["graph_layers"],"node_kinds":node_kinds,
            "edge_predicates":edge_predicates,
            "cluster_kinds":cluster_kinds.into_iter().collect::<Vec<_>>(),
            "node_count":node_count,"edge_count":edge_count,"cluster_count":cluster_count,
            "source_view_contract_ref":header["source_refs"]["source_view_contract_ref"]}),
        );
        let packet = phi_review(capture, view_id, root, &boundary)?;
        writer.json(
            &format!("__edge/philosophy/review-packet/{view_id}.json"),
            &packet,
        )?;
        let diagnostics = packet["packet"]["unresolved_diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        writer.json(
            &format!("__edge/philosophy/unresolved/{view_id}.json"),
            &json!({"schema":"tos_philosophy_mcp_unresolved_v1","view_id":view_id,
                "unresolved_count":diagnostics.len(),"unresolved":diagnostics,
                "runtime_projection_boundary":boundary}),
        )?;
        for limit in PHILOSOPHY_LIMITS {
            writer.json(
                &format!("__edge/philosophy/views/{view_id}/{limit}.json"),
                &phi_view_packet(capture, view, root, &boundary, &packet, limit)?,
            )?;
        }
        drop(clusters);
    }
    writer.json(
        "__edge/philosophy/views.json",
        &json!({"schema":"tos_philosophy_mcp_views_v1",
        "views":summaries,"counts":header["counts"],"graph_layers":layers,
        "layer_counts":header["layer_counts"],"visibility_model":header["visibility_model"],
        "runtime_projection_boundary":boundary}),
    )?;
    let source_refs = header["source_refs"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| v.as_str().is_some_and(|s| !s.is_empty()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<serde_json::Map<_, _>>()
        })
        .unwrap_or_default();
    writer.json("__edge/philosophy/contracts.json",&json!({"schema":"tos_philosophy_mcp_contracts_v1",
        "source_contract_refs":source_refs,
        "runtime_contract":{"runtime_owner":"Tree-of-Sophia","source_owner":"Tree-of-Sophia",
            "packet_shape":"bounded MCP resources and tools over ToS derived exports",
            "limits":["no writeback","no canon promotion","MCP packets are access aids, not source authority"]},
        "views":contracts,"node_kinds":all_node_kinds.into_iter().collect::<Vec<_>>(),
        "edge_predicates":all_predicates.into_iter().collect::<Vec<_>>(),
        "graph_layers":unique(&layers,"layer_id"),
        "cluster_kinds":all_cluster_kinds.into_iter().collect::<Vec<_>>(),
        "runtime_projection_boundary":boundary,
        "authority_note":"Tree-of-Sophia owns graph meaning; MCP exposes the access-plane contract only."}))?;
    Ok(views
        .first()
        .map(|v| id(v, "view_id").to_owned())
        .unwrap_or_else(|| "chronology".into()))
}

pub(crate) fn build(
    capture: &PublicCapture,
    output: &Path,
    source_root: &Path,
    header: &Value,
    catalog: &Value,
    max_static_bytes: u64,
) -> Result<StaticSummary> {
    if max_static_bytes == 0 || !output.is_dir() {
        return Err(Error::Budget("public D1 static output"));
    }
    let root = source_root
        .to_str()
        .ok_or(Error::Invalid("public D1 source path UTF-8"))?;
    let mut writer = Writer {
        root: output,
        capture,
        max: max_static_bytes,
        bytes: 0,
    };
    let web_inputs = web_assets(&mut writer, source_root)?;
    writer.json(
        "__edge/health.json",
        &json!({"service":"tree-of-sophia-access","ok":true,
        "write_enabled":false,"errors":[],"runtime":"cloudflare-worker"}),
    )?;
    let mut corpus_header = capture.header_object("corpus", "", 2 * 1024 * 1024)?;
    portable(&mut corpus_header, root);
    let default_corpus_view = corpus(&mut writer, &corpus_header, root)?;
    let catalog_bytes = serde_json::to_vec(catalog).map_err(|e| Error::Source(e.to_string()))?;
    if catalog_bytes.len() > 16 * 1024 * 1024 {
        return Err(Error::Budget("public D1 static catalog bytes"));
    }
    capture.charge_work(catalog_bytes.len() as u64)?;
    let mut portable_catalog: Value =
        serde_json::from_slice(&catalog_bytes).map_err(|e| Error::Source(e.to_string()))?;
    portable(&mut portable_catalog, root);
    writer.json("__edge/knowledge/catalog.json", &portable_catalog)?;
    writer.json(
        "__edge/knowledge/contracts.json",
        &knowledge_contracts(capture, source_root)?,
    )?;
    writer.json(
        "__edge/knowledge/exploration-contracts.json",
        &exploration_contracts(capture, source_root)?,
    )?;
    writer.json("__edge/source-gaps/all.json", &source_gaps(capture, root)?)?;
    if !capture.partitioned() {
        writer.json(
            "__edge/source-navigation/all.json",
            &legacy_navigation(capture, root)?,
        )?;
    }
    let default_philosophy_view = philosophy(&mut writer, root)?;
    writer.put("_headers", HEADERS.as_bytes())?;
    // The graph header is built by the same captured normalized producer used
    // for D1 rows. Binding it here prevents a mismatched static/D1 pair.
    if header["source_revision"].as_str().is_none() {
        return Err(Error::Invalid("public D1 static graph header"));
    }
    Ok(StaticSummary {
        default_corpus_view,
        default_philosophy_view,
        bytes: writer.bytes,
        web_dist: source_root.join("access/web/dist"),
        web_inputs,
    })
}
