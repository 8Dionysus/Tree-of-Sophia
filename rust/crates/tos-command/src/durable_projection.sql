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
