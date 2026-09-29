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
                    (*start + 8) as i64,
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
        Self { root, roots }
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
        fs::remove_dir_all(&self.root).unwrap();
    }
}
