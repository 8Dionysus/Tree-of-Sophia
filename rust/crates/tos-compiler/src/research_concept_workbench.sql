PRAGMA page_size=4096; PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF;
          CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL) WITHOUT ROWID;
          CREATE TABLE context_units(context_unit_ref TEXT PRIMARY KEY,language TEXT NOT NULL,part INTEGER NOT NULL,
            reading_ref TEXT NOT NULL,unit_kind TEXT NOT NULL,witness_order INTEGER NOT NULL,exact_text TEXT NOT NULL,
            exact_sha256 TEXT NOT NULL,analysis_tokens_json TEXT NOT NULL,alignment_links_json TEXT NOT NULL,
            speaker_role TEXT NOT NULL,speaker_status TEXT NOT NULL) WITHOUT ROWID;
          CREATE TABLE exact_occurrences(existing_occurrence_ref TEXT PRIMARY KEY,language TEXT NOT NULL,part INTEGER NOT NULL,
            context_unit_ref TEXT,reading_ref TEXT,unit_kind TEXT NOT NULL,witness_order INTEGER NOT NULL,token_ordinal INTEGER NOT NULL,
            exact_form TEXT NOT NULL,exact_form_sha256 TEXT NOT NULL,normalized_form TEXT NOT NULL,
            normalized_form_sha256 TEXT NOT NULL,analysis_key TEXT NOT NULL,analysis_key_sha256 TEXT NOT NULL,
            source_locator_sha256 TEXT NOT NULL,start_offset INTEGER NOT NULL,end_offset INTEGER NOT NULL,
            work_ref TEXT NOT NULL,in_work_scope INTEGER NOT NULL,scope_exclusion_code TEXT) WITHOUT ROWID;
          CREATE TABLE analysis_forms(language TEXT NOT NULL,analysis_key TEXT NOT NULL,analysis_key_sha256 TEXT NOT NULL,
            occurrence_count INTEGER NOT NULL,exact_variant_count INTEGER NOT NULL,morphology_state TEXT NOT NULL,
            PRIMARY KEY(language,analysis_key)) WITHOUT ROWID;
          CREATE INDEX occurrence_analysis_idx ON exact_occurrences(language,analysis_key);
          CREATE INDEX occurrence_context_idx ON exact_occurrences(context_unit_ref,token_ordinal);
          CREATE INDEX context_order_idx ON context_units(language,witness_order);