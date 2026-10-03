-- Opt-in managed immutable model selection. This is not source admission.
-- Never attach the source audit trigger: model bookkeeping does not modify
-- source rows. Every current read separately holds the actual source fences.
-- Old source generations remain historical, never current by this row alone.
-- The pointer is not history: every successful CAS also inserts its immutable
-- same-cut manifest version below in the SAME transaction. Pin-aware retention
-- may retire history only after accounting all historical model references.
CREATE TABLE IF NOT EXISTS cmd2_model_selection_v2 (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  selected_generation_digest char(64) NOT NULL
    CHECK (selected_generation_digest ~ '^[0-9a-f]{64}$'),
  selected_audit_generation bigint NOT NULL CHECK (selected_audit_generation >= 0),
  schema_version smallint NOT NULL CHECK (schema_version = 2),
  model_version bigint NOT NULL CHECK (model_version > 0),
  manifest_digest char(64) NOT NULL CHECK (manifest_digest ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (domain, selected_generation_digest, selected_audit_generation)
);

CREATE TABLE IF NOT EXISTS cmd2_model_manifest_history_v2 (
  domain text NOT NULL REFERENCES cmd2_domain(domain),
  selected_generation_digest char(64) NOT NULL
    CHECK (selected_generation_digest ~ '^[0-9a-f]{64}$'),
  selected_audit_generation bigint NOT NULL CHECK (selected_audit_generation >= 0),
  schema_version smallint NOT NULL CHECK (schema_version = 2),
  model_version bigint NOT NULL CHECK (model_version > 0),
  manifest_digest char(64) NOT NULL CHECK (manifest_digest ~ '^[0-9a-f]{64}$'),
  PRIMARY KEY (domain, selected_generation_digest, selected_audit_generation, model_version)
);
