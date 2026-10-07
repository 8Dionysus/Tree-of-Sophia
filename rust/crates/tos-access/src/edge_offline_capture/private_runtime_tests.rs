//! Native, tiny private Edge regression scenarios. No subprocess or Python oracle.
//! Fixture writes are test inputs, not source/rights admission or production writers.
use super::*;
use flate2::{Compression, write::GzEncoder};
use std::sync::atomic::{AtomicU64, Ordering};
use tos_compiler::local_prepared::{self, PreparedChange, PreparedRows};
use tos_compiler::local_prepared_aux::{self, AuxInstallLimits};

const REVISION: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const TABLES: &[&str] = &[
    "knowledge_nodes",
    "knowledge_relations",
    "edge_meta",
    "knowledge_lens_order",
    "knowledge_search_documents",
    "knowledge_search_grams",
    "knowledge_search_gram_stats",
    "knowledge_compact_lens",
    "knowledge_lens_memberships",
    "source_navigation_nodes",
    "source_navigation_node_payload",
    "source_navigation_edges",
    "source_navigation_edge_payload",
    "source_navigation_rights",
    "source_navigation_rights_payload",
];
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "tos-private-edge-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn typed(value: &Value) -> JsonValue {
    foundation(value, 8 * 1024 * 1024).unwrap()
}
fn value(value: &JsonValue) -> Value {
    serde_json::from_str(&compact_foundation(value, 8 * 1024 * 1024).unwrap()).unwrap()
}
fn canonical(value: &Value) -> Vec<u8> {
    let mut bytes = canonical_bytes_v1(
        &typed(value),
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::new(8 * 1024 * 1024, 128, 6_500_000, 4300).unwrap(),
    )
    .unwrap();
    bytes.push(b'\n');
    bytes
}
fn node(id: &str, title: &str) -> Value {
    json!({"id":id,"entity_id":format!("{id}-entity"),"native_id":format!("{id}-native"),
        "source_graph":"philosophy","kind_id":"concept","type_id":"tos.entity.concept",
        "type_mapping":{"status":"mapped","source_kind_id":"concept"},"content_revision":"0".repeat(64),
        "display":{"title":{"default":title},"summary":{"default":"Слово λόγος"},"summary_state":"source","kind_label":{"default":"Concept"},"provenance":{"source_summary_available":true}},
        "epistemic":{},"attributes":{},"semantics":{},"graph_layers":["authored"],"view_ids":["main"],"source_refs":["ToS/synthetic.json"]})
}
fn relation() -> Value {
    json!({"id":"r","native_id":"r-native","source_graph":"philosophy","from_id":"a","to_id":"b",
        "predicate_id":"related_to","relation_type_id":"tos.relation.related_to",
        "predicate_mapping":{"status":"mapped","source_predicate_id":"related_to"},"content_revision":"1".repeat(64),
        "display":{"label":{"default":"Related"},"explanation":{"default":"Synthetic"},"explanation_state":"source","provenance":{"source_explanation_available":true}},
        "epistemic":{},"attributes":{},"semantics":{},"graph_layers":["authored"],"view_ids":["main"],"source_refs":["ToS/synthetic.json"]})
}
fn header(revision: char) -> Value {
    json!({"schema":"tos_knowledge_graph_v1","source_revision":revision.to_string().repeat(64),
        "normalization_binding":{"schema":"tos_knowledge_graph_normalization_binding_v1","processor_digest":"b".repeat(64),"entity_registry_digest":"b".repeat(64),"relation_registry_digest":"b".repeat(64),"configuration_digest":"b".repeat(64)},
        "authority_boundary":{"source_owner":"Tree-of-Sophia","is_source":false,"is_canon":false,"writes_to_tree":false},"query_properties":[]})
}
fn catalog(revision: char) -> Value {
    json!({"schema":"tos_knowledge_catalog_v1","source_revision":revision.to_string().repeat(64),"lenses":[]})
}
fn navigation(changed: bool, payload: bool) -> Value {
    let mut row = json!({"node_id":"person","node_kind":"agent","source_ref":"synthetic/person.json",
        "label":if changed {"Исправлено"} else {"Человек — λόγος '"},"identity_status":"provisional","properties":{"unknown":{"false":false,"zero":0.0}}});
    if payload {
        // Use the existing D1 owner boundary, so this fixture remains chunked
        // when that boundary changes. No second numeric threshold lives here.
        let bytes = MAX_D1_SQL_ROW_VALUE_BYTES.checked_add(1).unwrap();
        assert!(bytes <= fixture_search_limits().max_payload_bytes as usize);
        row["properties"]["large"] = json!("x".repeat(bytes));
    }
    let nodes = if changed {
        vec![
            row,
            json!({"node_id":"new","node_kind":"work","source_ref":"","label":"Новое","identity_status":"provisional","properties":{}}),
        ]
    } else {
        vec![row]
    };
    let edges = if changed {
        vec![]
    } else {
        vec![
            json!({"edge_id":"edge","from_id":"person","to_id":"person","edge_kind":"version","predicate_id":"has_record_version","review_status":"unreviewed","source_refs":["synthetic/person.json"]}),
        ]
    };
    json!({"schema_version":"tos_source_navigation_v1","authority_boundary":"synthetic read-only navigation",
        "counts":{"nodes":nodes.len(),"edges":edges.len(),"rights":1},"nodes":nodes,"edges":edges,
        "rights":[{"rights_id":"rights","scope_refs":["person"],"assessment_status":if changed {"restricted"} else {"unknown"},"review_status":"unreviewed"}]})
}

/// Build the actual content-addressed gzip part profile read by D1ProjectionSnapshot.
/// Multiple roots can retain different manifests at the same namespace without replacing bytes.
fn projection(
    directory: &Path,
    name: &str,
    schema: &str,
    header: Value,
    collections: &[(&str, &str, Vec<Value>)],
) -> D1ProjectionSnapshot {
    let mut specs = Map::new();
    for (collection, key_field, rows) in collections {
        let mut rows = rows.clone();
        rows.sort_by(|a, b| {
            a[*key_field]
                .as_str()
                .unwrap()
                .cmp(b[*key_field].as_str().unwrap())
        });
        let decoded: Vec<u8> = rows
            .iter()
            .flat_map(|row| canonical(&json!({"key":row[*key_field],"value":row})))
            .collect();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&decoded).unwrap();
        let stored = encoder.finish().unwrap();
        let sha = digest(&stored);
        let relative = format!(
            "{}.parts/{}/{sha}.jsonl.gz",
            Path::new(name).file_stem().unwrap().to_str().unwrap(),
            &sha[..2]
        );
        let path = directory.join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, &stored).unwrap();
        specs.insert((*collection).into(), json!({"key_field":key_field,"order_fields":[key_field],"root":{"kind":"data","prefix":"","path":relative,"sha256":sha,"size_bytes":stored.len(),"decoded_bytes":decoded.len(),"decoded_sha256":digest(&decoded),"count":rows.len()}}));
    }
    let root = json!({"schema_version":"tos_partitioned_projection_v1","logical_schema":schema,"header":header,
        "limits":{"root_bytes":262144,"index_bytes":131072,"part_bytes":8388608,"key_bytes":4096},"collections":specs});
    D1ProjectionSnapshot::new(canonical(&root), directory.join(name)).unwrap()
}
fn root_binding(root: &D1ProjectionSnapshot) -> Value {
    json!({"namespace_path":root.namespace_path(),"root_json":String::from_utf8(root.root_bytes().to_vec()).unwrap(),"snapshot_sha256":root.snapshot_sha256()})
}
fn root_request(root: &D1ProjectionSnapshot) -> Value {
    json!({"namespace_path":root.namespace_path(),"root_json":String::from_utf8(root.root_bytes().to_vec()).unwrap(),"expected_sha256":root.snapshot_sha256()})
}
struct Rows(Vec<Value>, Vec<Value>);
impl PreparedRows for Rows {
    fn visit(
        &mut self,
        kind: &str,
        sink: &mut dyn FnMut(&JsonValue) -> tos_compiler::Result<()>,
    ) -> tos_compiler::Result<()> {
        for row in if kind == "node" { &self.0 } else { &self.1 } {
            sink(&typed(row))?;
        }
        Ok(())
    }
}
fn metadata(db: &Connection, key: &str) -> String {
    let mut query = db
        .prepare("SELECT json_chunk FROM edge_meta WHERE key=? ORDER BY part")
        .unwrap();
    query
        .query_map([key], |r| r.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect::<String>()
}
fn put_meta(db: &Connection, key: &str, raw: &str) {
    db.execute("DELETE FROM edge_meta WHERE key=?", [key])
        .unwrap();
    db.execute("INSERT INTO edge_meta VALUES(?,0,?)", params![key, raw])
        .unwrap();
}
fn serving(db: &Connection) -> BTreeMap<String, Vec<Vec<SqlValue>>> {
    TABLES
        .iter()
        .map(|table| {
            let mut statement = db
                .prepare(&format!(
                    "SELECT * FROM {table} ORDER BY {}",
                    (1..=db
                        .prepare(&format!("SELECT * FROM {table}"))
                        .unwrap()
                        .column_count())
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                ))
                .unwrap();
            let count = statement.column_count();
            let rows = statement
                .query_map([], |row| {
                    (0..count)
                        .map(|i| row.get(i))
                        .collect::<rusqlite::Result<Vec<SqlValue>>>()
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            ((*table).into(), rows)
        })
        .collect()
}
fn epoch(db: &Connection) -> i64 {
    db.query_row("SELECT epoch FROM knowledge_exploration_clock", [], |r| {
        r.get(0)
    })
    .unwrap()
}
fn auxiliary_current(db: &Connection) {
    let expected =
        auxiliary_binding_candidates(&metadata(db, "knowledge_reader_top"), epoch(db) as u64)
            .unwrap();
    for state in [
        "knowledge_compact_lens_state",
        "knowledge_lens_membership_state",
    ] {
        let (binding, valid): (String, i64) = db
            .query_row(&format!("SELECT binding,valid FROM {state}"), [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(valid, 1);
        assert!(expected.contains(&binding));
    }
}

fn fixture_search_limits() -> SearchBuildLimits {
    SearchBuildLimits {
        max_payload_bytes: 4 * 1024 * 1024,
        max_document_chars: 8_000_000,
        max_document_bytes: 64_000_000,
        max_rank_field_bytes: 8_000_000,
        max_postings: 2_000_000,
        max_work_bytes: 100_000_000,
        gram_batch_rows: 1024,
    }
}
fn assert_projected_successor(f: &Fixture, db: &Connection) {
    let mut postings = 0;
    let mut auxiliary = BTreeMap::<D1Table, usize>::new();
    for (kind, rows) in [("node", &f.rows.0), ("relation", &f.rows.1)] {
        for item in rows {
            let id = item["id"].as_str().unwrap();
            let position: i64 = db
                .query_row(
                    "SELECT position FROM knowledge_search_documents WHERE kind=? AND id=?",
                    params![if kind == "node" { "nodes" } else { "relations" }, id],
                    |r| r.get(0),
                )
                .unwrap();
            let raw = compact(item, 4 * 1024 * 1024).unwrap();
            let mut expected = project_private_knowledge_row(
                kind,
                position,
                &raw,
                "/synthetic",
                fixture_search_limits(),
            )
            .unwrap();
            expected.extend(
                project_private_lens_auxiliary_rows(kind, id, &raw, &typed(item), true, true)
                    .unwrap(),
            );
            for transition in expected {
                if transition.table == D1Table::KnowledgeSearchGrams {
                    postings += 1;
                }
                if matches!(
                    transition.table,
                    D1Table::KnowledgeCompactLens | D1Table::KnowledgeLensMemberships
                ) {
                    *auxiliary.entry(transition.table).or_default() += 1;
                }
                let row = transition.after.unwrap();
                let (name, columns, keys) = transition.table.shape();
                let key = selected_key(transition.table, &row).unwrap();
                let where_sql = keys
                    .iter()
                    .map(|column| format!("{column}=?"))
                    .collect::<Vec<_>>()
                    .join(" AND ");
                let actual: Vec<D1Cell> = db
                    .query_row(
                        &format!("SELECT {} FROM {name} WHERE {where_sql}", columns.join(",")),
                        params_from_iter(key.iter().map(sqlite_value)),
                        |r| {
                            Ok((0..columns.len())
                                .map(|i| match r.get_ref(i).unwrap() {
                                    ValueRef::Null => D1Cell::Null,
                                    ValueRef::Integer(v) => D1Cell::Integer(v),
                                    ValueRef::Text(v) => {
                                        D1Cell::Text(std::str::from_utf8(v).unwrap().into())
                                    }
                                    _ => panic!("unexpected projected storage class"),
                                })
                                .collect())
                        },
                    )
                    .unwrap();
                assert_eq!(actual, row, "{name} {id}");
            }
        }
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM knowledge_search_grams", [], |r| r
            .get::<_, usize>(
            0
        ))
        .unwrap(),
        postings
    );
    for (table, count) in auxiliary {
        assert_eq!(
            db.query_row(
                &format!("SELECT count(*) FROM {}", table.shape().0),
                [],
                |r| r.get::<_, usize>(0)
            )
            .unwrap(),
            count
        );
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM knowledge_nodes", [], |r| r
            .get::<_, usize>(0))
            .unwrap(),
        f.rows.0.len()
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM knowledge_relations", [], |r| r
            .get::<_, usize>(0))
            .unwrap(),
        f.rows.1.len()
    );
}

struct Fixture {
    directory: Directory,
    prepared: PathBuf,
    before: PathBuf,
    d1: PathBuf,
    binding: Value,
    before_binding: Value,
    source: PreparedSourceInputs,
    before_source: PreparedSourceInputs,
    nav: D1ProjectionSnapshot,
    rights: D1ProjectionSnapshot,
    rows: Rows,
    raw_nav: Value,
    separate: bool,
    selected_navigation: bool,
    selected_rights: bool,
    revision: char,
}
impl Fixture {
    fn new(available: bool, separate: bool, payload: bool) -> Self {
        Self::new_selection(available, separate, payload, true, separate)
    }
    fn without_navigation(stable_rights: bool) -> Self {
        Self::new_selection(false, true, false, false, stable_rights)
    }
    // Selection is fixed before the initial prepared publication; no root is
    // removed after a bound publication to manufacture a scope exception.
    fn new_selection(
        available: bool,
        separate: bool,
        payload: bool,
        selected_navigation: bool,
        selected_rights: bool,
    ) -> Self {
        let directory = Directory::new();
        let prepared = directory.0.join("prepared.sqlite");
        let before = directory.0.join("before.sqlite");
        let d1 = directory.0.join("d1.sqlite");
        let rows = Rows(
            vec![node("a", "Common"), node("A", "Upper"), node("b", "Common")],
            vec![relation()],
        );
        let raw_nav = navigation(false, payload);
        let (nav, rights) = Self::navigation_roots(&directory.0, &raw_nav, separate);
        let source = Self::source(
            &directory.0,
            'a',
            selected_navigation.then_some(&nav),
            selected_rights.then_some(&rights),
        );
        let binding = value(
            &local_prepared::publish_prepared_rows(
                &prepared,
                &typed(&header('a')),
                &typed(&catalog('a')),
                &mut Rows(rows.0.clone(), rows.1.clone()),
                PublicationLimits::default(),
            )
            .unwrap(),
        );
        let db = Connection::open(&prepared).unwrap();
        db.execute_batch("CREATE TABLE prepared_source_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),binding TEXT NOT NULL,inputs TEXT NOT NULL,sha256 TEXT NOT NULL)").unwrap();
        Self::bind(&db, &binding, &source);
        drop(db);
        fs::copy(&prepared, &before).unwrap(); // Tiny closed-file fixture copy only.
        let mut fixture = Self {
            directory,
            prepared,
            before,
            d1,
            before_binding: binding.clone(),
            binding,
            before_source: source.clone(),
            source,
            nav,
            rights,
            rows,
            raw_nav,
            separate,
            selected_navigation,
            selected_rights,
            revision: 'a',
        };
        fixture.build_d1(available);
        fixture
    }
    fn navigation_roots(
        directory: &Path,
        raw: &Value,
        separate: bool,
    ) -> (D1ProjectionSnapshot, D1ProjectionSnapshot) {
        let mut top = raw.clone();
        for key in ["nodes", "edges", "rights"] {
            top.as_object_mut().unwrap().remove(key);
        }
        let mut collections = vec![
            ("nodes", "node_id", raw["nodes"].as_array().unwrap().clone()),
            ("edges", "edge_id", raw["edges"].as_array().unwrap().clone()),
        ];
        if !separate {
            collections.push((
                "rights",
                "rights_id",
                raw["rights"].as_array().unwrap().clone(),
            ));
        }
        let nav = projection(
            directory,
            "navigation.json",
            "tos_source_navigation_v1",
            top.clone(),
            &collections,
        );
        let rights = projection(
            directory,
            "rights.json",
            "tos_source_navigation_rights_v1",
            json!({"schema_version":"tos_source_navigation_rights_v1","navigation_header":top}),
            &[(
                "rights",
                "rights_id",
                raw["rights"].as_array().unwrap().clone(),
            )],
        );
        (nav, rights)
    }
    fn source(
        directory: &Path,
        revision: char,
        nav: Option<&D1ProjectionSnapshot>,
        rights: Option<&D1ProjectionSnapshot>,
    ) -> PreparedSourceInputs {
        let catalog_root = projection(
            directory,
            "catalog.json",
            "tos_source_catalog_v2",
            json!({"schema_version":"tos_source_catalog_v2"}),
            &[(
                "records",
                "id",
                vec![json!({"id":"agent","label":"synthetic"})],
            )],
        );
        let claims = projection(
            directory,
            "claims.json",
            "tos_bibliographic_claims_v1",
            json!({"schema_version":"tos_bibliographic_claims_v1"}),
            &[("claims", "id", vec![])],
        );
        // Only participating source roots may advance. The prepared DB owns
        // normalized knowledge; it is not another participating source namespace.
        let mut roots = json!({"source-catalog":root_binding(&catalog_root),"bibliographic-claims":root_binding(&claims)});
        if let Some(nav) = nav {
            roots["source-navigation"] = root_binding(nav);
        }
        if let Some(rights) = rights {
            roots["source-navigation-rights"] = root_binding(rights);
        }
        PreparedSourceInputs::parse(&canonical(&json!({"schema":"tos_prepared_source_inputs_v1","source_revision":revision.to_string().repeat(64),"source_publication":null,"dependencies":{"entity-registry":"b".repeat(64)},"roots":roots})), PublicationLimits::default()).unwrap()
    }
    fn bind(db: &Connection, binding: &Value, source: &PreparedSourceInputs) {
        db.execute(
            "INSERT OR REPLACE INTO prepared_source_state VALUES(1,?,?,?)",
            params![
                String::from_utf8(canonical(binding)).unwrap(),
                std::str::from_utf8(source.raw()).unwrap(),
                source.digest()
            ],
        )
        .unwrap();
    }
    fn serving_connection(&self) -> Connection {
        let db = Connection::open(&self.d1).unwrap();
        // Disposable fixtures exercise logical publish/reverse boundaries, not
        // power-loss durability. Keep commits visible to capture connections
        // without syncing the file for every statement in a D1 SQL batch.
        db.execute_batch("PRAGMA synchronous=OFF").unwrap();
        db
    }
    fn build_d1(&mut self, available: bool) {
        let db = self.serving_connection();
        db.execute_batch("PRAGMA temp_store=MEMORY; BEGIN IMMEDIATE")
            .unwrap();
        // Reuse the maintained table declarations; fixture owns rows only.
        let declarations = include_str!("../../../tos-compiler/src/d1_public_schema.rs");
        for line in declarations.lines() {
            let line = line.trim();
            if line.starts_with("\"CREATE TABLE ") && line.contains("_next") {
                let sql: String = serde_json::from_str(line.strip_suffix(',').unwrap()).unwrap();
                if !sql.contains("knowledge_compact_lens_next")
                    && !sql.contains("knowledge_lens_memberships_next")
                {
                    db.execute_batch(&sql.replace("_next", "")).unwrap();
                }
            }
        }
        let prepared = Connection::open(&self.prepared).unwrap();
        for key in [
            "knowledge_reader_top",
            "knowledge_catalog",
            "knowledge_lens_top",
        ] {
            put_meta(&db, key, &metadata(&prepared, key));
        }
        let mut top =
            foundation_raw(metadata(&db, "knowledge_reader_top").as_bytes(), 1_048_576).unwrap();
        if let JsonValue::Object(fields) = &mut top {
            for (key, value) in fields {
                match key.as_str() {
                    Some("read_model_schema") => {
                        *value = JsonValue::String(JsonString::from_utf8(D1_SCHEMA))
                    }
                    Some("data_revision") => {
                        *value = JsonValue::String(JsonString::from_utf8(REVISION))
                    }
                    _ => {}
                }
            }
        } else {
            panic!("reader top object");
        }
        put_meta(
            &db,
            "knowledge_reader_top",
            &compact_foundation(&top, 1_048_576).unwrap(),
        );
        put_meta(
            &db,
            "data_revision",
            &compact(&json!({"sha256":REVISION}), 256).unwrap(),
        );
        put_meta(
            &db,
            "knowledge_top",
            &compact(&header('a'), 1_048_576).unwrap(),
        );
        put_meta(&db, "knowledge_exploration_top", &compact(&json!({"source_revision":"a".repeat(64),"authority_boundary":header('a')["authority_boundary"]}), 1_048_576).unwrap());
        put_meta(&db, "knowledge_search_top", &compact(&json!({"schema":"tos_knowledge_search_read_model_v3","source_revision":"a".repeat(64),"ngram_size":3,"matching_counts":"unknown-until-indexed-page-exhaustion"}), 1_048_576).unwrap());
        for (kind, rows) in [("node", &self.rows.0), ("relation", &self.rows.1)] {
            let mut ranked = rows.clone();
            ranked.sort_by_key(|r| {
                (
                    r["id"].as_str().unwrap().to_lowercase(),
                    rows.iter().position(|a| a == r).unwrap(),
                )
            });
            for (position, row) in ranked.iter().enumerate() {
                for projected in project_private_knowledge_row(
                    kind,
                    position as i64,
                    &compact(row, 1_048_576).unwrap(),
                    "/synthetic",
                    fixture_search_limits(),
                )
                .unwrap()
                {
                    db.execute(
                        &format!(
                            "INSERT INTO {} VALUES({})",
                            projected.table.shape().0,
                            vec!["?"; projected.table.shape().1.len()].join(",")
                        ),
                        params_from_iter(projected.after.unwrap().iter().map(sqlite_value)),
                    )
                    .unwrap();
                }
            }
        }
        db.execute_batch("INSERT INTO knowledge_search_gram_stats SELECT kind,n,gram,count(*) FROM knowledge_search_grams GROUP BY kind,n,gram;CREATE UNIQUE INDEX knowledge_search_address_id_idx ON knowledge_search_documents(kind,id);CREATE INDEX knowledge_search_address_tie_idx ON knowledge_search_documents(kind,id_lower,position);").unwrap();
        db.execute_batch("COMMIT").unwrap();
        db.execute_batch(include_str!(
            "../../../../../access/deploy/cloudflare-worker/migrations/0001-exploration.sql"
        ))
        .unwrap();
        db.execute_batch("BEGIN IMMEDIATE").unwrap();
        let binding_raw =
            auxiliary_binding_candidates(&metadata(&db, "knowledge_reader_top"), epoch(&db) as u64)
                .unwrap()
                .remove(0);
        let binding = foundation_raw(binding_raw.as_bytes(), 1_048_576).unwrap();
        local_prepared_aux::install_compact(&db, &binding, AuxInstallLimits::default()).unwrap();
        local_prepared_aux::install_membership(&db, &binding, AuxInstallLimits::default()).unwrap();
        db.execute_batch("COMMIT; BEGIN IMMEDIATE").unwrap();
        if available {
            for (kind, rows) in [
                ("nodes", &self.raw_nav["nodes"]),
                ("edges", &self.raw_nav["edges"]),
                ("rights", &self.raw_nav["rights"]),
            ] {
                for (ord, row) in rows.as_array().unwrap().iter().enumerate() {
                    for projected in project_private_navigation_row(
                        kind,
                        ord as i64,
                        &mut typed(row),
                        "/synthetic",
                    )
                    .unwrap()
                    {
                        db.execute(
                            &format!(
                                "INSERT INTO {} VALUES({})",
                                projected.table.shape().0,
                                vec!["?"; projected.table.shape().1.len()].join(",")
                            ),
                            params_from_iter(projected.after.unwrap().iter().map(sqlite_value)),
                        )
                        .unwrap();
                    }
                }
            }
            let raw = compact(&self.nav.metadata().unwrap(), 1_048_576).unwrap();
            put_meta(&db, "source_navigation_top", &raw);
            put_meta(
                &db,
                "source_navigation_header_digest",
                &compact(&json!({"sha256":digest(raw.as_bytes())}), 256).unwrap(),
            );
        } else {
            put_meta(&db, "source_navigation_top", "{}");
        }
        db.execute_batch("COMMIT").unwrap();
        auxiliary_current(&db);
    }
    fn advance(&mut self, navigation_changed: bool, all_changes: bool) {
        self.revision = if self.revision == 'a' { 'c' } else { 'e' };
        let mut changed = self.rows.0.iter().find(|r| r["id"] == "a").unwrap().clone();
        changed["display"]["title"]["default"] = json!(format!("After {}", self.revision));
        changed["attributes"]["opaque"] =
            serde_json::from_str(r#"{"2":9007199254740993,"1":-0.0,"null":null,"false":false}"#)
                .unwrap();
        let mut changes = vec![PreparedChange {
            operation: "update".into(),
            kind: "node".into(),
            identifier: "a".into(),
            item: Some(typed(&changed)),
            source_order: Some(3 * (1u64 << 32)),
        }];
        for row in &mut self.rows.0 {
            if row["id"] == "a" {
                *row = changed.clone();
            }
        }
        if all_changes {
            let new = node("new", "New");
            self.rows.0.push(new.clone());
            self.rows.1.clear();
            changes.push(PreparedChange {
                operation: "insert".into(),
                kind: "node".into(),
                identifier: "new".into(),
                item: Some(typed(&new)),
                source_order: Some(4 * (1u64 << 32)),
            });
            changes.push(PreparedChange {
                operation: "delete".into(),
                kind: "relation".into(),
                identifier: "r".into(),
                item: None,
                source_order: None,
            });
        }
        if navigation_changed {
            let previous_rights = self.raw_nav["rights"].clone();
            self.raw_nav = navigation(
                true,
                self.raw_nav["nodes"][0]["properties"]
                    .get("large")
                    .is_some(),
            );
            if self.separate {
                self.raw_nav["rights"] = previous_rights;
            }
        }
        let (nav, rights) = Self::navigation_roots(&self.directory.0, &self.raw_nav, self.separate);
        self.nav = nav;
        // Separate rights are nonparticipating: retain their exact root/header
        // bytes, even when the navigation node/edge counts change.
        if !self.separate {
            self.rights = rights;
        }
        self.source = Self::source(
            &self.directory.0,
            self.revision,
            self.selected_navigation.then_some(&self.nav),
            self.selected_rights.then_some(&self.rights),
        );
        source_scope_compatible(
            &source_inputs_value(&self.before_source).unwrap(),
            &source_inputs_value(&self.source).unwrap(),
        )
        .unwrap();
        let db = Connection::open(&self.prepared).unwrap();
        db.execute_batch("BEGIN IMMEDIATE").unwrap();
        self.binding = value(
            &local_prepared::apply_prepared_delta_transaction(
                &db,
                &typed(&self.binding),
                &typed(&header(self.revision)),
                &typed(&catalog(self.revision)),
                changes,
                PublicationLimits::default(),
            )
            .unwrap(),
        );
        Self::bind(&db, &self.binding, &self.source);
        db.execute_batch("COMMIT").unwrap();
    }
    fn request(&self, operation: &str, name: &str) -> Value {
        let mut request = json!({"schema":REQUEST_SCHEMA,"operation":operation,"d1_database":self.d1,"before_prepared_database":self.before,"after_prepared_database":self.prepared,
            "expected_d1_revision":REVISION,"before_binding":self.before_binding,"after_binding":self.binding,"before_source_inputs_json":null,"rights_root":null,
            "forward_sql":self.directory.0.join(format!("{name}.sql")),"rollback_sql":self.directory.0.join(format!("{name}.reverse.sql")),"manifest_json":self.directory.0.join(format!("{name}.json")),"limits":{"prepared":{
                "max_changes":512,"max_row_bytes":4194304,"max_metadata_bytes":8388608,"max_read_bytes":67108864,"max_rows":200000,"max_retained_bytes":33554432,"max_sql_bytes":268435456,"max_postings":2000000,"max_manifest_rows":200000}}});
        if operation == "prepared-catchup" {
            request["before_prepared_database"] = Value::Null;
            request["before_binding"] = Value::Null;
            request["before_source_inputs_json"] =
                json!(std::str::from_utf8(self.before_source.raw()).unwrap());
        }
        if operation == "source-navigation-bootstrap" {
            request["before_prepared_database"] = Value::Null;
            request["before_binding"] = Value::Null;
            let mut rights = root_request(&self.rights);
            rights["trusted_sha256"] = json!(self.rights.snapshot_sha256());
            request["rights_root"] = rights;
            add_projection_limits(&mut request);
        }
        request
    }
}
fn add_projection_limits(request: &mut Value) {
    request["limits"]["projection"] = json!({"max_changes":512,"max_input_bytes":1048576,"max_opened_parts":256,"max_stored_read_bytes":134217728,"max_decoded_bytes":134217728,"max_keys":4096,"max_written_parts":256,"max_written_decoded_bytes":134217728,"max_written_stored_bytes":134217728,"max_result_bytes":16777216});
}
fn capture(request: &Value) -> Result<Value, String> {
    let mut output = Vec::new();
    run_request(
        &serde_json::to_vec(request).unwrap(),
        &mut output,
        request["schema"] == TYPED_REQUEST_SCHEMA,
    )?;
    let envelope: Value = serde_json::from_slice(&output).map_err(|e| e.to_string())?;
    assert_eq!(
        envelope["schema"],
        json!(result_schema(request["schema"].as_str().unwrap()))
    );
    assert_eq!(envelope["operation"], request["operation"]);
    let receipt = envelope["receipt"]
        .as_object()
        .expect("native capture result must contain an object receipt");
    Ok(Value::Object(receipt.clone()))
}
fn sql(request: &Value, field: &str) -> String {
    fs::read_to_string(request[field].as_str().unwrap()).unwrap()
}
fn refused(request: &Value) -> String {
    let db = Connection::open(request["d1_database"].as_str().unwrap()).unwrap();
    let before = serving(&db);
    let error = capture(request).unwrap_err();
    // A fixture admission/lineage bug must not masquerade as the intended guard.
    for unrelated in [
        "nonparticipating prepared source",
        "required prepared projection root absent",
        "prepared source-navigation root absent",
        "prepared header profile changed",
        "D1 and prepared predecessor reader profile differs",
        "D1 prepared predecessor catalog/lens differs",
        "one exact prepared parent/successor transition required",
        "offline prepared source pair",
        "private source inputs",
        "D1 projection manifest",
        "capture request fields",
        "capture limits shape",
        "limit fields",
    ] {
        assert!(
            !error.contains(unrelated),
            "fixture construction error masked intended refusal: {error}"
        );
    }
    assert_eq!(serving(&db), before);
    for field in ["forward_sql", "rollback_sql", "manifest_json"] {
        assert!(
            !Path::new(request[field].as_str().unwrap()).exists(),
            "{field} published after refusal"
        );
    }
    error
}

fn refused_category(request: &Value, expected: &[&str]) {
    let error = refused(request);
    assert!(
        expected.iter().any(|category| error.contains(category)),
        "unexpected refusal category: {error}; expected {expected:?}"
    );
}
fn budget_refused(request: &Value) {
    let error = refused(request);
    // Native compiler Budget variants retain their Debug case through the
    // string-only CLI boundary; other owners retain their declared lower-case codes.
    assert!(
        error.contains("Budget(")
            || ["budget", "limit", "prepared delta frame count"]
                .iter()
                .any(|code| error.contains(code)),
        "expected a native budget refusal, received: {error}"
    );
}

#[test]
fn prepared_delta_add_update_delete_order_opaque_replay_reverse_and_auxiliary() {
    let mut f = Fixture::new(true, false, false);
    f.advance(false, true);
    let db = f.serving_connection();
    let original = serving(&db);
    let initial_epoch = epoch(&db);
    let request = f.request("prepared-delta", "delta");
    eprintln!("prepared delta phase: capture");
    let receipt = capture(&request).unwrap();
    eprintln!("prepared delta phase: capture complete");
    assert_eq!(receipt["d1_applied"], false);
    assert_eq!(serving(&db), original);
    let forward = sql(&request, "forward_sql");
    let reverse = sql(&request, "rollback_sql");
    eprintln!("prepared delta phase: forward SQL");
    db.execute_batch(&forward).unwrap();
    eprintln!("prepared delta phase: forward SQL complete");
    auxiliary_current(&db);
    assert_projected_successor(&f, &db);
    let published = serving(&db);
    let advanced = epoch(&db);
    assert!(advanced > initial_epoch);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM knowledge_nodes WHERE id='new'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM knowledge_relations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let raw: String = db
        .query_row("SELECT json FROM knowledge_nodes WHERE id='a'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(raw.contains("9007199254740993"));
    assert!(raw.contains("-0.0"));
    let ranking:Vec<String>=db.prepare("SELECT id FROM knowledge_search_documents WHERE kind='nodes' ORDER BY id_lower,position").unwrap().query_map([],|r|r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(ranking, vec!["A", "a", "b", "new"]);
    eprintln!("prepared delta phase: forward SQL");
    db.execute_batch(&forward).unwrap();
    eprintln!("prepared delta phase: forward SQL complete");
    assert_eq!(serving(&db), published);
    assert_eq!(epoch(&db), advanced);
    eprintln!("prepared delta phase: reverse SQL");
    db.execute_batch(&reverse).unwrap();
    eprintln!("prepared delta phase: reverse SQL complete");
    auxiliary_current(&db);
    assert_eq!(serving(&db), original);
    let reversed = epoch(&db);
    assert!(reversed > advanced);
    eprintln!("prepared delta phase: reverse SQL");
    db.execute_batch(&reverse).unwrap();
    eprintln!("prepared delta phase: reverse SQL complete");
    assert_eq!(epoch(&db), reversed);
    eprintln!("prepared delta phase: forward SQL");
    db.execute_batch(&forward).unwrap();
    eprintln!("prepared delta phase: forward SQL complete");
    assert_eq!(serving(&db), published);
    auxiliary_current(&db);
    let prepared = Connection::open(&f.prepared).unwrap();
    assert_eq!(
        prepared
            .query_row(
                "SELECT count(*) FROM knowledge_nodes WHERE id='new'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn delta_incomplete_staging_and_intervening_auxiliary_invalidation_are_atomic() {
    let mut f = Fixture::new(true, false, false);
    f.advance(false, true);
    let request = f.request("prepared-delta", "atomic");
    let receipt = capture(&request).unwrap();
    let forward = sql(&request, "forward_sql");
    let (stage, publish) = forward
        .split_once("INSERT INTO tos_delta_publications SELECT")
        .unwrap();
    let db = f.serving_connection();
    let original = serving(&db);
    db.execute_batch(stage).unwrap();
    assert_eq!(serving(&db), original);
    let staged = format!(
        "tos_rust_d1_{}_knowledge_nodes_keys",
        &receipt["target_d1_revision"].as_str().unwrap()[..12]
    );
    assert_eq!(
        db.execute(&format!("DELETE FROM {staged} WHERE id='new'"), [])
            .unwrap(),
        1
    );
    let error = db
        .execute_batch(&format!(
            "INSERT INTO tos_delta_publications SELECT{publish}"
        ))
        .unwrap_err();
    assert!(
        error.to_string().contains("incomplete D1 stage"),
        "unexpected publication refusal: {error}"
    );
    assert_eq!(serving(&db), original);
    db.execute_batch("ROLLBACK").ok();
    db.execute_batch(&forward).unwrap();
    db.execute("UPDATE knowledge_lens_membership_state SET valid=0", [])
        .unwrap();
    let current = serving(&db);
    assert!(db.execute_batch(&sql(&request, "rollback_sql")).is_err());
    assert_eq!(serving(&db), current);
}

#[test]
fn delta_stale_rows_and_independent_capture_budgets_refuse_before_outputs() {
    for field in [
        "max_read_bytes",
        "max_rows",
        "max_retained_bytes",
        "max_sql_bytes",
        "max_postings",
    ] {
        let mut f = Fixture::new(true, false, false);
        f.advance(false, true);
        let mut request = f.request("prepared-delta", field);
        request["limits"]["prepared"][field] = json!(1);
        budget_refused(&request);
    }
    let mut f = Fixture::new(true, false, false);
    f.advance(false, true);
    let db = f.serving_connection();
    db.execute("UPDATE knowledge_nodes SET json='{}' WHERE id='a'", [])
        .unwrap();
    let error = refused(&f.request("prepared-delta", "stale"));
    assert!(
        error.contains("D1 predecessor projected row differs"),
        "unexpected stale-row refusal: {error}"
    );
}

#[test]
fn catchup_reconciles_multiple_prepared_steps_and_replays_exact_reverse() {
    let mut f = Fixture::new(true, false, false);
    f.advance(false, true);
    f.advance(false, false);
    let request = f.request("prepared-catchup", "catchup");
    let db = f.serving_connection();
    let original = serving(&db);
    capture(&request).unwrap();
    db.execute_batch(&sql(&request, "forward_sql")).unwrap();
    let published = serving(&db);
    auxiliary_current(&db);
    assert_projected_successor(&f, &db);
    db.execute_batch(&sql(&request, "forward_sql")).unwrap();
    assert_eq!(serving(&db), published);
    db.execute_batch(&sql(&request, "rollback_sql")).unwrap();
    assert_eq!(serving(&db), original);
    auxiliary_current(&db);
    let prepared = Connection::open(&f.prepared).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&metadata(&prepared, "knowledge_reader_top")).unwrap()["source_revision"],
        json!("e".repeat(64))
    );
}

#[test]
fn prepared_pair_and_catchup_without_navigation_keep_complete_stable_source_inventory() {
    for stable_rights in [false, true] {
        for operation in ["prepared-delta", "prepared-catchup"] {
            let mut f = Fixture::without_navigation(stable_rights);
            let initial = source_inputs_value(&f.source).unwrap();
            assert!(initial["roots"].get("source-navigation").is_none());
            assert_eq!(
                initial["roots"].get("source-navigation-rights").is_some(),
                stable_rights
            );
            let rights_bytes = f.rights.root_bytes().to_vec();
            f.advance(false, true);
            if operation == "prepared-catchup" {
                f.advance(false, false);
            }
            let successor = source_inputs_value(&f.source).unwrap();
            // The selected inventory, including an unrelated rights namespace,
            // remains fully bound; only the prepared source revision advances.
            assert_eq!(successor["roots"], initial["roots"]);
            assert_eq!(successor["dependencies"], initial["dependencies"]);
            assert_eq!(f.rights.root_bytes(), rights_bytes.as_slice());
            let request = f.request(operation, "no-navigation");
            let db = f.serving_connection();
            let original = serving(&db);
            let receipt = capture(&request).unwrap();
            assert_eq!(receipt["d1_applied"], false);
            assert_eq!(serving(&db), original);
            let forward = sql(&request, "forward_sql");
            db.execute_batch(&forward).unwrap();
            assert_projected_successor(&f, &db);
            auxiliary_current(&db);
            let published = serving(&db);
            db.execute_batch(&forward).unwrap();
            assert_eq!(serving(&db), published);
            assert_eq!(metadata(&db, "source_navigation_top"), "{}");
            for table in [
                "source_navigation_nodes",
                "source_navigation_edges",
                "source_navigation_rights",
                "source_navigation_node_payload",
                "source_navigation_edge_payload",
                "source_navigation_rights_payload",
            ] {
                assert_eq!(
                    db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
            }
            db.execute_batch(&sql(&request, "rollback_sql")).unwrap();
            assert_eq!(serving(&db), original);
            auxiliary_current(&db);
            // Explicit navigation-delta still requires navigation. This one
            // deliberate absence is not a fixture-construction failure.
            // Use the exact one-step Pair fixture for this explicit refusal;
            // a multistep CatchUp predecessor would also fail parent lineage.
            if operation == "prepared-delta" {
                let required = f.request("source-navigation-delta", "required-navigation");
                let error = capture(&required).unwrap_err();
                assert_eq!(
                    error,
                    "source-navigation delta requires selected navigation roots"
                );
                assert!(
                    !error.contains("nonparticipating"),
                    "unexpected scope failure: {error}"
                );
                assert_eq!(serving(&db), original);
                for field in ["forward_sql", "rollback_sql", "manifest_json"] {
                    assert!(!Path::new(required[field].as_str().unwrap()).exists());
                }
            }
        }
    }
}

#[test]
fn catchup_incomplete_malformed_orphan_manifests_and_source_mismatch_refuse() {
    for mutation in [
        "DELETE FROM edge_meta WHERE key='knowledge_node_digest:a'",
        "UPDATE edge_meta SET json_chunk='{' WHERE key='knowledge_node_digest:a'",
        "INSERT INTO edge_meta VALUES('knowledge_node_digest:orphan',0,'{\"sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}')",
    ] {
        let mut f = Fixture::new(true, false, false);
        f.advance(false, true);
        let db = f.serving_connection();
        db.execute_batch(mutation).unwrap();
        refused_category(&f.request("prepared-catchup", "manifest"), &["manifest"]);
    }
    for field in ["max_manifest_rows", "max_changes", "max_read_bytes"] {
        let mut f = Fixture::new(true, false, false);
        f.advance(false, true);
        let mut r = f.request("prepared-catchup", field);
        r["limits"]["prepared"][field] = json!(1);
        budget_refused(&r);
    }
    let mut f = Fixture::new(true, false, false);
    f.advance(false, true);
    let mut r = f.request("prepared-catchup", "source");
    let mut source: Value =
        serde_json::from_str(r["before_source_inputs_json"].as_str().unwrap()).unwrap();
    source["source_revision"] = json!("f".repeat(64));
    r["before_source_inputs_json"] = json!(String::from_utf8(canonical(&source)).unwrap());
    refused_category(
        &r,
        &[
            "source or revision differs",
            "source selection",
            "source revision",
        ],
    );
}

#[test]
fn catchup_source_order_only_case_tie_changes_without_digest_changes() {
    let mut f = Fixture::new(true, false, false);
    let old_digest = metadata(
        &Connection::open(&f.prepared).unwrap(),
        "knowledge_node_digest:a",
    );
    let row = f.rows.0[0].clone();
    f.revision = 'c';
    let db = Connection::open(&f.prepared).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    f.binding = value(
        &local_prepared::apply_prepared_delta_transaction(
            &db,
            &typed(&f.binding),
            &typed(&header('c')),
            &typed(&catalog('c')),
            [PreparedChange {
                operation: "update".into(),
                kind: "node".into(),
                identifier: "a".into(),
                item: Some(typed(&row)),
                source_order: Some(4 * (1u64 << 32)),
            }],
            PublicationLimits::default(),
        )
        .unwrap(),
    );
    f.source = Fixture::source(
        &f.directory.0,
        'c',
        f.selected_navigation.then_some(&f.nav),
        f.selected_rights.then_some(&f.rights),
    );
    Fixture::bind(&db, &f.binding, &f.source);
    db.execute_batch("COMMIT").unwrap();
    assert_eq!(metadata(&db, "knowledge_node_digest:a"), old_digest);
    let request = f.request("prepared-catchup", "order-only");
    capture(&request).unwrap();
    let d1 = f.serving_connection();
    let original = serving(&d1);
    d1.execute_batch(&sql(&request, "forward_sql")).unwrap();
    let ranking:Vec<String>=d1.prepare("SELECT id FROM knowledge_search_documents WHERE kind='nodes' ORDER BY id_lower,position").unwrap().query_map([],|r|r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(ranking, vec!["A", "a", "b"]);
    assert_projected_successor(&f, &d1);
    d1.execute_batch(&sql(&request, "rollback_sql")).unwrap();
    assert_eq!(serving(&d1), original);
}

#[test]
fn navigation_delta_joint_inventory_payload_rights_and_exact_reverse() {
    let mut f = Fixture::new(true, false, true);
    f.advance(true, true);
    let request = f.request("source-navigation-delta", "navigation");
    let db = f.serving_connection();
    let original = serving(&db);
    assert!(
        db.query_row(
            "SELECT count(*) FROM source_navigation_node_payload",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap()
            > 0
    );
    capture(&request).unwrap();
    let forward = sql(&request, "forward_sql");
    let (stage, publish) = forward
        .split_once("INSERT INTO tos_delta_publications SELECT")
        .unwrap();
    db.execute_batch(stage).unwrap();
    assert_eq!(serving(&db), original);
    db.execute_batch(&format!(
        "INSERT INTO tos_delta_publications SELECT{publish}"
    ))
    .unwrap();
    db.execute_batch(&forward).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM source_navigation_nodes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM source_navigation_edges", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let rights: String = db
        .query_row("SELECT json FROM source_navigation_rights", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&rights).unwrap()["assessment_status"],
        "restricted"
    );
    db.execute_batch(&sql(&request, "rollback_sql")).unwrap();
    assert_eq!(serving(&db), original);
    auxiliary_current(&db);
}

#[test]
fn navigation_delta_unavailable_remains_empty_and_payload_tamper_refuses() {
    let mut f = Fixture::new(false, false, false);
    f.advance(true, true);
    let request = f.request("source-navigation-delta", "unavailable");
    capture(&request).unwrap();
    let db = f.serving_connection();
    db.execute_batch(&sql(&request, "forward_sql")).unwrap();
    assert_eq!(metadata(&db, "source_navigation_top"), "{}");
    assert_eq!(
        db.query_row("SELECT count(*) FROM source_navigation_nodes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let mut f = Fixture::new(true, false, true);
    f.advance(true, true);
    let db = f.serving_connection();
    db.execute(
        "DELETE FROM source_navigation_node_payload WHERE part=0",
        [],
    )
    .unwrap();
    refused_category(
        &f.request("source-navigation-delta", "payload-tamper"),
        &["payload", "digest"],
    );
}

#[test]
fn navigation_delta_header_digest_inventory_and_budget_guards_refuse() {
    for mutation in [
        "DELETE FROM edge_meta WHERE key='source_navigation_header_digest'",
        "UPDATE edge_meta SET json_chunk='{\"sha256\":\"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\"}' WHERE key='source_navigation_header_digest'",
        "DELETE FROM edge_meta WHERE key='source_navigation_row_digest:nodes:38a81e87e79631e602bf5fbd307ce2fcd382b1670c585ea09032aac778a80531'",
        "INSERT INTO edge_meta VALUES('source_navigation_row_digest:nodes:88f6811ab5d8fc6d3177f9b7609ae0fcebfda187e5046b62d38bb539e88b74d7',0,'{\"sha256\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}')",
        "DELETE FROM source_navigation_edges WHERE edge_id='edge'",
        "UPDATE edge_meta SET json_chunk='{}' WHERE key='source_navigation_top'",
    ] {
        let mut f = Fixture::new(true, false, false);
        f.advance(true, true);
        let db = f.serving_connection();
        db.execute_batch(mutation).unwrap();
        assert_eq!(
            db.changes(),
            1,
            "guard mutation did not affect the selected D1 fixture: {mutation}"
        );
        refused_category(
            &f.request("source-navigation-delta", "guard"),
            &["navigation", "metadata key absent"],
        );
    }
    for field in [
        "max_changes",
        "max_read_bytes",
        "max_retained_bytes",
        "max_rows",
    ] {
        let mut f = Fixture::new(true, false, true);
        f.advance(true, true);
        let mut r = f.request("source-navigation-delta", field);
        r["limits"]["prepared"][field] = json!(1);
        budget_refused(&r);
    }
}

#[test]
fn bootstrap_native_product_replay_reverse_and_subsequent_delta() {
    let mut f = Fixture::new(false, true, true);
    let request = f.request("source-navigation-bootstrap", "bootstrap");
    let db = f.serving_connection();
    let original = serving(&db);
    capture(&request).unwrap();
    db.execute_batch(&sql(&request, "forward_sql")).unwrap();
    let bootstrapped = serving(&db);
    let revision =
        serde_json::from_str::<Value>(&metadata(&db, "data_revision")).unwrap()["sha256"].clone();
    db.execute_batch(&sql(&request, "forward_sql")).unwrap();
    assert_eq!(serving(&db), bootstrapped);
    db.execute_batch(&sql(&request, "rollback_sql")).unwrap();
    assert_eq!(serving(&db), original);
    db.execute_batch(&sql(&request, "forward_sql")).unwrap();
    f.advance(true, false);
    let mut delta = f.request("source-navigation-delta", "after-bootstrap");
    delta["expected_d1_revision"] = revision;
    capture(&delta).unwrap();
    db.execute_batch(&sql(&delta, "forward_sql")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM source_navigation_nodes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    db.execute_batch(&sql(&delta, "rollback_sql")).unwrap();
    assert_eq!(serving(&db), bootstrapped);
}

#[test]
fn bootstrap_occupied_intervening_rights_tamper_and_projection_budgets_refuse() {
    let f = Fixture::new(true, true, false);
    refused_category(
        &f.request("source-navigation-bootstrap", "occupied"),
        &["not absent", "already present", "not empty"],
    );
    let f = Fixture::new(false, true, false);
    let mut request = f.request("source-navigation-bootstrap", "rights");
    request["rights_root"]["trusted_sha256"] = json!("f".repeat(64));
    refused_category(&request, &["rights expected and trusted digests differ"]);
    for field in [
        "max_opened_parts",
        "max_stored_read_bytes",
        "max_decoded_bytes",
        "max_keys",
    ] {
        let f = Fixture::new(false, true, false);
        let mut r = f.request("source-navigation-bootstrap", field);
        r["limits"]["projection"][field] = json!(1);
        budget_refused(&r);
    }
    let f = Fixture::new(false, true, false);
    let request = f.request("source-navigation-bootstrap", "intervening");
    capture(&request).unwrap();
    let db = f.serving_connection();
    db.execute("INSERT INTO source_navigation_nodes VALUES('intruder',0,'agent','','','provisional','{}','{}')",[]).unwrap();
    let prior = serving(&db);
    assert!(db.execute_batch(&sql(&request, "forward_sql")).is_err());
    assert_eq!(serving(&db), prior);
}

#[test]
fn bootstrap_tampered_rights_part_and_complete_product_row_budget_refuse() {
    let f = Fixture::new(false, true, false);
    let mut r = f.request("source-navigation-bootstrap", "row-budget");
    r["limits"]["prepared"]["max_rows"] = json!(1);
    budget_refused(&r);
    let f = Fixture::new(false, true, false);
    let root: Value = serde_json::from_slice(f.rights.root_bytes()).unwrap();
    let part = f.rights.namespace_path().parent().unwrap().join(
        root["collections"]["rights"]["root"]["path"]
            .as_str()
            .unwrap(),
    );
    let mut bytes = fs::read(&part).unwrap();
    bytes[0] ^= 1;
    fs::write(&part, bytes).unwrap();
    refused_category(
        &f.request("source-navigation-bootstrap", "tampered-part"),
        &["stored part digest"],
    );
}

#[test]
fn navigation_integrity_header_only_migration_and_row_inventory_refusal() {
    let f = Fixture::new(true, true, true);
    let db = f.serving_connection();
    db.execute(
        "DELETE FROM edge_meta WHERE key='source_navigation_header_digest'",
        [],
    )
    .unwrap();
    let mut request = json!({"schema":REQUEST_SCHEMA,"operation":"source-navigation-integrity","d1_database":f.d1,"expected_d1_revision":REVISION,"expected_source_revision":"a".repeat(64),"navigation_root":root_request(&f.nav),"rights_root":root_request(&f.rights),"header_only":true,"forward_sql":f.directory.0.join("integrity.sql"),"rollback_sql":f.directory.0.join("integrity.reverse.sql"),"manifest_json":f.directory.0.join("integrity.json"),"limits":f.request("prepared-delta","unused")["limits"]});
    add_projection_limits(&mut request);
    capture(&request).unwrap();
    let original = serving(&db);
    db.execute_batch(&sql(&request, "forward_sql")).unwrap();
    assert!(!metadata(&db, "source_navigation_header_digest").is_empty());
    db.execute_batch(&sql(&request, "rollback_sql")).unwrap();
    assert_eq!(serving(&db), original);
    request["header_only"] = json!(false);
    for (field, suffix) in [
        ("forward_sql", "sql"),
        ("rollback_sql", "reverse.sql"),
        ("manifest_json", "json"),
    ] {
        request[field] = json!(f.directory.0.join(format!("tampered.{suffix}")));
    }
    db.execute(
        "DELETE FROM source_navigation_node_payload WHERE part=0",
        [],
    )
    .unwrap();
    refused_category(&request, &["payload", "navigation", "digest"]);
}

#[test]
fn navigation_full_integrity_inventory_migration_then_delta_and_forged_header_refusal() {
    let mut f = Fixture::new(true, true, false);
    let db = f.serving_connection();
    db.execute("DELETE FROM edge_meta WHERE key GLOB 'source_navigation_row_digest:*' OR key='source_navigation_header_digest'",[]).unwrap();
    let original = serving(&db);
    let mut request = json!({"schema":REQUEST_SCHEMA,"operation":"source-navigation-integrity","d1_database":f.d1,"expected_d1_revision":REVISION,"expected_source_revision":"a".repeat(64),"navigation_root":root_request(&f.nav),"rights_root":root_request(&f.rights),"header_only":false,"forward_sql":f.directory.0.join("full-integrity.sql"),"rollback_sql":f.directory.0.join("full-integrity.reverse.sql"),"manifest_json":f.directory.0.join("full-integrity.json"),"limits":f.request("prepared-delta","unused")["limits"]});
    add_projection_limits(&mut request);
    capture(&request).unwrap();
    db.execute_batch(&sql(&request, "forward_sql")).unwrap();
    let migrated = serving(&db);
    let revision =
        serde_json::from_str::<Value>(&metadata(&db, "data_revision")).unwrap()["sha256"].clone();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM edge_meta WHERE key GLOB 'source_navigation_row_digest:*'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        3
    );
    db.execute_batch(&sql(&request, "rollback_sql")).unwrap();
    assert_eq!(serving(&db), original);
    db.execute_batch(&sql(&request, "forward_sql")).unwrap();
    f.advance(true, false);
    let mut delta = f.request("source-navigation-delta", "after-integrity");
    delta["expected_d1_revision"] = revision;
    capture(&delta).unwrap();
    db.execute_batch(&sql(&delta, "forward_sql")).unwrap();
    db.execute_batch(&sql(&delta, "rollback_sql")).unwrap();
    assert_eq!(serving(&db), migrated);
    // Header-only mode still checks the visible authority policy of the chosen root.
    let g = Fixture::new(true, true, false);
    let mut forged: Value = serde_json::from_slice(g.nav.root_bytes()).unwrap();
    forged["header"]["authority_boundary"] = json!("forged authority");
    let forged =
        D1ProjectionSnapshot::new(canonical(&forged), g.nav.namespace_path().to_path_buf())
            .unwrap();
    request["d1_database"] = json!(g.d1);
    request["expected_d1_revision"] = json!(REVISION);
    request["navigation_root"] = root_request(&forged);
    request["rights_root"] = root_request(&g.rights);
    request["header_only"] = json!(true);
    for (field, suffix) in [
        ("forward_sql", "sql"),
        ("rollback_sql", "reverse.sql"),
        ("manifest_json", "json"),
    ] {
        request[field] = json!(g.directory.0.join(format!("forged.{suffix}")));
    }
    refused_category(&request, &["header", "authority", "navigation"]);
}

#[test]
fn navigation_integrity_persisted_rights_drift_and_read_budget_refuse() {
    for small_budget in [false, true] {
        let f = Fixture::new(true, true, false);
        let db = f.serving_connection();
        // Integrity migration starts at the pre-companion product. Existing
        // companions correctly refuse blind refresh before checking row drift.
        db.execute("DELETE FROM edge_meta WHERE key GLOB 'source_navigation_row_digest:*' OR key='source_navigation_header_digest'", []).unwrap();
        if !small_budget {
            db.execute("UPDATE source_navigation_rights SET json='{}'", [])
                .unwrap();
        }
        let mut request = json!({"schema":REQUEST_SCHEMA,"operation":"source-navigation-integrity","d1_database":f.d1,"expected_d1_revision":REVISION,"expected_source_revision":"a".repeat(64),"navigation_root":root_request(&f.nav),"rights_root":root_request(&f.rights),"header_only":false,"forward_sql":f.directory.0.join("integrity-guard.sql"),"rollback_sql":f.directory.0.join("integrity-guard.reverse.sql"),"manifest_json":f.directory.0.join("integrity-guard.json"),"limits":f.request("prepared-delta","unused")["limits"]});
        add_projection_limits(&mut request);
        if small_budget {
            request["limits"]["prepared"]["max_read_bytes"] = json!(1);
        }
        if small_budget {
            budget_refused(&request);
        } else {
            refused_category(&request, &["rights", "navigation", "digest", "projected"]);
        }
    }
}

/// Production encoder with a fixture-owned interrupt watchdog. The source
/// transaction was selected by the test and remains borrowed/exclusively used;
/// no test-only transport profile or cell encoder remains here.
fn held_frame(
    db: &Connection,
    role: typed_snapshot::Role,
    input_field: &str,
    path: &Path,
    budget: &mut typed_snapshot::EncodeBudget,
) -> Value {
    assert!(
        !db.is_autocommit(),
        "frame requires a genuinely held transaction"
    );
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    let duration = std::time::Duration::from_secs(10);
    let deadline = std::time::Instant::now() + duration;
    let interrupt = db.get_interrupt_handle();
    let (finish, stopped) = std::sync::mpsc::channel();
    let result = std::thread::scope(|scope| {
        scope.spawn(move || {
            if matches!(
                stopped.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now())),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ) {
                interrupt.interrupt();
            }
        });
        let result = typed_snapshot::encode_borrowed(
            db,
            role,
            input_field,
            &mut output,
            budget,
            deadline,
            &|| Ok(()),
        );
        let _ = finish.send(());
        result
    })
    .unwrap();
    assert!(
        !db.is_autocommit(),
        "encoder released the selected transaction"
    );
    assert_eq!(
        result["frame_bytes"],
        json!(fs::metadata(path).unwrap().len())
    );
    assert_eq!(
        result["frame_sha256"],
        json!(Digest256::of_bytes(&fs::read(path).unwrap()).to_hex())
    );
    result
}

#[test]
fn production_encoder_import_preserves_uncommitted_utf16_text_views() {
    for (encoding, raw) in [
        (
            "UTF-16le",
            vec![0xff, 0xfe, 0x41, 0x00, 0x00, 0x00, 0x3d, 0xd8, 0x00, 0xde],
        ),
        (
            "UTF-16be",
            vec![0xfe, 0xff, 0x00, 0x41, 0x00, 0x00, 0xd8, 0x3d, 0xde, 0x00],
        ),
    ] {
        let directory = Directory::new();
        let database = directory.0.join("uncommitted.sqlite");
        let frame = directory.0.join("uncommitted.frame");
        let source = Connection::open(&database).unwrap();
        source.pragma_update(None, "encoding", encoding).unwrap();
        source.execute_batch("CREATE TABLE edge_meta(key TEXT NOT NULL,part INTEGER NOT NULL,json_chunk TEXT NOT NULL,PRIMARY KEY(key,part)); BEGIN").unwrap();
        source
            .execute(
                "INSERT INTO edge_meta VALUES('fixture',0,?1)",
                ["\u{feff}A\0😀"],
            )
            .unwrap();
        let observer = Connection::open(&database).unwrap();
        assert_eq!(
            observer
                .query_row("SELECT count(*) FROM edge_meta", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        let selected: Vec<u8> = source
            .query_row(
                "SELECT CAST(json_chunk AS BLOB) FROM edge_meta",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(selected, raw);
        let mut budget = typed_snapshot::EncodeBudget::new(1024 * 1024, 1024 * 1024).unwrap();
        let inventory = held_frame(
            &source,
            typed_snapshot::Role::Prepared,
            "after_prepared_database",
            &frame,
            &mut budget,
        );
        let imported = typed_snapshot::import(
            &frame,
            typed_snapshot::Role::Prepared,
            "after_prepared_database",
            1024 * 1024,
            1024 * 1024,
            100_000,
            1024,
        )
        .unwrap();
        assert_eq!(inventory, imported.inventory);
        assert_eq!(imported.inventory["database_encoding"], encoding);
        let (kind, returned): (String, Vec<u8>) = imported.connection.query_row("SELECT typeof(json_chunk),CAST(json_chunk AS BLOB) FROM edge_meta WHERE key='fixture' AND part=0", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(kind, "text");
        assert_eq!(returned, raw);
        assert!(!source.is_autocommit());
        source.execute_batch("ROLLBACK").unwrap();
        assert_eq!(
            observer
                .query_row("SELECT count(*) FROM edge_meta", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            imported
                .connection
                .query_row("SELECT count(*) FROM edge_meta", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}

#[test]
fn wal_two_held_transactions_same_file_feed_distinct_typed_frames() {
    let mut f = Fixture::new(true, false, false);
    let writer = Connection::open(&f.prepared).unwrap();
    assert_eq!(
        writer
            .query_row("PRAGMA journal_mode=WAL", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
    let mut before_connection =
        Connection::open_with_flags(&f.prepared, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let before = before_connection.transaction().unwrap();
    let old: String = before
        .query_row("SELECT json FROM knowledge_nodes WHERE id='a'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let old_top = metadata(&before, "knowledge_reader_top");
    let old_descriptor: String = before
        .query_row("SELECT descriptor FROM prepared_state", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&old_descriptor).unwrap()["mode"],
        "bootstrap"
    );
    let capture_limits = limits(
        &f.request("prepared-delta", "local")["limits"],
        "prepared-delta",
    )
    .unwrap();
    let mut read_bytes = D1ReadBytes::new(capture_limits);
    assert_eq!(
        prepared_source_inputs_held(
            &before,
            &typed(&f.before_binding),
            capture_limits,
            &mut read_bytes
        )
        .unwrap()
        .raw(),
        f.before_source.raw()
    );
    f.advance(false, true); // Separate writer commits while predecessor read transaction remains held.
    assert_eq!(
        before
            .query_row("SELECT json FROM knowledge_nodes WHERE id='a'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        old
    );
    assert_eq!(metadata(&before, "knowledge_reader_top"), old_top);
    assert!(!before.is_autocommit());
    let mut after_connection =
        Connection::open_with_flags(&f.prepared, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let after = after_connection.transaction().unwrap();
    let new_top = metadata(&after, "knowledge_reader_top");
    assert_ne!(old_top, new_top);
    assert_eq!(
        prepared_source_inputs_held(&after, &typed(&f.binding), capture_limits, &mut read_bytes)
            .unwrap()
            .raw(),
        f.source.raw()
    );
    assert!(
        prepared_source_inputs_held(
            &after,
            &typed(&f.before_binding),
            capture_limits,
            &mut read_bytes
        )
        .is_err()
    );
    assert_eq!(
        prepared_source_inputs_held(
            &before,
            &typed(&f.before_binding),
            capture_limits,
            &mut read_bytes
        )
        .unwrap()
        .raw(),
        f.before_source.raw()
    );
    let descriptor: Value = serde_json::from_str(
        &after
            .query_row("SELECT descriptor FROM prepared_state", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(descriptor["mode"], "delta-history");
    assert_eq!(
        descriptor["parent_data_revision"],
        f.before_binding["data_revision"]
    );
    let d1 = f.serving_connection();
    d1.execute_batch("BEGIN").unwrap();
    let original = serving(&d1);
    let before_frame = f.directory.0.join("before.frame");
    let after_frame = f.directory.0.join("after.frame");
    let d1_frame = f.directory.0.join("d1.frame");
    let mut frame_budget =
        typed_snapshot::EncodeBudget::new(64 * 1024 * 1024, 64 * 1024 * 1024).unwrap();
    held_frame(
        &before,
        typed_snapshot::Role::Prepared,
        "before_prepared_database",
        &before_frame,
        &mut frame_budget,
    );
    held_frame(
        &after,
        typed_snapshot::Role::Prepared,
        "after_prepared_database",
        &after_frame,
        &mut frame_budget,
    );
    held_frame(
        &d1,
        typed_snapshot::Role::D1,
        "d1_database",
        &d1_frame,
        &mut frame_budget,
    );
    assert_ne!(
        fs::read(&before_frame).unwrap(),
        fs::read(&after_frame).unwrap()
    );
    let mut request = f.request("prepared-delta", "wal");
    request["schema"] = json!(TYPED_REQUEST_SCHEMA);
    request["d1_database"] = json!(d1_frame);
    request["before_prepared_database"] = json!(before_frame);
    request["after_prepared_database"] = json!(after_frame);
    request["snapshot_frame_max_bytes"] = json!(64 * 1024 * 1024);
    request["snapshot_schema_max_allocation_bytes"] = json!(64 * 1024 * 1024);
    let receipt = capture(&request).unwrap();
    assert_eq!(receipt["d1_applied"], false);
    assert_eq!(serving(&d1), original);
    assert!(!before.is_autocommit());
    assert!(!after.is_autocommit());
    d1.execute_batch("ROLLBACK").unwrap();
    d1.execute_batch(&sql(&request, "forward_sql")).unwrap();
    auxiliary_current(&d1);
    d1.execute_batch(&sql(&request, "rollback_sql")).unwrap();
    assert_eq!(serving(&d1), original);
    before.rollback().unwrap();
    after.rollback().unwrap();
}
