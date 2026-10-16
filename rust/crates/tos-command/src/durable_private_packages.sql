-- Owner-private flat packages persisted separately from public CMD.2 source
-- rows and projections. Outbox and log rows carry hashes/counts only; package
-- bytes and logical references stay in these private, owner-tagged relations.
CREATE TABLE IF NOT EXISTS cmd2_private_owner_scope (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  owner_scope text NOT NULL CHECK (owner_scope ~ '^[0-9a-f]{64}$'),
  context_binding text NOT NULL CHECK (context_binding ~ '^[0-9a-f]{64}$'),
  next_commit_seq bigint NOT NULL DEFAULT 0 CHECK (next_commit_seq >= 0),
  PRIMARY KEY (domain, owner_scope)
);
CREATE TABLE IF NOT EXISTS cmd2_private_schema_profile (
  singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
  profile_id text NOT NULL
);
INSERT INTO cmd2_private_schema_profile(singleton,profile_id)
VALUES (true,'tos-private-owner-package-abi-v4')
ON CONFLICT (singleton) DO NOTHING;
-- v1/v2 drafts lack the scoped Text identity index. They may be upgraded only
-- when there is no Text current row whose identity contribution set is absent.
DROP TRIGGER IF EXISTS cmd2_private_schema_profile_immutable ON cmd2_private_schema_profile;
UPDATE cmd2_private_schema_profile
   SET profile_id='tos-private-owner-package-abi-v3'
 WHERE singleton AND profile_id IN ('tos-private-owner-package-abi-v1',
                                    'tos-private-owner-package-abi-v2');

CREATE TABLE IF NOT EXISTS cmd2_private_owner_audit_fence (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
  PRIMARY KEY (domain, owner_scope),
  FOREIGN KEY (domain, owner_scope)
    REFERENCES cmd2_private_owner_scope(domain, owner_scope) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS cmd2_private_package_history (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  target_ref text NOT NULL CHECK (octet_length(target_ref) BETWEEN 1 AND 4096),
  revision bigint NOT NULL CHECK (revision > 0),
  family text NOT NULL CONSTRAINT cmd2_private_package_history_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment')),
  package_digest text NOT NULL CHECK (package_digest ~ '^[0-9a-f]{64}$'),
  previous_digest text CHECK (previous_digest IS NULL OR previous_digest ~ '^[0-9a-f]{64}$'),
  member_count integer NOT NULL CHECK (member_count BETWEEN 1 AND 64),
  total_bytes bigint NOT NULL CHECK (total_bytes BETWEEN 1 AND 16777216),
  command_id text NOT NULL CHECK (octet_length(command_id) BETWEEN 1 AND 1024),
  request_digest text NOT NULL CHECK (request_digest ~ '^[0-9a-f]{64}$'),
  raw_request_digest text NOT NULL CHECK (raw_request_digest ~ '^[0-9a-f]{64}$'),
  owner_context_binding text NOT NULL CHECK (owner_context_binding ~ '^[0-9a-f]{64}$'),
  owner_configuration_digest text NOT NULL CHECK (owner_configuration_digest ~ '^[0-9a-f]{64}$'),
  source_head_seq bigint NOT NULL CHECK (source_head_seq >= 0),
  source_epoch bigint NOT NULL CHECK (source_epoch > 0),
  source_generation bigint NOT NULL CHECK (source_generation >= 0),
  source_revision text NOT NULL CHECK (source_revision ~ '^[0-9a-f]{64}$'),
  source_audit_generation bigint NOT NULL CHECK (source_audit_generation >= 0),
  private_owner_audit_generation bigint NOT NULL CHECK (private_owner_audit_generation >= 0),
  rights_version bigint NOT NULL CHECK (rights_version >= 0),
  rule_version bigint NOT NULL CHECK (rule_version >= 0),
  contract_digest text NOT NULL CHECK (contract_digest ~ '^[0-9a-f]{64}$'),
  job_id text NOT NULL CHECK (octet_length(job_id) BETWEEN 1 AND 256),
  job_fence bigint NOT NULL CHECK (job_fence > 0),
  readset_digest text NOT NULL CHECK (readset_digest ~ '^[0-9a-f]{64}$'),
  dependency_digest text NOT NULL CHECK (dependency_digest ~ '^[0-9a-f]{64}$'),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  PRIMARY KEY (domain, owner_scope, target_ref, revision),
  UNIQUE (domain, owner_scope, command_id),
  UNIQUE (domain, owner_scope, target_ref, revision, family, package_digest, member_count, total_bytes),
  FOREIGN KEY (domain, owner_scope)
    REFERENCES cmd2_private_owner_scope(domain, owner_scope) ON DELETE RESTRICT,
  CHECK ((revision = 1) = (previous_digest IS NULL)),
  CONSTRAINT cmd2_private_package_history_assessment_shape_check
    CHECK (family <> 'assessment' OR (member_count = 2 AND total_bytes <= 2097152)),
  CONSTRAINT cmd2_private_package_history_alignment_size_check
    CHECK (family <> 'alignment' OR total_bytes <= 12582912)
);
CREATE TABLE IF NOT EXISTS cmd2_private_package_member (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  target_ref text NOT NULL,
  revision bigint NOT NULL CHECK (revision > 0),
  member_name text NOT NULL CHECK (octet_length(member_name) BETWEEN 1 AND 128),
  member_bytes bytea NOT NULL CHECK (octet_length(member_bytes) <= 12582912),
  member_digest text NOT NULL CHECK (member_digest ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (domain, owner_scope, target_ref, revision, member_name),
  FOREIGN KEY (domain, owner_scope, target_ref, revision)
    REFERENCES cmd2_private_package_history(domain, owner_scope, target_ref, revision)
    ON DELETE RESTRICT
);
CREATE UNIQUE INDEX IF NOT EXISTS cmd2_private_package_member_digest_key
  ON cmd2_private_package_member(domain,owner_scope,target_ref,revision,member_name,member_digest);

CREATE TABLE IF NOT EXISTS cmd2_private_package_current (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  target_ref text NOT NULL CHECK (octet_length(target_ref) BETWEEN 1 AND 4096),
  family text NOT NULL CONSTRAINT cmd2_private_package_current_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment')),
  revision bigint NOT NULL CHECK (revision > 0),
  package_digest text NOT NULL CHECK (package_digest ~ '^[0-9a-f]{64}$'),
  member_count integer NOT NULL CHECK (member_count BETWEEN 1 AND 65),
  total_bytes bigint NOT NULL CHECK (total_bytes BETWEEN 1 AND 16777216),
  text_identity_root text,
  text_identity_count integer,
  command_id text NOT NULL CHECK (octet_length(command_id) BETWEEN 1 AND 1024),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  PRIMARY KEY (domain, owner_scope, target_ref),
  FOREIGN KEY (domain, owner_scope)
    REFERENCES cmd2_private_owner_scope(domain, owner_scope) ON DELETE RESTRICT,
  FOREIGN KEY (domain, owner_scope, target_ref, revision, family, package_digest, member_count, total_bytes)
    REFERENCES cmd2_private_package_history(
      domain, owner_scope, target_ref, revision, family, package_digest, member_count, total_bytes)
    DEFERRABLE INITIALLY DEFERRED,
  CONSTRAINT cmd2_private_package_current_assessment_shape_check
    CHECK (family <> 'assessment' OR (member_count = 2 AND total_bytes <= 2097152)),
  CONSTRAINT cmd2_private_package_current_alignment_size_check
    CHECK (family <> 'alignment' OR total_bytes <= 12582912),
  CONSTRAINT cmd2_private_package_current_text_identity_root_check
    CHECK (text_identity_root IS NULL OR text_identity_root ~ '^[0-9a-f]{64}$'),
  CONSTRAINT cmd2_private_package_current_text_identity_count_check
    CHECK (text_identity_count BETWEEN 0 AND 32768),
  CONSTRAINT cmd2_private_package_current_text_identity_shape_check
    CHECK ((family IN ('text', 'alignment')) = (text_identity_root IS NOT NULL AND text_identity_count IS NOT NULL))
);
ALTER TABLE cmd2_private_package_current
  ADD COLUMN IF NOT EXISTS text_identity_root text,
  ADD COLUMN IF NOT EXISTS text_identity_count integer;
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM cmd2_private_package_current
     WHERE family IN ('text','alignment') AND (text_identity_root IS NULL OR text_identity_count IS NULL)
  ) THEN
    RAISE EXCEPTION 'private Text current package requires identity index rebuild';
  END IF;
END $$;
ALTER TABLE cmd2_private_package_current
  DROP CONSTRAINT IF EXISTS cmd2_private_package_current_text_identity_root_check,
  DROP CONSTRAINT IF EXISTS cmd2_private_package_current_text_identity_count_check,
  DROP CONSTRAINT IF EXISTS cmd2_private_package_current_text_identity_shape_check;
ALTER TABLE cmd2_private_package_current
  ADD CONSTRAINT cmd2_private_package_current_text_identity_root_check
    CHECK (text_identity_root IS NULL OR text_identity_root ~ '^[0-9a-f]{64}$'),
  ADD CONSTRAINT cmd2_private_package_current_text_identity_count_check
    CHECK (text_identity_count BETWEEN 0 AND 32768),
  ADD CONSTRAINT cmd2_private_package_current_text_identity_shape_check
    CHECK ((family IN ('text', 'alignment')) = (text_identity_root IS NOT NULL AND text_identity_count IS NOT NULL));
CREATE UNIQUE INDEX IF NOT EXISTS cmd2_private_package_current_text_identity_key
  ON cmd2_private_package_current(
    domain,owner_scope,target_ref,family,revision,package_digest,text_identity_root
  );
CREATE TABLE IF NOT EXISTS cmd2_private_text_identity (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  target_ref text COLLATE "C" NOT NULL CHECK (octet_length(target_ref) BETWEEN 1 AND 4096),
  family text COLLATE "C" NOT NULL CHECK (family IN ('text','alignment')),
  revision bigint NOT NULL CHECK (revision > 0),
  package_digest text NOT NULL CHECK (package_digest ~ '^[0-9a-f]{64}$'),
  identity_root text NOT NULL CHECK (identity_root ~ '^[0-9a-f]{64}$'),
  member_name text COLLATE "C" NOT NULL CHECK (octet_length(member_name) BETWEEN 1 AND 128),
  member_digest text NOT NULL CHECK (member_digest ~ '^[0-9a-f]{64}$'),
  identity_token text COLLATE "C" NOT NULL CHECK (octet_length(identity_token) BETWEEN 1 AND 4096),
  PRIMARY KEY (domain,owner_scope,target_ref,revision,member_name,identity_token),
  FOREIGN KEY (domain,owner_scope,target_ref,family,revision,package_digest,identity_root)
    REFERENCES cmd2_private_package_current(
      domain,owner_scope,target_ref,family,revision,package_digest,text_identity_root
    ) DEFERRABLE INITIALLY DEFERRED,
  FOREIGN KEY (domain,owner_scope,target_ref,revision,member_name,member_digest)
    REFERENCES cmd2_private_package_member(
      domain,owner_scope,target_ref,revision,member_name,member_digest
    ) ON DELETE RESTRICT
);
ALTER TABLE cmd2_private_text_identity
  DROP CONSTRAINT IF EXISTS cmd2_private_text_identity_family_check;
ALTER TABLE cmd2_private_text_identity
  ADD CONSTRAINT cmd2_private_text_identity_family_check
    CHECK (family IN ('text','alignment'));
CREATE INDEX IF NOT EXISTS cmd2_private_text_identity_point
  ON cmd2_private_text_identity(
    domain,owner_scope,identity_token COLLATE "C",target_ref COLLATE "C",member_name COLLATE "C"
  ) INCLUDE (revision,package_digest,identity_root,member_digest);
CREATE INDEX IF NOT EXISTS cmd2_private_package_member_name_seek
  ON cmd2_private_package_member(domain,owner_scope,member_name,target_ref)
  INCLUDE (revision);

CREATE TABLE IF NOT EXISTS cmd2_private_package_archive (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  target_ref text NOT NULL,
  archive_ref text NOT NULL CHECK (octet_length(archive_ref) BETWEEN 1 AND 4096),
  family text NOT NULL CONSTRAINT cmd2_private_package_archive_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim')),
  package_digest text NOT NULL CHECK (package_digest ~ '^[0-9a-f]{64}$'),
  member_count integer NOT NULL CHECK (member_count BETWEEN 1 AND 65),
  total_bytes bigint NOT NULL CHECK (total_bytes BETWEEN 1 AND 16777216),
  source_revision text NOT NULL CHECK (source_revision ~ '^[0-9a-f]{64}$'),
  command_id text NOT NULL CHECK (octet_length(command_id) BETWEEN 1 AND 1024),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  PRIMARY KEY (domain, owner_scope, target_ref, archive_ref),
  FOREIGN KEY (domain, owner_scope)
    REFERENCES cmd2_private_owner_scope(domain, owner_scope) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS cmd2_private_package_archive_member (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  target_ref text NOT NULL,
  archive_ref text NOT NULL,
  member_name text NOT NULL CHECK (octet_length(member_name) BETWEEN 1 AND 128),
  member_bytes bytea NOT NULL CHECK (octet_length(member_bytes) <= 12582912),
  member_digest text NOT NULL CHECK (member_digest ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (domain, owner_scope, target_ref, archive_ref, member_name),
  FOREIGN KEY (domain, owner_scope, target_ref, archive_ref)
    REFERENCES cmd2_private_package_archive(domain, owner_scope, target_ref, archive_ref)
    ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS cmd2_private_command_receipt (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  command_id text NOT NULL CHECK (octet_length(command_id) BETWEEN 1 AND 1024),
  target_digest text NOT NULL CHECK (target_digest ~ '^[0-9a-f]{64}$'),
  family text NOT NULL CONSTRAINT cmd2_private_command_receipt_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment')),
  request_digest text NOT NULL CHECK (request_digest ~ '^[0-9a-f]{64}$'),
  raw_request_digest text NOT NULL CHECK (raw_request_digest ~ '^[0-9a-f]{64}$'),
  owner_context_binding text NOT NULL CHECK (owner_context_binding ~ '^[0-9a-f]{64}$'),
  owner_configuration_digest text NOT NULL CHECK (owner_configuration_digest ~ '^[0-9a-f]{64}$'),
  package_digest text NOT NULL CHECK (package_digest ~ '^[0-9a-f]{64}$'),
  response_bytes bytea NOT NULL CHECK (octet_length(response_bytes) <= 1048576),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  private_audit_generation_after bigint NOT NULL CHECK (private_audit_generation_after >= 0),
  PRIMARY KEY (domain, owner_scope, command_id),
  FOREIGN KEY (domain, owner_scope)
    REFERENCES cmd2_private_owner_scope(domain, owner_scope) ON DELETE RESTRICT
);

-- Private operational history and outbox retain no path, member name, or bytes.
CREATE TABLE IF NOT EXISTS cmd2_private_log (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  family text NOT NULL CONSTRAINT cmd2_private_log_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment')),
  command_digest text NOT NULL CHECK (command_digest ~ '^[0-9a-f]{64}$'),
  target_digest text NOT NULL CHECK (target_digest ~ '^[0-9a-f]{64}$'),
  request_digest text NOT NULL CHECK (request_digest ~ '^[0-9a-f]{64}$'),
  package_digest text NOT NULL CHECK (package_digest ~ '^[0-9a-f]{64}$'),
  member_count integer NOT NULL CHECK (member_count BETWEEN 0 AND 65),
  PRIMARY KEY (domain, owner_scope, commit_seq),
  FOREIGN KEY (domain, owner_scope)
    REFERENCES cmd2_private_owner_scope(domain, owner_scope) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS cmd2_private_outbox (
  domain text NOT NULL,
  owner_scope text NOT NULL,
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  family text NOT NULL CONSTRAINT cmd2_private_outbox_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment')),
  target_digest text NOT NULL CHECK (target_digest ~ '^[0-9a-f]{64}$'),
  package_digest text NOT NULL CHECK (package_digest ~ '^[0-9a-f]{64}$'),
  member_count integer NOT NULL CHECK (member_count BETWEEN 0 AND 65),
  PRIMARY KEY (domain, owner_scope, commit_seq),
  FOREIGN KEY (domain, owner_scope)
    REFERENCES cmd2_private_owner_scope(domain, owner_scope) ON DELETE RESTRICT
);

-- Replace the prior family checks and install exact Assessment and Alignment
-- package bounds. Reapplying these constraints makes profile migrations
-- explicit instead of accepting a newer marker over narrower DDL.
ALTER TABLE cmd2_private_package_history
  DROP CONSTRAINT IF EXISTS cmd2_private_package_history_family_check,
  DROP CONSTRAINT IF EXISTS cmd2_private_package_history_assessment_shape_check,
  DROP CONSTRAINT IF EXISTS cmd2_private_package_history_alignment_size_check;
ALTER TABLE cmd2_private_package_history
  ADD CONSTRAINT cmd2_private_package_history_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment')),
  ADD CONSTRAINT cmd2_private_package_history_assessment_shape_check
    CHECK (family <> 'assessment' OR (member_count = 2 AND total_bytes <= 2097152)),
  ADD CONSTRAINT cmd2_private_package_history_alignment_size_check
    CHECK (family <> 'alignment' OR total_bytes <= 12582912);
ALTER TABLE cmd2_private_package_current
  DROP CONSTRAINT IF EXISTS cmd2_private_package_current_family_check,
  DROP CONSTRAINT IF EXISTS cmd2_private_package_current_assessment_shape_check,
  DROP CONSTRAINT IF EXISTS cmd2_private_package_current_alignment_size_check;
ALTER TABLE cmd2_private_package_current
  ADD CONSTRAINT cmd2_private_package_current_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment')),
  ADD CONSTRAINT cmd2_private_package_current_assessment_shape_check
    CHECK (family <> 'assessment' OR (member_count = 2 AND total_bytes <= 2097152)),
  ADD CONSTRAINT cmd2_private_package_current_alignment_size_check
    CHECK (family <> 'alignment' OR total_bytes <= 12582912);
ALTER TABLE cmd2_private_package_archive
  DROP CONSTRAINT IF EXISTS cmd2_private_package_archive_family_check;
ALTER TABLE cmd2_private_package_archive
  ADD CONSTRAINT cmd2_private_package_archive_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim'));
ALTER TABLE cmd2_private_command_receipt
  DROP CONSTRAINT IF EXISTS cmd2_private_command_receipt_family_check;
ALTER TABLE cmd2_private_command_receipt
  ADD CONSTRAINT cmd2_private_command_receipt_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment'));
ALTER TABLE cmd2_private_log
  DROP CONSTRAINT IF EXISTS cmd2_private_log_family_check;
ALTER TABLE cmd2_private_log
  ADD CONSTRAINT cmd2_private_log_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment'));
ALTER TABLE cmd2_private_outbox
  DROP CONSTRAINT IF EXISTS cmd2_private_outbox_family_check;
ALTER TABLE cmd2_private_outbox
  ADD CONSTRAINT cmd2_private_outbox_family_check
    CHECK (family IN ('text', 'alignment', 'profile', 'claim', 'assessment'));

-- Alignment identity rows use the same exact owner parser/index, but only
-- native translation-alignment JSON members contribute. Install the new
-- family tag only after all existing Text identity markers are checked.
UPDATE cmd2_private_schema_profile
   SET profile_id='tos-private-owner-package-abi-v4'
 WHERE singleton AND profile_id='tos-private-owner-package-abi-v3';

CREATE OR REPLACE FUNCTION cmd2_private_bump_audit_fence() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  changed_domain text;
  changed_scope text;
BEGIN
  IF TG_OP = 'DELETE' THEN
    changed_domain := OLD.domain;
    changed_scope := OLD.owner_scope;
  ELSE
    changed_domain := NEW.domain;
    changed_scope := NEW.owner_scope;
  END IF;
  UPDATE cmd2_private_owner_audit_fence
     SET generation = generation + 1
   WHERE domain = changed_domain AND owner_scope = changed_scope
     AND generation < 9223372036854775807;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'private owner audit fence absent or exhausted';
  END IF;
  RETURN NULL;
END $$;
CREATE OR REPLACE FUNCTION cmd2_private_refuse_immutable_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'private immutable package history cannot be updated or deleted';
END $$;
CREATE OR REPLACE FUNCTION cmd2_private_refuse_truncate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'private owner package table cannot be truncated';
END $$;
CREATE OR REPLACE FUNCTION cmd2_private_guard_scope_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.domain IS DISTINCT FROM OLD.domain
     OR NEW.owner_scope IS DISTINCT FROM OLD.owner_scope
     OR NEW.context_binding IS DISTINCT FROM OLD.context_binding
     OR NEW.next_commit_seq < OLD.next_commit_seq
     OR NEW.next_commit_seq > OLD.next_commit_seq + 1 THEN
    RAISE EXCEPTION 'private owner scope binding/sequence changed invalidly';
  END IF;
  RETURN NEW;
END $$;
CREATE OR REPLACE FUNCTION cmd2_private_guard_audit_update() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.domain IS DISTINCT FROM OLD.domain
     OR NEW.owner_scope IS DISTINCT FROM OLD.owner_scope
     OR OLD.generation >= 9223372036854775807
     OR NEW.generation <> OLD.generation + 1 THEN
    RAISE EXCEPTION 'private owner audit fence update invalid';
  END IF;
  RETURN NEW;
END $$;

CREATE OR REPLACE TRIGGER cmd2_private_scope_binding_update
  BEFORE UPDATE ON cmd2_private_owner_scope FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_guard_scope_update();
CREATE OR REPLACE TRIGGER cmd2_private_scope_binding_delete
  BEFORE DELETE ON cmd2_private_owner_scope FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();
CREATE OR REPLACE TRIGGER cmd2_private_audit_fence_update
  BEFORE UPDATE ON cmd2_private_owner_audit_fence FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_guard_audit_update();
CREATE OR REPLACE TRIGGER cmd2_private_audit_fence_immutable
  BEFORE DELETE ON cmd2_private_owner_audit_fence FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();

DO $$
DECLARE table_name text;
BEGIN
  FOREACH table_name IN ARRAY ARRAY[
    'cmd2_private_package_current', 'cmd2_private_package_history',
    'cmd2_private_package_member', 'cmd2_private_package_archive',
    'cmd2_private_package_archive_member', 'cmd2_private_command_receipt',
    'cmd2_private_log', 'cmd2_private_outbox', 'cmd2_private_text_identity'
  ] LOOP
    EXECUTE format(
      'CREATE OR REPLACE TRIGGER %I AFTER INSERT OR UPDATE OR DELETE ON %I '
      || 'FOR EACH ROW EXECUTE FUNCTION cmd2_private_bump_audit_fence()',
      'cmd2_private_audit_' || table_name, table_name);
    EXECUTE format(
      'CREATE OR REPLACE TRIGGER %I BEFORE TRUNCATE ON %I '
      || 'FOR EACH STATEMENT EXECUTE FUNCTION cmd2_private_refuse_truncate()',
      'cmd2_private_refuse_' || table_name || '_truncate', table_name);
  END LOOP;
END $$;

REVOKE ALL ON cmd2_private_owner_scope, cmd2_private_schema_profile,
  cmd2_private_owner_audit_fence, cmd2_private_package_current,
  cmd2_private_package_history, cmd2_private_package_member,
  cmd2_private_package_archive, cmd2_private_package_archive_member,
  cmd2_private_command_receipt, cmd2_private_log, cmd2_private_outbox,
  cmd2_private_text_identity FROM PUBLIC;

CREATE OR REPLACE TRIGGER cmd2_private_schema_profile_immutable
  BEFORE UPDATE OR DELETE ON cmd2_private_schema_profile FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();
CREATE OR REPLACE TRIGGER cmd2_private_schema_profile_truncate
  BEFORE TRUNCATE ON cmd2_private_schema_profile FOR EACH STATEMENT
  EXECUTE FUNCTION cmd2_private_refuse_truncate();

CREATE OR REPLACE TRIGGER cmd2_private_history_immutable
  BEFORE UPDATE OR DELETE ON cmd2_private_package_history FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();
CREATE OR REPLACE TRIGGER cmd2_private_member_immutable
  BEFORE UPDATE OR DELETE ON cmd2_private_package_member FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();
CREATE OR REPLACE TRIGGER cmd2_private_archive_immutable
  BEFORE UPDATE OR DELETE ON cmd2_private_package_archive FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();
CREATE OR REPLACE TRIGGER cmd2_private_archive_member_immutable
  BEFORE UPDATE OR DELETE ON cmd2_private_package_archive_member FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();
CREATE OR REPLACE TRIGGER cmd2_private_receipt_immutable
  BEFORE UPDATE OR DELETE ON cmd2_private_command_receipt FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();
CREATE OR REPLACE TRIGGER cmd2_private_log_immutable
  BEFORE UPDATE OR DELETE ON cmd2_private_log FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();
CREATE OR REPLACE TRIGGER cmd2_private_outbox_immutable
  BEFORE UPDATE OR DELETE ON cmd2_private_outbox FOR EACH ROW
  EXECUTE FUNCTION cmd2_private_refuse_immutable_mutation();

DO $$
DECLARE table_name text;
BEGIN
  FOREACH table_name IN ARRAY ARRAY[
    'cmd2_private_owner_scope', 'cmd2_private_owner_audit_fence',
    'cmd2_private_package_current', 'cmd2_private_package_history',
    'cmd2_private_package_member', 'cmd2_private_package_archive',
    'cmd2_private_package_archive_member', 'cmd2_private_command_receipt',
    'cmd2_private_log', 'cmd2_private_outbox', 'cmd2_private_text_identity'
  ] LOOP
    EXECUTE format(
      'CREATE OR REPLACE TRIGGER %I BEFORE TRUNCATE ON %I '
      || 'FOR EACH STATEMENT EXECUTE FUNCTION cmd2_private_refuse_truncate()',
      'cmd2_private_refuse_' || table_name || '_truncate', table_name);
  END LOOP;
END $$;
