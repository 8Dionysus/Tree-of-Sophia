//! One disposable original synthetic provider fixture reused by QRY and API.
use super::*;
use rusqlite::{Connection, params};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
struct NoAbort;
impl AbortProbe for NoAbort {
    fn reason(&self) -> Option<AbortReason> {
        None
    }
}
pub struct ReadingFixture {
    pub root: PathBuf,
    pub roots: ExplicitReadingRoots,
    retained_for_consumers: bool,
}
const CONCEPT_ROUTE: &str = "ToS/candidate-intake/zarathustra/concept-workbench-v1";
const PRIVATE: &str = "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/concept-workbench-v1";
fn write(root: &std::path::Path, reference: &str, raw: &[u8], private: bool) {
    let path = root.join(reference);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, raw).unwrap();
    if private {
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}
fn sha(path: &std::path::Path) -> String {
    tos_foundation::Digest256::of_bytes(&fs::read(path).unwrap()).to_hex()
}
fn jsonl(rows: &[Value]) -> Vec<u8> {
    rows.iter()
        .map(|v| serde_json::to_string(v).unwrap() + "\n")
        .collect::<String>()
        .into_bytes()
}
impl ReadingFixture {
    pub fn new() -> Self {
        Self::with_layout(false)
    }
    pub fn new_shared_root() -> Self {
        Self::with_layout(true)
    }
    fn with_layout(shared: bool) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tos-reading-original-{}-{nonce}",
            std::process::id()
        ));
        let roots = ExplicitReadingRoots {
            source_root: root.join("source #data"),
            analysis_root: root.join(if shared {
                "source #data"
            } else {
                "analysis #data"
            }),
        };
        fs::create_dir_all(&roots.source_root).unwrap();
        fs::create_dir_all(&roots.analysis_root).unwrap();
        let source = &roots.source_root;
        let analysis = &roots.analysis_root;
        write(
            source,
            DEFAULT_REQUEST_REF,
            include_bytes!(
                "../../../../../ToS/candidate-intake/zarathustra/concept-workbench-v1/requests/fate.concept-request.v2.json"
            ),
            false,
        );
        let concept_ref = format!("{CONCEPT_ROUTE}/outputs/fate/concept-candidate.v1.json");
        write(source,&concept_ref,serde_json::to_vec(&json!({"concept_candidate_id":"candidate-concept","request_id":"request-two","concept_id":null})).unwrap().as_slice(),false);
        let text = "😀 „Schicksal!“ So Schicksal. Schick¬\nsal Schick-\nsal Schick\nsal.";
        let starts = text
            .match_indices("Schicksal")
            .map(|(byte, _)| text[..byte].chars().count())
            .collect::<Vec<_>>();
        let occurrences=(0..3).map(|i|json!({"language":"de","evidence_tier":if i==2{"semantic_neighbor"}else{"direct_or_morphological"},
            "selection_kind":if i==2{"semantic_exact"}else{"direct_exact"},"part":1,"reading_ref":"r1","unit_kind":"paragraph",
            "witness_order":1,"token_ordinal":i+1,"occurrence_ordinal_within_context":i+1,
            "occurrence_candidate_id":format!("candidate-{i}"),"existing_occurrence_ref":format!("old-{i}"),
            "context_unit_ref":"ctx","analysis_key_sha256":"analysis-sha"})).collect::<Vec<_>>();
        let occurrences_ref = format!("{CONCEPT_ROUTE}/outputs/fate/occurrence-spine.v1.jsonl");
        write(source, &occurrences_ref, &jsonl(&occurrences), false);
        let relations=(0..3).map(|i|json!({"relation_type":"lexical_realization","subject_refs":[format!("candidate-{i}")],"object_refs":["candidate-concept"],"relation_candidate_id":format!("relation-{i}"),"status":"proposed"})).collect::<Vec<_>>();
        let relations_ref = format!("{CONCEPT_ROUTE}/outputs/fate/relation-candidates.v1.jsonl");
        write(source, &relations_ref, &jsonl(&relations), false);
        let tasks_ref = format!("{CONCEPT_ROUTE}/outputs/fate/english-on-demand-worklist.v1.jsonl");
        write(
            source,
            &tasks_ref,
            &jsonl(&[json!({"source_occurrence_ref":"candidate-0","english_task_id":"task-0"})]),
            false,
        );
        let contexts_ref = format!("{CONCEPT_ROUTE}/context-unit-spine.v1.jsonl");
        write(
            source,
            &contexts_ref,
            &jsonl(&[json!({"context_unit_ref":"ctx","anchor_refs":["source-anchor"]})]),
            false,
        );
        let private_ref = format!("{PRIVATE}/requests/fate.request-analysis.v1.json");
        write(source,&private_ref,&serde_json::to_vec(&json!({"concept_candidate_ref":"candidate-concept","selected_forms":[{"language":"de","analysis_key_sha256":"analysis-sha","selection_kind":"direct_exact","probe_display":"Schicksal"},{"language":"de","analysis_key_sha256":"analysis-sha","selection_kind":"semantic_exact","probe_display":"Schicksal"}]})).unwrap(),true);
        let concept_db_ref = format!("{PRIVATE}/workbench-index.v1.sqlite3");
        let concept_db_path = source.join(&concept_db_ref);
        fs::create_dir_all(concept_db_path.parent().unwrap()).unwrap();
        let db = Connection::open(&concept_db_path).unwrap();
        db.execute_batch("CREATE TABLE exact_occurrences(existing_occurrence_ref TEXT,exact_form TEXT,analysis_key TEXT);CREATE TABLE context_units(context_unit_ref TEXT,language TEXT,witness_order INTEGER,exact_text TEXT,speaker_role TEXT,speaker_status TEXT,alignment_links_json TEXT,analysis_tokens_json TEXT);").unwrap();
        for i in 0..3 {
            db.execute(
                "INSERT INTO exact_occurrences VALUES(?,?,?)",
                params![format!("old-{i}"), "Schicksal", "schicksal"],
            )
            .unwrap();
        }
        db.execute(
            "INSERT INTO context_units VALUES(?,?,?,?,?,?,?,?)",
            params!["ctx", "de", 1, text, "narrator", "proposed", "[]", "[]"],
        )
        .unwrap();
        drop(db);
        fs::set_permissions(&concept_db_path, fs::Permissions::from_mode(0o600)).unwrap();
        let artifacts = [
            &concept_ref,
            &occurrences_ref,
            &relations_ref,
            &tasks_ref,
            &contexts_ref,
        ]
        .iter()
        .map(|r| json!({"ref":r,"sha256":sha(&source.join(r))}))
        .collect::<Vec<_>>();
        let private_artifacts = [&private_ref, &concept_db_ref]
            .iter()
            .map(|r| json!({"ref":r,"sha256":sha(&source.join(r))}))
            .collect::<Vec<_>>();
        write(source,&format!("{CONCEPT_ROUTE}/manifest.v1.json"),&serde_json::to_vec(&json!({"schema_version":"tos_zarathustra_concept_workbench_manifest_v1","concept_search_result_schema_sha256":"5f67d5b3abf88ecd88dcdb94f70eee0cb685b7ac81b7abbd41bc2b14542c3cc3","artifacts":artifacts,"private_artifacts":private_artifacts})).unwrap(),false);
        let reading_path = analysis.join("reading #db.sqlite3");
        let db = Connection::open(&reading_path).unwrap();
        db.execute_batch("CREATE TABLE metadata(key TEXT,value TEXT);
            CREATE TABLE contexts(context_unit_ref TEXT,language TEXT,part INTEGER,witness_order INTEGER,reading_ref TEXT,exact_text TEXT,exact_sha256 TEXT);
            CREATE TABLE occurrence_spans(existing_occurrence_ref TEXT,context_unit_ref TEXT,surface_unit_ref TEXT,start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT);
            CREATE TABLE discourse_segments(segment_id TEXT,context_unit_ref TEXT,sentence_unit_ref TEXT,start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,speaker_role TEXT,speaker_status TEXT,speaker_candidates_json TEXT,evidence_refs_json TEXT,kind TEXT,quote_depth INTEGER,speech_turn_id TEXT,utterer_role TEXT,performed_role TEXT,modality TEXT,attribution_basis TEXT);
            CREATE TABLE source_sentences(sentence_unit_ref TEXT,context_unit_ref TEXT,start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT);
            CREATE TABLE source_clauses(clause_id TEXT,sentence_unit_ref TEXT,context_unit_ref TEXT,start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT);
            CREATE TABLE formula_occurrences(formula_id TEXT,occurrence_id TEXT,context_unit_ref TEXT,start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT);
            CREATE TABLE formulas(formula_id TEXT,normalized_text TEXT,token_count INTEGER,occurrence_count INTEGER,reading_count INTEGER,status TEXT);
            CREATE TABLE translation_alignments(alignment_id TEXT,granularity TEXT,candidate_role TEXT,status TEXT,correspondence_shape TEXT,ordered_source_unit_refs_json TEXT,ordered_target_unit_refs_json TEXT,exact_source_text TEXT,exact_target_text TEXT,parent_paragraph_alignment_ref TEXT,reason_codes_json TEXT,competing_alignment_refs_json TEXT,semantic_equivalence_asserted INTEGER,human_acceptance INTEGER);").unwrap();
        db.execute(
            "INSERT INTO metadata VALUES(?,?)",
            params!["concept_workbench_sha256", sha(&concept_db_path)],
        )
        .unwrap();
        db.execute(
            "INSERT INTO metadata VALUES(?,?)",
            params!["method_version", "synthetic-original-v1"],
        )
        .unwrap();
        let count = text.chars().count();
        db.execute(
            "INSERT INTO contexts VALUES(?,?,?,?,?,?,?)",
            params!["ctx", "de", 1, 1, "r1", text, hash(text)],
        )
        .unwrap();
        for (i, start) in starts.iter().enumerate() {
            db.execute(
                "INSERT INTO occurrence_spans VALUES(?,?,?,?,?,?,?,?)",
                params![
                    format!("old-{i}"),
                    "ctx",
                    format!("surface-{i}"),
                    *start as i64,
                    (*start + "Schicksal".chars().count()) as i64,
                    "Schicksal",
                    hash("Schicksal"),
                    "proposed"
                ],
            )
            .unwrap();
        }
        let boundary = starts[0] + 10;
        for (id, a, b, role, performed, modality) in [
            ("quoted", 0, boundary, "dwarf", None, None),
            (
                "narrated",
                boundary,
                count,
                "narrator",
                Some("evil_spirit"),
                Some("hypothetical"),
            ),
        ] {
            let exact = text.chars().skip(a).take(b - a).collect::<String>();
            db.execute(
                "INSERT INTO discourse_segments VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                params![
                    id,
                    "ctx",
                    "sentence",
                    a as i64,
                    b as i64,
                    exact,
                    hash(&exact),
                    role,
                    "proposed",
                    format!("[\"{role}\"]"),
                    "[\"cue-source\"]",
                    "quoted_speech",
                    1,
                    "turn",
                    "zarathustra",
                    performed,
                    modality,
                    "reporting_clause"
                ],
            )
            .unwrap();
        }
        db.execute(
            "INSERT INTO source_sentences VALUES(?,?,?,?,?,?)",
            params!["sentence", "ctx", 0, count as i64, text, hash(text)],
        )
        .unwrap();
        let clause = text.chars().take(boundary).collect::<String>();
        db.execute(
            "INSERT INTO source_clauses VALUES(?,?,?,?,?,?,?,?)",
            params![
                "clause",
                "sentence",
                "ctx",
                0,
                boundary as i64,
                clause,
                hash(&clause),
                "ambiguous"
            ],
        )
        .unwrap();
        for (id, a, b) in [("member", 0, boundary), ("nearby", count - 10, count)] {
            let exact = text.chars().skip(a).take(b - a).collect::<String>();
            db.execute(
                "INSERT INTO formulas VALUES(?,?,?,?,?,?)",
                params![id, exact.to_lowercase(), 4, 3, 2, "proposed"],
            )
            .unwrap();
            db.execute(
                "INSERT INTO formula_occurrences VALUES(?,?,?,?,?,?,?,?)",
                params![
                    id,
                    format!("{id}-occ"),
                    "ctx",
                    a as i64,
                    b as i64,
                    exact,
                    hash(&exact),
                    "proposed"
                ],
            )
            .unwrap();
        }
        db.execute(
            "INSERT INTO translation_alignments VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                "align",
                "sentence",
                "primary",
                "proposed",
                "1:1",
                "[\"sentence\"]",
                "[\"ru-sentence\"]",
                text,
                "Русское сравнение",
                "paragraph-align",
                "[]",
                "[]",
                0,
                0
            ],
        )
        .unwrap();
        drop(db);
        fs::set_permissions(&reading_path, fs::Permissions::from_mode(0o600)).unwrap();
        write(source, "policy.json", b"{\"status\":\"proposed\"}\n", false);
        write(analysis, "companion.json", b"{}\n", false);
        write(analysis,READING_MANIFEST_REF,&serde_json::to_vec(&json!({"schema_version":"tos_zarathustra_reading_manifest_v1",
            "private_database":{"ref":"reading #db.sqlite3","sha256":sha(&reading_path),"mode":"0600"},
            "inputs":[{"ref":"policy.json","sha256":sha(&source.join("policy.json")),"role":"source_visible_voice_policy"}],
            "artifacts":[{"ref":"companion.json","sha256":sha(&analysis.join("companion.json"))}],
            "accepted":false,"human_review":false,"canon_effect":false,"publication_posture":"excluded_from_public_bundle"})).unwrap(),false);
        Self {
            root,
            roots,
            retained_for_consumers: false,
        }
    }
    /// Give the existing disposable source members task/request IDs admitted
    /// by the strict WordAnalysis schema, then re-seal their exact fixity.
    /// Used by QRY and actual CLI controls; this grants no source admission.
    pub fn with_word_analysis(self) -> Self {
        let fixture = self;
        let source = &fixture.roots.source_root;
        let manifest_ref = std::path::Path::new(DEFAULT_REQUEST_REF)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("manifest.v1.json");
        let manifest_path = source.join(manifest_ref);
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        let artifact = |suffix: &str| {
            manifest["artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["ref"].as_str().unwrap())
                .find(|reference| reference.ends_with(suffix))
                .unwrap()
                .to_owned()
        };
        let concept_path = source.join(artifact("/concept-candidate.v1.json"));
        let occurrences_path = source.join(artifact("/occurrence-spine.v1.jsonl"));
        let tasks_path = source.join(artifact("/english-on-demand-worklist.v1.jsonl"));
        let mut concept: Value = serde_json::from_slice(&fs::read(&concept_path).unwrap()).unwrap();
        concept["request_id"] = json!(format!(
            "tos.annotation.concept-request.sid-{}",
            &hash("word-analysis synthetic existing reading fixture request")[..32]
        ));
        fs::write(&concept_path, serde_json::to_vec(&concept).unwrap()).unwrap();
        // The existing fixture has three occurrence rows but only the first task.
        // Supply schema-shaped task IDs for these exact existing refs, including
        // rank two; do not invent another source or duplicate the query engine.
        let tasks = fs::read_to_string(occurrences_path)
            .unwrap()
            .lines()
            .map(|line| {
                let occurrence: Value = serde_json::from_str(line).unwrap();
                let source_ref = occurrence["occurrence_candidate_id"].as_str().unwrap();
                let task = json!({
                    "source_occurrence_ref": source_ref,
                    "english_task_id": format!(
                        "tos.annotation.english-translation-task.sid-{}",
                        &hash(source_ref)[..32]
                    )
                });
                serde_json::to_string(&task).unwrap() + "\n"
            })
            .collect::<String>();
        fs::write(tasks_path, tasks).unwrap();
        // Re-seal the fixture's own exact membership rather than disabling fixity
        // or substituting a permissive schema. No private DB bytes were changed.
        for field in ["artifacts", "private_artifacts"] {
            for row in manifest[field].as_array_mut().unwrap() {
                row["sha256"] = json!(sha(&source.join(row["ref"].as_str().unwrap())));
            }
        }
        fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        fixture
    }

    /// Opt-in disposition of this SAME successful synthetic fixture. The
    /// receipt remains provisional until its owning native test is accepted.
    /// No copied source, new producer, public rights or semantic authority.
    pub fn retain_word_for_consumers(
        &mut self,
        receipt: &std::path::Path,
        task_raw: &[u8],
        deadline: std::time::Instant,
    ) {
        use std::io::{Read, Seek, SeekFrom, Write};
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        use std::path::Path;
        const LOGICAL_CAP: u64 = 1_048_576;
        const ALLOCATED_CAP: u64 = 4_194_304;
        const RECEIPT_CAP: usize = 65_536;
        // This native fixture target is Linux-only, as its existing unix modes.
        const O_NOFOLLOW: i32 = 0o400000;
        const O_NONBLOCK: i32 = 0o4000;
        fn stamp(m: &fs::Metadata) -> Value {
            json!([
                m.dev(),
                m.ino(),
                m.uid(),
                m.mode(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec()
            ])
        }
        fn fresh(path: &Path, raw: &[u8], deadline: std::time::Instant) -> Value {
            assert!(std::time::Instant::now() < deadline);
            let mut held = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .unwrap();
            held.write_all(raw).unwrap();
            held.flush().unwrap();
            let identity = stamp(&held.metadata().unwrap());
            assert_eq!(identity, stamp(&fs::symlink_metadata(path).unwrap()));
            held.seek(SeekFrom::Start(0)).unwrap();
            let mut observed = Vec::new();
            (&mut held)
                .take(raw.len() as u64 + 1)
                .read_to_end(&mut observed)
                .unwrap();
            assert_eq!(observed, raw);
            assert_eq!(identity, stamp(&held.metadata().unwrap()));
            assert_eq!(identity, stamp(&fs::symlink_metadata(path).unwrap()));
            assert!(std::time::Instant::now() < deadline);
            identity
        }
        fn census(root: &Path, deadline: std::time::Instant) -> (Value, u64, u64) {
            let mut pending = vec![(root.to_owned(), 0usize)];
            let mut rows = Vec::new();
            let mut logical = 0u64;
            let mut allocated = 0u64;
            let mut files = 0;
            let mut directories = 0;
            while let Some((path, depth)) = pending.pop() {
                assert!(std::time::Instant::now() < deadline && depth <= 16);
                let named = fs::symlink_metadata(&path).unwrap();
                assert!(!named.file_type().is_symlink());
                allocated = allocated.checked_add(named.blocks() * 512).unwrap();
                assert!(allocated <= ALLOCATED_CAP);
                let reference = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                if named.is_dir() {
                    directories += 1;
                    assert!(directories <= 64);
                    let mut entries = Vec::new();
                    for entry in fs::read_dir(&path).unwrap() {
                        assert!(entries.len() < 128 && std::time::Instant::now() < deadline);
                        entries.push(entry.unwrap().path());
                    }
                    entries.sort();
                    pending.extend(entries.into_iter().map(|p| (p, depth + 1)));
                    assert!(pending.len() <= 128);
                    assert_eq!(stamp(&named), stamp(&fs::symlink_metadata(&path).unwrap()));
                    rows.push(json!({"ref":reference,"kind":"directory","stamp":stamp(&named)}));
                } else {
                    files += 1;
                    assert!(files <= 64 && named.is_file());
                    logical = logical.checked_add(named.len()).unwrap();
                    assert!(logical <= LOGICAL_CAP && named.len() <= LOGICAL_CAP);
                    let mut held = fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(O_NOFOLLOW | O_NONBLOCK)
                        .open(&path)
                        .unwrap();
                    assert_eq!(stamp(&named), stamp(&held.metadata().unwrap()));
                    let mut raw = Vec::new();
                    (&mut held)
                        .take(named.len() + 1)
                        .read_to_end(&mut raw)
                        .unwrap();
                    assert_eq!(raw.len() as u64, named.len());
                    assert!(std::time::Instant::now() < deadline);
                    assert_eq!(stamp(&named), stamp(&held.metadata().unwrap()));
                    assert_eq!(stamp(&named), stamp(&fs::symlink_metadata(&path).unwrap()));
                    rows.push(json!({"ref":reference,"kind":"file","bytes":named.len(),
                        "sha256":tos_foundation::Digest256::of_bytes(&raw).to_hex(),"stamp":stamp(&named)}));
                }
            }
            rows.sort_by(|a, b| a["ref"].as_str().cmp(&b["ref"].as_str()));
            (Value::Array(rows), logical, allocated)
        }
        assert!(std::time::Instant::now() < deadline);
        assert!(receipt.is_absolute() && task_raw.len() <= LOGICAL_CAP as usize);
        assert_eq!(self.root.canonicalize().unwrap(), self.root);
        assert!(
            self.roots.source_root.starts_with(&self.root)
                && self.roots.analysis_root.starts_with(&self.root)
        );
        let parent = receipt.parent().unwrap();
        assert_eq!(parent.canonicalize().unwrap(), parent);
        assert!(!parent.starts_with(&self.root));
        let parent_meta = fs::symlink_metadata(parent).unwrap();
        assert!(parent_meta.is_dir() && parent_meta.permissions().mode() & 0o077 == 0);
        assert_eq!(parent_meta.uid(), fs::metadata(&self.root).unwrap().uid());
        let (members, logical, allocated) = census(&self.root, deadline);
        let task_path = receipt.with_extension("task.json");
        assert_ne!(task_path, receipt);
        assert!(allocated + task_raw.len() as u64 + RECEIPT_CAP as u64 + 8192 <= ALLOCATED_CAP);
        let task_stamp = fresh(&task_path, task_raw, deadline);
        let packet = json!({"schema":"tos_native_word_fixture_retention_v1",
            "status":"provisional-awaiting-parent-native-test-acceptance",
            "fixture_kind":"synthetic-existing-reading-fixture",
            "fixture_root":self.root,"source_root":self.roots.source_root,
            "analysis_root":self.roots.analysis_root,"members":members,
            "logical_bytes":logical,"fixture_allocated_bytes":allocated,
            "task_ref":task_path,"task_stamp":task_stamp,"task_bytes":task_raw.len(),
            "task_sha256":tos_foundation::Digest256::of_bytes(task_raw).to_hex(),
            "query":"судьбы","language":"ru","rank":1,"include_semantic_neighbors":false,
            "provider_origin":"matching native Access build proof; no Python provider file retained",
            "source_guards_pre_post":true,
            "authority":{"accepted":false,"canon_effect":false,"grants_current_use":false,
                "public_or_local_text_unit_authority":false}});
        let raw = serde_json::to_vec(&packet).unwrap();
        assert!(raw.len() <= RECEIPT_CAP);
        fresh(receipt, &raw, deadline);
        let (after, after_logical, after_allocated) = census(&self.root, deadline);
        assert_eq!(packet["members"], after);
        assert_eq!(logical, after_logical);
        assert_eq!(allocated, after_allocated);
        assert!(
            allocated
                + fs::metadata(&task_path).unwrap().blocks() * 512
                + fs::metadata(receipt).unwrap().blocks() * 512
                <= ALLOCATED_CAP
        );
        assert!(std::time::Instant::now() < deadline);
        self.retained_for_consumers = true;
    }

    pub fn request(&self) -> ReadingSearchRequest {
        ReadingSearchRequest {
            query: "судьбы".to_owned(),
            language: "ru".to_owned(),
            limit: 1,
            include_semantic_neighbors: false,
            group_by: vec!["speaker".into(), "formula".into()],
            request_ref: None,
        }
    }
    pub fn query(
        &self,
        r: &ReadingSearchRequest,
        b: ReadingSearchBudget,
    ) -> Result<ReadingSearchResult> {
        execute_reading_search(
            &self.roots,
            &ReadingSoftware::embedded(),
            r,
            b,
            Arc::new(NoAbort),
        )
    }
    pub fn packet(&self, r: &ReadingSearchRequest) -> Value {
        serde_json::from_slice(
            &self
                .query(r, ReadingSearchBudget::local_default())
                .unwrap()
                .body,
        )
        .unwrap()
    }
}
impl Drop for ReadingFixture {
    fn drop(&mut self) {
        if !self.retained_for_consumers {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
}
