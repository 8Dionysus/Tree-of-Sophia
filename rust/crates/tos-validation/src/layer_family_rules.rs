//! Executable bounded source-layer family mechanics. These reports describe
//! exact owner predicates, never source admission, textual truth or rights.
//! Native JSON families retain Python's decoded-field semantics; the existing
//! text helpers retain their separate strict published profile.
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use serde_json::{Value, json};
use tos_foundation::{Digest256, RelativePath};
use crate::{KeyState, PredicateRead};
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::text_rules::{self, TextRuleReport, TextRuleState, LayerResource};

pub trait LayerFamilySource {
    fn current(&mut self, path: &str, max_bytes: usize, deadline: Instant) -> Result<Option<Vec<u8>>, ItemRefusal>;
    /// Resolves only the exact current or explicitly retained bytes. Never a
    /// mutable checkout, path search, unselected revision or network lookup.
    fn recorded(&mut self, path: &str, digest: &str, max_bytes: usize, deadline: Instant) -> Result<Option<Vec<u8>>, ItemRefusal>;
    fn schema(&mut self, path: &str, raw: &[u8], contract: &str, deadline: Instant) -> Result<bool, ItemRefusal>;
    fn generation(&self) -> String;
    fn checkpoint(&self, deadline: Instant) -> Result<(), ItemRefusal>;
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerFamilyIssue { pub path: String, pub code: &'static str, pub subject: String }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerProfileGap { pub path: String, pub profile: String }
#[derive(Debug, Clone, Default)]
pub struct LayerFamilyReport {
    pub issues: Vec<LayerFamilyIssue>,
    pub reads: Vec<PredicateRead>,
    /// Predicate names, not a completeness claim for a record or corpus.
    pub checked_predicates: Vec<(String, String)>,
    pub unsupported: Vec<LayerProfileGap>,
    pub text_reports: Vec<TextRuleReport>,
    pub metadata_bytes: u64,
}
pub struct LayerFamilyRules { limits: ItemLimits, state_bytes: usize, report: LayerFamilyReport, identities: BTreeMap<(String,String),String> }
impl LayerFamilyRules {
    pub fn new(limits: ItemLimits) -> Self { Self { limits, state_bytes: 0, report: LayerFamilyReport::default(), identities: BTreeMap::new() } }
    fn reserve(&mut self, n: usize) -> Result<(), ItemRefusal> {
        if Instant::now() >= self.limits.deadline { return Err(ItemRefusal::Deadline); }
        self.state_bytes = self.state_bytes.checked_add(n).filter(|n| *n <= self.limits.max_state_bytes).ok_or(ItemRefusal::Budget)?; Ok(())
    }
    fn issue(&mut self, path: &str, code: &'static str, subject: impl Into<String>) -> Result<(), ItemRefusal> {
        if self.report.issues.len() >= self.limits.max_issues { return Err(ItemRefusal::Budget); }
        let subject = subject.into(); self.reserve(path.len()+code.len()+subject.len()+96)?;
        self.report.issues.push(LayerFamilyIssue { path: path.into(), code, subject }); Ok(())
    }
    fn read(&mut self, read: PredicateRead) -> Result<(), ItemRefusal> { self.reserve(format!("{read:?}").len()+96)?; self.report.reads.push(read); Ok(()) }
    fn gap(&mut self, path: &str, profile: &str) -> Result<(), ItemRefusal> { self.reserve(path.len()+profile.len()+64)?; self.report.unsupported.push(LayerProfileGap { path:path.into(), profile:profile.into() }); Ok(()) }
    fn checked(&mut self, path: &str, predicate: &str) -> Result<(), ItemRefusal> { self.reserve(path.len()+predicate.len()+64)?; self.report.checked_predicates.push((path.into(),predicate.into())); Ok(()) }
    fn bytes(&mut self, source: &mut impl LayerFamilySource, path: &str, digest: Option<&str>) -> Result<Option<Vec<u8>>,ItemRefusal> {
        source.checkpoint(self.limits.deadline)?; safe(path)?;
        let raw = match digest { Some(d) => source.recorded(path,d,self.limits.max_member_bytes,self.limits.deadline)?, None => source.current(path,self.limits.max_member_bytes,self.limits.deadline)? };
        source.checkpoint(self.limits.deadline)?;
        if let Some(raw) = &raw {
            if raw.len()>self.limits.max_member_bytes { return Err(ItemRefusal::Budget); }
            if digest.is_some_and(|d| Digest256::of_bytes(raw).to_hex()!=d) { return Err(ItemRefusal::Source("layer-family recorded-input digest mismatch".into())); }
            self.report.metadata_bytes=self.report.metadata_bytes.checked_add(raw.len() as u64).filter(|n| *n<=self.limits.max_total_bytes).ok_or(ItemRefusal::Budget)?;
            // Charge simultaneously retained parser state as well as transport.
            self.reserve(raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
            self.read(PredicateRead::ExactBytes { locator: match digest { Some(d)=>format!("retained-or-current:{path}@{d}"),None=>path.into() }, digest:Digest256::of_bytes(raw).to_hex() })?;
        } else {
            self.read(PredicateRead::AbsentKey { namespace:format!("source-layer/{}",source.generation()), key:match digest { Some(d)=>format!("{path}@{d}"),None=>path.into() } })?;
            self.issue(path,"missing-exact-input",digest.unwrap_or("current"))?;
        }
        Ok(raw)
    }
    fn schema(&mut self,source:&mut impl LayerFamilySource,path:&str,raw:&[u8],contract:&str)->Result<bool,ItemRefusal>{
        let Some(schema)=self.bytes(source,contract,None)? else {return Err(ItemRefusal::Unsupported("missing current layer contract".into()))};
        self.read(PredicateRead::SchemaResource{uri:format!("https://tree-of-sophia.local/{contract}"),digest:Digest256::of_bytes(&schema).to_hex()})?;
        source.schema(path,raw,contract,self.limits.deadline)
    }
    pub fn record_gap(&mut self,path:&str,profile:&str)->Result<(),ItemRefusal>{self.gap(path,profile)}
    fn object(&mut self, source: &mut impl LayerFamilySource, path: &str, contract: Option<&str>) -> Result<Option<(Value,Vec<u8>)>,ItemRefusal> {
        let Some(raw)=self.bytes(source,path,None)? else { return Ok(None) };
        let Ok(value)=serde_json::from_slice::<Value>(&raw) else { self.issue(path,"invalid-json",path)?; return Ok(None) };
        if !value.is_object() { self.issue(path,"object-required",path)?; return Ok(None); }
        if let Some(contract)=contract {
            if !self.schema(source,path,&raw,contract)? { self.issue(path,"schema",contract)?; }
            self.checked(path,"Draft2020-12-owner-schema")?;
        }
        Ok(Some((value,raw)))
    }
    fn identity(&mut self, source:&impl LayerFamilySource,path:&str,namespace:&str,id:&str)->Result<(),ItemRefusal>{
        self.reserve(namespace.len()+id.len()+path.len()+96)?;
        if self.identities.insert((namespace.into(),id.into()),path.into()).is_some() { self.issue(path,"duplicate-identity",id)?; }
        self.read(PredicateRead::UniqueKey {namespace:namespace.into(),key:id.into(),owner:path.into()})?;
        self.read(PredicateRead::Range {namespace:namespace.into(),lower:id.into(),upper:id.into(),generation:source.generation()})
    }
    fn endpoint(&mut self,path:&str,label:&str,id:&str,present:bool)->Result<(),ItemRefusal>{
        self.read(PredicateRead::RefEndpoint {endpoint_type:format!("layer-packet/{path}/{label}"),id:id.into(),observed:if present {KeyState::Present}else{KeyState::Absent}})?;
        if !present { self.issue(path,"unresolved-local-reference",format!("{label}:{id}"))?; } Ok(())
    }
    pub fn inspect(&mut self,source:&mut impl LayerFamilySource,path:&str)->Result<(),ItemRefusal>{
        let Some((v,raw))=self.object(source,path,None)? else {return Ok(())};
        if path.ends_with("/artifact-witness.json") || path.ends_with("/composite-witness.json") || (path.ends_with("/representation.json") && (path.starts_with("ToS/source-witnesses/artifacts/") || path.starts_with("ToS/source-witnesses/scholarly-composites/"))) {
            return self.witness(source,path,&v,&raw);
        }
        let profile=s(&v,"schema_version");
        let contract=match profile {
            text_rules::TEXT_UNIT_PROFILE=>"source-text-unit-packet-v1.schema.json",
            text_rules::TEXT_LAYER_PROFILE=>"source-text-layer.schema.json",
            text_rules::ANCHOR_V2_PROFILE=>"source-anchor-v2.schema.json",
            "tos_semantic_ladder_packet_v4"=>"semantic-ladder-packet.schema.json",
            "tos_transfer_candidate_structural_crosswalk_v1"=>"transfer-candidate-structural-crosswalk.schema.json",
            "tos_semantic_annotation_packet_v2"=>"semantic-annotation-packet-v2.schema.json",
            "tos_translation_alignment_packet_v1"=>"translation-alignment-packet-v1.schema.json",
            _=>{self.gap(path,if profile.is_empty(){"unknown-layer-profile"}else{profile})?;return Ok(())}
        };
        let contract=format!("ToS/contracts/{contract}");
        let schema_valid=self.schema(source,path,&raw,&contract)?;
        if !schema_valid { self.issue(path,"schema",&contract)?; }
        self.checked(path,"Draft2020-12-owner-schema")?;
        match profile {
            "tos_semantic_ladder_packet_v4"=>self.semantic_ladder(path,&v)?,
            "tos_transfer_candidate_structural_crosswalk_v1"=>self.transfer(source,path,&v)?,
            text_rules::TEXT_UNIT_PROFILE|text_rules::TEXT_LAYER_PROFILE=>self.text(source,path,&v,&raw,profile,schema_valid)?,
            text_rules::ANCHOR_V2_PROFILE=>self.gap(path,"anchor-target-and-method-explicit-binding-required")?,
            "tos_semantic_annotation_packet_v2"=>{ self.packet_closure(path,&v,false)?; self.gap(path,"semantic-annotation-v2-review-competence-proposition-and-graph-owner-predicates")?; },
            "tos_translation_alignment_packet_v1"=>{ self.packet_closure(path,&v,true)?; self.gap(path,"translation-alignment-v1-side-cardinality-review-lineage-and-projection-owner-predicates")?; },
            _=>{}
        }
        source.checkpoint(self.limits.deadline)
    }
    /// Explicit anchor inputs avoid interpreting a source identity as a path.
    pub fn inspect_anchor(&mut self,source:&mut impl LayerFamilySource,path:&str,target:&str,target_digest:&str,method:&str,method_digest:&str)->Result<(),ItemRefusal>{
        let Some((_,raw))=self.object(source,path,None)? else{return Ok(())};
        let schema_valid=self.schema(source,path,&raw,"ToS/contracts/source-anchor-v2.schema.json")?;
        if !schema_valid{self.issue(path,"schema","source-anchor-v2")?;}
        let Some(bytes)=self.bytes(source,target,Some(target_digest))? else{return Ok(())};
        let Some(config)=self.bytes(source,method,Some(method_digest))? else{return Ok(())};
        let generation=source.generation();
        let report=text_rules::inspect_source_anchor_v2_single(&raw,&bytes,&config,&text_rules::AnchorRuleContext {anchor_path:path.into(),target_locator:target.into(),method_configuration_locator:method.into(),schema_checked:schema_valid,requested_profiles:vec![text_rules::ANCHOR_V2_PROFILE.into()],interval_generation:generation.clone(),reverse_generation:generation});
        self.absorb_text(path,report)
    }
    fn absorb_text(&mut self,path:&str,report:TextRuleReport)->Result<(),ItemRefusal>{
        if report.state==TextRuleState::BudgetExceeded{return Err(ItemRefusal::Budget)}
        for issue in &report.issues {self.issue(path,issue.code,&issue.subject)?;}
        for gap in &report.unsupported_profiles {self.gap(path,gap)?;}
        for read in &report.reads {self.read(read.clone())?;}
        // Preserve intervals/reverse facts alongside the helper's named scope.
        self.reserve(format!("{report:?}").len())?;
        self.report.text_reports.push(report); Ok(())
    }
    fn text(&mut self,source:&mut impl LayerFamilySource,path:&str,v:&Value,raw:&[u8],profile:&str,schema_valid:bool)->Result<(),ItemRefusal>{
        let generation=source.generation();
        if profile==text_rules::TEXT_UNIT_PROFILE {
            let binding=&v["source_layer"]; let locator=s(binding,"text_layer_ref");
            let Some(text)=self.bytes(source,locator,Some(s(binding,"text_layer_sha256")))? else {return Ok(())};
            let report=text_rules::inspect_source_text_unit_v1(raw,&text,&text_rules::TextRuleContext {packet_path:path.into(),frozen_text_locator:locator.into(),schema_checked:schema_valid,requested_profiles:vec![profile.into()],interval_generation:generation.clone(),reverse_generation:generation});
            self.absorb_text(path,report)?;
        } else {
            let mut bindings=BTreeMap::new();
            bindings.insert(s(&v["source_binding"],"source_file_ref").to_owned(),s(&v["source_binding"],"source_file_sha256").to_owned());
            bindings.insert(s(&v["representation"],"content_ref").to_owned(),s(&v["representation"],"content_sha256").to_owned());
            bindings.insert(s(&v["editorial_policy"],"policy_ref").to_owned(),s(&v["editorial_policy"],"policy_sha256").to_owned());
            for a in rows(&v["source_binding"],"anchors") {bindings.insert(s(a,"anchor_record_ref").into(),s(a,"anchor_record_sha256").into());}
            for input in rows(&v["derivation"],"input_layers") {
                let locator=s(input,"record_ref"); let digest=s(input,"record_sha256");
                if let Some(bytes)=self.bytes(source,locator,Some(digest))? {
                    if let Ok(predecessor)=serde_json::from_slice::<Value>(&bytes) {
                        bindings.insert(s(&predecessor["representation"],"content_ref").into(),s(&predecessor["representation"],"content_sha256").into());
                    }
                    bindings.insert(locator.into(),digest.into());
                }
            }
            for key in ["rights_record_refs","publication_authority_refs"] {
                for r in rows(&v["representation"],key) {bindings.insert(s(r,"ref").into(),s(r,"sha256").into());}
            }
            for maker in [&v["derivation"]["maker"]] {
                if maker["configuration_ref"].is_string() {bindings.insert(s(maker,"configuration_ref").into(),s(maker,"configuration_digest").into());}
            }
            let mut owned=Vec::new();
            for (locator,digest) in bindings {
                if locator.starts_with("tos.") {self.gap(path,"source-file-identity-resource-binding-required")?;continue;}
                if let Some(bytes)=self.bytes(source,&locator,Some(&digest))? {owned.push((locator,bytes));}
            }
            let resources:Vec<_>=owned.iter().map(|(locator,raw)|LayerResource {locator,raw}).collect();
            let report=text_rules::inspect_source_text_layer_v1(raw,&resources,&text_rules::LayerRuleContext {layer_path:path.into(),schema_checked:schema_valid,requested_profiles:vec![profile.into()],interval_generation:generation.clone(),reverse_generation:generation});
            self.absorb_text(path,report)?;
        } Ok(())
    }
    fn packet_closure(&mut self,path:&str,v:&Value,translation:bool)->Result<(),ItemRefusal>{
        let specs=if translation {vec![("alignments","alignment_id"),("reviews","review_id"),("projections","projection_id")]}else{vec![("entities","entity_id"),("claims","claim_id"),("relations","relation_id"),("reviews","review_id")]};
        let mut indices=BTreeMap::new();
        for (field,key) in specs { let mut ids=BTreeSet::new(); for row in rows(v,field) { let id=s(row,key); if !ids.insert(id.to_owned()) {self.issue(path,"duplicate-packet-identity",id)?;} } indices.insert(field,ids); }
        for (field,refs,target) in if translation {vec![("alignments","review_refs","reviews"),("alignments","competing_alignment_refs","alignments"),("reviews","reviewed_alignment_refs","alignments"),("projections","source_alignment_refs","alignments")]}else{vec![("entities","admission_review_refs","reviews"),("claims","review_refs","reviews"),("claims","competing_claim_refs","claims"),("relations","review_refs","reviews")]} {
            for row in rows(v,field) {for id in strs(row,refs) {self.endpoint(path,refs,id,indices[target].contains(id))?;}}
        }
        let rights=&v["rights_and_visibility"];
        if rights["publication_authorized"]==true && (rights["private_source_used"]==true || (!translation && matches!(s(rights,"source_content_visibility"),"local_only"|"restricted"|"unknown"))) {self.issue(path,"publication-boundary-widened",path)?;}
        self.checked(path,"packet-identity-review-competition-ref-closure-and-private-publication-ceiling")
    }
    fn semantic_ladder(&mut self,path:&str,v:&Value)->Result<(),ItemRefusal>{
        let stages:BTreeMap<_,_>=rows(v,"stages").iter().filter_map(|row|row["stage"].as_str().map(|id|(id,row))).collect();
        let empty=Value::Null; let stage=|name|stages.get(name).copied().unwrap_or(&empty);
        let active=|st:&Value| !matches!(s(st,"status"),"blocked"|"not-started");
        let result=&v["result"]; let candidate=stage("stable_sign_candidate"); let body=&candidate["body"];
        if body.is_object() && active(candidate) {
            if body["candidate_ref"]!=v["candidate_ref"] {self.issue(path,"candidate-identity-drift",s(v,"candidate_ref"))?;}
            let mut prior=BTreeSet::new();
            for name in ["exact_form","frequency_and_concordance","context","morphology","lemma","recurrence_within_section","recurrence_within_work","recurrence_within_author_corpus"] {prior.extend(strs(&stage(name)["body"],"occurrence_refs"));}
            let actual=set(body,"occurrence_refs");
            if actual.is_empty() || !actual.is_subset(&prior) {self.issue(path,"candidate-occurrence-evidence-unresolved",path)?;}
        }
        let manual=stage("manual_confirmation_or_rejection");
        if s(manual,"status")=="human-accepted" {
            if !manual["body"].is_object() || manual["body"]["accepted_sign_ref"]!=v["accepted_sign_ref"] {self.issue(path,"manual-sign-identity-drift",path)?;}
            if manual["body"].is_object() && !rows(result,"human_decision_refs").contains(&manual["body"]["review_receipt_ref"]) {self.issue(path,"manual-sign-receipt-absent",path)?;}
        }
        let relation=stage("relations_between_signs"); let rel_body=&relation["body"];
        let records=rows(rel_body,"relation_records");
        let relation_ids:BTreeSet<_>=records.iter().filter_map(|r|r["relation_ref"].as_str()).collect();
        let claim_ids:BTreeSet<_>=records.iter().filter_map(|r|r["claim_ref"].as_str()).collect();
        if !records.is_empty() {
            let endpoints:BTreeSet<_>=records.iter().flat_map(|r|[r["subject_sign_ref"].as_str(),r["object_sign_ref"].as_str()]).flatten().collect();
            if endpoints!=set(rel_body,"sign_refs") {self.issue(path,"relation-sign-endpoint-drift",path)?;}
            if !relation_ids.is_subset(&set(result,"relation_refs")) {self.issue(path,"relation-identity-absent",path)?;}
            if !claim_ids.is_subset(&set(result,"claim_refs")) {self.issue(path,"relation-claim-absent",path)?;}
        }
        let concept=stage("conceptual_interpretations"); let body=&concept["body"];
        if body.is_object() && active(concept) {
            if body["accepted_sign_ref"]!=v["accepted_sign_ref"] {self.issue(path,"concept-sign-identity-drift",path)?;}
            for field in ["concept_refs","claim_refs"] {if !set(body,field).is_subset(&set(result,field)) {self.issue(path,"concept-result-identity-absent",field)?;}}
        }
        let counter=stage("competing_readings"); let body=&counter["body"];
        if body.is_object() && active(counter) {
            let mut claims=set(body,"primary_claim_refs"); claims.extend(strs(body,"competing_claim_refs"));
            if !claims.is_subset(&set(result,"claim_refs")) {self.issue(path,"competing-reading-claim-absent",path)?;}
        }
        let graph=stage("graph_projection"); let body=&graph["body"];
        if s(graph,"status")=="projected" && body.is_object() {
            if !set(body,"relation_refs").is_subset(&relation_ids) {self.issue(path,"graph-relation-unresolved",path)?;}
            if !set(body,"claim_refs").is_subset(&set(result,"claim_refs")) {self.issue(path,"graph-claim-absent",path)?;}
            if !rows(result,"graph_projection_refs").contains(&body["projection_ref"]) {self.issue(path,"graph-projection-absent",path)?;}
        }
        self.checked(path,"_semantic_ladder_identity_issues/v4")
    }
    fn transfer(&mut self,source:&mut impl LayerFamilySource,path:&str,v:&Value)->Result<(),ItemRefusal>{
        let mut inputs=BTreeMap::new();
        for name in ["transfer_plan","candidate_anchor_set","target_numbered_unit_map","shared_label_correspondence","source_rights","target_rights"] {
            let binding=&v["inputs"][name]; let locator=s(binding,"ref"); let digest=s(binding,"sha256");
            let Some(raw)=self.bytes(source,locator,Some(digest))? else {return Ok(())};
            if !["transfer_plan","target_numbered_unit_map","shared_label_correspondence"].contains(&name){continue;}
            let Ok(input)=serde_json::from_slice::<Value>(&raw) else {self.issue(path,"invalid-crosswalk-input",name)?;return Ok(())};
            if !input.is_object() {self.issue(path,"crosswalk-input-object-required",name)?;return Ok(())}
            inputs.insert(name,input);
        }
        self.transfer_values(path,v,&inputs["transfer_plan"],&inputs["target_numbered_unit_map"],&inputs["shared_label_correspondence"])?;
        self.checked(path,"_transfer_candidate_crosswalk_issues/v1-and-six-exact-input-bindings")
    }
    fn transfer_values(&mut self,path:&str,v:&Value,plan:&Value,map:&Value,labels:&Value)->Result<(),ItemRefusal>{
        let expected:BTreeMap<_,_>=rows(plan,"candidate_target_units").iter().filter(|r|r.is_object() && r["work_ref"]==v["work_ref"]).map(|r|(s(r,"unit_id"),r)).collect();
        let pairings:BTreeMap<_,_>=rows(labels,"pairings").iter().filter(|r|r.is_object()).map(|r|(s(r,"unit_key"),r)).collect();
        for row in rows(map,"unit_starts") { if row["pdf_page"].is_number() && row["pdf_page"].as_i64().is_none() {return Err(ItemRefusal::Unsupported("crosswalk integer exceeds bounded native comparison profile".into()));} }
        for row in expected.values() {if row["page"].is_number() && row["page"].as_i64().is_none(){return Err(ItemRefusal::Unsupported("crosswalk integer exceeds bounded native comparison profile".into()));}}
        let starts:Vec<_>=rows(map,"unit_starts").iter().filter(|r|r.is_object() && r["pdf_page"].as_i64().is_some() && r["unit_key"].is_string()).collect();
        let candidates=rows(v,"candidates"); let actual:Vec<_>=candidates.iter().filter(|r|r.is_object()).map(|r|s(r,"candidate_unit_id")).collect();
        let actual_set:BTreeSet<_>=actual.iter().copied().collect();
        if actual.len()!=actual_set.len(){self.issue(path,"crosswalk-duplicate-candidate",path)?;}
        if actual_set!=expected.keys().copied().collect(){self.issue(path,"crosswalk-work-quota-drift",path)?;}
        let mut pairing_count=0usize; let mut on_page_count=0usize;
        for candidate in candidates {
            self.reserve(0)?;
            if !candidate.is_object(){self.issue(path,"crosswalk-non-object-candidate",path)?;continue;}
            let id=s(candidate,"candidate_unit_id"); let Some(expected)=expected.get(id) else {continue};
            for (actual,expected_key) in [("candidate_anchor_ref","anchor_ref"),("target_pdf_page","page"),("stratum","stratum")] {if candidate[actual]!=expected[expected_key]{self.issue(path,"crosswalk-candidate-binding-drift",format!("{id}/{actual}"))?;}}
            let Some(page)=expected["page"].as_i64() else {self.issue(path,"crosswalk-non-integer-page",id)?;continue;};
            let prior:Vec<_>=starts.iter().filter(|r|r["pdf_page"].as_i64().unwrap()<page).collect();
            let on_page:Vec<_>=starts.iter().filter(|r|r["pdf_page"].as_i64().unwrap()==page).collect();
            let following:Vec<_>=starts.iter().filter(|r|r["pdf_page"].as_i64().unwrap()>page).collect();
            let keys:Vec<_>=prior.last().into_iter().map(|r|s(r,"unit_key")).chain(on_page.iter().map(|r|s(r,"unit_key"))).collect();
            let on_keys:Vec<_>=on_page.iter().map(|r|s(r,"unit_key")).collect();
            if candidate["possible_unit_keys"]!=json!(keys){self.issue(path,"crosswalk-possible-unit-drift",id)?;}
            if candidate["starts_on_page_unit_keys"]!=json!(on_keys){self.issue(path,"crosswalk-on-page-unit-drift",id)?;}
            let relation=if on_page.is_empty(){"within-one-proposed-numbered-unit"}else{"prior-unit-spill-plus-unit-starts"};
            if s(candidate,"page_relation")!=relation{self.issue(path,"crosswalk-page-relation-drift",id)?;}
            if let Some(next)=following.first(){if candidate["next_proposed_start"]!=json!({"unit_key":next["unit_key"],"target_pdf_page":next["pdf_page"]}){self.issue(path,"crosswalk-next-start-drift",id)?;}}else{self.issue(path,"crosswalk-no-following-start",id)?;}
            for key in &keys {match pairings.get(key){None=>self.issue(path,"crosswalk-no-label-pairing",*key)?,Some(p) if p["translation_alignment_claimed"]!=false=>self.issue(path,"crosswalk-translation-alignment-widened",*key)?,_=>{}}}
            pairing_count+=keys.len(); on_page_count+=usize::from(!on_page.is_empty());
        }
        let summary=&v["summary"];
        if !summary.is_object(){self.issue(path,"crosswalk-summary-object-required",path)?;return Ok(())}
        let values=[("candidate_page_count",expected.len()),("random_page_count",expected.values().filter(|r|s(r,"stratum")=="random").count()),("hard_page_count",expected.values().filter(|r|s(r,"stratum")=="hard").count()),("page_with_unit_start_count",on_page_count),("page_without_unit_start_count",expected.len().saturating_sub(on_page_count)),("possible_pairing_count",pairing_count)];
        for (key,count) in values {if summary[key]!=json!(count){self.issue(path,"crosswalk-summary-drift",key)?;}}
        Ok(())
    }
    fn witness(&mut self,source:&mut impl LayerFamilySource,path:&str,v:&Value,raw:&[u8])->Result<(),ItemRefusal>{
        let composite=path.starts_with("ToS/source-witnesses/scholarly-composites/");
        let representation=path.ends_with("/representation.json");
        let id_key=if composite {"composite_id"}else{"artifact_id"};
        let kind=if composite {"scholarly-composite"}else{"artifact"};
        let contract=if representation {if composite {"scholarly-composite-file-representation.schema.json"}else{"artifact-visual-representation.schema.json"}}else if composite {"scholarly-composite-witness.schema.json"}else if s(v,"schema_version")=="tos_artifact_source_witness_v2" {"artifact-source-witness-v2.schema.json"}else{"artifact-source-witness.schema.json"};
        let contract=format!("ToS/contracts/{contract}");
        if !self.schema(source,path,raw,&contract)?{self.issue(path,"schema",&contract)?;}
        self.checked(path,"Draft2020-12-owner-schema")?;
        if representation {
            self.identity(source,path,"source-representation/file_id",s(v,"file_id"))?;
            if composite {self.identity(source,path,"composite-representation/id",s(v,"representation_id"))?;}
            let reference=s(v,if composite{"composite_ref"}else{"artifact_ref"});
            if let Some((owner,_))=self.object(source,reference,None)? {if owner[id_key]!=v[id_key]{self.issue(path,"representation-owner-id-drift",reference)?;}}
        }else{
            self.identity(source,path,&format!("{kind}/id"),s(v,id_key))?;
            let parent=path.rsplit_once('/').map(|(p,_)|p).unwrap_or("");
            if parent.split('/').any(|p|p.eq_ignore_ascii_case("cdli") || (composite&&(p.eq_ignore_ascii_case("dcclt")||p.eq_ignore_ascii_case("oracc")))) {self.issue(path,"provider-keyed-source-identity",parent)?;}
            for target in [s(v,"research_ref")].into_iter().chain(strs(v,"philosophy_planting_refs")) {self.bytes(source,target,None)?;}
            self.public_metadata_ceiling(path,v)?;
            if composite {
                let mut coverage=BTreeSet::new();
                for row in rows(v,"coverage_observations") {if !coverage.insert((s(row,"provider"),s(row,"surface"))){self.issue(path,"duplicate-composite-coverage",s(row,"provider"))?;}}
                // Composite member closure needs the complete source-owned artifact
                // identity namespace, not snapshot stable_ids navigation claims.
                for row in rows(v,"member_observations") {let id=s(row,"member_artifact_id"); let present=self.identities.contains_key(&("artifact/id".into(),id.into()));self.endpoint(path,"member-artifact",id,present)?;}
            }
        }
        let rights_path=s(v,"rights_ref");
        if let Some((rights,_))=self.object(source,rights_path,Some("ToS/contracts/rights-record.schema.json"))?{
            let required=if representation {if composite{vec![s(v,id_key),s(v,"representation_id"),s(v,"file_id")]}else{vec![s(v,id_key),s(v,"file_id")]}}else{vec![s(v,id_key)]};
            if rights["scope_refs"].is_array() && required.iter().any(|id|!strs(&rights,"scope_refs").contains(id)){self.issue(path,"witness-rights-scope-drift",rights_path)?;}
            let visibility=if !representation{"public_metadata_only"}else if composite{"local_only"}else{"public_payload"};
            let postures:&[&str]=if !representation {&["metadata_only"]}else if composite {&["not_authorized","unknown","authorized_with_conditions","authorized"]}else{&["authorized","authorized_with_conditions"]};
            if s(&rights,"visibility")!=visibility || !postures.contains(&s(&rights,"redistribution_posture")){self.issue(path,"witness-rights-posture-drift",rights_path)?;}
        }
        let discovery_path=s(v,"discovery_ref");
        if let Some((discovery,_))=self.object(source,discovery_path,Some("ToS/contracts/material-discovery-record.schema.json"))?{
            if !strs(&discovery["target"],"known_tos_refs").contains(&s(v,id_key)){self.issue(path,"witness-discovery-target-omits-id",discovery_path)?;}
            if !representation && s(&discovery["target"],"target_kind")!=kind {self.issue(path,"witness-discovery-target-kind-drift",discovery_path)?;}
            self.gap(path,"validated-discovery-and-provenance-event-output-closure")?;
        }
        if representation {self.gap(path,if composite{"composite-representation-payload-custody-fixity-and-provenance-digests"}else{"artifact-representation-payload-sha1-jpeg-tracking-and-provenance-digests"})?;}
        else if !composite {self.gap(path,"native-artifact-retained-creation-or-legacy-planting-provenance")?;}
        self.checked(path,"witness-identity-source-refs-metadata-ceiling-and-bound-rights-discovery-posture")
    }
    fn public_metadata_ceiling(&mut self,path:&str,v:&Value)->Result<(),ItemRefusal>{
        let mut stack=vec![v];
        while let Some(value)=stack.pop(){
            self.reserve(0)?;
            match value {
                Value::Object(map)=>{if map.keys().any(|key|["text","source_text","transliteration","translation","image_data","line_art_data","payload"].contains(&key.as_str())){self.issue(path,"metadata-content-exposure",path)?;break;} stack.extend(map.values());},
                Value::Array(rows)=>stack.extend(rows),
                Value::String(value) if ["/srv/","/home/","/tmp/","/var/tmp/"].iter().any(|p|value.starts_with(p))=>{self.issue(path,"metadata-owner-local-path-exposure",path)?;break;},
                _=>{}
            }
        }
        Ok(())
    }
    pub fn finish(self)->LayerFamilyReport {self.report}
}
fn s<'a>(v:&'a Value,key:&str)->&'a str{v.get(key).and_then(Value::as_str).unwrap_or("")}
fn rows<'a>(v:&'a Value,key:&str)->&'a [Value]{v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])}
fn strs<'a>(v:&'a Value,key:&str)->Vec<&'a str>{rows(v,key).iter().filter_map(Value::as_str).collect()}
fn set<'a>(v:&'a Value,key:&str)->BTreeSet<&'a str>{strs(v,key).into_iter().collect()}
fn safe(path:&str)->Result<(),ItemRefusal>{RelativePath::parse(path).map(|_|()).map_err(|_|ItemRefusal::Unsupported("unsafe source-layer path".into()))}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn limits()->ItemLimits {ItemLimits{max_member_bytes:2_097_152,max_total_bytes:32_000_000,max_state_bytes:64_000_000,max_issues:1000,deadline:Instant::now()+Duration::from_secs(10)}}
    struct Fixture {files:BTreeMap<String,Vec<u8>>, schemas:Vec<String>, lie:bool}
    impl LayerFamilySource for Fixture {
        fn current(&mut self,path:&str,_:usize,_:Instant)->Result<Option<Vec<u8>>,ItemRefusal>{Ok(self.files.get(path).cloned())}
        fn recorded(&mut self,path:&str,digest:&str,_:usize,_:Instant)->Result<Option<Vec<u8>>,ItemRefusal>{Ok(self.files.get(path).filter(|bytes|self.lie||Digest256::of_bytes(bytes).to_hex()==digest).cloned())}
        fn schema(&mut self,_:&str,_:&[u8],contract:&str,_:Instant)->Result<bool,ItemRefusal>{self.schemas.push(contract.into());Ok(true)}
        fn generation(&self)->String{"synthetic-current-cut".into()}
        fn checkpoint(&self,deadline:Instant)->Result<(),ItemRefusal>{if Instant::now()>=deadline{Err(ItemRefusal::Deadline)}else{Ok(())}}
    }
    fn crosswalk()->(Value,Fixture){
        let plan=json!({"candidate_target_units":[{"unit_id":"candidate","work_ref":"work","page":4,"anchor_ref":"anchor","stratum":"hard"}]});
        let map=json!({"unit_starts":[{"unit_key":"old","pdf_page":1},{"unit_key":"new","pdf_page":4},{"unit_key":"next","pdf_page":7}]});
        let labels=json!({"pairings":[{"unit_key":"old","translation_alignment_claimed":false},{"unit_key":"new","translation_alignment_claimed":false}]});
        let mut files=BTreeMap::new();let mut inputs=serde_json::Map::new();
        for (name,value) in [("transfer_plan",plan),("target_numbered_unit_map",map),("shared_label_correspondence",labels),("candidate_anchor_set",json!({})),("source_rights",json!({})),("target_rights",json!({}))]{let path=format!("ToS/research-packets/test/{name}.json");let raw=serde_json::to_vec(&value).unwrap();inputs.insert(name.into(),json!({"ref":path,"sha256":Digest256::of_bytes(&raw).to_hex()}));files.insert(path,raw);}
        files.insert("ToS/contracts/transfer-candidate-structural-crosswalk.schema.json".into(),b"{}".to_vec());
        let value=json!({"schema_version":"tos_transfer_candidate_structural_crosswalk_v1","work_ref":"work","inputs":inputs,"candidates":[{"candidate_unit_id":"candidate","candidate_anchor_ref":"anchor","target_pdf_page":4,"stratum":"hard","possible_unit_keys":["old","new"],"starts_on_page_unit_keys":["new"],"page_relation":"prior-unit-spill-plus-unit-starts","next_proposed_start":{"unit_key":"next","target_pdf_page":7}}],"summary":{"candidate_page_count":1,"random_page_count":0,"hard_page_count":1,"page_with_unit_start_count":1,"page_without_unit_start_count":0,"possible_pairing_count":2}});
        (value,Fixture{files,schemas:Vec::new(),lie:false})
    }
    fn run(value:&Value,mut fixture:Fixture)->(LayerFamilyReport,Fixture){let path="ToS/research-packets/test/crosswalk.json";fixture.files.insert(path.into(),serde_json::to_vec(value).unwrap());let mut rules=LayerFamilyRules::new(limits());rules.inspect(&mut fixture,path).unwrap();(rules.finish(),fixture)}
    #[test]
    fn transfer_source_schema_inputs_and_owner_predicates_remain_bound(){
        let (v,fixture)=crosswalk();let(report,fixture)=run(&v,fixture);
        assert!(report.issues.is_empty(),"{:?}",report.issues);
        assert!(report.unsupported.is_empty());
        assert_eq!(fixture.schemas,vec!["ToS/contracts/transfer-candidate-structural-crosswalk.schema.json"]);
        assert!(report.checked_predicates.iter().any(|(_,p)|p.starts_with("_transfer_candidate_crosswalk_issues")));
        assert!(report.reads.iter().any(|r|matches!(r,PredicateRead::ExactBytes{locator,..} if locator.contains("target_numbered_unit_map"))));
        let (mut v,fixture)=crosswalk();v["candidates"][0]["next_proposed_start"]["target_pdf_page"]=json!(8);v["summary"]["possible_pairing_count"]=json!(1);
        let(report,_)=run(&v,fixture);assert!(report.issues.iter().any(|i|i.code=="crosswalk-next-start-drift"));assert!(report.issues.iter().any(|i|i.code=="crosswalk-summary-drift"));
        let (v,mut fixture)=crosswalk();let path=v["inputs"]["shared_label_correspondence"]["ref"].as_str().unwrap();fixture.files.insert(path.into(),serde_json::to_vec(&json!({"pairings":[{"unit_key":"old","translation_alignment_claimed":true}]})).unwrap());
        let(report,_)=run(&v,fixture);assert!(report.issues.iter().any(|i|i.code=="missing-exact-input"));assert!(!report.checked_predicates.iter().any(|(_,p)|p.starts_with("_transfer_candidate_crosswalk_issues")));
    }
    #[test]
    fn retained_adapter_lies_and_state_deadlines_refuse(){
        let(v,mut fixture)=crosswalk();fixture.lie=true;let path=v["inputs"]["transfer_plan"]["ref"].as_str().unwrap();fixture.files.insert(path.into(),b"{}".to_vec());let packet="ToS/research-packets/test/crosswalk.json";fixture.files.insert(packet.into(),serde_json::to_vec(&v).unwrap());
        let mut rules=LayerFamilyRules::new(limits());assert!(matches!(rules.inspect(&mut fixture,packet),Err(ItemRefusal::Source(_))));
        let mut budget=limits();budget.max_state_bytes=1;let mut rules=LayerFamilyRules::new(budget);assert_eq!(rules.inspect(&mut fixture,packet),Err(ItemRefusal::Budget));
        let mut budget=limits();budget.deadline=Instant::now();let mut rules=LayerFamilyRules::new(budget);assert_eq!(rules.inspect(&mut fixture,packet),Err(ItemRefusal::Deadline));
    }
    #[test]
    fn semantic_ladder_keeps_candidate_evidence_and_graph_identities_distinct(){
        let value=json!({"candidate_ref":"candidate","accepted_sign_ref":"sign","stages":[{"stage":"exact_form","body":{"occurrence_refs":["occurrence"]}},{"stage":"stable_sign_candidate","status":"proposed","body":{"candidate_ref":"candidate","occurrence_refs":["other"]}},{"stage":"relations_between_signs","body":{"sign_refs":["sign","other-sign"],"relation_records":[{"relation_ref":"relation","claim_ref":"claim","subject_sign_ref":"sign","object_sign_ref":"other-sign"}]}},{"stage":"graph_projection","status":"projected","body":{"relation_refs":["wrong"],"claim_refs":["claim"],"projection_ref":"projection"}}],"result":{"relation_refs":["relation"],"claim_refs":["claim"],"graph_projection_refs":["projection"]}});
        let mut rules=LayerFamilyRules::new(limits());rules.semantic_ladder("fixture",&value).unwrap();let report=rules.finish();assert!(report.issues.iter().any(|i|i.code=="candidate-occurrence-evidence-unresolved"));assert!(report.issues.iter().any(|i|i.code=="graph-relation-unresolved"));
    }
}
