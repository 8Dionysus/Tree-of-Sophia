-- Optional owner extension for the native PG Work revision route.
-- Provision this explicitly in the selected owner schema before enabling the
-- protected Work grant. Normal reads and writes only verify this exact shape.
CREATE TABLE cmd2_work_revision_history_v1 (
    domain text NOT NULL REFERENCES cmd2_domain(domain),
    store_id bytea NOT NULL CHECK (octet_length(store_id) = 16),
    source_path text NOT NULL,
    record_id text NOT NULL,
    generation bigint NOT NULL CHECK (generation > 0),
    previous_receipt_digest char(64),
    transaction_id text NOT NULL,
    command_id text NOT NULL,
    owner_configuration_raw_digest char(64) NOT NULL,
    owner_configuration_canonical_digest char(64) NOT NULL,
    request_raw_digest char(64) NOT NULL,
    request_canonical_digest char(64) NOT NULL,
    expected_publication text,
    publication_observation_digest char(64) NOT NULL,
    compound_members_digest char(64) NOT NULL,
    authorization_raw bytea NOT NULL CHECK (octet_length(authorization_raw) <= 65536),
    before_refs bytea NOT NULL CHECK (octet_length(before_refs) <= 65536),
    after_refs bytea NOT NULL CHECK (octet_length(after_refs) <= 65536),
    receipt_digest char(64) NOT NULL,
    prepare_id bytea NOT NULL,
    commit_seq bigint NOT NULL,
    PRIMARY KEY (domain, source_path, record_id, generation),
    UNIQUE (domain, source_path, record_id, transaction_id),
    UNIQUE (domain, source_path, record_id, command_id),
    UNIQUE (domain, prepare_id)
);

CREATE TRIGGER cmd2_audit_cmd2_work_revision_history_v1
AFTER INSERT OR UPDATE OR DELETE ON cmd2_work_revision_history_v1
FOR EACH ROW EXECUTE FUNCTION cmd2_bump_audit_fence();

CREATE TRIGGER cmd2_refuse_cmd2_work_revision_history_v1_truncate
BEFORE TRUNCATE ON cmd2_work_revision_history_v1
FOR EACH STATEMENT EXECUTE FUNCTION cmd2_refuse_audited_truncate();
