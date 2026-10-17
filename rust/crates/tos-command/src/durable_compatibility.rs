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
predicates=owner-native-corpus-create-agent;unique-index-kind-token;unique-text-identity-token;range-source-home-C-collated-half-open-home-slash-to-home0;range-source-inventory-all;range-source-cohort-generation;range-source-dependencies-v1-v2
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

/// Mixed public-metadata source-cohort compatibility with explicit owner
/// contributions for Global Claims and public native Text. The v1 mixed
/// member wire cannot be read as this complete shape.
pub(super) const SOURCE_COMPATIBILITY_PROFILE_V3: &[u8] = br#"tos-managed-mixed-public-metadata-source-compatibility-v3
carrier-profiles=tos.source-file.original-v1:1,tos.source-file.agent-create-v1:1,tos.source-file.agent-create-pg-v1:1,tos.source-file.revision-v1:1
semantic-index=exact-ToS-path;owner-derived-mixed-public-metadata,claim,event,anchor,form;per-kind-token-to-path;global-token-not-on-two-source-paths
mixed-projection=tos_managed_mixed_source_inventory_member_v2;all-selected-public-record-profiles-by-owner;exact-schema-and-registry-input-digests;typed-claim-owner-and-public-text-owner-contributions;private-and-historical-owners-remain-refused
text-identity-completeness=cmd2_source_text_identity;public-ToS-candidate-boundary-excludes-source-witnesses-owner-local;full-current-candidate-rebuild-before-source_complete-during-bootstrap;normal-cold-reopen-verifies-the-exact-current-candidate-rowset-and-refuses-mismatch;rebuild-only-during-bootstrap-or-explicitly-authorized-reindex;future-member-replacements-update-contributions-and-bump-removed-or-added-token-generations-atomically;point-reads-require-current-source-definition-source_complete-audit-generation-and-exact-cmd2_current-member-digest;absent-token-generation-zero-is-observed-under-the-same-source-fence
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

/// Mixed public source cohort with the typed reverse Claim topology index
/// used by bounded Item adoption. Earlier source definitions do not prove
/// this relation projection is complete.
pub(super) const SOURCE_COMPATIBILITY_PROFILE_V4: &[u8] = br#"tos-managed-mixed-public-metadata-source-compatibility-v4
carrier-profiles=tos.source-file.original-v1:1,tos.source-file.agent-create-v1:1,tos.source-file.agent-create-pg-v1:1,tos.source-file.revision-v1:1
semantic-index=exact-ToS-path;owner-derived-mixed-public-metadata,claim,event,anchor,form;per-kind-token-to-path;global-token-not-on-two-source-paths
mixed-projection=tos_managed_mixed_source_inventory_member_v2;all-selected-public-record-profiles-by-owner;exact-schema-and-registry-input-digests;typed-claim-owner-and-public-text-owner-contributions;private-and-historical-owners-remain-refused
text-identity-completeness=cmd2_source_text_identity;public-ToS-candidate-boundary-excludes-source-witnesses-owner-local;full-current-candidate-rebuild-before-source_complete-during-bootstrap;normal-cold-reopen-verifies-the-exact-current-candidate-rowset-and-refuses-mismatch;rebuild-only-during-bootstrap-or-explicitly-authorized-reindex;future-member-replacements-update-contributions-and-bump-removed-or-added-token-generations-atomically;point-reads-require-current-source-definition-source_complete-audit-generation-and-exact-cmd2_current-member-digest;absent-token-generation-zero-is-observed-under-the-same-source-fence
claim-dependency-completeness=cmd2_source_claim_dependency;exact-public-source-claims-streams;one-row-per-embodied_by-or-exemplified_by-Claim-with-both-endpoints;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset-and-current-cmd2_current-plus-claim-identity-digest;ordinary-replacements-atomically-replace-old-new-contributions-and-bump-both-subject-object-range-predicates;same-transaction-point-lookup-returns-rows-and-range-generation-under-audit-source-fences
agent-pg-unique=token-cross-kind-over-metadata-claim-event-anchor-form
agent-pg-dependency=tos_initial_mixed_agent_dependency_commitment_v3:3;raw-configuration-sha256+canonical-Agent-dependency-template-v1;template-binds-selected-Agent-profile-schema-map-and-creation-contract/implementation-inputs;excludes-existing-Agent-record-event-anchor-form-payloads
predicates=owner-native-corpus-create-agent;unique-index-kind-token;unique-text-identity-token;range-claim-dependency-v1;range-source-home-C-collated-half-open-home-slash-to-home0;range-source-inventory-all;range-source-cohort-generation;range-source-dependencies-v1-v2
readset-json=top-level-array;exact-tag-namespace-key-version-digest-locator;absent-tag-namespace-key;generation-tag-kind-owner-scope-token-definition-version-generation
index-readset-json=sorted-array-of-kind-token-path-tuples
managed-original-readset=tos.managed-agent-original-reads-v1-object-with-reads-and-original
attempt-binding=registered-command-request-config-delta-source-epoch-readset-indexes-projections-profile-contract-rights-version-rule-version-schema-profile-job-fence
concurrency=read-committed;lock-audit-fence-first-then-attempt-for-update-then-domain-for-update;revalidate-exact-absent-and-predicate-generations;atomic-current-history-index-predicate-receipt-log-outbox-attempt
inventory-overlay=immutable-history-base;effective-seq-at-committed-log-cut;latest-overlay-at-or-after-source-commit-and-at-or-before-selected-cut;bind-source-revision-content-digest-definition-digest;never-rewrite-earlier-history
"#;

/// Mixed public source cohort with both typed Claim topology and Item File
/// assignment indexes. Earlier definitions do not prove either full index.
pub(super) const SOURCE_COMPATIBILITY_PROFILE_V5: &[u8] = br#"tos-managed-mixed-public-metadata-source-compatibility-v5
carrier-profiles=tos.source-file.original-v1:1,tos.source-file.agent-create-v1:1,tos.source-file.agent-create-pg-v1:1,tos.source-file.revision-v1:1
semantic-index=exact-ToS-path;owner-derived-mixed-public-metadata,claim,event,anchor,form;per-kind-token-to-path;global-token-not-on-two-source-paths
mixed-projection=tos_managed_mixed_source_inventory_member_v2;all-selected-public-record-profiles-by-owner;exact-schema-and-registry-input-digests;typed-claim-owner-and-public-text-owner-contributions;private-and-historical-owners-remain-refused
text-identity-completeness=cmd2_source_text_identity;public-ToS-candidate-boundary-excludes-source-witnesses-owner-local;full-current-candidate-rebuild-before-source_complete-during-bootstrap;normal-cold-reopen-exact-rowset-and-current-cmd2_current-plus-candidate-identity-digest;ordinary-replacements-atomically-replace-contributions-and-bump-affected-token-generations;same-transaction-point-read-returns-rows-and-range-generation-under-source-audit-fences
claim-dependency-completeness=cmd2_source_claim_dependency;exact-public-source-claims-streams;one-row-per-embodied_by-or-exemplified_by-Claim-with-both-endpoints;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset-and-current-cmd2_current-plus-claim-identity-digest;ordinary-replacements-atomically-replace-old-new-contributions-and-bump-both-subject-object-range-predicates;same-transaction-point-lookup-returns-rows-and-range-generation-under-audit-source-fences
item-file-assignment-completeness=cmd2_source_item_file_assignment;exact-current-public-item-manifests-and-payload-ordinals;preserve-multiplicity-of-file-id-assignments;rights-id-and-rights-member-digest-per-current-manifest;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset-and-current-cmd2_current-plus-item-identity-and-rights-digest;ordinary-replacements-atomically-replace-old-new-manifest-contributions-and-bump-affected-file-id-and-rights-id-range-predicates;same-transaction-point-lookup-returns-rows-and-range-generation-under-audit-source-fences;payload-bytes-stay-in-protected-custody
agent-pg-unique=token-cross-kind-over-metadata-claim-event-anchor-form
agent-pg-dependency=tos_initial_mixed_agent_dependency_commitment_v3:3;raw-configuration-sha256+canonical-Agent-dependency-template-v1;template-binds-selected-Agent-profile-schema-map-and-creation-contract/implementation-inputs;excludes-existing-Agent-record-event-anchor-form-payloads
predicates=owner-native-corpus-create-agent;unique-index-kind-token;unique-text-identity-token;range-claim-dependency-v1;range-item-file-v1;range-item-rights-v1;range-source-home-C-collated-half-open-home-slash-to-home0;range-source-inventory-all;range-source-cohort-generation;range-source-dependencies-v1-v2
readset-json=top-level-array;exact-tag-namespace-key-version-digest-locator;absent-tag-namespace-key;generation-tag-kind-owner-scope-token-definition-version-generation
index-readset-json=sorted-array-of-kind-token-path-tuples
managed-original-readset=tos.managed-agent-original-reads-v1-object-with-reads-and-original
attempt-binding=registered-command-request-config-delta-source-epoch-readset-indexes-projections-profile-contract-rights-version-rule-version-schema-profile-job-fence
concurrency=read-committed;lock-audit-fence-first-then-attempt-for-update-then-domain-for-update;revalidate-exact-absent-and-predicate-generations;atomic-current-history-index-predicate-receipt-log-outbox-attempt
inventory-overlay=immutable-history-base;effective-seq-at-committed-log-cut;latest-overlay-at-or-after-source-commit-and-at-or-before-selected-cut;bind-source-revision-content-digest-definition-digest;never-rewrite-earlier-history
"#;

/// Mixed public source cohort with a complete, separately unique Sign
/// promotion-candidate index. The exact source index/projection contract is
/// part of this inner profile; V5 domains must rebootstrap before use.
pub(super) const SOURCE_COMPATIBILITY_PROFILE_V6: &[u8] = br#"tos-managed-mixed-public-metadata-source-compatibility-v6
carrier-profiles=tos.source-file.original-v1:1,tos.source-file.agent-create-v1:1,tos.source-file.agent-create-pg-v1:1,tos.source-file.revision-v1:1
semantic-index=exact-ToS-path;owner-derived-mixed-public-metadata,claim,event,anchor,form,sign-candidate;exact-item-adoption-family-metadata-claim-event-form-package;per-kind-token-to-path;sign-candidate-is-a-separate-kind-and-does-not-collide-cross-kind-with-claim
mixed-projection=tos_managed_mixed_source_inventory_member_v2;all-selected-public-record-profiles-by-owner;exact-schema-and-registry-input-digests;typed-claim-owner-public-text-owner-and-sign-record-contributions;private-and-historical-owners-remain-refused
text-identity-completeness=cmd2_source_text_identity;public-candidate-paths-only;exclude-source-witnesses-owner-local-subtree;full-current-candidate-rebuild-before-source_complete;normal-cold-reopen-exact-current-candidate-rowset;ordinary-replacements-atomically-replace-contributions-and-bump-added-removed-token-generations;point-reads-fenced-by-source-definition-source_complete-audit-generation-and-current-member-digest
claim-dependency-completeness=cmd2_source_claim_dependency;typed-current-Claim-endpoint-rows;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-replacements-atomically-replace-old-new-contributions-and-bump-both-endpoint-range-predicates;same-transaction-point-lookup-under-source-audit-fences
item-file-assignment-completeness=cmd2_source_item_file_assignment;typed-current-Item-manifest-File-assignments;path-and-payload-ordinal-preserve-multiplicity;rights-ID-assignment-is-current-and-manifest-bound;exact-current-manifest-rights-and-Item-identity-bound;File-ID-and-rights-ID-Range-generation-fenced;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-replacements-atomically-replace-old-new-contributions
item-edition-assignment-completeness=cmd2_source_item_file_assignment.edition_id;one-current-Item-manifest-per-Item-and-Edition;exact-current-manifest-and-Item-identity-bound;Edition-ID-Range-generation-fenced-even-for-empty-lookups;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-replacements-atomically-replace-old-new-contributions
sign-candidate-completeness=cmd2_source_index-kind-sign-candidate;token=exact-tos.claim.*-promotion_basis.candidate.id;path=current-sign.json;one-current-Sign-owner-per-token;separate-per-kind-Unique-predicate-including-absence;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-creation-inserts-index-and-bumps-token-generation-atomically
agent-pg-unique=token-cross-kind-over-metadata-claim-event-anchor-form;sign-candidate-is-intentionally-excluded-from-cross-kind-collision
agent-pg-dependency=tos_initial_mixed_agent_dependency_commitment_v3:3;raw-configuration-sha256+canonical-Agent-dependency-template-v1;template-binds-selected-Agent-profile-schema-map-and-creation-contract/implementation-inputs;excludes-existing-Agent-record-event-anchor-form-payloads
predicates=owner-native-corpus-create-agent;unique-index-kind-token;unique-text-identity-token;unique-sign-candidate-tos.claim-id;range-claim-dependency-v1;range-item-edition-v1;range-item-file-v1;range-item-rights-v1;range-source-home-C-collated-half-open-home-slash-to-home0;range-source-inventory-all;range-source-cohort-generation;range-source-dependencies-v1-v2
readset-json=top-level-array;exact-tag-namespace-key-version-digest-locator;absent-tag-namespace-key;generation-tag-kind-owner-scope-token-definition-version-generation
index-readset-json=sorted-array-of-kind-token-path-tuples
managed-original-readset=tos.managed-agent-original-reads-v1-object-with-reads-and-original
attempt-binding=registered-command-request-config-delta-source-epoch-readset-indexes-projections-profile-contract-rights-version-rule-version-schema-profile-job-fence
concurrency=read-committed;lock-audit-fence-first-then-attempt-for-update-then-domain-for-update;revalidate-exact-absent-and-predicate-generations;atomic-current-history-index-predicate-receipt-log-outbox-attempt
inventory-overlay=immutable-history-base;effective-seq-at-committed-log-cut;latest-overlay-at-or-after-source-commit-and-at-or-before-selected-cut;bind-source-revision-content-digest-definition-digest;never-rewrite-earlier-history
selected-publication=separate-current-pointer;truthful-legacy-v1-v4-import-at-bootstrap-completion;imported-transition-generations-and-tokens-preserved;no-synthetic-pg-sequence;canonical-chain-receipt-and-exact-terminal-parent
"#;

/// Adds typed public HumanForms and Historical Claim revision packages to the
/// mixed source writer. V6 remains byte-stable so existing V6 cohorts refuse
/// explicitly and cold-bootstrap the required owner projections.
pub(super) const SOURCE_COMPATIBILITY_PROFILE_V7: &[u8] = br#"tos-managed-mixed-public-metadata-source-compatibility-v7
carrier-profiles=tos.source-file.original-v1:1,tos.source-file.agent-create-v1:1,tos.source-file.agent-create-pg-v1:1,tos.source-file.revision-v1:1
semantic-index=exact-ToS-path;owner-derived-mixed-public-metadata,claim,event,anchor,form,sign-candidate;exact-item-adoption-family-metadata-claim-event-form-package;typed-HumanForms-successor-form-IDs-only;sign-candidate-is-a-separate-kind-and-does-not-collide-cross-kind-with-claim
mixed-projection=tos_managed_mixed_source_inventory_member_v2;all-selected-public-record-profiles-by-owner;exact-schema-and-registry-input-digests;typed-claim-owner-public-text-owner-sign-record-owner-and-historical-claim-owner-contributions;private-owner-data-remains-refused;claim-archive-members-are-digest-only-and-bound-to-ordered-history
forms-historical-writer=exact-family-tag-owner-schema-operation;bounded-owner-configuration-retained-with-raw-and-canonical-digests;complete-exact-source-read-closure-retained-through-STO-history;recovery-rederives-owner-projection-and-new-form-index-rows-from-predecessor-and-successor;owner-current-rights-and-source-fences-remain-required
text-identity-completeness=cmd2_source_text_identity;public-candidate-paths-only;exclude-source-witnesses-owner-local-subtree;full-current-candidate-rebuild-before-source_complete;normal-cold-reopen-exact-current-candidate-rowset;ordinary-replacements-atomically-replace-contributions-and-bump-added-removed-token-generations;point-reads-fenced-by-source-definition-source_complete-audit-generation-and-current-member-digest
claim-dependency-completeness=cmd2_source_claim_dependency;typed-current-Claim-endpoint-rows;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-replacements-atomically-replace-old-new-contributions-and-bump-both-endpoint-range-predicates;same-transaction-point-lookup-under-source-audit-fences
item-file-assignment-completeness=cmd2_source_item_file_assignment;typed-current-Item-manifest-File-assignments;path-and-payload-ordinal-preserve-multiplicity;rights-ID-assignment-is-current-and-manifest-bound;exact-current-manifest-rights-and-Item-identity-bound;File-ID-and-rights-ID-Range-generation-fenced;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-replacements-atomically-replace-old-new-contributions
item-edition-assignment-completeness=cmd2_source_item_file_assignment.edition_id;one-current-Item-manifest-per-Item-and-Edition;exact-current-manifest-and-Item-identity-bound;Edition-ID-Range-generation-fenced-even-for-empty-lookups;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-replacements-atomically-replace-old-new-contributions
sign-candidate-completeness=cmd2_source_index-kind-sign-candidate;token=exact-tos.claim.*-promotion_basis.candidate.id;path=current-sign.json;one-current-Sign-owner-per-token;separate-per-kind-Unique-predicate-including-absence;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-creation-inserts-index-and-bumps-token-generation-atomically
agent-pg-unique=token-cross-kind-over-metadata-claim-event-anchor-form;sign-candidate-is-intentionally-excluded-from-cross-kind-collision
agent-pg-dependency=tos_initial_mixed_agent_dependency_commitment_v3:3;raw-configuration-sha256+canonical-Agent-dependency-template-v1;template-binds-selected-Agent-profile-schema-map-and-creation-contract/implementation-inputs;excludes-existing-Agent-record-event-anchor-form-payloads
predicates=owner-native-corpus-create-agent;unique-index-kind-token;unique-text-identity-token;unique-sign-candidate-tos.claim-id;range-claim-dependency-v1;range-item-edition-v1;range-item-file-v1;range-item-rights-v1;range-source-home-C-collated-half-open-home-slash-to-home0;range-source-inventory-all;range-source-cohort-generation;range-source-dependencies-v1-v2
readset-json=top-level-array;exact-tag-namespace-key-version-digest-locator;absent-tag-namespace-key;generation-tag-kind-owner-scope-token-definition-version-generation
index-readset-json=sorted-array-of-kind-token-path-tuples
managed-original-readset=tos.managed-agent-original-reads-v1-object-with-reads-and-original
attempt-binding=registered-command-request-config-delta-source-epoch-readset-indexes-projections-profile-contract-rights-version-rule-version-schema-profile-job-fence;Forms-Historical-v3-source-read-envelope-binds-exact-family-owner-config-request-and-source-locators
concurrency=read-committed;lock-audit-fence-first-then-attempt-for-update-then-domain-for-update;revalidate-exact-absent-and-predicate-generations;atomic-current-history-index-predicate-receipt-log-outbox-attempt
inventory-overlay=immutable-history-base;effective-seq-at-committed-log-cut;latest-overlay-at-or-after-source-commit-and-at-or-before-selected-cut;bind-source-revision-content-digest-definition-digest;never-rewrite-earlier-history
selected-publication=separate-current-pointer;truthful-legacy-v1-v4-import-at-bootstrap-completion;imported-transition-generations-and-tokens-preserved;no-synthetic-pg-sequence;canonical-chain-receipt-and-exact-terminal-parent
"#;

/// Claim reverse-reference intervals and explicit as-of baseline.
pub(super) const SOURCE_COMPATIBILITY_PROFILE_V8: &[u8] = br#"tos-managed-mixed-public-metadata-source-compatibility-v8
carrier-profiles=tos.source-file.original-v1:1,tos.source-file.agent-create-v1:1,tos.source-file.agent-create-pg-v1:1,tos.source-file.revision-v1:1
semantic-index=exact-ToS-path;owner-derived-mixed-public-metadata,claim,event,anchor,form,sign-candidate;exact-item-adoption-family-metadata-claim-event-form-package;typed-HumanForms-successor-form-IDs-only;sign-candidate-is-a-separate-kind-and-does-not-collide-cross-kind-with-claim
mixed-projection=tos_managed_mixed_source_inventory_member_v2;all-selected-public-record-profiles-by-owner;exact-schema-and-registry-input-digests;typed-claim-owner-public-text-owner-sign-record-owner-and-historical-claim-owner-contributions;private-owner-data-remains-refused;claim-archive-members-are-digest-only-and-bound-to-ordered-history
forms-historical-writer=exact-family-tag-owner-schema-operation;bounded-owner-configuration-retained-with-raw-and-canonical-digests;complete-exact-source-read-closure-retained-through-STO-history;recovery-rederives-owner-projection-and-new-form-index-rows-from-predecessor-and-successor;owner-current-rights-and-source-fences-remain-required
claim-navigation-interval-completeness=cmd2_source_claim_navigation_reference;half-open-valid_from_seq-valid_to_seq;explicit-bootstrap-baseline-no-fabricated-prebaseline-history;as-of-source-history-and-exact-member-bytes;active-current-read-filter;endpoint-and-completeness-predicate-generations-fenced;bounded-rebuild-only-at-declared-bootstrap-cut;cold-verifies-current-rowset-interval-shapes-nonoverlap-and-baseline;public-ToS-paths-only
text-identity-completeness=cmd2_source_text_identity;public-candidate-paths-only;exclude-source-witnesses-owner-local-subtree;full-current-candidate-rebuild-before-source_complete;normal-cold-reopen-exact-current-candidate-rowset;ordinary-replacements-atomically-replace-contributions-and-bump-added-removed-token-generations;point-reads-fenced-by-source-definition-source_complete-audit-generation-and-current-member-digest
claim-dependency-completeness=cmd2_source_claim_dependency;typed-current-Claim-endpoint-rows;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-replacements-atomically-replace-old-new-contributions-and-bump-both-endpoint-range-predicates;same-transaction-point-lookup-under-source-audit-fences
item-file-assignment-completeness=cmd2_source_item_file_assignment;typed-current-Item-manifest-File-assignments;path-and-payload-ordinal-preserve-multiplicity;rights-ID-assignment-is-current-and-manifest-bound;exact-current-manifest-rights-and-Item-identity-bound;File-ID-and-rights-ID-Range-generation-fenced;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-replacements-atomically-replace-old-new-contributions
item-edition-assignment-completeness=cmd2_source_item_file_assignment.edition_id;one-current-Item-manifest-per-Item-and-Edition;exact-current-manifest-and-Item-identity-bound;Edition-ID-Range-generation-fenced-even-for-empty-lookups;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-replacements-atomically-replace-old-new-contributions
sign-candidate-completeness=cmd2_source_index-kind-sign-candidate;token=exact-tos.claim.*-promotion_basis.candidate.id;path=current-sign.json;one-current-Sign-owner-per-token;separate-per-kind-Unique-predicate-including-absence;bounded-full-rebuild-before-source_complete;normal-cold-reopen-exact-rowset;ordinary-creation-inserts-index-and-bumps-token-generation-atomically
agent-pg-unique=token-cross-kind-over-metadata-claim-event-anchor-form;sign-candidate-is-intentionally-excluded-from-cross-kind-collision
agent-pg-dependency=tos_initial_mixed_agent_dependency_commitment_v3:3;raw-configuration-sha256+canonical-Agent-dependency-template-v1;template-binds-selected-Agent-profile-schema-map-and-creation-contract/implementation-inputs;excludes-existing-Agent-record-event-anchor-form-payloads
predicates=owner-native-corpus-create-agent;unique-index-kind-token;unique-text-identity-token;unique-sign-candidate-tos.claim-id;range-claim-dependency-v1;range-claim-navigation-reference-v2;range-item-edition-v1;range-item-file-v1;range-item-rights-v1;range-source-home-C-collated-half-open-home-slash-to-home0;range-source-inventory-all;range-source-cohort-generation;range-source-dependencies-v1-v2
readset-json=top-level-array;exact-tag-namespace-key-version-digest-locator;absent-tag-namespace-key;generation-tag-kind-owner-scope-token-definition-version-generation
index-readset-json=sorted-array-of-kind-token-path-tuples
managed-original-readset=tos.managed-agent-original-reads-v1-object-with-reads-and-original
attempt-binding=registered-command-request-config-delta-source-epoch-readset-indexes-projections-profile-contract-rights-version-rule-version-schema-profile-job-fence;Forms-Historical-v3-source-read-envelope-binds-exact-family-owner-config-request-and-source-locators
concurrency=read-committed;lock-audit-fence-first-then-attempt-for-update-then-domain-for-update;revalidate-exact-absent-and-predicate-generations;atomic-current-history-index-predicate-receipt-log-outbox-attempt
inventory-overlay=immutable-history-base;effective-seq-at-committed-log-cut;latest-overlay-at-or-after-source-commit-and-at-or-before-selected-cut;bind-source-revision-content-digest-definition-digest;never-rewrite-earlier-history
selected-publication=separate-current-pointer;truthful-legacy-v1-v4-import-at-bootstrap-completion;imported-transition-generations-and-tokens-preserved;no-synthetic-pg-sequence;canonical-chain-receipt-and-exact-terminal-parent
"#;

/// Claim-navigation interval table key, indexes, and baseline schema successor.
pub(super) const DURABLE_SCHEMA_ABI_PROFILE_V7: &[u8] = br#"cmd2-coordinator-schema-abi-v7
postgres-major=16
durable-tables=cmd2_domain,cmd2_audit_fence,cmd2_job,cmd2_predicate,cmd2_attempt,cmd2_source_index,cmd2_source_text_identity,cmd2_source_annotation_identity,cmd2_source_claim_dependency,cmd2_source_claim_navigation_reference,cmd2_source_item_file_assignment,cmd2_source_publication_current,cmd2_source_publication_history,cmd2_source_publication_import,cmd2_member,cmd2_current,cmd2_history,cmd2_receipt,cmd2_log,cmd2_source_inventory_projection,cmd2_outbox,cmd2_source_navigation_policy_current,cmd2_source_navigation_policy_history
audited-metadata-tables=cmd2_job,cmd2_predicate,cmd2_attempt,cmd2_member,cmd2_current,cmd2_history,cmd2_receipt,cmd2_log,cmd2_outbox,cmd2_source_index,cmd2_source_inventory_projection,cmd2_source_text_identity,cmd2_source_annotation_identity,cmd2_source_claim_dependency,cmd2_source_claim_navigation_reference,cmd2_source_item_file_assignment,cmd2_source_publication_current,cmd2_source_publication_history,cmd2_source_publication_import
audited-table-triggers=cmd2_audit_<table>:after-insert-update-delete;cmd2_refuse_<table>_truncate:before-truncate;cmd2_audit_domain_insert;cmd2_audit_domain_update;cmd2_refuse_domain_truncate;cmd2_fence_maintenance_transition;cmd2_invalidate_source_maintenance
audited-trigger-functions=cmd2_register_audit_domain,cmd2_bump_audit_fence,cmd2_refuse_audited_truncate,cmd2_fence_maintenance_transition,cmd2_invalidate_source_maintenance
source-seeks=current-cold-domain-prepare-id-member-slot;current-subject-domain-subject-C;history-primary-key-domain-subject-revision;source-index-unique-domain-kind-token;source-index-seek-domain-path-C-kind-C-token-C;predicate-primary-key-domain-kind-owner-scope-token;predicate-seek-domain-owner-kind-C-scope-C-token-C;annotation-identity-primary-domain-token-C-path-C;annotation-identity-path-domain-path-C-token-C;claim-dependency-primary-domain-claim-id;claim-dependency-subject-domain-predicate-subject-claim;claim-dependency-object-domain-predicate-object-claim;claim-dependency-path-domain-path-claim;claim-navigation-reference-interval-identity-domain-path-claim-kind-endpoint-basis-valid-from;claim-navigation-reference-active-unique-domain-path-claim-kind-endpoint-basis;claim-navigation-reference-active-seek-domain-kind-endpoint-path-claim;claim-navigation-reference-asof-domain-kind-endpoint-valid-from;claim-navigation-reference-path-domain-path-valid-from;item-file-assignment-primary-domain-manifest-ordinal;item-file-assignment-file-domain-file-manifest-ordinal;item-file-assignment-manifest-domain-manifest-ordinal;item-file-assignment-item-domain-item-manifest-ordinal;item-file-assignment-rights-domain-rights-manifest-ordinal;item-file-assignment-edition-domain-edition-manifest-ordinal;source-publication-history-domain-transaction-generation
lab-carrier=cmd2.lab.embedded-revision:1:CMD2LAB1
cold-membership=cmd2-history-key-v1:tag,u32be-domain-len,domain,u32be-subject-len,subject,u64be-revision;cmd2-current-key-v1:tag,u32be-domain-len,domain,u32be-subject-len,subject
cold-namespaces=cmd2.history.v1,cmd2.current.v1
source-navigation-policy=explicit-public-metadata-carrier-and-use;owner-receipt-bound-export-model-profile-epoch-withdrawal-generation;current-row-for-share;versioned-install-revoke-history;writer-lock-order=audit-fence-domain-policy-current;owner-fence-rechecked-before-commit;does-not-advance-source-audit-generation;separate-from-cmd-rights
source-text-identity=typed-public-text-token-set;multivalued-token-to-path;public-candidate-paths-only;exclude-ToS/source-witnesses/owner-local-subtree;C-collated-token-path-indexes;generation-bound-by-source-predicate
source-annotation-identity=typed-semantic-annotation-entity-IDs;multivalued-token-to-exact-packet-path;packet-member-and-definition-digests;owner-local-excluded;per-token-generation-fenced
source-claim-dependency=typed-embodied_by-and-exemplified_by-current-Claim-rows;both-subject-and-object-seek-paths;exact-current-member-and-Claim-identity-bound;Range-generation-fenced;audited-and-cold-verified
source-claim-navigation-reference=typed-Claim-value-owner-input-and-shared-schema-references;half-open-valid_from_seq-valid_to_seq-intervals;explicit-cmd2_domain-bootstrap-baseline;active-unique-identity;as-of-source-history-bound;audited-and-cold-verified
source-item-file-assignment=typed-current-Item-manifest-File-assignments;path-and-payload-ordinal-preserve-multiplicity;rights-ID-assignment-is-current-and-manifest-bound;exact-current-manifest-and-Item-identity-bound;File-ID-and-rights-ID-Range-generation-fenced;audited-and-cold-verified
source-publication=independent-selected-source-control-current-and-history;full-bootstrap-cut-establishes-absent-generation-zero-before-source_complete;publication-generation-advances-by-two;metadata-only-canonical-package-max-1048576-bytes;source-refs-bind-exact-predecessor-and-successor;unrelated-source-commits-do-not-move-publication-state
source-publication-import=legacy-v1-v4-ordered-chain-only;bootstrap-completion-is-the-import-event;original-generation-and-token-preserved;imported-source-commit-sequence-is-null;canonical-receipt-binds-bootstrap-cut-store-held-head-and-chain-root;first-pg-commit-extends-exact-import-terminal
outbox-enqueue-clock=cmd2_outbox.enqueued_at;nullable-timestamptz;server-clock_timestamp-default;legacy-null-age-unknown;exact-replay-preserves-original;utc-cold-row-encoding
outbox-oldest-event-seek=cmd2_outbox-unique-domain-commit-seq;query-domain-and-seq-range;ORDER-BY-commit_seq;LIMIT-1;verify-exact-next-seq
"#;

/// Dedicated owner-private package profile. Private package bytes, archives,
/// receipts, and audit generations remain outside the public CMD.2 source ABI.
pub(super) const PRIVATE_PACKAGE_ABI_PROFILE_V4: &str = "tos-private-owner-package-abi-v4";
