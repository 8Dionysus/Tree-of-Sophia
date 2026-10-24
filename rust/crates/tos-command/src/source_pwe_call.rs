//! One original SOURCE operation, shared by every PWE Claim in its batch.
//! This is preserve-only source mechanics, never a QualifiedAccount issuer.
use super::*;
use crate::pwe_source_owner::{
    SourcePayer, check_description_revision_source, extract_claim_dependencies_source,
    source_canonical,
};
use tos_compiler::pwe_native::{Limits, Session};

pub(super) struct SourceCall<'s, 'p, 'w> {
    session: &'s mut Session<'p>,
    payer: &'s SourcePayer<'w>,
    whole: &'w Rc<RefCell<ClaimCallBudget>>,
    registry: Option<(tos_foundation::SourceRevision, JsonValue)>,
}

/// The caller invokes this ONCE outside its complete source-operation loop.
/// All limits clip the actual remaining original allowance; none replenishes it.
pub(super) fn with_operation<T>(
    whole: &Rc<RefCell<ClaimCallBudget>>,
    deadline: Instant,
    cancelled: &AtomicBool,
    run: impl FnOnce(&mut SourceCall<'_, '_, '_>) -> SourceCommandResult<T>,
) -> SourceCommandResult<T> {
    let payer = SourcePayer::new(whole, deadline, cancelled)?;
    let (input, state) = {
        let original = whole.try_borrow().map_err(|_| {
            SourceCommandError::Invalid("PWE original source payer already borrowed")
        })?;
        (
            usize::try_from(original.remaining_read()?).map_err(|_| {
                SourceCommandError::Unsupported("PWE original source allowance conversion")
            })?,
            original.remaining_live()?,
        )
    };
    let mut session = Session::new_source(
        Limits {
            max_input_bytes: input,
            max_nodes: input.min(1_000_000),
            max_work: input,
            max_refs: 4096,
            max_depth: 96,
            max_items: 4096,
            max_state_bytes: state,
            deadline,
        },
        cancelled,
        &payer,
    )
    .map_err(|_| SourceCommandError::Unsupported("PWE original source Session allowance"))?;
    let mut call = SourceCall {
        session: &mut session,
        payer: &payer,
        whole,
        registry: None,
    };
    let result = run(&mut call);
    // Attempt the same original Session cutoff on Err as well; preserve the
    // first refusal and every spent prefix. This is not an owner-currentness
    // certificate; the maintained PG/source owner still supplies that fence.
    let after = call
        .session
        .source_allowance()
        .map_err(|error| payer.error(error, "PWE source operation final Session cutoff"));
    match (result, after) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(_)) => Ok(value),
    }
}

pub(super) fn is_pwe(ctx: &CommandContext, claim: &JsonValue) -> SourceCommandResult<bool> {
    let (_, descriptor) = profile(ctx, text(claim, "predicate")?)?;
    Ok(text(&descriptor, "reader")? == "pwe-exact-basis-v1")
}

pub(super) fn request_requires_pwe(ctx: &CommandContext) -> SourceCommandResult<bool> {
    let (config, source, _, create, _) = config(ctx)?;
    let request = parse(&ctx.request_raw)?;
    if !["claims.create", "claim.revise"].contains(&text(&request, "operation")?) {
        // Existing describe/inspect-version semantics do not newly require a
        // proposition profile merely to read a previously retained Claim.
        return Ok(false);
    }
    if create {
        if let Some(claims) = request.object_get("claims").and_then(JsonValue::as_array) {
            for claim in claims {
                if is_pwe(ctx, claim)? {
                    return Ok(true);
                }
            }
        }
        return Ok(false);
    }
    let claims = rows(selected(ctx, source.as_str())?)?;
    let claim = claims
        .get(text(&config, "claim_id")?)
        .ok_or(SourceCommandError::Conflict(
            "PWE delegated source Claim absent",
        ))?;
    is_pwe(ctx, claim)
}

impl SourceCall<'_, '_, '_> {
    fn inspect(&mut self, bytes: usize, work: usize) -> SourceCommandResult<()> {
        self.session
            .admit_source_prefix(bytes, work)
            .map_err(|error| self.payer.error(error, "PWE source caller inspection"))
    }
    fn retain(&mut self, bytes: usize) -> SourceCommandResult<()> {
        self.session
            .retain_source_state(bytes)
            .map_err(|error| self.payer.error(error, "PWE source caller holder"))
    }
    fn canonical(&mut self, value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
        source_canonical(value, self.session, self.payer)
    }
    fn reference(&mut self, claim: &JsonValue) -> SourceCommandResult<JsonValue> {
        let raw = self.canonical(claim)?;
        self.inspect(raw.len(), raw.len())?;
        let digest = Digest256::of_bytes(&raw);
        let id = text(claim, "claim_id")?;
        self.retain(
            id.len()
                .checked_mul(8)
                .and_then(|n| n.checked_add(1024))
                .ok_or(SourceCommandError::Unsupported(
                    "PWE source Claim reference holder",
                ))?,
        )?;
        Ok(object(vec![
            ("id", field(claim, "claim_id")?.clone()),
            ("version", field(claim, "claim_version")?.clone()),
            ("digest", string(&digest.to_prefixed())),
        ]))
    }
    pub(super) fn description_revision(
        &mut self,
        old: &JsonValue,
        new: &JsonValue,
    ) -> SourceCommandResult<()> {
        let old_ref = self.reference(old)?;
        let new_ref = self.reference(new)?;
        check_description_revision_source(self.session, old, &old_ref, new, &new_ref, self.payer)
    }
    /// Resolve EVERY bound occurrence through the maintained complete source
    /// inventory, exact current/history/archive readers and generated catalogue.
    /// Formal meaning/role checks and semantic use still require the held PWE
    /// consumer; this source receipt cannot stand in for NativeBasis.
    pub(super) fn preserve(
        &mut self,
        ctx: &CommandContext,
        cut: Option<&CorpusCutReader>,
        read: &mut RetainedProfileRead,
        claim: &JsonValue,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<JsonValue> {
        if !read
            .whole_call
            .as_ref()
            .is_some_and(|payer| Rc::ptr_eq(payer, self.whole))
        {
            return Err(SourceCommandError::Conflict(
                "PWE retained reader uses another source payer",
            ));
        }
        let dependencies = extract_claim_dependencies_source(claim, self.session, self.payer)?;
        let count = dependencies
            .bound
            .len()
            .checked_add(dependencies.unresolved.len())
            .ok_or(SourceCommandError::Unsupported(
                "PWE source occurrence count overflow",
            ))?;
        // Fixed receipt keys/holders and original source slot order are owned
        // here. The upstream extractor has already paid its own allocations.
        self.retain(
            count
                .checked_mul(2048)
                .and_then(|n| n.checked_add(4096))
                .ok_or(SourceCommandError::Unsupported(
                    "PWE source occurrence holder overflow",
                ))?,
        )?;
        let mut bound = Vec::new();
        let mut unresolved = Vec::new();
        bound
            .try_reserve_exact(dependencies.bound.len())
            .map_err(|_| SourceCommandError::Unsupported("PWE source occurrence reservation"))?;
        unresolved
            .try_reserve_exact(dependencies.unresolved.len())
            .map_err(|_| {
                SourceCommandError::Unsupported("PWE unresolved occurrence reservation")
            })?;
        for slot in dependencies.bound {
            crate::source_creation_store::active(deadline, cancelled)?;
            let id = text(&slot.reference, "id")?;
            self.retain(
                id.len()
                    .checked_mul(8)
                    .and_then(|n| n.checked_add(8192))
                    .ok_or(SourceCommandError::Unsupported(
                        "PWE source exact holder overflow",
                    ))?,
            )?;
            // Admit source-bound tree comparisons BEFORE either keyed lookup.
            // The iterator reads length headers only; no key comparison/clone.
            self.inspect(0, 4)?;
            let steps = read
                .inventory()?
                .claims
                .len()
                .checked_add(read.inventory()?.objects.len())
                .and_then(|n| n.checked_add(2))
                .ok_or(SourceCommandError::Unsupported(
                    "PWE inventory iteration bound",
                ))?;
            self.inspect(0, steps)?;
            let id_work = id
                .len()
                .checked_add(1)
                .ok_or(SourceCommandError::Unsupported(
                    "PWE inventory identity work",
                ))?;
            let mut lookup = 0usize;
            for map in [&read.inventory()?.claims, &read.inventory()?.objects] {
                for key in map.keys() {
                    lookup = lookup
                        .checked_add(key.len())
                        .and_then(|n| n.checked_add(id_work))
                        .ok_or(SourceCommandError::Unsupported(
                            "PWE inventory lookup bound",
                        ))?;
                }
            }
            self.inspect(lookup, lookup)?;
            let provenance =
                if read.inventory()?.claims.contains_key(id) {
                    let resolved = resolve_claim_reference_evidence(
                        ctx,
                        Some(read),
                        &slot.reference,
                        deadline,
                        cancelled,
                    )?;
                    resolved.provenance
                } else if read.inventory()?.objects.contains_key(id) {
                    let (_, _, _, provenance, _) = exact_metadata_version(
                        ctx,
                        cut,
                        Some(read),
                        &slot.reference,
                        worker,
                        deadline,
                        cancelled,
                    )?;
                    provenance
                } else if {
                    let keys = read
                        .entities
                        .as_object()
                        .ok_or(SourceCommandError::Invalid("PWE source kind registry"))?;
                    self.inspect(
                        0,
                        keys.len()
                            .checked_add(1)
                            .ok_or(SourceCommandError::Unsupported(
                                "PWE registry iteration bound",
                            ))?,
                    )?;
                    let mut work = id.len();
                    for (key, _) in keys {
                        work = work
                            .checked_add(key.units().len() + 32)
                            .ok_or(SourceCommandError::Unsupported("PWE registry lookup bound"))?;
                    }
                    self.inspect(work, work)?;
                    read.entities
                        .object_get("registry_id")
                        .and_then(JsonValue::as_str)
                        == Some(id)
                } {
                    // The actual selected kind registry is a formal typed-ref
                    // dependency, not an invented metadata instance.
                    let actual = if self
                        .registry
                        .as_ref()
                        .is_some_and(|(revision, _)| *revision == ctx.base_revision)
                    {
                        let saved = &self.registry.as_ref().unwrap().1;
                        let bytes = source_canonical(saved, self.session, self.payer)?;
                        self.session
                            .retain_source_state(bytes.len())
                            .map_err(|e| self.payer.error(e, "PWE kind registry pin copy"))?;
                        saved.clone()
                    } else {
                        let raw = self.canonical(&read.entities)?;
                        self.inspect(raw.len(), raw.len())?;
                        let digest = Digest256::of_bytes(&raw);
                        self.retain(
                            id.len()
                                .checked_mul(16)
                                .and_then(|n| n.checked_add(2048))
                                .ok_or(SourceCommandError::Unsupported(
                                    "PWE kind registry pin holder",
                                ))?,
                        )?;
                        let actual = object(vec![
                            ("id", field(&read.entities, "registry_id")?.clone()),
                            (
                                "version",
                                field(&read.entities, "registry_version")?.clone(),
                            ),
                            ("digest", string(&digest.to_prefixed())),
                        ]);
                        self.registry = Some((ctx.base_revision, actual.clone()));
                        actual
                    };
                    let left = self.canonical(&actual)?;
                    let right = self.canonical(&slot.reference)?;
                    let cost = left.len().checked_add(right.len()).ok_or(
                        SourceCommandError::Unsupported("PWE kind registry equality bound"),
                    )?;
                    self.inspect(cost, cost)?;
                    if left != right {
                        return Err(SourceCommandError::Conflict(
                            "PWE exact kind registry pin differs",
                        ));
                    }
                    let raw = selected(ctx, ENTITIES)?;
                    self.inspect(raw.len(), raw.len())?;
                    object(vec![
                        ("source_ref", string(ENTITIES)),
                        (
                            "raw_sha256",
                            string(&Digest256::of_bytes(raw).to_prefixed()),
                        ),
                    ])
                } else {
                    return Err(SourceCommandError::Unsupported(
                        "PWE exact dependency requires its maintained source transport",
                    ));
                };
            let retained = self.canonical(&provenance)?.len();
            self.retain(retained)?;
            bound.push(object(vec![
                ("role", string(slot.role)),
                ("outer", number(slot.outer as u64)),
                ("inner", number(slot.inner as u64)),
                ("record", slot.reference),
                ("provenance", provenance),
            ]));
        }
        for slot in dependencies.unresolved {
            unresolved.push(object(vec![
                ("role", string(slot.role)),
                ("outer", number(slot.outer as u64)),
                ("inner", number(slot.inner as u64)),
            ]));
        }
        Ok(object(vec![
            ("adapter", string("pwe-exact-basis-v1")),
            ("source_claim_id", field(claim, "claim_id")?.clone()),
            ("bound_occurrences", JsonValue::Array(bound)),
            ("unresolved_occurrences", JsonValue::Array(unresolved)),
            ("semantic_admission", JsonValue::Bool(false)),
        ]))
    }
}
