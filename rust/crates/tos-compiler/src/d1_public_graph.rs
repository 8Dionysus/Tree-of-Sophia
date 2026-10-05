//! Public-projection input bridge for the native normalizers. This is a
//! derived publication snapshot, not an authored source cut, installed-current
//! custody or a disclosure grant. Every family row is an exact captured value
//! or a bounded ordering record over that value.

use crate::{
    Error, QueryVocabulary, Result, SourceBinding,
    d1_public_capture::{CreationState, CreationStateHold, PublicCapture, PublicCaptureLimits},
    knowledge_repository::RepositoryRootInput,
    knowledge_stage::{
        ExactInputReceipt, InputCollectionReceipt, InputRow, KnowledgeStage, StageOwner,
    },
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
use tos_foundation::{Digest256, Digest256Hasher};

const FAMILY_PROFILE: &[(&str, &str, &str, &str)] = &[
    ("philosophy", "nodes", "philosophy", "nodes"),
    ("philosophy", "edges", "philosophy", "edges"),
    (
        "source-navigation",
        "nodes",
        "corpus",
        "source_navigation/nodes",
    ),
    (
        "source-navigation",
        "edges",
        "corpus",
        "source_navigation/edges",
    ),
    ("source-claims", "nodes", "bibliographic", "nodes"),
    ("source-claims", "edges", "bibliographic", "edges"),
    (
        "source-claims",
        "claim_traces",
        "bibliographic",
        "claim_traces",
    ),
    ("canon", "nodes", "corpus", "nodes"),
    ("canon", "relation_packs", "corpus", "relation_packs"),
    ("canon", "relation_edges", "corpus", "relation_edges"),
    (
        "candidate-intake",
        "relation_packs",
        "corpus",
        "relation_packs",
    ),
    (
        "candidate-intake",
        "relation_edges",
        "corpus",
        "relation_edges",
    ),
    ("repository", "branches", "corpus", "branches"),
    ("repository", "manifests", "corpus", "manifests"),
    ("repository", "resources", "corpus", "resources"),
    ("repository", "source_order", "corpus", "source_order"),
];

// Maintained public graph root (knowledge.py::build_knowledge_graph). It is a
// software projection with a source_ref for navigation, not source-home bytes
// or a native-current issuer. This adapter is private to the disposable
// public-build Stage; the native repository root contract remains unchanged.
const PUBLIC_ROOT_PRODUCER: &str = "tos-public-repository-root-projection-v1";
const PUBLIC_ROOT_ROW: &str = r#"{"node_id":"tree-of-sophia","label":"Tree of Sophia","node_type":"repository-root","summary":"Корень индексированной структуры репозитория Tree of Sophia.","source_ref":"ToS/source_home.manifest.json","view_ids":["corpus-topology"],"authority_layer":"source_home"}"#;

pub(crate) struct PublicRepositoryRoot {
    source_cut: String,
    material_sha256: String,
    producer_sha256: String,
}

impl PublicRepositoryRoot {
    pub(crate) fn new(stage: &KnowledgeStage<'_>, source_revision: &str) -> Result<Self> {
        let source_cut = &stage.exact_receipt()?.binding.source_cut;
        if !stage.public_build() || source_cut != &format!("public-projection:{source_revision}") {
            return Err(Error::Invalid("public D1 repository root binding"));
        }
        let mut software = Digest256Hasher::new();
        software.update(PUBLIC_ROOT_PRODUCER.as_bytes());
        software.update(source_cut.as_bytes());
        software.update(PUBLIC_ROOT_ROW.as_bytes());
        Ok(Self {
            source_cut: source_cut.clone(),
            material_sha256: Digest256::of_bytes(PUBLIC_ROOT_ROW.as_bytes()).to_hex(),
            producer_sha256: software.finalize().to_hex(),
        })
    }

    /// Exact maintained software projection, scoped to this captured derived
    /// snapshot. This does not assert authored source-home or live authority.
    pub(crate) fn captured_native_projection(
        capture: &PublicCapture,
        stage: &KnowledgeStage<'_>,
        source_revision: &str,
    ) -> Result<Self> {
        capture.check_custody()?;
        let binding = &stage.exact_receipt()?.binding;
        if stage.public_build()
            || binding.owner_profile != "tos-native-projection-snapshot-v1"
            || binding.source_cut != format!("native-projection:{source_revision}")
        {
            return Err(Error::Invalid("native captured projection root binding"));
        }
        let mut software = Digest256Hasher::new();
        software.update(b"tos-native-captured-repository-projection-v1");
        for raw in [
            include_bytes!("native_snapshot.rs").as_slice(),
            include_bytes!("d1_public_graph.rs").as_slice(),
            include_bytes!("d1_public_capture.rs").as_slice(),
            include_bytes!("d1_public_header.rs").as_slice(),
            include_bytes!("d1_public_semantics.rs").as_slice(),
        ] {
            software.update(&(raw.len() as u64).to_be_bytes());
            software.update(raw);
        }
        software.update(binding.source_cut.as_bytes());
        software.update(PUBLIC_ROOT_ROW.as_bytes());
        Ok(Self {
            source_cut: binding.source_cut.clone(),
            material_sha256: Digest256::of_bytes(PUBLIC_ROOT_ROW.as_bytes()).to_hex(),
            producer_sha256: software.finalize().to_hex(),
        })
    }

    pub(crate) fn input(&self) -> RepositoryRootInput<'_> {
        RepositoryRootInput {
            source_cut: &self.source_cut,
            material: PUBLIC_ROOT_ROW.as_bytes(),
            material_sha256: &self.material_sha256,
            identity_id: "root:tree-of-sophia",
        }
    }

    pub(crate) fn software_binding(&self) -> &str {
        &self.producer_sha256
    }
}

fn id_field(collection: &str) -> &'static str {
    match collection {
        "nodes" => "node_id",
        "edges" | "relation_edges" => "edge_id",
        "relation_packs" => "pack_id",
        "claim_traces" => "claim_ref",
        "branches" | "resources" | "manifests" => "id",
        _ => "",
    }
}

fn row_id(value: &Value, collection: &str, ordinal: u64) -> Result<String> {
    let field = id_field(collection);
    let id = if collection == "relation_edges" {
        if value
            .get("pack_id")
            .is_some_and(|pack| !pack.is_null() && !pack.is_string())
        {
            return Err(Error::Invalid("public D1 relation pack ID type"));
        }
        let edge = value
            .get("edge_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                value
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .ok_or(Error::Invalid("public D1 relation edge ID"))?;
        value
            .get("pack_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|pack| format!("{pack}:{edge}"))
            .unwrap_or_else(|| edge.to_owned())
    } else {
        value
            .get(field)
            .and_then(Value::as_str)
            .or_else(|| {
                if matches!(collection, "branches" | "resources" | "manifests") {
                    value.get("path").and_then(Value::as_str)
                } else {
                    None
                }
            })
            .map(str::to_owned)
            .or_else(|| {
                matches!(collection, "branches" | "resources" | "manifests")
                    .then(|| format!("{collection}:{ordinal}"))
            })
            .ok_or(Error::Invalid("public D1 family authored row ID"))?
    };
    if id.is_empty() || id.len() > 4096 || id.contains('\0') {
        return Err(Error::Invalid("public D1 family row ID"));
    }
    Ok(id)
}

fn row_id_owned(value: &Value, collection: &str, ordinal: u64) -> Result<String> {
    use std::fmt::Write;
    let id = if collection == "relation_edges" {
        if value
            .get("pack_id")
            .is_some_and(|v| !v.is_null() && !v.is_string())
        {
            return Err(Error::Invalid("public D1 relation pack ID type"));
        }
        let edge = value
            .get("edge_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .or_else(|| {
                value
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
            })
            .ok_or(Error::Invalid("public D1 relation edge ID"))?;
        let pack = value
            .get("pack_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let bytes = edge
            .len()
            .checked_add(pack.map_or(0, |v| v.len() + 1))
            .filter(|n| *n <= 4096)
            .ok_or(Error::Invalid("public D1 family row ID"))?;
        let mut output = String::with_capacity(bytes);
        if let Some(pack) = pack {
            output.push_str(pack);
            output.push(':');
        }
        output.push_str(edge);
        output
    } else if let Some(id) = value
        .get(id_field(collection))
        .and_then(Value::as_str)
        .or_else(|| {
            if matches!(collection, "branches" | "resources" | "manifests") {
                value.get("path").and_then(Value::as_str)
            } else {
                None
            }
        })
    {
        if id.len() > 4096 {
            return Err(Error::Invalid("public D1 family row ID"));
        }
        id.to_owned()
    } else if matches!(collection, "branches" | "resources" | "manifests") {
        let mut output = String::with_capacity(collection.len() + 21);
        write!(&mut output, "{collection}:{ordinal}")
            .map_err(|_| Error::Invalid("public D1 family row ID"))?;
        output
    } else {
        return Err(Error::Invalid("public D1 family authored row ID"));
    };
    if id.is_empty() || id.len() > 4096 || id.contains('\0') {
        return Err(Error::Invalid("public D1 family row ID"));
    }
    Ok(id)
}

fn family_filter(value: &Value, source: &str, collection: &str) -> bool {
    match (source, collection) {
        ("canon", "nodes") => true,
        ("canon", "relation_edges") => {
            value.get("owner_branch").and_then(Value::as_str) == Some("ToS/canon")
        }
        ("canon" | "candidate-intake", "relation_packs") => {
            ["pack_id", "path"].into_iter().all(|field| {
                value
                    .get(field)
                    .and_then(Value::as_str)
                    .is_some_and(|text| !text.trim().is_empty())
            })
        }
        ("candidate-intake", "relation_edges") => {
            value.get("owner_branch").and_then(Value::as_str) != Some("ToS/canon")
        }
        _ => true,
    }
}

fn add_ref(
    db: &Connection,
    source: &str,
    collection: &str,
    id: &str,
    role: &str,
    input_collection: &str,
    source_key: &str,
) -> Result<()> {
    db.execute("INSERT INTO public_family_rows(source_graph,collection,id,input_role,input_collection,source_key,synthetic,synthetic_sha256) VALUES (?1,?2,?3,?4,?5,?6,NULL,NULL)",
        params![source,collection,id,role,input_collection,source_key])?;
    Ok(())
}

fn add_synthetic(
    db: &Connection,
    source: &str,
    collection: &str,
    id: &str,
    raw: &[u8],
) -> Result<()> {
    db.execute("INSERT INTO public_family_rows(source_graph,collection,id,input_role,input_collection,source_key,synthetic,synthetic_sha256) VALUES (?1,?2,?3,NULL,NULL,NULL,?4,?5)",
        params![source,collection,id,raw,Digest256::of_bytes(raw).as_bytes().as_slice()])?;
    Ok(())
}

/// Add exact input references once, retaining at most one source row at a
/// time. Source and candidate packs are disjoint by their authored owner
/// branch; the repository order records carry only captured ordinals.
pub(crate) fn prepare_family_rows(
    capture: &PublicCapture,
    limits: PublicCaptureLimits,
) -> Result<()> {
    capture.prepare_family_rows_once(limits)
}

/// Internal SQL body; only PublicCapture's sanctioned transition calls it.
pub(crate) fn prepare_family_rows_unsealed(
    capture: &PublicCapture,
    limits: PublicCaptureLimits,
    held: &std::fs::File,
) -> Result<()> {
    capture.check_custody()?;
    if limits.max_rows == 0 || limits.max_work_bytes == 0 || limits.max_staging_bytes < 65536 {
        return Err(Error::Budget("public D1 family limits"));
    }
    let db = capture.write_family_db(held)?;
    let pages = limits.max_staging_bytes / 4096;
    if pages > i64::MAX as u64 {
        return Err(Error::Budget("public D1 family page bound"));
    }
    let main_pages: i64 = db.query_row(&format!("PRAGMA max_page_count={pages}"), [], |row| {
        row.get(0)
    })?;
    let temp_pages: i64 =
        db.query_row(&format!("PRAGMA temp.max_page_count={pages}"), [], |row| {
            row.get(0)
        })?;
    if main_pages != pages as i64 || temp_pages != pages as i64 {
        return Err(Error::Budget("public D1 family page admission"));
    }
    db.execute_batch("CREATE TABLE public_family_rows(source_graph TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,input_role TEXT,input_collection TEXT,source_key TEXT,synthetic BLOB,synthetic_sha256 BLOB,PRIMARY KEY(source_graph,collection,id)) WITHOUT ROWID; CREATE INDEX public_family_input ON public_family_rows(input_role,input_collection,source_key); CREATE TABLE public_pack_paths(pack_id TEXT PRIMARY KEY,path TEXT NOT NULL) WITHOUT ROWID;")?;
    let mut total = 0u64;
    let mut work = 0u64;
    for (source, collection, role, input_collection) in FAMILY_PROFILE {
        let mut ordinal = 0u64;
        let mut after: Option<(String, String, String)> = None;
        loop {
            // Hold only bounded references between the read cursor and one
            // durable page. The cursor is closed before BEGIN/COMMIT.
            let mut page = Vec::with_capacity(128);
            let mut pack_page = Vec::new();
            let mut seen = 0usize;
            {
                let mut stmt=db.prepare("SELECT sort0,sort1,source_key,json FROM capture_rows WHERE role=?1 AND collection=?2 AND (?3 IS NULL OR (sort0,sort1,source_key)>(?3,?4,?5)) ORDER BY sort0,sort1,source_key LIMIT 128")?;
                let mut rows = stmt.query(params![
                    role,
                    input_collection,
                    after.as_ref().map(|v| v.0.as_str()),
                    after.as_ref().map(|v| v.1.as_str()),
                    after.as_ref().map(|v| v.2.as_str())
                ])?;
                while let Some(row) = rows.next()? {
                    let sort0: String = row.get(0)?;
                    let sort1: String = row.get(1)?;
                    let key: String = row.get(2)?;
                    let raw: Vec<u8> = row.get(3)?;
                    work = work
                        .checked_add(raw.len() as u64)
                        .filter(|n| *n <= limits.max_work_bytes)
                        .ok_or(Error::Budget("public D1 family work"))?;
                    capture.charge_work(raw.len() as u64)?;
                    let value: Value =
                        serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))?;
                    if *source == "canon" && *collection == "relation_packs" {
                        if let (Some(id), Some(path)) = (
                            value.get("pack_id").and_then(Value::as_str),
                            value.get("path").and_then(Value::as_str),
                        ) && !id.trim().is_empty()
                            && !path.trim().is_empty()
                        {
                            pack_page.push((id.to_owned(), path.to_owned()));
                        }
                    }
                    if family_filter(&value, source, collection) {
                        let id = row_id(&value, collection, ordinal)?;
                        page.push((key.clone(), id, ordinal));
                    }
                    after = Some((sort0, sort1, key));
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(Error::Budget("public D1 family ordinal"))?;
                    seen += 1;
                }
            }
            if seen == 0 {
                break;
            }
            db.execute_batch("BEGIN IMMEDIATE")?;
            for (id, path) in pack_page {
                capture.charge_work((id.len() + path.len()) as u64)?;
                db.execute(
                    "INSERT INTO public_pack_paths(pack_id,path) VALUES (?1,?2)",
                    params![id, path],
                )?;
            }
            for (key, id, position) in page {
                add_ref(&db, source, collection, &id, role, input_collection, &key)?;
                total = total
                    .checked_add(1)
                    .filter(|n| *n <= limits.max_rows)
                    .ok_or(Error::Budget("public D1 family rows"))?;
                if *source == "repository" {
                    let material = json!({"collection":collection,"id":id,"ordinal":position});
                    let encoded =
                        serde_json::to_vec(&material).map_err(|e| Error::Source(e.to_string()))?;
                    let order_id = format!("{collection}:{position:020}");
                    add_synthetic(&db, "repository", "source_order", &order_id, &encoded)?;
                    total = total
                        .checked_add(1)
                        .filter(|n| *n <= limits.max_rows)
                        .ok_or(Error::Budget("public D1 family rows"))?;
                    work = work
                        .checked_add(encoded.len() as u64)
                        .filter(|n| *n <= limits.max_work_bytes)
                        .ok_or(Error::Budget("public D1 family work"))?;
                    capture.charge_work(encoded.len() as u64)?;
                }
            }
            db.execute_batch("COMMIT")?;
        }
    }
    Ok(())
}

/// Same maintained family selection, with each SQL payload borrowed and each
/// page owner admitted before copying. Guards expire only after its rows and
/// durable batch have dropped; the retained seek key has a separate lifetime.
pub(crate) fn prepare_family_rows_owned(
    capture: &PublicCapture,
    limits: PublicCaptureLimits,
    state: &CreationState<'_>,
) -> Result<()> {
    capture.prepare_family_rows_once_with_owned_state(limits, Some(state))
}

pub(crate) fn prepare_family_rows_owned_unsealed(
    capture: &PublicCapture,
    limits: PublicCaptureLimits,
    held: &std::fs::File,
    state: &CreationState<'_>,
) -> Result<()> {
    capture.check_custody()?;
    if limits.max_rows == 0 || limits.max_work_bytes == 0 || limits.max_staging_bytes < 65536 {
        return Err(Error::Budget("public D1 family limits"));
    }
    let _db_frame = state.hold(tos_source_store::PinnedSqliteConnection::immutable_retained_rust_state_upper_bound()
        + tos_source_store::PinnedSqliteConnection::immutable_open_rust_workspace_upper_bound()
        + tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
        + std::mem::size_of::<rusqlite::Statement<'_>>() + std::mem::size_of::<rusqlite::Rows<'_>>()
        + 4 * std::mem::size_of::<rusqlite::types::ValueRef<'_>>())?;
    let db = capture.write_family_db(held)?;
    let pages = limits.max_staging_bytes / 4096;
    if pages > i64::MAX as u64 {
        return Err(Error::Budget("public D1 family page bound"));
    }
    let pragma_hold = state.hold(128)?;
    let main_pages: i64 =
        db.query_row(&format!("PRAGMA max_page_count={pages}"), [], |r| r.get(0))?;
    let temp_pages: i64 =
        db.query_row(&format!("PRAGMA temp.max_page_count={pages}"), [], |r| {
            r.get(0)
        })?;
    drop(pragma_hold);
    if main_pages != pages as i64 || temp_pages != pages as i64 {
        return Err(Error::Budget("public D1 family page admission"));
    }
    db.execute_batch("CREATE TABLE public_family_rows(source_graph TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,input_role TEXT,input_collection TEXT,source_key TEXT,synthetic BLOB,synthetic_sha256 BLOB,PRIMARY KEY(source_graph,collection,id)) WITHOUT ROWID; CREATE INDEX public_family_input ON public_family_rows(input_role,input_collection,source_key); CREATE TABLE public_pack_paths(pack_id TEXT PRIMARY KEY,path TEXT NOT NULL) WITHOUT ROWID;")?;
    let mut total = 0u64;
    let mut work = 0u64;
    for (source, collection, role, input_collection) in FAMILY_PROFILE {
        let mut ordinal = 0u64;
        let mut after_hold = None;
        let mut after: Option<(String, String, String)> = None;
        loop {
            let slots = 128
                * (std::mem::size_of::<(String, String, u64)>()
                    + std::mem::size_of::<(String, String)>())
                + 256 * std::mem::size_of::<CreationStateHold<'_, '_>>();
            let _page_slots = state.hold(slots)?;
            let mut page_holds = Vec::with_capacity(256);
            let mut page = Vec::with_capacity(128);
            let mut pack_page = Vec::with_capacity(128);
            let mut seen = 0usize;
            {
                let mut stmt = db.prepare("SELECT sort0,sort1,source_key,json FROM capture_rows WHERE role=?1 AND collection=?2 AND (?3 IS NULL OR (sort0,sort1,source_key)>(?3,?4,?5)) ORDER BY sort0,sort1,source_key LIMIT 128")?;
                let mut rows = stmt.query(params![
                    role,
                    input_collection,
                    after.as_ref().map(|v| v.0.as_str()),
                    after.as_ref().map(|v| v.1.as_str()),
                    after.as_ref().map(|v| v.2.as_str())
                ])?;
                while let Some(row) = rows.next()? {
                    state.active()?;
                    let sort0 = row.get_ref(0)?.as_str().map_err(|_| Error::Invalid("public D1 SQL text"))?;
                    let sort1 = row.get_ref(1)?.as_str().map_err(|_| Error::Invalid("public D1 SQL text"))?;
                    let key = row.get_ref(2)?.as_str().map_err(|_| Error::Invalid("public D1 SQL text"))?;
                    let raw = row.get_ref(3)?.as_blob().map_err(|_| Error::Invalid("public D1 SQL blob"))?;
                    work = work
                        .checked_add(raw.len() as u64)
                        .filter(|n| *n <= limits.max_work_bytes)
                        .ok_or(Error::Budget("public D1 family work"))?;
                    capture.charge_work(raw.len() as u64)?;
                    let json_limits =
                        tos_foundation::JsonLimits::new(raw.len().max(1), 96, 1_000_000, 4096)
                            .map_err(|_| Error::Budget("public D1 family JSON limits"))?;
                    state.with_serde_owned_with_limits(raw, json_limits, |value| {
                        if *source == "canon" && *collection == "relation_packs" {
                            if let (Some(id), Some(path)) = (
                                value.get("pack_id").and_then(Value::as_str),
                                value.get("path").and_then(Value::as_str),
                            ) && !id.trim().is_empty()
                                && !path.trim().is_empty()
                            {
                                let bytes = id
                                    .len()
                                    .checked_add(path.len())
                                    .ok_or(Error::Budget("public D1 pack page state"))?;
                                page_holds.push(state.hold(bytes)?);
                                pack_page.push((id.to_owned(), path.to_owned()));
                            }
                        }
                        if family_filter(value, source, collection) {
                            // row_id's validated output is <=4096; reject an
                            // oversized borrowed candidate before its copy.
                            let candidate = if *collection == "relation_edges" {
                                let edge = value
                                    .get("edge_id")
                                    .and_then(Value::as_str)
                                    .map(str::trim)
                                    .filter(|s| !s.is_empty())
                                    .or_else(|| {
                                        value
                                            .get("id")
                                            .and_then(Value::as_str)
                                            .map(str::trim)
                                            .filter(|s| !s.is_empty())
                                    });
                                edge.map(|edge| {
                                    edge.len().saturating_add(
                                        value
                                            .get("pack_id")
                                            .and_then(Value::as_str)
                                            .map(str::trim)
                                            .filter(|s| !s.is_empty())
                                            .map_or(0, |s| s.len().saturating_add(1)),
                                    )
                                })
                            } else {
                                value
                                    .get(id_field(collection))
                                    .and_then(Value::as_str)
                                    .or_else(|| {
                                        if matches!(
                                            *collection,
                                            "branches" | "resources" | "manifests"
                                        ) {
                                            value.get("path").and_then(Value::as_str)
                                        } else {
                                            None
                                        }
                                    })
                                    .map(str::len)
                            };
                            if candidate.is_some_and(|n| n > 4096) {
                                return Err(Error::Invalid("public D1 family row ID"));
                            }
                            let bytes = key
                                .len()
                                .checked_add(candidate.unwrap_or(collection.len() + 21))
                                .ok_or(Error::Budget("public D1 row page state"))?;
                            page_holds.push(state.hold(bytes)?);
                            page.push((
                                key.to_owned(),
                                row_id_owned(value, collection, ordinal)?,
                                ordinal,
                            ));
                        }
                        Ok(())
                    })?;
                    let next_bytes = sort0
                        .len()
                        .checked_add(sort1.len())
                        .and_then(|n| n.checked_add(key.len()))
                        .ok_or(Error::Budget("public D1 family seek state"))?;
                    let next_hold = state.hold(next_bytes)?;
                    after = Some((sort0.to_owned(), sort1.to_owned(), key.to_owned()));
                    after_hold = Some(next_hold);
                    ordinal = ordinal
                        .checked_add(1)
                        .ok_or(Error::Budget("public D1 family ordinal"))?;
                    seen += 1;
                }
            }
            if seen == 0 {
                break;
            }
            db.execute_batch("BEGIN IMMEDIATE")?;
            for (id, path) in pack_page {
                capture.charge_work((id.len() + path.len()) as u64)?;
                db.execute(
                    "INSERT INTO public_pack_paths(pack_id,path) VALUES (?1,?2)",
                    params![id, path],
                )?;
            }
            for (key, id, position) in page {
                add_ref(&db, source, collection, &id, role, input_collection, &key)?;
                total = total
                    .checked_add(1)
                    .filter(|n| *n <= limits.max_rows)
                    .ok_or(Error::Budget("public D1 family rows"))?;
                if *source == "repository" {
                    let _material_hold = state.hold(
                        2048usize
                            .checked_add(id.len())
                            .and_then(|n| n.checked_add(collection.len()))
                            .ok_or(Error::Budget("public D1 order record state"))?,
                    )?;
                    let material = json!({"collection":collection,"id":id,"ordinal":position});
                    let order_id = format!("{collection}:{position:020}");
                    state.with_json_encoded(&material, 8192, |encoded| {
                        work = work
                            .checked_add(encoded.len() as u64)
                            .filter(|n| *n <= limits.max_work_bytes)
                            .ok_or(Error::Budget("public D1 family work"))?;
                        capture.charge_work(encoded.len() as u64)?;
                        add_synthetic(&db, "repository", "source_order", &order_id, encoded)
                    })?;
                    total = total
                        .checked_add(1)
                        .filter(|n| *n <= limits.max_rows)
                        .ok_or(Error::Budget("public D1 family rows"))?;
                }
            }
            db.execute_batch("COMMIT")?;
            drop(page_holds);
        }
        drop(after);
        drop(after_hold);
    }
    Ok(())
}

fn root_item(hash: &mut Digest256Hasher, id: &str, digest: &[u8]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(digest);
}

pub(crate) fn captured_input_roots(
    capture: &PublicCapture,
    vocabulary: &QueryVocabulary,
) -> Result<(Vec<InputCollectionReceipt>, String, Digest256)> {
    capture.check_custody()?;
    let db = capture.read_db()?;
    let mut collections = Vec::new();
    for source in &vocabulary.sources {
        let mut names = FAMILY_PROFILE
            .iter()
            .filter(|(graph, _, _, _)| *graph == source.source_graph_id.as_str())
            .map(|(_, collection, _, _)| (*collection).to_owned())
            .collect::<Vec<_>>();
        if source.adapter_profile == "declared-identity-and-source-ref-joins-v1" {
            names.push("join_scope".to_owned());
        }
        names.sort();
        names.dedup();
        if names.is_empty() {
            return Err(Error::Invalid("public D1 unsupported source adapter"));
        }
        for name in names {
            let mut hash = Digest256Hasher::new();
            let mut count = 0u64;
            let mut rows=db.prepare("SELECT f.id,coalesce(f.synthetic_sha256,c.sha256) FROM public_family_rows f LEFT JOIN capture_rows c ON c.role=f.input_role AND c.collection=f.input_collection AND c.source_key=f.source_key WHERE f.source_graph=?1 AND f.collection=?2 ORDER BY f.id")?;
            let mut rows = rows.query(params![source.source_graph_id, name])?;
            while let Some(row) = rows.next()? {
                let id: String = row.get(0)?;
                let sha: Vec<u8> = row.get(1)?;
                if sha.len() != 32 {
                    return Err(Error::Invalid("public D1 family digest"));
                }
                capture.charge_work(id.len() as u64 + sha.len() as u64)?;
                root_item(&mut hash, &id, &sha);
                count = count
                    .checked_add(1)
                    .ok_or(Error::Budget("public D1 receipt rows"))?;
            }
            collections.push(InputCollectionReceipt {
                source_graph: source.source_graph_id.clone(),
                collection: name,
                input_role: source.input_role.clone(),
                adapter_profile: source.adapter_profile.clone(),
                expected_count: count,
                expected_root_sha256: hash.finalize().to_hex(),
            });
        }
    }
    let corpus_root = capture.source_digest("ToS/derived-exports/tos_corpus_index.min.json")?;
    let mut membership = Digest256Hasher::new();
    for collection in &collections {
        membership.update(collection.source_graph.as_bytes());
        membership.update(b"\0");
        membership.update(collection.collection.as_bytes());
        membership.update(b"\0");
        membership.update(collection.expected_root_sha256.as_bytes());
        membership.update(b"\0");
    }
    let membership = membership.finalize().to_hex();
    Ok((collections, membership, corpus_root))
}

pub(crate) fn captured_input_roots_owned(
    capture: &PublicCapture,
    vocabulary: &QueryVocabulary,
    state: &CreationState<'_>,
) -> Result<(Vec<InputCollectionReceipt>, String, Digest256)> {
    capture.check_custody()?;
    let count = vocabulary
        .sources
        .len()
        .checked_mul(FAMILY_PROFILE.len() + 1)
        .ok_or(Error::Budget("public D1 receipt collection capacity"))?;
    state.retain(
        count
            .checked_mul(std::mem::size_of::<InputCollectionReceipt>())
            .ok_or(Error::Budget("public D1 receipt collection state"))?,
    )?;
    let mut collections = Vec::with_capacity(count);
    let _db_frame = state.hold(
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
            + std::mem::size_of::<rusqlite::Statement<'_>>()
            + std::mem::size_of::<rusqlite::Rows<'_>>()
            + 4 * std::mem::size_of::<rusqlite::types::ValueRef<'_>>(),
    )?;
    let db = capture.read_db()?;
    for source in &vocabulary.sources {
        let names_cap = FAMILY_PROFILE.len() + 1;
        let _names_hold =
            state.hold(names_cap * (std::mem::size_of::<&str>() + std::mem::size_of::<usize>()))?;
        let mut names: Vec<&str> = Vec::with_capacity(names_cap);
        for (graph, collection, _, _) in FAMILY_PROFILE {
            if *graph == source.source_graph_id.as_str() {
                names.push(collection);
            }
        }
        if source.adapter_profile == "declared-identity-and-source-ref-joins-v1" {
            names.push("join_scope");
        }
        state.charge_work(names.len())?;
        names.sort();
        names.dedup();
        if names.is_empty() {
            return Err(Error::Invalid("public D1 unsupported source adapter"));
        }
        for name in names {
            let mut hash = Digest256Hasher::new();
            let mut count = 0u64;
            let mut stmt = db.prepare("SELECT f.id,coalesce(f.synthetic_sha256,c.sha256) FROM public_family_rows f LEFT JOIN capture_rows c ON c.role=f.input_role AND c.collection=f.input_collection AND c.source_key=f.source_key WHERE f.source_graph=?1 AND f.collection=?2 ORDER BY f.id")?;
            let mut rows = stmt.query(params![source.source_graph_id, name])?;
            while let Some(row) = rows.next()? {
                state.active()?;
                let id = row.get_ref(0)?.as_str().map_err(|_| Error::Invalid("public D1 SQL text"))?;
                let sha = row.get_ref(1)?.as_blob().map_err(|_| Error::Invalid("public D1 SQL blob"))?;
                if sha.len() != 32 {
                    return Err(Error::Invalid("public D1 family digest"));
                }
                capture.charge_work((id.len() + sha.len()) as u64)?;
                root_item(&mut hash, id, sha);
                count = count
                    .checked_add(1)
                    .ok_or(Error::Budget("public D1 receipt rows"))?;
            }
            let bytes = source
                .source_graph_id
                .len()
                .checked_add(name.len())
                .and_then(|n| n.checked_add(source.input_role.len()))
                .and_then(|n| n.checked_add(source.adapter_profile.len()))
                .and_then(|n| n.checked_add(64))
                .ok_or(Error::Budget("public D1 receipt strings"))?;
            state.retain(bytes)?;
            collections.push(InputCollectionReceipt {
                source_graph: source.source_graph_id.clone(),
                collection: name.to_owned(),
                input_role: source.input_role.clone(),
                adapter_profile: source.adapter_profile.clone(),
                expected_count: count,
                expected_root_sha256: hash.finalize().to_hex(),
            });
        }
    }
    let corpus_root = capture.source_digest("ToS/derived-exports/tos_corpus_index.min.json")?;
    let mut membership = Digest256Hasher::new();
    for collection in &collections {
        state.active()?;
        capture.charge_work(
            (collection.source_graph.len()
                + collection.collection.len()
                + collection.expected_root_sha256.len()
                + 3) as u64,
        )?;
        membership.update(collection.source_graph.as_bytes());
        membership.update(b"\0");
        membership.update(collection.collection.as_bytes());
        membership.update(b"\0");
        membership.update(collection.expected_root_sha256.as_bytes());
        membership.update(b"\0");
    }
    state.retain(64)?;
    Ok((collections, membership.finalize().to_hex(), corpus_root))
}

pub(crate) fn exact_receipt(
    capture: &PublicCapture,
    vocabulary: &QueryVocabulary,
    source_revision: &str,
) -> Result<ExactInputReceipt> {
    let (collections, membership, corpus_root) = captured_input_roots(capture, vocabulary)?;
    Ok(ExactInputReceipt {
        binding: SourceBinding {
            owner_profile: "tos-public-projection-snapshot-v1".into(),
            source_cut: format!("public-projection:{source_revision}"),
            through_commit_seq: 0,
            membership_root: membership,
            index_generation: "public-d1-v9".into(),
            route_map_version: "public-d1-v9".into(),
            reader_abi: "public-d1-v9".into(),
            projection_root_sha256: corpus_root.to_hex(),
            complete: true,
        },
        collections,
    })
}

pub(crate) struct PublicStageOwner<'a> {
    pub capture: &'a PublicCapture,
    pub receipt: ExactInputReceipt,
}
impl StageOwner for PublicStageOwner<'_> {
    fn verify_receipt(&self, receipt: &ExactInputReceipt) -> Result<()> {
        let a = &receipt.binding;
        let b = &self.receipt.binding;
        if a.owner_profile != b.owner_profile
            || a.source_cut != b.source_cut
            || a.through_commit_seq != b.through_commit_seq
            || a.membership_root != b.membership_root
            || a.index_generation != b.index_generation
            || a.route_map_version != b.route_map_version
            || a.reader_abi != b.reader_abi
            || a.projection_root_sha256 != b.projection_root_sha256
            || a.complete != b.complete
            || receipt.collections.len() != self.receipt.collections.len()
            || receipt
                .collections
                .iter()
                .zip(&self.receipt.collections)
                .any(|(a, b)| {
                    a.source_graph != b.source_graph
                        || a.collection != b.collection
                        || a.input_role != b.input_role
                        || a.adapter_profile != b.adapter_profile
                        || a.expected_count != b.expected_count
                        || a.expected_root_sha256 != b.expected_root_sha256
                })
        {
            return Err(Error::Invalid("public D1 source snapshot receipt"));
        }
        // The Stage receives only this private captured snapshot. Published
        // source currentness is checked once by the build caller after Stage
        // completion, before any output is made visible.
        self.capture.check_custody()
    }
    fn recheck_sealed_cut(&self, receipt: &ExactInputReceipt) -> Result<()> {
        self.verify_receipt(receipt)
    }
}

pub(crate) fn ingest_family_rows(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    max_work_bytes: u64,
) -> Result<u64> {
    capture.check_custody()?;
    let db = capture.read_db()?;
    let mut statement=db.prepare("SELECT f.source_graph,f.collection,f.id,coalesce(f.synthetic,c.json) FROM public_family_rows f LEFT JOIN capture_rows c ON c.role=f.input_role AND c.collection=f.input_collection AND c.source_key=f.source_key ORDER BY f.source_graph,f.collection,f.id")?;
    let mut rows = statement.query([])?;
    let (max_batch_rows, max_batch_bytes) = stage.input_batch_limits();
    let mut batch = Vec::<(String, String, String, Vec<u8>)>::new();
    let mut batch_bytes = 0u64;
    let mut work = 0u64;
    let mut count = 0u64;
    let flush = |stage: &mut KnowledgeStage<'_>,
                 batch: &mut Vec<(String, String, String, Vec<u8>)>|
     -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let borrowed = batch
            .iter()
            .map(|(source, collection, id, payload)| InputRow {
                source_graph: source,
                collection,
                id,
                payload,
            })
            .collect::<Vec<_>>();
        stage.ingest_input_batch(&borrowed)?;
        batch.clear();
        Ok(())
    };
    while let Some(row) = rows.next()? {
        let source: String = row.get(0)?;
        let collection: String = row.get(1)?;
        let id: String = row.get(2)?;
        let raw: Vec<u8> = row.get(3)?;
        let bytes = (id.len() + raw.len()) as u64;
        work = work
            .checked_add(bytes)
            .filter(|n| *n <= max_work_bytes)
            .ok_or(Error::Budget("public D1 stage transfer work"))?;
        capture.charge_work(bytes)?;
        let next_batch_bytes = batch_bytes
            .checked_add(bytes)
            .ok_or(Error::Budget("public D1 input batch bytes"))?;
        if !batch.is_empty()
            && (batch.len() >= max_batch_rows || next_batch_bytes > max_batch_bytes)
        {
            flush(stage, &mut batch)?;
            batch_bytes = 0;
        }
        if bytes > max_batch_bytes {
            stage.ingest_input(InputRow {
                source_graph: &source,
                collection: &collection,
                id: &id,
                payload: &raw,
            })?;
        } else {
            batch_bytes = batch_bytes
                .checked_add(bytes)
                .ok_or(Error::Budget("public D1 input batch bytes"))?;
            batch.push((source, collection, id, raw));
        }
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("public D1 stage transfer rows"))?;
    }
    flush(stage, &mut batch)?;
    Ok(count)
}

/// Serial borrowed transfer; input rows remain in the held capture cursor,
/// and the existing Stage validates/hashes each original row before insert.
pub(crate) fn ingest_family_rows_owned(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    max_work_bytes: u64,
    state: &CreationState<'_>,
) -> Result<u64> {
    capture.check_custody()?;
    let _db_frame = state.hold(
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
            + std::mem::size_of::<rusqlite::Statement<'_>>()
            + std::mem::size_of::<rusqlite::Rows<'_>>()
            + 4 * std::mem::size_of::<rusqlite::types::ValueRef<'_>>(),
    )?;
    let db = capture.read_db()?;
    let mut stmt = db.prepare("SELECT f.source_graph,f.collection,f.id,coalesce(f.synthetic,c.json) FROM public_family_rows f LEFT JOIN capture_rows c ON c.role=f.input_role AND c.collection=f.input_collection AND c.source_key=f.source_key ORDER BY f.source_graph,f.collection,f.id")?;
    let mut rows = stmt.query([])?;
    let mut count = 0u64;
    let mut work = 0u64;
    while let Some(row) = rows.next()? {
        state.active()?;
        let source = row.get_ref(0)?.as_str().map_err(|_| Error::Invalid("public D1 SQL text"))?;
        let collection = row.get_ref(1)?.as_str().map_err(|_| Error::Invalid("public D1 SQL text"))?;
        let id = row.get_ref(2)?.as_str().map_err(|_| Error::Invalid("public D1 SQL text"))?;
        let payload = row.get_ref(3)?.as_blob().map_err(|_| Error::Invalid("public D1 SQL blob"))?;
        let bytes = id
            .len()
            .checked_add(payload.len())
            .ok_or(Error::Budget("public D1 stage transfer work"))? as u64;
        work = work
            .checked_add(bytes)
            .filter(|n| *n <= max_work_bytes)
            .ok_or(Error::Budget("public D1 stage transfer work"))?;
        capture.charge_work(bytes)?;
        stage.ingest_input(InputRow {
            source_graph: source,
            collection,
            id,
            payload,
        })?;
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("public D1 stage transfer rows"))?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::{family_filter, row_id};
    use serde_json::json;

    #[test]
    fn prepared_family_classifies_unknown_relation_owners_as_candidates() {
        let canonical = json!({"owner_branch":"ToS/canon"});
        let candidate = json!({"owner_branch":"ToS/candidate-intake"});
        let unknown = json!({"owner_branch":"ToS/future"});
        let missing = json!({});

        assert!(family_filter(&canonical, "canon", "relation_edges"));
        assert!(!family_filter(
            &canonical,
            "candidate-intake",
            "relation_edges"
        ));
        for value in [&candidate, &unknown, &missing] {
            assert!(!family_filter(value, "canon", "relation_edges"));
            assert!(family_filter(value, "candidate-intake", "relation_edges"));
        }
    }

    #[test]
    fn prepared_family_uses_python_pack_map_and_edge_identity_rules() {
        assert!(family_filter(
            &json!({"pack_id":" p ","path":" pack.json "}),
            "canon",
            "relation_packs"
        ));
        assert!(!family_filter(
            &json!({"pack_id":"p","path":"  "}),
            "candidate-intake",
            "relation_packs"
        ));
        assert_eq!(
            row_id(
                &json!({"edge_id":" ","id":" edge-a ","pack_id":" pack-a "}),
                "relation_edges",
                0
            )
            .unwrap(),
            "pack-a:edge-a"
        );
        assert_eq!(
            row_id(&json!({"id":"edge-a"}), "relation_edges", 0).unwrap(),
            "edge-a"
        );
        assert_eq!(
            row_id(&json!({"id":"edge-a","pack_id":null}), "relation_edges", 0).unwrap(),
            "edge-a"
        );
        assert!(row_id(&json!({"id":"edge-a","pack_id":false}), "relation_edges", 0).is_err());
    }
}
