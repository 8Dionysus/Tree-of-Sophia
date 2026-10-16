-- CMD.2 private PostgreSQL 16 coordinator schema for native source import and
-- search. This schema has no ToS source admission. Deploy in a dedicated
-- private database with ordinary query roles unable to bypass adapter rights,
-- write coordinator tables, or alter its DDL and audit triggers. The database
-- owner remains trusted; this source comment does not prove a live deployment.
-- All digest strings are lowercase SHA-256 hex, matching the CMD.1 ABI.
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
  selected_generation_digest char(64),
  source_claim_navigation_reference_baseline_seq bigint
    CHECK (source_claim_navigation_reference_baseline_seq >= 0)
);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS schema_profile_digest char(64);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS published_seq bigint NOT NULL DEFAULT 0
  CHECK (published_seq >= 0);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS complete_cut_digest char(64);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS complete_cut_generation bigint
  CHECK (complete_cut_generation >= 0);
ALTER TABLE cmd2_domain ADD COLUMN IF NOT EXISTS selected_generation_digest char(64);
-- A NULL Claim-navigation baseline means the current-only index has not been
-- declared complete for any as-of cut. Bootstrap records an exact baseline.
ALTER TABLE cmd2_domain
  ADD COLUMN IF NOT EXISTS source_claim_navigation_reference_baseline_seq bigint
  CHECK (source_claim_navigation_reference_baseline_seq >= 0);
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
-- Revision attempts retain the exact protected owner configuration identity so
-- process-loss replay cannot bind a new config to an old prepare ID.
ALTER TABLE cmd2_attempt ADD COLUMN IF NOT EXISTS source_configuration_raw_digest char(64);
ALTER TABLE cmd2_attempt ADD COLUMN IF NOT EXISTS source_configuration_canonical_digest char(64);
ALTER TABLE cmd2_attempt ADD COLUMN IF NOT EXISTS source_publication_package bytea;

-- Owner-derived unique identities. Original bytes remain in current/history.
CREATE TABLE IF NOT EXISTS cmd2_source_index (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  kind text NOT NULL,
  token text NOT NULL,
  path text NOT NULL,
  definition_digest char(64) NOT NULL,
  PRIMARY KEY(domain,kind,token)
);
-- Text identities may legitimately recur on separate exact source members or
-- retained ancestry. Keep public Text contributions multivalued by member;
-- token admission is fenced by the matching cmd2_predicate generation.
CREATE TABLE IF NOT EXISTS cmd2_source_text_identity (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  token text COLLATE "C" NOT NULL CHECK (length(token) BETWEEN 1 AND 4096),
  path text COLLATE "C" NOT NULL,
  member_digest char(64) NOT NULL CHECK (member_digest ~ '^[0-9a-f]{64}$'),
  definition_digest char(64) NOT NULL CHECK (definition_digest ~ '^[0-9a-f]{64}$'),
  CONSTRAINT cmd2_source_text_identity_public_path CHECK (
    path LIKE 'ToS/%'
    AND path <> 'ToS/source-witnesses/owner-local'
    AND path NOT LIKE 'ToS/source-witnesses/owner-local/%'
  ),
  PRIMARY KEY(domain,token,path)
);
DO $$ BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conrelid='cmd2_source_text_identity'::regclass
      AND conname='cmd2_source_text_identity_public_path'
  ) THEN
    ALTER TABLE cmd2_source_text_identity
      ADD CONSTRAINT cmd2_source_text_identity_public_path CHECK (
        path LIKE 'ToS/%'
        AND path <> 'ToS/source-witnesses/owner-local'
        AND path NOT LIKE 'ToS/source-witnesses/owner-local/%'
      );
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS cmd2_source_text_identity_token_seek
  ON cmd2_source_text_identity(domain,token COLLATE "C",path COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_source_text_identity_path_seek
  ON cmd2_source_text_identity(domain,path COLLATE "C",token COLLATE "C");
-- Semantic annotation entity IDs can have multiple packet owners. Keep this
-- separate from the one-owner source identity table and fence each token with
-- its own unique-predicate generation.
CREATE TABLE IF NOT EXISTS cmd2_source_annotation_identity (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  token text COLLATE "C" NOT NULL CHECK (length(token) BETWEEN 1 AND 4096),
  path text COLLATE "C" NOT NULL,
  member_digest char(64) NOT NULL CHECK (member_digest ~ '^[0-9a-f]{64}$'),
  definition_digest char(64) NOT NULL CHECK (definition_digest ~ '^[0-9a-f]{64}$'),
  CONSTRAINT cmd2_source_annotation_identity_public_path CHECK (
    path LIKE 'ToS/source-witnesses/%'
    AND path <> 'ToS/source-witnesses/owner-local'
    AND path NOT LIKE 'ToS/source-witnesses/owner-local/%'
  ),
  PRIMARY KEY(domain,token,path)
);
CREATE INDEX IF NOT EXISTS cmd2_source_annotation_identity_token_seek
  ON cmd2_source_annotation_identity(domain,token COLLATE "C",path COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_source_annotation_identity_path_seek
  ON cmd2_source_annotation_identity(domain,path COLLATE "C",token COLLATE "C");
-- Exact reverse Claim topology contributions for Item adoption. One row owns
-- each current embodied_by/exemplified_by Claim; source bytes remain in
-- cmd2_current/history and every endpoint range is fenced by cmd2_predicate.
CREATE TABLE IF NOT EXISTS cmd2_source_claim_dependency (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  predicate text COLLATE "C" NOT NULL
    CHECK (predicate IN ('embodied_by','exemplified_by')),
  claim_id text COLLATE "C" NOT NULL
    CHECK (length(claim_id) BETWEEN 1 AND 4096),
  path text COLLATE "C" NOT NULL,
  subject_endpoint text COLLATE "C" NOT NULL
    CHECK (length(subject_endpoint) BETWEEN 1 AND 4096 AND subject_endpoint LIKE 'tos.%'),
  object_endpoint text COLLATE "C" NOT NULL
    CHECK (length(object_endpoint) BETWEEN 1 AND 4096 AND object_endpoint LIKE 'tos.%'),
  member_digest char(64) NOT NULL CHECK (member_digest ~ '^[0-9a-f]{64}$'),
  definition_digest char(64) NOT NULL CHECK (definition_digest ~ '^[0-9a-f]{64}$'),
  CONSTRAINT cmd2_source_claim_dependency_public_path CHECK (
    (
      path ~ '^ToS/source-witnesses/relations/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/works/[^/]+(/[^/]+)*/expressions/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/works/[^/]+(/[^/]+)*/expressions/[^/]+/editions/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/items/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/([^/]+/)*editions/[^/]+/items/[^/]+/source-claims[.]jsonl$'
    )
    AND path !~ '(^|/)([.][.]?|[.]git)(/|$)'
    AND path !~ E'[\\001-\\037]'
    AND position(chr(92) in path) = 0
    AND path <> 'ToS/source-witnesses/owner-local'
    AND path NOT LIKE 'ToS/source-witnesses/owner-local/%'
  ),
  PRIMARY KEY(domain,claim_id)
);
CREATE INDEX IF NOT EXISTS cmd2_source_claim_dependency_subject_seek
  ON cmd2_source_claim_dependency(domain,predicate COLLATE "C",subject_endpoint COLLATE "C",claim_id COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_source_claim_dependency_object_seek
  ON cmd2_source_claim_dependency(domain,predicate COLLATE "C",object_endpoint COLLATE "C",claim_id COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_source_claim_dependency_path_seek
  ON cmd2_source_claim_dependency(domain,path COLLATE "C",claim_id COLLATE "C");
ALTER TABLE cmd2_source_claim_dependency
  DROP CONSTRAINT IF EXISTS cmd2_source_claim_dependency_public_path;
ALTER TABLE cmd2_source_claim_dependency
  ADD CONSTRAINT cmd2_source_claim_dependency_public_path CHECK (
    (
      path ~ '^ToS/source-witnesses/relations/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/works/[^/]+(/[^/]+)*/expressions/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/works/[^/]+(/[^/]+)*/expressions/[^/]+/editions/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/items/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/([^/]+/)*editions/[^/]+/items/[^/]+/source-claims[.]jsonl$'
    )
    AND path !~ '(^|/)([.][.]?|[.]git)(/|$)'
    AND path !~ E'[\\001-\\037]'
    AND position(chr(92) in path) = 0
    AND path <> 'ToS/source-witnesses/owner-local'
    AND path NOT LIKE 'ToS/source-witnesses/owner-local/%'
  );
-- Exact reverse references used by source-navigation Claim refresh. Each Claim
-- owns one row per complete JSON-value ToS ID or public source path; owner
-- stream bytes remain authoritative in cmd2_current/history.
CREATE TABLE IF NOT EXISTS cmd2_source_claim_navigation_reference (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  claim_id text COLLATE "C" NOT NULL
    CHECK (length(claim_id) BETWEEN 1 AND 4096 AND claim_id LIKE 'tos.claim.%'),
  path text COLLATE "C" NOT NULL,
  basis text COLLATE "C" NOT NULL
    CHECK (basis IN ('claim-value','owner-input','shared-schema')),
  endpoint_kind text COLLATE "C" NOT NULL
    CHECK (endpoint_kind IN ('tos-id','source-path')),
  endpoint text COLLATE "C" NOT NULL CHECK (length(endpoint) BETWEEN 1 AND 4096),
  member_digest char(64) NOT NULL CHECK (member_digest ~ '^[0-9a-f]{64}$'),
  definition_digest char(64) NOT NULL CHECK (definition_digest ~ '^[0-9a-f]{64}$'),
  valid_from_seq bigint NOT NULL CHECK (valid_from_seq >= 0),
  valid_to_seq bigint CHECK (valid_to_seq IS NULL OR valid_to_seq > valid_from_seq),
  CONSTRAINT cmd2_source_claim_navigation_reference_public_path CHECK (
    (
      path ~ '^ToS/source-witnesses/relations/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/works/[^/]+(/[^/]+)*/expressions/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/works/[^/]+(/[^/]+)*/expressions/[^/]+/editions/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/items/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/([^/]+/)*editions/[^/]+/items/[^/]+/source-claims[.]jsonl$'
    )
    AND path !~ '(^|/)([.][.]?|[.]git)(/|$)'
    AND path !~ E'[\\001-\\037]'
    AND position(chr(92) in path) = 0
    AND path <> 'ToS/source-witnesses/owner-local'
    AND path NOT LIKE 'ToS/source-witnesses/owner-local/%'
  ),
  CONSTRAINT cmd2_source_claim_navigation_reference_endpoint_shape CHECK (
    (endpoint_kind='tos-id' AND endpoint LIKE 'tos.%') OR
    (endpoint_kind='source-path' AND endpoint LIKE 'ToS/%'
      AND endpoint <> 'ToS/source-witnesses/owner-local'
      AND endpoint NOT LIKE 'ToS/source-witnesses/owner-local/%')
  ),
  PRIMARY KEY(domain,path,claim_id,endpoint_kind,endpoint,basis,valid_from_seq)
);
ALTER TABLE cmd2_source_claim_navigation_reference
  ADD COLUMN IF NOT EXISTS basis text COLLATE "C" NOT NULL DEFAULT 'claim-value'
  CHECK (basis IN ('claim-value','owner-input','shared-schema'));
ALTER TABLE cmd2_source_claim_navigation_reference
  ADD COLUMN IF NOT EXISTS valid_from_seq bigint
  CHECK (valid_from_seq IS NULL OR valid_from_seq >= 0);
ALTER TABLE cmd2_source_claim_navigation_reference
  ADD COLUMN IF NOT EXISTS valid_to_seq bigint
  CHECK (valid_to_seq IS NULL OR (valid_from_seq IS NOT NULL AND valid_to_seq > valid_from_seq));
ALTER TABLE cmd2_source_claim_navigation_reference
  DROP CONSTRAINT IF EXISTS cmd2_source_claim_navigation_reference_interval_shape;
ALTER TABLE cmd2_source_claim_navigation_reference
  ADD CONSTRAINT cmd2_source_claim_navigation_reference_interval_shape
  CHECK (valid_from_seq IS NULL OR valid_to_seq IS NULL OR valid_to_seq > valid_from_seq);
-- Fresh empty tables can enforce the new non-NULL start. Existing current-only
-- rows deliberately remain unversioned and cannot be mistaken for history;
-- only a legitimate bootstrap/reindex may replace them with a declared cut.
DO $$ BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM cmd2_source_claim_navigation_reference WHERE valid_from_seq IS NULL
  ) THEN
    ALTER TABLE cmd2_source_claim_navigation_reference
      ALTER COLUMN valid_from_seq SET NOT NULL;
  END IF;
END $$;
ALTER TABLE cmd2_source_claim_navigation_reference
  DROP CONSTRAINT IF EXISTS cmd2_source_claim_navigation_reference_pkey;
DROP INDEX IF EXISTS cmd2_source_claim_navigation_reference_interval_identity;
CREATE UNIQUE INDEX cmd2_source_claim_navigation_reference_interval_identity
  ON cmd2_source_claim_navigation_reference(
    domain,path,claim_id,endpoint_kind,endpoint,basis,valid_from_seq
  ) NULLS NOT DISTINCT;
DROP INDEX IF EXISTS cmd2_source_claim_navigation_reference_active_unique;
CREATE UNIQUE INDEX cmd2_source_claim_navigation_reference_active_unique
  ON cmd2_source_claim_navigation_reference(domain,path,claim_id,endpoint_kind,endpoint,basis)
  WHERE valid_from_seq IS NOT NULL AND valid_to_seq IS NULL;
ALTER TABLE cmd2_source_claim_navigation_reference
  DROP CONSTRAINT IF EXISTS cmd2_source_claim_navigation_reference_public_path;
ALTER TABLE cmd2_source_claim_navigation_reference
  ADD CONSTRAINT cmd2_source_claim_navigation_reference_public_path CHECK (
    (
      path ~ '^ToS/source-witnesses/relations/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/works/[^/]+(/[^/]+)*/expressions/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/works/[^/]+(/[^/]+)*/expressions/[^/]+/editions/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/items/[^/]+/source-claims[.]jsonl$' OR
      path ~ '^ToS/source-witnesses/([^/]+/)*editions/[^/]+/items/[^/]+/source-claims[.]jsonl$'
    )
    AND path !~ '(^|/)([.][.]?|[.]git)(/|$)'
    AND path !~ E'[\\001-\\037]'
    AND position(chr(92) in path) = 0
    AND path <> 'ToS/source-witnesses/owner-local'
    AND path NOT LIKE 'ToS/source-witnesses/owner-local/%'
  );
DROP INDEX IF EXISTS cmd2_source_claim_navigation_reference_endpoint_seek;
CREATE INDEX cmd2_source_claim_navigation_reference_endpoint_seek
  ON cmd2_source_claim_navigation_reference(domain,endpoint_kind COLLATE "C",endpoint COLLATE "C",valid_from_seq,path COLLATE "C",claim_id COLLATE "C");
DROP INDEX IF EXISTS cmd2_source_claim_navigation_reference_path_seek;
CREATE INDEX cmd2_source_claim_navigation_reference_path_seek
  ON cmd2_source_claim_navigation_reference(domain,path COLLATE "C",valid_from_seq,claim_id COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_source_claim_navigation_reference_endpoint_active_seek
  ON cmd2_source_claim_navigation_reference(domain,endpoint_kind COLLATE "C",endpoint COLLATE "C",path COLLATE "C",claim_id COLLATE "C")
  WHERE valid_from_seq IS NOT NULL AND valid_to_seq IS NULL;
-- Multiplicity-preserving current Item-manifest File assignments. Payload
-- bytes remain in the protected custody store; this index contains metadata.
CREATE TABLE IF NOT EXISTS cmd2_source_item_file_assignment (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  item_id text COLLATE "C" NOT NULL
    CHECK (length(item_id) BETWEEN 1 AND 4096 AND item_id LIKE 'tos.item.%'),
  manifest_path text COLLATE "C" NOT NULL,
  edition_id text COLLATE "C" NOT NULL DEFAULT 'tos.edition.unmigrated'
    CHECK (length(edition_id) BETWEEN 1 AND 4096 AND edition_id LIKE 'tos.edition.%'),
  payload_ordinal integer NOT NULL CHECK (payload_ordinal >= 0),
  file_id text COLLATE "C" NOT NULL
    CHECK (length(file_id) BETWEEN 1 AND 4096 AND file_id LIKE 'tos.file.%'),
  member_digest char(64) NOT NULL CHECK (member_digest ~ '^[0-9a-f]{64}$'),
  rights_id text COLLATE "C" NOT NULL
    CHECK (length(rights_id) BETWEEN 1 AND 4096 AND rights_id LIKE 'tos.rights.%'),
  rights_digest char(64) NOT NULL CHECK (rights_digest ~ '^[0-9a-f]{64}$'),
  definition_digest char(64) NOT NULL CHECK (definition_digest ~ '^[0-9a-f]{64}$'),
  CONSTRAINT cmd2_source_item_file_assignment_public_path CHECK (
    manifest_path LIKE 'ToS/source-witnesses/%/items/%/item.manifest.json'
    AND manifest_path NOT LIKE 'ToS/source-witnesses/owner-local/%'
    AND manifest_path NOT LIKE 'ToS/source-witnesses/.record-revisions/%'
  ),
  PRIMARY KEY(domain,manifest_path,payload_ordinal)
);
ALTER TABLE cmd2_source_item_file_assignment
  ADD COLUMN IF NOT EXISTS edition_id text COLLATE "C" NOT NULL DEFAULT 'tos.edition.unmigrated'
    CHECK (length(edition_id) BETWEEN 1 AND 4096 AND edition_id LIKE 'tos.edition.%');
CREATE INDEX IF NOT EXISTS cmd2_source_item_file_assignment_file_seek
  ON cmd2_source_item_file_assignment(domain,file_id COLLATE "C",manifest_path COLLATE "C",payload_ordinal);
CREATE INDEX IF NOT EXISTS cmd2_source_item_file_assignment_manifest_seek
  ON cmd2_source_item_file_assignment(domain,manifest_path COLLATE "C",payload_ordinal);
CREATE INDEX IF NOT EXISTS cmd2_source_item_file_assignment_item_seek
  ON cmd2_source_item_file_assignment(domain,item_id COLLATE "C",manifest_path COLLATE "C",payload_ordinal);
CREATE INDEX IF NOT EXISTS cmd2_source_item_file_assignment_rights_seek
  ON cmd2_source_item_file_assignment(domain,rights_id COLLATE "C",manifest_path COLLATE "C",payload_ordinal);
CREATE INDEX IF NOT EXISTS cmd2_source_item_file_assignment_edition_seek
  ON cmd2_source_item_file_assignment(domain,edition_id COLLATE "C",manifest_path COLLATE "C",payload_ordinal);
-- Exact owner-derived Agent inventory contribution; original body custody
-- stays in current/history. NULL is an unproved scope, never empty inventory.
ALTER TABLE cmd2_source_index ADD COLUMN IF NOT EXISTS inventory_projection bytea;
ALTER TABLE cmd2_attempt ADD COLUMN IF NOT EXISTS source_projections bytea;

-- The current selected publication is an independent source-owned pointer.
-- Its explicit generation-zero row is established from the full bootstrap cut;
-- unrelated authored-source commits leave it unchanged. Transition history
-- retains metadata-only packages while source bodies remain in current/history.
-- A legacy STO chain is attached to the real bootstrap-completion event. Its
-- original transitions retain their native generation/token and deliberately
-- have no fabricated PostgreSQL commit sequence.
CREATE TABLE IF NOT EXISTS cmd2_source_publication_import (
  domain text PRIMARY KEY REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  origin_kind text NOT NULL CHECK (origin_kind='legacy_sto_import'),
  import_event_id text NOT NULL CHECK (import_event_id ~ '^sha256:[0-9a-f]{64}$'),
  receipt_digest char(64) NOT NULL CHECK (receipt_digest ~ '^[0-9a-f]{64}$'),
  receipt_bytes bytea NOT NULL CHECK (octet_length(receipt_bytes) BETWEEN 1 AND 65536),
  store_id bytea NOT NULL CHECK (octet_length(store_id) = 16),
  bootstrap_source_revision char(64) NOT NULL CHECK (bootstrap_source_revision ~ '^[0-9a-f]{64}$'),
  bootstrap_membership_digest char(64) NOT NULL CHECK (bootstrap_membership_digest ~ '^[0-9a-f]{64}$'),
  bootstrap_membership_count bigint NOT NULL CHECK (bootstrap_membership_count >= 0),
  import_source_head bigint NOT NULL CHECK (import_source_head >= 0),
  chain_root_digest char(64) NOT NULL CHECK (chain_root_digest ~ '^[0-9a-f]{64}$'),
  transition_count bigint NOT NULL CHECK (transition_count > 0),
  head_control_digest char(64) NOT NULL CHECK (head_control_digest ~ '^[0-9a-f]{64}$'),
  terminal_generation bigint NOT NULL CHECK (terminal_generation > 0),
  terminal_token text NOT NULL CHECK (terminal_token ~ '^sha256:[0-9a-f]{64}$'),
  UNIQUE (domain, import_event_id)
);
CREATE TABLE IF NOT EXISTS cmd2_source_publication_current (
  domain text PRIMARY KEY REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  state text NOT NULL CHECK (state IN ('absent','present')),
  publication_generation bigint NOT NULL CHECK (publication_generation >= 0),
  token text CHECK (token ~ '^sha256:[0-9a-f]{64}$'),
  base_token text CHECK (base_token IS NULL OR base_token ~ '^sha256:[0-9a-f]{64}$'),
  base_generation bigint NOT NULL CHECK (base_generation >= 0),
  transaction_id text CHECK (transaction_id IS NULL OR transaction_id ~ '^sha256:[0-9a-f]{64}$'),
  package_digest char(64) CHECK (package_digest IS NULL OR package_digest ~ '^[0-9a-f]{64}$'),
  package_bytes bytea CHECK (package_bytes IS NULL OR octet_length(package_bytes) BETWEEN 1 AND 1048576),
  source_commit_seq bigint,
  origin_kind text,
  import_event_id text,
  import_transition_index bigint,
  import_receipt_digest char(64),
  store_id bytea NOT NULL CHECK (octet_length(store_id) = 16),
  bootstrap_source_revision char(64) NOT NULL CHECK (bootstrap_source_revision ~ '^[0-9a-f]{64}$'),
  bootstrap_membership_digest char(64) NOT NULL CHECK (bootstrap_membership_digest ~ '^[0-9a-f]{64}$'),
  bootstrap_membership_count bigint NOT NULL CHECK (bootstrap_membership_count >= 0)
);
CREATE TABLE IF NOT EXISTS cmd2_source_publication_history (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  publication_generation bigint NOT NULL CHECK (publication_generation >= 0),
  state text NOT NULL CHECK (state IN ('absent','present')),
  token text CHECK (token ~ '^sha256:[0-9a-f]{64}$'),
  base_token text CHECK (base_token IS NULL OR base_token ~ '^sha256:[0-9a-f]{64}$'),
  base_generation bigint NOT NULL CHECK (base_generation >= 0),
  transaction_id text CHECK (transaction_id IS NULL OR transaction_id ~ '^sha256:[0-9a-f]{64}$'),
  package_digest char(64) CHECK (package_digest IS NULL OR package_digest ~ '^[0-9a-f]{64}$'),
  package_bytes bytea CHECK (package_bytes IS NULL OR octet_length(package_bytes) BETWEEN 1 AND 1048576),
  source_commit_seq bigint,
  origin_kind text,
  import_event_id text,
  import_transition_index bigint,
  import_receipt_digest char(64),
  store_id bytea NOT NULL CHECK (octet_length(store_id) = 16),
  bootstrap_source_revision char(64) NOT NULL CHECK (bootstrap_source_revision ~ '^[0-9a-f]{64}$'),
  bootstrap_membership_digest char(64) NOT NULL CHECK (bootstrap_membership_digest ~ '^[0-9a-f]{64}$'),
  bootstrap_membership_count bigint NOT NULL CHECK (bootstrap_membership_count >= 0),
  PRIMARY KEY (domain, publication_generation),
  UNIQUE (domain, source_commit_seq)
);
-- Additive support for databases that already created the first publication
-- ABI. Old present rows are genuine PG commits; the explicit absent row was a
-- real bootstrap absence. Imported rows use NULL source_commit_seq only.
ALTER TABLE cmd2_source_publication_current
  ADD COLUMN IF NOT EXISTS origin_kind text,
  ADD COLUMN IF NOT EXISTS import_event_id text,
  ADD COLUMN IF NOT EXISTS import_transition_index bigint,
  ADD COLUMN IF NOT EXISTS import_receipt_digest char(64);
ALTER TABLE cmd2_source_publication_history
  ADD COLUMN IF NOT EXISTS origin_kind text,
  ADD COLUMN IF NOT EXISTS import_event_id text,
  ADD COLUMN IF NOT EXISTS import_transition_index bigint,
  ADD COLUMN IF NOT EXISTS import_receipt_digest char(64);
ALTER TABLE cmd2_source_publication_current ALTER COLUMN source_commit_seq DROP NOT NULL;
ALTER TABLE cmd2_source_publication_history ALTER COLUMN source_commit_seq DROP NOT NULL;
UPDATE cmd2_source_publication_current SET origin_kind=CASE WHEN state='absent' THEN 'bootstrap_absent' ELSE 'pg_commit' END WHERE origin_kind IS NULL;
UPDATE cmd2_source_publication_history SET origin_kind=CASE WHEN state='absent' THEN 'bootstrap_absent' ELSE 'pg_commit' END WHERE origin_kind IS NULL;
ALTER TABLE cmd2_source_publication_current ALTER COLUMN origin_kind SET NOT NULL;
ALTER TABLE cmd2_source_publication_history ALTER COLUMN origin_kind SET NOT NULL;
ALTER TABLE cmd2_source_publication_current
  DROP CONSTRAINT IF EXISTS cmd2_source_publication_current_source_commit_seq_check,
  DROP CONSTRAINT IF EXISTS cmd2_source_publication_current_origin_check,
  DROP CONSTRAINT IF EXISTS cmd2_source_publication_current_import_digest_check;
ALTER TABLE cmd2_source_publication_history
  DROP CONSTRAINT IF EXISTS cmd2_source_publication_history_source_commit_seq_check,
  DROP CONSTRAINT IF EXISTS cmd2_source_publication_history_origin_check,
  DROP CONSTRAINT IF EXISTS cmd2_source_publication_history_import_digest_check;
DO $$
DECLARE constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname FROM pg_constraint
    WHERE conrelid='cmd2_source_publication_current'::regclass AND contype='c'
      AND pg_get_constraintdef(oid) ILIKE '%source_commit_seq%'
  LOOP
    EXECUTE format('ALTER TABLE cmd2_source_publication_current DROP CONSTRAINT %I', constraint_name);
  END LOOP;
  FOR constraint_name IN
    SELECT conname FROM pg_constraint
    WHERE conrelid='cmd2_source_publication_history'::regclass AND contype='c'
      AND pg_get_constraintdef(oid) ILIKE '%source_commit_seq%'
  LOOP
    EXECUTE format('ALTER TABLE cmd2_source_publication_history DROP CONSTRAINT %I', constraint_name);
  END LOOP;
END $$;
ALTER TABLE cmd2_source_publication_current
  ADD CONSTRAINT cmd2_source_publication_current_source_commit_seq_check
    CHECK (source_commit_seq IS NULL OR source_commit_seq >= 0),
  ADD CONSTRAINT cmd2_source_publication_current_origin_check CHECK (
    (state='absent' AND publication_generation=0 AND token IS NULL
      AND base_token IS NULL AND base_generation=0 AND transaction_id IS NULL
      AND package_digest IS NULL AND package_bytes IS NULL AND source_commit_seq=0
      AND origin_kind='bootstrap_absent' AND import_event_id IS NULL
      AND import_transition_index IS NULL AND import_receipt_digest IS NULL)
    OR
    (state='present' AND publication_generation > 0 AND token IS NOT NULL
      AND transaction_id IS NOT NULL AND package_digest IS NOT NULL
      AND package_bytes IS NOT NULL
      AND publication_generation = base_generation + 2
      AND ((base_generation=0 AND base_token IS NULL)
        OR (base_generation>0 AND base_token IS NOT NULL))
      AND ((origin_kind='pg_commit' AND source_commit_seq > 0
            AND import_event_id IS NULL AND import_transition_index IS NULL
            AND import_receipt_digest IS NULL)
        OR (origin_kind='legacy_sto_import' AND source_commit_seq IS NULL
            AND import_event_id IS NOT NULL AND import_transition_index > 0
            AND import_receipt_digest IS NOT NULL)))
  ),
  ADD CONSTRAINT cmd2_source_publication_current_import_digest_check
    CHECK (import_receipt_digest IS NULL OR import_receipt_digest ~ '^[0-9a-f]{64}$');
ALTER TABLE cmd2_source_publication_history
  ADD CONSTRAINT cmd2_source_publication_history_source_commit_seq_check
    CHECK (source_commit_seq IS NULL OR source_commit_seq >= 0),
  ADD CONSTRAINT cmd2_source_publication_history_origin_check CHECK (
    (state='absent' AND publication_generation=0 AND token IS NULL
      AND base_token IS NULL AND base_generation=0 AND transaction_id IS NULL
      AND package_digest IS NULL AND package_bytes IS NULL AND source_commit_seq=0
      AND origin_kind='bootstrap_absent' AND import_event_id IS NULL
      AND import_transition_index IS NULL AND import_receipt_digest IS NULL)
    OR
    (state='present' AND publication_generation > 0 AND token IS NOT NULL
      AND transaction_id IS NOT NULL AND package_digest IS NOT NULL
      AND package_bytes IS NOT NULL
      AND publication_generation = base_generation + 2
      AND ((base_generation=0 AND base_token IS NULL)
        OR (base_generation>0 AND base_token IS NOT NULL))
      AND ((origin_kind='pg_commit' AND source_commit_seq > 0
            AND import_event_id IS NULL AND import_transition_index IS NULL
            AND import_receipt_digest IS NULL)
        OR (origin_kind='legacy_sto_import' AND source_commit_seq IS NULL
            AND import_event_id IS NOT NULL AND import_transition_index > 0
            AND import_receipt_digest IS NOT NULL)))
  ),
  ADD CONSTRAINT cmd2_source_publication_history_import_digest_check
    CHECK (import_receipt_digest IS NULL OR import_receipt_digest ~ '^[0-9a-f]{64}$');
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conrelid='cmd2_source_publication_current'::regclass
      AND conname='cmd2_source_publication_current_import_fk'
  ) THEN
    ALTER TABLE cmd2_source_publication_current ADD CONSTRAINT cmd2_source_publication_current_import_fk
      FOREIGN KEY (domain, import_event_id)
      REFERENCES cmd2_source_publication_import(domain, import_event_id) ON DELETE RESTRICT;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conrelid='cmd2_source_publication_history'::regclass
      AND conname='cmd2_source_publication_history_import_fk'
  ) THEN
    ALTER TABLE cmd2_source_publication_history ADD CONSTRAINT cmd2_source_publication_history_import_fk
      FOREIGN KEY (domain, import_event_id)
      REFERENCES cmd2_source_publication_import(domain, import_event_id) ON DELETE RESTRICT;
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS cmd2_source_publication_history_transaction_seek
  ON cmd2_source_publication_history(domain,transaction_id,publication_generation);

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

-- Exact byte-order cold source seeks. These support bounded keyset pages;
-- the complete schema digest binds their physical access contract.
CREATE INDEX IF NOT EXISTS cmd2_current_source_cold_seek
  ON cmd2_current(domain,prepare_id,member_slot);
CREATE INDEX IF NOT EXISTS cmd2_current_source_subject_c_seek
  ON cmd2_current(domain,subject COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_source_index_cold_seek
  ON cmd2_source_index(domain,kind COLLATE "C",token COLLATE "C");
-- Bounded successor checks address the semantic rows owned by one exact source
-- path. Keep path first so a revision never scans another member's identities.
CREATE INDEX IF NOT EXISTS cmd2_source_index_path_semantic_seek
  ON cmd2_source_index(domain,path COLLATE "C",kind COLLATE "C",token COLLATE "C");
CREATE INDEX IF NOT EXISTS cmd2_predicate_source_cold_seek
  ON cmd2_predicate(domain,owner,kind COLLATE "C",scope COLLATE "C",token COLLATE "C");

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

-- A retained source-inventory projection is an explicit as-of overlay. Its
-- source revision remains tied to immutable history, and effective_seq must
-- name an already committed log head rather than a future/arbitrary cut.
CREATE TABLE IF NOT EXISTS cmd2_source_inventory_projection (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  subject text NOT NULL,
  effective_seq bigint NOT NULL CHECK (effective_seq > 0),
  source_revision bigint NOT NULL CHECK (source_revision > 0),
  content_digest char(64) NOT NULL,
  definition_digest char(64) NOT NULL,
  inventory_projection bytea NOT NULL
    CHECK (octet_length(inventory_projection) <= 1048576),
  PRIMARY KEY (domain, subject, effective_seq),
  FOREIGN KEY (domain, subject, source_revision)
    REFERENCES cmd2_history(domain, subject, revision),
  FOREIGN KEY (domain, effective_seq)
    REFERENCES cmd2_log(domain, commit_seq)
);

CREATE TABLE IF NOT EXISTS cmd2_outbox (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  commit_seq bigint NOT NULL CHECK (commit_seq > 0),
  event_id text NOT NULL,
  enqueued_at timestamptz DEFAULT clock_timestamp(),
  PRIMARY KEY (domain, event_id),
  UNIQUE (domain, commit_seq)
);
-- Additive migration deliberately leaves pre-existing events NULL: their
-- enqueue age is unknown, not reconstructed from sequence or client time.
ALTER TABLE cmd2_outbox ADD COLUMN IF NOT EXISTS enqueued_at timestamptz;
ALTER TABLE cmd2_outbox ALTER COLUMN enqueued_at SET DEFAULT clock_timestamp();
ALTER TABLE cmd2_outbox
  DROP CONSTRAINT IF EXISTS cmd2_outbox_enqueued_at_finite;
ALTER TABLE cmd2_outbox
  ADD CONSTRAINT cmd2_outbox_enqueued_at_finite
    CHECK (enqueued_at IS NULL OR isfinite(enqueued_at));

-- Source navigation disclosure is a separate current-use policy from CMD
-- source rights. It is opt-in, versioned, and retained independently from
-- navigation projection readiness so a withdrawal fences the next read.
CREATE TABLE IF NOT EXISTS cmd2_source_navigation_policy_current (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  consumer_id text NOT NULL CHECK (length(consumer_id) BETWEEN 1 AND 256),
  policy_issuer_ref text NOT NULL CHECK (length(policy_issuer_ref) BETWEEN 1 AND 4096),
  policy_receipt_id text NOT NULL CHECK (length(policy_receipt_id) BETWEEN 1 AND 512),
  export_receipt_id text NOT NULL CHECK (length(export_receipt_id) BETWEEN 1 AND 512),
  model_receipt_id text NOT NULL CHECK (length(model_receipt_id) BETWEEN 1 AND 512),
  carrier_layer text NOT NULL CHECK (carrier_layer = 'tos_source_navigation_public_metadata_v1'),
  intended_use text NOT NULL CHECK (intended_use = 'read_only_public_metadata_navigation_v1'),
  model_abi text NOT NULL CHECK (length(model_abi) BETWEEN 1 AND 256),
  selection_profile text NOT NULL CHECK (length(selection_profile) BETWEEN 1 AND 256),
  policy_epoch bigint NOT NULL CHECK (policy_epoch > 0),
  withdrawal_generation bigint NOT NULL CHECK (withdrawal_generation >= 0),
  policy_version bigint NOT NULL CHECK (policy_version > 0),
  allowed boolean NOT NULL,
  receipt_sha256 char(64) NOT NULL CHECK (receipt_sha256 ~ '^[0-9a-f]{64}$'),
  owner_configuration_sha256 char(64) NOT NULL CHECK (owner_configuration_sha256 ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (domain, consumer_id)
);

CREATE TABLE IF NOT EXISTS cmd2_source_navigation_policy_history (
  domain text NOT NULL REFERENCES cmd2_domain(domain) ON DELETE RESTRICT,
  consumer_id text NOT NULL CHECK (length(consumer_id) BETWEEN 1 AND 256),
  policy_version bigint NOT NULL CHECK (policy_version > 0),
  command_id text NOT NULL CHECK (length(command_id) BETWEEN 1 AND 256),
  operation text NOT NULL CHECK (operation IN ('install', 'revoke')),
  request_sha256 char(64) NOT NULL CHECK (request_sha256 ~ '^[0-9a-f]{64}$'),
  policy_issuer_ref text NOT NULL CHECK (length(policy_issuer_ref) BETWEEN 1 AND 4096),
  policy_receipt_id text NOT NULL CHECK (length(policy_receipt_id) BETWEEN 1 AND 512),
  export_receipt_id text NOT NULL CHECK (length(export_receipt_id) BETWEEN 1 AND 512),
  model_receipt_id text NOT NULL CHECK (length(model_receipt_id) BETWEEN 1 AND 512),
  carrier_layer text NOT NULL CHECK (carrier_layer = 'tos_source_navigation_public_metadata_v1'),
  intended_use text NOT NULL CHECK (intended_use = 'read_only_public_metadata_navigation_v1'),
  model_abi text NOT NULL CHECK (length(model_abi) BETWEEN 1 AND 256),
  selection_profile text NOT NULL CHECK (length(selection_profile) BETWEEN 1 AND 256),
  policy_epoch bigint NOT NULL CHECK (policy_epoch > 0),
  withdrawal_generation bigint NOT NULL CHECK (withdrawal_generation >= 0),
  allowed boolean NOT NULL,
  receipt_sha256 char(64) NOT NULL CHECK (receipt_sha256 ~ '^[0-9a-f]{64}$'),
  owner_configuration_sha256 char(64) NOT NULL CHECK (owner_configuration_sha256 ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (domain, consumer_id, policy_version),
  UNIQUE (domain, consumer_id, command_id)
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
    'cmd2_current','cmd2_history','cmd2_receipt','cmd2_log','cmd2_outbox',
    'cmd2_source_index','cmd2_source_text_identity','cmd2_source_annotation_identity','cmd2_source_claim_dependency','cmd2_source_claim_navigation_reference','cmd2_source_item_file_assignment',
    'cmd2_source_inventory_projection','cmd2_source_publication_current',
    'cmd2_source_publication_history','cmd2_source_publication_import'
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

-- Navigation-use policy state is a separate owner fence, not source metadata:
-- policy install/withdrawal must not invalidate the selected source-generation
-- audit root. Keep truncation prohibited while the versioned writer serializes
-- changes through the normal audit-fence and domain locks.
DO $$
DECLARE table_name text;
BEGIN
  FOREACH table_name IN ARRAY ARRAY[
    'cmd2_source_navigation_policy_current','cmd2_source_navigation_policy_history'
  ] LOOP
    EXECUTE format(
      'CREATE OR REPLACE TRIGGER %I BEFORE TRUNCATE ON %I '
      || 'FOR EACH STATEMENT EXECUTE FUNCTION cmd2_refuse_audited_truncate()',
      'cmd2_refuse_' || table_name || '_truncate', table_name);
  END LOOP;
END $$;
