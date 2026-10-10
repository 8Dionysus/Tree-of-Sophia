PRAGMA page_size=4096; PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;
        CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL) WITHOUT ROWID;
        CREATE TABLE contexts(context_unit_ref TEXT PRIMARY KEY,language TEXT,part INTEGER,reading_ref TEXT,
          unit_kind TEXT,witness_order INTEGER,exact_text TEXT,exact_sha256 TEXT,anchor_refs_json TEXT) WITHOUT ROWID;
        CREATE TABLE source_sentences(sentence_unit_ref TEXT PRIMARY KEY,context_unit_ref TEXT,
          start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT) WITHOUT ROWID;
        CREATE TABLE source_clauses(clause_id TEXT PRIMARY KEY,sentence_unit_ref TEXT,context_unit_ref TEXT,
          start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT) WITHOUT ROWID;
        CREATE TABLE discourse_segments(segment_id TEXT PRIMARY KEY,context_unit_ref TEXT,sentence_unit_ref TEXT,
          start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,speaker_role TEXT,speaker_status TEXT,
          speaker_candidates_json TEXT,evidence_refs_json TEXT,kind TEXT,quote_depth INTEGER,speech_turn_id TEXT,
          utterer_role TEXT,attribution_basis TEXT,performed_role TEXT,modality TEXT) WITHOUT ROWID;
        CREATE INDEX discourse_context_idx ON discourse_segments(context_unit_ref,start_offset,end_offset);
        CREATE TABLE quote_events(event_id TEXT PRIMARY KEY,context_unit_ref TEXT,offset INTEGER,event_json TEXT) WITHOUT ROWID;
        CREATE TABLE occurrence_spans(existing_occurrence_ref TEXT,context_unit_ref TEXT,surface_unit_ref TEXT,
          start_offset INTEGER,end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT,
          PRIMARY KEY(existing_occurrence_ref,context_unit_ref,start_offset)) WITHOUT ROWID;
        CREATE TABLE formulas(formula_id TEXT PRIMARY KEY,normalized_text TEXT,token_count INTEGER,
          occurrence_count INTEGER,reading_count INTEGER,status TEXT) WITHOUT ROWID;
        CREATE TABLE formula_occurrences(formula_id TEXT,occurrence_id TEXT,context_unit_ref TEXT,start_offset INTEGER,
          end_offset INTEGER,exact_text TEXT,exact_sha256 TEXT,status TEXT,
          PRIMARY KEY(occurrence_id,context_unit_ref,start_offset)) WITHOUT ROWID;
        CREATE INDEX formula_context_idx ON formula_occurrences(context_unit_ref,start_offset,end_offset);
        CREATE TABLE formula_relations(relation_id TEXT PRIMARY KEY,relation_json TEXT) WITHOUT ROWID;
        CREATE TABLE translation_alignments(alignment_id TEXT PRIMARY KEY,claim_id TEXT NOT NULL,granularity TEXT NOT NULL,
          part INTEGER NOT NULL,parent_paragraph_alignment_ref TEXT NOT NULL,parent_sentence_alignment_ref TEXT,
          candidate_role TEXT NOT NULL,correspondence_shape TEXT NOT NULL,ordered_source_unit_refs_json TEXT NOT NULL,
          ordered_target_unit_refs_json TEXT NOT NULL,exact_source_text TEXT NOT NULL,exact_target_text TEXT NOT NULL,
          score_millionths INTEGER NOT NULL,score_components_json TEXT NOT NULL,status TEXT NOT NULL,reason_codes_json TEXT NOT NULL,
          competing_alignment_refs_json TEXT NOT NULL,semantic_equivalence_asserted INTEGER NOT NULL,human_acceptance INTEGER NOT NULL) WITHOUT ROWID;
