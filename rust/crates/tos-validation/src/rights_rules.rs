//! Source rights-record mechanics, separate from current authorization.
//! Schema validity and stored permission wording cannot establish a current
//! grant, revocation fence, personal consent, publication right or legal review.
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use serde_json::Value;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};
use crate::{KeyState, PredicateRead};
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const CONTRACT: &str = "ToS/contracts/rights-record.schema.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RightsIssue { pub path: String, pub code: &'static str, pub subject: String }
#[derive(Debug, Clone)]
pub struct SourceRightsReport {
    pub revision: SourceRevision,
    pub carrier_membership: SourceMembershipV1,
    pub rights_record_count: u64,
    pub issues: Vec<RightsIssue>,
    pub reads: Vec<PredicateRead>,
    /// Current-use rights are deliberately never inferred from these bytes.
    pub missing_authority: Vec<&'static str>,
}

struct Accumulator { limits: ItemLimits, state: usize, total: u64,
    issues: Vec<RightsIssue>, reads: Vec<PredicateRead>, count: u64 }
impl Accumulator {
    fn reserve(&mut self, n: usize) -> Result<(), ItemRefusal> {
        self.state = self.state.checked_add(n).filter(|n| *n <= self.limits.max_state_bytes)
            .ok_or(ItemRefusal::Budget)?; Ok(())
    }
    fn issue(&mut self,path:&str,code:&'static str,subject:&str)->Result<(),ItemRefusal>{
        if self.issues.len() >= self.limits.max_issues { return Err(ItemRefusal::Budget); }
        self.reserve(path.len()+code.len()+subject.len()+96)?;
        self.issues.push(RightsIssue{path:path.into(),code,subject:subject.into()}); Ok(())
    }
    fn read(&mut self,read:PredicateRead)->Result<(),ItemRefusal>{
        let n=match &read { PredicateRead::ExactPath{path,digest}=>path.len()+digest.len(),
            PredicateRead::RefEndpoint{endpoint_type,id,..}=>endpoint_type.len()+id.len(),
            PredicateRead::SchemaResource{uri,digest}=>uri.len()+digest.len(),
            _=>return Err(ItemRefusal::Unsupported("rights typed-read accounting".into())) };
        self.reserve(n+96)?; self.reads.push(read); Ok(())
    }
}

fn check(deadline:Instant,cancelled:&AtomicBool)->Result<(),ItemRefusal>{
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {Err(ItemRefusal::Deadline)} else {Ok(())}
}

fn source_refs(cut:&CorpusCutReader,path:&str,value:&Value,state:&mut Accumulator,
    cancelled:&AtomicBool)->Result<(),ItemRefusal>{
    let refs=["source_refs","source_record_refs","receipt_refs"].into_iter().flat_map(|field|
        value.get(field).and_then(Value::as_array).into_iter().flatten());
    for target in refs {
        check(state.limits.deadline,cancelled)?;
        let Some(target)=target.as_str() else {state.issue(path,"unresolved-source-ref","non-string")?;continue};
        // The source law treats a non-ToS reference as external evidence, not
        // as a filesystem read or a claim of its remote existence.
        if !target.starts_with("ToS/") {continue}
        let relative=RelativePath::parse(target).map_err(|_|ItemRefusal::Unsupported("rights source reference path".into()))?;
        let present=cut.presence(cut.current().revision(),&relative).is_some();
        state.read(PredicateRead::RefEndpoint {endpoint_type:"source-path".into(),id:target.into(),
            observed:if present {KeyState::Present}else{KeyState::Absent}})?;
        if !present {state.issue(path,"unresolved-source-ref",target)?;}
    }
    Ok(())
}

/// Execute current rights records from real selected bytes and the selected
/// worker. The full source namespace is read to EOF; rights shapes use the
/// ordinary decoded-field JSON profile of the existing native Item loaders.
pub fn inspect_rights_from_cut(cut:&CorpusCutReader,limits:ItemLimits,cancelled:&AtomicBool,
    schemas:&mut CutWorkerSchemaExecutor)->Result<SourceRightsReport,ItemRefusal>{
    check(limits.deadline,cancelled)?;
    if limits.max_member_bytes==0 || limits.max_member_bytes==usize::MAX
        || limits.max_total_bytes==0 || limits.max_total_bytes==u64::MAX
        || limits.max_state_bytes==0 || limits.max_state_bytes==usize::MAX
        || limits.max_issues==0 || limits.max_issues==usize::MAX {return Err(ItemRefusal::Budget)}
    let revision=cut.current().revision();
    if schemas.source_revision()!=revision {return Err(ItemRefusal::Source("rights worker selected another source cut".into()))}
    let schema=RelativePath::parse(CONTRACT).map_err(|_|ItemRefusal::Unsupported("rights contract".into()))?;
    let metadata=cut.current().member(&schema).ok_or_else(||ItemRefusal::Unsupported("missing current rights contract".into()))?;
    let mut state=Accumulator{limits,state:0,total:0,issues:Vec::new(),reads:Vec::new(),count:0};
    state.read(PredicateRead::SchemaResource{uri:CONTRACT.into(),digest:metadata.sha256.to_hex()})?;
    let mut stream=cut.stream(revision).map_err(store_error)?;
    while let Some(member)=stream.next_member(limits.deadline,cancelled).map_err(store_error)? {
        check(limits.deadline,cancelled)?;
        state.total=state.total.checked_add(member.raw.len() as u64).filter(|n|*n<=limits.max_total_bytes).ok_or(ItemRefusal::Budget)?;
        let path=member.path.as_str();
        if !path.starts_with("ToS/source-witnesses/") || !path.ends_with(".json") {continue}
        if member.raw.len()>limits.max_member_bytes {return Err(ItemRefusal::Budget)}
        let selected_path=path.ends_with("/rights.json") || path.starts_with("ToS/source-witnesses/rights/");
        let value=match crate::native_decoded_value(&member.raw,limits.max_member_bytes) {
            Ok(value)=>value,
            Err(ItemRefusal::Source(_)) if selected_path=>{state.issue(path,"invalid-json",path)?;continue},
            Err(ItemRefusal::Source(_))=>continue,
            Err(error)=>return Err(error),
        };
        let version=value.get("schema_version").and_then(Value::as_str).unwrap_or("");
        if !selected_path && version!="tos_rights_record_v1" {continue}
        if version!="tos_rights_record_v1" && version.starts_with("tos_rights_record_") {
            return Err(ItemRefusal::Unsupported(format!("unknown rights profile {version}")))
        }
        if !value.is_object(){state.issue(path,"object-required",path)?;continue}
        state.count=state.count.checked_add(1).ok_or(ItemRefusal::Budget)?;
        state.read(PredicateRead::ExactPath{path:path.into(),digest:Digest256::of_bytes(&member.raw).to_hex()})?;
        // Transient parsed state has a finite logical budget. Persistent state
        // is independently charged in the issue/read accumulator.
        let transient=member.raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?;
        if state.state.checked_add(transient).is_none_or(|n|n>limits.max_state_bytes){return Err(ItemRefusal::Budget)}
        if !schemas.check(path,&member.raw,CONTRACT,limits.deadline,cancelled)? {state.issue(path,"schema",CONTRACT)?;}
        source_refs(cut,path,&value,&mut state,cancelled)?;
        let mut layer_ids=BTreeSet::new(); let mut layer_state=0usize;
        if let Some(layers)=value.get("layer_assessments").and_then(Value::as_array){
            for (ordinal,layer) in layers.iter().enumerate(){
                check(limits.deadline,cancelled)?;
                if let Some(id)=layer.get("layer_id").and_then(Value::as_str){
                    layer_state=layer_state.checked_add(id.len()+64).ok_or(ItemRefusal::Budget)?;
                    if state.state.checked_add(transient).and_then(|n|n.checked_add(layer_state))
                        .is_none_or(|n|n>limits.max_state_bytes){return Err(ItemRefusal::Budget)}
                    if !layer_ids.insert(id){state.issue(path,"duplicate-layer-id",id)?;}
                }
                let location=format!("{path}#layer_assessments/{}",ordinal+1);
                source_refs(cut,&location,layer,&mut state,cancelled)?;
            }
        }
    }
    let carrier_membership=stream.coverage().ok_or_else(||ItemRefusal::Source("rights source EOF incomplete".into()))?;
    check(limits.deadline,cancelled)?;
    Ok(SourceRightsReport{revision,carrier_membership,rights_record_count:state.count,
        issues:state.issues,reads:state.reads,missing_authority:vec![
            "current-owner-grant-and-revocation-fence", "read-serving-current-use-recheck"]})
}

fn store_error(error:tos_source_store::StoreError)->ItemRefusal{
    match error.code {tos_source_store::StoreErrorCode::BudgetExceeded=>ItemRefusal::Budget,
        tos_source_store::StoreErrorCode::UnsupportedFormat|tos_source_store::StoreErrorCode::UnsupportedPlatform=>ItemRefusal::Unsupported(error.to_string()),
        _=>ItemRefusal::Source(error.to_string())}
}
