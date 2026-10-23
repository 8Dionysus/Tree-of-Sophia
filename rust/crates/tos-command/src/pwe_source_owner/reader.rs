//! Native Claim reader data, invoked only after actual profile/schema/grant
//! selection by the maintained source_claims owner. This pure extraction is
//! neither grounding evidence nor a source/assessment authorization token.
use super::{MAX_RECORDS,canonical,invalid,unsupported};
use crate::source_command::{self as cmd,SourceCommandResult};
use crate::source_claims::ClaimCallBudget;
use std::{cell::RefCell,rc::Rc,sync::atomic::AtomicBool,time::Instant};
use tos_compiler::pwe_native::Session;
use tos_foundation::JsonValue;

pub(crate) struct NativeExactSlot {
    pub role:&'static str,pub outer:usize,pub inner:usize,pub reference:JsonValue,
}
pub(crate) struct NativeUnresolvedSlot {pub role:&'static str,pub outer:usize,pub inner:usize}
pub(crate) struct ClaimDependencies {
    pub bound:Vec<NativeExactSlot>,pub unresolved:Vec<NativeUnresolvedSlot>,
}
/// Preserve-only source creation still resolves EVERY declared bound exact
/// slot through the maintained native owner route. Unknown slots remain in the
/// original Claim; this result cannot construct QualifiedAccount/ResolvedBasis.
pub(crate) fn extract_claim_dependencies(claim:&JsonValue,session:&mut Session<'_>,
    whole:&Rc<RefCell<ClaimCallBudget>>,deadline:Instant,cancelled:&AtomicBool)->SourceCommandResult<ClaimDependencies> {
    crate::source_creation_store::active(deadline,cancelled)?;
    let raw=canonical(claim)?;
    let fee=raw.len().checked_mul(64).ok_or(unsupported("PWE source reader holder overflow"))?;
    whole.try_borrow_mut().map_err(|_|invalid("PWE whole-call budget in use"))?.retain(fee)?;
    let claim:serde_json::Value=serde_json::from_slice(&raw).map_err(|_|invalid("PWE selected source Claim conversion"))?;
    let object=claim.get("object").ok_or(invalid("PWE Claim object absent"))?;
    let prepared=session.prepare(object).map_err(|_|invalid("PWE source account shape"))?;
    session.check_claim_content(&prepared,&claim).map_err(|_|invalid("PWE source wrapper/endpoint disagreement"))?;
    let extracted=session.preserved_basis(&prepared).map_err(|_|invalid("PWE source exact-basis grammar"))?;
    if extracted.slots.len().checked_add(extracted.unresolved.len()).is_none_or(|n|n>MAX_RECORDS) {
        return Err(unsupported("PWE native dependency slot cap"));
    }
    let mut bound=Vec::new();let mut unresolved=Vec::new();
    let holders=extracted.slots.len().checked_mul(std::mem::size_of::<NativeExactSlot>())
        .and_then(|n|n.checked_add(extracted.unresolved.len().checked_mul(std::mem::size_of::<NativeUnresolvedSlot>())?))
        .ok_or(unsupported("PWE dependency holder overflow"))?;
    whole.try_borrow_mut().map_err(|_|invalid("PWE whole-call budget in use"))?.retain(holders)?;
    bound.try_reserve_exact(extracted.slots.len()).map_err(|_|unsupported("PWE dependency reservation"))?;
    unresolved.try_reserve_exact(extracted.unresolved.len()).map_err(|_|unsupported("PWE dependency reservation"))?;
    for slot in extracted.slots {
        whole.try_borrow_mut().map_err(|_|invalid("PWE whole-call budget in use"))?
            .retain(slot.reference.id().len().checked_add(1024).ok_or(unsupported("PWE exact slot overflow"))?)?;
        bound.push(NativeExactSlot{role:slot.role,outer:slot.outer,inner:slot.inner,
            reference:cmd::object(vec![("id",cmd::string(slot.reference.id())),("version",cmd::number(slot.reference.version())),
                ("digest",cmd::string(&slot.reference.digest().to_prefixed()))])});
    }
    for slot in extracted.unresolved {unresolved.push(NativeUnresolvedSlot{role:slot.role,outer:slot.outer,inner:slot.inner});}
    crate::source_creation_store::active(deadline,cancelled)?;
    Ok(ClaimDependencies{bound,unresolved})
}

/// Addressed D2 guard for the maintained source owner's description-only route.
/// The owner still validates both schemas, exact predecessor/history, grant and
/// current source before persisting. This function neither writes nor admits.
pub(crate) fn check_description_revision(session:&mut Session<'_>,old:&JsonValue,old_ref:&JsonValue,
    new:&JsonValue,new_ref:&JsonValue,whole:&Rc<RefCell<ClaimCallBudget>>,
    deadline:Instant,cancelled:&AtomicBool)->SourceCommandResult<()> {
    crate::source_creation_store::active(deadline,cancelled)?;
    let mut values=Vec::new();
    for value in [old,old_ref,new,new_ref] {
        let raw=canonical(value)?;
        whole.try_borrow_mut().map_err(|_|invalid("PWE whole-call budget in use"))?
            .retain(raw.len().checked_mul(64).ok_or(unsupported("PWE correction holder overflow"))?)?;
        values.push(serde_json::from_slice(&raw).map_err(|_|invalid("PWE correction selected JSON"))?);
    }
    session.description_revision(&values[0],&values[1],&values[2],&values[3])
        .map_err(|_|invalid("PWE description-only revision changed protected source/proposition/evidence/lineage"))?;
    crate::source_creation_store::active(deadline,cancelled)
}

/// Separate new Claim identity with exact predecessor/successor and provenance
/// lineage. General evidence correction must never be disguised as D2 wording.
pub(crate) fn check_independent_correction(session:&mut Session<'_>,old:&JsonValue,old_ref:&JsonValue,
    new:&JsonValue,new_ref:&JsonValue,lineage:&JsonValue,whole:&Rc<RefCell<ClaimCallBudget>>,
    deadline:Instant,cancelled:&AtomicBool)->SourceCommandResult<()> {
    crate::source_creation_store::active(deadline,cancelled)?;
    let mut values=Vec::new();
    for value in [old,old_ref,new,new_ref,lineage] {
        let raw=canonical(value)?;
        whole.try_borrow_mut().map_err(|_|invalid("PWE whole-call budget in use"))?
            .retain(raw.len().checked_mul(64).ok_or(unsupported("PWE correction holder overflow"))?)?;
        values.push(serde_json::from_slice(&raw).map_err(|_|invalid("PWE correction selected JSON"))?);
    }
    session.independent_correction(&values[0],&values[1],&values[2],&values[3],&values[4])
        .map_err(|_|invalid("PWE independent correction identity/version/lineage differs"))?;
    crate::source_creation_store::active(deadline,cancelled)
}
