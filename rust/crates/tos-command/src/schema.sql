-- Synthetic CMD.1 laboratory schema only. No ToS source or rights owner data.
CREATE TABLE IF NOT EXISTS cmd1_coordinator (
  domain text PRIMARY KEY,
  head_seq bigint NOT NULL DEFAULT 0 CHECK (head_seq >= 0),
  rights_version bigint NOT NULL DEFAULT 0 CHECK (rights_version >= 0),
  rights_allowed boolean NOT NULL DEFAULT true,
  rule_version bigint NOT NULL DEFAULT 0 CHECK (rule_version >= 0),
  contract_digest char(64) NOT NULL
);
CREATE TABLE IF NOT EXISTS cmd1_record (
  domain text NOT NULL REFERENCES cmd1_coordinator(domain),
  namespace text NOT NULL,
  key text NOT NULL,
  version bigint NOT NULL CHECK (version > 0),
  digest char(64) NOT NULL,
  byte_length bigint NOT NULL CHECK (byte_length > 0),
  PRIMARY KEY(domain, namespace, key)
);
CREATE TABLE IF NOT EXISTS cmd1_record_history (
  domain text NOT NULL REFERENCES cmd1_coordinator(domain),
  namespace text NOT NULL,
  key text NOT NULL,
  version bigint NOT NULL CHECK (version > 0),
  digest char(64) NOT NULL,
  byte_length bigint NOT NULL CHECK (byte_length > 0),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  PRIMARY KEY(domain,namespace,key,version)
);
CREATE TABLE IF NOT EXISTS cmd1_predicate (
  domain text NOT NULL REFERENCES cmd1_coordinator(domain),
  kind text NOT NULL,
  owner text NOT NULL,
  scope text NOT NULL,
  token text NOT NULL,
  definition_version text NOT NULL,
  generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
  complete boolean NOT NULL DEFAULT false,
  PRIMARY KEY(domain,kind,owner,scope,token)
);
CREATE TABLE IF NOT EXISTS cmd1_job (
  domain text NOT NULL REFERENCES cmd1_coordinator(domain),
  job_id text NOT NULL,
  fence_epoch bigint NOT NULL CHECK (fence_epoch > 0),
  PRIMARY KEY(domain,job_id)
);
CREATE TABLE IF NOT EXISTS cmd1_receipt (
  domain text NOT NULL REFERENCES cmd1_coordinator(domain),
  command_id text NOT NULL,
  commit_seq bigint NOT NULL,
  raw_request_digest char(64) NOT NULL,
  delta_digest char(64) NOT NULL,
  attestation_digest char(64) NOT NULL,
  input_profile_id text NOT NULL,
  PRIMARY KEY(domain,command_id),
  UNIQUE(domain,commit_seq)
);
CREATE TABLE IF NOT EXISTS cmd1_commit_log (
  domain text NOT NULL REFERENCES cmd1_coordinator(domain),
  commit_seq bigint NOT NULL,
  event_kind text NOT NULL CHECK (event_kind IN ('command','rights','rule')),
  command_id text NOT NULL,
  delta_digest char(64) NOT NULL,
  members text[] NOT NULL,
  PRIMARY KEY(domain,commit_seq),
  UNIQUE(domain,event_kind,command_id)
);
CREATE TABLE IF NOT EXISTS cmd1_outbox (
  domain text NOT NULL REFERENCES cmd1_coordinator(domain),
  commit_seq bigint NOT NULL,
  event_id text NOT NULL,
  PRIMARY KEY(domain,event_id),
  UNIQUE(domain,commit_seq)
);
CREATE TABLE IF NOT EXISTS cmd1_publication (
  domain text PRIMARY KEY REFERENCES cmd1_coordinator(domain),
  through_seq bigint NOT NULL DEFAULT 0 CHECK (through_seq >= 0),
  log_digest char(64)
);
