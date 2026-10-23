//! Bounded structural operation bindings. No source resolution, output-profile
//! execution, proof verification, completeness issuance or admission occurs here.
use crate::{Digest256, ExactRecordRef, JsonNumberKind, JsonValue, RecordVersion};

/// Per-call admission. Byte charge is a conservative compact JSON encoding bound
/// (six bytes per retained UTF-16 unit, including keys), not a raw-file digest.
/// `max_nodes` bounds node visits AND inspected/comparison string units; `max_refs`
/// bounds reference visits, including comparisons. All passes share these caps.
/// No input/source bodies or key sets are cloned or allocated by this checker.
#[derive(Clone, Copy, Debug)]
pub struct OperationBindingLimits {
    pub max_envelope_bytes: usize,
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_refs: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingError {
    Malformed(&'static str),
    UnsupportedProfile,
    ResourceLimit,
    IncompatibleDisposition,
    DuplicateSlot,
    CertificateBoundaryMismatch,
    IncompatibleBoundary,
    UnverifiedBridge,
}
impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for BindingError {}
type BindingResult<T> = std::result::Result<T, BindingError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueDisposition {
    Produced,
    Undefined,
    Partial,
    NoCompleteValue,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionStatus {
    NotStarted,
    Running,
    Completed,
    ResourceLimit,
    Unsupported,
    Error,
    BlockedByUpstream,
}

/// Checked means structurally checked ONLY. The reference body, projection and
/// codomain still require the producer's exact output-profile adapter (Q4/B1).
/// This immutable borrow cannot grant source authority or validate a theorem.
#[derive(Debug)]
pub struct CheckedOperationBinding<'a> {
    envelope: &'a JsonValue,
    run_id: &'a str,
    value: ValueDisposition,
    execution: ExecutionStatus,
    output: Option<&'a JsonValue>,
    output_profile: Option<&'a JsonValue>,
}
impl<'a> CheckedOperationBinding<'a> {
    pub fn as_json(&self) -> &'a JsonValue { self.envelope }
    pub fn run_id(&self) -> &'a str { self.run_id }
    pub fn value_disposition(&self) -> ValueDisposition { self.value }
    pub fn execution_status(&self) -> ExecutionStatus { self.execution }
    pub fn output_ref(&self) -> Option<&'a JsonValue> { self.output }
    pub fn output_profile_ref(&self) -> Option<&'a JsonValue> { self.output_profile }
}

/// The left binding has one output. The selected right slot is exact and local.
/// Merely naming a bridge never verifies its semantic transport contract.
#[derive(Debug)]
pub struct CompositionLink<'a> {
    right_input_slot: &'a str,
    bridge_ref: Option<&'a ExactRecordRef>,
}
impl<'a> CompositionLink<'a> {
    pub fn exact(right_input_slot: &'a str) -> Self {
        Self { right_input_slot, bridge_ref: None }
    }
    pub fn through_bridge(right_input_slot: &'a str, bridge_ref: &'a ExactRecordRef) -> Self {
        Self { right_input_slot, bridge_ref: Some(bridge_ref) }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyComparison { ExactPinsEqual, Changed, Unresolved }

/// Reserved owner-adapter evidence; NO constructor, deserializer, default or
/// generic issuer exists in K0. A wire coverage string cannot create this type.
/// A future reviewed owner must bind capture to the actual envelope, observable,
/// manifest, cut, extraction procedure and limits before adding an issuance seam.
#[derive(Debug)]
pub struct ManifestCoverageEvidence<'a> {
    binding: &'a JsonValue,
}

struct Budget {
    limits: OperationBindingLimits,
    bytes: usize,
    work: usize,
    refs: usize,
}
fn add(counter: &mut usize, amount: usize, cap: usize) -> BindingResult<()> {
    let next = counter.checked_add(amount).ok_or(BindingError::ResourceLimit)?;
    if next > cap { return Err(BindingError::ResourceLimit); }
    *counter = next;
    Ok(())
}
impl Budget {
    fn new(limits: OperationBindingLimits) -> BindingResult<Self> {
        // Same recursion safety ceiling as the foundation JSON codec. A caller
        // cannot authorize an unbounded native stack through a usize limit.
        if limits.max_envelope_bytes == 0 || limits.max_nodes == 0
            || limits.max_depth == 0 || limits.max_depth > 128 || limits.max_refs == 0 {
            return Err(BindingError::ResourceLimit);
        }
        Ok(Self { limits, bytes: 0, work: 0, refs: 0 })
    }
    fn work(&mut self, n: usize) -> BindingResult<()> { add(&mut self.work, n, self.limits.max_nodes) }
    fn bytes(&mut self, n: usize) -> BindingResult<()> { add(&mut self.bytes, n, self.limits.max_envelope_bytes) }
    fn reference(&mut self) -> BindingResult<()> { add(&mut self.refs, 1, self.limits.max_refs) }
    fn text_eq(&mut self, a: &str, b: &str) -> BindingResult<bool> {
        self.work(a.len())?; self.work(b.len())?;
        Ok(a == b)
    }
    fn string(&mut self, s: &crate::JsonString) -> BindingResult<()> {
        self.work(s.units().len())?;
        self.bytes(s.units().len().checked_mul(6).and_then(|n| n.checked_add(2))
            .ok_or(BindingError::ResourceLimit)?)
    }
    fn walk(&mut self, value: &JsonValue, depth: usize) -> BindingResult<()> {
        if depth > self.limits.max_depth { return Err(BindingError::ResourceLimit); }
        self.work(1)?;
        match value {
            JsonValue::Null => self.bytes(4)?,
            JsonValue::Bool(v) => self.bytes(if *v { 4 } else { 5 })?,
            JsonValue::Number(n) => { self.work(n.lexeme.len())?; self.bytes(n.lexeme.len())?; }
            JsonValue::String(s) => self.string(s)?,
            JsonValue::Array(values) => {
                self.bytes(2)?;
                for (i, v) in values.iter().enumerate() {
                    if i != 0 { self.bytes(1)?; }
                    self.walk(v, depth + 1)?;
                }
            }
            JsonValue::Object(entries) => {
                self.bytes(2)?;
                for (i, (key, v)) in entries.iter().enumerate() {
                    self.work(1)?; self.string(key)?; self.bytes(if i == 0 { 1 } else { 2 })?;
                    for (earlier, _) in &entries[..i] {
                        self.work(1)?; self.work(earlier.units().len())?; self.work(key.units().len())?;
                        if earlier.units() == key.units() {
                            return Err(BindingError::Malformed("duplicate object key"));
                        }
                    }
                    self.walk(v, depth + 1)?;
                }
            }
        }
        Ok(())
    }
}

// No JsonValue::object_get: it allocates a UTF-16 key on every lookup. These
// bounded scans also charge duplicate detection and direct API follow-up work.
fn field<'a>(v: &'a JsonValue, name: &str, b: &mut Budget) -> BindingResult<&'a JsonValue> {
    let entries = v.as_object().ok_or(BindingError::Malformed("expected object"))?;
    for (key, value) in entries {
        b.work(1)?; b.work(key.units().len())?; b.work(name.len())?;
        if key.as_str() == Some(name) { return Ok(value); }
    }
    Err(BindingError::Malformed("missing field"))
}
fn fields(v: &JsonValue, names: &[&str], b: &mut Budget) -> BindingResult<()> {
    let entries = v.as_object().ok_or(BindingError::Malformed("expected object"))?;
    if entries.len() != names.len() { return Err(BindingError::Malformed("closed object fields")); }
    for name in names { field(v, name, b)?; }
    Ok(())
}
fn array(v: &JsonValue) -> BindingResult<&[JsonValue]> {
    v.as_array().ok_or(BindingError::Malformed("expected array"))
}
fn text<'a>(v: &'a JsonValue, b: &mut Budget) -> BindingResult<&'a str> {
    let s = v.as_str().ok_or(BindingError::Malformed("expected UTF-8 string"))?;
    b.work(s.len())?;
    if s.is_empty() { return Err(BindingError::Malformed("empty string")); }
    Ok(s)
}
fn label(v: &JsonValue, expected: &str, b: &mut Budget) -> BindingResult<()> {
    let actual = text(v, b)?;
    if !b.text_eq(actual, expected)? { return Err(BindingError::UnsupportedProfile); }
    Ok(())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BorrowedRef<'a> { id: &'a str, version: RecordVersion, digest: Digest256 }
fn reference<'a>(v: &'a JsonValue, b: &mut Budget) -> BindingResult<BorrowedRef<'a>> {
    b.reference()?; fields(v, &["id", "version", "digest"], b)?;
    let id = text(field(v, "id", b)?, b)?;
    // StableId's non-control identity rule, without allocating a String.
    if id.chars().any(char::is_control) { return Err(BindingError::Malformed("invalid stable id")); }
    let number = match field(v, "version", b)? {
        JsonValue::Number(n) if n.kind == JsonNumberKind::Int => n,
        _ => return Err(BindingError::Malformed("expected u64 version")),
    };
    b.work(number.lexeme.len())?;
    if number.lexeme.is_empty() || number.lexeme.starts_with('0')
        || !number.lexeme.bytes().all(|c| c.is_ascii_digit()) {
        return Err(BindingError::Malformed("expected positive u64 version"));
    }
    b.work(number.lexeme.len())?;
    let version = RecordVersion::new(number.lexeme.parse::<u64>()
        .map_err(|_| BindingError::Malformed("version outside u64"))?)
        .map_err(|_| BindingError::Malformed("zero version"))?;
    let digest = Digest256::from_prefixed(text(field(v, "digest", b)?, b)?)
        .map_err(|_| BindingError::Malformed("expected sha256 digest"))?;
    Ok(BorrowedRef { id, version, digest })
}
fn refs_equal(a: &JsonValue, c: &JsonValue, b: &mut Budget) -> BindingResult<bool> {
    let a = reference(a, b)?; let c = reference(c, b)?;
    Ok(b.text_eq(a.id, c.id)? && a.version == c.version && a.digest == c.digest)
}
fn same_field(a: &JsonValue, an: &str, c: &JsonValue, cn: &str, b: &mut Budget) -> BindingResult<bool> {
    let a = field(a, an, b)?; let c = field(c, cn, b)?;
    refs_equal(a, c, b)
}
fn unique_slots(rows: &[JsonValue], b: &mut Budget) -> BindingResult<()> {
    for (i, row) in rows.iter().enumerate() {
        let slot = text(field(row, "slot", b)?, b)?;
        for earlier in &rows[..i] {
            b.work(1)?;
            let other = text(field(earlier, "slot", b)?, b)?;
            if b.text_eq(slot, other)? { return Err(BindingError::DuplicateSlot); }
        }
    }
    Ok(())
}
fn inputs(v: &JsonValue, b: &mut Budget) -> BindingResult<()> {
    let rows = array(v)?;
    for row in rows {
        b.work(1)?; fields(row, &["slot", "profile_ref", "record_ref"], b)?;
        text(field(row, "slot", b)?, b)?;
        reference(field(row, "profile_ref", b)?, b)?;
        reference(field(row, "record_ref", b)?, b)?;
    }
    unique_slots(rows, b)
}
fn inputs_equal(a: &JsonValue, c: &JsonValue, b: &mut Budget) -> BindingResult<bool> {
    let a = array(a)?; let c = array(c)?;
    if a.len() != c.len() { return Ok(false); }
    for (a, c) in a.iter().zip(c) {
        b.work(1)?;
        let sa = text(field(a, "slot", b)?, b)?; let sc = text(field(c, "slot", b)?, b)?;
        if !b.text_eq(sa, sc)? || !same_field(a, "profile_ref", c, "profile_ref", b)?
            || !same_field(a, "record_ref", c, "record_ref", b)? { return Ok(false); }
    }
    Ok(true)
}
fn manifest(v: &JsonValue, b: &mut Budget) -> BindingResult<()> {
    fields(v, &["manifest_ref", "observable_ref", "coverage", "entries"], b)?;
    reference(field(v, "manifest_ref", b)?, b)?;
    reference(field(v, "observable_ref", b)?, b)?;
    match text(field(v, "coverage", b)?, b)? {
        "DeclaredCompleteForObservable" | "Incomplete" | "Unresolved" => (),
        _ => return Err(BindingError::UnsupportedProfile),
    }
    let rows = array(field(v, "entries", b)?)?;
    for row in rows {
        b.work(1)?; fields(row, &["slot", "role", "ref"], b)?;
        text(field(row, "slot", b)?, b)?;
        match text(field(row, "role", b)?, b)? {
            "Source" | "Interpretation" | "Theory" | "Structure" | "Map" | "RuleSet"
            | "Operator" | "Context" | "SelectionPolicy" | "CandidateDomain" | "Coverage"
            | "Method" | "AssessmentPolicy" | "AuthorityOrCompetence" | "OtherExplicit" => (),
            _ => return Err(BindingError::UnsupportedProfile),
        }
        reference(field(row, "ref", b)?, b)?;
    }
    unique_slots(rows, b)
}
fn disposition<'a>(v: &'a JsonValue, b: &mut Budget)
    -> BindingResult<(ValueDisposition, Option<&'a JsonValue>, Option<&'a JsonValue>)> {
    let kind = text(field(v, "kind", b)?, b)?;
    match kind {
        "Produced" | "Partial" => {
            if kind == "Produced" { fields(v, &["kind", "output_ref", "output_profile_ref"], b)?; }
            else {
                fields(v, &["kind", "output_ref", "output_profile_ref", "partial_schema_ref", "coverage_ref"], b)?;
                reference(field(v, "partial_schema_ref", b)?, b)?;
                reference(field(v, "coverage_ref", b)?, b)?;
            }
            let output = field(v, "output_ref", b)?; let profile = field(v, "output_profile_ref", b)?;
            reference(output, b)?; reference(profile, b)?;
            Ok((if kind == "Produced" { ValueDisposition::Produced } else { ValueDisposition::Partial }, Some(output), Some(profile)))
        }
        "Undefined" => {
            fields(v, &["kind", "reason", "domain_evidence_ref"], b)?;
            text(field(v, "reason", b)?, b)?;
            reference(field(v, "domain_evidence_ref", b)?, b)?;
            Ok((ValueDisposition::Undefined, None, None))
        }
        "NoCompleteValue" => {
            fields(v, &["kind", "reason"], b)?; text(field(v, "reason", b)?, b)?;
            Ok((ValueDisposition::NoCompleteValue, None, None))
        }
        _ => Err(BindingError::UnsupportedProfile),
    }
}
fn execution(v: &JsonValue, b: &mut Budget) -> BindingResult<ExecutionStatus> {
    Ok(match text(v, b)? {
        "NotStarted" => ExecutionStatus::NotStarted, "Running" => ExecutionStatus::Running,
        "Completed" => ExecutionStatus::Completed, "ResourceLimit" => ExecutionStatus::ResourceLimit,
        "Unsupported" => ExecutionStatus::Unsupported, "Error" => ExecutionStatus::Error,
        "BlockedByUpstream" => ExecutionStatus::BlockedByUpstream,
        _ => return Err(BindingError::UnsupportedProfile),
    })
}
fn qualifier(v: &JsonValue, assessment: bool, b: &mut Budget) -> BindingResult<()> {
    let kind = text(field(v, "kind", b)?, b)?;
    let ref_name = match (assessment, kind) {
        (false, "Applicable") => "evidence_ref",
        (false, "NotApplicable") => "failed_precondition_ref",
        (true, "AssessmentReference") => "assessment_ref",
        (true, "NotRelevant") => return fields(v, &["kind"], b),
        (false, "ApplicabilityUnresolved") | (true, "NotAssessed") => {
            fields(v, &["kind", "reason"], b)?;
            text(field(v, "reason", b)?, b)?; return Ok(());
        }
        _ => return Err(BindingError::UnsupportedProfile),
    };
    fields(v, &["kind", ref_name], b)?; reference(field(v, ref_name, b)?, b)?;
    Ok(())
}
fn certificate(c: &JsonValue, e: &CheckedOperationBinding<'_>, b: &mut Budget) -> BindingResult<()> {
    fields(c, &["certificate_ref", "binding_status", "bound_run_id", "bound_operator_ref",
        "bound_input_refs", "bound_context_ref", "bound_selection_ref",
        "bound_dependency_manifest_ref", "bound_output_ref"], b)?;
    reference(field(c, "certificate_ref", b)?, b)?;
    label(field(c, "binding_status", b)?, "UnverifiedReference", b)?;
    let run = text(field(c, "bound_run_id", b)?, b)?;
    let ci = field(c, "bound_input_refs", b)?; inputs(ci, b)?;
    let ei = field(e.envelope, "inputs", b)?;
    let m = field(e.envelope, "dependency_manifest", b)?;
    let mut good = b.text_eq(run, e.run_id)?;
    for (bound, actual) in [
        ("bound_operator_ref", "operator_ref"),
        ("bound_context_ref", "context_ref"),
        ("bound_selection_ref", "selection_ref"),
    ] {
        good &= same_field(c, bound, e.envelope, actual, b)?;
    }
    good &= same_field(c, "bound_dependency_manifest_ref", m, "manifest_ref", b)?;
    good &= inputs_equal(ci, ei, b)?;
    // Even a mismatching certificate is fully structurally checked above/below;
    // no claimed binding_status upgrades an unverified reference to a proof.
    let bound_output = field(c, "bound_output_ref", b)?;
    let output_matches = match e.output {
        Some(output) => !bound_output.is_null() && refs_equal(bound_output, output, b)?,
        None => {
            if !bound_output.is_null() { reference(bound_output, b)?; }
            bound_output.is_null()
        }
    };
    if !good || !output_matches { return Err(BindingError::CertificateBoundaryMismatch); }
    Ok(())
}
fn check<'a>(envelope: &'a JsonValue, b: &mut Budget) -> BindingResult<CheckedOperationBinding<'a>> {
    b.walk(envelope, 0)?;
    fields(envelope, &["schema_version", "run_id", "operator_ref", "context_ref", "selection_ref",
        "inputs", "dependency_manifest", "value", "execution", "applicability", "assessment",
        "certificates", "warrant_refs", "authority_boundary"], b)?;
    label(field(envelope, "schema_version", b)?, "tos_operation_binding_v1", b)?;
    label(field(envelope, "authority_boundary", b)?, "StructuralBindingOnlyNoAdmissionOrProofVerification", b)?;
    let run_id = text(field(envelope, "run_id", b)?, b)?;
    for name in ["operator_ref", "context_ref", "selection_ref"] { reference(field(envelope, name, b)?, b)?; }
    inputs(field(envelope, "inputs", b)?, b)?;
    manifest(field(envelope, "dependency_manifest", b)?, b)?;
    let (value, output, output_profile) = disposition(field(envelope, "value", b)?, b)?;
    let execution = execution(field(envelope, "execution", b)?, b)?;
    let compatible = match value {
        ValueDisposition::Produced | ValueDisposition::Undefined => execution == ExecutionStatus::Completed,
        ValueDisposition::Partial => execution != ExecutionStatus::NotStarted,
        ValueDisposition::NoCompleteValue => execution != ExecutionStatus::Completed,
    };
    if !compatible { return Err(BindingError::IncompatibleDisposition); }
    qualifier(field(envelope, "applicability", b)?, false, b)?;
    qualifier(field(envelope, "assessment", b)?, true, b)?;
    let checked = CheckedOperationBinding { envelope, run_id, value, execution, output, output_profile };
    for c in array(field(envelope, "certificates", b)?)? { b.work(1)?; certificate(c, &checked, b)?; }
    let warrants = array(field(envelope, "warrant_refs", b)?)?;
    for (i, w) in warrants.iter().enumerate() {
        reference(w, b)?;
        for earlier in &warrants[..i] {
            b.work(1)?;
            if refs_equal(w, earlier, b)? { return Err(BindingError::Malformed("duplicate warrant ref")); }
        }
    }
    Ok(checked)
}

pub fn check_operation_binding(envelope: &JsonValue, limits: OperationBindingLimits)
    -> BindingResult<CheckedOperationBinding<'_>> {
    check(envelope, &mut Budget::new(limits)?)
}

/// Re-admits both complete borrowed envelopes and the link under ONE call cap.
/// Success asserts only an exact structural boundary. It says nothing about
/// H2 joint witnesses, Q1/Q2 eligibility, Q3 quantifiers or downstream execution.
pub fn check_composition(left: &CheckedOperationBinding<'_>, right: &CheckedOperationBinding<'_>,
    link: &CompositionLink<'_>, limits: OperationBindingLimits) -> BindingResult<()> {
    let b = &mut Budget::new(limits)?;
    let left = check(left.envelope, b)?; let right = check(right.envelope, b)?;
    b.work(link.right_input_slot.len())?; b.bytes(link.right_input_slot.len())?;
    if let Some(bridge) = link.bridge_ref {
        b.reference()?; b.work(bridge.id.as_str().len())?; b.bytes(bridge.id.as_str().len())?;
        return Err(BindingError::UnverifiedBridge);
    }
    if left.value != ValueDisposition::Produced || left.execution != ExecutionStatus::Completed {
        return Err(BindingError::IncompatibleBoundary);
    }
    let rows = array(field(right.envelope, "inputs", b)?)?;
    let mut selected = None;
    for row in rows {
        b.work(1)?;
        let slot = text(field(row, "slot", b)?, b)?;
        if b.text_eq(slot, link.right_input_slot)? { selected = Some(row); break; }
    }
    let input = selected.ok_or(BindingError::IncompatibleBoundary)?;
    if !refs_equal(left.output.ok_or(BindingError::IncompatibleBoundary)?, field(input, "record_ref", b)?, b)?
        || !refs_equal(left.output_profile.ok_or(BindingError::IncompatibleBoundary)?, field(input, "profile_ref", b)?, b)?
        || !same_field(left.envelope, "context_ref", right.envelope, "context_ref", b)?
        || !same_field(left.envelope, "selection_ref", right.envelope, "selection_ref", b)? {
        return Err(BindingError::IncompatibleBoundary);
    }
    Ok(())
}

fn entries_equal(a: &JsonValue, c: &JsonValue, b: &mut Budget) -> BindingResult<bool> {
    let a = array(a)?; let c = array(c)?;
    if a.len() != c.len() { return Ok(false); }
    for row in a {
        let slot = text(field(row, "slot", b)?, b)?;
        let mut found = false;
        for other in c {
            b.work(1)?;
            let os = text(field(other, "slot", b)?, b)?;
            if b.text_eq(slot, os)? {
                let role = text(field(row, "role", b)?, b)?;
                let other_role = text(field(other, "role", b)?, b)?;
                if !b.text_eq(role, other_role)? || !same_field(row, "ref", other, "ref", b)? { return Ok(false); }
                found = true; break;
            }
        }
        if !found { return Ok(false); }
    }
    Ok(true)
}

/// Does not compare run IDs, certificate IDs or serialized object field order.
/// Equal pins without independently issued owner evidence remain Unresolved.
pub fn compare_dependency_bindings(old: &CheckedOperationBinding<'_>, new: &CheckedOperationBinding<'_>,
    old_coverage: Option<&ManifestCoverageEvidence<'_>>, new_coverage: Option<&ManifestCoverageEvidence<'_>>,
    limits: OperationBindingLimits) -> BindingResult<DependencyComparison> {
    let b = &mut Budget::new(limits)?;
    let old = check(old.envelope, b)?; let new = check(new.envelope, b)?;
    let om = field(old.envelope, "dependency_manifest", b)?; let nm = field(new.envelope, "dependency_manifest", b)?;
    for name in ["operator_ref", "context_ref", "selection_ref"] {
        if !same_field(old.envelope, name, new.envelope, name, b)? { return Ok(DependencyComparison::Changed); }
    }
    let oi = field(old.envelope, "inputs", b)?; let ni = field(new.envelope, "inputs", b)?;
    if !same_field(om, "observable_ref", nm, "observable_ref", b)? || !inputs_equal(oi, ni, b)?
        || !entries_equal(field(om, "entries", b)?, field(nm, "entries", b)?, b)? {
        return Ok(DependencyComparison::Changed);
    }
    match (old.output_profile, new.output_profile) {
        (Some(a), Some(c)) if !refs_equal(a, c, b)? => return Ok(DependencyComparison::Changed),
        (None, Some(_)) | (Some(_), None) => return Ok(DependencyComparison::Changed),
        _ => (),
    }
    // K0 intentionally issues no instance of this private-field evidence type.
    // Pointer binding prevents evidence for another envelope from being reused.
    let owner_bound = old_coverage.is_some_and(|e| std::ptr::eq(e.binding, old.envelope))
        && new_coverage.is_some_and(|e| std::ptr::eq(e.binding, new.envelope));
    let oc = text(field(om, "coverage", b)?, b)?; let nc = text(field(nm, "coverage", b)?, b)?;
    if owner_bound && oc == "DeclaredCompleteForObservable" && nc == "DeclaredCompleteForObservable" {
        Ok(DependencyComparison::ExactPinsEqual)
    } else { Ok(DependencyComparison::Unresolved) }
}
