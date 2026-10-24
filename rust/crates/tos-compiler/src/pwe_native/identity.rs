//! Q2/Q3 and D1/D2 read mechanics. These results never merge stored identities.
use super::{
    basis::{Error, PweRef, Result, Session},
    qualification::{DispatchRow, Kind, QualifiedAccount},
    source::{CheckedMetadata, ResolvedBasis},
};
use serde_json::Value;
use tos_foundation::{Digest256, SourceRevision};
use tos_validation::assessment::{AssessmentMechanicsReport, AssessmentSchemaBindingV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewLayer {
    SelectedAccounts,
    Historical,
}
/// Pure projection coordinates. Native owners still retain and check the full
/// actual source identity; a supplied digest cannot construct that proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewSource {
    SourceCut { revision: SourceRevision, input_identity: Digest256 },
    PgEpoch { input_identity: Digest256 },
}
impl ViewSource {
    pub fn source_revision(self) -> Option<SourceRevision> {
        match self {
            Self::SourceCut { revision, .. } => Some(revision),
            Self::PgEpoch { .. } => None,
        }
    }
    pub fn input_identity(self) -> Digest256 {
        match self {
            Self::SourceCut { input_identity, .. } | Self::PgEpoch { input_identity } => input_identity,
        }
    }
}
pub struct Query<'v> {
    pub context: &'v Value,
    pub scope: &'v Value,
    pub criterion: &'v Value,
    pub identity_level: &'v str,
    pub layer: ViewLayer,
    pub account_modality: &'v str,
    pub assertion_layer: &'v str,
    pub joint_carrier: &'v Value,
    pub scenario_ref: &'v Value,
    pub source: ViewSource,
    pub assessment_schema_binding: AssessmentSchemaBindingV1,
    pub assessment_scope: &'v Value,
    pub assessment_policy: &'v Value,
    pub observed_now: &'v str,
}
/// Exact coordinates of this immutable projection, retained with the result.
/// An old projection is not current simply because its contents are immutable.
#[derive(Clone, Copy)]
pub struct ViewBasis<'v> {
    pub context: &'v Value,
    pub scope: &'v Value,
    pub criterion: &'v Value,
    pub identity_level: &'v str,
    pub source: ViewSource,
    pub account_modality: &'v str,
    pub assertion_layer: &'v str,
    pub joint_carrier: &'v Value,
    pub scenario_ref: &'v Value,
    pub assessment_scope: &'v Value,
    pub assessment_policy: &'v Value,
    pub observed_now: &'v str,
    pub assessment_schema_binding: AssessmentSchemaBindingV1,
}
/// Native owner hooks are deliberately obligations, not booleans supplied in
/// manifests. The maintained protected caller must implement actual selected
/// source/current access fences and exact signature/argument/role dispatch.
/// The current repository has no registered PWE source-Claim route. Such a
/// caller returns NeedsBinding until the owner-approved registration is present.
pub trait ConsumerOwner {
    fn current_fence(&mut self, basis: &ViewBasis<'_>) -> Result<()>;
    /// A native access scope contains the exact assessed Claim record. A view
    /// over several Claims therefore needs each Claim's protected scope, while
    /// content scope/context/criterion remain common query coordinates. The
    /// default preserves the original single-scope consumer behavior.
    fn assessment_scope<'o>(
        &'o mut self,
        basis: &'o ViewBasis<'_>,
        _claim_ref: &Value,
    ) -> Result<&'o Value> {
        Ok(basis.assessment_scope)
    }
    fn account_dispatch<'o>(
        &'o mut self,
        view: &ViewBasis<'_>,
        account: &QualifiedAccount<'_>,
        claim: &Value,
        basis: &ResolvedBasis<'_>,
    ) -> Result<DispatchRow<'o>>;
}
pub struct IdentityInput<'v, 'r> {
    pub account: &'r QualifiedAccount<'v>,
    pub basis: &'r ResolvedBasis<'v>,
    pub metadata: &'r [CheckedMetadata<'v>],
    pub claim: &'v Value,
    pub claim_ref: &'v Value,
    pub assessment: Option<&'v AssessmentMechanicsReport>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeDisposition {
    Same,
    Distinct,
    NegativeAssertion,
    UnclassifiedSign,
    Candidate,
    Ineligible,
}
pub struct AccountObservation<'v> {
    pub claim: PweRef<'v>,
    pub disposition: EdgeDisposition,
    pub admission: Option<&'v Value>,
    pub dependencies: Vec<PweRef<'v>>,
}
pub struct SameEdge<'v> {
    pub left: &'v Value,
    pub right: &'v Value,
    pub claim: PweRef<'v>,
}
pub struct Conflict<'v> {
    pub distinct_claim: PweRef<'v>,
    pub same_path: Vec<PweRef<'v>>,
}
pub struct IdentityView<'v> {
    basis: ViewBasis<'v>,
    observations: Vec<AccountObservation<'v>>,
    same: Vec<SameEdge<'v>>,
    distinct: Vec<SameEdge<'v>>,
}
impl<'v> IdentityView<'v> {
    pub fn basis(&self) -> &ViewBasis<'v> {
        &self.basis
    }
    pub fn observations(&self) -> &[AccountObservation<'v>] {
        &self.observations
    }
    pub fn same_edges(&self) -> &[SameEdge<'v>] {
        &self.same
    }
    pub fn distinct_constraints(&self) -> &[SameEdge<'v>] {
        &self.distinct
    }
    /// Path in this one exact context/criterion/scope/cut only. Absence is
    /// NoEligiblePathInSelectedCut; it is never a distinctness assertion.
    pub fn path<O: ConsumerOwner>(
        &self,
        s: &mut Session<'_>,
        left: &Value,
        right: &Value,
        owner: &mut O,
    ) -> Result<Option<Vec<PweRef<'v>>>> {
        owner.current_fence(&self.basis)?;
        s.check()?;
        let result = self.path_unfenced(s, left, right)?;
        owner.current_fence(&self.basis)?;
        s.check()?;
        Ok(result)
    }
    fn path_unfenced(
        &self,
        s: &mut Session<'_>,
        left: &Value,
        right: &Value,
    ) -> Result<Option<Vec<PweRef<'v>>>> {
        s.admit(left)?;
        s.admit(right)?;
        s.typed(left)?;
        s.typed(right)?;
        // Identity PER is supported only on participating vertices. No global
        // reflexivity is invented for an unobserved or inaccessible endpoint.
        let mut vertices: Vec<&'v Value> = s.vec(0)?;
        for e in &self.same {
            for v in [e.left, e.right] {
                let mut present = false;
                for x in &vertices {
                    if s.equal(x, v)? {
                        present = true;
                        break;
                    }
                }
                if !present {
                    s.push(&mut vertices, v)?;
                }
            }
        }
        let mut start = None;
        let mut end = None;
        for (i, v) in vertices.iter().enumerate() {
            if s.equal(v, left)? {
                start = Some(i);
            }
            if s.equal(v, right)? {
                end = Some(i);
            }
        }
        let (Some(start), Some(end)) = (start, end) else {
            return Ok(None);
        };
        let mut prior = s.vec(vertices.len())?;
        prior.resize(vertices.len(), None::<(usize, usize)>);
        let mut queue = s.vec(vertices.len())?;
        queue.push(start);
        prior[start] = Some((start, usize::MAX));
        let mut head = 0;
        while head < queue.len() {
            s.tick(1)?;
            let node = queue[head];
            head += 1;
            if node == end {
                break;
            }
            for (edge_index, e) in self.same.iter().enumerate() {
                s.tick(1)?;
                let next = if s.equal(vertices[node], e.left)? {
                    Some(e.right)
                } else if s.equal(vertices[node], e.right)? {
                    Some(e.left)
                } else {
                    None
                };
                if let Some(next) = next {
                    for (i, v) in vertices.iter().enumerate() {
                        if s.equal(v, next)? && prior[i].is_none() {
                            prior[i] = Some((node, edge_index));
                            queue.push(i);
                        }
                    }
                }
            }
        }
        if prior[end].is_none() {
            return Ok(None);
        }
        let mut path = s.vec(0)?;
        let mut cursor = end;
        while cursor != start {
            s.tick(1)?;
            let (parent, e) = prior[cursor].ok_or(Error::Malformed("path predecessor"))?;
            s.push(&mut path, self.same[e].claim)?;
            cursor = parent;
        }
        path.reverse();
        s.tick(path.len())?;
        Ok(Some(path))
    }
    pub fn conflicts<O: ConsumerOwner>(
        &self,
        s: &mut Session<'_>,
        owner: &mut O,
    ) -> Result<Vec<Conflict<'v>>> {
        owner.current_fence(&self.basis)?;
        s.check()?;
        let mut out = s.vec(0)?;
        for d in &self.distinct {
            if let Some(path) = self.path_unfenced(s, d.left, d.right)? {
                s.push(
                    &mut out,
                    Conflict {
                        distinct_claim: d.claim,
                        same_path: path,
                    },
                )?;
            }
        }
        owner.current_fence(&self.basis)?;
        s.check()?;
        Ok(out)
    }
}

impl Session<'_> {
    /// Building a view is a separate consumer execution. A producer receipt is
    /// not an argument. No view is released if a fence/dispatch/budget fails.
    pub fn identity_view<'v, O: ConsumerOwner>(
        &mut self,
        q: &Query<'v>,
        inputs: &[IdentityInput<'v, '_>],
        owner: &mut O,
    ) -> Result<IdentityView<'v>> {
        let basis = ViewBasis {
            context: q.context,
            scope: q.scope,
            criterion: q.criterion,
            identity_level: q.identity_level,
            account_modality: q.account_modality,
            assertion_layer: q.assertion_layer,
            joint_carrier: q.joint_carrier,
            scenario_ref: q.scenario_ref,
            source: q.source,
            assessment_scope: q.assessment_scope,
            assessment_policy: q.assessment_policy,
            observed_now: q.observed_now,
            assessment_schema_binding: q.assessment_schema_binding,
        };
        self.check()?;
        for v in [
            q.context,
            q.scope,
            q.criterion,
            q.assessment_scope,
            q.assessment_policy,
            q.joint_carrier,
            q.scenario_ref,
        ] {
            self.admit(v)?;
        }
        self.typed(q.context)?;
        self.typed(q.scope)?;
        self.typed(q.criterion)?;
        self.exact(q.assessment_policy)?;
        self.exact(q.joint_carrier)?;
        self.exact(q.scenario_ref)?;
        self.tick(
            q.observed_now
                .len()
                .checked_add(q.identity_level.len())
                .ok_or(Error::ResourceLimit)?,
        )?;
        owner.current_fence(&basis)?;
        self.check()?;
        if q.layer == ViewLayer::Historical {
            return Err(Error::NeedsBinding(
                "historical interpretation contract and warrant W",
            ));
        }
        let mut view = IdentityView {
            basis,
            observations: self.vec(inputs.len())?,
            same: self.vec(0)?,
            distinct: self.vec(0)?,
        };
        for input in inputs {
            self.check()?;
            let a = input.account;
            if a.prepared.kind != Kind::Identity {
                return Err(Error::Malformed("identity account"));
            }
            self.admit(a.value())?;
            self.admit(input.claim)?;
            self.admit(input.claim_ref)?;
            let cr = self.exact(input.claim_ref)?;
            self.record_matches(input.claim, cr, "claim_id", "claim_version")?;
            self.check_claim_content(&a.prepared, input.claim)?;
            let qualification = self.field(a.value(), "qualification")?;
            let modality = self.str_field(qualification, "modality")?;
            let assertion_layer = self.str_field(input.claim, "assertion_layer")?;
            if !self.same_text(modality, q.account_modality)?
                || !self.same_text(assertion_layer, q.assertion_layer)?
            {
                return Err(Error::QualificationMismatch(
                    "selected account modality/layer",
                ));
            }
            self.check_role_matrix(&a.prepared, input.metadata)?;
            if !self.equal(input.basis.account, a.value())? {
                return Err(Error::ExactMismatch);
            }
            if !self.equal(a.scope, q.scope)? || !self.equal(a.context, q.context)? {
                return Err(Error::QualificationMismatch("exact query scope/context"));
            }
            let b = self.field(a.value(), "criterion")?;
            let criterion = self.bound(b, "identity criterion")?;
            if !self.equal(criterion, q.criterion)? {
                return Err(Error::QualificationMismatch("exact query criterion"));
            }
            let level = self.str_field(a.value(), "identity_level")?;
            if !self.same_text(level, q.identity_level)? {
                return Err(Error::QualificationMismatch("identity level"));
            }
            // One actual binding must check whole kind/signature/predicate,
            // declared argument roles and all resolved basis slots. Never infer
            // short same-as from a lexical inner predicate or bare registry ref.
            self.check()?;
            let row = owner.account_dispatch(&view.basis, a, input.claim, input.basis)?;
            self.check_dispatch(a, &row)?;
            self.check()?;
            let eligible = match input.assessment {
                Some(report) => {
                    let access_scope = owner.assessment_scope(&view.basis, input.claim_ref)?;
                    self.current_assessment(q, cr, report, access_scope)?
                }
                None => false,
            };
            let outcome = self.str_field(a.value(), "asserted_outcome")?;
            let disposition = if !eligible {
                EdgeDisposition::Ineligible
            } else if matches!(outcome, "candidate-match" | "undetermined") {
                EdgeDisposition::Candidate
            } else {
                match input.claim.get("polarity").and_then(Value::as_str) {
                    Some("positive") => match outcome {
                        "same-referent" => EdgeDisposition::Same,
                        "distinct-referents" => EdgeDisposition::Distinct,
                        _ => return Err(Error::Malformed("outcome")),
                    },
                    Some("negative") => EdgeDisposition::NegativeAssertion,
                    Some("unknown") | None => EdgeDisposition::UnclassifiedSign,
                    _ => return Err(Error::Malformed("polarity")),
                }
            };
            let mut dependencies = self.vec(input.basis.slots.len())?;
            for slot in &input.basis.slots {
                self.tick(1)?;
                dependencies.push(slot.slot.reference);
            }
            self.push(
                &mut view.observations,
                AccountObservation {
                    claim: cr,
                    disposition,
                    admission: input
                        .assessment
                        .map(AssessmentMechanicsReport::current_admission),
                    dependencies,
                },
            )?;
            if matches!(
                disposition,
                EdgeDisposition::Same | EdgeDisposition::Distinct
            ) {
                let edge = SameEdge {
                    left: self.field(a.value(), "left")?,
                    right: self.field(a.value(), "right")?,
                    claim: cr,
                };
                match disposition {
                    EdgeDisposition::Same => self.push(&mut view.same, edge)?,
                    EdgeDisposition::Distinct => self.push(&mut view.distinct, edge)?,
                    _ => {}
                }
            }
        }
        owner.current_fence(&view.basis)?;
        self.check()?;
        Ok(view)
    }
    fn current_assessment(
        &mut self,
        q: &Query<'_>,
        claim: PweRef<'_>,
        report: &AssessmentMechanicsReport,
        access_scope: &Value,
    ) -> Result<bool> {
        // Reports are native mechanics results, never authorization tokens.
        // Their own source/schema worker holders remain with ConsumerOwner.
        self.tick(report.work_used())?;
        if report.source_revision() != q.source.source_revision()
            || report.source_input_identity() != q.source.input_identity()
            || report.schema_binding() != q.assessment_schema_binding
            || !self.same_text(report.observed_now(), q.observed_now)?
        {
            return Err(Error::ExactMismatch);
        }
        self.admit(report.scope())?;
        self.admit(report.current_admission())?;
        self.admit(access_scope)?;
        if !self.equal(report.scope(), access_scope)? {
            return Err(Error::QualificationMismatch("current assessment scope"));
        }
        let a = report.current_admission();
        let subject = self.field(a, "subject")?;
        if self.exact(subject)? != claim {
            return Err(Error::ExactMismatch);
        }
        let policy = self.field(a, "policy")?;
        if !self.equal(policy, q.assessment_policy)? {
            return Err(Error::ExactMismatch);
        }
        let used = self.field(a, "use")?;
        let requested = self.field(access_scope, "requested_use")?;
        if !self.equal(used, requested)? {
            return Err(Error::QualificationMismatch("current use"));
        }
        if report.source_read_required() && !report.source_read_ready() {
            return Ok(false);
        }
        let can_use = self
            .field(a, "can_use")?
            .as_bool()
            .ok_or(Error::Malformed("native admission can_use"))?;
        let status = self.str_field(a, "status")?;
        if can_use && status == "admitted-with-limits" {
            return Err(Error::NeedsBinding("current admission limits interpreter"));
        }
        Ok(can_use && status == "admitted")
    }
}

/// One admissible-world observation. Positive and negative are explicit facts
/// supplied by the joint-carrier owner, never a closed-world interpretation of
/// a PWE source group. A group is a conjunction, not all authors in that world.
pub struct JointWorld<'v> {
    pub world_ref: &'v Value,
    pub positive: &'v [Value],
    pub negative: &'v [Value],
}
pub struct JointCarrier<'v> {
    pub reference: &'v Value,
    pub context: &'v Value,
    pub scope: &'v Value,
}
pub enum AttributionQuestion<'v> {
    Possible(&'v Value),
    Necessary(&'v Value),
    PossibleJoint(&'v Value, &'v Value),
    Excluded(&'v Value),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributionAnswer<'v> {
    Witness(PweRef<'v>),
    Counterexample(PweRef<'v>),
    NoObservedWitness,
    NeedsCoverage,
}
pub struct AttributionObservation<'v> {
    pub carrier: PweRef<'v>,
    pub context: &'v Value,
    pub scope: &'v Value,
    pub answer: AttributionAnswer<'v>,
}
pub trait JointCarrierOwner {
    fn current_fence(&mut self, carrier: &JointCarrier<'_>) -> Result<()>;
    /// Validate this observation's membership, context, scope, interpretation
    /// and positive/negative warrants in ONE selected joint carrier.
    fn admissible_world(
        &mut self,
        carrier: &JointCarrier<'_>,
        world: &JointWorld<'_>,
    ) -> Result<()>;
}
impl Session<'_> {
    pub fn attribution<'v, O: JointCarrierOwner>(
        &mut self,
        carrier: &JointCarrier<'v>,
        question: AttributionQuestion<'_>,
        worlds: &[JointWorld<'v>],
        owner: &mut O,
    ) -> Result<AttributionObservation<'v>> {
        for v in [carrier.reference, carrier.context, carrier.scope] {
            self.admit(v)?;
        }
        let carrier_ref = self.exact(carrier.reference)?;
        self.typed(carrier.context)?;
        self.typed(carrier.scope)?;
        owner.current_fence(carrier)?;
        self.check()?;
        let (a, b) = match question {
            AttributionQuestion::Possible(x)
            | AttributionQuestion::Necessary(x)
            | AttributionQuestion::Excluded(x) => (x, None),
            AttributionQuestion::PossibleJoint(a, b) => (a, Some(b)),
        };
        self.admit(a)?;
        self.typed(a)?;
        if let Some(b) = b {
            self.admit(b)?;
            self.typed(b)?;
        }
        if worlds.len() > self.limits.max_items {
            return Err(Error::ResourceLimit);
        }
        let mut answer = None;
        for (i, world) in worlds.iter().enumerate() {
            self.admit(world.world_ref)?;
            let wr = self.exact(world.world_ref)?;
            for prior in &worlds[..i] {
                let pr = self.exact(prior.world_ref)?;
                if wr == pr {
                    return Err(Error::Malformed("duplicate joint world"));
                }
            }
            if world.positive.len() > self.limits.max_items
                || world.negative.len() > self.limits.max_items
            {
                return Err(Error::ResourceLimit);
            }
            for values in [world.positive, world.negative] {
                for (i, x) in values.iter().enumerate() {
                    self.admit(x)?;
                    self.typed(x)?;
                    for y in &values[..i] {
                        if self.equal(x, y)? {
                            return Err(Error::Malformed("duplicate world atom"));
                        }
                    }
                }
            }
            for x in world.positive {
                for y in world.negative {
                    if self.equal(x, y)? {
                        return Err(Error::Malformed("inconsistent world atom"));
                    }
                }
            }
            owner.admissible_world(carrier, world)?;
            self.check()?;
            let positive = self.contains_typed(world.positive, a)?;
            let negative = self.contains_typed(world.negative, a)?;
            let witnessed = match question {
                AttributionQuestion::Possible(_) => positive,
                AttributionQuestion::PossibleJoint(_, _) => {
                    positive
                        && self.contains_typed(
                            world.positive,
                            b.ok_or(Error::Malformed("joint query"))?,
                        )?
                }
                AttributionQuestion::Necessary(_) => negative,
                AttributionQuestion::Excluded(_) => positive,
            };
            if witnessed && answer.is_none() {
                answer = Some(match question {
                    AttributionQuestion::Necessary(_) | AttributionQuestion::Excluded(_) => {
                        AttributionAnswer::Counterexample(wr)
                    }
                    _ => AttributionAnswer::Witness(wr),
                });
            }
        }
        owner.current_fence(carrier)?;
        self.check()?;
        Ok(AttributionObservation {
            carrier: carrier_ref,
            context: carrier.context,
            scope: carrier.scope,
            answer: answer.unwrap_or(match question {
                AttributionQuestion::Necessary(_) | AttributionQuestion::Excluded(_) => {
                    AttributionAnswer::NeedsCoverage
                }
                _ => AttributionAnswer::NoObservedWitness,
            }),
        })
    }
    fn contains_typed(&mut self, xs: &[Value], x: &Value) -> Result<bool> {
        for y in xs {
            if self.equal(x, y)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// D2: check an independent new Claim plus a separate exact lineage record.
    /// This only checks supplied content; no old selection is deactivated.
    pub fn independent_correction<'v>(
        &mut self,
        old: &Value,
        old_ref: &Value,
        new: &Value,
        new_ref: &Value,
        lineage: &'v Value,
    ) -> Result<&'v Value> {
        for v in [old, old_ref, new, new_ref, lineage] {
            self.admit(v)?;
        }
        let before = self.exact(old_ref)?;
        let after = self.exact(new_ref)?;
        self.record_matches(old, before, "claim_id", "claim_version")?;
        self.record_matches(new, after, "claim_id", "claim_version")?;
        if self.same_text(before.id, after.id)? || after.version != 1 {
            return Err(Error::Malformed("independent Claim identity/version"));
        }
        let top = self.field(new, "supersedes_claim_ref")?;
        if !top.is_null() {
            return Err(Error::Malformed("initial supersedes must be null"));
        }
        let object = self.field(new, "object")?;
        let supersedes = self.field(object, "supersedes_proposition")?;
        self.prepare_admitted(object)?;
        if !supersedes.is_null() {
            return Err(Error::Malformed(
                "independent proposition lineage is separate",
            ));
        }
        self.closed(
            lineage,
            &["predecessor", "successor", "provenance", "selection"],
        )?;
        let r = self.field(lineage, "predecessor")?;
        if self.exact(r)? != before {
            return Err(Error::ExactMismatch);
        }
        let r = self.field(lineage, "successor")?;
        if self.exact(r)? != after {
            return Err(Error::ExactMismatch);
        }
        for role in ["provenance", "selection"] {
            let r = self.field(lineage, role)?;
            self.exact(r)?;
        }
        Ok(lineage)
    }
    /// A description revision keeps all proposition and exact evidence fields.
    /// Original bytes remain independently addressed by old_ref. Recompute all
    /// derived views under the newly selected active basis after any revision.
    /// Only claim_version and the existing /object explanatory allow-list may
    /// differ. Every other top-level field and its presence are frozen,
    /// including unknown/profile-specific source fields. This is narrower than
    /// the maintained general Claim correction route, which can revise evidence.
    pub fn description_revision(
        &mut self,
        old: &Value,
        old_ref: &Value,
        new: &Value,
        new_ref: &Value,
    ) -> Result<()> {
        for v in [old, old_ref, new, new_ref] {
            self.admit(v)?;
        }
        let before = self.exact(old_ref)?;
        let after = self.exact(new_ref)?;
        self.record_matches(old, before, "claim_id", "claim_version")?;
        self.record_matches(new, after, "claim_id", "claim_version")?;
        if !self.same_text(before.id, after.id)?
            || after.version != before.version.checked_add(1).ok_or(Error::ResourceLimit)?
        {
            return Err(Error::Malformed("description revision identity/version"));
        }
        let old_fields = old
            .as_object()
            .ok_or(Error::Malformed("old Claim object"))?;
        let new_fields = new
            .as_object()
            .ok_or(Error::Malformed("new Claim object"))?;
        if old_fields.len() != new_fields.len() {
            return Err(Error::Malformed("description revision top-level field set"));
        }
        // Actual shared source-claim-record.schema.json required fields.
        // This presence check does not replace the native owner's full schema
        // execution. Optional fields are not defaulted to null/empty/unknown.
        for key in [
            "schema_version",
            "claim_id",
            "claim_type",
            "assertion_layer",
            "subject_ref",
            "predicate",
            "object",
            "evidence_refs",
            "maker",
            "provenance_event_ref",
            "epistemic_status",
            "review_status",
            "claim_version",
            "visibility",
        ] {
            self.field(old, key)?;
            self.field(new, key)?;
        }
        for (key, value) in old_fields {
            self.tick(key.len())?;
            let other = self.field(new, key)?;
            if matches!(key.as_str(), "claim_version" | "object") {
                continue;
            }
            // Freeze evidence_refs, supporting_quotes, counterevidence_refs,
            // alternative_claim_refs, provenance, maker, qualifiers, posture,
            // schema/lineage/admission/visibility and opaque extensions alike.
            if !self.equal(value, other)? {
                return Err(Error::Malformed(
                    "description revision protected top-level field",
                ));
            }
        }
        let a = self.field(old, "object")?;
        let b = self.field(new, "object")?;
        self.prepare_admitted(a)?;
        self.prepare_admitted(b)?;
        let am = a.as_object().ok_or(Error::Malformed("old PWE object"))?;
        let bm = b.as_object().ok_or(Error::Malformed("new PWE object"))?;
        if am.len() != bm.len() {
            return Err(Error::Malformed("proposition fields changed"));
        }
        for (key, value) in am {
            // These are presentation/explanation only, never identity, name,
            // role, scope, basis, qualification, assignees or source locations.
            if ["limitations", "account", "grounds", "counterreading"].contains(&key.as_str()) {
                continue;
            }
            let other = self.field(b, key)?;
            if !self.equal(value, other)? {
                return Err(Error::Malformed("proposition changed"));
            }
        }
        Ok(())
    }
}
