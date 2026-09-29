//! Whole initial Claim publication. Authored creation is already committed;
//! prepared errors require rollback of the caller's entire transaction.
//! Native execution/profile review remains separate from source admission.
use crate::{
    source_claim_publication_assembly as assembly, source_claim_publication_bytes as bytes,
    source_claim_publication_closure as closure,
    source_claim_publication_context::Context,
    source_claim_publication_dependencies::{self as deps, Declaration, Dependencies},
    source_claim_publication_normalize::{ClaimCandidateLimits, ClaimCandidateRegistries},
    source_claim_publication_roots::{MutationLimits, Roots, Snapshot},
    source_creation_store::CommittedClaimObservation,
};
use rusqlite::{Connection, Transaction};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};
use tos_compiler::{
    Error, QueryVocabulary, Result,
    knowledge_normalization::SourceRow,
    local_prepared::PublicationLimits,
    prepared_catalog_index::CatalogMaintenanceLimits,
    prepared_catalog_semantics::{CatalogInputs, SourceOrderProfile},
    prepared_semantic_index::SemanticMaintenanceLimits,
    prepared_source_binding::{
        PreparedSourceInputs, SourceMaintenanceReceipt,
        apply_source_bound_prepared_delta_transaction, read_prepared_source_inputs_transaction,
    },
    source_bibliographic::BibliographicLimits,
};
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, JsonValue, emit_value_preserved_json, parse_json,
};
use tos_validation::source_cut::CutWorkerSchemaExecutor;
const META: usize = 1_048_576;
fn typed(value: &Value) -> Result<JsonValue> {
    let raw = bytes::canonical(value, META)?;
    Ok(parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits::new(META, 128, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Claim typed JSON limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?
    .root()
    .clone())
}
fn view(value: &JsonValue) -> Result<Value> {
    let raw = emit_value_preserved_json(
        value,
        JsonLimits::new(META, 128, 1_000_000, 4096)
            .map_err(|_| Error::Budget("Claim detached view limits"))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    bytes::parse(&raw, META)
}
fn stable(value: &Value) -> Result<String> {
    SourceRow::parse(&bytes::canonical(value, META)?, META)?.stable_digest()
}
fn source_vector(mut value: Value, limits: PublicationLimits) -> Result<PreparedSourceInputs> {
    let roots = value["roots"]
        .as_object()
        .ok_or(Error::Invalid("Claim vector roots"))?;
    let digests: BTreeMap<String, Value> = roots
        .iter()
        .map(|(role, root)| (role.clone(), root["snapshot_sha256"].clone()))
        .collect();
    value["source_revision"] = json!(bytes::row_digest(
        &json!({"schema":"tos_agent_source_root_vector_v1","roots":digests,"source_publication":value["source_publication"],"dependencies":value["dependencies"]}),
        META
    )?);
    PreparedSourceInputs::parse(&bytes::canonical(&value, META)?, limits)
}
fn catalog_after(
    before: &CatalogInputs,
    source: &PreparedSourceInputs,
    normalization: &Value,
) -> Result<CatalogInputs> {
    let mut header = view(&before.header)?;
    header["source_revision"] = json!(source.source_revision());
    header["normalization_binding"] = normalization.clone();
    Ok(CatalogInputs {
        header: typed(&header)?,
        entity_registry: before.entity_registry.clone(),
        relation_registry: before.relation_registry.clone(),
        lenses: before.lenses.clone(),
        source_order_profile: SourceOrderProfile::SourceGraphId,
    })
}
/// Named native source identities. These identify the compiled source closure;
/// they do not attest source meaning, compatibility review or deployment.
#[derive(Clone, Debug)]
pub struct NativeClaimProfiles {
    pub dependency_implementation: String,
    pub declaration: String,
    pub agent_publication: String,
    pub claim_publication: String,
}
struct ExecutingImage {
    file: std::fs::File,
    identity: (u64, u64, u64, i64, i64, i64, i64),
    digest: String,
}
impl ExecutingImage {
    fn identity(metadata: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
        use std::os::unix::fs::MetadataExt;
        (
            metadata.dev(),
            metadata.ino(),
            metadata.len(),
            metadata.mtime(),
            metadata.mtime_nsec(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    }
    fn observe(deadline: Instant, cancelled: &AtomicBool) -> Result<Self> {
        use std::io::Read;
        let mut file = std::fs::File::open("/proc/self/exe")?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > 512 * 1024 * 1024 {
            return Err(Error::Budget("Claim executing image bytes"));
        }
        let identity = Self::identity(&metadata);
        let mut hash = tos_foundation::Digest256Hasher::new();
        let mut buffer = [0u8; 65536];
        let mut total = 0u64;
        loop {
            if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return Err(Error::Budget("Claim executing image deadline"));
            }
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            total = total
                .checked_add(n as u64)
                .filter(|n| *n <= 512 * 1024 * 1024)
                .ok_or(Error::Budget("Claim executing image cap"))?;
            hash.update(&buffer[..n]);
        }
        if total != metadata.len() || Self::identity(&file.metadata()?) != identity {
            return Err(Error::Invalid(
                "Claim executing image changed while hashing",
            ));
        }
        let this = Self {
            file,
            identity,
            digest: hash.finalize().to_hex(),
        };
        this.verify()?;
        Ok(this)
    }
    fn verify(&self) -> Result<()> {
        let current = std::fs::File::open("/proc/self/exe")?;
        if Self::identity(&current.metadata()?) != self.identity
            || Self::identity(&self.file.metadata()?) != self.identity
        {
            return Err(Error::Invalid("Claim current executing image changed"));
        }
        Ok(())
    }
}
fn profiles_for_image(digest: &str) -> Result<NativeClaimProfiles> {
    let profile = |schema: &str| {
        bytes::row_digest(
            &json!({"schema":schema,"executing_artifact_sha256":digest}),
            META,
        )
    };
    Ok(NativeClaimProfiles {
        dependency_implementation: profile("tos_native_prepared_source_dependencies_v1")?,
        declaration: profile("tos_native_bibliographic_dependency_enumerator_profile_v1")?,
        agent_publication: profile("tos_native_claim_source_assembly_profile_v1")?,
        claim_publication: profile("tos_native_initial_claim_publication_v1")?,
    })
}
/// Explicit compatibility review input. Old hashes are retained predecessor
/// evidence; only named fields transition to actual selected native identities.
#[derive(Clone, Debug)]
pub struct ReviewedClaimProfileTransition {
    pub dependency_implementation_before: String,
    pub declaration_before: String,
    pub agent_publication_before: String,
    pub claim_publication_before: Option<String>,
    pub normalization_before: Value,
    pub normalization_after: Value,
    pub native_normalization_processor_sha256: String,
    pub review_ref: String,
}
#[derive(Clone, Copy)]
pub struct ClaimPublicationLimits {
    pub max_nodes: usize,
    pub max_relations: usize,
    pub max_claims: usize,
    pub max_bytes: usize,
    pub max_row_bytes: usize,
    /// Assertion-context/source cohort bound; readable presentation retains its own 64-pointer limit.
    pub max_contexts: usize,
    pub max_vm_steps: u64,
    pub cow_target_bytes: usize,
}
impl Default for ClaimPublicationLimits {
    fn default() -> Self {
        Self {
            max_nodes: 4096,
            max_relations: 16384,
            max_claims: 512,
            max_bytes: 16_777_216,
            max_row_bytes: 8_388_608,
            max_contexts: 4096,
            max_vm_steps: 100_000_000,
            cow_target_bytes: 262_144,
        }
    }
}
impl ClaimPublicationLimits {
    fn validate(self) -> Result<()> {
        if self.max_nodes == 0
            || self.max_nodes > 4096
            || self.max_relations == 0
            || self.max_relations > 16384
            || self.max_claims == 0
            || self.max_claims > 512
            || self.max_bytes == 0
            || self.max_bytes > 16_777_216
            || self.max_row_bytes == 0
            || self.max_row_bytes > 8_388_608
            || self.max_contexts == 0
            || self.max_contexts > 4096
            || self.max_vm_steps < 100
            || self.max_vm_steps > 1_000_000_000
            || !(256..=8_388_608).contains(&self.cow_target_bytes)
        {
            return Err(Error::Budget("Claim whole operation limits"));
        }
        Ok(())
    }
}
/// Sole progress owner for this connection during the whole operation. Install
/// before BEGIN using unchecked_transaction; no subprocess runs inside the tx.
pub struct ClaimPublicationProgress<'a> {
    connection: &'a Connection,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
    steps: Arc<AtomicU64>,
    maximum: u64,
}
impl<'a> ClaimPublicationProgress<'a> {
    pub fn install(
        connection: &'a Connection,
        cancelled: Arc<AtomicBool>,
        deadline: Instant,
        max_vm_steps: u64,
    ) -> Result<Self> {
        if !(100..=1_000_000_000).contains(&max_vm_steps) || Instant::now() >= deadline {
            return Err(Error::Budget("Claim progress deadline/VM steps"));
        }
        let steps = Arc::new(AtomicU64::new(0));
        let count = steps.clone();
        let cancel = cancelled.clone();
        connection.progress_handler(
            100,
            Some(move || {
                count.fetch_add(100, Ordering::Relaxed) >= max_vm_steps
                    || cancel.load(Ordering::Relaxed)
                    || Instant::now() >= deadline
            }),
        );
        Ok(Self {
            connection,
            cancelled,
            deadline,
            steps,
            maximum: max_vm_steps,
        })
    }
    fn verify(&self, tx: &Transaction<'_>) -> Result<()> {
        if !std::ptr::eq(self.connection, &**tx)
            || self.cancelled.load(Ordering::Relaxed)
            || Instant::now() >= self.deadline
            || self.steps.load(Ordering::Relaxed) > self.maximum
        {
            return Err(Error::Budget("Claim caller progress owner"));
        }
        Ok(())
    }
}
impl Drop for ClaimPublicationProgress<'_> {
    fn drop(&mut self) {
        self.connection.progress_handler(0, None::<fn() -> bool>);
    }
}
struct Applied {
    connection: usize,
    total_changes: u64,
    schema: u64,
    source: PreparedSourceInputs,
    catalog: CatalogInputs,
    receipt: Value,
    publication_limits: PublicationLimits,
}
pub struct ClaimAdditionPublication {
    observer: CommittedClaimObservation,
    image: ExecutingImage,
    source: PreparedSourceInputs,
    binding: JsonValue,
    catalog: CatalogInputs,
    roots: Roots,
    raw: assembly::AssembledAddition,
    catalog_addition: assembly::CatalogAddition,
    profiles: NativeClaimProfiles,
    review: ReviewedClaimProfileTransition,
    vocabulary: QueryVocabulary,
    descriptor: Vec<u8>,
    entity: Vec<u8>,
    relation: Vec<u8>,
    limits: ClaimPublicationLimits,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    applied: Option<Applied>,
    attempted: bool,
}
impl ClaimAdditionPublication {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        owner_config: &Path,
        source: PreparedSourceInputs,
        binding: JsonValue,
        catalog: CatalogInputs,
        receipt_sha256: Digest256,
        request_digest: Digest256,
        review: ReviewedClaimProfileTransition,
        vocabulary: QueryVocabulary,
        descriptor: Vec<u8>,
        limits: ClaimPublicationLimits,
        bibliographic_limits: BibliographicLimits,
        worker: &mut CutWorkerSchemaExecutor,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self> {
        limits.validate()?;
        catalog.validate()?;
        if catalog.source_order_profile != SourceOrderProfile::SourceGraphId
            || review.review_ref.is_empty()
            || review.review_ref.len() > 4096
        {
            return Err(Error::Invalid(
                "Claim canonical predecessor/review reference",
            ));
        }
        for sha in [
            &review.dependency_implementation_before,
            &review.declaration_before,
            &review.agent_publication_before,
            &review.native_normalization_processor_sha256,
        ] {
            bytes::sha(sha)?;
        }
        if let Some(sha) = &review.claim_publication_before {
            bytes::sha(sha)?;
        }
        if review.normalization_after["processor_digest"]
            != review.native_normalization_processor_sha256
        {
            return Err(Error::Invalid("Claim native processor selection"));
        }
        let limits_p = PublicationLimits::default();
        let source_value = view(&source.value()?)?;
        if source_vector(source_value.clone(), limits_p)?.raw() != source.raw() {
            return Err(Error::Invalid("Claim source vector profile"));
        }
        let deps = &source_value["dependencies"];
        if deps["declaration-profile"] != review.declaration_before
            || deps["agent-publication-profile"] != review.agent_publication_before
            || deps
                .get("claim-publication-profile")
                .and_then(Value::as_str)
                != review.claim_publication_before.as_deref()
            || deps["normalization"] != stable(&review.normalization_before)?
            || view(&catalog.header)?["normalization_binding"] != review.normalization_before
            || deps["entity-registry"] != stable(&view(&catalog.entity_registry)?)?
            || deps["relation-registry"] != stable(&view(&catalog.relation_registry)?)?
            || deps.get("nonparticipating-profile").is_none()
        {
            return Err(Error::Invalid("Claim reviewed predecessor profiles"));
        }
        let mut snapshots = BTreeMap::new();
        for (role, root) in source.roots() {
            snapshots.insert(
                role.clone(),
                Snapshot::parse(
                    root.root_bytes.clone(),
                    root.namespace_path.clone().into(),
                    &root.snapshot_sha256,
                )?,
            );
        }
        for (role, schema, collections) in [
            (
                "source-navigation",
                "tos_agent_source_navigation_rows_v1",
                vec![("nodes", "node_id"), ("edges", "edge_id")],
            ),
            (
                "bibliographic-claims",
                "tos_agent_bibliographic_rows_v1",
                vec![
                    ("nodes", "node_id"),
                    ("edges", "edge_id"),
                    ("claim_traces", "claim_ref"),
                ],
            ),
        ] {
            let snapshot = snapshots
                .get(role)
                .ok_or(Error::Invalid("Claim required independent root"))?;
            if snapshot.manifest["header"] != json!({"schema_version":schema})
                || snapshot.manifest["collections"]
                    .as_object()
                    .map(|o| o.len())
                    != Some(collections.len())
            {
                return Err(Error::Invalid("Claim raw root profile"));
            }
            for (collection, key) in collections {
                let spec = &snapshot.manifest["collections"][collection];
                if spec["key_field"] != key || spec["order_fields"] != json!([key]) {
                    return Err(Error::Invalid("Claim raw collection key profile"));
                }
            }
        }
        let header = snapshots
            .get("source-catalog")
            .ok_or(Error::Invalid("Claim source catalog root"))?
            .manifest["header"]
            .clone();
        let mut roots = Roots::new(snapshots, MutationLimits::default())?;
        let deadline = bibliographic_limits.deadline;
        let mut observer = CommittedClaimObservation::select(
            owner_config,
            receipt_sha256,
            request_digest,
            deadline,
            &cancelled,
        )
        .map_err(|e| Error::Source(format!("{e:?}")))?;
        // This is the real selected schema/source-cut revision, independent
        // of the derived prepared source vector; it is no invented Cut proof.
        let revision = worker.source_revision();
        let (catalog_addition, raw) = assembly::assemble(
            &mut observer,
            &mut roots,
            "source-catalog",
            &header,
            revision,
            worker,
            bibliographic_limits,
            &cancelled,
        )?;
        let entity = emit_value_preserved_json(
            &catalog.entity_registry,
            JsonLimits::new(META, 128, 1_000_000, 4096)
                .map_err(|_| Error::Budget("Claim entity registry"))?,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        let relation = emit_value_preserved_json(
            &catalog.relation_registry,
            JsonLimits::new(META, 128, 1_000_000, 4096)
                .map_err(|_| Error::Budget("Claim relation registry"))?,
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        if descriptor.len() > META {
            return Err(Error::Budget("Claim descriptor bytes"));
        }
        let image = ExecutingImage::observe(deadline, &cancelled)?;
        if image.digest != review.native_normalization_processor_sha256 {
            return Err(Error::Invalid(
                "Claim reviewed processor differs from current executing image",
            ));
        }
        let mut expected_normalization = review.normalization_before.clone();
        expected_normalization["processor_digest"] = json!(image.digest);
        if expected_normalization != review.normalization_after {
            return Err(Error::Invalid(
                "Claim normalization transition changes configuration",
            ));
        }
        let profiles = profiles_for_image(&image.digest)?;
        Ok(Self {
            observer,
            image,
            source,
            binding,
            catalog,
            roots,
            raw,
            catalog_addition,
            profiles,
            review,
            vocabulary,
            descriptor,
            entity,
            relation,
            limits,
            deadline,
            cancelled,
            applied: None,
            attempted: false,
        })
    }
    fn verify_current(&self) -> Result<()> {
        self.image.verify()?;
        if self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.deadline {
            return Err(Error::Budget("Claim source scope deadline"));
        }
        self.observer
            .verify_current(self.deadline, &self.cancelled)
            .map_err(|e| Error::Source(format!("{e:?}")))
    }
    pub fn binding(&self) -> &JsonValue {
        &self.binding
    }
    pub fn result(&self) -> Result<Value> {
        self.applied
            .as_ref()
            .map(|a| a.receipt.clone())
            .ok_or(Error::Invalid("Claim not applied"))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn apply_transaction(
        &mut self,
        tx: &Transaction<'_>,
        progress: &ClaimPublicationProgress<'_>,
        publication_limits: PublicationLimits,
        catalog_limits: CatalogMaintenanceLimits,
        semantic_limits: SemanticMaintenanceLimits,
    ) -> Result<Value> {
        if self.attempted || self.applied.is_some() || tx.is_autocommit() {
            return Err(Error::Invalid("one active Claim caller transaction"));
        }
        if !Arc::ptr_eq(&progress.cancelled, &self.cancelled)
            || progress.deadline > self.deadline
            || progress.maximum > self.limits.max_vm_steps
        {
            return Err(Error::Invalid(
                "Claim progress scope differs from prepared operation",
            ));
        }
        self.attempted = true;
        progress.verify(tx)?;
        self.verify_current()?;
        let start = tx.total_changes();
        let mut dependency_limits = deps::Limits::default();
        dependency_limits.max_claims = self.limits.max_claims;
        if read_prepared_source_inputs_transaction(
            tx,
            &self.binding,
            &self.catalog,
            publication_limits,
        )?
        .raw()
            != self.source.raw()
        {
            return Err(Error::Invalid("Claim selected predecessor changed"));
        }
        let mut dependency = Dependencies::new(tx, dependency_limits)?;
        let before_binding = view(&self.binding)?;
        let state = dependency.state(
            &before_binding,
            self.source.digest(),
            &self.review.declaration_before,
            &self.review.dependency_implementation_before,
        )?;
        let mut context = Context::new(tx, dependency_limits);
        let context_state = context.state(&before_binding, self.source.digest())?;
        // This exact reviewed transition precedes any Claim declaration mutation.
        let mut native_value = view(&self.source.value()?)?;
        native_value["dependencies"]["declaration-profile"] = json!(self.profiles.declaration);
        native_value["dependencies"]["agent-publication-profile"] =
            json!(self.profiles.agent_publication);
        if self.review.claim_publication_before.is_some() {
            native_value["dependencies"]["claim-publication-profile"] =
                json!(self.profiles.claim_publication);
        }
        native_value["dependencies"]["normalization"] =
            json!(stable(&self.review.normalization_after)?);
        let native_source = source_vector(native_value, publication_limits)?;
        let native_catalog = catalog_after(
            &self.catalog,
            &native_source,
            &self.review.normalization_after,
        )?;
        let mut transition_limits = publication_limits;
        transition_limits.max_mutations = transition_limits
            .max_mutations
            .checked_sub(2)
            .ok_or(Error::Budget("Claim profile state reservations"))?;
        let mut reviewed_dependencies = BTreeMap::new();
        for (key, old, new) in [
            (
                "declaration-profile",
                self.review.declaration_before.clone(),
                self.profiles.declaration.clone(),
            ),
            (
                "agent-publication-profile",
                self.review.agent_publication_before.clone(),
                self.profiles.agent_publication.clone(),
            ),
            (
                "normalization",
                stable(&self.review.normalization_before)?,
                stable(&self.review.normalization_after)?,
            ),
        ] {
            if old != new {
                reviewed_dependencies.insert(key.to_owned(), (old, new));
            }
        }
        if let Some(old) = &self.review.claim_publication_before {
            if *old != self.profiles.claim_publication {
                reviewed_dependencies.insert(
                    "claim-publication-profile".to_owned(),
                    (old.clone(), self.profiles.claim_publication.clone()),
                );
            }
        }
        let reviewed = tos_compiler::local_prepared::ReviewedNormalizationTransition {
            before_processor: bytes::text(&self.review.normalization_before, "processor_digest")?,
            after_processor: &self.review.native_normalization_processor_sha256,
            review_ref: &self.review.review_ref,
        };
        let transition =
            tos_compiler::prepared_source_binding::transition_prepared_source_profiles_transaction(
                tx,
                &self.binding,
                &self.source,
                &native_source,
                &self.catalog,
                &native_catalog,
                &reviewed,
                &reviewed_dependencies,
                transition_limits,
                catalog_limits,
                semantic_limits,
            )?;
        let native_binding = view(&transition.publication.binding)?;
        dependency.rebind_reviewed(
            state,
            &native_binding,
            native_source.digest(),
            &self.profiles.declaration,
            &self.profiles.dependency_implementation,
        )?;
        context.rebind(context_state, &native_binding, native_source.digest())?;
        if !dependency.lookup("unresolved", &Value::Null)?.is_empty() {
            return Err(Error::Invalid("Claim unresolved dependency closure"));
        }
        let declarations: Vec<Declaration> = self
            .raw
            .declarations
            .iter()
            .map(|row| {
                Declaration::parse(
                    &row.raw,
                    dependency_limits.max_row,
                    dependency_limits.max_dependencies,
                )
            })
            .collect::<Result<_>>()?;
        for declaration in &declarations {
            dependency.require_absent(&declaration.id)?;
            for kind in ["claim", "identity"] {
                if !dependency.lookup(kind, &json!(declaration.id))?.is_empty() {
                    return Err(Error::Invalid(
                        "Claim predecessor depends on new Claim; broader closure required",
                    ));
                }
            }
        }
        let context_state = context.state(&native_binding, native_source.digest())?;
        let candidate_limits = ClaimCandidateLimits {
            max_nodes: self.limits.max_nodes,
            max_retained_nodes: self.limits.max_nodes,
            max_relations: self.limits.max_relations,
            max_traces: self.limits.max_claims,
            max_contexts: self.limits.max_contexts,
            max_row_bytes: self.limits.max_row_bytes,
            max_input_bytes: self.limits.max_bytes,
            max_output_bytes: self.limits.max_bytes,
        };
        let registries = ClaimCandidateRegistries {
            entity_bytes: &self.entity,
            relation_bytes: &self.relation,
            descriptor_bytes: &self.descriptor,
            vocabulary: &self.vocabulary,
            expected_normalization_binding: &self.review.normalization_after,
        };
        let closure = closure::normalize(
            tx,
            &mut self.roots,
            &self.raw,
            &context_state,
            candidate_limits,
            dependency_limits,
            &registries,
        )?;
        progress.verify(tx)?;
        let raw_reads = self.roots.accounting();
        let raw_root = self.roots.stage(
            "bibliographic-claims",
            closure.raw_changes,
            None,
            self.limits.cow_target_bytes,
        )?;
        let catalog_root = self.roots.stage(
            "source-catalog",
            std::mem::take(&mut self.catalog_addition.changes),
            Some(self.catalog_addition.header.clone()),
            self.limits.cow_target_bytes,
        )?;
        let mut after_value = view(&native_source.value()?)?;
        after_value["dependencies"]["claim-publication-profile"] =
            json!(self.profiles.claim_publication);
        for (role, root) in [
            ("bibliographic-claims", raw_root),
            ("source-catalog", catalog_root),
        ] {
            after_value["roots"][role] = json!({"namespace_path":root.path.to_str().ok_or(Error::Invalid("Claim successor namespace"))?,"root_json":std::str::from_utf8(&root.raw).map_err(|_|Error::Invalid("Claim successor root UTF8"))?,"snapshot_sha256":bytes::digest(&root.raw)});
        }
        let after_source = source_vector(after_value, publication_limits)?;
        let after_catalog = catalog_after(
            &native_catalog,
            &after_source,
            &self.review.normalization_after,
        )?;
        let state = dependency.state(
            &native_binding,
            native_source.digest(),
            &self.profiles.declaration,
            &self.profiles.dependency_implementation,
        )?;
        let finalizer = dependency.stage(
            state,
            &declarations,
            after_source.digest(),
            after_source.source_revision(),
        )?;
        let reserve = finalizer as u64 + 3 * closure.context_ids.len() as u64 + 1;
        let used = tx
            .total_changes()
            .checked_sub(start)
            .ok_or(Error::Invalid("Claim cumulative counter"))?;
        let mut paired_limits = publication_limits;
        paired_limits.max_mutations = publication_limits
            .max_mutations
            .checked_sub(used)
            .and_then(|n| n.checked_sub(reserve))
            .filter(|n| *n >= 2)
            .ok_or(Error::Budget("Claim cumulative finalizer reservation"))?;
        let changed_nodes = closure.changes.iter().filter(|c| c.kind == "node").count();
        let changed_relations = closure.changes.len() - changed_nodes;
        let paired = apply_source_bound_prepared_delta_transaction(
            tx,
            &transition.publication.binding,
            &native_source,
            &after_source,
            &native_catalog,
            &after_catalog,
            closure.changes.into_iter().map(Ok),
            paired_limits,
            catalog_limits,
            semantic_limits,
            &self.review.native_normalization_processor_sha256,
        )?;
        let final_binding = view(&paired.publication.binding)?;
        dependency.finalize(
            &final_binding,
            after_source.digest(),
            &self.profiles.declaration,
            &self.profiles.dependency_implementation,
        )?;
        context.append(
            context_state,
            &final_binding,
            after_source.digest(),
            &closure.context_ids,
            &closure.nodes,
        )?;
        context.state(&final_binding, after_source.digest())?;
        dependency.state(
            &final_binding,
            after_source.digest(),
            &self.profiles.declaration,
            &self.profiles.dependency_implementation,
        )?;
        let mutations = tx
            .total_changes()
            .checked_sub(start)
            .ok_or(Error::Invalid("Claim cumulative mutations"))?;
        if mutations > publication_limits.max_mutations {
            return Err(Error::Budget("Claim combined mutations; rollback required"));
        }
        progress.verify(tx)?;
        self.verify_current()?;
        let receipt = receipt(
            &paired,
            &declarations,
            closure.normalized_nodes,
            closure.normalized_relations,
            changed_nodes,
            changed_relations,
            mutations,
            raw_reads,
            self.observer.receipt(),
        )?;
        if bytes::canonical(&receipt, self.limits.max_bytes)?.len() > self.limits.max_bytes {
            return Err(Error::Budget("Claim receipt bytes"));
        }
        let schema = tx.query_row("PRAGMA main.schema_version", [], |r| r.get(0))?;
        self.applied = Some(Applied {
            connection: (&**tx as *const Connection) as usize,
            total_changes: tx.total_changes(),
            schema,
            source: after_source,
            catalog: after_catalog,
            receipt: receipt.clone(),
            publication_limits,
        });
        Ok(receipt)
    }
    pub fn commit_transaction(
        mut self,
        tx: Transaction<'_>,
        progress: &ClaimPublicationProgress<'_>,
    ) -> Result<Value> {
        progress.verify(&tx)?;
        let applied = self
            .applied
            .as_ref()
            .ok_or(Error::Invalid("Claim commit before apply"))?;
        let schema: u64 = tx.query_row("PRAGMA main.schema_version", [], |r| r.get(0))?;
        if applied.connection != (&*tx as *const Connection) as usize
            || tx.is_autocommit()
            || tx.total_changes() != applied.total_changes
            || schema != applied.schema
        {
            return Err(Error::Invalid(
                "exact applied Claim transaction changed; rollback required",
            ));
        }
        let binding = typed(&applied.receipt["binding"])?;
        if read_prepared_source_inputs_transaction(
            &tx,
            &binding,
            &applied.catalog,
            applied.publication_limits,
        )?
        .raw()
            != applied.source.raw()
        {
            return Err(Error::Invalid("Claim final paired selection changed"));
        }
        self.verify_current()?;
        progress.verify(&tx)?;
        tx.commit()?;
        let mut receipt = self.applied.take().unwrap().receipt;
        receipt["prepared_committed"] = json!(true);
        Ok(receipt)
    }
}
#[allow(clippy::too_many_arguments)]
fn receipt(
    paired: &SourceMaintenanceReceipt,
    declarations: &[Declaration],
    nodes: usize,
    relations: usize,
    changed_nodes: usize,
    changed_relations: usize,
    mutations: u64,
    raw_reads: Value,
    creation: &Value,
) -> Result<Value> {
    let optional = |v: &Option<JsonValue>| -> Result<Value> {
        v.as_ref()
            .map(view)
            .transpose()
            .map(|v| v.unwrap_or(Value::Null))
    };
    Ok(
        json!({"schema":"tos_initial_claim_publication_v1","binding":view(&paired.publication.binding)?,"source_header":optional(&paired.publication.source_header)?,"catalog":optional(&paired.publication.catalog)?,"semantic_report":optional(&paired.publication.semantic_report)?,"source_inputs_sha256":paired.source_inputs_sha256,"sql_mutations":mutations,"source_command_committed":true,"prepared_committed":false,"claim_ids":declarations.iter().map(|d|d.id.clone()).collect::<Vec<_>>(),"normalized_nodes":nodes,"normalized_relations":relations,"changed_nodes":changed_nodes,"changed_relations":changed_relations,"creation_request_digest":creation["request_digest"],"source_transition_verified":true,"complete_incidence_verified":true,"singleton_context_addition":true,"global_source_currentness_verified":false,"raw_projection_reads":raw_reads,"consumer_switched":false,"is_semantic_acceptance":false}),
    )
}
