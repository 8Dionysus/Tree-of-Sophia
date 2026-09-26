//! Exact native decoded-field metadata predicates from the Python source owner.
//! These APIs do not open or verify external content, resolve selectors, replay text edits,
//! evaluate schemas, authenticate makers, or admit a source/translation/sign.
//! The caller separately owns selected schema execution and immutable bindings.
use std::collections::{BTreeMap,BTreeSet};
use std::sync::atomic::{AtomicBool,Ordering};
use std::time::Instant;
use serde_json::Value;
use tos_foundation::{Digest256,RelativePath};
use crate::{KeyState,PredicateRead};
use crate::item_rules::ItemRefusal;

#[derive(Debug,Clone,Copy)]
pub struct TextMetadataLimits {pub max_packet_bytes:usize,pub max_state_bytes:usize,pub max_issues:usize,pub deadline:Instant}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum TextMetadataState {CheckedMetadata,InvalidInput,Unsupported}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct TextMetadataIssue {pub code:&'static str,pub subject:String,pub message:String}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct TextMetadataReport {pub profile:&'static str,pub scope:&'static str,pub packet_digest:String,pub state:TextMetadataState,pub issues:Vec<TextMetadataIssue>,pub reads:Vec<PredicateRead>}
struct Metadata<'a>{path:&'a str,limits:TextMetadataLimits,cancelled:&'a AtomicBool,state_bytes:usize,report:TextMetadataReport}
impl Metadata<'_>{
    fn tick(&self)->Result<(),ItemRefusal>{if self.cancelled.load(Ordering::Relaxed){Err(ItemRefusal::Source("text metadata cancelled".into()))}else if Instant::now()>=self.limits.deadline{Err(ItemRefusal::Deadline)}else{Ok(())}}
    fn reserve(&mut self,bytes:usize)->Result<(),ItemRefusal>{self.tick()?;self.state_bytes=self.state_bytes.checked_add(bytes).filter(|n|*n<=self.limits.max_state_bytes).ok_or(ItemRefusal::Budget)?;Ok(())}
    fn issue(&mut self,message:impl Into<String>)->Result<(),ItemRefusal>{
        let message=message.into();if self.report.issues.len()>=self.limits.max_issues{return Err(ItemRefusal::Budget)}self.reserve(self.path.len()+message.len()+96)?;
        if self.report.state!=TextMetadataState::Unsupported{self.report.state=TextMetadataState::InvalidInput;}
        self.report.issues.push(TextMetadataIssue{code:"metadata-owner-predicate",subject:self.path.into(),message});Ok(())
    }
    fn read(&mut self,read:PredicateRead)->Result<(),ItemRefusal>{self.reserve(format!("{read:?}").len()+96)?;self.report.reads.push(read);Ok(())}
    fn index<'a>(&mut self,items:&'a[Value],key:&str,label:&str)->Result<BTreeMap<&'a str,&'a Value>,ItemRefusal>{
        let mut index=BTreeMap::new();for row in items.iter().filter(|r|r.is_object()){self.tick()?;if let Some(id)=row[key].as_str(){self.reserve(id.len()+64)?;if index.insert(id,row).is_some(){self.issue(format!("duplicate {label} identity: {id}"))?;}
            self.read(PredicateRead::UniqueKey{namespace:format!("metadata/{}/{}/{label}",self.report.profile,self.report.packet_digest),key:id.into(),owner:self.path.into()})?;}}
        Ok(index)
    }
    fn refs(&mut self,refs:Vec<&str>,known:&BTreeMap<&str,&Value>,label:&str)->Result<(),ItemRefusal>{for id in refs{let present=known.contains_key(id);self.read(PredicateRead::RefEndpoint{endpoint_type:format!("metadata/{}/{}/{label}",self.report.profile,self.report.packet_digest),id:id.into(),observed:if present{KeyState::Present}else{KeyState::Absent}})?;if !present{self.issue(format!("unresolved {label} reference: {id}"))?;}}Ok(())}
    fn anchor(&mut self,v:&Value)->Result<(),ItemRefusal>{
        if !v["supersedes_anchor_ref"].is_null()&&v["supersedes_anchor_ref"]==v["anchor_id"]{self.issue("source anchor cannot supersede itself")?;}
        let payload=&v["selector_payload"];let publication=&v["publication_boundary"];
        if !payload.is_object(){return Ok(())}
        if s(payload,"kind")=="withheld_selector_receipt"{if publication["source_text_in_record"]==true{self.issue("withheld selector receipt cannot claim source text in record")?;}return Ok(())}
        let expression=&payload["expression"];if !expression.is_object(){return Ok(())}
        let envelopes:Vec<&Value>=match s(expression,"mode"){
            "single"=>expression.get("selector").filter(|r|r.is_object()).into_iter().collect(),
            "refinement_chain"=>rows(expression,"steps").iter().filter(|r|r.is_object()).collect(),
            "alternatives"=>rows(expression,"alternatives").iter().filter(|r|r.is_object()).collect(),_=>rows(expression,"steps").iter().filter(|r|r.is_object()).collect()
        };
        let envelope_count=envelopes.len();let types:BTreeSet<String>=envelopes.iter().filter_map(|r|r.get("selector")).filter(|r|r.is_object()).map(|r|s(r,"type").to_owned()).collect();
        let target_digest=&v["target"]["file_sha256"];
        if matches!(s(expression,"mode"),"single"|"refinement_chain")&&!envelopes.is_empty()&&envelopes[0]["state"]["representation_sha256"]!=*target_digest{self.issue("first selector state is not the exact target file state")?;}
        if s(expression,"mode")=="alternatives"{
            let digests:BTreeSet<_>=envelopes.iter().filter(|r|r["state"].is_object()).map(|r|r["state"]["representation_sha256"].to_string()).collect();
            if digests!=BTreeSet::from([target_digest.to_string()]){self.issue("alternatives do not independently begin from the exact target state")?;}
        }
        for envelope in envelopes{
            self.tick()?;let selector=&envelope["selector"];let state=&envelope["state"];if !selector.is_object()||!state.is_object(){continue}
            let kind=s(selector,"type");if matches!(kind,"text_quote"|"text_position")&&state.get("character_normalization").is_none(){self.issue(format!("{kind} lacks explicit character normalization"))?;}
            if matches!(kind,"text_position"|"byte_position"){if let (Some(start),Some(end))=(int(&selector["start"])?,int(&selector["end"])?){if start>=end{self.issue(format!("{kind} interval is empty or reversed"))?;}}}
            if kind=="page_region"{
                let values=[number(&selector["x"])?,number(&selector["y"])?,number(&selector["width"])?,number(&selector["height"])?];
                if let [Some(x),Some(y),Some(width),Some(height)]=values{
                    match s(selector,"coordinate_space"){
                        "normalized_0_1"=>if x+width>1.0||y+height>1.0{self.issue("normalized page region exceeds the unit square")?;},
                        "pixels"|"points"=>if let(Some(source_width),Some(source_height))=(number(&selector["source_width"])?,number(&selector["source_height"])?){if x+width>source_width||y+height>source_height{self.issue("page region exceeds the declared representation extent")?;}},_=>{}
                    }
                }
            }
        }
        if s(publication,"record_storage")=="tracked"&&s(publication,"source_content_visibility")!="public"&&types.contains("text_quote"){self.issue("tracked nonpublic anchor cannot carry a text quote")?;}
        if publication["source_text_in_record"]==false&&types.contains("text_quote"){self.issue("text quote contradicts source_text_in_record=false")?;}
        if publication["source_text_in_record"]==true&&!types.contains("text_quote"){self.issue("source_text_in_record=true has no text-bearing selector")?;}
        if s(v,"resolution_status")=="mechanically_resolved"&&envelope_count==0{self.issue("mechanically resolved anchor has no selector expression")?;}
        Ok(())
    }
    fn layer(&mut self,v:&Value)->Result<(),ItemRefusal>{
        let id=s(v,"layer_id");if v["supersedes_layer_ref"]==v["layer_id"]{self.issue("source text layer cannot supersede itself")?;}
        let representation=&v["representation"];let scope=&representation["text_scope"];
        if let (Some(start),Some(end))=(int(&scope["start"])?,int(&scope["end"])?){if end<start{self.issue("representation text scope is reversed")?;}}
        let tracked=representation["tracked_content"]==true;let visibility=s(representation,"content_visibility");let published=representation["publication_authorized"]==true;let authority=rows(representation,"publication_authority_refs");
        if (s(representation,"storage")=="tracked")!=tracked{self.issue("representation storage and tracked-content posture disagree")?;}
        if tracked&&visibility!="public"{self.issue("tracked source text layer must be public content")?;}
        if published&&visibility!="public"{self.issue("nonpublic source text layer cannot authorize publication")?;}
        if published&&authority.is_empty(){self.issue("publication authorization has no authority reference")?;}
        if !published&&!authority.is_empty(){self.issue("publication authority references contradict the closed gate")?;}
        let anchor_ids:BTreeSet<_>=rows(&v["source_binding"],"anchors").iter().filter(|r|r.is_object()).map(|r|s(r,"anchor_id")).collect();let derivation=&v["derivation"];
        if rows(derivation,"input_layers").iter().any(|r|r.is_object()&&s(r,"layer_id")==id){self.issue("source text layer cannot derive from itself")?;}
        let payload=&derivation["change_payload"];let operations=if s(payload,"kind")=="explicit_operations"{rows(payload,"operations")}else{&[]};
        let mut last_input=-1i64;let mut last_output=-1i64;
        for operation in operations.iter().filter(|r|r.is_object()){
            self.tick()?;let edit=s(operation,"edit_id");
            for(prefix,last)in[("input",&mut last_input),("output",&mut last_output)]{
                let span=&operation[format!("{prefix}_span")];let start=int(&span["start"])?;let end=int(&span["end"])?;
                if let(Some(start),Some(end))=(start,end){if end<start{self.issue(format!("{edit} {prefix} span is reversed"))?;}if start<*last{self.issue(format!("explicit {prefix} edit spans overlap or are out of order"))?;}*last=(*last).max(end);}
                if let Some(exact)=operation[format!("{prefix}_exact")].as_str(){
                    if s(operation,&format!("{prefix}_sha256"))!=Digest256::of_bytes(exact.as_bytes()).to_hex(){self.issue(format!("{edit} {prefix} text digest drifted"))?;}
                    if let(Some(start),Some(end))=(start,end){if end.checked_sub(start)!=i64::try_from(exact.chars().count()).ok(){self.issue(format!("{edit} {prefix} span length differs from text"))?;}}
                }
            }
            let evidence=set(operation,"evidence_anchor_refs");if evidence.is_empty()||!evidence.is_subset(&anchor_ids){self.issue(format!("{edit} evidence anchors leave the source binding"))?;}
        }
        for annotation in rows(&v["uncertainty"],"annotations").iter().filter(|r|r.is_object()){
            self.tick()?;if !anchor_ids.contains(s(annotation,"anchor_ref")){self.issue("uncertainty annotation leaves the source binding")?;}
            for alternative in rows(annotation,"alternatives").iter().filter(|r|r.is_object()){
                let value=&alternative["value"];if alternative["value_in_record"]==true&&!value.is_string(){self.issue("in-record uncertainty alternative omits its value")?;}
                if alternative["value_in_record"]==false&&!value.is_null(){self.issue("withheld uncertainty alternative exposes a value")?;}
                if let Some(value)=value.as_str(){if s(alternative,"value_sha256")!=Digest256::of_bytes(value.as_bytes()).to_hex(){self.issue("uncertainty alternative digest drifted")?;}}
            }
        }Ok(())
    }
    fn unit(&mut self,v:&Value)->Result<(),ItemRefusal>{
        let schemes=self.index(rows(v,"schemes"),"scheme_id","text-unit scheme")?;let anchors=self.index(rows(v,"anchors"),"anchor_ref","text-unit anchor")?;let units=self.index(rows(v,"units"),"unit_id","text unit")?;let segmentations=self.index(rows(v,"segmentations"),"segmentation_id","text segmentation")?;let reviews=self.index(rows(v,"reviews"),"review_id","text-unit review")?;self.index(rows(v,"projections"),"projection_id","text-unit projection")?;
        if v["packet_id"]==v["supersedes_packet_ref"]{self.issue("source-text-unit packet cannot supersede itself")?;}
        let layer=&v["source_layer"];let layer_ref=&layer["text_layer_ref"];let layer_digest=&layer["text_layer_sha256"];
        let mut ordinals=BTreeSet::new();let mut intervals=BTreeMap::new();
        for anchor in rows(v,"anchors").iter().filter(|r|r.is_object()){
            self.tick()?;let id=s(anchor,"anchor_ref");if let Some(ordinal)=int(&anchor["ordinal"])?{if !ordinals.insert(ordinal){self.issue(format!("duplicate text-unit anchor ordinal: {ordinal}"))?;}}
            if anchor["text_layer_ref"]!=*layer_ref{self.issue(format!("anchor escapes frozen source layer: {id}"))?;}
            if anchor["text_layer_sha256"]!=*layer_digest{self.issue(format!("anchor source-layer digest drifted: {id}"))?;}
            let selector=&anchor["selector"];if let(Some(start),Some(end))=(int(&selector["start"])?,int(&selector["end"])?){
                if start>end{self.issue(format!("anchor selector is reversed: {id}"))?;continue;}
                if start==end&&s(anchor,"anchor_role")!="milestone"{self.issue(format!("non-milestone anchor selector is empty: {id}"))?;}
                self.reserve(id.len()+80)?;intervals.insert(id,(start,end));
            }
        }
        for scheme in rows(v,"schemes").iter().filter(|r|r.is_object()){
            self.tick()?;let id=s(scheme,"scheme_id");if scheme["scheme_id"]==scheme["supersedes_scheme_ref"]{self.issue(format!("text-unit scheme cannot supersede itself: {id}"))?;}
            if s(v,"content_posture")!="public_synthetic_contract_exercise"&&s(&scheme["method"],"maker_kind")=="synthetic_fixture"{self.issue(format!("synthetic scheme maker escaped the synthetic laboratory: {id}"))?;}
        }
        for unit in rows(v,"units").iter().filter(|r|r.is_object()){
            self.tick()?;let id=s(unit,"unit_id");if unit["unit_id"]==unit["supersedes_unit_ref"]{self.issue(format!("text unit cannot supersede itself: {id}"))?;}
            let refs=strs(unit,"ordered_anchor_refs");self.refs(refs.clone(),&anchors,"unit anchor")?;let spans:Vec<_>=refs.iter().filter_map(|id|intervals.get(id).copied()).collect();
            for pair in spans.windows(2){if pair[1].0<pair[0].1{self.issue(format!("unit anchor members overlap or reverse: {id}"))?;}}
            if s(unit,"continuity")=="contiguous"&&spans.len()>1&&spans.windows(2).any(|p|p[1].0!=p[0].1){self.issue(format!("contiguous unit has a gap between members: {id}"))?;}
            if s(unit,"continuity")=="discontinuous"&&spans.len()>1&&spans.windows(2).all(|p|p[1].0==p[0].1){self.issue(format!("discontinuous unit has no discontinuity: {id}"))?;}
            let parents=strs(unit,"parent_unit_refs");let children=strs(unit,"ordered_child_unit_refs");self.refs(parents.clone(),&units,"parent unit")?;self.refs(children.clone(),&units,"child unit")?;
            if parents.contains(&id){self.issue(format!("text unit is its own parent: {id}"))?;}
            if children.contains(&id){self.issue(format!("text unit is its own child: {id}"))?;}
            for parent in parents{if units.get(parent).is_some_and(|u|!strs(u,"ordered_child_unit_refs").contains(&id)){self.issue(format!("unit parent relation is not reciprocal: {id} -> {parent}"))?;}}
            for child in children{if units.get(child).is_some_and(|u|!strs(u,"parent_unit_refs").contains(&id)){self.issue(format!("unit child relation is not reciprocal: {id} -> {child}"))?;}}
        }
        for review in rows(v,"reviews").iter().filter(|r|r.is_object()){
            self.tick()?;let id=s(review,"review_id");let refs=strs(review,"segmentation_refs");self.refs(refs.clone(),&segmentations,"reviewed segmentation")?;self.refs(strs(review,"reviewed_unit_refs"),&units,"reviewed unit")?;
            if review["source_layer_ref"]!=*layer_ref{self.issue(format!("review escapes frozen source layer: {id}"))?;}
            if review["source_layer_sha256"]!=*layer_digest{self.issue(format!("review source-layer digest drifted: {id}"))?;}
            if review["language_competence"].is_object()&&layer.is_object()&&review["language_competence"]["language"]!=layer["language"]{self.issue(format!("review language competence does not match source: {id}"))?;}
            for segmentation in refs{if segmentations.get(segmentation).is_some_and(|s|!strs(s,"review_refs").contains(&id)){self.issue(format!("review-to-segmentation relation is not reciprocal: {id} -> {segmentation}"))?;}}
        }
        for segmentation in rows(v,"segmentations").iter().filter(|r|r.is_object()){
            self.tick()?;let id=s(segmentation,"segmentation_id");if segmentation["segmentation_id"]==segmentation["supersedes_segmentation_ref"]{self.issue(format!("text segmentation cannot supersede itself: {id}"))?;}
            let scheme_ref=s(segmentation,"scheme_ref");self.refs(vec![scheme_ref],&schemes,"segmentation scheme")?;let scheme=schemes.get(scheme_ref);let unit_refs=strs(segmentation,"ordered_unit_refs");self.refs(unit_refs.clone(),&units,"segmentation unit")?;
            if let Some(scheme)=scheme{let allowed=set(scheme,"unit_kinds");for unit_ref in &unit_refs{if units.get(unit_ref).is_some_and(|u|!allowed.contains(s(u,"unit_kind"))){self.issue(format!("segmentation unit kind is outside its scheme: {id} -> {unit_ref}"))?;}}}
            let competing=strs(segmentation,"competing_segmentation_refs");self.refs(competing.clone(),&segmentations,"competing segmentation")?;
            for other in competing{if other==id{self.issue(format!("text segmentation competes with itself: {id}"))?;}else if segmentations.get(other).is_some_and(|s|!strs(s,"competing_segmentation_refs").contains(&id)){self.issue(format!("competing-segmentation relation is not reciprocal: {id} -> {other}"))?;}}
            let review_refs=strs(segmentation,"review_refs");self.refs(review_refs.clone(),&reviews,"segmentation review")?;
            for review in &review_refs{if reviews.get(review).is_some_and(|r|!strs(r,"segmentation_refs").contains(&id)){self.issue(format!("segmentation-to-review relation is not reciprocal: {id} -> {review}"))?;}}
            let status=s(segmentation,"status");
            if matches!(status,"partially_reviewed"|"accepted")&&review_refs.is_empty(){self.issue(format!("reviewed segmentation lacks review: {id}"))?;}
            if status=="accepted"{
                let unit_set:BTreeSet<_>=unit_refs.iter().copied().collect();
                let full=review_refs.iter().filter_map(|id|reviews.get(id)).any(|r|s(r,"outcome")=="accepted"&&s(r,"review_scope")=="all_units"&&set(r,"reviewed_unit_refs")==unit_set&&s(r,"reviewer_kind")=="real_human"&&r["source_visible"]==true&&r["independent_boundary_decision_recorded_before_assistance"]==true&&r["language_competence"]["declared"]==true);
                if !full{self.issue(format!("accepted segmentation lacks full competent real-human review: {id}"))?;}
                for unit_ref in &unit_refs{if units.get(unit_ref).is_some_and(|u|s(u,"boundary_posture")!="reviewed_accepted"){self.issue(format!("accepted segmentation contains an unaccepted unit: {id} -> {unit_ref}"))?;}}
            }
            if status=="observed_source_structure"{
                if scheme.is_none_or(|scheme|!matches!(s(scheme,"analysis_role"),"source_layout"|"source_structure")||!matches!(s(scheme,"boundary_basis"),"source_layout"|"source_markup")){self.issue(format!("source-observed segmentation is not source-layout/markup based: {id}"))?;}
                for unit_ref in &unit_refs{if units.get(unit_ref).is_some_and(|u|s(u,"boundary_posture")!="source_attested"){self.issue(format!("source-observed segmentation contains a proposed unit: {id} -> {unit_ref}"))?;}}
            }
            let coverage=&segmentation["coverage"];let scope=s(coverage,"scope_anchor_ref");if !anchors.contains_key(scope){self.issue(format!("unresolved coverage scope anchor: {scope}"))?;continue;}
            let Some(scope_interval)=intervals.get(scope).copied()else{continue};let excluded=strs(coverage,"excluded_anchor_refs");self.refs(excluded.clone(),&anchors,"excluded coverage anchor")?;
            let mut all=Vec::new();
            for unit_ref in &unit_refs{if let Some(unit)=units.get(unit_ref){if s(unit,"surface_posture")=="source_bearing"{for anchor in strs(unit,"ordered_anchor_refs"){if let Some((start,end))=intervals.get(anchor){self.reserve(unit_ref.len()+96)?;all.push((*start,*end,(*unit_ref).to_owned()));}}}}}
            for anchor in &excluded{if let Some((start,end))=intervals.get(anchor){self.reserve(anchor.len()+105)?;all.push((*start,*end,format!("excluded:{anchor}")));}}
            for (start,end,member) in &all{if *start<scope_interval.0||*end>scope_interval.1{self.issue(format!("coverage member escapes scope: {id} -> {member}"))?;}}
            all.sort();self.tick()?;let posture=s(coverage,"coverage_posture");let overlaps=all.windows(2).any(|p|p[1].0<p[0].1);
            if matches!(posture,"exhaustive_nonoverlapping"|"declared_partial")&&overlaps{self.issue(format!("coverage overlaps without declaration: {id}"))?;}
            if posture=="overlap_declared"&&scheme.is_some_and(|s|s["policies"]["overlap"]!="allow_declared"){self.issue(format!("declared overlap conflicts with scheme policy: {id}"))?;}
            let mut merged:Vec<(i64,i64)>=Vec::new();for(start,end,_)in all{self.tick()?;if let Some(last)=merged.last_mut(){if start<=last.1{last.1=last.1.max(end);continue;}}merged.push((start,end));}
            if merged!=vec![scope_interval]{self.issue(format!("coverage does not reconstruct exact scope without hidden gaps: {id}"))?;}
            if posture!="declared_partial"&&!excluded.is_empty(){self.issue(format!("non-partial segmentation declares excluded ranges: {id}"))?;}
            if posture=="declared_partial"&&excluded.is_empty(){self.issue(format!("partial segmentation lacks explicit excluded ranges: {id}"))?;}
        }
        let used:BTreeSet<_>=rows(v,"segmentations").iter().filter(|r|r.is_object()).flat_map(|r|strs(r,"ordered_unit_refs")).collect();for id in units.keys().filter(|id|!used.contains(**id)){self.issue(format!("text unit is not owned by any segmentation: {id}"))?;}
        for(key,ref_key,index,items,label)in[("scheme_id","supersedes_scheme_ref",&schemes,rows(v,"schemes"),"scheme"),("unit_id","supersedes_unit_ref",&units,rows(v,"units"),"unit"),("segmentation_id","supersedes_segmentation_ref",&segmentations,rows(v,"segmentations"),"segmentation")]{
            for row in items.iter().filter(|r|r.is_object()){self.tick()?;let start=s(row,key);let mut seen=BTreeSet::from([start]);let mut cursor=row[ref_key].as_str();while let Some(id)=cursor{self.tick()?;let Some(prior)=index.get(id)else{break};if !seen.insert(id){self.issue(format!("{label} supersession contains a cycle: {start}"))?;break;}cursor=prior[ref_key].as_str();}}
        }
        let rights=&v["rights_and_visibility"];if rights.is_object()&&layer.is_object(){
            if rights["source_visibility"]!=layer["visibility"]{self.issue("source-layer and packet visibility differ")?;}
            if let(Some(source),Some(packet))=(rank(s(rights,"source_visibility")),rank(s(rights,"packet_visibility"))){if rank(s(rights,"effective_visibility"))!=Some(source.max(packet)){self.issue("effective visibility is not the most restrictive source or packet component")?;}}
            if rights["publication_authorized"]==true&&(s(rights,"effective_visibility")!="public"||rights["private_source_used"]==true||layer["publication_authorized"]!=true){self.issue("text-unit publication authority widens source boundary")?;}
        }
        for projection in rows(v,"projections").iter().filter(|r|r.is_object()){
            self.tick()?;let id=s(projection,"projection_id");let refs=strs(projection,"source_segmentation_refs");self.refs(refs.clone(),&segmentations,"projected segmentation")?;
            if s(projection,"projection_kind")=="graph"||s(projection,"admission_posture")=="accepted_only"{for ref_id in refs{if segmentations.get(ref_id).is_some_and(|s|s["status"]!="accepted"){self.issue(format!("accepted-only projection uses unaccepted segmentation: {id} -> {ref_id}"))?;}}}
            if let(Some(packet),Some(projected))=(rank(s(rights,"effective_visibility")),rank(s(projection,"visibility"))){if projected<packet{self.issue(format!("text-unit projection visibility widens packet boundary: {id}"))?;}}
        }Ok(())
    }
}
fn s<'a>(v:&'a Value,key:&str)->&'a str{v.get(key).and_then(Value::as_str).unwrap_or("")}
fn rows<'a>(v:&'a Value,key:&str)->&'a[Value]{v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])}
fn strs<'a>(v:&'a Value,key:&str)->Vec<&'a str>{rows(v,key).iter().filter_map(Value::as_str).collect()}
fn set<'a>(v:&'a Value,key:&str)->BTreeSet<&'a str>{strs(v,key).into_iter().collect()}
fn int(value:&Value)->Result<Option<i64>,ItemRefusal>{
    if value.is_boolean(){return Err(ItemRefusal::Unsupported("bool-as-int metadata numeric profile".into()))}
    if let Some(v)=value.as_i64(){return Ok(Some(v))}
    if let Some(number)=value.as_number(){let lexical=number.to_string();if !lexical.contains(['.','e','E']){return Err(ItemRefusal::Unsupported("metadata integer exceeds bounded signed integer profile".into()))}}
    Ok(None)
}
fn number(value:&Value)->Result<Option<f64>,ItemRefusal>{
    if value.is_boolean(){return Err(ItemRefusal::Unsupported("bool-as-number metadata numeric profile".into()))}
    if let Some(integer)=value.as_i64(){if integer.unsigned_abs()>9_007_199_254_740_992{return Err(ItemRefusal::Unsupported("metadata geometry integer exceeds exact binary64 range".into()))}return Ok(Some(integer as f64));}
    if let Some(number)=value.as_f64(){if number.is_finite()&&value.as_number().is_some_and(|n|n.to_string().contains(['.','e','E'])){return Ok(Some(number))}return Err(ItemRefusal::Unsupported("unsupported metadata geometry numeric representation".into()))}
    if value.is_number(){return Err(ItemRefusal::Unsupported("metadata geometry number is outside finite binary64".into()))}
    Ok(None)
}
fn rank(value:&str)->Option<u8>{match value{"public"=>Some(0),"public_metadata_only"=>Some(1),"controlled"=>Some(2),"local_only"=>Some(3),"restricted"=>Some(4),"unknown"=>Some(5),_=>None}}
fn inspect(raw:&[u8],path:&str,limits:TextMetadataLimits,cancelled:&AtomicBool,profile:&'static str,run:fn(&mut Metadata<'_>,&Value)->Result<(),ItemRefusal>)->Result<TextMetadataReport,ItemRefusal>{
    if limits.max_packet_bytes==0||limits.max_packet_bytes>2_097_152||limits.max_state_bytes==0||limits.max_state_bytes>134_217_728||limits.max_issues==0||limits.max_issues>8192{return Err(ItemRefusal::Budget)}
    RelativePath::parse(path).map_err(|_|ItemRefusal::Unsupported("text metadata packet path".into()))?;
    if raw.len()>limits.max_packet_bytes{return Err(ItemRefusal::Budget)}
    let mut metadata=Metadata{path,limits,cancelled,state_bytes:0,report:TextMetadataReport{profile,scope:"owner-metadata-predicates-only",packet_digest:Digest256::of_bytes(raw).to_hex(),state:TextMetadataState::CheckedMetadata,issues:Vec::new(),reads:Vec::new()}};
    metadata.reserve(raw.len().checked_mul(8).ok_or(ItemRefusal::Budget)?)?;
    metadata.read(PredicateRead::ExactPath{path:path.into(),digest:metadata.report.packet_digest.clone()})?;
    let value=match crate::native_decoded_value(raw,limits.max_packet_bytes){
        Ok(value)=>value,
        Err(ItemRefusal::Source(reason))=>{metadata.issue(reason)?;return Ok(metadata.report)},
        Err(ItemRefusal::Unsupported(reason))=>{metadata.report.state=TextMetadataState::Unsupported;metadata.issue(reason)?;return Ok(metadata.report)},
        Err(reason)=>return Err(reason),
    };
    if !value.is_object(){metadata.issue("metadata packet is not an object")?;return Ok(metadata.report)}
    if s(&value,"schema_version")!=profile{metadata.report.state=TextMetadataState::Unsupported;metadata.issue(format!("unsupported metadata profile: {}",s(&value,"schema_version")))?;return Ok(metadata.report)}
    match run(&mut metadata,&value){Ok(())=>{},Err(ItemRefusal::Unsupported(reason))=>{metadata.report.state=TextMetadataState::Unsupported;metadata.issue(reason)?;},Err(reason)=>return Err(reason)}
    metadata.tick()?;Ok(metadata.report)
}
pub fn inspect_source_anchor_v2_metadata(raw:&[u8],path:&str,limits:TextMetadataLimits,cancelled:&AtomicBool)->Result<TextMetadataReport,ItemRefusal>{inspect(raw,path,limits,cancelled,"tos_source_anchor_v2",Metadata::anchor)}
pub fn inspect_source_text_layer_metadata(raw:&[u8],path:&str,limits:TextMetadataLimits,cancelled:&AtomicBool)->Result<TextMetadataReport,ItemRefusal>{inspect(raw,path,limits,cancelled,"tos_source_text_layer_v1",Metadata::layer)}
pub fn inspect_source_text_unit_v1_metadata(raw:&[u8],path:&str,limits:TextMetadataLimits,cancelled:&AtomicBool)->Result<TextMetadataReport,ItemRefusal>{inspect(raw,path,limits,cancelled,"tos_source_text_unit_packet_v1",Metadata::unit)}
#[cfg(test)]
mod tests{
    use super::*;
    use std::time::Duration;
    type Inspect=fn(&[u8],&str,TextMetadataLimits,&AtomicBool)->Result<TextMetadataReport,ItemRefusal>;
    fn limits()->TextMetadataLimits{TextMetadataLimits{max_packet_bytes:1_048_576,max_state_bytes:16_777_216,max_issues:4096,deadline:Instant::now()+Duration::from_secs(10)}}
    fn fixture(relative:&str)->Vec<u8>{std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..").join("ToS/research-packets/foundation-laboratory-2026-07").join(relative)).unwrap()}
    fn changed(raw:&[u8],change:impl FnOnce(&mut Value))->Vec<u8>{let mut value:Value=serde_json::from_slice(raw).unwrap();change(&mut value);serde_json::to_vec(&value).unwrap()}
    #[test]
    fn exact_owner_metadata_fixtures_need_no_external_text_or_unicode_resolver(){
        let cancelled=AtomicBool::new(false);
        let cases:[(Inspect,&str);8]=[
            (inspect_source_anchor_v2_metadata,"source-anchor-v2-abc/variant-a.anchor.json"),
            (inspect_source_anchor_v2_metadata,"source-anchor-v2-abc/variant-b.anchor.json"),
            (inspect_source_anchor_v2_metadata,"source-anchor-v2-abc/variant-c.anchor.json"),
            (inspect_source_text_layer_metadata,"source-text-layer-abc/variant-a.layer.json"),
            (inspect_source_text_layer_metadata,"source-text-layer-abc/variant-b.layer.json"),
            (inspect_source_text_layer_metadata,"source-text-layer-abc/variant-c.layer.json"),
            (inspect_source_text_unit_v1_metadata,"source-text-unit-v1-abc/variant-a-source-layout-observation.json"),
            (inspect_source_text_unit_v1_metadata,"source-text-unit-v1-abc/variant-b-competing-segmentations.json")];
        for(inspect,path)in cases{let raw=fixture(path);let report=inspect(&raw,path,limits(),&cancelled).unwrap();assert_eq!(report.state,TextMetadataState::CheckedMetadata,"{:?}",report.issues);assert!(report.issues.is_empty());assert_eq!(report.packet_digest,Digest256::of_bytes(&raw).to_hex());assert_eq!(report.scope,"owner-metadata-predicates-only");assert!(report.reads.iter().any(|r|matches!(r,PredicateRead::ExactPath{digest,..}if digest==&report.packet_digest)));}
        let path="source-text-unit-v1-abc/variant-c-invalid-acceptance.json";let report=inspect_source_text_unit_v1_metadata(&fixture(path),path,limits(),&cancelled).unwrap();assert_eq!(report.issues.len(),6);assert!(report.issues.iter().any(|i|i.message.starts_with("accepted segmentation lacks full competent real-human review")));
    }
    #[test]
    fn metadata_layer_digest_coverage_and_selector_authority_defects_remain_visible(){
        let cancelled=AtomicBool::new(false);
        let raw=fixture("source-text-layer-abc/variant-b.layer.json");
        let raw=changed(&raw,|v|v["derivation"]["change_payload"]["operations"][0]["output_sha256"]=Value::String("0".repeat(64)));
        let report=inspect_source_text_layer_metadata(&raw,"changed.layer.json",limits(),&cancelled).unwrap();assert!(report.issues.iter().any(|i|i.message.ends_with("output text digest drifted")));
        let raw=fixture("source-anchor-v2-abc/variant-a.anchor.json");
        let raw=changed(&raw,|v|v["publication_boundary"]["source_text_in_record"]=Value::Bool(false));
        let report=inspect_source_anchor_v2_metadata(&raw,"changed.anchor.json",limits(),&cancelled).unwrap();assert!(report.issues.iter().any(|i|i.message=="text quote contradicts source_text_in_record=false"));
    }
    #[test]
    fn native_representation_numeric_budgets_and_cancellation_refuse(){
        let cancelled=AtomicBool::new(false);let raw=fixture("source-text-unit-v1-abc/variant-a-source-layout-observation.json");
        let unrepresentable=br#"{"schema_version":"tos_source_anchor_v2","extension":"\ud800"}"#;
        let report=inspect_source_anchor_v2_metadata(unrepresentable,"unrepresentable.json",limits(),&cancelled).unwrap();assert_eq!(report.state,TextMetadataState::Unsupported);assert_eq!(report.packet_digest,Digest256::of_bytes(unrepresentable).to_hex());assert_eq!(report.issues.len(),1);
        let malformed=br#"{"schema_version":"tos_source_anchor_v2",}"#;
        let report=inspect_source_anchor_v2_metadata(malformed,"malformed.json",limits(),&cancelled).unwrap();assert_eq!(report.state,TextMetadataState::InvalidInput);assert_eq!(report.issues.len(),1);
        let boolean=changed(&raw,|v|v["anchors"][0]["ordinal"]=Value::Bool(true));let report=inspect_source_text_unit_v1_metadata(&boolean,"boolean.json",limits(),&cancelled).unwrap();assert_eq!(report.state,TextMetadataState::Unsupported);
        let huge=changed(&raw,|v|v["anchors"][0]["ordinal"]=Value::from(u64::MAX));let report=inspect_source_text_unit_v1_metadata(&huge,"huge.json",limits(),&cancelled).unwrap();assert_eq!(report.state,TextMetadataState::Unsupported);
        let mut budget=limits();budget.max_state_bytes=1;assert_eq!(inspect_source_text_unit_v1_metadata(&raw,"budget.json",budget,&cancelled),Err(ItemRefusal::Budget));
        let mut deadline=limits();deadline.deadline=Instant::now();assert_eq!(inspect_source_text_unit_v1_metadata(&raw,"deadline.json",deadline,&cancelled),Err(ItemRefusal::Deadline));
        cancelled.store(true,Ordering::Relaxed);assert!(matches!(inspect_source_text_unit_v1_metadata(&raw,"cancelled.json",limits(),&cancelled),Err(ItemRefusal::Source(_))));
    }
}
