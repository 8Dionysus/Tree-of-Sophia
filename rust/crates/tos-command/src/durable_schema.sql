-- CMD.2 private shadow laboratory only. This schema has no ToS source admission.
-- All digest strings are lowercase SHA-256 hex, matching the CMD.1 lab ABI.
CREATE TABLE IF NOT EXISTS cmd2_domain (
  domain text PRIMARY KEY,
  head_seq bigint NOT NULL DEFAULT 0 CHECK (head_seq >= 0),
  rights_version bigint NOT NULL DEFAULT 0 CHECK (rights_version >= 0),
  rights_allowed boolean NOT NULL DEFAULT true,
  rule_version bigint NOT NULL DEFAULT 0 CHECK (rule_version >= 0),
  contract_digest char(64) NOT NULL,
  schema_profile_digest char(64),
  published_seq bigint NOT NULL DEFAULT 0 CHECK (published_seq >= 0),
  complete_cut_digest char(64),
  complete_cut_generation bigint CHECK (complete_cut_generation >= 0),
  selected_generation_digest char(64)
);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS schema_profile_digest char(64);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS published_seq bigint NOT NULL DEFAULT 0
  CHECK (published_seq >= 0);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS complete_cut_digest char(64);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS complete_cut_generation bigint
  CHECK (complete_cut_generation >= 0);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS selected_generation_digest char(64);
-- Initial immutable selection and maintained controlled generation are distinct.
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS source_revision char(64);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS source_membership_digest char(64);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS source_membership_count bigint CHECK (source_membership_count >= 0);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS source_epoch bigint CHECK (source_epoch > 0);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS source_generation bigint CHECK (source_generation >= 0);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS source_complete boolean NOT NULL DEFAULT false;
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS source_definition_digest char(64);

-- Every selected metadata mutation must change this independent, lockable
-- generation. Every writer and the short publisher locks it before attempt
-- and domain rows; AFTER triggers then re-enter that same transaction lock.
-- The publisher compares the private offline audit generation. Direct fence writes and
-- trigger/DDL bypass are outside this private laboratory trust profile.
CREATE TABLE IF NOT EXISTS cmd2_audit_fence (
  domain text PRIMARY KEY REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
  maintenance_state text NOT NULL DEFAULT 'normal'
    CHECK (maintenance_state IN ('normal','active'))
);
ALTER TABLE cmd2_audit_fence ADD COLUMN IF NOT EXISTS maintenance_state text
  NOT NULL DEFAULT 'normal' CHECK (maintenance_state IN ('normal','active'));

CREATE TABLE IF NOT EXISTS cmd2_job (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  job_id text NOT NULL,
  fence_epoch bigint NOT NULL CHECK (fence_epoch > 0),
  PRIMARY KEY (domain, job_id)
);

CREATE TABLE IF NOT EXISTS cmd2_predicate (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  kind text NOT NULL,
  owner text NOT NULL,
  scope text NOT NULL,
  token text NOT NULL,
  definition_version text NOT NULL,
  generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
  complete boolean NOT NULL DEFAULT false,
  PRIMARY KEY (domain, kind, owner, scope, token)
);

-- This row is the persistent arbitration point for commit, cancel and replay.
-- It remains after abort and is never inferred from a missing command receipt.
CREATE TABLE IF NOT EXISTS cmd2_attempt (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  prepare_id bytea NOT NULL CHECK (octet_length(prepare_id) BETWEEN 1 AND 65535),
  command_id text NOT NULL,
  raw_request_digest char(64) NOT NULL,
  delta_digest char(64) NOT NULL,
  state text NOT NULL CHECK (state IN ('registered', 'ready', 'committed', 'aborted')),
  attempt_fence bigint NOT NULL CHECK (attempt_fence > 0),
  commit_seq bigint CHECK (commit_seq > 0),
  receipt_digest char(64),
  PRIMARY KEY (domain, prepare_id),
  UNIQUE (domain, command_id),
  CHECK ((state = 'committed') = (commit_seq IS NOT NULL AND receipt_digest IS NOT NULL))
);
ALTER TABLE cmd2_attempt ADD COLUMN IF NOT EXISTS source_reads bytea;
ALTER TABLE cmd2_attempt ADD COLUMN IF NOT EXISTS source_indexes bytea;
ALTER TABLE cmd2_attempt ADD COLUMN IF NOT EXISTS source_epoch bigint CHECK (source_epoch > 0);

-- Owner-derived unique identities. Original bytes remain in current/history.
CREATE TABLE IF NOT EXISTS cmd2_source_index (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  kind text NOT NULL,
  token text NOT NULL,
  path text NOT NULL,
  definition_digest char(64) NOT NULL,
  PRIMARY KEY(domain,kind,token)
);
-- Exact owner-derived Agent inventory contribution; original body custody
-- stays in current/history. NULL is an unproved scope, never empty inventory.
ALTER TABLE cmd2_source_index ADD COLUMN IF NOT EXISTS inventory_projection bytea;
ALTER TABLE cmd2_attempt ADD COLUMN IF NOT EXISTS source_projections bytea;

-- A ready member has every coordinate needed for exact cold STO recovery.
-- The pin is shared across the members of one compound seal. The adapter must
-- check that invariant when it attaches rows and again under the commit lock.
CREATE TABLE IF NOT EXISTS cmd2_member (
  domain text NOT NULL,
  prepare_id bytea NOT NULL,
  member_slot integer NOT NULL CHECK (member_slot >= 0),
  profile_id text NOT NULL,
  profile_version text NOT NULL,
  subject text NOT NULL,
  expected_revision bigint CHECK (expected_revision > 0),
  expected_digest char(64),
  proposed_revision bigint NOT NULL CHECK (proposed_revision > 0),
  content_digest char(64) NOT NULL,
  content_length bigint NOT NULL CHECK (content_length >= 0),
  store_id bytea NOT NULL CHECK (octet_length(store_id) = 16),
  custody_domain_digest char(64) NOT NULL,
  custody_domain bytea NOT NULL CHECK (octet_length(custody_domain) > 0),
  pin_id bytea NOT NULL CHECK (octet_length(pin_id) = 16),
  pin_fence bigint NOT NULL CHECK (pin_fence > 0),
  segment_digest char(64) NOT NULL,
  segment_size bigint NOT NULL CHECK (segment_size > 0),
  frame_index integer NOT NULL CHECK (frame_index >= 0),
  frame_header_offset bigint NOT NULL CHECK (frame_header_offset >= 0),
  frame_digest char(64) NOT NULL,
  frame_length bigint NOT NULL CHECK (frame_length >= 0),
  sto_receipt_id char(64) NOT NULL,
  durability_class text NOT NULL,
  PRIMARY KEY (domain, prepare_id, member_slot),
  UNIQUE (domain, prepare_id, subject),
  UNIQUE (domain, prepare_id, sto_receipt_id),
  FOREIGN KEY (domain, prepare_id) REFERENCES cmd2_attempt(domain, prepare_id),
  CHECK (content_digest = frame_digest AND content_length = frame_length),
  CHECK ((expected_revision IS NULL) = (expected_digest IS NULL))
);

-- Current and history each retain the complete locator rather than a bare
-- hash or a synthetic byte reference. The immutable history is the selected
-- predecessor route, gated by current rights before any STO disclosure.
CREATE TABLE IF NOT EXISTS cmd2_current (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  subject text NOT NULL,
  revision bigint NOT NULL CHECK (revision > 0),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  prepare_id bytea NOT NULL,
  member_slot integer NOT NULL,
  profile_id text NOT NULL,
  profile_version text NOT NULL,
  content_digest char(64) NOT NULL,
  content_length bigint NOT NULL CHECK (content_length >= 0),
  store_id bytea NOT NULL CHECK (octet_length(store_id) = 16),
  custody_domain_digest char(64) NOT NULL,
  custody_domain bytea NOT NULL,
  pin_id bytea NOT NULL CHECK (octet_length(pin_id) = 16),
  pin_fence bigint NOT NULL CHECK (pin_fence > 0),
  segment_digest char(64) NOT NULL,
  segment_size bigint NOT NULL CHECK (segment_size > 0),
  frame_index integer NOT NULL CHECK (frame_index >= 0),
  frame_header_offset bigint NOT NULL CHECK (frame_header_offset >= 0),
  frame_digest char(64) NOT NULL,
  frame_length bigint NOT NULL CHECK (frame_length >= 0),
  sto_receipt_id char(64) NOT NULL,
  durability_class text NOT NULL,
  PRIMARY KEY (domain, subject),
  FOREIGN KEY (domain, prepare_id, member_slot)
    REFERENCES cmd2_member(domain, prepare_id, member_slot)
);

CREATE TABLE IF NOT EXISTS cmd2_history (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  subject text NOT NULL,
  revision bigint NOT NULL CHECK (revision > 0),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  prepare_id bytea NOT NULL,
  member_slot integer NOT NULL,
  profile_id text NOT NULL,
  profile_version text NOT NULL,
  content_digest char(64) NOT NULL,
  content_length bigint NOT NULL CHECK (content_length >= 0),
  store_id bytea NOT NULL CHECK (octet_length(store_id) = 16),
  custody_domain_digest char(64) NOT NULL,
  custody_domain bytea NOT NULL,
  pin_id bytea NOT NULL CHECK (octet_length(pin_id) = 16),
  pin_fence bigint NOT NULL CHECK (pin_fence > 0),
  segment_digest char(64) NOT NULL,
  segment_size bigint NOT NULL CHECK (segment_size > 0),
  frame_index integer NOT NULL CHECK (frame_index >= 0),
  frame_header_offset bigint NOT NULL CHECK (frame_header_offset >= 0),
  frame_digest char(64) NOT NULL,
  frame_length bigint NOT NULL CHECK (frame_length >= 0),
  sto_receipt_id char(64) NOT NULL,
  durability_class text NOT NULL,
  PRIMARY KEY (domain, subject, revision),
  UNIQUE (domain, prepare_id, member_slot),
  FOREIGN KEY (domain, prepare_id, member_slot)
    REFERENCES cmd2_member(domain, prepare_id, member_slot)
);

CREATE TABLE IF NOT EXISTS cmd2_receipt (
  domain text NOT NULL,
  command_id text NOT NULL,
  prepare_id bytea NOT NULL,
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  raw_request_digest char(64) NOT NULL,
  delta_digest char(64) NOT NULL,
  receipt_digest char(64) NOT NULL,
  members_root char(64) NOT NULL,
  PRIMARY KEY (domain, command_id),
  UNIQUE (domain, commit_seq),
  FOREIGN KEY (domain, prepare_id) REFERENCES cmd2_attempt(domain, prepare_id)
);

CREATE TABLE IF NOT EXISTS cmd2_log (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  event_kind text NOT NULL CHECK (event_kind IN ('command', 'rights', 'rule')),
  command_id text NOT NULL,
  delta_digest char(64) NOT NULL,
  members_root char(64) NOT NULL,
  PRIMARY KEY (domain, commit_seq),
  UNIQUE (domain, event_kind, command_id)
);

CREATE TABLE IF NOT EXISTS cmd2_outbox (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  event_id text NOT NULL,
  PRIMARY KEY (domain, event_id),
  UNIQUE (domain, commit_seq)
);

-- Exact source-carrier metadata stays alongside its existing byte locator.
ALTER TABLE cmd2_member ADD COLUMN IF NOT EXISTS source_mode integer CHECK(source_mode >= 0 AND source_mode <= 4095);
ALTER TABLE cmd2_member ADD COLUMN IF NOT EXISTS source_dependencies text[];
ALTER TABLE cmd2_current ADD COLUMN IF NOT EXISTS source_mode integer CHECK(source_mode >= 0 AND source_mode <= 4095);
ALTER TABLE cmd2_current ADD COLUMN IF NOT EXISTS source_dependencies text[];
ALTER TABLE cmd2_history ADD COLUMN IF NOT EXISTS source_mode integer CHECK(source_mode >= 0 AND source_mode <= 4095);
ALTER TABLE cmd2_history ADD COLUMN IF NOT EXISTS source_dependencies text[];
ALTER TABLE cmd2_current ADD COLUMN IF NOT EXISTS inventory_projection bytea;
ALTER TABLE cmd2_history ADD COLUMN IF NOT EXISTS inventory_projection bytea;
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS source_projection_digest char(64);

CREATE OR REPLACE FUNCTION cmd2_register_audit_domain() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO cmd2_audit_fence(domain,generation) VALUES(NEW.domain,0);
  RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION cmd2_bump_audit_fence() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE changed_domain text;
BEGIN
  IF TG_OP = 'UPDATE' THEN
    IF NEW.domain IS DISTINCT FROM OLD.domain THEN
      RAISE EXCEPTION 'CMD2 domain identity cannot move';
    END IF;
  END IF;
  IF TG_OP = 'DELETE' THEN
    changed_domain := OLD.domain;
  ELSE
    changed_domain := NEW.domain;
  END IF;
  UPDATE cmd2_audit_fence SET generation=generation+1
    WHERE domain=changed_domain AND generation < 9223372036854775807;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'CMD2 audit fence absent or exhausted for %', changed_domain;
  END IF;
  RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION cmd2_refuse_audited_truncate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'CMD2 audited table cannot be truncated';
END $$;

-- Entering or leaving maintenance invalidates every earlier certificate.
-- A future physical-maintenance route must keep 'active' for the entire
-- operation and authorize normal only after independent verified completion.
CREATE OR REPLACE FUNCTION cmd2_fence_maintenance_transition() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.domain IS DISTINCT FROM OLD.domain THEN
    RAISE EXCEPTION 'CMD2 audit fence domain cannot move';
  END IF;
  IF NEW.maintenance_state IS DISTINCT FROM OLD.maintenance_state THEN
    IF NEW.generation IS DISTINCT FROM OLD.generation
       OR OLD.generation >= 9223372036854775807 THEN
      RAISE EXCEPTION 'CMD2 maintenance transition generation invalid';
    END IF;
    NEW.generation := OLD.generation + 1;
  END IF;
  RETURN NEW;
END $$;

CREATE OR REPLACE TRIGGER cmd2_audit_domain_insert
  AFTER INSERT ON cmd2_domain FOR EACH ROW
  EXECUTE FUNCTION cmd2_register_audit_domain();
CREATE OR REPLACE TRIGGER cmd2_audit_domain_update
  AFTER UPDATE ON cmd2_domain FOR EACH ROW
  EXECUTE FUNCTION cmd2_bump_audit_fence();
CREATE OR REPLACE TRIGGER cmd2_refuse_domain_truncate
  BEFORE TRUNCATE ON cmd2_domain FOR EACH STATEMENT
  EXECUTE FUNCTION cmd2_refuse_audited_truncate();
CREATE OR REPLACE TRIGGER cmd2_fence_maintenance_transition
  BEFORE UPDATE ON cmd2_audit_fence FOR EACH ROW
  EXECUTE FUNCTION cmd2_fence_maintenance_transition();

CREATE OR REPLACE FUNCTION cmd2_invalidate_source_maintenance() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.maintenance_state IS DISTINCT FROM OLD.maintenance_state THEN
    UPDATE cmd2_domain SET source_complete=false,source_epoch=source_epoch+1,
      source_projection_digest=NULL,selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL
      WHERE domain=NEW.domain AND source_revision IS NOT NULL;
    UPDATE cmd2_predicate SET complete=false WHERE domain=NEW.domain;
  END IF;
  RETURN NULL;
END $$;
CREATE OR REPLACE TRIGGER cmd2_invalidate_source_maintenance
  AFTER UPDATE ON cmd2_audit_fence FOR EACH ROW
  EXECUTE FUNCTION cmd2_invalidate_source_maintenance();

DO $$
DECLARE table_name text;
BEGIN
  FOREACH table_name IN ARRAY ARRAY[
    'cmd2_job','cmd2_predicate','cmd2_attempt','cmd2_member',
    'cmd2_current','cmd2_history','cmd2_receipt','cmd2_log','cmd2_outbox','cmd2_source_index'
  ] LOOP
    EXECUTE format(
      'CREATE OR REPLACE TRIGGER %I AFTER INSERT OR UPDATE OR DELETE ON %I '
      || 'FOR EACH ROW EXECUTE FUNCTION cmd2_bump_audit_fence()',
      'cmd2_audit_' || table_name, table_name);
    EXECUTE format(
      'CREATE OR REPLACE TRIGGER %I BEFORE TRUNCATE ON %I '
      || 'FOR EACH STATEMENT EXECUTE FUNCTION cmd2_refuse_audited_truncate()',
      'cmd2_refuse_' || table_name || '_truncate', table_name);
  END LOOP;
END $$;
