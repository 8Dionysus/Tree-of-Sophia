-- Explicit opt-in only. This schema is not part of durable_schema.sql or the
-- V1 schema profile. It replaces the existing ten generic audit triggers
-- with a per-domain opt-in journal. Inactive domains retain the original V1
-- fence behavior. Active domain updates always consume a generation; selector
-- only updates journal an equal semantic-header commitment and their selector
-- fields are read separately.

CREATE TABLE IF NOT EXISTS cmd2_audit_delta_v1 (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  generation bigint NOT NULL CHECK (generation > 0),
  table_id smallint NOT NULL CHECK (table_id BETWEEN 1 AND 11),
  operation text NOT NULL CHECK (operation IN ('I','U','D')),
  old_key bytea,
  new_key bytea,
  old_commitment bytea,
  new_commitment bytea,
  PRIMARY KEY (domain,generation),
  CHECK (
    (operation='I' AND old_key IS NULL AND old_commitment IS NULL
                   AND new_key IS NOT NULL AND octet_length(new_key)>0
                   AND new_commitment IS NOT NULL AND octet_length(new_commitment)=32)
    OR
    (operation='U' AND old_key IS NOT NULL AND octet_length(old_key)>0
                   AND new_key IS NOT NULL AND octet_length(new_key)>0
                   AND old_commitment IS NOT NULL AND octet_length(old_commitment)=32
                   AND new_commitment IS NOT NULL AND octet_length(new_commitment)=32)
    OR
    (operation='D' AND old_key IS NOT NULL AND octet_length(old_key)>0
                   AND old_commitment IS NOT NULL AND octet_length(old_commitment)=32
                   AND new_key IS NULL AND new_commitment IS NULL)
  )
);

-- A domain is journaled only after an explicit cold-baseline activation.
-- Other domains keep the V1 fence behavior and produce no journal or WAL.
CREATE TABLE IF NOT EXISTS cmd2_audit_delta_v1_domain (
  domain text PRIMARY KEY REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  baseline_generation bigint NOT NULL CHECK (baseline_generation >= 0),
  profile_digest char(64) NOT NULL CHECK (profile_digest ~ '^[0-9a-f]{64}$')
);

-- IDs 1..10 exactly follow METADATA_TABLES; 11 is the semantic domain header.
-- The key is the UTF-8 JSON array of the table's exact stable primary-key
-- fields. Bytea fields retain PostgreSQL's row_to_json `\\x...` spelling.
CREATE OR REPLACE FUNCTION cmd2_audit_delta_v1_row_key(
  p_table_id smallint, p_row json
) RETURNS bytea
LANGUAGE plpgsql STABLE STRICT AS $$
DECLARE key_text text;
BEGIN
  CASE p_table_id
    WHEN 1 THEN key_text := json_build_array(p_row->'job_id')::text;
    WHEN 2 THEN key_text := json_build_array(p_row->'kind',p_row->'owner',p_row->'scope',p_row->'token')::text;
    WHEN 3 THEN key_text := json_build_array(p_row->'prepare_id')::text;
    WHEN 4 THEN key_text := json_build_array(p_row->'prepare_id',p_row->'member_slot')::text;
    WHEN 5 THEN key_text := json_build_array(p_row->'subject')::text;
    WHEN 6 THEN key_text := json_build_array(p_row->'subject',p_row->'revision')::text;
    WHEN 7 THEN key_text := json_build_array(p_row->'command_id')::text;
    WHEN 8 THEN key_text := json_build_array(p_row->'commit_seq')::text;
    WHEN 9 THEN key_text := json_build_array(p_row->'commit_seq')::text;
    WHEN 10 THEN key_text := json_build_array(p_row->'kind',p_row->'token',p_row->'path')::text;
    WHEN 11 THEN key_text := json_build_array(p_row->'domain')::text;
    ELSE RAISE EXCEPTION 'CMD2 audit delta unknown table id %',p_table_id;
  END CASE;
  RETURN convert_to(key_text,'UTF8');
END $$;

-- The first ten values are exactly row_to_json(table_row)::text. Domain
-- selection/publication fields are deliberately omitted and are read by the
-- separate selector query. Keep this field list aligned with the V1 semantic
-- domain header that append_private_metadata already audits.
CREATE OR REPLACE FUNCTION cmd2_audit_delta_v1_row_commitment(
  p_table_id smallint, p_row json
) RETURNS bytea
LANGUAGE plpgsql STABLE STRICT AS $$
DECLARE serialized text;
BEGIN
  IF p_table_id BETWEEN 1 AND 10 THEN
    serialized := p_row::text;
  ELSIF p_table_id=11 THEN
    SELECT row_to_json(header_row)::text INTO serialized
    FROM (
      SELECT p_row->>'domain' AS domain,
             (p_row->>'head_seq')::bigint AS head_seq,
             (p_row->>'rights_version')::bigint AS rights_version,
             (p_row->>'rights_allowed')::boolean AS rights_allowed,
             (p_row->>'rule_version')::bigint AS rule_version,
             (p_row->>'contract_digest')::char(64) AS contract_digest,
             (p_row->>'schema_profile_digest')::char(64) AS schema_profile_digest,
             (p_row->>'source_revision')::char(64) AS source_revision,
             (p_row->>'source_membership_digest')::char(64) AS source_membership_digest,
             (p_row->>'source_membership_count')::bigint AS source_membership_count,
             (p_row->>'source_epoch')::bigint AS source_epoch,
             (p_row->>'source_generation')::bigint AS source_generation,
             (p_row->>'source_complete')::boolean AS source_complete,
             (p_row->>'source_definition_digest')::char(64) AS source_definition_digest
    ) AS header_row;
  ELSE
    RAISE EXCEPTION 'CMD2 audit delta unknown table id %',p_table_id;
  END IF;
  RETURN sha256(convert_to(serialized,'UTF8'));
END $$;

CREATE OR REPLACE FUNCTION cmd2_audit_delta_v1_record_row() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  changed_domain text;
  table_id smallint;
  operation text;
  old_row json;
  new_row json;
  old_key bytea;
  new_key bytea;
  old_commitment bytea;
  new_commitment bytea;
  next_generation bigint;
BEGIN
  IF TG_OP='UPDATE' AND NEW.domain IS DISTINCT FROM OLD.domain THEN
    RAISE EXCEPTION 'CMD2 metadata domain identity cannot move';
  END IF;
  changed_domain := CASE WHEN TG_OP='DELETE' THEN OLD.domain ELSE NEW.domain END;
  table_id := CASE TG_TABLE_NAME
    WHEN 'cmd2_job' THEN 1
    WHEN 'cmd2_predicate' THEN 2
    WHEN 'cmd2_attempt' THEN 3
    WHEN 'cmd2_member' THEN 4
    WHEN 'cmd2_current' THEN 5
    WHEN 'cmd2_history' THEN 6
    WHEN 'cmd2_receipt' THEN 7
    WHEN 'cmd2_log' THEN 8
    WHEN 'cmd2_outbox' THEN 9
    WHEN 'cmd2_source_index' THEN 10
    ELSE NULL
  END;
  IF table_id IS NULL THEN
    RAISE EXCEPTION 'CMD2 audit delta unknown metadata table %',TG_TABLE_NAME;
  END IF;
  operation := CASE TG_OP WHEN 'INSERT' THEN 'I' WHEN 'UPDATE' THEN 'U' ELSE 'D' END;
  IF NOT EXISTS (
    SELECT 1 FROM cmd2_audit_delta_v1_domain WHERE domain=changed_domain
  ) THEN
    -- Preserve the pre-opt-in V1 fence contract for every unrelated domain.
    UPDATE cmd2_audit_fence
       SET generation=generation+1
     WHERE domain=changed_domain AND generation<9223372036854775807;
    IF NOT FOUND THEN
      RAISE EXCEPTION 'CMD2 audit fence absent or exhausted for %',changed_domain;
    END IF;
    RETURN NULL;
  END IF;
  IF TG_OP<>'INSERT' THEN
    old_row := row_to_json(OLD);
    old_key := cmd2_audit_delta_v1_row_key(table_id,old_row);
    old_commitment := cmd2_audit_delta_v1_row_commitment(table_id,old_row);
  END IF;
  IF TG_OP<>'DELETE' THEN
    new_row := row_to_json(NEW);
    new_key := cmd2_audit_delta_v1_row_key(table_id,new_row);
    new_commitment := cmd2_audit_delta_v1_row_commitment(table_id,new_row);
  END IF;
  UPDATE cmd2_audit_fence
     SET generation=generation+1
   WHERE domain=changed_domain AND generation<9223372036854775807
   RETURNING generation INTO next_generation;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'CMD2 audit fence absent or exhausted for %',changed_domain;
  END IF;
  INSERT INTO cmd2_audit_delta_v1(
    domain,generation,table_id,operation,old_key,new_key,old_commitment,new_commitment
  ) VALUES (
    changed_domain,next_generation,table_id,operation,old_key,new_key,old_commitment,new_commitment
  );
  RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION cmd2_audit_delta_v1_record_domain_header() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  old_row json;
  new_row json;
  old_key bytea;
  new_key bytea;
  old_commitment bytea;
  new_commitment bytea;
  next_generation bigint;
BEGIN
  IF NEW.domain IS DISTINCT FROM OLD.domain THEN
    RAISE EXCEPTION 'CMD2 domain identity cannot move';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM cmd2_audit_delta_v1_domain WHERE domain=NEW.domain
  ) THEN
    -- Unactivated domains keep V1's every-UPDATE fence increment.
    UPDATE cmd2_audit_fence
       SET generation=generation+1
     WHERE domain=NEW.domain AND generation<9223372036854775807;
    IF NOT FOUND THEN
      RAISE EXCEPTION 'CMD2 audit fence absent or exhausted for %',NEW.domain;
    END IF;
    RETURN NULL;
  END IF;
  old_row := row_to_json(OLD);
  new_row := row_to_json(NEW);
  old_commitment := cmd2_audit_delta_v1_row_commitment(11::smallint,old_row);
  new_commitment := cmd2_audit_delta_v1_row_commitment(11::smallint,new_row);
  -- Selector/publication-only changes still consume a generation and append
  -- an equal-commitment domain row. Final selector values are read separately.
  old_key := cmd2_audit_delta_v1_row_key(11::smallint,old_row);
  new_key := cmd2_audit_delta_v1_row_key(11::smallint,new_row);
  UPDATE cmd2_audit_fence
     SET generation=generation+1
   WHERE domain=NEW.domain AND generation<9223372036854775807
   RETURNING generation INTO next_generation;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'CMD2 audit fence absent or exhausted for %',NEW.domain;
  END IF;
  INSERT INTO cmd2_audit_delta_v1(
    domain,generation,table_id,operation,old_key,new_key,old_commitment,new_commitment
  ) VALUES (
    NEW.domain,next_generation,11,'U',old_key,new_key,old_commitment,new_commitment
  );
  RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION cmd2_audit_delta_v1_refuse_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'CMD2 audit delta journal is append-only';
END $$;

CREATE OR REPLACE FUNCTION cmd2_audit_delta_v1_refuse_truncate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'CMD2 audit delta journal cannot be truncated';
END $$;

CREATE OR REPLACE FUNCTION cmd2_audit_delta_v1_refuse_profile_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'CMD2 audit delta domain profile is immutable';
END $$;

CREATE OR REPLACE FUNCTION cmd2_audit_delta_v1_refuse_profile_truncate() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'CMD2 audit delta domain profile cannot be truncated';
END $$;

DROP TRIGGER IF EXISTS cmd2_audit_delta_v1_immutable ON cmd2_audit_delta_v1;
CREATE TRIGGER cmd2_audit_delta_v1_immutable
  BEFORE UPDATE OR DELETE ON cmd2_audit_delta_v1 FOR EACH ROW
  EXECUTE FUNCTION cmd2_audit_delta_v1_refuse_mutation();
DROP TRIGGER IF EXISTS cmd2_audit_delta_v1_refuse_truncate ON cmd2_audit_delta_v1;
CREATE TRIGGER cmd2_audit_delta_v1_refuse_truncate
  BEFORE TRUNCATE ON cmd2_audit_delta_v1 FOR EACH STATEMENT
  EXECUTE FUNCTION cmd2_audit_delta_v1_refuse_truncate();
DROP TRIGGER IF EXISTS cmd2_audit_delta_v1_domain_immutable ON cmd2_audit_delta_v1_domain;
CREATE TRIGGER cmd2_audit_delta_v1_domain_immutable
  BEFORE UPDATE OR DELETE ON cmd2_audit_delta_v1_domain FOR EACH ROW
  EXECUTE FUNCTION cmd2_audit_delta_v1_refuse_profile_mutation();
DROP TRIGGER IF EXISTS cmd2_audit_delta_v1_domain_refuse_truncate ON cmd2_audit_delta_v1_domain;
CREATE TRIGGER cmd2_audit_delta_v1_domain_refuse_truncate
  BEFORE TRUNCATE ON cmd2_audit_delta_v1_domain FOR EACH STATEMENT
  EXECUTE FUNCTION cmd2_audit_delta_v1_refuse_profile_truncate();

DROP TRIGGER IF EXISTS cmd2_audit_domain_update ON cmd2_domain;
CREATE TRIGGER cmd2_audit_domain_update
  AFTER UPDATE ON cmd2_domain FOR EACH ROW
  EXECUTE FUNCTION cmd2_audit_delta_v1_record_domain_header();

DO $$
DECLARE table_name text;
BEGIN
  FOREACH table_name IN ARRAY ARRAY[
    'cmd2_job','cmd2_predicate','cmd2_attempt','cmd2_member',
    'cmd2_current','cmd2_history','cmd2_receipt','cmd2_log','cmd2_outbox','cmd2_source_index'
  ] LOOP
    EXECUTE format('DROP TRIGGER IF EXISTS %I ON %I', 'cmd2_audit_' || table_name, table_name);
    EXECUTE format(
      'CREATE TRIGGER %I AFTER INSERT OR UPDATE OR DELETE ON %I '
      || 'FOR EACH ROW EXECUTE FUNCTION cmd2_audit_delta_v1_record_row()',
      'cmd2_audit_' || table_name, table_name);
  END LOOP;
END $$;
