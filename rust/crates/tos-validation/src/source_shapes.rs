//! Current source instance/schema and repository-reference mechanics.
//! Routes derive from exact selected schema roots; ambiguity or an unknown
//! version remains an explicit gap. A schema result never covers other rules.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use serde_json::Value;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
use crate::{KeyState, PredicateRead};

#[derive(Debug,Clone)]
pub struct SourceShapeReport {
    pub revision:SourceRevision,
    pub carrier_membership:SourceMembershipV1,
    pub checked_instances:u64,
    pub issues:Vec<(String,String)>,
    pub unsupported:Vec<(String,String)>,
    pub reads:Vec<PredicateRead>,
}
struct State {limits:ItemLimits,bytes:u64,state:usize,issues:Vec<(String,String)>,gaps:Vec<(String,String)>,reads:Vec<PredicateRead>}
impl State {
    fn reserve(&mut self,n:usize)->Result<(),ItemRefusal>{self.state=self.state.checked_add(n).filter(|n|*n<=self.limits.max_state_bytes).ok_or(ItemRefusal::Budget)?;Ok(())}
    fn raw(&mut self,n:usize)->Result<(),ItemRefusal>{self.bytes=self.bytes.checked_add(n as u64).filter(|n|*n<=self.limits.max_total_bytes).ok_or(ItemRefusal::Budget)?;if n>self.limits.max_member_bytes{Err(ItemRefusal::Budget)}else{Ok(())}}
    fn issue(&mut self,path:&str,code:&str)->Result<(),ItemRefusal>{if self.issues.len()>=self.limits.max_issues{return Err(ItemRefusal::Budget)}self.reserve(path.len()+code.len()+64)?;self.issues.push((path.into(),code.into()));Ok(())}
    fn gap(&mut self,path:&str,code:&str)->Result<(),ItemRefusal>{self.reserve(path.len()+code.len()+64)?;self.gaps.push((path.into(),code.into()));Ok(())}
    fn read(&mut self,read:PredicateRead)->Result<(),ItemRefusal>{let bytes=match &read{PredicateRead::ExactPath{path,digest}=>path.len()+digest.len(),PredicateRead::RefEndpoint{endpoint_type,id,..}=>endpoint_type.len()+id.len(),_=>return Err(ItemRefusal::Unsupported("source-shape read accounting".into()))};self.reserve(bytes+96)?;self.reads.push(read);Ok(())}
}
fn check(limits:ItemLimits,cancelled:&AtomicBool)->Result<(),ItemRefusal>{if cancelled.load(Ordering::Relaxed)||Instant::now()>=limits.deadline{Err(ItemRefusal::Deadline)}else if limits.max_member_bytes==0||limits.max_total_bytes==0||limits.max_state_bytes==0||limits.max_issues==0{Err(ItemRefusal::Budget)}else{Ok(())}}

/// Extract only root instance version constraints, following same-document
/// root $refs/allOf/anyOf/oneOf. Do not harvest unrelated nested object/Claim
/// schema_version properties and turn them into an instance routing registry.
fn root_versions(root:&Value,node:&Value,pending:&mut BTreeSet<String>,out:&mut BTreeSet<String>,depth:usize)->Result<(),ItemRefusal>{
    if depth>64{return Err(ItemRefusal::Unsupported("schema root route depth".into()))}
    if let Some(version)=node.pointer("/properties/schema_version/const").and_then(Value::as_str){out.insert(version.into());}
    if let Some(versions)=node.pointer("/properties/schema_version/enum").and_then(Value::as_array){out.extend(versions.iter().filter_map(Value::as_str).map(str::to_owned));}
    if let Some(reference)=node.get("$ref").and_then(Value::as_str){
        if reference.starts_with('#')&&pending.insert(reference.into()){
            let pointer=&reference[1..];let target=if pointer.is_empty(){Some(root)}else{root.pointer(pointer)};
            if let Some(target)=target{root_versions(root,target,pending,out,depth+1)?;}
        }
    }
    for keyword in ["allOf","anyOf","oneOf"]{if let Some(branches)=node.get(keyword).and_then(Value::as_array){for branch in branches{root_versions(root,branch,pending,out,depth+1)?;}}}
    Ok(())
}

pub fn inspect_source_shapes_from_cut(cut:&CorpusCutReader,limits:ItemLimits,cancelled:&AtomicBool,
    schemas:&mut CutWorkerSchemaExecutor)->Result<SourceShapeReport,ItemRefusal>{
    check(limits,cancelled)?;let revision=cut.current().revision();
    if schemas.source_revision()!=revision{return Err(ItemRefusal::Source("shape worker belongs to another source cut".into()))}
    let mut state=State{limits,bytes:0,state:0,issues:Vec::new(),gaps:Vec::new(),reads:Vec::new()};
    let mut by_version:BTreeMap<String,BTreeSet<String>>=BTreeMap::new();let mut by_uri=BTreeMap::new();
    for metadata in cut.current().members(){
        check(limits,cancelled)?;let path=metadata.path.as_str();
        if !path.starts_with("ToS/contracts/")||!path.ends_with(".schema.json"){continue}
        let member=cut.read_member(revision,&metadata.path,limits.max_member_bytes as u64,limits.deadline,cancelled).map_err(store_error)?;
        state.raw(member.raw.len())?;
        let schema=crate::published_value(&member.raw,limits.max_member_bytes).map_err(|error|ItemRefusal::Unsupported(format!("source schema route: {error:?}")))?;
        let uri=schema.get("$id").and_then(Value::as_str).ok_or_else(||ItemRefusal::Unsupported("source schema route ID".into()))?;
        state.reserve(uri.len()+path.len()+128)?;
        if by_uri.insert(uri.to_owned(),path.to_owned()).is_some(){return Err(ItemRefusal::Unsupported("duplicate source schema route ID".into()))}
        let mut versions=BTreeSet::new();root_versions(&schema,&schema,&mut BTreeSet::new(),&mut versions,0)?;
        for version in versions{state.reserve(version.len()+path.len()+128)?;by_version.entry(version).or_default().insert(path.into());}
    }
    let mut stream=cut.stream(revision).map_err(store_error)?;let mut checked_instances=0u64;
    while let Some(member)=stream.next_member(limits.deadline,cancelled).map_err(store_error)?{
        check(limits,cancelled)?;state.raw(member.raw.len())?;let path=member.path.as_str();
        if !(path.starts_with("ToS/source-witnesses/")||path.starts_with("ToS/research-packets/"))||!path.ends_with(".json"){continue}
        let value=match serde_json::from_slice::<Value>(&member.raw){Ok(value)=>value,Err(_)=>{state.issue(path,"invalid-json")?;continue}};
        if !value.is_object(){state.gap(path,"non-object-source-owner-route")?;continue}
        state.read(PredicateRead::ExactPath{path:path.into(),digest:Digest256::of_bytes(&member.raw).to_hex()})?;
        let version=value.get("schema_version").and_then(Value::as_str);
        let explicit=value.get("$schema").and_then(Value::as_str).and_then(|uri|by_uri.get(uri));
        let route=explicit.cloned().or_else(||version.and_then(|version|by_version.get(version)).filter(|routes|routes.len()==1).and_then(|routes|routes.first().cloned()));
        if let Some(contract)=route{
            if !schemas.check(path,&member.raw,&contract,limits.deadline,cancelled)?{state.issue(path,"source-owner-schema")?;}
            checked_instances=checked_instances.checked_add(1).ok_or(ItemRefusal::Budget)?;
        }else{state.gap(path,if version.is_some(){"unknown-or-ambiguous-source-schema-route"}else{"source-owner-without-root-schema-version"})?;}
        let refs=["source_refs","source_record_refs","receipt_refs"].into_iter().flat_map(|field|value.get(field).and_then(Value::as_array).into_iter().flatten())
            .chain(["rights_ref","provenance_ref","forensic_report_ref","resource_inventory_ref","generated_from_manifest_ref","item_manifest_ref"].into_iter().filter_map(|field|value.get(field)));
        for reference in refs{check(limits,cancelled)?;let Some(reference)=reference.as_str()else{state.issue(path,"unresolved-source-ref")?;continue};if !reference.starts_with("ToS/"){continue}
            let relative=RelativePath::parse(reference).map_err(|_|ItemRefusal::Unsupported("source-shape reference path".into()))?;
            let present=cut.presence(revision,&relative).is_some();state.read(PredicateRead::RefEndpoint{endpoint_type:"source-path".into(),id:reference.into(),observed:if present{KeyState::Present}else{KeyState::Absent}})?;
            if !present{state.issue(path,"unresolved-source-ref")?;}
        }
    }
    let carrier_membership=stream.coverage().ok_or_else(||ItemRefusal::Source("source shape EOF incomplete".into()))?;
    check(limits,cancelled)?;
    Ok(SourceShapeReport{revision,carrier_membership,checked_instances,issues:state.issues,unsupported:state.gaps,reads:state.reads})
}
fn store_error(error:tos_source_store::StoreError)->ItemRefusal{match error.code{tos_source_store::StoreErrorCode::BudgetExceeded=>ItemRefusal::Budget,tos_source_store::StoreErrorCode::UnsupportedFormat|tos_source_store::StoreErrorCode::UnsupportedPlatform=>ItemRefusal::Unsupported(error.to_string()),_=>ItemRefusal::Source(error.to_string())}}
