-- CMD.2 private shadow laboratory only. This schema has no ToS source admission.
-- All digest strings are lowercase SHA-256 hex, matching the CMD.1 lab ABI.
CREATE TABLE IF NOT EXISTS cmd2_domain (
  domain text PRIMARY KEY,
  head_seq bigint NOT NULL DEFAULT 0 CHECK (head_seq >= 0),
  rights_version bigint NOT NULL DEFAULT 0 CHECK (rights_version >= 0),
  rights_allowed boolean NOT NULL DEFAULT true,
  rule_version bigint NOT NULL DEFAULT 0 CHECK (rule_version >= 0),
  contract_digest char(64) NOT NULL,
  published_seq bigint NOT NULL DEFAULT 0 CHECK (published_seq >= 0),
  complete_cut_digest char(64)
);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS published_seq bigint NOT NULL DEFAULT 0
  CHECK (published_seq >= 0);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS complete_cut_digest char(64);

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
