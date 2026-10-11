//! Stable owner-declared compatibility profiles for the private durable path.
//!
//! Bump these descriptors when their owned contracts change. Keep routine
//! implementation edits out of these identities.

/// Managed source-cohort compatibility. This describes the persisted source
/// admission/readset contract, not the Rust implementation that enforces it.
pub(super) const SOURCE_COMPATIBILITY_PROFILE_V1: &[u8] = br#"tos-managed-agent-source-compatibility-v1
carrier-profiles=tos.source-file.original-v1:1,tos.source-file.agent-create-v1:1,tos.source-file.agent-create-pg-v1:1,tos.source-file.revision-v1:1
semantic-index=exact-ToS-path;owner-derived-metadata,claim,event,anchor,form;per-kind-token-to-path;global-token-not-on-two-source-paths
agent-pg-unique=token-cross-kind-over-metadata-claim-event-anchor-form
agent-pg-dependency=tos_initial_agent_dependency_commitment_v2:2
predicates=owner-native-corpus-create-agent;unique-index-kind-token;range-source-home-C-collated-half-open-home-slash-to-home0;range-source-inventory-all;range-source-cohort-generation;range-source-dependencies-v1-v2
readset-json=top-level-array;exact-tag-namespace-key-version-digest-locator;absent-tag-namespace-key;generation-tag-kind-owner-scope-token-definition-version-generation
index-readset-json=sorted-array-of-kind-token-path-tuples
managed-original-readset=tos.managed-agent-original-reads-v1-object-with-reads-and-original
attempt-binding=registered-command-request-config-delta-source-epoch-readset-indexes-projections-profile-contract-rights-version-rule-version-schema-profile-job-fence
concurrency=read-committed;lock-audit-fence-first-then-attempt-for-update-then-domain-for-update;revalidate-exact-absent-and-predicate-generations;atomic-current-history-index-predicate-receipt-log-outbox-attempt
inventory-overlay=immutable-history-base;effective-seq-at-committed-log-cut;latest-overlay-at-or-after-source-commit-and-at-or-before-selected-cut;bind-source-revision-content-digest-definition-digest;never-rewrite-earlier-history
"#;

/// Mixed public-metadata source-cohort compatibility. This is deliberately a
/// new profile: prior Agent-only projections and their source-definition
/// digests cannot be read as complete mixed-source evidence.
pub(super) const SOURCE_COMPATIBILITY_PROFILE_V2: &[u8] = br#"tos-managed-mixed-public-metadata-source-compatibility-v2
carrier-profiles=tos.source-file.original-v1:1,tos.source-file.agent-create-v1:1,tos.source-file.agent-create-pg-v1:1,tos.source-file.revision-v1:1
semantic-index=exact-ToS-path;owner-derived-mixed-public-metadata,claim,event,anchor,form;per-kind-token-to-path;global-token-not-on-two-source-paths
mixed-projection=tos_managed_mixed_source_inventory_member_v1;all-selected-public-record-profiles-by-owner;exact-schema-and-registry-input-digests;Claims-private-native-text-remain-refused
agent-pg-unique=token-cross-kind-over-metadata-claim-event-anchor-form
agent-pg-dependency=tos_initial_mixed_agent_dependency_commitment_v3:3;raw-configuration-sha256+canonical-Agent-dependency-template-v1;template-binds-selected-Agent-profile-schema-map-and-creation-contract/implementation-inputs;excludes-existing-Agent-record-event-anchor-form-payloads
predicates=owner-native-corpus-create-agent;unique-index-kind-token;range-source-home-C-collated-half-open-home-slash-to-home0;range-source-inventory-all;range-source-cohort-generation;range-source-dependencies-v1-v2
readset-json=top-level-array;exact-tag-namespace-key-version-digest-locator;absent-tag-namespace-key;generation-tag-kind-owner-scope-token-definition-version-generation
index-readset-json=sorted-array-of-kind-token-path-tuples
managed-original-readset=tos.managed-agent-original-reads-v1-object-with-reads-and-original
attempt-binding=registered-command-request-config-delta-source-epoch-readset-indexes-projections-profile-contract-rights-version-rule-version-schema-profile-job-fence
concurrency=read-committed;lock-audit-fence-first-then-attempt-for-update-then-domain-for-update;revalidate-exact-absent-and-predicate-generations;atomic-current-history-index-predicate-receipt-log-outbox-attempt
inventory-overlay=immutable-history-base;effective-seq-at-committed-log-cut;latest-overlay-at-or-after-source-commit-and-at-or-before-selected-cut;bind-source-revision-content-digest-definition-digest;never-rewrite-earlier-history
"#;

/// PostgreSQL 16 durable coordinator schema ABI and persisted audit IDs.
/// `durable_schema.sql` remains the DDL initializer; this version is advanced
/// by the owner when that physical/semantic contract changes.
pub(super) const DURABLE_SCHEMA_ABI_PROFILE_V1: &[u8] = br#"cmd2-coordinator-schema-abi-v1
postgres-major=16
durable-tables=cmd2_domain,cmd2_audit_fence,cmd2_job,cmd2_predicate,cmd2_attempt,cmd2_source_index,cmd2_member,cmd2_current,cmd2_history,cmd2_receipt,cmd2_log,cmd2_source_inventory_projection,cmd2_outbox
audited-metadata-tables=cmd2_job,cmd2_predicate,cmd2_attempt,cmd2_member,cmd2_current,cmd2_history,cmd2_receipt,cmd2_log,cmd2_outbox,cmd2_source_index,cmd2_source_inventory_projection
audited-table-triggers=cmd2_audit_<table>:after-insert-update-delete;cmd2_refuse_<table>_truncate:before-truncate;cmd2_audit_domain_insert;cmd2_audit_domain_update;cmd2_refuse_domain_truncate;cmd2_fence_maintenance_transition;cmd2_invalidate_source_maintenance
audited-trigger-functions=cmd2_register_audit_domain,cmd2_bump_audit_fence,cmd2_refuse_audited_truncate,cmd2_fence_maintenance_transition,cmd2_invalidate_source_maintenance
source-seeks=current-cold-domain-prepare-id-member-slot;current-subject-domain-subject-C;history-primary-key-domain-subject-revision;source-index-unique-domain-kind-token;source-index-seek-domain-path-C-kind-C-token-C;predicate-primary-key-domain-kind-owner-scope-token;predicate-seek-domain-owner-kind-C-scope-C-token-C
lab-carrier=cmd2.lab.embedded-revision:1:CMD2LAB1
cold-membership=cmd2-history-key-v1:tag,u32be-domain-len,domain,u32be-subject-len,subject,u64be-revision;cmd2-current-key-v1:tag,u32be-domain-len,domain,u32be-subject-len,subject
cold-namespaces=cmd2.history.v1,cmd2.current.v1
"#;
