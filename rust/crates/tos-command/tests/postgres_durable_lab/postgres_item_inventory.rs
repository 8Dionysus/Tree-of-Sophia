//! Executable PG Item transaction -> cold source -> native prepared consumer.
//! Uses the shared production ABI, the existing isolated PG/STO laboratory and
//! the frozen synthetic Item source. No helper-only or testspec PASS substitutes
//! for reaching the final assertions. Run only in the admitted PG/build window.
use super::*;
use tos_command::source_command::{CommandContext, SourceFile};
use tos_command::source_creation_store::IsolatedCreationRoot;
use tos_compiler::source_bibliographic::{BibliographicLimits, BibliographicSink};
use tos_compiler::source_witness_catalog::{SourceCatalogLimits, SourceCatalogValidator};

// Reuse the same hash-checked capture and explicit private-root rebasing as
// the local owner tests, without changing or copying their implementation.
#[allow(dead_code)]
#[path = "../../src/source_native_test_fixtures.rs"]
mod captured;

const COMMAND: &str = "test:pg-item-inventory-commit-recovery";
const EDITION: &str =
    "ToS/source-witnesses/works/synthetic/parent/expressions/first/editions/first/edition.json";
const ITEM: &str = "ToS/source-witnesses/works/synthetic/parent/expressions/first/editions/first/items/first/item.json";
const ITEM_ID: &str = "tos.item.synthetic.first";
const PRIVATE_TEXT: &str = "PG_ITEM_PRIVATE_SOURCE_CANARY";
const PAYLOAD: &[u8] = br#"<DjVuXML><BODY><OBJECT width="1200" height="1800"><PARAM name="DPI" value="300"/><PARAGRAPH><LINE><WORD>PG_ITEM_PRIVATE_SOURCE_CANARY</WORD></LINE></PARAGRAPH></OBJECT></BODY></DjVuXML>"#;
const ENVELOPE: &str = "tos_local_pg_item_adoption_result_v1";
const ITEM_IMPLEMENTATIONS: &[&str] = &[
    "source_item_adoption.rs",
    "source_native_cli.rs",
    "source_command.rs",
    "source_revisions.rs",
    "source_work_transaction.rs",
    "source_forms.rs",
    "source_private_assessment_sources.rs",
    "source_assessment_journal.rs",
    "source_expression_edition.rs",
    "source_work_expression.rs",
    "source_expression_responsibility.rs",
    "source_read_owner.rs",
    "source_private_profile.rs",
    "source_item_inventory.rs",
    "source_item_inventory_extended.rs",
    "source_item_deposit.rs",
    "source_text_owner_ocr.rs",
];

const TARGET: RevisionTarget = RevisionTarget {
    owner_schema: "tos_local_corpus_revision_owner_v3",
    source_path: EDITION,
    forms_path: "ToS/source-witnesses/works/synthetic/parent/expressions/first/editions/first/edition.human-forms.json",
    history_path: "ToS/source-witnesses/works/synthetic/parent/expressions/first/editions/first/source-revision-history.json",
    record_id: "tos.edition.synthetic.first",
    record_type: "edition",
    command_id: COMMAND,
    form_ids: &["tos.form.synthetic.edition.first.name"],
    form_fields: &["metadata.preferred-name"],
    allowed_fields: &["preferred_label"],
    record_id_field: "record_id",
    record_schema_version: None,
    preferred_label: None,
    notes: None,
    native_field_patch: None,
    resulting_version: 2,
    resulting_form_version: 2,
};

fn item_software(repository: &Path, deadline: Instant, cancel: &AtomicBool) -> SoftwareFixture {
    let mut paths = software_component_paths()
        .into_iter()
        .collect::<BTreeSet<_>>();
    paths.extend(
        ITEM_IMPLEMENTATIONS
            .iter()
            .map(|name| format!("rust/crates/tos-command/src/{name}")),
    );
    paths.extend(
        [
            "rust/crates/tos-command/src/source_item_adoption_pg.rs",
            "rust/crates/tos-command/src/source_native_pg_item_adoption_cli.rs",
            "rust/crates/tos-compiler/src/source_bibliographic_versions.rs",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    // The explicit production list must track the real owner, including PR267's
    // journal bytes. Refuse a changed ABI instead of silently omitting a pin.
    let owner =
        fs::read_to_string(repository.join("rust/crates/tos-command/src/source_item_adoption.rs"))
            .unwrap();
    let pins = owner
        .split("const IMPLEMENTATIONS: &[&str] = &[")
        .nth(1)
        .unwrap()
        .split("];")
        .next()
        .unwrap();
    let actual = pins
        .lines()
        .filter_map(|line| line.trim().strip_prefix('"')?.split('"').next())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual.len(),
        19,
        "review the accepted Item implementation baseline"
    );
    assert!(
        actual.iter().all(|path| paths.contains(*path)),
        "capture every actual Item implementation"
    );
    software_fixture_for_components(repository, paths.into_iter().collect(), deadline, cancel)
}

fn schema_worker(
    cut: &CorpusCutReader,
    deadline: Instant,
    cancel: &AtomicBool,
) -> CutWorkerSchemaExecutor {
    CutWorkerSchemaExecutor::from_cut(
        cut,
        tos_validation::FormatProfile::LegacyPythonObserved20260923,
        worker_identity(),
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 128,
            max_receipt_bytes: 262_144,
        },
        deadline,
        cancel,
    )
    .unwrap()
}

fn worker_identity() -> ExactWorkerIdentity {
    let path = PathBuf::from(
        std::env::var_os("TOS_SCHEMA_WORKER_PATH").expect("admitted exact schema worker"),
    );
    assert!(path.is_absolute());
    ExactWorkerIdentity {
        sha256: Digest256::of_bytes(&fs::read(&path).unwrap()),
        absolute_path: path,
    }
}

struct ItemCase {
    case: RevisionCase,
    source: SourceCutFixture,
    bootstrap_context: CommandContext,
    proposal: Value,
    config: Value,
    input: PathBuf,
}

fn item_case(
    repository: &Path,
    software_fixture: &SoftwareFixture,
    deadline: Instant,
    cancel: &AtomicBool,
) -> ItemCase {
    let mut lab = Lab::new(&database_url());
    fs::set_permissions(&lab._root.0, fs::Permissions::from_mode(0o700)).unwrap();
    let store_root = lab._root.0.canonicalize().unwrap();
    let scratch = ScratchRoot::new();
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    let copy = scratch.0.join("source-copy");
    fs::create_dir(&copy).unwrap();
    fs::set_permissions(&copy, fs::Permissions::from_mode(0o700)).unwrap();
    let mut packet = captured::item_orphan_recovery(repository, &copy);
    let input = PathBuf::from(packet["input"].as_str().unwrap());
    fs::write(&input, PAYLOAD).unwrap();
    fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
    let config = &mut packet["config"];
    config["source_root"] = json!(store_root);
    config["payload_basename"] = json!("synthetic.djvu.xml");
    config["original_basename"] = json!("synthetic.djvu.xml");
    config["media_type"] = json!("application/vnd.djvu+xml");
    config["byte_size"] = json!(PAYLOAD.len());
    config["sha256"] = json!(Digest256::of_bytes(PAYLOAD).to_hex());
    config["file_id"] = json!(format!(
        "tos.file.sha256.{}",
        Digest256::of_bytes(PAYLOAD).to_hex()
    ));
    let scopes = json!([config["item_id"], config["file_id"]]);
    packet["proposal"]["rights"]["scope_refs"] = scopes;
    let owner_path = PathBuf::from(packet["owner"].as_str().unwrap());
    fs::write(&owner_path, canonical(&packet["config"])).unwrap();
    fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut members = SourceMembers::new();
    let mut pending = vec![copy.join("ToS")];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink());
            if kind.is_dir() {
                pending.push(entry.path());
                continue;
            }
            assert!(kind.is_file());
            let path = entry
                .path()
                .strip_prefix(&copy)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            if tos_source_store::is_authored_source_path_v1(&path) {
                let bytes = fs::read(entry.path()).unwrap();
                assert!(bytes.len() <= 2_097_152);
                members.insert(
                    path,
                    (bytes, fs::metadata(entry.path()).unwrap().mode() & 0o777),
                );
            }
        }
    }
    // Freeze the compiler's schema closure in the original PG source cohort;
    // the subsequent authenticated export must not fill it from a local mirror.
    for entry in fs::read_dir(repository.join("ToS/contracts")).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file()
            && path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".schema.json")
        {
            let relative = path.strip_prefix(repository).unwrap().to_str().unwrap();
            if !members.contains_key(relative) {
                copy_repository_member(repository, &copy, &mut members, relative);
            }
        }
    }
    assert!(members.len() <= 512);
    assert!(members.values().map(|(raw, _)| raw.len()).sum::<usize>() <= 8 * 1024 * 1024);
    let source = source_cut_from_members(scratch, copy, members, deadline, cancel);
    let (software, components) = software_reader(software_fixture, deadline, cancel);
    let context = CommandContext {
        base_revision: source.revision,
        configuration_raw: fs::read(&owner_path).unwrap(),
        request_raw: canonical(&packet["proposal"]),
        recorded_at: "2026-10-23T00:00:00+00:00".into(),
        effective_uid: rustix::process::getuid().as_raw().into(),
        files: components
            .members()
            .map(|member| SourceFile {
                path: member.path.clone(),
                raw: software
                    .read_selected_component(&components, &member.path, 2_097_152, deadline, cancel)
                    .unwrap(),
            })
            .collect(),
    };
    let initial = lab
        .db
        .bootstrap_source_cohort(
            &lab.store,
            &lab.domain,
            &source.cut,
            source.revision,
            source.cut.stream(source.revision).unwrap().expectation(),
            &context,
            &software,
            &components,
            &mut schema_worker(&source.cut, deadline, cancel),
            contract_digest(),
            "private-job",
            1,
            tos_command::source_cohort::BootstrapJobMode::RequireExisting,
            &|| Ok(()),
            deadline,
            cancel,
        )
        .expect("synthetic captured sources bootstrap through the existing PG/STO owner");
    let initial_head = lab.head_seq();
    assert!(
        !store_root.join("ToS").exists(),
        "PG has no authored-source local mirror"
    );
    let owner_scratch = ScratchRoot::new();
    fs::set_permissions(&owner_scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
    ItemCase {
        case: RevisionCase {
            _owner_scratch: owner_scratch,
            lab,
            cohort: initial.cohort,
            source_root: store_root,
            original_record: source.members[EDITION].0.clone(),
            all_source: source.members.clone(),
            initial_head,
            owner_config: owner_path,
            target: TARGET,
        },
        source,
        bootstrap_context: context,
        proposal: packet["proposal"].clone(),
        config: packet["config"].clone(),
        input,
    }
}

fn item_call(
    case: &RevisionCase,
    binary: &Path,
    invocation: &Path,
    request: &Value,
    success: bool,
) -> NativeInvocation {
    invoke_with_expected_exit(
        binary,
        invocation,
        &case.lab.url,
        request,
        false,
        success,
        ENVELOPE,
        None,
        false,
    )
}

fn source_state(case: &RevisionCase) -> Value {
    let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
    let mut result = serde_json::Map::new();
    for table in [
        "cmd2_current",
        "cmd2_source_publication_current",
        "cmd2_source_inventory_projection",
    ] {
        let rows = db.query(&format!("SELECT row_to_json(t)::text FROM (SELECT * FROM {table} WHERE domain=$1) t ORDER BY row_to_json(t)::text COLLATE \"C\""), &[&case.lab.domain]).unwrap();
        result.insert(
            table.into(),
            json!(
                rows.into_iter()
                    .map(|row| row.get::<_, String>(0))
                    .collect::<Vec<_>>()
            ),
        );
    }
    result.insert("head".into(), json!(case.lab.head_seq()));
    Value::Object(result)
}

// Test-only failure at the actual PG apply, after ready/seal. Names are owned
// by this unique private domain; Drop removes them even when an assertion fails.
struct ApplyInterrupt {
    url: String,
    name: String,
    armed: bool,
}
impl ApplyInterrupt {
    fn install(case: &RevisionCase) -> Self {
        let name = format!(
            "tos_test_item_{}",
            &Digest256::of_bytes(case.lab.domain.as_bytes()).to_hex()[..24]
        );
        let domain = case.lab.domain.replace('\'', "''");
        let mut db = Client::connect(&case.lab.url, NoTls).unwrap();
        db.batch_execute(&format!(
            "CREATE SEQUENCE public.{name}_fired;
            CREATE FUNCTION public.{name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            IF NEW.domain='{domain}' THEN PERFORM nextval('public.{name}_fired'::regclass);
            RAISE EXCEPTION 'intentional ready-item interrupt'; END IF; RETURN NEW; END $$;
            CREATE TRIGGER {name} BEFORE INSERT OR UPDATE ON cmd2_current
            FOR EACH ROW EXECUTE FUNCTION public.{name}();"
        ))
        .unwrap();
        Self {
            url: case.lab.url.clone(),
            name,
            armed: true,
        }
    }
    fn remove(&mut self) -> bool {
        let mut db = Client::connect(&self.url, NoTls).unwrap();
        let fired: bool = db
            .query_one(
                &format!("SELECT is_called FROM public.{}_fired", self.name),
                &[],
            )
            .unwrap()
            .get(0);
        db.batch_execute(&format!("DROP TRIGGER {} ON cmd2_current; DROP FUNCTION public.{}(); DROP SEQUENCE public.{}_fired;", self.name, self.name, self.name)).unwrap();
        self.armed = false;
        fired
    }
}
impl Drop for ApplyInterrupt {
    fn drop(&mut self) {
        if self.armed {
            if let Ok(mut db) = Client::connect(&self.url, NoTls) {
                let _ = db.batch_execute(&format!("DROP TRIGGER IF EXISTS {} ON cmd2_current; DROP FUNCTION IF EXISTS public.{}(); DROP SEQUENCE IF EXISTS public.{}_fired;", self.name, self.name, self.name));
            }
        }
    }
}

struct GraphRows {
    nodes: Vec<Value>,
    edges: Vec<Value>,
    claims: Vec<Value>,
}
impl BibliographicSink for GraphRows {
    fn row(&mut self, collection: &str, _: &str, raw: &[u8]) -> tos_compiler::Result<()> {
        let value: Value = serde_json::from_slice(raw).unwrap();
        match collection {
            "nodes" => self.nodes.push(value),
            "edges" => self.edges.push(value),
            "claim_traces" => self.claims.push(value),
            _ => panic!("unexpected bibliographic collection {collection}"),
        }
        Ok(())
    }
}

fn contains_value(value: &Value, expected: &Value) -> bool {
    value == expected
        || match value {
            Value::Array(values) => values.iter().any(|v| contains_value(v, expected)),
            Value::Object(values) => values.values().any(|v| contains_value(v, expected)),
            _ => false,
        }
}

struct ExportOwner<'a> {
    cut: &'a CorpusCutReader,
    expected: tos_compiler::knowledge_stage::ExactInputReceipt,
    revision: SourceRevision,
    membership: tos_source_store::SourceMembershipV1,
}
impl tos_compiler::knowledge_stage::StageOwner for ExportOwner<'_> {
    fn verify_receipt(
        &self,
        receipt: &tos_compiler::knowledge_stage::ExactInputReceipt,
    ) -> tos_compiler::Result<()> {
        if serde_json::to_value(&receipt.binding).unwrap()
            != serde_json::to_value(&self.expected.binding).unwrap()
            || serde_json::to_value(&receipt.collections).unwrap()
                != serde_json::to_value(&self.expected.collections).unwrap()
        {
            return Err(tos_compiler::Error::Invalid(
                "PG Item exact export receipt changed",
            ));
        }
        self.recheck_sealed_cut(receipt)
    }
    fn recheck_sealed_cut(
        &self,
        _: &tos_compiler::knowledge_stage::ExactInputReceipt,
    ) -> tos_compiler::Result<()> {
        if self.cut.current().revision() != self.revision
            || self
                .cut
                .stream(self.revision)
                .map_err(|_| tos_compiler::Error::Invalid("PG Item export stream"))?
                .expectation()
                != self.membership
        {
            return Err(tos_compiler::Error::Invalid(
                "PG Item exact export membership changed",
            ));
        }
        Ok(())
    }
}

fn prepared_consumer(
    item: &mut ItemCase,
    software_fixture: &SoftwareFixture,
    expected_inventory: &Value,
    isolation: &tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Value {
    let case = &mut item.case;
    let (software, components) = software_reader(software_fixture, deadline, cancel);
    // A complete authenticated current export, produced from PG/STO, is the
    // compiler input. The frozen original is only the retained bootstrap basis.
    let isolated = IsolatedCreationRoot::create(&case._owner_scratch.0, deadline, cancel).unwrap();
    let export = tos_command::source_current_cut::select_current_source_cut(
        &mut case.lab.db,
        &case.lab.store,
        &case.lab.domain,
        &item.source.cut,
        item.source.revision,
        item.source
            .cut
            .stream(item.source.revision)
            .unwrap()
            .expectation(),
        &item.bootstrap_context,
        &software,
        &components,
        &mut schema_worker(&item.source.cut, deadline, cancel),
        None,
        &isolated,
        None,
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 2048,
            max_selected_object_bytes: 8_388_608,
            json: JsonLimits::default(),
        },
        CutReadLimits {
            max_revisions: 4,
            max_members: 2048,
            max_total_bytes: 33_554_432,
            max_member_bytes: 8_388_608,
        },
        deadline,
        cancel,
    )
    .expect("complete current PG source exports with its retained history and publication");
    assert_eq!(export.generation().commit_seq(), case.lab.head_seq());
    let cut = export.cut();
    let revision = cut.current().revision();
    let membership = cut.stream(revision).unwrap().expectation();
    let binding = tos_compiler::SourceBinding {
        owner_profile: "synthetic-pg-item-prepared-consumer".into(),
        source_cut: revision.0.to_hex(),
        through_commit_seq: case.lab.head_seq(),
        membership_root: membership.digest.to_hex(),
        index_generation: revision.0.to_hex(),
        route_map_version: "synthetic-pg-item-v1".into(),
        reader_abi: "synthetic-pg-item-v1".into(),
        projection_root_sha256: export.generation().digest().to_hex(),
        complete: true,
    };
    let l = BibliographicLimits {
        catalog: SourceCatalogLimits {
            max_files: 512,
            max_rows: 4096,
            max_file_bytes: 2_097_152,
            max_row_bytes: 1_048_576,
            max_contract_bytes: 16_777_216,
            max_output_row_bytes: 1_048_576,
        },
        max_claim_cohort_rows: 16,
        max_claim_cohort_bytes: 16_777_216,
        max_output_rows: 4096,
        max_output_bytes: 16_777_216,
        deadline,
    };
    let plan = tos_compiler::plan_source_catalog_inputs(
        cut,
        revision,
        membership,
        &binding,
        tos_compiler::SourceCatalogInputLimits {
            max_manifest_members: 2048,
            max_selected_members: 512,
            max_plan_bytes: 16_777_216,
            max_work_bytes: 67_108_864,
        },
        l,
        cancel,
    )
    .unwrap();
    let stage_owner = ExportOwner {
        cut,
        expected: plan.input_receipt(),
        revision,
        membership,
    };
    let mut stage = tos_compiler::knowledge_stage::KnowledgeStage::create(
        &case._owner_scratch.0.join("item-catalog.sqlite"),
        tos_compiler::knowledge_stage::StageLimits {
            sqlite: tos_compiler::Limits {
                max_rows: 8192,
                max_row_bytes: 2_097_152,
                max_output_bytes: 67_108_864,
                max_work_bytes: 134_217_728,
                sqlite_cache_kib: 512,
                max_sql_vm_steps: 20_000_000,
            },
            max_temp_bytes: 67_108_864,
            max_seek_rows: 16,
            max_seek_bytes: 33_554_432,
        },
        plan.input_receipt(),
        &stage_owner,
        isolation,
    )
    .unwrap();
    let worker = worker_identity();
    let mut operation = tos_validation::executor::BatchStreamBudget::laboratory();
    operation.max_chunks = l.max_output_rows;
    operation.max_total_units = l.max_output_rows;
    operation.total_execution_wall = deadline.saturating_duration_since(Instant::now());
    operation.operation_cpu_seconds = operation.total_execution_wall.as_secs().saturating_add(1);
    let validator = SourceCatalogValidator::from_cut(
        cut,
        &worker,
        ExecutorBudget::laboratory(),
        CutWorkerLimits {
            max_receipts: 4096,
            max_receipt_bytes: 16_777_216,
        },
        operation,
        deadline,
        cancel,
    )
    .unwrap();
    let mut forms = tos_command::source_forms_compiler::NativeBibliographicForms;
    let produced = tos_compiler::render_source_bibliographic_plan(
        &plan, cut, revision, membership, &mut stage, &validator, &mut forms, l, 512, 16_777_216,
    )
    .unwrap();
    let read = |path: &str| {
        cut.read_member(
            revision,
            &RelativePath::parse(path).unwrap(),
            2_097_152,
            deadline,
            cancel,
        )
        .unwrap()
        .raw
    };
    let entities: Value = serde_json::from_slice(&read(SHARED_SOURCE[0])).unwrap();
    let source = tos_compiler::source_bibliographic::BibliographicSourceCut {
        cut,
        expected_revision: revision,
        expected_membership: membership,
        stage_source_cut: &binding.source_cut,
        max_read_files: 512,
        max_read_bytes: 16_777_216,
    };
    let navigation = tos_compiler::source_navigation_source::project_source_navigation_from_cut(
        &mut stage,
        &produced.catalog,
        &source,
        &validator,
        &entities,
        &mut forms,
        l,
    )
    .unwrap();
    assert!(
        navigation
            .diagnostics()
            .iter()
            .all(|d| d["level"] != "error")
    );
    let nav_item = navigation.value()["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node_id"] == ITEM_ID)
        .expect("native compiler projects the committed Item");
    let file_id = item.config["file_id"].as_str().unwrap();
    let nav_file = navigation.value()["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["node_id"] == file_id)
        .expect("native compiler projects the committed exact File");
    assert_eq!(
        nav_file["properties"]["sha256"],
        expected_inventory["file_sha256"]
    );
    assert_eq!(
        nav_file["properties"]["media_type"],
        expected_inventory["media_type"]
    );
    let expected_item: Value = serde_json::from_slice(&read(ITEM)).unwrap();
    assert_eq!(nav_item["properties"]["source_record"], expected_item);
    let item_home = ITEM.rsplit_once('/').unwrap().0;
    let actual_inventory: Value =
        serde_json::from_slice(&read(&format!("{item_home}/resource-inventory.json"))).unwrap();
    assert_eq!(actual_inventory["files"][0], *expected_inventory);
    let mut graph = GraphRows {
        nodes: vec![],
        edges: vec![],
        claims: vec![],
    };
    tos_compiler::source_bibliographic::render_bibliographic_graph(
        &mut stage,
        &produced.catalog,
        &produced.bibliographic,
        l,
        &mut graph,
    )
    .unwrap();
    // The standard five-carrier native prepare input is generated from this
    // exact compiler result. No hand-authored prepared row or SQLite insert.
    let root = case._owner_scratch.0.join("prepared-source");
    fs::create_dir(&root).unwrap();
    let write = |path: &str, value: &Value| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
    };
    write(
        "ToS/derived-exports/tos_corpus_index.min.json",
        &json!({
            "schema_version":"tos_corpus_index_v1", "nodes":[], "resources":[], "manifests":[], "branches":[], "relation_edges":[], "relation_packs":[], "graph_views":[], "source_navigation":navigation.value()
        }),
    );
    write(
        "ToS/derived-exports/philosophy_graph_projection.min.json",
        &json!({"schema_version":"tos_philosophy_graph_projection_v2", "nodes":[], "edges":[], "layers":[], "views":[], "clusters":[]}),
    );
    write(
        "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
        &json!({"schema_version":"tos_source_witness_bibliographic_graph_v1", "nodes":graph.nodes, "edges":graph.edges, "claim_traces":graph.claims}),
    );
    for path in SHARED_SOURCE {
        let target = root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, read(path)).unwrap();
    }
    let output = case._owner_scratch.0.join("prepared-publication");
    let receipt =
        tos_compiler::native_prepare::prepare(tos_compiler::native_prepare::PrepareRequest {
            source_root: &root,
            output_dir: &output,
            publication: tos_compiler::local_prepared::PublicationLimits {
                max_bytes: 33_554_432,
                ..Default::default()
            },
            search_scratch: None,
            maintenance: None,
            max_seconds: deadline
                .saturating_duration_since(Instant::now())
                .as_secs()
                .min(60),
            source_limits: Some(tos_compiler::native_prepare::SourceBootstrapLimits {
                max_input_bytes: 16_777_216,
                max_capture_bytes: 33_554_432,
                max_stage_bytes: 67_108_864,
                max_temp_bytes: 33_554_432,
            }),
        })
        .expect("the actual native publisher commits a fresh prepared SQLite snapshot");
    assert_eq!(receipt["status"], "completed");
    assert_eq!(receipt["ongoing_currentness_granted"], false);
    let completed: Value =
        serde_json::from_slice(&fs::read(output.join("completed.json")).unwrap()).unwrap();
    assert_eq!(completed, receipt);
    let mut packets = Vec::new();
    for _ in 0..2 {
        let executor = tos_access::prepared_local::PreparedLocalExecutor::open(
            output.join("snapshot.sqlite"),
            output.join("binding.json"),
            None,
        )
        .unwrap();
        let mut reads = Vec::new();
        for id in [ITEM_ID, file_id] {
            let response = tos_access::http::handle_get(
                &executor,
                "GET",
                &format!("/api/knowledge/nodes/{id}?relation_limit=0"),
                tos_access::prepared_local::profile(),
            );
            assert_eq!(
                response.status,
                200,
                "{}",
                String::from_utf8_lossy(&response.body)
            );
            let packet: Value = serde_json::from_slice(&response.body).unwrap();
            let matches = packet["matches"]
                .as_array()
                .expect("real prepared inspect matches");
            assert!(matches.iter().any(|row| row["native_id"] == id && row["source_graph"] == "source-navigation"));
            let encoded = serde_json::to_string(&packet).unwrap();
            if id == ITEM_ID {
                assert!(
                    contains_value(&packet, &expected_item),
                    "prepared inspect preserves the complete committed Item record"
                );
            } else {
                assert!(encoded.contains(expected_inventory["file_sha256"].as_str().unwrap()));
            }
            assert!(!encoded.contains(PRIVATE_TEXT));
            reads.push(packet);
        }
        packets.push(reads);
        drop(executor);
    }
    assert_eq!(
        packets[0], packets[1],
        "reopened prepared consumer reads the same independent binding"
    );
    let mut stream = cut.stream(revision).unwrap();
    while stream.next_member(deadline, cancel).unwrap().is_some() {}
    assert_eq!(
        stream.coverage(),
        Some(membership),
        "complete export bytes still match at final consumer disclosure"
    );
    json!({"export_commit_seq":export.generation().commit_seq(),"export_source_revision":revision.0.to_prefixed(),"inventory_sha256":Digest256::of_bytes(&read(&format!("{item_home}/resource-inventory.json"))).to_hex(),"native_prepared_receipt":receipt,"consumer":packets[0]})
}

#[test]
#[ignore = "requires selected PG16, matching native owner/schema worker and existing kernel resource ticket"]
fn real_pg_item_inventory_commit_replay_ready_recovery_and_prepared_consumer() {
    let deadline = Instant::now() + Duration::from_secs(300);
    let cancel = AtomicBool::new(false);
    // The existing kernel-verified ticket, not a fixture no-op, owns compiler
    // spill/RAM confinement. This test never creates or relabels an admission.
    let isolation = tos_compiler::private_tmpfs_stage::PrivateTmpfsStageIsolation::select_issued_from_environment()
        .expect("existing admitted PG/build window must supply the real private stage ticket");
    let lab_root = PathBuf::from(
        std::env::var_os("TOS_CMD2_LAB_ROOT").expect("explicit admitted PG fixture storage"),
    );
    assert!(
        lab_root.is_absolute()
            && lab_root
                .canonicalize()
                .unwrap()
                .starts_with(isolation.root())
    );
    let repository = repository_root();
    let software = item_software(&repository, deadline, &cancel);
    let mut item = item_case(&repository, &software, deadline, &cancel);
    let binary = native_binary();
    let invocation_path = item
        .case
        ._owner_scratch
        .0
        .canonicalize()
        .unwrap()
        .join("pg-item-invocation.json");
    invocation(
        &item.case,
        &software,
        &binary,
        &worker_identity().absolute_path,
        &invocation_path,
        &item.case.owner_config,
        "tos_local_pg_item_adoption_invocation_v1",
    );
    let before = source_state(&item.case);
    let preview = item_call(&item.case, &binary, &invocation_path, &item.proposal, true);
    assert_eq!(preview.result["durable_commit"], Value::Null);
    let mut request = preview.result["result"]["prepared_request"].clone();
    assert!(
        request.is_object(),
        "production PG prepare returns the exact prepared request"
    );
    request["command_id"] = json!(COMMAND);
    let expected_inventory = request["inventory"].clone();
    assert_eq!(expected_inventory["profile"], "djvu_xml_pages_v1");
    assert_eq!(expected_inventory["summary"]["page_count"], 1);
    assert_eq!(
        expected_inventory["file_sha256"],
        Digest256::of_bytes(PAYLOAD).to_hex()
    );
    assert!(
        !serde_json::to_string(&expected_inventory)
            .unwrap()
            .contains(PRIVATE_TEXT)
    );
    assert_eq!(
        source_state(&item.case),
        before,
        "prepare makes no PG source/publication change"
    );
    let mut interrupt = ApplyInterrupt::install(&item.case);
    let refused = item_call(&item.case, &binary, &invocation_path, &request, false);
    let fired = interrupt.remove();
    assert!(fired, "failure must reach actual PG apply after ready/seal");
    assert_eq!(refused.result["refused"], true);
    assert_eq!(
        source_state(&item.case),
        before,
        "failed PG apply is atomic"
    );
    let mut sql = Client::connect(&item.case.lab.url, NoTls).unwrap();
    let ready = sql.query_one("SELECT state,prepare_id,delta_digest FROM cmd2_attempt WHERE domain=$1 AND command_id=$2", &[&item.case.lab.domain, &COMMAND]).unwrap();
    assert_eq!(ready.get::<_, String>(0), "ready");
    let prepare_id: Vec<u8> = ready.get(1);
    let delta: String = ready.get(2);
    drop(sql);
    // Each invocation is a new native process. The retry must reopen the ready
    // attempt, reuse its retained source bytes and commit exactly once.
    let committed = item_call(&item.case, &binary, &invocation_path, &request, true);
    assert_eq!(committed.result["replayed"], false);
    assert_eq!(committed.result["result"]["metadata_committed"], true);
    let head = item.case.lab.head_seq();
    assert_eq!(head, item.case.initial_head + 1);
    assert_eq!(committed.result["durable_commit"]["commit_seq"], head);
    assert_eq!(
        committed.result["durable_commit"]["delta_sha256"],
        format!("sha256:{delta}")
    );
    assert_eq!(
        committed.result["durable_commit"]["prepare_id_sha256"],
        Digest256::of_bytes(&prepare_id).to_prefixed()
    );
    let durable_counts =
        ["current", "history", "receipt", "log", "outbox"].map(|table| item.case.lab.count(table));
    item.case.lab.db = DurablePgCoordinator::connect(&item.case.lab.url).unwrap();
    item.case.lab.store =
        tos_segment_store::SegmentStore::open_existing(&item.case.source_root, limits()).unwrap();
    item.case
        .lab
        .db
        .cold_verify_cut(&item.case.lab.store, &item.case.lab.domain)
        .unwrap();
    let after = source_state(&item.case);
    let replay = item_call(&item.case, &binary, &invocation_path, &request, true);
    assert_eq!(replay.result["replayed"], true);
    assert_eq!(replay.result["durable_commit"]["commit_seq"], head);
    assert_eq!(
        replay.result["result"]["receipt"],
        committed.result["result"]["receipt"]
    );
    assert_eq!(
        source_state(&item.case),
        after,
        "replay creates no second source/publication revision"
    );
    assert_eq!(
        ["current", "history", "receipt", "log", "outbox"].map(|table| item.case.lab.count(table)),
        durable_counts
    );
    let mut collision_request = request.clone();
    collision_request["inventory"]["summary"]["word_count"] = json!(2);
    let collision = item_call(
        &item.case,
        &binary,
        &invocation_path,
        &collision_request,
        false,
    );
    assert_eq!(collision.result["refused"], true);
    assert!(
        collision.result["stderr"]
            .as_str()
            .unwrap()
            .contains("source revision recovery request/config binding differs")
            || collision.result["stdout"]
                .as_str()
                .unwrap()
                .contains("source revision recovery request/config binding differs"),
        "a same-command collision must reach the exact recovery request fence"
    );
    assert_eq!(source_state(&item.case), after);
    assert_eq!(
        ["current", "history", "receipt", "log", "outbox"].map(|table| item.case.lab.count(table)),
        durable_counts
    );
    assert_eq!(fs::read(&item.input).unwrap(), PAYLOAD);
    assert!(
        !item.case.source_root.join("ToS").exists(),
        "consumer source stays in PG/STO"
    );
    let home = ITEM.rsplit_once('/').unwrap().0;
    let inventory: Value = serde_json::from_slice(&source_member(
        &mut item.case,
        &format!("{home}/resource-inventory.json"),
    ))
    .unwrap();
    assert_eq!(inventory["files"][0], expected_inventory);
    assert_eq!(inventory["source_text_included"], false);
    assert_eq!(
        inventory["provenance_event_ref"],
        item.config["inventory_event_id"]
    );
    let rights: Value = serde_json::from_slice(&source_member(
        &mut item.case,
        &format!("{home}/rights.json"),
    ))
    .unwrap();
    assert_eq!(rights, request["rights"]);
    assert_eq!(rights["visibility"], "local_only");
    assert_eq!(rights["redistribution_posture"], "not_authorized");
    let original = item
        .case
        .lab
        .db
        .cold_recover_exact(&item.case.lab.store, &item.case.lab.domain, EDITION, 1)
        .unwrap();
    assert_eq!(
        item.case
            .lab
            .db
            .warm_read_selected(&item.case.lab.store, &original, 1_048_576)
            .unwrap(),
        item.case.original_record
    );
    let prepared = prepared_consumer(
        &mut item,
        &software,
        &expected_inventory,
        &isolation,
        deadline,
        &cancel,
    );
    assert_eq!(
        item.case.lab.head_seq(),
        head,
        "prepared publication never mutates the source head"
    );
    item.case
        .lab
        .db
        .revoke_local(&item.case.lab.domain)
        .unwrap();
    let revoked_state = source_state(&item.case);
    let revoked = item_call(&item.case, &binary, &invocation_path, &request, false);
    assert_eq!(revoked.result["refused"], true);
    assert_eq!(
        source_state(&item.case),
        revoked_state,
        "revoked rights prevent replay without another source commit"
    );
    println!(
        "TOS_PG_ITEM_INVENTORY_ACCEPTANCE {}",
        json!({
            "classification":"synthetic executed assertions only; no rights or semantic admission",
            "source_commit":software.selection.source_git_commit,
            "source_tree":software.selection.source_git_tree,
            "committed_durable_counts":durable_counts,
            "prepare_resources":preview.resources, "interrupted_apply_resources":refused.resources,
            "recovery_commit":committed.result["durable_commit"], "recovery_resources":committed.resources,
            "replay_commit":replay.result["durable_commit"], "replay_resources":replay.resources,
            "same_command_collision":collision.result, "revoked_replay":revoked.result,
            "prepared_publication_consumer":prepared
        })
    );
}
