//! Executable source-owned Claim and bibliographic closure families.
//! All source bytes come from the immutable cut; schemas execute through the
//! existing bounded worker. Reconstructed historical compounds remain mechanical
//! observations and never confer current publication or source admission.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use serde_json::{Value, json};
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_source_store::{CorpusCutReader, SourceMembershipV1};
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::record_biblio_cut::{BiblioCurrentRecord, SourceCutRecordReport, account, check, current, reserve, store_error};
use crate::relation_rules::{RelationIssue, RelationShadow, inspect_current_topology};
use crate::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
use crate::{KeyState, PredicateRead, ValidationFact};

const ENTITY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const LEGACY_BASE: &str = "ToS/contracts/claim-packet.schema.json";
const BASE: &str = "ToS/contracts/source-claim-record.schema.json";
const LEGACY_TOPOLOGY: [&str;3] = ["work-expression-claims.jsonl", "expression-edition-claims.jsonl", "edition-item-claims.jsonl"];
const TOPOLOGY_EVENT: &str = "tos.event.annotation.source-witness-bibliographic-topology.2026-07-31";
const CHRONOLOGY_EVENT: &str = "tos.event.annotation.friedrich-nietzsche.first-publication-chronology.2026-07-31";
const DERIVATION_EVENT: &str = "tos.event.annotation.expression-derivation.antonovsky-revision-lineage.2026-08-01";
const RESPONSIBILITY: [&str;6] = ["authored_by", "contributed_by", "translated_by", "edited_by", "afterword_by", "designed_by"];

#[derive(Debug, Clone)]
pub struct BiblioClaim {
    pub path: String,
    pub line: usize,
    pub value: Value,
    pub raw_sha256: String,
    pub native: bool,
}
#[derive(Debug)]
pub struct SourceCutBiblioReport {
    pub source_revision: SourceRevision,
    pub carrier_membership: SourceMembershipV1,
    pub shadow: RelationShadow,
    /// Immutable current rows; retained rows never join this namespace.
    pub claims: Vec<BiblioClaim>,
    pub bytes_read: u64,
    pub native_compounds: Vec<crate::native_compound::NativeCompoundObservation>,
}
struct Route<'a> { reader: &'a str, domain: Vec<String>, range: Vec<String>, layers: Vec<String>, versions: BTreeMap<&'a str,&'a str>, profile: &'a Value }
struct Rules<'a> { limits: ItemLimits, cancelled: &'a AtomicBool, state: usize, ancestry_state: usize, bytes: u64, anchors: BTreeSet<String>, reserved: BTreeSet<String>, schema_seen: BTreeSet<String>, shadow: RelationShadow }
impl Rules<'_> {
    fn issue(&mut self, code: &'static str, location: &str) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        if self.shadow.issues.len() >= self.limits.max_issues { return Err(ItemRefusal::BudgetCheck {check:"bibliography issue count",used:(self.shadow.issues.len() as u64).checked_add(1),limit:Some(self.limits.max_issues as u64)}); }
        reserve(&mut self.state, location.len()+std::mem::size_of::<RelationIssue>(), self.limits.max_state_bytes)?;
        self.shadow.issues.push(RelationIssue { code, location: location.into() }); Ok(())
    }
    fn read(&mut self, row: PredicateRead) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        reserve(&mut self.state, crate::record_biblio_cut::predicate_state(&row)?, self.limits.max_state_bytes)?;
        self.shadow.reads.push(row); Ok(())
    }
    fn skip(&mut self, reason: &str) -> Result<(), ItemRefusal> {
        if !self.shadow.skipped_profiles.contains(reason) {reserve(&mut self.state, reason.len()+std::mem::size_of::<String>(), self.limits.max_state_bytes)?;}
        self.shadow.unsupported = true; self.shadow.skipped_profiles.insert(reason.into()); Ok(())
    }
    fn checked(&mut self,profile:&str)->Result<(),ItemRefusal> {
        if !self.shadow.checked_profiles.contains(profile) {reserve(&mut self.state,profile.len()+std::mem::size_of::<String>(),self.limits.max_state_bytes)?;self.shadow.checked_profiles.insert(profile.into());}Ok(())
    }
    fn endpoint(&mut self, id: &str, allowed: &[String], types: &BTreeMap<&str,&Value>, kinds: &BTreeMap<&str,&str>, records: &BTreeMap<String,BiblioCurrentRecord>, location: &str) -> Result<(),ItemRefusal> {
        if self.reserved.contains(id) && !records.contains_key(id) {
            self.read(PredicateRead::RefEndpoint { endpoint_type:allowed.join("|"),id:id.into(),observed:KeyState::Reserved })?;
            self.skip("native-semantic-packet-endpoint-version-owner")?; return Ok(());
        }
        let actual = records.get(id).and_then(|r| kinds.get(r.kind.as_str()));
        self.read(PredicateRead::RefEndpoint { endpoint_type: allowed.join("|"), id: id.into(), observed: if actual.is_some() { KeyState::Present } else { KeyState::Absent } })?;
        reserve_check(self.state,self.ancestry_state,self.limits.max_state_bytes)?;
        if !actual.is_some_and(|actual| ancestor(types, actual, allowed)) { self.issue("claim-endpoint-kind-or-missing", location)?; } Ok(())
    }
}
fn strings(row: &Value, field: &str) -> Vec<String> { row.get(field).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default() }
fn s<'a>(row: &'a Value, field: &str) -> Option<&'a str> { row.get(field).and_then(Value::as_str) }
fn ancestor(types: &BTreeMap<&str,&Value>, actual: &str, allowed: &[String]) -> bool {
    let mut pending=vec![actual]; let mut visited=BTreeSet::new();
    while let Some(id)=pending.pop() { if !visited.insert(id) { continue; } if allowed.iter().any(|allowed|allowed==id) { return true; } if let Some(row)=types.get(id) { pending.extend(row["parent_type_ids"].as_array().into_iter().flatten().filter_map(Value::as_str)); } }
    false
}
fn reserve_check(state:usize,amount:usize,limit:usize)->Result<(),ItemRefusal> {
    let used=state.checked_add(amount).ok_or(ItemRefusal::Budget)?;
    if used>limit {return Err(ItemRefusal::BudgetCheck{check:"bibliography simultaneous logical state",used:Some(used as u64),limit:Some(limit as u64)});}Ok(())
}
fn strict_decoded(raw:&[u8],rules:&Rules<'_>)->Result<(Value,usize),ItemRefusal> {
    let available=rules.limits.max_state_bytes.checked_sub(rules.state).ok_or(ItemRefusal::Budget)?;
    let codec=tos_foundation::JsonLimits::new(rules.limits.max_member_bytes,64,300_000,4_300).map_err(|_|ItemRefusal::Budget)?;
    crate::record_biblio_cut::bounded_decoded_state(raw,codec,available,rules.limits.deadline,rules.cancelled)
}
fn legacy_decoded(raw:&[u8],rules:&Rules<'_>)->Result<(Value,usize),ItemRefusal> {
    crate::record_biblio_cut::bounded_legacy_decoded_state(raw,rules.limits.max_member_bytes,rules.limits.max_state_bytes.checked_sub(rules.state).ok_or(ItemRefusal::Budget)?,rules.limits.deadline,rules.cancelled)
}
fn fact_state(fact:&ValidationFact)->Result<usize,ItemRefusal> {
    std::mem::size_of::<ValidationFact>().checked_add(fact.namespace.len()).and_then(|n|n.checked_add(fact.key.len())).and_then(|n|n.checked_add(fact.value_digest.len())).ok_or(ItemRefusal::Budget)
}
fn owned(path: &str) -> bool { path.starts_with("ToS/source-witnesses/") && !path.split('/').any(|p| matches!(p,"catalog"|"owner-local"|"payload"|"local-content")) }
fn claim_stream(path: &str) -> bool { owned(path) && path.ends_with("-claims.jsonl") }

/// Requires record-family output from the same exact current cut. EOF is
/// checked again for this family's Claim/event/Item-manifest traversal.
pub fn inspect_bibliography_from_cut(cut: &CorpusCutReader, records: &SourceCutRecordReport, limits: ItemLimits, cancelled: &AtomicBool, schemas: &mut CutWorkerSchemaExecutor) -> Result<SourceCutBiblioReport, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() { return Err(ItemRefusal::Source("bibliography schema cut mismatch".into())); }
    if records.source_revision != cut.current().revision() { return Err(ItemRefusal::Source("bibliography record cut mismatch".into())); }
    let mut rules=Rules { limits, cancelled, state:std::mem::size_of::<Rules<'_>>(), ancestry_state:0, bytes:0, anchors:BTreeSet::new(), reserved:records.observations.iter().filter_map(|r|match r {crate::record_rules::RecordObservation::NativeReservation {id,..}=>Some(id.clone()),_=>None}).collect(), schema_seen:BTreeSet::new(), shadow:RelationShadow::default() };
    reserve(&mut rules.state,rules.reserved.iter().map(|id|id.len()+std::mem::size_of::<String>()).sum(),limits.max_state_bytes)?;
    schema_read(cut,BASE,&mut rules)?;
    let mut registries=Vec::new();
    for (path, contract) in [(ENTITY,"ToS/contracts/semantic-entity-type-registry.schema.json"),(RELATION,"ToS/contracts/semantic-relation-type-registry.schema.json")] {
        let raw=current(cut,path,limits,cancelled,&mut rules.bytes)?;
        reserve(&mut rules.state,raw.len()+std::mem::size_of::<Vec<u8>>(),limits.max_state_bytes)?;
        if !schemas.check(path,&raw,contract,limits.deadline,cancelled)? { return Err(ItemRefusal::Unsupported(format!("invalid bibliography registry {path}"))); }
        schema_read(cut,contract,&mut rules)?;
        let (value,value_state)=strict_decoded(&raw,&rules)?;
        reserve(&mut rules.state,value_state,limits.max_state_bytes)?;
        rules.read(PredicateRead::Registry { uri:path.into(), version:value["registry_version"].to_string(), digest:Digest256::of_bytes(&raw).to_prefixed() })?;
        registries.push(value);
        rules.state-=raw.len()+std::mem::size_of::<Vec<u8>>();
        drop(raw);
    }
    let mut types=BTreeMap::new(); let mut kinds=BTreeMap::new(); let mut routes=BTreeMap::new();
    for entry in registries[0]["types"].as_array().ok_or_else(|| ItemRefusal::Unsupported("entity registry types".into()))? {
        check(limits.deadline,cancelled)?;
        let id=s(entry,"type_id").ok_or_else(|| ItemRefusal::Unsupported("entity type ID".into()))?;
        if !types.contains_key(id) {reserve(&mut rules.state,std::mem::size_of::<(&str,&Value)>(),limits.max_state_bytes)?;}
        if types.insert(id,entry).is_some() { rules.issue("duplicate-entity-type",id)?; }
        if let Some(mappings)=entry["source_mappings"].as_array() { for mapping in mappings { if s(mapping,"source_graph")==Some("source-claims") { if let Some(kind)=s(mapping,"source_kind_id") { if !kinds.contains_key(kind) {reserve(&mut rules.state,std::mem::size_of::<(&str,&str)>(),limits.max_state_bytes)?;} if kinds.insert(kind,id).is_some() { rules.issue("duplicate-kind-owner",kind)?; } } } } }
    }
    for entry in registries[1]["relations"].as_array().ok_or_else(|| ItemRefusal::Unsupported("relation registry relations".into()))? {
        check(limits.deadline,cancelled)?;
        let Some(profile)=entry.get("source_claim_profile") else { continue; };
        let mut mappings=entry["source_mappings"].as_array().into_iter().flatten().filter(|m| s(m,"source_graph")==Some("source-claims") && s(m,"scope")==Some("claim-predicate"));
        let mapping=mappings.next();
        if mapping.is_none() || mappings.next().is_some() || entry["abstract"]!=false || s(entry,"assertion_mode")!=Some("reified-claim") || entry["evidence_required"]!=true { return Err(ItemRefusal::Unsupported("ambiguous Claim profile owner".into())); }
        let predicate=s(mapping.unwrap(),"source_predicate_id").ok_or_else(|| ItemRefusal::Unsupported("Claim predicate route".into()))?;
        let reader=s(profile,"reader").ok_or_else(|| ItemRefusal::Unsupported("Claim reader route".into()))?;
        let mut versions=BTreeMap::new();
        for schema in profile["schemas"].as_array().ok_or_else(|| ItemRefusal::Unsupported("Claim schema routes".into()))? {
            let version=s(schema,"schema_version").ok_or_else(|| ItemRefusal::Unsupported("Claim schema version".into()))?;
            let path=s(schema,"schema_ref").ok_or_else(|| ItemRefusal::Unsupported("Claim schema path".into()))?;
            reserve(&mut rules.state,std::mem::size_of::<(&str,&str)>(),limits.max_state_bytes)?;
            if versions.insert(version,path).is_some() { return Err(ItemRefusal::Unsupported("duplicate Claim schema route".into())); }
            schema_read(cut,path,&mut rules)?;
            for dependency in strings(schema,"schema_dependencies") { schema_read(cut,&dependency,&mut rules)?; }
            reserve(&mut rules.state,predicate.len()+1+version.len()+std::mem::size_of::<String>(),limits.max_state_bytes)?;
            rules.shadow.declared_profiles.insert(format!("{predicate}@{version}"));
        }
        let domain=strings(entry,"domain_type_ids");let range=strings(entry,"range_type_ids");let layers=strings(profile,"assertion_layers");
        let payload=domain.iter().chain(&range).chain(&layers).try_fold(0usize,|n,s|n.checked_add(std::mem::size_of::<String>()+s.len())).ok_or(ItemRefusal::Budget)?;
        reserve(&mut rules.state,std::mem::size_of::<(&str,Route<'_>)>()+payload,limits.max_state_bytes)?;
        if routes.insert(predicate,Route { reader,domain,range,layers,versions,profile }).is_some() { return Err(ItemRefusal::Unsupported("duplicate Claim predicate owner".into())); }
    }
    let links=types.values().try_fold(0usize,|n,row|n.checked_add(row["parent_type_ids"].as_array().map_or(0,Vec::len))).ok_or(ItemRefusal::Budget)?;
    rules.ancestry_state=types.len().checked_mul(std::mem::size_of::<&str>()).and_then(|n|n.checked_add(links.checked_add(1)?.checked_mul(std::mem::size_of::<&str>())?)).ok_or(ItemRefusal::Budget)?;
    let mut claims=Vec::new(); let mut events=BTreeMap::new(); let mut item_editions=BTreeMap::new(); let mut stream=cut.stream(cut.current().revision()).map_err(store_error)?;
    while let Some(member)=stream.next_member(limits.deadline,cancelled).map_err(store_error)? {
        check(limits.deadline,cancelled)?; account(&mut rules.bytes,member.raw.len(),limits.max_total_bytes)?;
        if member.raw.len()>limits.max_member_bytes { return Err(ItemRefusal::BudgetCheck {check:"bibliography member bytes",used:Some(member.raw.len() as u64),limit:Some(limits.max_member_bytes as u64)}); }
        let path=member.path.as_str();
        if !owned(path) { continue; }
        let member_state=member.stable_ids.iter().try_fold(std::mem::size_of_val(&member)+member.raw.len()+path.len(),|n,id|n.checked_add(std::mem::size_of::<String>()+id.len())).ok_or(ItemRefusal::Budget)?;
        reserve(&mut rules.state,member_state,limits.max_state_bytes)?;
        if path.ends_with("/item.manifest.json") {
            let (value,value_state)=legacy_decoded(&member.raw,&rules)?;
            let ceiling=rules.limits.max_state_bytes;rules.limits.max_state_bytes=ceiling.checked_sub(value_state).ok_or(ItemRefusal::Budget)?;
            if let (Some(id),Some(edition))=(s(&value,"item_id"),s(&value,"embodiment_ref")) {
                reserve(&mut rules.state,id.len()+edition.len()+std::mem::size_of::<(String,String)>(),rules.limits.max_state_bytes)?;
                if item_editions.insert(id.to_owned(),edition.to_owned()).is_some() { rules.issue("duplicate-manifest-item",path)?; }
            }
            drop(value);rules.limits.max_state_bytes=ceiling;
        }
        if !path.ends_with(".jsonl") { rules.state-=member_state;continue; }
        rules.read(PredicateRead::ExactPath { path:path.into(),digest:Digest256::of_bytes(&member.raw).to_prefixed() })?;
        for (index,line) in member.raw.split(|b| *b==b'\n').enumerate() {
            check(limits.deadline,cancelled)?;
            if line.iter().all(u8::is_ascii_whitespace) { continue; }
            let (value,value_state)=if path.ends_with("/source-claims.jsonl") {strict_decoded(line,&rules)?} else {legacy_decoded(line,&rules)?};
            reserve(&mut rules.state,value_state,limits.max_state_bytes)?;
            if path.ends_with("/anchors.jsonl") { if let Some(id)=s(&value,"anchor_id") { if !rules.anchors.contains(id) {reserve(&mut rules.state,id.len()+std::mem::size_of::<String>(),limits.max_state_bytes)?;} if !rules.anchors.insert(id.into()) { rules.issue("duplicate-biblio-anchor",path)?; } } }
            if claim_stream(path) {
                if claims.len()>=65_536 { return Err(ItemRefusal::Budget); }
                reserve(&mut rules.state,path.len()+64+std::mem::size_of::<BiblioClaim>()-std::mem::size_of::<Value>(),limits.max_state_bytes)?;
                claims.push(BiblioClaim { path:path.into(),line:index+1,value,raw_sha256:Digest256::of_bytes(&member.raw).to_hex(),native:path.ends_with("/source-claims.jsonl") });
            } else if let Some(id)=s(&value,"event_id") {
                let contract=match s(&value,"schema_version") {Some("tos_provenance_event_v1")=>"ToS/contracts/provenance-event.schema.json",Some("tos_provenance_event_v2")=>"ToS/contracts/provenance-event-v2.schema.json",_=>return Err(ItemRefusal::Unsupported(format!("unknown bibliography event profile {path}:{}",index+1)))};
                schema_read(cut,contract,&mut rules)?;
                if !schemas.check(&format!("{path}:{}",index+1),line,contract,limits.deadline,cancelled)? {rules.issue("bibliography-event-schema",path)?;}
                if s(&value,"schema_version")==Some("tos_provenance_event_v2") {for code in crate::provenance_rules::semantic_issues(&value,limits.max_issues,limits.deadline)? {rules.issue(code,path)?;}}

                let id=id.to_owned();
                reserve(&mut rules.state,id.len()+std::mem::size_of::<(String,Value)>()-std::mem::size_of::<Value>(),limits.max_state_bytes)?;
                let replaced=events.contains_key(&id);let key_cost=id.len()+std::mem::size_of::<(String,Value)>();
                if let Some(previous)=events.insert(id,value) {debug_assert!(replaced); rules.state-=crate::record_biblio_cut::decoded_state(&previous)?-std::mem::size_of::<Value>()+key_cost;rules.issue("duplicate-biblio-event",path)?; }
            } else {drop(value);rules.state-=value_state;}
        }
        rules.state-=member_state;
    }
    let membership=stream.coverage().ok_or_else(|| ItemRefusal::Source("bibliography EOF missing".into()))?;
    if membership!=records.current_membership { return Err(ItemRefusal::Source("bibliography membership mismatch".into())); }
    rules.shadow.observed_endpoints=records.records.len(); rules.shadow.observed_claims=claims.len();
    rules.read(PredicateRead::Prefix { namespace:"source-current-Claim-files".into(),prefix:"ToS/source-witnesses/".into(),generation:membership.digest.to_prefixed() })?;
    let mut identities=BTreeSet::new(); let mut topology=Vec::new();let mut topology_state=0usize;
    // Only the consumed Work->Expression, Expression->Edition and Edition->Item owners are reconstructed here. One
    // invocation shares the exact read/index budget across all its Claims.
    let mut verified_native=BTreeSet::new();let mut native_compounds=Vec::new();
    if claims.iter().any(|claim|claim.native && matches!(s(&claim.value,"predicate"),Some("has_expression"|"embodied_by"|"exemplified_by"))) {
        let mut compound_limits=limits;
        compound_limits.max_state_bytes=limits.max_state_bytes.checked_sub(rules.state).ok_or(ItemRefusal::Budget)?;
        compound_limits.max_total_bytes=limits.max_total_bytes.checked_sub(rules.bytes).ok_or(ItemRefusal::Budget)?;
        let mut compounds=crate::native_compound::NativeCompoundReader::new(cut,compound_limits,cancelled)?;
        for claim in claims.iter().filter(|c|c.native && matches!(s(&c.value,"predicate"),Some("has_expression"|"embodied_by"|"exemplified_by"))) {
            let location=format!("{}:{}",claim.path,claim.line);
            compounds.set_remaining_state(limits.max_state_bytes.checked_sub(rules.state).ok_or(ItemRefusal::Budget)?)?;
            let inspected=compounds.verify(&claim.path,&claim.value,schemas);
            // The cache remains alive while issues, observations and reads grow.
            // Both concrete owners consume the same existing family envelope.
            rules.limits.max_state_bytes=limits.max_state_bytes.checked_sub(compounds.retained_state_bytes()).ok_or(ItemRefusal::Budget)?;
            match inspected {
                Ok(observation)=>{
                    let id=s(&claim.value,"claim_id").ok_or_else(||ItemRefusal::Source("compound Claim identity missing".into()))?;
                    reserve(&mut rules.state,observation.claim_id.len()+observation.claim_path.len()+observation.transaction_id.len()+observation.manifest_sha256.len()+std::mem::size_of::<crate::native_compound::NativeCompoundObservation>(),rules.limits.max_state_bytes)?;
                    match observation.transport {
                        crate::native_compound::NativeTransportState::Committed=>{if !verified_native.contains(id) {reserve(&mut rules.state,id.len()+std::mem::size_of::<String>(),rules.limits.max_state_bytes)?;}verified_native.insert(id.to_owned());let profile=match s(&claim.value,"predicate") {Some("has_expression")=>"native-work-expression-exact-compound-plan-and-current-lineage@1",Some("embodied_by")=>"native-expression-edition-exact-compound-plan-and-current-lineage@1",_=>"native-edition-item-exact-compound-plan-and-current-lineage@1"};rules.checked(profile)?;},
                        crate::native_compound::NativeTransportState::Pending=>rules.issue(match s(&claim.value,"predicate") {Some("has_expression")=>"native-work-expression-transaction-pending",Some("embodied_by")=>"native-expression-edition-transaction-pending",_=>"native-edition-item-transaction-pending"},&location)?,
                        crate::native_compound::NativeTransportState::RolledBack=>rules.issue(match s(&claim.value,"predicate") {Some("has_expression")=>"native-work-expression-transaction-rolled-back",Some("embodied_by")=>"native-expression-edition-transaction-rolled-back",_=>"native-edition-item-transaction-rolled-back"},&location)?,
                        crate::native_compound::NativeTransportState::Orphan=>rules.issue(match s(&claim.value,"predicate") {Some("has_expression")=>"native-work-expression-transaction-orphan",Some("embodied_by")=>"native-expression-edition-transaction-orphan",_=>"native-edition-item-transaction-orphan"},&location)?,
                    }
                    rules.read(PredicateRead::ExactBytes {locator:format!("transaction:{}",observation.transaction_id),digest:observation.manifest_sha256.clone()})?;
                    native_compounds.push(observation);
                },
                Err(ItemRefusal::Source(_))=>rules.issue(match s(&claim.value,"predicate") {Some("has_expression")=>"native-work-expression-compound-evidence",Some("embodied_by")=>"native-expression-edition-compound-evidence",_=>"native-edition-item-compound-evidence"},&location)?,
                Err(ItemRefusal::Unsupported(reason))=>rules.skip(&format!("native-bibliographic-compound:{reason}"))?,
                Err(error)=>return Err(error),
            }
        }
        let (bytes,mut reads)=compounds.finish();
        account(&mut rules.bytes,usize::try_from(bytes).map_err(|_|ItemRefusal::Budget)?,limits.max_total_bytes)?;
        // Transfer owned read strings once; retain the old Vec allocation in
        // the same ceiling until append releases its elements and we drop it.
        let old_buffer=reads.capacity().checked_mul(std::mem::size_of::<PredicateRead>()).ok_or(ItemRefusal::Budget)?;
        let mut read_state=0usize;
        for read in &reads {
            check(limits.deadline,cancelled)?;
            read_state=read_state.checked_add(crate::record_biblio_cut::predicate_state(read)?).ok_or(ItemRefusal::Budget)?;
        }
        reserve(&mut rules.state,read_state,limits.max_state_bytes.checked_sub(old_buffer).ok_or(ItemRefusal::Budget)?)?;
        rules.shadow.reads.append(&mut reads);drop(reads);
        rules.limits.max_state_bytes=limits.max_state_bytes;
    }
    for claim in &claims {
        inspect_claim(cut,claim,&routes,&types,&kinds,&records.records,&events,schemas,&mut rules)?;
        if let Some(id)=s(&claim.value,"claim_id") {
            if !identities.contains(id) {reserve(&mut rules.state,id.len()+std::mem::size_of::<String>(),limits.max_state_bytes)?;}
            if !identities.insert(id.to_owned()) { rules.issue("duplicate-claim-id",&format!("{}:{}",claim.path,claim.line))?; }
            rules.read(PredicateRead::UniqueKey { namespace:"source-claim-id".into(),key:id.into(),owner:format!("{}:{}",claim.path,claim.line) })?;
        }
        if matches!(s(&claim.value,"predicate"),Some("has_expression"|"embodied_by"|"exemplified_by")) { let cost=crate::record_biblio_cut::decoded_state(&claim.value)?;reserve(&mut rules.state,cost,limits.max_state_bytes)?;topology_state=topology_state.checked_add(cost).ok_or(ItemRefusal::Budget)?;topology.push(claim.value.clone()); }
    }
    // The existing pure topology owner already covers declared forward/reverse
    // refs, endpoint pairs, multiplicity and Item-manifest edition agreement.
    // Native compounds must be verified before this owner API may claim a
    // verified union. A mixed union therefore refuses topology completeness.
    if records.records.len()>65_536 || topology.len()>65_536 { return Err(ItemRefusal::Budget); }
    let mut values=Vec::new();let mut values_state=0usize;
    for record in records.records.values() {let cost=crate::record_biblio_cut::decoded_state(&record.value)?;reserve(&mut rules.state,cost,limits.max_state_bytes)?;values_state=values_state.checked_add(cost).ok_or(ItemRefusal::Budget)?;values.push(record.value.clone());}
    let native_topology=claims.iter().any(|claim|claim.native && matches!(s(&claim.value,"predicate"),Some("has_expression"|"embodied_by"|"exemplified_by")) && !s(&claim.value,"claim_id").is_some_and(|id|verified_native.contains(id)));
    let topology_report=inspect_current_topology(&values,&topology,&item_editions,!native_topology,&membership.digest.to_prefixed());
    drop(values);drop(topology);rules.state-=values_state+topology_state;
    check(limits.deadline,cancelled)?;
    merge_shadow(&mut rules,topology_report)?;
    inspect_closure(&records.records,&claims,&membership.digest.to_prefixed(),&mut rules)?;
    inspect_batches(&claims,&events,&records.records,&mut rules)?;
    // No complete-source verdict escapes this family. Other native compounds
    // and retained profile execution remain explicit missing owner coverage.
    rules.skip("other-native-compound-transaction-reconstruction-and-current-parent-lineage")?;
    rules.skip("retained-frozen-profile-source-admission")?;
    check(limits.deadline,cancelled)?;
    Ok(SourceCutBiblioReport { source_revision:cut.current().revision(),carrier_membership:membership,shadow:rules.shadow,claims,bytes_read:rules.bytes,native_compounds })
}

fn schema_read(cut:&CorpusCutReader,path:&str,rules:&mut Rules<'_>)->Result<(),ItemRefusal> {
    check(rules.limits.deadline,rules.cancelled)?;
    let relative=RelativePath::parse(path).map_err(|_|ItemRefusal::Unsupported("Claim schema dependency path".into()))?;
    if !rules.schema_seen.insert(path.into()) {return Ok(());}
    reserve(&mut rules.state,path.len()+std::mem::size_of::<String>(),rules.limits.max_state_bytes)?;
    let raw=current(cut,path,rules.limits,rules.cancelled,&mut rules.bytes)?;
    let temporary=raw.len()+std::mem::size_of::<Vec<u8>>()+path.len()+std::mem::size_of::<RelativePath>();
    reserve(&mut rules.state,temporary,rules.limits.max_state_bytes)?;
    let (value,value_state)=strict_decoded(&raw,rules)?;
    reserve(&mut rules.state,value_state,rules.limits.max_state_bytes)?;
    let uri=s(&value,"$id").ok_or_else(||ItemRefusal::Unsupported("source schema dependency ID".into()))?;
    let result=rules.read(PredicateRead::SchemaResource {uri:uri.into(),digest:Digest256::of_bytes(&raw).to_prefixed()});
    drop(value);drop(raw);drop(relative);rules.state-=temporary+value_state;result
}

fn merge_shadow(rules: &mut Rules<'_>, shadow:RelationShadow) -> Result<(),ItemRefusal> {
    if shadow.issue_sink_truncated { return Err(ItemRefusal::Budget); }
    for issue in shadow.issues { rules.issue(issue.code,&issue.location)?; }
    for read in shadow.reads { rules.read(read)?; }
    for fact in shadow.facts { reserve(&mut rules.state,fact_state(&fact)?,rules.limits.max_state_bytes)?; rules.shadow.facts.push(fact); }
    for profile in shadow.skipped_profiles { rules.skip(&profile)?; }
    rules.shadow.checked_profiles.extend(shadow.checked_profiles); rules.shadow.unsupported |= shadow.unsupported; Ok(())
}

fn inspect_claim(cut:&CorpusCutReader, claim:&BiblioClaim, routes:&BTreeMap<&str,Route<'_>>, types:&BTreeMap<&str,&Value>, kinds:&BTreeMap<&str,&str>, records:&BTreeMap<String,BiblioCurrentRecord>, events:&BTreeMap<String,Value>, schemas:&mut impl CutSchemaExecutor, rules:&mut Rules<'_>) -> Result<(),ItemRefusal> {
    let raw_cost=crate::record_biblio_cut::decoded_wire_size(&claim.value,rules.limits.max_state_bytes.checked_sub(rules.state).ok_or(ItemRefusal::Budget)?)?;
    let bytes=serde_json::to_vec(&claim.value).map_err(|_|ItemRefusal::Unsupported("Claim worker serialization".into()))?;
    let temporary=raw_cost+std::mem::size_of::<Vec<u8>>();
    reserve_check(rules.state,temporary,rules.limits.max_state_bytes)?;
    let ceiling=rules.limits.max_state_bytes;rules.limits.max_state_bytes=ceiling.checked_sub(temporary).ok_or(ItemRefusal::Budget)?;
    let result=inspect_claim_inner(cut,claim,routes,types,kinds,records,events,schemas,rules,&bytes);
    drop(bytes);rules.limits.max_state_bytes=ceiling;result
}
fn inspect_claim_inner(cut:&CorpusCutReader, claim:&BiblioClaim, routes:&BTreeMap<&str,Route<'_>>, types:&BTreeMap<&str,&Value>, kinds:&BTreeMap<&str,&str>, records:&BTreeMap<String,BiblioCurrentRecord>, events:&BTreeMap<String,Value>, schemas:&mut impl CutSchemaExecutor, rules:&mut Rules<'_>,bytes:&[u8]) -> Result<(),ItemRefusal> {
    let row=&claim.value; let location=format!("{}:{}",claim.path,claim.line);
    let basename=claim.path.rsplit('/').next().unwrap_or("");
    let Some(predicate)=s(row,"predicate") else { rules.issue("claim-predicate",&location)?; return Ok(()); };
    if claim.native {
        let Some(route)=routes.get(predicate) else { rules.issue("unrecognized-predicate",&location)?; return Ok(()); };
        let Some(contract)=s(row,"schema_version").and_then(|v|route.versions.get(v)) else { rules.issue("unrecognized-Claim-schema-version",&location)?; return Ok(()); };
        if !schemas.check(&location,&bytes,contract,rules.limits.deadline,rules.cancelled)? || !schemas.check(&location,&bytes,BASE,rules.limits.deadline,rules.cancelled)? { rules.issue("claim-profile-schema",&location)?; return Ok(()); }
        if s(row,"claim_type")!=Some("relation") || s(row,"claim_id")==s(row,"subject_ref") || s(row,"claim_id")==s(row,"object") {rules.issue("Claim-profile-identity",&location)?;}
        if !matches!(s(row,"visibility"),Some("public"|"public_metadata_only")) { rules.issue("claim-public-shape",&location)?; }
        if !s(row,"assertion_layer").is_some_and(|layer|route.layers.iter().any(|v|v==layer)) { rules.issue("claim-assertion-layer",&location)?; }
        if let Some(subject)=s(row,"subject_ref") { rules.endpoint(subject,&route.domain,types,kinds,records,&location)?; } else { rules.issue("claim-subject",&location)?; }
        if matches!(route.reader,"structured-reference-value-v1"|"structured-value-v1"|"identity-transition-v1"|"identity-transition-v2") {
            let raw=serde_json::to_vec(&row["object"]).map_err(|_|ItemRefusal::Unsupported("structured value".into()))?;
            if !schemas.check(&location,&raw,"ToS/contracts/source-structured-value.schema.json",rules.limits.deadline,rules.cancelled)? || s(&row["object"],"kind")!=s(&route.profile,"value_kind") {rules.issue("Claim-shared-structured-value",&location)?;}
        }
        if s(&route.profile["object_reference_set"],"structure_adapter")==Some("scoped-members-v1") {
            let raw=serde_json::to_vec(&row["object"]).map_err(|_|ItemRefusal::Unsupported("member structure".into()))?;
            if !schemas.check(&location,&raw,"ToS/contracts/scoped-member-structure.schema.json",rules.limits.deadline,rules.cancelled)? {rules.issue("Claim-shared-member-structure",&location)?;}
            member_structure(row,rules,&location)?;
        }
        if let Some(display)=row.get("qualifiers").and_then(|q|q.get("display_fields")) {if s(display,"schema_version")==Some("tos_claim_display_fields_v1") {
            let raw=serde_json::to_vec(&row["qualifiers"]).map_err(|_|ItemRefusal::Unsupported("Claim display fields".into()))?;
            if !schemas.check(&location,&raw,"ToS/contracts/claim-display-fields.schema.json",rules.limits.deadline,rules.cancelled)? {rules.issue("Claim-display-fields-schema",&location)?;}
        }}
        if matches!(predicate,"document_catalogue_date"|"document_catalogue_origin"|"document_catalogue_destination") {
            let attribution=&row["qualifiers"]["catalogue_attribution"];
            let field=match predicate {"document_catalogue_date"=>"assigned-date","document_catalogue_origin"=>"origin",_=>"destination"};
            if s(attribution,"field_role")!=Some(field)||!s(attribution,"evidence_ref").is_some_and(|v|strings(row,"evidence_refs").iter().any(|r|r==v))||predicate=="document_catalogue_date" && attribution.get("source_wording")!=row["object"].get("source_wording") {rules.issue("document-catalogue-attribution",&location)?;}
        }
        match route.reader {
            "identity-relation-v1"|"semantic-relation-v1" => {
                if let Some(object)=s(row,"object") { rules.endpoint(object,&route.range,types,kinds,records,&location)?; } else { rules.issue("claim-object-kind",&location)?; }
                rules.checked(&format!("{predicate}@{}",s(row,"schema_version").unwrap_or("")))?;
                // Ordinary domain/range checking does not accept the compound
                // append/revision plan or specialized semantic ownership.
                if matches!(predicate,"contains_work"|"translated_by"|"described_by"|"metadata_at"|"downloadable_at"|"rights_statement_at") {rules.skip(&format!("native-compound-owner-evidence:{predicate}"))?;}
            }
            "structured-reference-value-v1" => {
                let set=&route.profile["object_reference_set"];
                let members=strings(&row["object"],"members");
                let allowed=strings(set,"member_type_ids");
                let mut seen=BTreeSet::new();
                let min=set["min_items"].as_u64().unwrap_or(0); let max=set["max_items"].as_u64().unwrap_or(128);
                if (members.len() as u64)<min || (members.len() as u64)>max { rules.issue("claim-member-count",&location)?; }
                for member in &members {
                    if !seen.insert(member) { rules.issue("claim-member-duplicate",&location)?; }
                    if set["subject_is_member"]==false && s(row,"subject_ref")==Some(member.as_str()) { rules.issue("claim-member-self",&location)?; }
                    rules.endpoint(member,&allowed,types,kinds,records,&location)?;
                }
                if set["subject_is_member"]==true && !s(row,"subject_ref").is_some_and(|v|members.iter().any(|m|m==v)) { rules.issue("claim-subject-not-member",&location)?; }
                rules.skip(&format!("structured-reader-owner-evidence:{predicate}"))?;
            }
            "identity-transition-v1"|"identity-transition-v2" => identity_proposal(row,route.reader,types,kinds,records,rules,&location)?,
            "historical-temporal-v1"|"document-catalogue-temporal-v1" => {
                if s(&row["object"],"kind")==Some("relative-order") {if let Some(anchor)=s(&row["object"]["relative"],"anchor_ref") {rules.endpoint(anchor,&["tos.entity.historical-situation".into()],types,kinds,records,&location)?;}}
                let (contract,root)=if route.reader=="document-catalogue-temporal-v1" {("ToS/contracts/document-catalogue-claim.schema.json","ToS/contracts/document-catalogue-claim.schema.json#/$defs/documentDate")}else{("ToS/contracts/historical-claim.schema.json","ToS/contracts/historical-claim.schema.json#/$defs/historicalDate")};
                schema_read(cut,contract,rules)?;
                let raw=serde_json::to_vec(&row["object"]).map_err(|_|ItemRefusal::Unsupported("historical temporal value".into()))?;
                if !schemas.check(&location,&raw,root,rules.limits.deadline,rules.cancelled)? {rules.issue("Claim-shared-historical-value",&location)?;}
                rules.checked(&format!("{predicate}@{}",s(row,"schema_version").unwrap_or("")))?;
            }
            "structured-value-v1" => {rules.checked(&format!("{predicate}@{}",s(row,"schema_version").unwrap_or("")))?;}
            _ => rules.skip(&format!("source-Claim-reader:{}:{predicate}",route.reader))?,
        }
        if matches!(predicate,"contains_work"|"translated_by") { qualified(row,predicate,rules,&location)?; }
    } else {
        let (contract,subject_kind,object_kinds,expected,role)=match basename {
            "membership-claims.jsonl" => (LEGACY_BASE,"collection",vec!["work"],Some("contains_work"),None),
            "responsibility-claims.jsonl" => (LEGACY_BASE,"",vec!["agent"],None,None),
            "publication-claims.jsonl" => (LEGACY_BASE,"edition",vec![],None,Some("unreviewed-evidence-bearing-publication-claims")),
            "provision-activity-claims.jsonl" => (LEGACY_BASE,"edition",vec![],Some("provision_activity"),Some("unreviewed-evidence-bearing-provision-activity-claims")),
            "work-chronology-claims.jsonl" => (LEGACY_BASE,"work",vec![],Some("first_publication_chronology"),Some("unreviewed-evidence-bearing-work-chronology-claims")),
            "work-expression-claims.jsonl" => (LEGACY_BASE,"work",vec!["expression"],Some("has_expression"),Some("unreviewed-work-expression-topology-claims")),
            "expression-edition-claims.jsonl" => (LEGACY_BASE,"expression",vec!["edition"],Some("embodied_by"),Some("unreviewed-expression-edition-topology-claims")),
            "edition-item-claims.jsonl" => (LEGACY_BASE,"edition",vec!["item"],Some("exemplified_by"),Some("unreviewed-edition-item-topology-claims")),
            "expression-derivation-claims.jsonl" => (LEGACY_BASE,"expression",vec!["expression"],Some("is_derivative_of"),Some("unreviewed-source-reported-expression-derivation-claims")),
            "historical-claims.jsonl" => ("ToS/contracts/historical-claim.schema.json","",vec![],None,None),
            "object-link-claims.jsonl" => ("ToS/contracts/object-link-claim.schema.json","",vec!["link"],None,None),
            _ => { rules.skip(&format!("non-bibliographic-legacy-stream:{basename}"))?; return Ok(()); }
        };
        schema_read(cut,contract,rules)?;
        if !schemas.check(&location,&bytes,contract,rules.limits.deadline,rules.cancelled)? { rules.issue("legacy-Claim-schema",&location)?; return Ok(()); }
        if expected.is_some_and(|p|p!=predicate) { rules.issue("legacy-Claim-predicate",&location)?; }
        if !matches!(basename,"object-link-claims.jsonl"|"expression-derivation-claims.jsonl"|"historical-claims.jsonl") && s(row,"claim_type")!=Some("bibliographic") { rules.issue("legacy-Claim-type",&location)?; }
        let mut subject_kind=subject_kind;
        if basename=="responsibility-claims.jsonl" { subject_kind=match predicate { "authored_by"|"contributed_by"=>"work", "translated_by"=>"expression", "edited_by"|"afterword_by"|"designed_by"=>"edition", _=> { rules.issue("legacy-responsibility-predicate",&location)?; "" } }; }
        if let Some(subject)=s(row,"subject_ref") { require_kind(subject,&[subject_kind],records,rules,&location)?; } else { rules.issue("legacy-Claim-subject",&location)?; }
        if !object_kinds.is_empty() { if let Some(object)=s(row,"object") { require_kind(object,&object_kinds,records,rules,&location)?; } else { rules.issue("legacy-Claim-object",&location)?; } }
        if LEGACY_TOPOLOGY.contains(&basename) {
            if s(row,"assertion_layer")!=Some("bibliographic_assertion") || row["maker"]!=json!({"maker_type":"model","agent_ref":"model:codex"}) || s(row,"provenance_event_ref")!=Some("tos.event.annotation.source-witness-bibliographic-topology.2026-07-31") || s(row,"epistemic_status")!=Some("observed") || s(row,"review_status")!=Some("unreviewed") || row["reviews"]!=json!([]) || s(row,"visibility")!=Some("public_metadata_only") { rules.issue("legacy-topology-bounded-posture",&location)?; }
            let mut expected_evidence=BTreeSet::new();
            for field in ["subject_ref","object"] { if let Some(record)=s(row,field).and_then(|id|records.get(id)) { expected_evidence.insert(record.path.clone()); if record.kind=="item" { if let Some(path)=s(&record.value,"item_manifest_ref") { expected_evidence.insert(path.into()); } } } }
            if strings(row,"evidence_refs").into_iter().collect::<BTreeSet<_>>()!=expected_evidence { rules.issue("legacy-topology-exact-endpoint-evidence",&location)?; }
        }
        if LEGACY_TOPOLOGY.contains(&basename) && claim.path!=format!("ToS/source-witnesses/relations/{}/{basename}",basename.strip_suffix("-claims.jsonl").unwrap()) { rules.issue("legacy-topology-owned-path",&location)?; }
        if matches!(basename,"publication-claims.jsonl"|"provision-activity-claims.jsonl") {
            let sibling=format!("{}/edition.json",claim.path.rsplit_once('/').unwrap().0);
            if !s(row,"subject_ref").and_then(|id|records.get(id)).is_some_and(|r|r.path==sibling) { rules.issue("legacy-edition-sibling-owner",&location)?; }
        }
        if basename=="historical-claims.jsonl" {
            let Some(route)=routes.get(predicate) else {rules.issue("legacy-historical-predicate",&location)?;return Ok(());};
            if let Some(subject)=s(row,"subject_ref") {rules.endpoint(subject,&route.domain,types,kinds,records,&location)?;}
            if predicate=="historical_dating" {if let Some(anchor)=s(&row["object"]["relative"],"anchor_ref") {rules.endpoint(anchor,&["tos.entity.historical-situation".into()],types,kinds,records,&location)?;}}
            else if let Some(object)=s(row,"object") {rules.endpoint(object,&route.range,types,kinds,records,&location)?;} else {rules.issue("legacy-historical-object",&location)?;}
        }
        if basename=="expression-derivation-claims.jsonl" {
            let raw=serde_json::to_vec(&row["qualifiers"]).map_err(|_|ItemRefusal::Unsupported("derivation qualifiers".into()))?;
            if !schemas.check(&location,&raw,"ToS/contracts/expression-derivation.schema.json",rules.limits.deadline,rules.cancelled)? {rules.issue("derivation-qualifier-schema",&location)?;}
        }
        if basename=="provision-activity-claims.jsonl" {
            let object=&row["object"]; let raw=serde_json::to_vec(object).map_err(|_|ItemRefusal::Unsupported("provision object".into()))?;
            if !schemas.check(&location,&raw,"ToS/contracts/provision-activity.schema.json",rules.limits.deadline,rules.cancelled)? { rules.issue("provision-object-schema",&location)?; }
            provision(object,records,rules,&location)?;
        }
        if basename=="work-chronology-claims.jsonl" {
            let object=&row["object"]; let raw=serde_json::to_vec(object).map_err(|_|ItemRefusal::Unsupported("chronology object".into()))?;
            if !schemas.check(&location,&raw,"ToS/contracts/first-publication-chronology.schema.json",rules.limits.deadline,rules.cancelled)? { rules.issue("chronology-object-schema",&location)?; }
            chronology(row,records,rules,&location)?;
        }
        if basename=="responsibility-claims.jsonl" {
            let matching=s(row,"provenance_event_ref").and_then(|id|events.get(id)).and_then(|event|event["outputs"].as_array()).into_iter().flatten().filter_map(|output|s(output,"role")).find(|role|matches!(*role,"unreviewed-translation-responsibility-claims"|"unreviewed-evidence-bearing-responsibility-claims"));
            if let Some(role)=matching { bind_event(cut,claim,role,events,rules,&location)?; } else { rules.issue("responsibility-event-output-role",&location)?; }
        }
        if let Some(role)=role { bind_event(cut,claim,role,events,rules,&location)?; } else if !matches!(basename,"responsibility-claims.jsonl"|"membership-claims.jsonl"|"object-link-claims.jsonl"|"historical-claims.jsonl") { rules.skip(&format!("legacy-batch-provenance-profile:{basename}"))?; } else if !s(row,"provenance_event_ref").is_some_and(|id|events.contains_key(id)) { rules.issue("legacy-event-unresolved",&location)?; }
        rules.checked(&format!("legacy-bibliography:{basename}"))?;
    }
    // Existence remains distinct from digest-bound original input resolution.
    for field in ["evidence_refs","counterevidence_refs"] { for reference in strings(row,field) {
        check(rules.limits.deadline,rules.cancelled)?;
        if reference.starts_with("tos.anchor.") {
            if !rules.anchors.contains(&reference) { rules.skip("boundary-and-versioned-anchor-owner-resolution")?; }
            continue;
        }
        if !reference.starts_with("ToS/") { continue; }
        let path=RelativePath::parse(&reference).map_err(|_|ItemRefusal::Unsupported("Claim evidence path".into()))?;
        let present=cut.current().member(&path).is_some();
        rules.read(PredicateRead::IdentityKey { namespace:"source-current-path".into(),key:reference.clone(),observed:if present {KeyState::Present} else {KeyState::Absent} })?;
        if !present { rules.issue("Claim-evidence-current-file-missing",&location)?; }
    } }
    let fact=ValidationFact { namespace:"source-Claim-value".into(),key:s(row,"claim_id").unwrap_or(&location).into(),value_digest:Digest256::of_bytes(&bytes).to_prefixed() };
    reserve(&mut rules.state,fact_state(&fact)?,rules.limits.max_state_bytes)?;
    rules.shadow.facts.push(fact); Ok(())
}
fn require_kind(id:&str, kinds:&[&str], records:&BTreeMap<String,BiblioCurrentRecord>, rules:&mut Rules<'_>, location:&str)->Result<(),ItemRefusal> {
    let target=records.get(id);
    rules.read(PredicateRead::RefEndpoint { endpoint_type:kinds.join("|"),id:id.into(),observed:if target.is_some(){KeyState::Present}else{KeyState::Absent} })?;
    if !target.is_some_and(|r|kinds.contains(&r.kind.as_str()) || kinds.contains(&"") && r.kind!="link") { rules.issue("bibliography-endpoint-kind-or-missing",location)?; } Ok(())
}
fn qualified(row:&Value,predicate:&str,rules:&mut Rules<'_>,location:&str)->Result<(),ItemRefusal> {
    let scope=if predicate=="contains_work" {"membership_scope"} else {"attribution_scope"};
    for field in ["statement","statement_language","statement_script",scope] { if !s(&row["qualifiers"],field).is_some_and(|v|!v.trim().is_empty()) { rules.issue("qualified-bibliography-statement",location)?; } } Ok(())
}
fn bind_event(cut:&CorpusCutReader, claim:&BiblioClaim, role:&str, events:&BTreeMap<String,Value>, rules:&mut Rules<'_>, location:&str)->Result<(),ItemRefusal> {
    let Some(event)=s(&claim.value,"provenance_event_ref").and_then(|id|events.get(id)) else { rules.issue("bibliography-provenance-event-missing",location)?; return Ok(()); };
    let expected=json!({"ref":claim.path,"role":role,"sha256":claim.raw_sha256});
    if !event["outputs"].as_array().is_some_and(|rows|rows.contains(&expected)) { rules.issue("bibliography-provenance-output-binding",location)?; }
    for input in event["inputs"].as_array().into_iter().flatten() {
        check(rules.limits.deadline,rules.cancelled)?;
        let (Some(path),Some(digest))=(s(input,"ref"),s(input,"sha256")) else { continue; };
        if !path.starts_with("ToS/") { continue; }
        let Ok(expected)=Digest256::from_hex(digest) else { rules.issue("bibliography-input-digest-format",location)?; continue; };
        let relative=RelativePath::parse(path).map_err(|_|ItemRefusal::Unsupported("bibliography input path".into()))?;
        let mut resolved=false;
        for snapshot in cut.revisions() {
            check(rules.limits.deadline,rules.cancelled)?;
            // Retained source JSON can preserve exact earlier record evidence.
            // Contracts and other source types retain current owner authority.
            if snapshot.revision()!=cut.current().revision() && !(owned(path)&&path.ends_with(".json")) { continue; }
            let Some(member)=snapshot.member(&relative) else { continue; };
            if member.sha256!=expected { continue; }
            let raw=cut.read_member(snapshot.revision(),&relative,rules.limits.max_member_bytes as u64,rules.limits.deadline,rules.cancelled).map_err(store_error)?;
            account(&mut rules.bytes,raw.raw.len(),rules.limits.max_total_bytes)?;
            rules.read(PredicateRead::ExactBytes { locator:format!("{}:{path}",snapshot.revision().0.to_hex()),digest:expected.to_prefixed() })?;
            resolved=true; break;
        }
        if !resolved { rules.issue("bibliography-recorded-input-unresolved",location)?; }
    }
    Ok(())
}
fn provision(object:&Value,records:&BTreeMap<String,BiblioCurrentRecord>,rules:&mut Rules<'_>,location:&str)->Result<(),ItemRefusal> {
    let (places,agents):(&[&str],&[&str])=match s(object,"provision_kind") {Some("publication")=>(&["publication_place"],&["publisher"]),Some("production")=>(&["production_place"],&["producer"]),Some("distribution")=>(&["distribution_place"],&["distributor"]),Some("manufacture")=>(&["manufacture_place"],&["manufacturer","printer"]),_=>(&[],&[])};
    for (field,allowed,normalized,kinds) in [("places",places,"normalized_place_ref",vec!["place"]),("agents",agents,"normalized_agent_ref",vec!["agent","organization"])] {
        for row in object[field].as_array().into_iter().flatten() {
            check(rules.limits.deadline,rules.cancelled)?;
            if !allowed.is_empty() && !s(row,"role").is_some_and(|v|allowed.contains(&v)) { rules.issue("provision-role-incompatible",location)?; }
            if let Some(id)=s(row,normalized) { require_kind(id,&kinds,records,rules,location)?; }
        }
    }
    let temporal=&object["temporal"];
    if s(temporal,"kind")==Some("interval") && s(temporal,"start").zip(s(temporal,"end")).is_some_and(|(a,b)|a>b) { rules.issue("provision-interval-reversed",location)?; }
    if s(object,"event_posture")==Some("source_statement_only") && temporal.is_object() && s(temporal,"role")!=Some("statement_date") { rules.issue("provision-statement-temporal-role",location)?; } Ok(())
}
fn chronology(claim:&Value,records:&BTreeMap<String,BiblioCurrentRecord>,rules:&mut Rules<'_>,location:&str)->Result<(),ItemRefusal> {
    let object=&claim["object"]; let interval=&object["interval"];
    let (start,end)=(s(interval,"start"),s(interval,"end"));
    if start.zip(end).is_some_and(|(a,b)|a>b) { rules.issue("chronology-interval-reversed",location)?; }
    let stages=object["stages"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let dates:Vec<_>=stages.iter().filter_map(|v|s(v,"date")).collect();
    if dates.windows(2).any(|p|p[0]>p[1]) { rules.issue("chronology-stage-order",location)?; }
    match s(object,"sequence_posture") { Some("single_event") if stages.len()!=1 || s(interval,"boundary_meaning")!=Some("single_stage") =>rules.issue("chronology-single-stage",location)?, Some("staged_sequence") if stages.len()<2 || s(interval,"boundary_meaning")!=Some("earliest_stage_to_sequence_completion")=>rules.issue("chronology-staged-sequence",location)?,_=>{} }
    if dates.first().zip(start).is_some_and(|(date,start)|!date.starts_with(start)) || dates.last().zip(end).is_some_and(|(date,end)|!date.starts_with(end)) { rules.issue("chronology-boundary-stage",location)?; }
    for stage in stages {
        check(rules.limits.deadline,rules.cancelled)?;
        if let Some(id)=s(stage,"edition_ref") {
            require_kind(id,&["edition"],records,rules,location)?;
            if let Some(edition)=records.get(id) { if !strings(&edition.value,"embodies_expression_refs").iter().any(|id|records.get(id).is_some_and(|expression|s(&expression.value,"work_ref")==s(claim,"subject_ref"))) { rules.issue("chronology-stage-edition-other-work",location)?; } }
        }
    }
    if claim["maker"]!=json!({"maker_type":"model","agent_ref":"model:codex"}) || s(claim,"epistemic_status")!=Some("reported") || s(claim,"review_status")!=Some("unreviewed") || claim["reviews"]!=json!([]) || s(claim,"visibility")!=Some("public_metadata_only") { rules.issue("chronology-bounded-posture",location)?; }
    Ok(())
}

fn closure_field(row:&BiblioClaim)->Option<&'static str> {
    match s(&row.value,"predicate") {
        Some("contains_work")=>Some("membership_claim_refs"),
        Some(p) if RESPONSIBILITY.contains(&p)=>Some("responsibility_claim_refs"),
        Some("first_publication_chronology")=>Some("chronology_claim_refs"),
        Some("provision_activity")=>Some("provision_activity_claim_refs"),
        Some("is_derivative_of")=>Some("derivation_claim_refs"),
        Some("described_by"|"metadata_at"|"downloadable_at"|"rights_statement_at")=>Some("association_claim_refs"),
        _ if row.path.ends_with("/publication-claims.jsonl")=>Some("publication_claim_refs"),
        _=>None,
    }
}
fn inspect_closure(records:&BTreeMap<String,BiblioCurrentRecord>,claims:&[BiblioClaim],generation:&str,rules:&mut Rules<'_>)->Result<(),ItemRefusal> {
    let mut union:BTreeMap<(&str,&str),BTreeSet<&str>>=BTreeMap::new();
    let mut by_id=BTreeMap::new();
    let mut chronology_subjects=BTreeSet::new();
    let mut indexes=std::mem::size_of_val(&union)+std::mem::size_of_val(&by_id)+std::mem::size_of_val(&chronology_subjects);
    reserve(&mut rules.state,indexes,rules.limits.max_state_bytes)?;
    for claim in claims {
        check(rules.limits.deadline,rules.cancelled)?;
        let (Some(id),Some(subject))=(s(&claim.value,"claim_id"),s(&claim.value,"subject_ref")) else { continue; };
        if !by_id.contains_key(id) {let cost=std::mem::size_of::<(&str,&BiblioClaim)>();reserve(&mut rules.state,cost,rules.limits.max_state_bytes)?;indexes=indexes.checked_add(cost).ok_or(ItemRefusal::Budget)?;}
        by_id.entry(id).or_insert(claim);
        let Some(field)=closure_field(claim) else { continue; };
        let target=if field=="association_claim_refs" { s(&claim.value,"object").unwrap_or("") } else {subject};
        if !union.contains_key(&(target,field)) {let cost=std::mem::size_of::<((&str,&str),BTreeSet<&str>)>();reserve(&mut rules.state,cost,rules.limits.max_state_bytes)?;indexes=indexes.checked_add(cost).ok_or(ItemRefusal::Budget)?;}
        let refs=union.entry((target,field)).or_default();
        if !refs.contains(id) {let cost=std::mem::size_of::<&str>();reserve(&mut rules.state,cost,rules.limits.max_state_bytes)?;indexes=indexes.checked_add(cost).ok_or(ItemRefusal::Budget)?;refs.insert(id);}
        if field=="chronology_claim_refs" && !chronology_subjects.contains(subject) {let cost=std::mem::size_of::<&str>();reserve(&mut rules.state,cost,rules.limits.max_state_bytes)?;indexes=indexes.checked_add(cost).ok_or(ItemRefusal::Budget)?;chronology_subjects.insert(subject);}
    }
    let nietzsche:BTreeSet<_>=records.iter().filter(|(_,r)|r.kind=="work" && r.path.starts_with("ToS/source-witnesses/works/friedrich-nietzsche/")).map(|(id,_)|id.as_str()).collect();
    let nietzsche_state=std::mem::size_of_val(&nietzsche)+nietzsche.len()*std::mem::size_of::<&str>();
    reserve(&mut rules.state,nietzsche_state,rules.limits.max_state_bytes)?;indexes=indexes.checked_add(nietzsche_state).ok_or(ItemRefusal::Budget)?;
    if chronology_subjects!=nietzsche { rules.issue("Nietzsche-chronology-subject-closure","ToS/source-witnesses/chronology/friedrich-nietzsche/first-publication")?; }
    for (id,record) in records {
        check(rules.limits.deadline,rules.cancelled)?;
        let fields:&[&str]=match record.kind.as_str() {"collection"=>&["membership_claim_refs"],"work"=>&["responsibility_claim_refs"],"expression"=>&["responsibility_claim_refs","derivation_claim_refs"],"edition"=>&["responsibility_claim_refs","publication_claim_refs","provision_activity_claim_refs"],"link"=>&["association_claim_refs"],_=>&[]};
        for field in fields.iter().copied().chain((nietzsche.contains(id.as_str())).then_some("chronology_claim_refs")) {
            let refs:Vec<_>=record.value[field].as_array().into_iter().flatten().filter_map(Value::as_str).collect();let actual:BTreeSet<_>=refs.iter().copied().collect();
            let scratch=std::mem::size_of_val(&refs)+std::mem::size_of_val(&actual)+refs.len()*std::mem::size_of::<&str>()+actual.len()*std::mem::size_of::<&str>();
            reserve_check(rules.state,scratch,rules.limits.max_state_bytes)?;let ceiling=rules.limits.max_state_bytes;rules.limits.max_state_bytes=ceiling.checked_sub(scratch).ok_or(ItemRefusal::Budget)?;
            let expected=union.get(&(id.as_str(),field));
            let empty=BTreeSet::new();let expected=expected.unwrap_or(&empty);
            if actual.len()!=refs.len() || &actual!=expected { rules.issue("bibliography-exact-reverse-closure",&format!("{}#{field}",record.path))?; }
            rules.read(PredicateRead::ReverseRefs {target:id.clone(),relation:format!("bibliography:{field}"),generation:generation.into()})?;
            rules.read(PredicateRead::Range {namespace:"bibliography-current-Claim-field".into(),lower:format!("{field}:{id}"),upper:format!("{field}:{id}"),generation:generation.into()})?;
            if field=="chronology_claim_refs" && expected.len()!=1 { rules.issue("Nietzsche-one-chronology",&record.path)?; }
            if field=="association_claim_refs" {
                for reference in expected { if let Some(claim)=by_id.get(*reference) { if claim.value.get("provenance_event_ref")!=record.value.get("provenance_event_ref") { rules.issue("Link-Claim-provenance-mismatch",&record.path)?; } } }
            }
            if field=="responsibility_claim_refs" && nietzsche.contains(id.as_str()) {
                let authors:Vec<_>=actual.iter().filter_map(|id|by_id.get(*id)).filter(|c|s(&c.value,"predicate")==Some("authored_by")).collect();
                if authors.len()!=1 || s(&authors[0].value,"object")!=Some("tos.agent.friedrich-nietzsche") { rules.issue("Nietzsche-explicit-authorship",&record.path)?; }
            }
            let payload=json!({"declared":actual,"observed":expected});let payload_state=crate::record_biblio_cut::decoded_state(&payload)?;
            reserve_check(rules.state,payload_state,rules.limits.max_state_bytes)?;
            crate::record_biblio_cut::decoded_wire_size(&payload,rules.limits.max_state_bytes.checked_sub(rules.state).and_then(|n|n.checked_sub(payload_state)).ok_or(ItemRefusal::Budget)?)?;
            let raw=serde_json::to_vec(&payload).map_err(|_|ItemRefusal::Unsupported("closure fact representation".into()))?;
            let fact=ValidationFact {namespace:"bibliography-subject-closure".into(),key:format!("{field}:{id}"),value_digest:Digest256::of_bytes(&raw).to_prefixed()};
            reserve_check(rules.state,payload_state+raw.len()+std::mem::size_of::<Vec<u8>>()+fact_state(&fact)?,rules.limits.max_state_bytes)?;
            reserve(&mut rules.state,fact_state(&fact)?,rules.limits.max_state_bytes)?;rules.shadow.facts.push(fact);
            drop(payload);drop(raw);drop(actual);drop(refs);rules.limits.max_state_bytes=ceiling;
        }
    }
    drop(union);drop(by_id);drop(chronology_subjects);drop(nietzsche);rules.state-=indexes;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration,Instant};
    fn rules(cancelled:&AtomicBool)->Rules<'_> { Rules {limits:ItemLimits {max_member_bytes:1_048_576,max_total_bytes:16_777_216,max_state_bytes:8_388_608,max_issues:64,deadline:Instant::now()+Duration::from_secs(5)},cancelled,state:std::mem::size_of::<Rules<'_>>(),ancestry_state:0,bytes:0,anchors:BTreeSet::new(),reserved:BTreeSet::new(),schema_seen:BTreeSet::new(),shadow:RelationShadow::default()} }
    fn record(kind:&str,value:Value)->BiblioCurrentRecord { BiblioCurrentRecord {path:format!("ToS/source-witnesses/{kind}/example/{kind}.json"),kind:kind.into(),value} }
    fn claim(value:Value)->BiblioClaim {BiblioClaim {path:"ToS/source-witnesses/relations/source-claims.jsonl".into(),line:1,value,raw_sha256:"00".repeat(32),native:true} }
    #[test]
    fn qualified_membership_responsibility_and_reverse_closure_oracles() {
        let cancelled=AtomicBool::new(false);
        let mut records=BTreeMap::from([("tos.collection.c".into(),record("collection",json!({"record_id":"tos.collection.c","membership_claim_refs":["tos.claim.m"]}))), ("tos.work.w".into(),record("work",json!({"record_id":"tos.work.w","responsibility_claim_refs":[]})))]);
        let member=claim(json!({"claim_id":"tos.claim.m","subject_ref":"tos.collection.c","object":"tos.work.w","predicate":"contains_work","qualifiers":{"statement":"reported member","statement_language":"en","statement_script":"Latn","membership_scope":"asserted"}}));
        let mut positive=rules(&cancelled); qualified(&member.value,"contains_work",&mut positive,"m").unwrap(); inspect_closure(&records,&[member.clone()],"exact-eof",&mut positive).unwrap(); assert!(positive.shadow.issues.is_empty(),"{:?}",positive.shadow.issues);
        assert!(positive.shadow.reads.iter().any(|r|matches!(r,PredicateRead::ReverseRefs {relation,..} if relation=="bibliography:membership_claim_refs")));
        records.get_mut("tos.collection.c").unwrap().value["membership_claim_refs"]=json!([]);
        let mut omitted=rules(&cancelled);inspect_closure(&records,&[member.clone()],"exact-eof",&mut omitted).unwrap();assert!(omitted.shadow.issues.iter().any(|r|r.code=="bibliography-exact-reverse-closure"));
        let mut blank=member.value.clone();blank["qualifiers"]["membership_scope"]=json!(" ");let mut bad=rules(&cancelled);qualified(&blank,"contains_work",&mut bad,"m").unwrap();assert!(bad.shadow.issues.iter().any(|r|r.code=="qualified-bibliography-statement"));
        let mut responsibility=rules(&cancelled); qualified(&member.value,"translated_by",&mut responsibility,"t").unwrap();assert!(!responsibility.shadow.issues.is_empty());
    }
    #[test]
    fn provision_roles_chronology_sequence_and_endpoint_oracles() {
        let cancelled=AtomicBool::new(false);
        let records=BTreeMap::from([("tos.agent.a".into(),record("agent",json!({"record_id":"tos.agent.a"}))), ("tos.place.p".into(),record("place",json!({"record_id":"tos.place.p"}))), ("tos.edition.e".into(),record("edition",json!({"record_id":"tos.edition.e","embodies_expression_refs":["tos.expression.x"]}))), ("tos.expression.x".into(),record("expression",json!({"record_id":"tos.expression.x","work_ref":"tos.work.w"})))]);
        let activity=json!({"provision_kind":"publication","places":[{"role":"publication_place","normalized_place_ref":"tos.place.p"}],"agents":[{"role":"publisher","normalized_agent_ref":"tos.agent.a"}],"temporal":{"kind":"interval","start":"1883","end":"1885","role":"statement_date"},"event_posture":"source_statement_only"});
        let mut positive=rules(&cancelled);provision(&activity,&records,&mut positive,"p").unwrap();assert!(positive.shadow.issues.is_empty());
        let mut bad_activity=activity.clone();bad_activity["agents"][0]["role"]=json!("printer");bad_activity["temporal"]["end"]=json!("1882");let mut bad=rules(&cancelled);provision(&bad_activity,&records,&mut bad,"p").unwrap();assert!(bad.shadow.issues.iter().any(|r|r.code=="provision-role-incompatible"));assert!(bad.shadow.issues.iter().any(|r|r.code=="provision-interval-reversed"));
        let chronology_claim=json!({"subject_ref":"tos.work.w","object":{"interval":{"start":"1883","end":"1885","boundary_meaning":"earliest_stage_to_sequence_completion"},"stages":[{"date":"1883-02","edition_ref":"tos.edition.e"},{"date":"1885"}],"sequence_posture":"staged_sequence"},"maker":{"maker_type":"model","agent_ref":"model:codex"},"epistemic_status":"reported","review_status":"unreviewed","reviews":[],"visibility":"public_metadata_only"});
        let mut good=rules(&cancelled);chronology(&chronology_claim,&records,&mut good,"c").unwrap();assert!(good.shadow.issues.is_empty(),"{:?}",good.shadow.issues);
        let mut reversed=chronology_claim.clone();reversed["object"]["stages"][0]["date"]=json!("1886");let mut bad=rules(&cancelled);chronology(&reversed,&records,&mut bad,"c").unwrap();assert!(bad.shadow.issues.iter().any(|r|r.code=="chronology-stage-order"));
        let mut endpoint=rules(&cancelled);require_kind("tos.agent.a",&["place"],&records,&mut endpoint,"typed").unwrap();assert!(endpoint.shadow.issues.iter().any(|r|r.code=="bibliography-endpoint-kind-or-missing"));
    }
    #[test]
    fn scoped_order_and_identity_plan_preserve_declared_topology() {
        let cancelled=AtomicBool::new(false);
        let structure=json!({"subject_ref":"tos.artifact.whole","object":{"kind":"physical-part-composition","members":["tos.artifact.a","tos.artifact.b"],"ordering":{"mode":"total","precedes":[["tos.artifact.a","tos.artifact.b"]]}}});
        let mut good=rules(&cancelled);member_structure(&structure,&mut good,"order").unwrap();assert!(good.shadow.issues.is_empty());
        let mut cyclic=structure.clone();cyclic["object"]["ordering"]["precedes"]=json!([["tos.artifact.a","tos.artifact.b"],["tos.artifact.b","tos.artifact.a"]]);let mut bad=rules(&cancelled);member_structure(&cyclic,&mut bad,"order").unwrap();assert!(bad.shadow.issues.iter().any(|i|i.code=="structure-cycle"));
        let mut partial=structure.clone();partial["object"]["ordering"]["precedes"]=json!([]);let mut bad=rules(&cancelled);member_structure(&partial,&mut bad,"order").unwrap();assert!(bad.shadow.issues.iter().any(|i|i.code=="structure-total-incomparable"));
        let reference=|id:&str|json!({"id":id,"version":1,"digest":format!("sha256:{}","00".repeat(32))});
        let proposal=json!({"claim_id":"tos.claim.p","subject_ref":"tos.work.a","supersedes_claim_ref":null,"object":{"operation":"merge","members":["tos.work.a","tos.work.b","tos.work.c"],"predecessors":[reference("tos.work.a"),reference("tos.work.b")],"successors":[reference("tos.work.c")],"mapping":[{"predecessor":"tos.work.a","successor":"tos.work.c"},{"predecessor":"tos.work.b","successor":"tos.work.c"}],"supersedes_proposal":null,"unresolved_links":[]}});
        let records=["a","b","c"].into_iter().map(|suffix|(format!("tos.work.{suffix}"),record("work",json!({"record_id":format!("tos.work.{suffix}")})))).collect();let kinds=BTreeMap::from([("work".into(),"tos.entity.work".into())]);let types=BTreeMap::from([("tos.entity.work".into(),json!({"abstract":false,"object_role":"identity"}))]);
        let mut good=rules(&cancelled);identity_proposal(&proposal,"identity-transition-v1",&types,&kinds,&records,&mut good,"proposal").unwrap();assert!(good.shadow.issues.is_empty());assert!(good.shadow.unsupported);
        let mut incomplete=proposal.clone();incomplete["object"]["mapping"].as_array_mut().unwrap().pop();let mut bad=rules(&cancelled);identity_proposal(&incomplete,"identity-transition-v1",&types,&kinds,&records,&mut bad,"proposal").unwrap();assert!(bad.shadow.issues.iter().any(|i|i.code=="proposal-complete-mapping"));
    }
    #[test]
    fn batch_configuration_uses_observed_legacy_counts_and_authority_ceiling() {
        let cancelled=AtomicBool::new(false);
        let mut row=claim(json!({"claim_id":"tos.claim.a","predicate":"has_expression"}));row.native=false;row.path="ToS/source-witnesses/relations/work-expression/work-expression-claims.jsonl".into();
        let config=json!({"work_expression_claims_materialized":1,"expression_edition_claims_materialized":0,"edition_item_claims_materialized":0,"topology_claims_reviewed":0,"source_text_admitted":false,"human_review_performed":false,"textual_equivalence_claims_created":0,"semantic_claims_created":0,"canon_promotion_performed":false});
        let event=json!({"event_type":"annotation","agent_refs":["model:codex"],"method":{"maker_type":"model","name":"declared-bibliographic-topology-materialization","version":"1","configuration":config},"status":"completed_with_warnings"});
        let mut events=BTreeMap::from([(TOPOLOGY_EVENT.into(),event)]);let records=BTreeMap::new();let mut good=rules(&cancelled);inspect_batches(&[row.clone()],&events,&records,&mut good).unwrap();assert!(good.shadow.issues.is_empty());
        events.get_mut(TOPOLOGY_EVENT).unwrap()["method"]["configuration"]["source_text_admitted"]=json!(true);let mut bad=rules(&cancelled);inspect_batches(&[row.clone()],&events,&records,&mut bad).unwrap();assert!(bad.shadow.issues.iter().any(|i|i.code=="topology-exact-legacy-batch-configuration"));
        let mut absent=rules(&cancelled);inspect_batches(&[row],&BTreeMap::new(),&records,&mut absent).unwrap();assert!(absent.shadow.issues.iter().any(|i|i.code=="topology-owned-batch-event-missing"));
    }
    #[test]
    fn cancellation_and_state_budgets_refuse_without_success_projection() {
        let cancelled=AtomicBool::new(true);let mut r=rules(&cancelled);assert!(matches!(r.issue("example","x"),Err(ItemRefusal::Source(_))));
        let cancelled=AtomicBool::new(false);let mut r=rules(&cancelled);r.limits.max_state_bytes=1;assert!(matches!(r.read(PredicateRead::AbsentKey {namespace:"test".into(),key:"x".into()}),Err(ItemRefusal::BudgetCheck {..})));
        let mut r=rules(&cancelled);r.limits.max_issues=0;assert_eq!(r.issue("example","x"),Err(ItemRefusal::BudgetCheck {check:"bibliography issue count",used:Some(1),limit:Some(0)}));
    }
}

fn event_posture(event:&Value,method:&str,rules:&mut Rules<'_>,location:&str)->Result<(),ItemRefusal> {
    if s(event,"event_type")!=Some("annotation") || event["agent_refs"]!=json!(["model:codex"]) || s(&event["method"],"maker_type")!=Some("model") || s(&event["method"],"name")!=Some(method) || s(&event["method"],"version")!=Some("1") || s(event,"status")!=Some("completed_with_warnings") { rules.issue("bibliography-batch-posture",location)?; } Ok(())
}
fn exact_batch_inputs(event:&Value,expected:BTreeSet<String>,rules:&mut Rules<'_>,location:&str)->Result<(),ItemRefusal> {
    let rows=event["inputs"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let actual:BTreeSet<_>=rows.iter().filter_map(|r|s(r,"ref")).map(str::to_owned).collect();
    if actual!=expected || rows.len()!=actual.len() { rules.issue("bibliography-batch-exact-input-set",location)?; } Ok(())
}
fn inspect_batches(claims:&[BiblioClaim],events:&BTreeMap<String,Value>,records:&BTreeMap<String,BiblioCurrentRecord>,rules:&mut Rules<'_>)->Result<(),ItemRefusal> {
    let topology:Vec<_>=claims.iter().filter(|c|!c.native && LEGACY_TOPOLOGY.iter().any(|basename|c.path.ends_with(basename))).collect();
    if !topology.is_empty() {
        let location="ToS/source-witnesses/relations/provenance.jsonl";
        if let Some(event)=events.get(TOPOLOGY_EVENT) {
            event_posture(event,"declared-bibliographic-topology-materialization",rules,location)?;
            let count=|predicate:&str|topology.iter().filter(|c|s(&c.value,"predicate")==Some(predicate)).count();
            let expected=json!({"work_expression_claims_materialized":count("has_expression"),"expression_edition_claims_materialized":count("embodied_by"),"edition_item_claims_materialized":count("exemplified_by"),"topology_claims_reviewed":0,"source_text_admitted":false,"human_review_performed":false,"textual_equivalence_claims_created":0,"semantic_claims_created":0,"canon_promotion_performed":false});
            if event["method"]["configuration"]!=expected { rules.issue("topology-exact-legacy-batch-configuration",location)?; }
        } else { rules.issue("topology-owned-batch-event-missing",location)?; }
    }
    let chronology:Vec<_>=claims.iter().filter(|c|!c.native && c.path.ends_with("/work-chronology-claims.jsonl")).collect();
    if !chronology.is_empty() {
        let path="ToS/source-witnesses/chronology/friedrich-nietzsche/first-publication/work-chronology-claims.jsonl";
        let mut inputs=BTreeSet::from([LEGACY_BASE.into(),"ToS/contracts/first-publication-chronology.schema.json".into()]);
        for claim in &chronology {
            check(rules.limits.deadline,rules.cancelled)?;
            if claim.path!=path || s(&claim.value,"provenance_event_ref")!=Some(CHRONOLOGY_EVENT) || s(&claim.value,"assertion_layer")!=Some("scholarly_report") { rules.issue("chronology-owned-route-and-event",&claim.path)?; }
            let evidence=strings(&claim.value,"evidence_refs");
            if !evidence.iter().any(|v|v.contains("authorial-witness-route")) || !evidence.iter().any(|v|v.contains("AUTHORIAL_WITNESS_ROUTE.md")) || evidence.iter().any(|v|!v.starts_with("ToS/")) { rules.issue("chronology-documentary-evidence-set",&claim.path)?; }
            inputs.extend(evidence);
        }
        if let Some(event)=events.get(CHRONOLOGY_EVENT) {
            event_posture(event,"faceted-first-publication-chronology-materialization",rules,path)?;
            let output=json!([{"ref":path,"role":"unreviewed-evidence-bearing-work-chronology-claims","sha256":chronology[0].raw_sha256}]);
            if event["outputs"]!=output { rules.issue("chronology-exact-batch-output",path)?; }
            exact_batch_inputs(event,inputs,rules,path)?;
            let expected=json!({"works_materialized":7,"chronology_claims_materialized":7,"staged_sequence_claims":1,"single_event_claims":6,"chronology_claims_reviewed":0,"composition_claims_created":0,"source_text_admitted":false,"human_review_performed":false,"semantic_claims_created":0,"canon_promotion_performed":false});
            if event["method"]["configuration"]!=expected { rules.issue("chronology-owned-fixed-configuration",path)?; }
        } else { rules.issue("chronology-owned-batch-event-missing",path)?; }
    }
    let derivations:Vec<_>=claims.iter().filter(|c|!c.native && c.path.ends_with("/expression-derivation-claims.jsonl")).collect();
    if !derivations.is_empty() {
        let path="ToS/source-witnesses/relations/expression-derivation/expression-derivation-claims.jsonl";
        let mut edges:BTreeMap<String,BTreeSet<String>>=BTreeMap::new(); let mut pairs=BTreeSet::new();let mut endpoints=BTreeSet::new();
        let mut inputs=BTreeSet::from([LEGACY_BASE.into(),"ToS/contracts/expression-derivation.schema.json".into()]);
        for claim in &derivations {
            check(rules.limits.deadline,rules.cancelled)?;
            let row=&claim.value;
            if claim.path!=path || s(row,"claim_type")!=Some("relation") || s(row,"assertion_layer")!=Some("bibliographic_assertion") || s(row,"provenance_event_ref")!=Some(DERIVATION_EVENT) || row["maker"]!=json!({"maker_type":"model","agent_ref":"model:codex"}) || s(row,"epistemic_status")!=Some("reported") || s(row,"review_status")!=Some("unreviewed") || row["reviews"]!=json!([]) || s(row,"visibility")!=Some("public_metadata_only") { rules.issue("derivation-owned-bounded-posture",path)?; }
            if let (Some(subject),Some(object))=(s(row,"subject_ref"),s(row,"object")) {
                if subject==object { rules.issue("derivation-irreflexive",path)?; }
                if records.get(subject).zip(records.get(object)).is_some_and(|(a,b)|a.value.get("work_ref")!=b.value.get("work_ref")) { rules.issue("derivation-same-work",path)?; }
                if !pairs.insert((subject.to_owned(),object.to_owned())) { rules.issue("derivation-duplicate-pair",path)?; }
                reserve(&mut rules.state,std::mem::size_of::<(String,String)>()+subject.len()+object.len()+std::mem::size_of::<(String,BTreeSet<String>)>()+subject.len()+std::mem::size_of::<String>()+object.len()+2*std::mem::size_of::<String>()+subject.len()+object.len(),rules.limits.max_state_bytes)?;
                edges.entry(subject.into()).or_default().insert(object.into());endpoints.insert(subject.to_owned());endpoints.insert(object.to_owned());
            }
            let evidence=strings(row,"evidence_refs");
            if !evidence.iter().any(|v|v.starts_with("tos.anchor.")) { rules.issue("derivation-source-anchor-return",path)?; }
            inputs.extend(evidence.into_iter().filter(|v|v.starts_with("ToS/")));
        }
        for id in &endpoints { if let Some(record)=records.get(id) {inputs.insert(record.path.clone());} }
        // Kahn traversal preserves cycle detection without a recursive stack.
        let mut indegree:BTreeMap<String,usize>=endpoints.iter().map(|id|(id.clone(),0)).collect();
        for targets in edges.values() {for target in targets {*indegree.entry(target.clone()).or_default()+=1;}}
        let mut queue:Vec<String>=indegree.iter().filter(|(_,degree)|**degree==0).map(|(id,_)|id.clone()).collect();let mut visited=0;
        while let Some(id)=queue.pop() {check(rules.limits.deadline,rules.cancelled)?;visited+=1;for target in edges.get(&id).into_iter().flatten() {let degree=indegree.get_mut(target).unwrap();*degree-=1;if *degree==0 {queue.push(target.clone());}}}
        if visited!=endpoints.len() {rules.issue("derivation-cycle",path)?;}
        if let Some(event)=events.get(DERIVATION_EVENT) {
            event_posture(event,"source-reported-expression-derivation-materialization",rules,path)?;
            let output=json!([{"ref":path,"role":"unreviewed-source-reported-expression-derivation-claims","sha256":derivations[0].raw_sha256}]);
            if event["outputs"]!=output {rules.issue("derivation-exact-batch-output",path)?;}
            exact_batch_inputs(event,inputs,rules,path)?;
            let expected=json!({"expression_identities_materialized":endpoints.len(),"derivation_claims_materialized":derivations.len(),"revision_claims_materialized":derivations.iter().filter(|c|s(&c.value["qualifiers"],"derivation_kind")==Some("revision")).count(),"claims_collated":derivations.iter().filter(|c|s(&c.value["qualifiers"],"collation_status")!=Some("not_collated")).count(),"claims_reviewed":derivations.iter().filter(|c|s(&c.value,"review_status")!=Some("unreviewed")).count(),"unsupported_1911_to_1907_edge_created":false,"unsupported_2007_to_1911_edge_created":false,"source_text_admitted":false,"human_review_performed":false,"equivalence_claims_created":0,"semantic_claims_created":0,"canon_promotion_performed":false});
            if event["method"]["configuration"]!=expected {rules.issue("derivation-exact-batch-configuration",path)?;}
        } else {rules.issue("derivation-owned-batch-event-missing",path)?;}
    }
    Ok(())
}

fn member_structure(claim:&Value,rules:&mut Rules<'_>,location:&str)->Result<(),ItemRefusal> {
    let value=&claim["object"];let members:BTreeSet<_>=strings(value,"members").into_iter().collect();
    if s(claim,"subject_ref").is_some_and(|v|members.contains(v)) {rules.issue("structure-subject-member",location)?;}
    let mode=s(&value["ordering"],"mode");let edges=value["ordering"]["precedes"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    if members.len()>128 || edges.len()>8128 {return Err(ItemRefusal::Budget);}
    if mode==Some("unordered") && !edges.is_empty() {rules.issue("structure-unordered-precedence",location)?;}
    let mut outgoing:BTreeMap<String,BTreeSet<String>>=members.iter().map(|m|(m.clone(),BTreeSet::new())).collect();let mut degree:BTreeMap<String,usize>=members.iter().map(|m|(m.clone(),0)).collect();
    for edge in edges {
        check(rules.limits.deadline,rules.cancelled)?;
        let pair=edge.as_array().filter(|p|p.len()==2).and_then(|p|p[0].as_str().zip(p[1].as_str()));
        let Some((before,after))=pair else {rules.issue("structure-precedence-shape",location)?;continue;};
        if !members.contains(before)||!members.contains(after) {rules.issue("structure-precedence-outside-members",location)?;continue;}
        if !outgoing.get_mut(before).unwrap().insert(after.into()) {rules.issue("structure-duplicate-precedence",location)?;continue;}*degree.get_mut(after).unwrap()+=1;
    }
    let mut ready:Vec<_>=degree.iter().filter(|(_,n)|**n==0).map(|(m,_)|m.clone()).collect();let mut visited=0;
    while let Some(member)=ready.pop() {check(rules.limits.deadline,rules.cancelled)?;if mode==Some("total") && !ready.is_empty() {rules.issue("structure-total-incomparable",location)?;}visited+=1;for after in outgoing.get(&member).into_iter().flatten() {let n=degree.get_mut(after).unwrap();*n-=1;if *n==0 {ready.push(after.clone());}}}
    if visited!=members.len() {rules.issue("structure-cycle",location)?;}
    if s(value,"kind")==Some("collection-member-order") {
        let collection=&value["collection_version"];let bindings=value["membership_versions"].as_array().map(Vec::as_slice).unwrap_or(&[]);let ids:BTreeSet<_>=bindings.iter().filter_map(|r|s(r,"id")).collect();
        if !exact_ref(collection,false) || s(collection,"id")!=s(claim,"subject_ref") || !s(collection,"id").is_some_and(|v|v.starts_with("tos.collection.")) || bindings.len()!=members.len() || ids.len()!=bindings.len() || bindings.iter().any(|r|!exact_ref(r,true)) {rules.issue("collection-order-exact-basis-shape",location)?;}
    } Ok(())
}
fn exact_ref(value:&Value,claim:bool)->bool {
    let Some(object)=value.as_object() else{return false;};
    object.len()==3 && s(value,"id").is_some_and(|id|id.starts_with(if claim {"tos.claim."}else{"tos."})) && value["version"].as_u64().is_some_and(|v|v>0&&v<=9_007_199_254_740_991) && s(value,"digest").and_then(|d|d.strip_prefix("sha256:")).is_some_and(|d|Digest256::from_hex(d).is_ok())
}
fn identity_proposal(claim:&Value,reader:&str,types:&BTreeMap<&str,&Value>,kinds:&BTreeMap<&str,&str>,records:&BTreeMap<String,BiblioCurrentRecord>,rules:&mut Rules<'_>,location:&str)->Result<(),ItemRefusal> {
    let value=&claim["object"];let left=value["predecessors"].as_array().map(Vec::as_slice).unwrap_or(&[]);let right=value["successors"].as_array().map(Vec::as_slice).unwrap_or(&[]);let mappings=value["mapping"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    if left.is_empty()||right.is_empty()||left.len()>8||right.len()>8||mappings.len()>8 {rules.issue("proposal-bounded-participant-shape",location)?;return Ok(());}
    let ids:Vec<_>=left.iter().chain(right).filter_map(|r|s(r,"id")).collect();let distinct:BTreeSet<_>=ids.iter().copied().collect();let members=strings(value,"members");
    if left.iter().chain(right).any(|r|!exact_ref(r,false)) || distinct.len()!=ids.len() || members.len()!=ids.len() || members.iter().map(String::as_str).collect::<BTreeSet<_>>()!=distinct || !s(claim,"subject_ref").is_some_and(|subject|left.iter().any(|r|s(r,"id")==Some(subject))) || s(claim,"claim_id").is_some_and(|id|distinct.contains(id)) {rules.issue("proposal-participant-union",location)?;}
    if !matches!((s(value,"operation"),left.len(),right.len()),(Some("merge"),2..=8,1)|(Some("split"),1,2..=8)) {rules.issue("proposal-merge-split-topology",location)?;}
    let expected:BTreeSet<_>=left.iter().filter_map(|r|s(r,"id")).flat_map(|old|right.iter().filter_map(|r|s(r,"id")).map(move|new|(old,new))).collect();let actual:Vec<_>=mappings.iter().filter_map(|m|s(m,"predecessor").zip(s(m,"successor"))).collect();
    if actual.len()!=expected.len()||actual.iter().copied().collect::<BTreeSet<_>>()!=expected {rules.issue("proposal-complete-mapping",location)?;}
    let previous=&value["supersedes_proposal"];
    if !previous.is_null() && (!exact_ref(previous,true)||s(previous,"id")==s(claim,"claim_id")) {rules.issue("proposal-predecessor-ref",location)?;}
    if claim.get("supersedes_claim_ref").and_then(Value::as_str)!=s(previous,"id") {rules.issue("proposal-succession-navigation",location)?;}
    for item in value["unresolved_links"].as_array().into_iter().flatten() {if !exact_ref(&item["claim"],true)||s(&item["claim"],"id")==s(claim,"claim_id") {rules.issue("proposal-unresolved-link-ref",location)?;}}
    for id in distinct {
        check(rules.limits.deadline,rules.cancelled)?;
        let entry=records.get(id).and_then(|r|kinds.get(r.kind.as_str())).and_then(|kind|types.get(kind));
        let eligible=entry.is_some_and(|entry|entry["abstract"]==false && (s(entry,"object_role")==Some("identity") || reader=="identity-transition-v2" && s(entry,"object_role")==Some("semantic") && s(&entry["source_record_profile"],"reader")==Some("semantic-metadata-v1") && s(&entry["source_record_profile"],"identity_proposal_adapter")==Some("exact-semantic-metadata-v1")));
        if !eligible {rules.issue("proposal-concrete-eligible-endpoint",location)?;}
        rules.read(PredicateRead::RefEndpoint {endpoint_type:"identity-proposal-participant".into(),id:id.into(),observed:if entry.is_some(){KeyState::Present}else{KeyState::Absent}})?;
    }
    rules.skip("identity-proposal-exact-version-reader-lineage-and-related-Claim-resolution")?;Ok(())
}

pub const BIBLIOGRAPHIC_DELTA_RULE_ID:&str="tos-source-bibliographic-compound-delta";
pub const BIBLIOGRAPHIC_DELTA_RULE_VERSION:&str="1";
/// The owning command binds these bytes to the selected immutable base and
/// candidate cut. Paths are source locators; these values carry no permission,
/// committed transport status or semantic admission.
pub struct BiblioDeltaInput<'a> {
    pub parent_path:&'a str,
    pub endpoint_path:&'a str,
    pub claim_path:&'a str,
    pub parent_before_raw:&'a [u8],
    pub parent_after_raw:&'a [u8],
    pub endpoint_raw:&'a [u8],
    pub claim_raw:&'a [u8],
}
/// Execute exact owner append/delta laws used by native topology, Collection
/// membership and Expression responsibility commands. This is source mechanics
/// over pinned candidate bytes; whole-plan/forms/dependency reconstruction,
/// initial absence, current lineage, grants and commit remain separate gates.
pub fn inspect_bibliographic_delta(input:BiblioDeltaInput<'_>,limits:ItemLimits,cancelled:&AtomicBool,schemas:&mut impl CutSchemaExecutor)->Result<RelationShadow,ItemRefusal> {
    let mut rules=Rules {limits,cancelled,state:std::mem::size_of::<Rules<'_>>(),ancestry_state:0,bytes:0,anchors:BTreeSet::new(),reserved:BTreeSet::new(),schema_seen:BTreeSet::new(),shadow:RelationShadow::default()};
    for raw in [input.parent_before_raw,input.parent_after_raw,input.endpoint_raw,input.claim_raw] {
        check(limits.deadline,cancelled)?;if raw.len()>limits.max_member_bytes{return Err(ItemRefusal::Budget);}account(&mut rules.bytes,raw.len(),limits.max_total_bytes)?;
    }
    let (before,cost)=legacy_decoded(input.parent_before_raw,&rules)?;reserve(&mut rules.state,cost,limits.max_state_bytes)?;
    let (after,cost)=legacy_decoded(input.parent_after_raw,&rules)?;reserve(&mut rules.state,cost,limits.max_state_bytes)?;
    let (endpoint,cost)=legacy_decoded(input.endpoint_raw,&rules)?;reserve(&mut rules.state,cost,limits.max_state_bytes)?;
    let (claim,claim_state)=strict_decoded(input.claim_raw,&rules)?;reserve(&mut rules.state,claim_state,limits.max_state_bytes)?;
    let predicate=s(&claim,"predicate").ok_or_else(||ItemRefusal::Unsupported("bibliographic delta predicate".into()))?;
    let (parent_kind,endpoint_kind,field,child_dir)=match predicate {
        "has_expression"=>("work","expression","expression_claim_refs",Some("expressions")),
        "embodied_by"=>("expression","edition","embodiment_claim_refs",Some("editions")),
        "exemplified_by"=>("edition","item","exemplar_claim_refs",Some("items")),
        "contains_work"=>("collection","work","membership_claim_refs",None),
        "translated_by"=>("expression","agent","responsibility_claim_refs",None),
        _=>return Err(ItemRefusal::Unsupported(format!("bibliographic delta profile:{predicate}"))),
    };
    for (path,raw,contract) in [(input.parent_path,input.parent_after_raw,"ToS/contracts/corpus-record.schema.json"),(input.endpoint_path,input.endpoint_raw,"ToS/contracts/corpus-record.schema.json"),(input.claim_path,input.claim_raw,"ToS/contracts/source-relation-claim.schema.json")] {
        check(limits.deadline,cancelled)?;
        if !schemas.check(path,raw,contract,limits.deadline,cancelled)? {rules.issue("bibliographic-delta-schema",path)?;}
        rules.read(PredicateRead::ExactPath {path:path.into(),digest:Digest256::of_bytes(raw).to_prefixed()})?;
    }
    rules.read(PredicateRead::ExactBytes {locator:format!("before:{}",input.parent_path),digest:Digest256::of_bytes(input.parent_before_raw).to_prefixed()})?;
    let parent_id=s(&before,"record_id");let endpoint_id=s(&endpoint,"record_id");let claim_id=s(&claim,"claim_id");
    if s(&before,"record_type")!=Some(parent_kind)||s(&after,"record_type")!=Some(parent_kind)||parent_id.is_none()||s(&after,"record_id")!=parent_id||s(&endpoint,"record_type")!=Some(endpoint_kind)||endpoint_id.is_none() {rules.issue("delta-preserved-typed-identities",input.parent_path)?;}
    if !before["record_version"].as_u64().filter(|v|*v>0).and_then(|v|v.checked_add(1)).is_some_and(|v|after["record_version"].as_u64()==Some(v)) {rules.issue("delta-one-successor-version",input.parent_path)?;}
    let refs=strings(&before,field);let unique:BTreeSet<_>=refs.iter().collect();let mut expected=refs.clone();if let Some(id)=claim_id {expected.push(id.into());}
    if !before[field].is_array()||before[field].as_array().unwrap().len()!=refs.len()||refs.len()!=unique.len()||claim_id.is_none()||claim_id.is_some_and(|id|refs.iter().any(|v|v==id))||after[field]!=json!(expected) {rules.issue("delta-exact-new-Claim-append",input.parent_path)?;}
    let (Some(before_fields),Some(after_fields))=(before.as_object(),after.as_object()) else {rules.issue("delta-parent-object",input.parent_path)?;return Ok(rules.shadow);};
    if before_fields.iter().filter(|(key,_)|key.as_str()!="record_version"&&key.as_str()!=field).any(|(key,value)|after_fields.get(key)!=Some(value)) || after_fields.keys().filter(|key|key.as_str()!="record_version"&&key.as_str()!=field).any(|key|!before_fields.contains_key(key)) {rules.issue("delta-descriptive-fields-preserved",input.parent_path)?;}
    if s(&claim,"schema_version")!=Some("tos_source_relation_claim_v1")||s(&claim,"claim_type")!=Some("relation")||s(&claim,"subject_ref")!=parent_id||s(&claim,"object")!=endpoint_id||claim["claim_version"]!=1||s(&claim,"review_status")!=Some("unreviewed")||claim.get("assessment_refs").is_some_and(|v|*v!=json!([]))||claim.get("supersedes_claim_ref").is_some_and(|v|!v.is_null())||s(&claim,"visibility")!=Some("public_metadata_only") {rules.issue("delta-unreviewed-new-typed-Claim",input.claim_path)?;}
    if let Some(directory)=child_dir {
        if s(&claim,"assertion_layer")!=Some("bibliographic_assertion")||s(&claim,"epistemic_status")!=Some("observed")||s(&claim,"polarity")!=Some("positive")||claim.get("confidence").is_some_and(|v|!v.is_null()) {rules.issue("delta-topology-initial-Claim-posture",input.claim_path)?;}
        if endpoint["record_version"]!=1||s(&endpoint,"identity_status")!=Some("provisional")||s(&endpoint,"same_as_posture")!=Some("no_equivalence_claim")||endpoint.get("supersedes_ref").is_some_and(|v|!v.is_null()) {rules.issue("delta-new-provisional-endpoint",input.endpoint_path)?;}
        let parent_relative=RelativePath::parse(input.parent_path).map_err(|_|ItemRefusal::Unsupported("delta parent path".into()))?;let endpoint_relative=RelativePath::parse(input.endpoint_path).map_err(|_|ItemRefusal::Unsupported("delta child path".into()))?;
        let parent_home=parent_relative.as_str().rsplit_once('/').map(|v|v.0).unwrap_or("");let child_home=endpoint_relative.as_str().rsplit_once('/').map(|v|v.0).unwrap_or("");let expected_home=format!("{parent_home}/{directory}/");
        let child_segment=child_home.strip_prefix(&expected_home).filter(|v|!v.is_empty()&&!v.contains('/'));
        let segment_valid=child_segment.is_some_and(|v|v.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||matches!(b,b'.'|b'-')) && !v.starts_with('.') && !v.starts_with('-') && !v.ends_with('.') && !v.ends_with('-') && !v.as_bytes().windows(2).any(|p|matches!(p[0],b'.'|b'-')&&matches!(p[1],b'.'|b'-')));
        if !input.parent_path.ends_with(&format!("/{parent_kind}.json"))||!input.endpoint_path.ends_with(&format!("/{endpoint_kind}.json"))||!segment_valid||!input.parent_path.starts_with("ToS/source-witnesses/")||matches!(parent_kind,"work"|"expression")&&!input.parent_path.starts_with("ToS/source-witnesses/works/")||parent_kind=="expression"&&!parent_home.rsplit_once('/').is_some_and(|(p,_)|p.ends_with("/expressions"))||parent_kind=="edition"&&!input.parent_path.split('/').any(|p|p=="editions") {rules.issue("delta-canonical-new-child-home",input.endpoint_path)?;}
        let evidence=strings(&claim,"evidence_refs");if evidence.len()!=2||evidence.iter().map(String::as_str).collect::<BTreeSet<_>>()!=BTreeSet::from([input.parent_path,input.endpoint_path]) {rules.issue("delta-exact-endpoint-evidence",input.claim_path)?;}
        let common=["schema_version","record_type","record_id","record_version","preferred_label","field_languages","variant_labels","identity_status","source_refs","external_identifiers","same_as_posture","notes","supersedes_ref"];
        let allowed_extra:&[&str]=match endpoint_kind {"edition"=>&["embodies_expression_refs","edition_statement","publication_claim_refs","provision_activity_claim_refs","exemplar_claim_refs","responsibility_claim_refs"],"item"=>&["item_manifest_ref"],_=>&[]};
        if matches!(endpoint_kind,"edition"|"item") && endpoint.as_object().is_some_and(|object|object.keys().any(|key|!common.contains(&key.as_str())&&!allowed_extra.contains(&key.as_str()))) {rules.issue("delta-child-scope-fields",input.endpoint_path)?;}
        let empty_fields:&[&str]=match endpoint_kind {"expression"=>&["responsibility_claim_refs","embodiment_claim_refs","derivation_claim_refs"],"edition"=>&["publication_claim_refs","exemplar_claim_refs","provision_activity_claim_refs","responsibility_claim_refs"],_=>&[]};
        for empty in empty_fields {if endpoint.get(*empty).is_some_and(|v|*v!=json!([]))||matches!(*empty,"responsibility_claim_refs"|"embodiment_claim_refs"|"publication_claim_refs"|"exemplar_claim_refs")&&endpoint.get(*empty).is_none() {rules.issue("delta-no-inferred-endpoint-Claims",input.endpoint_path)?;}}
        if endpoint_kind=="expression" && s(&endpoint,"work_ref")!=parent_id||endpoint_kind=="edition" && endpoint["embodies_expression_refs"]!=json!([parent_id])||endpoint_kind=="item" && s(&endpoint,"item_manifest_ref")!=Some(&format!("{child_home}/item.manifest.json")) {rules.issue("delta-exact-child-parent-or-manifest",input.endpoint_path)?;}
        for field in ["variant_labels","external_identifiers"] {if !endpoint[field].as_array().is_some_and(|a|a.iter().all(|v|v.is_object()&&if endpoint_kind=="expression" {s(v,"status")!=Some("verified")}else{s(v,"status")==Some("unverified")})) {rules.issue("delta-unverified-child-labels-and-identifiers",input.endpoint_path)?;}}
    } else {
        if !matches!(s(&claim,"assertion_layer"),Some("bibliographic_assertion"|"scholarly_report")) {rules.issue("delta-qualified-attachment-layer",input.claim_path)?;}
        qualified(&claim,predicate,&mut rules,input.claim_path)?;
        let evidence=strings(&claim,"evidence_refs");if evidence.is_empty()||evidence.iter().collect::<BTreeSet<_>>().len()!=evidence.len() {rules.issue("delta-explicit-attribution-evidence",input.claim_path)?;}
    }
    rules.checked(&format!("{BIBLIOGRAPHIC_DELTA_RULE_ID}@{BIBLIOGRAPHIC_DELTA_RULE_VERSION}:{predicate}"))?;
    rules.skip("whole-compound-plan-forms-dependency-byte-custody-current-lineage-and-permission-fence")?;
    check(limits.deadline,cancelled)?;Ok(rules.shadow)
}
