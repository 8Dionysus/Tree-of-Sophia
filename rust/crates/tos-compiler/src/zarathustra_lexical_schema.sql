PRAGMA page_size=4096;
PRAGMA journal_mode=OFF;
PRAGMA synchronous=OFF;
PRAGMA temp_store=MEMORY;
CREATE TABLE metadata(
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            ) WITHOUT ROWID;
CREATE TABLE source_items(
              item_ref TEXT PRIMARY KEY,
              part_order INTEGER NOT NULL,
              file_id TEXT NOT NULL,
              file_sha256 TEXT NOT NULL,
              language TEXT NOT NULL,
              edition_ref TEXT NOT NULL,
              manifest_ref TEXT NOT NULL,
              resource_inventory_ref TEXT NOT NULL,
              rights_ref TEXT NOT NULL
            ) WITHOUT ROWID;
CREATE TABLE pages(
              item_ref TEXT NOT NULL,
              resource_id TEXT NOT NULL,
              tei_path TEXT NOT NULL,
              facs_ref TEXT,
              page_label TEXT,
              PRIMARY KEY(item_ref, resource_id)
            ) WITHOUT ROWID;
CREATE TABLE sections(
              item_ref TEXT NOT NULL,
              resource_id TEXT NOT NULL,
              tei_path TEXT NOT NULL,
              tei_depth INTEGER,
              page_label TEXT,
              tei_n TEXT,
              tei_type TEXT,
              parent_resource_id TEXT,
              PRIMARY KEY(item_ref, resource_id)
            ) WITHOUT ROWID;
CREATE TABLE forms(
              form_key TEXT PRIMARY KEY,
              exact_form TEXT NOT NULL,
              normalized_form TEXT NOT NULL,
              exact_form_sha256 TEXT NOT NULL,
              normalized_form_sha256 TEXT NOT NULL,
              occurrence_count INTEGER NOT NULL
            ) WITHOUT ROWID;
CREATE TABLE occurrences(
              occurrence_id TEXT PRIMARY KEY,
              item_ref TEXT NOT NULL,
              token_ordinal INTEGER NOT NULL,
              form_key TEXT NOT NULL,
              exact_form TEXT NOT NULL,
              normalized_form TEXT NOT NULL,
              exact_form_sha256 TEXT NOT NULL,
              normalized_form_sha256 TEXT NOT NULL,
              page_resource_id TEXT NOT NULL,
              section_resource_id TEXT,
              text_node_path TEXT NOT NULL,
              start_offset INTEGER NOT NULL,
              end_offset INTEGER NOT NULL,
              editorial_status TEXT NOT NULL
            ) WITHOUT ROWID;
CREATE INDEX occurrences_exact_idx ON occurrences(exact_form);
CREATE INDEX occurrences_normalized_idx ON occurrences(normalized_form);
CREATE INDEX occurrences_page_idx ON occurrences(item_ref, page_resource_id);
CREATE INDEX occurrences_section_idx ON occurrences(item_ref, section_resource_id);
CREATE VIRTUAL TABLE page_fts USING fts5(
              item_ref UNINDEXED,
              page_resource_id UNINDEXED,
              section_refs UNINDEXED,
              exact_text,
              normalized_text,
              lemma,
              phrase,
              prefix,
              section,
              page,
              language,
              edition,
              translation,
              sign_candidate,
              tokenize='unicode61 remove_diacritics 0'
            );
PRAGMA compile_options;
