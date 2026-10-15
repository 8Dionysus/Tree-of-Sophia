-- Explicitly initialized, private derived projection. This file is deliberately
-- separate from durable_schema.sql, whose bytes bind the source schema profile.
-- Projection rows are derived from retained source history and are never a
-- second source store or an authority for external owners.

-- Source-text v1 uses PostgreSQL's trusted trigram operator class only for a
-- bounded candidate seek.  Unicode and field semantics remain in tos-query;
-- this extension never becomes a second text normalizer.
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS cmd2_history_projection_event_seek
  ON cmd2_history(domain,commit_seq,member_slot);

-- Bounded historical lookup for a selected source path at an outbox event cut.
CREATE INDEX IF NOT EXISTS cmd2_history_projection_subject_cut_seek
  ON cmd2_history(domain,subject,commit_seq DESC);

CREATE TABLE IF NOT EXISTS cmd2_projection_consumer (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  consumer_id text NOT NULL CHECK (octet_length(consumer_id) BETWEEN 1 AND 256),
  profile_id text NOT NULL CHECK (octet_length(profile_id) BETWEEN 1 AND 128),
  profile_version text NOT NULL CHECK (octet_length(profile_version) BETWEEN 1 AND 128),
  source_contract_digest char(64) NOT NULL,
  cursor_seq bigint NOT NULL DEFAULT 0 CHECK (cursor_seq >= 0),
  observed_head_seq bigint,
  observed_rights_version bigint,
  observed_rights_allowed boolean,
  observed_rule_version bigint,
  observed_contract_digest char(64),
  observed_schema_profile_digest char(64),
  observed_source_generation bigint,
  observed_source_complete boolean,
  -- NULL means this consumer predates source-text v1.  It is never filled in
  -- by a migration or by an apply; a new consumer must replay from seq 0.
  source_text_search_profile text,
  source_text_search_profile_version integer,
  PRIMARY KEY (domain, consumer_id),
  CONSTRAINT cmd2_projection_consumer_currentness_check CHECK (
    cursor_seq = 0 OR (
      observed_head_seq IS NOT NULL AND observed_rights_version IS NOT NULL AND
      observed_rights_allowed IS NOT NULL AND observed_rule_version IS NOT NULL AND
      observed_contract_digest IS NOT NULL AND observed_schema_profile_digest IS NOT NULL AND
      observed_source_complete IS NOT NULL
    )
  ),
  CONSTRAINT cmd2_projection_consumer_source_text_profile_check CHECK (
    (source_text_search_profile IS NULL AND source_text_search_profile_version IS NULL) OR
    (source_text_search_profile IS NOT NULL AND
     source_text_search_profile_version IS NOT NULL AND
     source_text_search_profile_version > 0)
  )
);

ALTER TABLE cmd2_projection_consumer
  ADD COLUMN IF NOT EXISTS source_text_search_profile text;
ALTER TABLE cmd2_projection_consumer
  ADD COLUMN IF NOT EXISTS source_text_search_profile_version integer;

-- Unpublished, resumable initial text projection. The ordinary consumer cursor
-- stays zero until the complete selected source cut has been traversed. Backup
-- may retain these derived rows; a changed source fence requires a new consumer.
CREATE TABLE IF NOT EXISTS cmd2_projection_text_bootstrap (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  snapshot_json text NOT NULL CHECK (octet_length(snapshot_json) <= 8192),
  after_path text NOT NULL DEFAULT '' CHECK (octet_length(after_path) <= 4096),
  processed_sources bigint NOT NULL DEFAULT 0 CHECK (processed_sources >= 0),
  completed boolean NOT NULL DEFAULT false,
  PRIMARY KEY(domain,consumer_id),
  FOREIGN KEY(domain,consumer_id)
    REFERENCES cmd2_projection_consumer(domain,consumer_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS cmd2_projection_document (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  source_path text NOT NULL,
  document_id text NOT NULL,
  source_revision bigint NOT NULL CHECK (source_revision > 0),
  title text NOT NULL,
  body text NOT NULL,
  aliases text[] NOT NULL DEFAULT '{}',
  aliases_text text NOT NULL DEFAULT '',
  identity text NOT NULL,
  document_type text NOT NULL,
  -- These are produced by the Rust FND Unicode16 normalizer in the same
  -- outbox publication transaction.  They are nullable for legacy rows so a
  -- schema upgrade cannot advertise an old consumer as source-text ready.
  source_text_identity_norm text,
  source_text_title_norm text,
  source_text_aliases_norm text,
  source_text_body_norm text,
  source_text_norm text,
  search_vector tsvector GENERATED ALWAYS AS (
    pg_catalog.setweight(
      pg_catalog.to_tsvector('pg_catalog.simple'::pg_catalog.regconfig, coalesce(title, '')), 'A'
    ) ||
    pg_catalog.setweight(
      pg_catalog.to_tsvector('pg_catalog.simple'::pg_catalog.regconfig, coalesce(aliases_text, '')), 'B'
    ) ||
    pg_catalog.setweight(
      pg_catalog.to_tsvector('pg_catalog.simple'::pg_catalog.regconfig, coalesce(body, '')), 'C'
    )
  ) STORED,
  PRIMARY KEY (domain, consumer_id, source_path, document_id),
  FOREIGN KEY (domain, consumer_id)
    REFERENCES cmd2_projection_consumer(domain, consumer_id) ON DELETE CASCADE
);

ALTER TABLE cmd2_projection_document
  ADD COLUMN IF NOT EXISTS source_text_identity_norm text;
ALTER TABLE cmd2_projection_document
  ADD COLUMN IF NOT EXISTS source_text_title_norm text;
ALTER TABLE cmd2_projection_document
  ADD COLUMN IF NOT EXISTS source_text_aliases_norm text;
ALTER TABLE cmd2_projection_document
  ADD COLUMN IF NOT EXISTS source_text_body_norm text;
ALTER TABLE cmd2_projection_document
  ADD COLUMN IF NOT EXISTS source_text_norm text;

CREATE INDEX IF NOT EXISTS cmd2_projection_document_text_gin
  ON cmd2_projection_document USING gin(search_vector);
CREATE INDEX IF NOT EXISTS cmd2_projection_document_identity_seek
  ON cmd2_projection_document(domain,consumer_id,identity COLLATE "C",source_path COLLATE "C",document_id COLLATE "C");
-- Candidate-only LIKE seek.  The adapter still fetches bounded raw fields and
-- lets tos-query perform the final Unicode16 recheck and rank.
CREATE INDEX IF NOT EXISTS cmd2_projection_document_source_text_trgm
  ON cmd2_projection_document USING gin(source_text_norm gin_trgm_ops);

CREATE TABLE IF NOT EXISTS cmd2_projection_dependency (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  source_path text NOT NULL,
  document_id text NOT NULL,
  dependency_kind text NOT NULL CHECK (dependency_kind = 'source'),
  dependency_path text NOT NULL,
  PRIMARY KEY (domain,consumer_id,source_path,document_id,dependency_kind,dependency_path),
  FOREIGN KEY (domain,consumer_id,source_path,document_id)
    REFERENCES cmd2_projection_document(domain,consumer_id,source_path,document_id)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS cmd2_projection_dependency_path_seek
  ON cmd2_projection_dependency(domain,consumer_id,dependency_kind,dependency_path COLLATE "C");

-- Event-local source-navigation cohorts are private derived state. The
-- producer's authored event identity remains distinct from CMD's committed
-- outbox identity; neither row family is a source or semantic authority.
CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_consumer (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  consumer_id text NOT NULL CHECK (octet_length(consumer_id) BETWEEN 1 AND 256),
  profile_id text NOT NULL CHECK (octet_length(profile_id) BETWEEN 1 AND 128),
  profile_version text NOT NULL CHECK (octet_length(profile_version) BETWEEN 1 AND 128),
  source_contract_digest char(64) NOT NULL CHECK (source_contract_digest ~ '^[0-9a-f]{64}$'),
  cursor_seq bigint NOT NULL DEFAULT 0 CHECK (cursor_seq >= 0),
  source_revision char(64),
  last_event_id text,
  last_input_root char(64),
  last_output_root char(64),
  coverage_seq bigint NOT NULL DEFAULT 0 CHECK (coverage_seq >= 0),
  coverage_root char(64) CHECK (coverage_root IS NULL OR coverage_root ~ '^[0-9a-f]{64}$'),
  adjacency_seq bigint NOT NULL DEFAULT 0 CHECK (adjacency_seq >= 0),
  adjacency_root char(64) CHECK (adjacency_root IS NULL OR adjacency_root ~ '^[0-9a-f]{64}$'),
  adjacency_ready boolean NOT NULL DEFAULT false,
  verified_prefix_version smallint CHECK (verified_prefix_version IS NULL OR verified_prefix_version IN (1,2,3)),
  verified_prefix_seq bigint CHECK (verified_prefix_seq IS NULL OR verified_prefix_seq >= 0),
  verified_prefix_coverage_root char(64) CHECK (verified_prefix_coverage_root IS NULL OR verified_prefix_coverage_root ~ '^[0-9a-f]{64}$'),
  verified_prefix_cohort_root char(64) CHECK (verified_prefix_cohort_root IS NULL OR verified_prefix_cohort_root ~ '^[0-9a-f]{64}$'),
  verified_prefix_digest char(64) CHECK (verified_prefix_digest IS NULL OR verified_prefix_digest ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (domain,consumer_id),
  CONSTRAINT cmd2_projection_navigation_consumer_cursor_check CHECK (
    (cursor_seq = 0 AND source_revision IS NULL AND last_event_id IS NULL AND
      last_input_root IS NULL AND last_output_root IS NULL) OR
    (cursor_seq > 0 AND source_revision IS NOT NULL AND
      source_revision ~ '^[0-9a-f]{64}$' AND last_event_id IS NOT NULL AND
      octet_length(last_event_id) BETWEEN 1 AND 256 AND
      last_input_root IS NOT NULL AND last_input_root ~ '^[0-9a-f]{64}$' AND
      last_output_root IS NOT NULL AND last_output_root ~ '^[0-9a-f]{64}$')
  ),
  CONSTRAINT cmd2_projection_navigation_consumer_verified_prefix_check CHECK (
    (verified_prefix_version IS NULL AND verified_prefix_seq IS NULL AND verified_prefix_coverage_root IS NULL AND
      verified_prefix_cohort_root IS NULL AND verified_prefix_digest IS NULL) OR
    (verified_prefix_version IS NOT NULL AND verified_prefix_version IN (1,2,3) AND verified_prefix_seq IS NOT NULL AND verified_prefix_coverage_root IS NOT NULL AND
      verified_prefix_cohort_root IS NOT NULL AND verified_prefix_digest IS NOT NULL)
  )
);
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS coverage_seq bigint NOT NULL DEFAULT 0 CHECK (coverage_seq >= 0);
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS coverage_root char(64) CHECK (coverage_root IS NULL OR coverage_root ~ '^[0-9a-f]{64}$');
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS adjacency_seq bigint NOT NULL DEFAULT 0 CHECK (adjacency_seq >= 0);
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS adjacency_root char(64) CHECK (adjacency_root IS NULL OR adjacency_root ~ '^[0-9a-f]{64}$');
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS adjacency_ready boolean NOT NULL DEFAULT false;
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS verified_prefix_version smallint CHECK (verified_prefix_version IS NULL OR verified_prefix_version IN (1,2,3));
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS verified_prefix_seq bigint CHECK (verified_prefix_seq IS NULL OR verified_prefix_seq >= 0);
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS verified_prefix_coverage_root char(64) CHECK (verified_prefix_coverage_root IS NULL OR verified_prefix_coverage_root ~ '^[0-9a-f]{64}$');
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS verified_prefix_cohort_root char(64) CHECK (verified_prefix_cohort_root IS NULL OR verified_prefix_cohort_root ~ '^[0-9a-f]{64}$');
ALTER TABLE cmd2_projection_navigation_consumer
  ADD COLUMN IF NOT EXISTS verified_prefix_digest char(64) CHECK (verified_prefix_digest IS NULL OR verified_prefix_digest ~ '^[0-9a-f]{64}$');
-- v1 checkpoints do not bind exact active manifests or range terminal roots.
-- Keep their format marker so reads fail closed and an explicit full bootstrap
-- can replace only the derived navigation projection at the current source cut.
UPDATE cmd2_projection_navigation_consumer
SET verified_prefix_version=1
WHERE verified_prefix_version IS NULL AND verified_prefix_seq IS NOT NULL
  AND verified_prefix_coverage_root IS NOT NULL
  AND verified_prefix_cohort_root IS NOT NULL AND verified_prefix_digest IS NOT NULL;
ALTER TABLE cmd2_projection_navigation_consumer
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_consumer_verified_prefix_check;
ALTER TABLE cmd2_projection_navigation_consumer
  ADD CONSTRAINT cmd2_projection_navigation_consumer_verified_prefix_check CHECK (
    (verified_prefix_version IS NULL AND verified_prefix_seq IS NULL AND verified_prefix_coverage_root IS NULL AND
      verified_prefix_cohort_root IS NULL AND verified_prefix_digest IS NULL) OR
    (verified_prefix_version IS NOT NULL AND verified_prefix_version IN (1,2,3) AND verified_prefix_seq IS NOT NULL AND verified_prefix_coverage_root IS NOT NULL AND
      verified_prefix_cohort_root IS NOT NULL AND verified_prefix_digest IS NOT NULL)
  );

-- Preserve old format markers: source-backed bootstrap is the only v3 admission.
ALTER TABLE cmd2_projection_navigation_consumer
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_consumer_verified_prefix_version_check;
ALTER TABLE cmd2_projection_navigation_consumer
  ADD CONSTRAINT cmd2_projection_navigation_consumer_verified_prefix_version_check
  CHECK (verified_prefix_version IS NULL OR verified_prefix_version IN (1,2,3));

-- Immutable compressed radix nodes; the exact owner tuple remains in each leaf.
-- Nodes and root pointers are transactional PG state and travel in native backup.
CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_cohort_tree (
  domain text NOT NULL, consumer_id text NOT NULL,
  root_json text NOT NULL CHECK (octet_length(root_json) BETWEEN 4 AND 512),
  PRIMARY KEY(domain,consumer_id),
  FOREIGN KEY(domain,consumer_id) REFERENCES cmd2_projection_navigation_consumer(domain,consumer_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_cohort_tree_node (
  domain text NOT NULL, consumer_id text NOT NULL,
  node_sha256 text NOT NULL CHECK (node_sha256 ~ '^[0-9a-f]{64}$'),
  node_json text NOT NULL CHECK (octet_length(node_json) BETWEEN 1 AND 65536),
  PRIMARY KEY(domain,consumer_id,node_sha256),
  FOREIGN KEY(domain,consumer_id) REFERENCES cmd2_projection_navigation_consumer(domain,consumer_id) ON DELETE CASCADE
);

-- Explicit initial construction retains exact Original carriers and its
-- selected-cut/keyset progress independently of publication. Cursor zero and
-- adjacency_ready=false keep every incomplete stage unreadable.
CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_initial (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  selection_json text NOT NULL CHECK (octet_length(selection_json) <= 1048576),
  header_raw text NOT NULL CHECK (octet_length(header_raw) <= 1048576),
  position_json text NOT NULL CHECK (octet_length(position_json) <= 8192),
  PRIMARY KEY (domain,consumer_id),
  FOREIGN KEY (domain,consumer_id)
    REFERENCES cmd2_projection_navigation_consumer(domain,consumer_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_initial_row (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  collection text NOT NULL CHECK (collection IN ('nodes','edges','rights')),
  row_key text NOT NULL CHECK (octet_length(row_key) BETWEEN 1 AND 512),
  row_id text NOT NULL CHECK (octet_length(row_id) BETWEEN 1 AND
    CASE WHEN collection='edges' THEN 512 ELSE 256 END),
  ordinal bigint,
  row_json text NOT NULL CHECK (octet_length(row_json) <= 4194304),
  row_sha256 char(64) NOT NULL CHECK (row_sha256 ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (domain,consumer_id,collection,row_key),
  FOREIGN KEY (domain,consumer_id)
    REFERENCES cmd2_projection_navigation_initial(domain,consumer_id) ON DELETE CASCADE,
  CHECK ((collection='rights' AND ordinal IS NOT NULL AND ordinal>=0) OR
    (collection IN ('nodes','edges') AND ordinal IS NULL AND row_key=row_id))
);
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_initial_row_seek
  ON cmd2_projection_navigation_initial_row(domain,consumer_id,collection,row_key COLLATE "C");

-- One exact whole-source bootstrap, tied to the owner-selected managed model.
-- Its rows are separate from event cohorts so an event update can replace a
-- source owner's baseline contribution without confusing it with provenance.
CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_bootstrap (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  through_seq bigint NOT NULL CHECK (through_seq > 0),
  source_revision char(64) NOT NULL CHECK (source_revision ~ '^[0-9a-f]{64}$'),
  bootstrap_root char(64) NOT NULL CHECK (bootstrap_root ~ '^[0-9a-f]{64}$'),
  owner_binding_json text NOT NULL,
  header_raw text NOT NULL CHECK (octet_length(header_raw) <= 1048576),
  header_sha256 char(64) NOT NULL CHECK (header_sha256 ~ '^[0-9a-f]{64}$'),
  node_count bigint NOT NULL CHECK (node_count >= 0),
  edge_count bigint NOT NULL CHECK (edge_count >= 0),
  -- `right_count` is the original receipt/header count; the normalized
  -- query-facing carriers may have a different cardinality.
  right_count bigint NOT NULL CHECK (right_count >= 0),
  projected_right_count bigint NOT NULL DEFAULT 0 CHECK (projected_right_count >= 0),
  diagnostic_count bigint NOT NULL CHECK (diagnostic_count >= 0),
  PRIMARY KEY (domain,consumer_id),
  FOREIGN KEY (domain,consumer_id)
    REFERENCES cmd2_projection_navigation_consumer(domain,consumer_id) ON DELETE CASCADE
);
ALTER TABLE cmd2_projection_navigation_bootstrap
  ADD COLUMN IF NOT EXISTS projected_right_count bigint NOT NULL DEFAULT 0
  CHECK (projected_right_count >= 0);

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_bootstrap_owner (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  owner_source_path text NOT NULL,
  source_sha256 char(64) NOT NULL CHECK (source_sha256 ~ '^[0-9a-f]{64}$'),
  size_bytes bigint NOT NULL CHECK (size_bytes > 0),
  PRIMARY KEY (domain,consumer_id,owner_source_path),
  FOREIGN KEY (domain,consumer_id)
    REFERENCES cmd2_projection_navigation_bootstrap(domain,consumer_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_bootstrap_row (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  row_kind text NOT NULL CHECK (row_kind IN ('node','edge','right','diagnostic')),
  row_id text NOT NULL CHECK (octet_length(row_id) BETWEEN 1 AND
    CASE WHEN row_kind='edge' THEN 512 ELSE 256 END),
  owner_source_path text NOT NULL,
  owner_record_id text NOT NULL CHECK (octet_length(owner_record_id) BETWEEN 1 AND 256),
  active boolean NOT NULL DEFAULT true,
  visible boolean,
  from_id text,
  to_id text,
  row_json text NOT NULL CHECK (octet_length(row_json) <= 4194304),
  row_sha256 char(64) NOT NULL CHECK (row_sha256 ~ '^[0-9a-f]{64}$'),
  CONSTRAINT cmd2_projection_navigation_bootstrap_row_visibility_check CHECK (
    (row_kind IN ('node','edge') AND visible IS NOT NULL) OR
    (row_kind IN ('right','diagnostic') AND visible IS NULL)
  ),
  PRIMARY KEY (domain,consumer_id,row_kind,row_id,owner_source_path,owner_record_id),
  FOREIGN KEY (domain,consumer_id,owner_source_path)
    REFERENCES cmd2_projection_navigation_bootstrap_owner(domain,consumer_id,owner_source_path)
    ON DELETE CASCADE
);
ALTER TABLE cmd2_projection_navigation_bootstrap_row
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_bootstrap_row_row_id_check;
ALTER TABLE cmd2_projection_navigation_bootstrap_row
  ADD CONSTRAINT cmd2_projection_navigation_bootstrap_row_row_id_check
    CHECK (octet_length(row_id) BETWEEN 1 AND CASE WHEN row_kind='edge' THEN 512 ELSE 256 END);
ALTER TABLE cmd2_projection_navigation_bootstrap_row
  ADD COLUMN IF NOT EXISTS from_id text;
ALTER TABLE cmd2_projection_navigation_bootstrap_row
  ADD COLUMN IF NOT EXISTS to_id text;
UPDATE cmd2_projection_navigation_bootstrap_row
SET from_id=row_json::jsonb->>'from_id',to_id=row_json::jsonb->>'to_id'
WHERE row_kind='edge' AND (from_id IS NULL OR to_id IS NULL);

CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_bootstrap_row_owner_seek
  ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,owner_source_path COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_bootstrap_edge_from_seek
  ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,from_id COLLATE "C",row_id COLLATE "C")
  WHERE row_kind='edge' AND active AND visible;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_bootstrap_edge_from_all_seek
  ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,from_id COLLATE "C",row_id COLLATE "C")
  WHERE row_kind='edge' AND active;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_bootstrap_node_id_seek
  ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,row_id COLLATE "C")
  WHERE row_kind='node' AND active AND visible;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_bootstrap_node_id_all_seek
  ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,row_id COLLATE "C")
  WHERE row_kind='node' AND active;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_bootstrap_edge_id_seek
  ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,row_id COLLATE "C")
  WHERE row_kind='edge' AND active;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_bootstrap_edge_to_seek
  ON cmd2_projection_navigation_bootstrap_row(domain,consumer_id,to_id COLLATE "C",row_id COLLATE "C")
  WHERE row_kind='edge' AND active;

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_adjacency_certificate (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  node_id text NOT NULL CHECK (octet_length(node_id) BETWEEN 1 AND 256),
  edge_count bigint NOT NULL CHECK (edge_count >= 0),
  edge_digest char(64) NOT NULL CHECK (edge_digest ~ '^[0-9a-f]{64}$'),
  certificate_root char(64) NOT NULL CHECK (certificate_root ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (domain,consumer_id,node_id),
  FOREIGN KEY (domain,consumer_id)
    REFERENCES cmd2_projection_navigation_consumer(domain,consumer_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_bootstrap_dependency (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  owner_source_path text NOT NULL,
  owner_record_id text NOT NULL,
  dependency_path text NOT NULL,
  PRIMARY KEY (domain,consumer_id,owner_source_path,owner_record_id,dependency_path),
  FOREIGN KEY (domain,consumer_id,owner_source_path)
    REFERENCES cmd2_projection_navigation_bootstrap_owner(domain,consumer_id,owner_source_path)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_bootstrap_dependency_path_seek
  ON cmd2_projection_navigation_bootstrap_dependency(domain,consumer_id,dependency_path COLLATE "C");

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_coverage_event (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  event_sequence bigint NOT NULL CHECK (event_sequence > 0),
  event_id text NOT NULL CHECK (octet_length(event_id) BETWEEN 1 AND 256),
  source_path text NOT NULL,
  record_id text NOT NULL CHECK (octet_length(record_id) BETWEEN 1 AND 256),
  source_revision char(64) NOT NULL CHECK (source_revision ~ '^[0-9a-f]{64}$'),
  input_root char(64) NOT NULL CHECK (input_root ~ '^[0-9a-f]{64}$'),
  dependency_root char(64) NOT NULL CHECK (dependency_root ~ '^[0-9a-f]{64}$'),
  output_root char(64) NOT NULL CHECK (output_root ~ '^[0-9a-f]{64}$'),
  previous_coverage_root char(64) NOT NULL CHECK (previous_coverage_root ~ '^[0-9a-f]{64}$'),
  coverage_root char(64) NOT NULL CHECK (coverage_root ~ '^[0-9a-f]{64}$'),
  cohort_count integer NOT NULL DEFAULT 0 CHECK (cohort_count >= 0),
  cohort_set_root char(64) CHECK (cohort_set_root IS NULL OR cohort_set_root ~ '^[0-9a-f]{64}$'),
  cohort_manifest bytea,
  PRIMARY KEY (domain,consumer_id,event_sequence),
  CHECK ((cohort_count = 0 AND cohort_set_root IS NULL AND cohort_manifest IS NULL) OR
         (cohort_count > 0 AND cohort_set_root IS NOT NULL AND cohort_manifest IS NOT NULL)),
  FOREIGN KEY (domain,consumer_id)
    REFERENCES cmd2_projection_navigation_consumer(domain,consumer_id) ON DELETE CASCADE
);
ALTER TABLE cmd2_projection_navigation_coverage_event
  ADD COLUMN IF NOT EXISTS cohort_count integer NOT NULL DEFAULT 0 CHECK (cohort_count >= 0);
ALTER TABLE cmd2_projection_navigation_coverage_event
  ADD COLUMN IF NOT EXISTS cohort_set_root char(64) CHECK (
    cohort_set_root IS NULL OR cohort_set_root ~ '^[0-9a-f]{64}$'
  );
ALTER TABLE cmd2_projection_navigation_coverage_event
  ADD COLUMN IF NOT EXISTS cohort_manifest bytea;

-- One atomic consumer advance may cover several consecutive committed events.
-- This row retains the complete ordered event/member proof even when no owner
-- output is produced; `terminal_cohort_manifest` seals the replacement set at
-- the held head, including an empty set. The ordinary coverage_event table
-- remains the unchanged single-event format for existing consumers.
CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_coverage_range (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  cursor_before bigint NOT NULL CHECK (cursor_before >= 0),
  through_seq bigint NOT NULL CHECK (through_seq > cursor_before),
  held_source_head_seq bigint NOT NULL,
  terminal_event_id text NOT NULL CHECK (octet_length(terminal_event_id) BETWEEN 1 AND 256),
  range_root char(64) NOT NULL CHECK (range_root ~ '^[0-9a-f]{64}$'),
  range_manifest bytea NOT NULL CHECK (octet_length(range_manifest) <= 67108864),
  source_revision char(64) NOT NULL CHECK (source_revision ~ '^[0-9a-f]{64}$'),
  input_root char(64) NOT NULL CHECK (input_root ~ '^[0-9a-f]{64}$'),
  dependency_root char(64) NOT NULL CHECK (dependency_root ~ '^[0-9a-f]{64}$'),
  output_root char(64) NOT NULL CHECK (output_root ~ '^[0-9a-f]{64}$'),
  previous_coverage_root char(64) NOT NULL CHECK (previous_coverage_root ~ '^[0-9a-f]{64}$'),
  coverage_root char(64) NOT NULL CHECK (coverage_root ~ '^[0-9a-f]{64}$'),
  terminal_cohort_count integer NOT NULL CHECK (terminal_cohort_count >= 0),
  terminal_cohort_root char(64) NOT NULL CHECK (terminal_cohort_root ~ '^[0-9a-f]{64}$'),
  terminal_cohort_manifest bytea NOT NULL CHECK (octet_length(terminal_cohort_manifest) <= 67108864),
  PRIMARY KEY (domain,consumer_id,through_seq),
  CHECK (octet_length(range_manifest) > 0 AND octet_length(terminal_cohort_manifest) > 0),
  FOREIGN KEY (domain,consumer_id)
    REFERENCES cmd2_projection_navigation_consumer(domain,consumer_id) ON DELETE CASCADE
);
ALTER TABLE cmd2_projection_navigation_coverage_range
  ADD COLUMN IF NOT EXISTS held_source_head_seq bigint;
UPDATE cmd2_projection_navigation_coverage_range
   SET held_source_head_seq=through_seq
 WHERE held_source_head_seq IS NULL;
ALTER TABLE cmd2_projection_navigation_coverage_range
  ALTER COLUMN held_source_head_seq SET NOT NULL;
DO $$ BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
     WHERE conrelid='cmd2_projection_navigation_coverage_range'::regclass
       AND conname='cmd2_projection_navigation_coverage_range_held_head_check'
  ) THEN
    ALTER TABLE cmd2_projection_navigation_coverage_range
      ADD CONSTRAINT cmd2_projection_navigation_coverage_range_held_head_check
      CHECK (held_source_head_seq >= through_seq);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_coverage_range_start_seek
  ON cmd2_projection_navigation_coverage_range(domain,consumer_id,cursor_before,through_seq);

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_cohort (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  source_path text NOT NULL,
  record_id text NOT NULL CHECK (octet_length(record_id) BETWEEN 1 AND 256),
  -- CMD outbox identity and authored ToS event identity are separate facts.
  event_id text NOT NULL CHECK (octet_length(event_id) BETWEEN 1 AND 256),
  event_sequence bigint NOT NULL CHECK (event_sequence > 0),
  coverage_manifest_sha256 char(64) CHECK (
    coverage_manifest_sha256 IS NULL OR coverage_manifest_sha256 ~ '^[0-9a-f]{64}$'
  ),
  source_event_id text NOT NULL CHECK (octet_length(source_event_id) BETWEEN 1 AND 256),
  source_event_version bigint NOT NULL CHECK (source_event_version > 0),
  predecessor_source_revision char(64) NOT NULL CHECK (predecessor_source_revision ~ '^[0-9a-f]{64}$'),
  source_revision char(64) NOT NULL CHECK (source_revision ~ '^[0-9a-f]{64}$'),
  basis_kind text NOT NULL DEFAULT 'record-mutation',
  basis_dependency_path text,
  basis_dependency_sha256 char(64) CHECK (
    basis_dependency_sha256 IS NULL OR basis_dependency_sha256 ~ '^[0-9a-f]{64}$'
  ),
  basis_dependency_causes_json text,
  basis_dependency_causes_root char(64) CHECK (
    basis_dependency_causes_root IS NULL OR basis_dependency_causes_root ~ '^[0-9a-f]{64}$'
  ),
  basis_history_path text,
  basis_history_sha256 char(64) CHECK (
    basis_history_sha256 IS NULL OR basis_history_sha256 ~ '^[0-9a-f]{64}$'
  ),
  basis_claim_id text,
  basis_revision_proof_root char(64) CHECK (
    basis_revision_proof_root IS NULL OR basis_revision_proof_root ~ '^[0-9a-f]{64}$'
  ),
  claim_owner_inventory_root char(64) CHECK (
    claim_owner_inventory_root IS NULL OR claim_owner_inventory_root ~ '^[0-9a-f]{64}$'
  ),
  claim_stream_rows_scanned bigint CHECK (
    claim_stream_rows_scanned IS NULL OR claim_stream_rows_scanned > 0
  ),
  claim_event_basis_root char(64) CHECK (
    claim_event_basis_root IS NULL OR claim_event_basis_root ~ '^[0-9a-f]{64}$'
  ),
  claim_navigation_manifest_root char(64) CHECK (
    claim_navigation_manifest_root IS NULL OR claim_navigation_manifest_root ~ '^[0-9a-f]{64}$'
  ),
  predecessor_cohort_root char(64) CHECK (
    predecessor_cohort_root IS NULL OR predecessor_cohort_root ~ '^[0-9a-f]{64}$'
  ),
  dependency_root char(64) NOT NULL CHECK (dependency_root ~ '^[0-9a-f]{64}$'),
  input_root char(64) NOT NULL CHECK (input_root ~ '^[0-9a-f]{64}$'),
  output_root char(64) NOT NULL CHECK (output_root ~ '^[0-9a-f]{64}$'),
  dependency_count integer NOT NULL CHECK (dependency_count > 0),
  node_count integer NOT NULL CHECK (node_count >= 0),
  edge_count integer NOT NULL CHECK (edge_count >= 0),
  diagnostic_count integer NOT NULL CHECK (diagnostic_count >= 0),
  cohort_bytes bigint NOT NULL CHECK (cohort_bytes >= 0),
  PRIMARY KEY (domain,consumer_id,source_path,record_id),
  FOREIGN KEY (domain,consumer_id)
    REFERENCES cmd2_projection_navigation_consumer(domain,consumer_id) ON DELETE CASCADE
);
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS basis_kind text NOT NULL DEFAULT 'record-mutation';
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS coverage_manifest_sha256 char(64) CHECK (
    coverage_manifest_sha256 IS NULL OR coverage_manifest_sha256 ~ '^[0-9a-f]{64}$'
  );
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS basis_dependency_path text;
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS basis_dependency_sha256 char(64);
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS basis_dependency_causes_json text;
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS basis_dependency_causes_root char(64);
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS basis_history_path text;
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS basis_history_sha256 char(64);
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS basis_claim_id text;
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS basis_revision_proof_root char(64);
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS claim_owner_inventory_root char(64);
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS claim_stream_rows_scanned bigint;
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS claim_event_basis_root char(64);
ALTER TABLE cmd2_projection_navigation_cohort
  ADD COLUMN IF NOT EXISTS claim_navigation_manifest_root char(64);
ALTER TABLE cmd2_projection_navigation_cohort
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_cohort_basis_kind_check;
ALTER TABLE cmd2_projection_navigation_cohort
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_cohort_check;
ALTER TABLE cmd2_projection_navigation_cohort
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_cohort_basis_shape_check;
ALTER TABLE cmd2_projection_navigation_cohort
  ADD CONSTRAINT cmd2_projection_navigation_cohort_basis_kind_check CHECK (
    basis_kind IN ('record-mutation','dependency-refresh','dependency-refresh-set','claim-mutation','claim-dependency-refresh','claim-revision-refresh','range-reconciliation','claim-range-reconciliation')
  );
ALTER TABLE cmd2_projection_navigation_cohort
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_cohort_basis_history_sha256_check;
ALTER TABLE cmd2_projection_navigation_cohort
  ADD CONSTRAINT cmd2_projection_navigation_cohort_basis_history_sha256_check CHECK (
    basis_history_sha256 IS NULL OR basis_history_sha256 ~ '^[0-9a-f]{64}$'
  );
ALTER TABLE cmd2_projection_navigation_cohort
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_cohort_basis_revision_proof_root_check;
ALTER TABLE cmd2_projection_navigation_cohort
  ADD CONSTRAINT cmd2_projection_navigation_cohort_basis_revision_proof_root_check CHECK (
    basis_revision_proof_root IS NULL OR basis_revision_proof_root ~ '^[0-9a-f]{64}$'
  );
ALTER TABLE cmd2_projection_navigation_cohort
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_cohort_basis_dependency_causes_root_check;
ALTER TABLE cmd2_projection_navigation_cohort
  ADD CONSTRAINT cmd2_projection_navigation_cohort_basis_dependency_causes_root_check CHECK (
    basis_dependency_causes_root IS NULL OR basis_dependency_causes_root ~ '^[0-9a-f]{64}$'
  );
ALTER TABLE cmd2_projection_navigation_cohort
  ADD CONSTRAINT cmd2_projection_navigation_cohort_basis_shape_check CHECK (
    (basis_kind='record-mutation'
      AND basis_dependency_path IS NULL AND basis_dependency_sha256 IS NULL
      AND basis_dependency_causes_json IS NULL AND basis_dependency_causes_root IS NULL
      AND basis_history_path IS NULL AND basis_history_sha256 IS NULL
      AND basis_claim_id IS NULL AND basis_revision_proof_root IS NULL)
    OR (basis_kind='dependency-refresh'
      AND basis_dependency_path IS NOT NULL AND basis_dependency_sha256 IS NOT NULL
      AND basis_dependency_causes_json IS NULL AND basis_dependency_causes_root IS NULL
      AND basis_history_path IS NULL AND basis_history_sha256 IS NULL
      AND basis_claim_id IS NULL AND basis_revision_proof_root IS NULL)
    OR (basis_kind='dependency-refresh-set'
      AND basis_dependency_path IS NULL AND basis_dependency_sha256 IS NULL
      AND basis_dependency_causes_json IS NOT NULL
      AND basis_dependency_causes_root IS NOT NULL
      AND basis_history_path IS NULL AND basis_history_sha256 IS NULL
      AND basis_claim_id IS NULL AND basis_revision_proof_root IS NULL)
    OR (basis_kind='claim-mutation'
      AND basis_dependency_path IS NULL AND basis_dependency_sha256 IS NULL
      AND basis_dependency_causes_json IS NULL AND basis_dependency_causes_root IS NULL
      AND basis_history_path IS NULL AND basis_history_sha256 IS NULL
      AND basis_claim_id IS NULL
      AND claim_owner_inventory_root IS NOT NULL
      AND claim_stream_rows_scanned IS NOT NULL
      AND claim_event_basis_root IS NOT NULL
      AND claim_navigation_manifest_root IS NOT NULL)
    OR (basis_kind='claim-dependency-refresh'
      AND basis_dependency_path IS NULL AND basis_dependency_sha256 IS NULL
      AND basis_dependency_causes_json IS NULL AND basis_dependency_causes_root IS NULL
      AND basis_history_path IS NULL AND basis_history_sha256 IS NULL
      AND basis_claim_id IS NULL AND basis_revision_proof_root IS NULL
      AND claim_owner_inventory_root IS NOT NULL
      AND claim_stream_rows_scanned IS NOT NULL
      AND claim_event_basis_root IS NOT NULL
      AND claim_navigation_manifest_root IS NOT NULL)
    OR (basis_kind='claim-revision-refresh'
      AND basis_dependency_path IS NOT NULL AND basis_dependency_sha256 IS NOT NULL
      AND basis_dependency_causes_json IS NULL AND basis_dependency_causes_root IS NULL
      AND basis_history_path IS NOT NULL AND basis_history_sha256 IS NOT NULL
      AND basis_claim_id IS NOT NULL AND octet_length(basis_claim_id) BETWEEN 1 AND 256
      AND basis_revision_proof_root IS NOT NULL
      AND claim_owner_inventory_root IS NULL
      AND claim_stream_rows_scanned IS NULL
      AND claim_event_basis_root IS NULL
      AND claim_navigation_manifest_root IS NULL)
    OR (basis_kind='range-reconciliation'
      AND basis_dependency_path IS NULL AND basis_dependency_sha256 IS NULL
      AND basis_dependency_causes_json IS NULL AND basis_dependency_causes_root IS NULL
      AND basis_history_path IS NULL AND basis_history_sha256 IS NULL
      AND basis_claim_id IS NULL AND basis_revision_proof_root IS NULL
      AND claim_owner_inventory_root IS NULL
      AND claim_stream_rows_scanned IS NULL
      AND claim_event_basis_root IS NULL
      AND claim_navigation_manifest_root IS NULL)
    OR (basis_kind='claim-range-reconciliation'
      AND basis_dependency_path IS NULL AND basis_dependency_sha256 IS NULL
      AND basis_dependency_causes_json IS NULL AND basis_dependency_causes_root IS NULL
      AND basis_history_path IS NULL AND basis_history_sha256 IS NULL
      AND basis_claim_id IS NULL AND basis_revision_proof_root IS NULL
      AND claim_owner_inventory_root IS NOT NULL
      AND claim_stream_rows_scanned IS NOT NULL
      AND claim_event_basis_root IS NOT NULL
      AND claim_navigation_manifest_root IS NOT NULL)
    OR (basis_kind IN ('record-mutation','dependency-refresh','dependency-refresh-set')
      AND claim_owner_inventory_root IS NULL
      AND claim_stream_rows_scanned IS NULL
      AND claim_event_basis_root IS NULL
      AND claim_navigation_manifest_root IS NULL)
  );

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_dependency (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  source_path text NOT NULL,
  record_id text NOT NULL,
  dependency_path text NOT NULL,
  dependency_role text NOT NULL CHECK (dependency_role IN (
    'event','event-schema','event-receipt','record','entity-registry','entity-registry-schema','record-schema',
    'human-forms','forms-schema','record-history','version-source','version-schema','additional'
  )),
  observation_state text NOT NULL CHECK (observation_state IN ('present','absent')),
  sha256 char(64),
  size_bytes bigint,
  absent_reason text,
  PRIMARY KEY (domain,consumer_id,source_path,record_id,dependency_path),
  FOREIGN KEY (domain,consumer_id,source_path,record_id)
    REFERENCES cmd2_projection_navigation_cohort(domain,consumer_id,source_path,record_id)
    ON DELETE CASCADE,
  CONSTRAINT cmd2_projection_navigation_dependency_state_check CHECK (
    (observation_state='present' AND sha256 ~ '^[0-9a-f]{64}$' AND
      size_bytes > 0 AND absent_reason IS NULL) OR
    (observation_state='absent' AND sha256 IS NULL AND size_bytes IS NULL AND
      octet_length(absent_reason) BETWEEN 1 AND 128)
  )
);

CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_dependency_path_seek
  ON cmd2_projection_navigation_dependency(domain,consumer_id,dependency_path COLLATE "C");

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_node (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  source_path text NOT NULL,
  record_id text NOT NULL,
  node_id text NOT NULL CHECK (octet_length(node_id) BETWEEN 1 AND 256),
  visible boolean NOT NULL,
  row_sha256 char(64) NOT NULL CHECK (row_sha256 ~ '^[0-9a-f]{64}$'),
  from_id text,
  to_id text,
  row_json text NOT NULL CHECK (octet_length(row_json) <= 4194304),
  CONSTRAINT cmd2_projection_navigation_node_endpoint_check CHECK (from_id IS NULL AND to_id IS NULL),
  PRIMARY KEY (domain,consumer_id,source_path,record_id,node_id),
  FOREIGN KEY (domain,consumer_id,source_path,record_id)
    REFERENCES cmd2_projection_navigation_cohort(domain,consumer_id,source_path,record_id)
    ON DELETE CASCADE
);
ALTER TABLE cmd2_projection_navigation_node ADD COLUMN IF NOT EXISTS visible boolean;
ALTER TABLE cmd2_projection_navigation_node ADD COLUMN IF NOT EXISTS row_sha256 char(64);
ALTER TABLE cmd2_projection_navigation_node ADD COLUMN IF NOT EXISTS from_id text;
ALTER TABLE cmd2_projection_navigation_node ADD COLUMN IF NOT EXISTS to_id text;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_node_id_seek
  ON cmd2_projection_navigation_node(domain,consumer_id,node_id COLLATE "C")
  WHERE visible;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_node_id_all_seek
  ON cmd2_projection_navigation_node(domain,consumer_id,node_id COLLATE "C");

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_edge (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  source_path text NOT NULL,
  record_id text NOT NULL,
  edge_id text NOT NULL CHECK (octet_length(edge_id) BETWEEN 1 AND 512),
  from_id text NOT NULL CHECK (octet_length(from_id) BETWEEN 1 AND 256),
  to_id text NOT NULL CHECK (octet_length(to_id) BETWEEN 1 AND 256),
  visible boolean NOT NULL,
  row_sha256 char(64) NOT NULL CHECK (row_sha256 ~ '^[0-9a-f]{64}$'),
  row_json text NOT NULL CHECK (octet_length(row_json) <= 4194304),
  CONSTRAINT cmd2_projection_navigation_edge_endpoints_check CHECK (
    octet_length(from_id) BETWEEN 1 AND 256 AND octet_length(to_id) BETWEEN 1 AND 256
  ),
  PRIMARY KEY (domain,consumer_id,source_path,record_id,edge_id),
  FOREIGN KEY (domain,consumer_id,source_path,record_id)
    REFERENCES cmd2_projection_navigation_cohort(domain,consumer_id,source_path,record_id)
    ON DELETE CASCADE
);
ALTER TABLE cmd2_projection_navigation_edge ADD COLUMN IF NOT EXISTS from_id text;
ALTER TABLE cmd2_projection_navigation_edge
  DROP CONSTRAINT IF EXISTS cmd2_projection_navigation_edge_edge_id_check;
ALTER TABLE cmd2_projection_navigation_edge
  ADD CONSTRAINT cmd2_projection_navigation_edge_edge_id_check
    CHECK (octet_length(edge_id) BETWEEN 1 AND 512);
ALTER TABLE cmd2_projection_navigation_edge ADD COLUMN IF NOT EXISTS to_id text;
ALTER TABLE cmd2_projection_navigation_edge ADD COLUMN IF NOT EXISTS visible boolean;
ALTER TABLE cmd2_projection_navigation_edge ADD COLUMN IF NOT EXISTS row_sha256 char(64);
UPDATE cmd2_projection_navigation_edge
SET from_id=row_json::jsonb->>'from_id',to_id=row_json::jsonb->>'to_id'
WHERE from_id IS NULL OR to_id IS NULL;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_edge_from_seek
  ON cmd2_projection_navigation_edge(domain,consumer_id,from_id COLLATE "C",edge_id COLLATE "C")
  WHERE visible;
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_edge_from_all_seek
  ON cmd2_projection_navigation_edge(domain,consumer_id,from_id COLLATE "C",edge_id COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_edge_id_seek
  ON cmd2_projection_navigation_edge(domain,consumer_id,edge_id COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_projection_navigation_edge_to_seek
  ON cmd2_projection_navigation_edge(domain,consumer_id,to_id COLLATE "C",edge_id COLLATE "C");

CREATE TABLE IF NOT EXISTS cmd2_projection_navigation_diagnostic (
  domain text NOT NULL,
  consumer_id text NOT NULL,
  source_path text NOT NULL,
  record_id text NOT NULL,
  diagnostic_ordinal integer NOT NULL CHECK (diagnostic_ordinal >= 0),
  row_json text NOT NULL CHECK (octet_length(row_json) <= 4194304),
  PRIMARY KEY (domain,consumer_id,source_path,record_id,diagnostic_ordinal),
  FOREIGN KEY (domain,consumer_id,source_path,record_id)
    REFERENCES cmd2_projection_navigation_cohort(domain,consumer_id,source_path,record_id)
    ON DELETE CASCADE
);
