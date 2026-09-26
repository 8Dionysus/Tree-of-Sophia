//! Executable source-owned Claim and bibliographic closure families.
//! All source bytes come from the immutable cut; schemas execute through the
//! existing bounded worker. Local closure does not admit historical compounds.
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
const BASE: &str = "ToS/contracts/source-claim-record.schema.json";
const LEGACY_TOPOLOGY: [&str;3] = ["work-expression-claims.jsonl", "expression-edition-claims.jsonl", "edition-item-claims.jsonl"];
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
}
struct Route { reader: String, domain: Vec<String>, range: Vec<String>, layers: Vec<String>, versions: BTreeMap<String,String>, profile: Value }
struct Rules<'a> { limits: ItemLimits, cancelled: &'a AtomicBool, state: usize, bytes: u64, shadow: RelationShadow }
impl Rules<'_> {
    fn issue(&mut self, code: &'static str, location: &str) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        if self.shadow.issues.len() >= self.limits.max_issues { return Err(ItemRefusal::Budget); }
        reserve(&mut self.state, location.len()+64, self.limits.max_state_bytes)?;
        self.shadow.issues.push(RelationIssue { code, location: location.into() }); Ok(())
    }
    fn read(&mut self, row: PredicateRead) -> Result<(), ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        reserve(&mut self.state, format!("{row:?}").len()+64, self.limits.max_state_bytes)?;
        self.shadow.reads.push(row); Ok(())
    }
    fn skip(&mut self, reason: &str) -> Result<(), ItemRefusal> {
        reserve(&mut self.state, reason.len()+64, self.limits.max_state_bytes)?;
        self.shadow.unsupported = true; self.shadow.skipped_profiles.insert(reason.into()); Ok(())
    }
    fn endpoint(&mut self, id: &str, allowed: &[String], types: &BTreeMap<String,Value>, kinds: &BTreeMap<String,String>, records: &BTreeMap<String,BiblioCurrentRecord>, location: &str) -> Result<(),ItemRefusal> {
        let actual = records.get(id).and_then(|r| kinds.get(&r.kind));
        self.read(PredicateRead::RefEndpoint { endpoint_type: allowed.join("|"), id: id.into(), observed: if actual.is_some() { KeyState::Present } else { KeyState::Absent } })?;
        if !actual.is_some_and(|actual| ancestor(types, actual, allowed)) { self.issue("claim-endpoint-kind-or-missing", location)?; } Ok(())
    }
}
fn strings(row: &Value, field: &str) -> Vec<String> { row.get(field).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default() }
fn s<'a>(row: &'a Value, field: &str) -> Option<&'a str> { row.get(field).and_then(Value::as_str) }
fn ancestor(types: &BTreeMap<String, Value>, actual: &str, allowed: &[String]) -> bool {
    let mut pending=vec![actual.to_owned()]; let mut visited=BTreeSet::new();
    while let Some(id)=pending.pop() { if !visited.insert(id.clone()) { continue; } if allowed.contains(&id) { return true; } if let Some(row)=types.get(&id) { pending.extend(strings(row,"parent_type_ids")); } }
    false
}
fn decoded(raw: &[u8]) -> Result<Value,ItemRefusal> { serde_json::from_slice(raw).map_err(|_| ItemRefusal::Unsupported("bibliography native JSON representation".into())) }
fn owned(path: &str) -> bool { path.starts_with("ToS/source-witnesses/") && !path.split('/').any(|p| matches!(p,"catalog"|"owner-local"|"payload"|"local-content")) }
fn claim_stream(path: &str) -> bool { owned(path) && path.ends_with("-claims.jsonl") }

/// Requires record-family output from the same exact current cut. EOF is
/// checked again for this family's Claim/event/Item-manifest traversal.
pub fn inspect_bibliography_from_cut(cut: &CorpusCutReader, records: &SourceCutRecordReport, limits: ItemLimits, cancelled: &AtomicBool, schemas: &mut CutWorkerSchemaExecutor) -> Result<SourceCutBiblioReport, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    if schemas.source_revision() != cut.current().revision() { return Err(ItemRefusal::Source("bibliography schema cut mismatch".into())); }
    if records.source_revision != cut.current().revision() { return Err(ItemRefusal::Source("bibliography record cut mismatch".into())); }
    let mut rules=Rules { limits, cancelled, state:0, bytes:0, shadow:RelationShadow::default() };
    let mut registries=Vec::new();
    for (path, contract) in [(ENTITY,"ToS/contracts/semantic-entity-type-registry.schema.json"),(RELATION,"ToS/contracts/semantic-relation-type-registry.schema.json")] {
        let raw=current(cut,path,limits,cancelled,&mut rules.bytes)?;
        reserve(&mut rules.state,raw.len()*3,limits.max_state_bytes)?;
        if !schemas.check(path,&raw,contract,limits.deadline,cancelled)? { return Err(ItemRefusal::Unsupported(format!("invalid bibliography registry {path}"))); }
        schema_read(cut,contract,&mut rules)?;
        let value=decoded(&raw)?;
        rules.read(PredicateRead::Registry { uri:path.into(), version:value["registry_version"].to_string(), digest:Digest256::of_bytes(&raw).to_prefixed() })?;
        registries.push(value);
    }
    let mut types=BTreeMap::new(); let mut kinds=BTreeMap::new(); let mut routes=BTreeMap::new();
    for entry in registries[0]["types"].as_array().ok_or_else(|| ItemRefusal::Unsupported("entity registry types".into()))? {
        check(limits.deadline,cancelled)?;
        let id=s(entry,"type_id").ok_or_else(|| ItemRefusal::Unsupported("entity type ID".into()))?;
        if types.insert(id.to_owned(),entry.clone()).is_some() { rules.issue("duplicate-entity-type",id)?; }
        if let Some(mappings)=entry["source_mappings"].as_array() { for mapping in mappings { if s(mapping,"source_graph")==Some("source-claims") { if let Some(kind)=s(mapping,"source_kind_id") { if kinds.insert(kind.into(),id.into()).is_some() { rules.issue("duplicate-kind-owner",kind)?; } } } } }
    }
    for entry in registries[1]["relations"].as_array().ok_or_else(|| ItemRefusal::Unsupported("relation registry relations".into()))? {
        check(limits.deadline,cancelled)?;
        let Some(profile)=entry.get("source_claim_profile") else { continue; };
        let mappings:Vec<_>=entry["source_mappings"].as_array().into_iter().flatten().filter(|m| s(m,"source_graph")==Some("source-claims") && s(m,"scope")==Some("claim-predicate")).collect();
        if mappings.len()!=1 || entry["abstract"]!=false || s(entry,"assertion_mode")!=Some("reified-claim") || entry["evidence_required"]!=true { return Err(ItemRefusal::Unsupported("ambiguous Claim profile owner".into())); }
        let predicate=s(mappings[0],"source_predicate_id").ok_or_else(|| ItemRefusal::Unsupported("Claim predicate route".into()))?;
        let reader=s(profile,"reader").ok_or_else(|| ItemRefusal::Unsupported("Claim reader route".into()))?;
        let mut versions=BTreeMap::new();
        for schema in profile["schemas"].as_array().ok_or_else(|| ItemRefusal::Unsupported("Claim schema routes".into()))? {
            let version=s(schema,"schema_version").ok_or_else(|| ItemRefusal::Unsupported("Claim schema version".into()))?;
            let path=s(schema,"schema_ref").ok_or_else(|| ItemRefusal::Unsupported("Claim schema path".into()))?;
            if versions.insert(version.into(),path.into()).is_some() { return Err(ItemRefusal::Unsupported("duplicate Claim schema route".into())); }
            schema_read(cut,path,&mut rules)?;
            for dependency in strings(schema,"schema_dependencies") { schema_read(cut,&dependency,&mut rules)?; }
            reserve(&mut rules.state,predicate.len()+version.len()+path.len()+128,limits.max_state_bytes)?;
            rules.shadow.declared_profiles.insert(format!("{predicate}@{version}"));
        }
        if routes.insert(predicate.to_owned(),Route { reader:reader.into(),domain:strings(entry,"domain_type_ids"),range:strings(entry,"range_type_ids"),layers:strings(profile,"assertion_layers"),versions,profile:profile.clone() }).is_some() { return Err(ItemRefusal::Unsupported("duplicate Claim predicate owner".into())); }
    }
    let mut claims=Vec::new(); let mut events=BTreeMap::new(); let mut item_editions=BTreeMap::new(); let mut stream=cut.stream(cut.current().revision()).map_err(store_error)?;
    while let Some(member)=stream.next_member(limits.deadline,cancelled).map_err(store_error)? {
        check(limits.deadline,cancelled)?; account(&mut rules.bytes,member.raw.len(),limits.max_total_bytes)?;
        if member.raw.len()>limits.max_member_bytes { return Err(ItemRefusal::Budget); }
        let path=member.path.as_str();
        if !owned(path) { continue; }
        if path.ends_with("/item.manifest.json") {
            let value=decoded(&member.raw)?;
            if let (Some(id),Some(edition))=(s(&value,"item_id"),s(&value,"embodiment_ref")) {
                reserve(&mut rules.state,id.len()+edition.len()+128,limits.max_state_bytes)?;
                if item_editions.insert(id.to_owned(),edition.to_owned()).is_some() { rules.issue("duplicate-manifest-item",path)?; }
            }
        }
        if !path.ends_with(".jsonl") { continue; }
        rules.read(PredicateRead::ExactPath { path:path.into(),digest:Digest256::of_bytes(&member.raw).to_prefixed() })?;
        for (index,line) in member.raw.split(|b| *b==b'\n').enumerate() {
            check(limits.deadline,cancelled)?;
            if line.iter().all(u8::is_ascii_whitespace) { continue; }
            let value=decoded(line)?;
            if claim_stream(path) {
                if claims.len()>=65_536 { return Err(ItemRefusal::Budget); }
                reserve(&mut rules.state,line.len()*3+path.len()+128,limits.max_state_bytes)?;
                claims.push(BiblioClaim { path:path.into(),line:index+1,value,raw_sha256:Digest256::of_bytes(&member.raw).to_hex(),native:path.ends_with("/source-claims.jsonl") });
            } else if let Some(id)=s(&value,"event_id") {
                reserve(&mut rules.state,line.len()*3+id.len()+128,limits.max_state_bytes)?;
                if events.insert(id.to_owned(),value.clone()).is_some() { rules.issue("duplicate-biblio-event",path)?; }
            }
        }
    }
    let membership=stream.coverage().ok_or_else(|| ItemRefusal::Source("bibliography EOF missing".into()))?;
    if membership!=records.current_membership { return Err(ItemRefusal::Source("bibliography membership mismatch".into())); }
    rules.shadow.observed_endpoints=records.records.len(); rules.shadow.observed_claims=claims.len();
    rules.read(PredicateRead::Prefix { namespace:"source-current-Claim-files".into(),prefix:"ToS/source-witnesses/".into(),generation:membership.digest.to_prefixed() })?;
    let mut identities=BTreeSet::new(); let mut topology=Vec::new();
    for claim in &claims {
        inspect_claim(cut,claim,&routes,&types,&kinds,&records.records,&events,schemas,&mut rules)?;
        if let Some(id)=s(&claim.value,"claim_id") {
            if !identities.insert(id.to_owned()) { rules.issue("duplicate-claim-id",&format!("{}:{}",claim.path,claim.line))?; }
            rules.read(PredicateRead::UniqueKey { namespace:"source-claim-id".into(),key:id.into(),owner:format!("{}:{}",claim.path,claim.line) })?;
        }
        if matches!(s(&claim.value,"predicate"),Some("has_expression"|"embodied_by"|"exemplified_by")) { topology.push(claim.value.clone()); }
    }
    // The existing pure topology owner already covers declared forward/reverse
    // refs, endpoint pairs, multiplicity and Item-manifest edition agreement.
    // Native compounds must be verified before this owner API may claim a
    // verified union. A mixed union therefore refuses topology completeness.
    if records.records.len()>65_536 || topology.len()>65_536 { return Err(ItemRefusal::Budget); }
    let values:Vec<_>=records.records.values().map(|r| r.value.clone()).collect();
    reserve(&mut rules.state,values.iter().map(|v|v.to_string().len()*3).sum::<usize>()+topology.iter().map(|v|v.to_string().len()*3).sum::<usize>(),limits.max_state_bytes)?;
    let native_topology=claims.iter().any(|claim|claim.native && matches!(s(&claim.value,"predicate"),Some("has_expression"|"embodied_by"|"exemplified_by")));
    let topology_report=inspect_current_topology(&values,&topology,&item_editions,!native_topology,&membership.digest.to_prefixed());
    check(limits.deadline,cancelled)?;
    merge_shadow(&mut rules,topology_report)?;
    inspect_closure(&records.records,&claims,&membership.digest.to_prefixed(),&mut rules)?;
    // No complete-source verdict escapes this family. Native compound grants,
    // retained profile execution and exact legacy batch configurations remain
    // separate missing contracts even when the local structural checks pass.
    rules.skip("bibliography-legacy-batch-fixed-configuration-and-native-compound-history")?;
    rules.skip("retained-frozen-profile-source-admission")?;
    check(limits.deadline,cancelled)?;
    Ok(SourceCutBiblioReport { source_revision:cut.current().revision(),carrier_membership:membership,shadow:rules.shadow,claims,bytes_read:rules.bytes })
}

fn schema_read(cut:&CorpusCutReader,path:&str,rules:&mut Rules<'_>)->Result<(),ItemRefusal> {
    check(rules.limits.deadline,rules.cancelled)?;
    let relative=RelativePath::parse(path).map_err(|_|ItemRefusal::Unsupported("Claim schema dependency path".into()))?;
    let Some(member)=cut.current().member(&relative) else { return Err(ItemRefusal::Unsupported(format!("missing Claim schema dependency {path}"))); };
    rules.read(PredicateRead::SchemaResource { uri:format!("https://tree-of-sophia.local/{path}"),digest:member.sha256.to_prefixed() })
}

fn merge_shadow(rules: &mut Rules<'_>, shadow:RelationShadow) -> Result<(),ItemRefusal> {
    if shadow.issue_sink_truncated { return Err(ItemRefusal::Budget); }
    for issue in shadow.issues { rules.issue(issue.code,&issue.location)?; }
    for read in shadow.reads { rules.read(read)?; }
    for fact in shadow.facts { reserve(&mut rules.state,format!("{fact:?}").len()+64,rules.limits.max_state_bytes)?; rules.shadow.facts.push(fact); }
    for profile in shadow.skipped_profiles { rules.skip(&profile)?; }
    rules.shadow.checked_profiles.extend(shadow.checked_profiles); rules.shadow.unsupported |= shadow.unsupported; Ok(())
}

fn inspect_claim(cut:&CorpusCutReader, claim:&BiblioClaim, routes:&BTreeMap<String,Route>, types:&BTreeMap<String,Value>, kinds:&BTreeMap<String,String>, records:&BTreeMap<String,BiblioCurrentRecord>, events:&BTreeMap<String,Value>, schemas:&mut impl CutSchemaExecutor, rules:&mut Rules<'_>) -> Result<(),ItemRefusal> {
    let row=&claim.value; let location=format!("{}:{}",claim.path,claim.line);
    let basename=claim.path.rsplit('/').next().unwrap_or("");
    let Some(predicate)=s(row,"predicate") else { rules.issue("claim-predicate",&location)?; return Ok(()); };
    let bytes=serde_json::to_vec(row).map_err(|_|ItemRefusal::Unsupported("Claim worker serialization".into()))?;
    reserve(&mut rules.state,bytes.len(),rules.limits.max_state_bytes)?;
    if claim.native {
        let Some(route)=routes.get(predicate) else { rules.issue("unrecognized-predicate",&location)?; return Ok(()); };
        let Some(contract)=s(row,"schema_version").and_then(|v|route.versions.get(v)) else { rules.issue("unrecognized-Claim-schema-version",&location)?; return Ok(()); };
        if !schemas.check(&location,&bytes,contract,rules.limits.deadline,rules.cancelled)? { rules.issue("claim-profile-schema",&location)?; return Ok(()); }
        if !matches!(s(row,"visibility"),Some("public"|"public_metadata_only")) { rules.issue("claim-public-shape",&location)?; }
        if !s(row,"assertion_layer").is_some_and(|layer|route.layers.iter().any(|v|v==layer)) { rules.issue("claim-assertion-layer",&location)?; }
        if let Some(subject)=s(row,"subject_ref") { rules.endpoint(subject,&route.domain,types,kinds,records,&location)?; } else { rules.issue("claim-subject",&location)?; }
        match route.reader.as_str() {
            "identity-relation-v1"|"semantic-relation-v1" => {
                if let Some(object)=s(row,"object") { rules.endpoint(object,&route.range,types,kinds,records,&location)?; } else { rules.issue("claim-object-kind",&location)?; }
                rules.shadow.checked_profiles.insert(format!("{predicate}@{}",s(row,"schema_version").unwrap_or("")));
                // Ordinary domain/range checking does not accept the compound
                // append/revision plan or specialized semantic ownership.
                rules.skip(&format!("native-compound-or-semantic-evidence:{predicate}"))?;
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
            _ => rules.skip(&format!("source-Claim-reader:{}:{predicate}",route.reader))?,
        }
        if matches!(predicate,"contains_work"|"translated_by") { qualified(row,predicate,rules,&location)?; }
    } else {
        let (contract,subject_kind,object_kinds,expected,role)=match basename {
            "membership-claims.jsonl" => (BASE,"collection",vec!["work"],Some("contains_work"),Some("unreviewed-evidence-bearing-membership-claims")),
            "responsibility-claims.jsonl" => (BASE,"",vec!["agent"],None,None),
            "publication-claims.jsonl" => (BASE,"edition",vec![],None,Some("unreviewed-evidence-bearing-publication-claims")),
            "provision-activity-claims.jsonl" => (BASE,"edition",vec![],Some("provision_activity"),Some("unreviewed-evidence-bearing-provision-activity-claims")),
            "work-chronology-claims.jsonl" => (BASE,"work",vec![],Some("first_publication_chronology"),Some("unreviewed-evidence-bearing-work-chronology-claims")),
            "work-expression-claims.jsonl" => (BASE,"work",vec!["expression"],Some("has_expression"),Some("unreviewed-evidence-bearing-work-expression-claims")),
            "expression-edition-claims.jsonl" => (BASE,"expression",vec!["edition"],Some("embodied_by"),Some("unreviewed-evidence-bearing-expression-edition-claims")),
            "edition-item-claims.jsonl" => (BASE,"edition",vec!["item"],Some("exemplified_by"),Some("unreviewed-evidence-bearing-edition-item-claims")),
            "object-link-claims.jsonl" => ("ToS/contracts/object-link-claim.schema.json","",vec!["link"],None,None),
            _ => { rules.skip(&format!("non-bibliographic-legacy-stream:{basename}"))?; return Ok(()); }
        };
        schema_read(cut,contract,rules)?;
        if !schemas.check(&location,&bytes,contract,rules.limits.deadline,rules.cancelled)? { rules.issue("legacy-Claim-schema",&location)?; return Ok(()); }
        if expected.is_some_and(|p|p!=predicate) { rules.issue("legacy-Claim-predicate",&location)?; }
        if basename!="object-link-claims.jsonl" && s(row,"claim_type")!=Some("bibliographic") { rules.issue("legacy-Claim-type",&location)?; }
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
        if LEGACY_TOPOLOGY.contains(&basename) && claim.path!=format!("ToS/source-witnesses/relations/{basename}") { rules.issue("legacy-topology-owned-path",&location)?; }
        if matches!(basename,"publication-claims.jsonl"|"provision-activity-claims.jsonl") {
            let sibling=format!("{}/edition.json",claim.path.rsplit_once('/').unwrap().0);
            if !s(row,"subject_ref").and_then(|id|records.get(id)).is_some_and(|r|r.path==sibling) { rules.issue("legacy-edition-sibling-owner",&location)?; }
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
        if let Some(role)=role { bind_event(cut,claim,role,events,rules,&location)?; } else { rules.skip(&format!("legacy-batch-provenance-profile:{basename}"))?; }
        rules.shadow.checked_profiles.insert(format!("legacy-bibliography:{basename}"));
    }
    // Existence remains distinct from digest-bound original input resolution.
    for field in ["evidence_refs","counterevidence_refs"] { for reference in strings(row,field) {
        check(rules.limits.deadline,rules.cancelled)?;
        if !reference.starts_with("ToS/") { continue; }
        let path=RelativePath::parse(&reference).map_err(|_|ItemRefusal::Unsupported("Claim evidence path".into()))?;
        let present=cut.current().member(&path).is_some();
        rules.read(PredicateRead::IdentityKey { namespace:"source-current-path".into(),key:reference.clone(),observed:if present {KeyState::Present} else {KeyState::Absent} })?;
        if !present { rules.issue("Claim-evidence-current-file-missing",&location)?; }
    } }
    let fact=ValidationFact { namespace:"source-Claim-value".into(),key:s(row,"claim_id").unwrap_or(&location).into(),value_digest:Digest256::of_bytes(&bytes).to_prefixed() };
    reserve(&mut rules.state,format!("{fact:?}").len()+64,rules.limits.max_state_bytes)?;
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
        Some("described_by"|"metadata_at"|"downloadable_at"|"rights_statement_at")=>Some("association_claim_refs"),
        _ if row.path.ends_with("/publication-claims.jsonl")=>Some("publication_claim_refs"),
        _=>None,
    }
}
fn inspect_closure(records:&BTreeMap<String,BiblioCurrentRecord>,claims:&[BiblioClaim],generation:&str,rules:&mut Rules<'_>)->Result<(),ItemRefusal> {
    let mut union:BTreeMap<(String,String),BTreeSet<String>>=BTreeMap::new();
    let mut by_id=BTreeMap::new();
    let mut chronology_subjects=BTreeSet::new();
    for claim in claims {
        check(rules.limits.deadline,rules.cancelled)?;
        let (Some(id),Some(subject))=(s(&claim.value,"claim_id"),s(&claim.value,"subject_ref")) else { continue; };
        reserve(&mut rules.state,id.len()+subject.len()+128,rules.limits.max_state_bytes)?;
        by_id.entry(id).or_insert(claim);
        let Some(field)=closure_field(claim) else { continue; };
        let target=if field=="association_claim_refs" { s(&claim.value,"object").unwrap_or("") } else {subject};
        union.entry((target.into(),field.into())).or_default().insert(id.into());
        if field=="chronology_claim_refs" { chronology_subjects.insert(subject); }
    }
    let nietzsche:BTreeSet<_>=records.iter().filter(|(_,r)|r.kind=="work" && r.path.starts_with("ToS/source-witnesses/works/friedrich-nietzsche/")).map(|(id,_)|id.as_str()).collect();
    if chronology_subjects!=nietzsche { rules.issue("Nietzsche-chronology-subject-closure","ToS/source-witnesses/chronology/friedrich-nietzsche/first-publication")?; }
    for (id,record) in records {
        check(rules.limits.deadline,rules.cancelled)?;
        let fields:&[&str]=match record.kind.as_str() {"collection"=>&["membership_claim_refs"],"work"|"expression"=>&["responsibility_claim_refs"],"edition"=>&["responsibility_claim_refs","publication_claim_refs","provision_activity_claim_refs"],"link"=>&["association_claim_refs"],_=>&[]};
        for field in fields.iter().copied().chain((nietzsche.contains(id.as_str())).then_some("chronology_claim_refs")) {
            let refs=strings(&record.value,field); let actual:BTreeSet<_>=refs.iter().cloned().collect();
            let expected=union.get(&(id.clone(),field.into())).cloned().unwrap_or_default();
            if actual.len()!=refs.len() || actual!=expected { rules.issue("bibliography-exact-reverse-closure",&format!("{}#{field}",record.path))?; }
            rules.read(PredicateRead::ReverseRefs {target:id.clone(),relation:format!("bibliography:{field}"),generation:generation.into()})?;
            rules.read(PredicateRead::Range {namespace:"bibliography-current-Claim-field".into(),lower:format!("{field}:{id}"),upper:format!("{field}:{id}"),generation:generation.into()})?;
            if field=="chronology_claim_refs" && expected.len()!=1 { rules.issue("Nietzsche-one-chronology",&record.path)?; }
            if field=="association_claim_refs" {
                for reference in &expected { if let Some(claim)=by_id.get(reference.as_str()) { if claim.value.get("provenance_event_ref")!=record.value.get("provenance_event_ref") { rules.issue("Link-Claim-provenance-mismatch",&record.path)?; } } }
            }
            if field=="responsibility_claim_refs" && nietzsche.contains(id.as_str()) {
                let authors:Vec<_>=actual.iter().filter_map(|id|by_id.get(id.as_str())).filter(|c|s(&c.value,"predicate")==Some("authored_by")).collect();
                if authors.len()!=1 || s(&authors[0].value,"object")!=Some("tos.agent.friedrich-nietzsche") { rules.issue("Nietzsche-explicit-authorship",&record.path)?; }
            }
            let raw=serde_json::to_vec(&json!({"declared":actual,"observed":expected})).map_err(|_|ItemRefusal::Unsupported("closure fact representation".into()))?;
            reserve(&mut rules.state,raw.len()+id.len()+field.len()+128,rules.limits.max_state_bytes)?;
            rules.shadow.facts.push(ValidationFact {namespace:"bibliography-subject-closure".into(),key:format!("{field}:{id}"),value_digest:Digest256::of_bytes(&raw).to_prefixed()});
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration,Instant};
    fn rules(cancelled:&AtomicBool)->Rules<'_> { Rules {limits:ItemLimits {max_member_bytes:1_048_576,max_total_bytes:16_777_216,max_state_bytes:8_388_608,max_issues:64,deadline:Instant::now()+Duration::from_secs(5)},cancelled,state:0,bytes:0,shadow:RelationShadow::default()} }
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
    fn cancellation_and_state_budgets_refuse_without_success_projection() {
        let cancelled=AtomicBool::new(true);let mut r=rules(&cancelled);assert!(matches!(r.issue("example","x"),Err(ItemRefusal::Source(_))));
        let cancelled=AtomicBool::new(false);let mut r=rules(&cancelled);r.limits.max_state_bytes=1;assert!(matches!(r.read(PredicateRead::AbsentKey {namespace:"test".into(),key:"x".into()}),Err(ItemRefusal::Budget)));
        let mut r=rules(&cancelled);r.limits.max_issues=0;assert_eq!(r.issue("example","x"),Err(ItemRefusal::Budget));
    }
}
