//! Read-only reconstruction of maintained native bibliographic publications.
//! Historical transport evidence never grants a current writer or admission.
use crate::PredicateRead;
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::record_biblio_cut::{account, check, current, reserve};
use crate::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonEmissionProfile, JsonLimits, JsonMode, JsonValue,
    RelativePath, canonical_bytes_v1, canonical_count_v1, emit_json_profile, parse_json,
};
use tos_source_store::CorpusCutReader;

const HOME: &str = "ToS/source-witnesses";
const CONTROL: &str = "ToS/source-witnesses/.metadata-publication.json";
const TRANSACTIONS: &str = "ToS/source-witnesses/.metadata-transactions";
const HISTORY: &str = "source-revision-history.json";
const PROTOCOL: &str = "tos_selected_source_metadata_v1";
const MAX_GENERATION: u64 = 9_007_199_254_740_991;
const MAX_SIDE: usize = 8 * 1024 * 1024;
const MAX_FILE: usize = 2 * 1024 * 1024;
const MAX_MANIFEST: usize = 512 * 1024;
const MAX_HISTORY: usize = 128;
pub(crate) type Package = BTreeMap<String, Vec<u8>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTransportState {
    Committed,
    RolledBack,
    Pending,
    Orphan,
}

#[derive(Debug)]
pub struct NativeCompoundObservation {
    pub claim_path: String,
    pub claim_id: String,
    pub transaction_id: String,
    pub manifest_sha256: String,
    pub transport: NativeTransportState,
}

// Maintained bibliographic recipes share only their transport and exact
// buffer-construction law. These constants are owner profiles, not grants.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CompoundKind {
    CollectionWork,
    ExpressionResponsibility,
    WorkExpression,
    ExpressionEdition,
    EditionItem,
}
impl CompoundKind {
    fn from_operation(operation: &str) -> Result<Self, ItemRefusal> {
        match operation {
            "collection.work.attach" => Ok(Self::CollectionWork),
            "expression.responsibility.attach" => Ok(Self::ExpressionResponsibility),
            "work.expression.create" => Ok(Self::WorkExpression),
            "expression.edition.create" => Ok(Self::ExpressionEdition),
            "item.adopt" => Ok(Self::EditionItem),
            other => Err(ItemRefusal::Unsupported(format!(
                "retained compound parent handler {other}"
            ))),
        }
    }
    fn from_predicate(predicate: &str) -> Result<Self, ItemRefusal> {
        match predicate {
            "contains_work" => Ok(Self::CollectionWork),
            "translated_by" => Ok(Self::ExpressionResponsibility),
            "has_expression" => Ok(Self::WorkExpression),
            "embodied_by" => Ok(Self::ExpressionEdition),
            "exemplified_by" => Ok(Self::EditionItem),
            other => Err(ItemRefusal::Unsupported(format!(
                "native compound predicate {other}"
            ))),
        }
    }
    fn parent_kind(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection",
            Self::ExpressionResponsibility => "expression",
            Self::WorkExpression => "work",
            Self::ExpressionEdition => "expression",
            Self::EditionItem => "edition",
        }
    }
    fn child_kind(self) -> &'static str {
        match self {
            Self::CollectionWork => "work",
            Self::ExpressionResponsibility => "agent",
            Self::WorkExpression => "expression",
            Self::ExpressionEdition => "edition",
            Self::EditionItem => "item",
        }
    }
    fn parent_key(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection_id",
            Self::ExpressionResponsibility => "expression_id",
            Self::WorkExpression => "work_id",
            Self::ExpressionEdition => "expression_id",
            Self::EditionItem => "edition_id",
        }
    }
    fn child_key(self) -> &'static str {
        match self {
            Self::CollectionWork => "work_id",
            Self::ExpressionResponsibility => "agent_id",
            Self::WorkExpression => "expression_id",
            Self::ExpressionEdition => "edition_id",
            Self::EditionItem => "item_id",
        }
    }
    fn parent_path(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection_source_path",
            Self::ExpressionResponsibility => "expression_source_path",
            Self::WorkExpression => "work_source_path",
            Self::ExpressionEdition => "expression_source_path",
            Self::EditionItem => "edition_source_path",
        }
    }
    fn child_path(self) -> &'static str {
        match self {
            Self::CollectionWork => "work_source_path",
            Self::ExpressionResponsibility => "agent_source_path",
            Self::WorkExpression => "expression_source_path",
            Self::ExpressionEdition => "edition_source_path",
            Self::EditionItem => "item_source_path",
        }
    }
    fn parent_file(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection.json",
            Self::ExpressionResponsibility => "expression.json",
            Self::WorkExpression => "work.json",
            Self::ExpressionEdition => "expression.json",
            Self::EditionItem => "edition.json",
        }
    }
    fn child_file(self) -> &'static str {
        match self {
            Self::CollectionWork => "work.json",
            Self::ExpressionResponsibility => "agent.json",
            Self::WorkExpression => "expression.json",
            Self::ExpressionEdition => "edition.json",
            Self::EditionItem => "item.json",
        }
    }
    fn parent_forms(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection.human-forms.json",
            Self::ExpressionResponsibility => "expression.human-forms.json",
            Self::WorkExpression => "work.human-forms.json",
            Self::ExpressionEdition => "expression.human-forms.json",
            Self::EditionItem => "edition.human-forms.json",
        }
    }
    fn child_forms(self) -> &'static str {
        match self {
            Self::CollectionWork => "work.human-forms.json",
            Self::ExpressionResponsibility => "agent.human-forms.json",
            Self::WorkExpression => "expression.human-forms.json",
            Self::ExpressionEdition => "edition.human-forms.json",
            Self::EditionItem => "item.human-forms.json",
        }
    }
    fn child_form_request(self) -> &'static str {
        match self {
            Self::CollectionWork => "forms",
            Self::ExpressionResponsibility => "forms",
            Self::WorkExpression => "expression_forms",
            Self::ExpressionEdition => "edition_forms",
            Self::EditionItem => "item_forms",
        }
    }
    fn field(self) -> &'static str {
        match self {
            Self::CollectionWork => "membership_claim_refs",
            Self::ExpressionResponsibility => "responsibility_claim_refs",
            Self::WorkExpression => "expression_claim_refs",
            Self::ExpressionEdition => "embodiment_claim_refs",
            Self::EditionItem => "exemplar_claim_refs",
        }
    }
    fn predicate(self) -> &'static str {
        match self {
            Self::CollectionWork => "contains_work",
            Self::ExpressionResponsibility => "translated_by",
            Self::WorkExpression => "has_expression",
            Self::ExpressionEdition => "embodied_by",
            Self::EditionItem => "exemplified_by",
        }
    }
    fn operation(self) -> &'static str {
        match self {
            Self::CollectionWork => "collection.work.attach",
            Self::ExpressionResponsibility => "expression.responsibility.attach",
            Self::WorkExpression => "work.expression.create",
            Self::ExpressionEdition => "expression.edition.create",
            Self::EditionItem => "item.adopt",
        }
    }
    fn request_schema(self) -> &'static str {
        match self {
            Self::CollectionWork => "tos_local_collection_membership_command_v1",
            Self::ExpressionResponsibility => "tos_local_expression_responsibility_command_v1",
            Self::WorkExpression => "tos_local_work_expression_command_v1",
            Self::ExpressionEdition => "tos_local_expression_edition_command_v1",
            Self::EditionItem => "tos_local_item_adoption_command_v1",
        }
    }
    fn authorization_schema(self) -> &'static str {
        match self {
            Self::CollectionWork => "tos_collection_membership_authorization_v1",
            Self::ExpressionResponsibility => "tos_expression_responsibility_authorization_v1",
            Self::WorkExpression => "tos_work_expression_authorization_v1",
            Self::ExpressionEdition => "tos_expression_edition_authorization_v1",
            Self::EditionItem => "tos_item_adoption_authorization_v1",
        }
    }
    fn receipt_schema(self) -> &'static str {
        match self {
            Self::CollectionWork => "tos_collection_membership_receipt_v1",
            Self::ExpressionResponsibility => "tos_expression_responsibility_receipt_v1",
            Self::WorkExpression => "tos_work_expression_receipt_v1",
            Self::ExpressionEdition => "tos_expression_edition_receipt_v1",
            Self::EditionItem => "tos_edition_item_receipt_v1",
        }
    }
    fn receipt_file(self) -> &'static str {
        match self {
            Self::CollectionWork => "membership-attachment-receipt.json",
            Self::ExpressionResponsibility => "responsibility-attachment-receipt.json",
            Self::WorkExpression => "work-expression-receipt.json",
            Self::ExpressionEdition => "expression-edition-receipt.json",
            Self::EditionItem => "edition-item-receipt.json",
        }
    }
    fn module(self) -> &'static str {
        match self {
            Self::CollectionWork => "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_collection_commands.py",
            Self::ExpressionResponsibility => "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_responsibility_commands.py",
            Self::WorkExpression => {
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_expression_commands.py"
            }
            Self::ExpressionEdition => {
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_edition_commands.py"
            }
            Self::EditionItem => {
                "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_item_commands.py"
            }
        }
    }
    fn executor(self) -> &'static str {
        match self {
            Self::CollectionWork => "software:tos-source-membership-commands",
            Self::ExpressionResponsibility => "software:tos-source-responsibility-commands",
            Self::WorkExpression => "software:tos-source-expression-commands",
            Self::ExpressionEdition => "software:tos-source-edition-commands",
            Self::EditionItem => "software:tos-source-item-commands",
        }
    }
    fn procedure(self) -> &'static str {
        match self {
            Self::CollectionWork => "native-collection-membership-metadata-serialization",
            Self::ExpressionResponsibility => "native-expression-responsibility-metadata-serialization",
            Self::WorkExpression => "native-work-expression-metadata-serialization",
            Self::ExpressionEdition => "native-expression-edition-metadata-serialization",
            Self::EditionItem => "native-item-adoption-metadata-serialization",
        }
    }
    fn component(self) -> &'static str {
        match self {
            Self::CollectionWork => "ToS native Collection membership adapter",
            Self::ExpressionResponsibility => "ToS native Expression responsibility adapter",
            Self::WorkExpression => "ToS native Work Expression adapter",
            Self::ExpressionEdition => "ToS native Expression Edition adapter",
            Self::EditionItem => "ToS native Edition Item adapter",
        }
    }
    fn relation_attachment(self)->bool {matches!(self,Self::CollectionWork|Self::ExpressionResponsibility)}
    fn parent_form_grant(self)->&'static str {if self==Self::CollectionWork {"allowed_collection_form_ids"}else{"allowed_expression_form_ids"}}
    fn record_key(self)->&'static str {match self {Self::CollectionWork=>"work",Self::ExpressionResponsibility=>"agent",_=>"record"}}
    fn publication_home<'a>(self,scope:&'a Value)->Result<&'a str,ItemRefusal> {
        parent(text(scope,if self.relation_attachment() {"claim_source_path"}else{self.child_path()})?)
    }
    fn initial_backlink(self, record: &Value, parent: &Value) -> bool {
        match self {
            Self::ExpressionResponsibility => true, // Existing Agent receives no invented backlink.
            Self::CollectionWork => true, // Existing Work has no invented Collection backlink.
            Self::EditionItem => true, // Item backlink is its exact manifest path, checked at its scope/current read.
            Self::WorkExpression => record["work_ref"] == *parent,
            Self::ExpressionEdition => record["embodies_expression_refs"]
                .as_array()
                .is_some_and(|refs| refs.contains(parent)),
        }
    }
}

fn bad(message: &str) -> ItemRefusal {
    ItemRefusal::Source(format!("native compound: {message}"))
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, ItemRefusal> {
    v.get(key).and_then(Value::as_str).ok_or_else(|| bad(key))
}
fn integer(v: &Value, key: &str) -> Result<u64, ItemRefusal> {
    v.get(key).and_then(Value::as_u64).ok_or_else(|| bad(key))
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>, ItemRefusal> {
    v.get(key).and_then(Value::as_array).ok_or_else(|| bad(key))
}
fn keys(v: &Value, wanted: &[&str]) -> Result<(), ItemRefusal> {
    let object = v.as_object().ok_or_else(|| bad("object required"))?;
    if object.len() != wanted.len() || wanted.iter().any(|k| !object.contains_key(*k)) {
        return Err(bad("exact fields"));
    }
    Ok(())
}
fn hash(s: &str) -> Result<&str, ItemRefusal> {
    let raw = s
        .strip_prefix("sha256:")
        .ok_or_else(|| bad("prefixed sha256"))?;
    if raw.len() != 64
        || !raw
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(bad("sha256 grammar"));
    }
    Ok(raw)
}
fn limits() -> JsonLimits {
    JsonLimits::new(MAX_SIDE, 64, 300_000, 4_300).expect("finite JSON limits")
}
fn ordered(raw: &[u8]) -> Result<JsonValue, ItemRefusal> {
    parse_json(raw, JsonMode::PublishedStrict, limits())
        .map(|v| v.into_root())
        .map_err(|e| ItemRefusal::Unsupported(format!("compound published JSON: {e:?}")))
}
fn decode(raw: &[u8]) -> Result<Value, ItemRefusal> {
    ordered(raw)?;
    serde_json::from_slice(raw)
        .map_err(|_| ItemRefusal::Unsupported("compound decoded representation".into()))
}
fn canonical(v: &Value) -> Result<Vec<u8>, ItemRefusal> {
    canonical_ordered(&ordered(
        &serde_json::to_vec(v).map_err(|_| bad("serialization"))?,
    )?)
}
fn canonical_ordered(v: &JsonValue) -> Result<Vec<u8>, ItemRefusal> {
    canonical_bytes_v1(v, CanonicalProfile::SourceCommandInputV1, limits())
        .map_err(|e| ItemRefusal::Unsupported(format!("compound canonical: {e:?}")))
}
fn digest(v: &Value) -> Result<String, ItemRefusal> {
    Ok(Digest256::of_bytes(&canonical(v)?).to_prefixed())
}
fn pretty(v: &JsonValue) -> Result<Vec<u8>, ItemRefusal> {
    emit_json_profile(v, JsonEmissionProfile::SourceFormSetPublishedV1, limits())
        .map(|v| v.bytes)
        .map_err(|e| ItemRefusal::Unsupported(format!("compound record bytes: {e:?}")))
}
fn reference(v: &Value, id: &str, version: &str) -> Result<Value, ItemRefusal> {
    let n = integer(v, version)?;
    if n == 0 {
        return Err(bad("positive record version"));
    }
    Ok(json!({"id":text(v,id)?,"version":n,"digest":digest(v)?}))
}
// The real reference has these three owned fields. Price its shape without
// constructing a stand-in reference or executing its source digest.
fn reference_state(value:&Value,id:&str,version:&str)->Result<usize,ItemRefusal> {
    let id=text(value,id)?;let number=value.get(version).and_then(Value::as_number).ok_or_else(||bad("record version number"))?;
    std::mem::size_of::<Value>().checked_add(3*std::mem::size_of::<(String,Value)>()).and_then(|n|n.checked_add("id".len()+"version".len()+"digest".len())).and_then(|n|n.checked_add(id.len())).and_then(|n|n.checked_add(number.as_str().len())).and_then(|n|n.checked_add("sha256:".len()+2*std::mem::size_of::<Digest256>())).ok_or(ItemRefusal::Budget)
}
fn file_refs(files: &Package) -> Value {
    Value::Object(
        files
            .iter()
            .map(|(name, raw)| {
                (
                    name.clone(),
                    json!({"sha256":Digest256::of_bytes(raw).to_prefixed(),"bytes":raw.len()}),
                )
            })
            .collect(),
    )
}
fn selected_names(path: &str) -> Result<[String; 3], ItemRefusal> {
    let base = path
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| bad("record path"))?;
    let stem = base
        .strip_suffix(".json")
        .ok_or_else(|| bad("record basename"))?;
    Ok([
        base.into(),
        format!("{stem}.human-forms.json"),
        HISTORY.into(),
    ])
}
fn parent(path: &str) -> Result<&str, ItemRefusal> {
    path.rsplit_once('/')
        .map(|v| v.0)
        .ok_or_else(|| bad("source parent"))
}
fn metadata_path(path: &str, directory: bool) -> Result<(), ItemRefusal> {
    RelativePath::parse(path).map_err(|_| bad("canonical metadata path"))?;
    let parts: Vec<_> = path.split('/').collect();
    if path.len() > 1024
        || !(3..=24).contains(&parts.len())
        || !path.starts_with(&format!("{HOME}/"))
        || parts.iter().any(|p| {
            p.starts_with('.')
                || matches!(
                    *p,
                    "payload" | "private" | "local-content" | "owner-local" | "catalog"
                )
        })
        || !directory && (parts.len() < 4 || !(path.ends_with(".json") || path.ends_with(".jsonl")))
    {
        return Err(bad("public metadata path scope"));
    }
    Ok(())
}
fn is_ancestor(a: &str, b: &str) -> bool {
    b.strip_prefix(a).is_some_and(|tail| tail.starts_with('/'))
}

struct Transaction {
    manifest: Value,
    manifest_sha256: String,
    status: String,
    files: BTreeMap<String, (Option<Vec<u8>>, Option<Vec<u8>>)>,
}

/// One family invocation owns the directory index and deduplicated exact reads.
/// Neither cache nor historical state survives the selected operation.
pub(crate) struct NativeCompoundReader<'a> {
    cut: &'a CorpusCutReader,
    limits: ItemLimits,
    cancelled: &'a AtomicBool,
    paths: BTreeSet<&'a str>,
    raw: BTreeMap<String, Vec<u8>>,
    raw_cache_state: usize,
    transactions: BTreeMap<String, std::sync::Arc<Transaction>>,
    histories: BTreeMap<(String, String), std::sync::Arc<Value>>,
    state: usize,
    temporary_state: usize,
    bytes: u64,
    reads: Vec<PredicateRead>,
    publication: Option<Value>,
}
impl<'a> NativeCompoundReader<'a> {
    pub(crate) fn new(
        cut: &'a CorpusCutReader,
        limits: ItemLimits,
        cancelled: &'a AtomicBool,
    ) -> Result<Self, ItemRefusal> {
        let mut this = Self {
            cut,
            limits,
            cancelled,
            paths: BTreeSet::new(),
            raw: BTreeMap::new(),
            raw_cache_state: 0,
            transactions: BTreeMap::new(),
            histories: BTreeMap::new(),
            state: std::mem::size_of::<Self>(),
            temporary_state: 0,
            bytes: 0,
            reads: Vec::new(),
            publication: None,
        };
        for member in cut.current().members() {
            check(limits.deadline, cancelled)?;
            reserve(
                &mut this.state,
                std::mem::size_of::<&str>(),
                limits.max_state_bytes,
            )?;
            this.paths.insert(member.path.as_str());
        }
        let startup_temporary=this.temporary_state;
        if let Some(raw) = this.optional(CONTROL, 8192)? {
            let state=this.decoded(&raw)?;
            let retained=crate::record_biblio_cut::decoded_state(&state)?;
            this.temporary(retained)?;
            state_valid_with(&state,&mut |value|this.canonical_observation(value))?;this.release_temporary(retained);
            // Move the same tree from the temporary scope into publication.
            this.temporary_state-=retained;
            this.publication=Some(state);
        }
        this.release_temporary_since(startup_temporary);
        this.release_raw_cache();
        Ok(this)
    }
    // `verify` has released its reconstruction temporaries and optional raw
    // cache before bibliography grows Rules. The remaining state still owns
    // the selected index, publication, transaction/history caches and reads.
    pub(crate) fn retained_state_bytes(&self) -> usize {
        self.state
    }
    pub(crate) fn set_remaining_state(&mut self, available: usize) -> Result<(), ItemRefusal> {
        if self.state > available {
            return Err(ItemRefusal::BudgetCheck {check:"compound retained state after Rules growth",used:Some(self.state as u64),limit:Some(available as u64)});
        }
        self.limits.max_state_bytes = available;
        Ok(())
    }
    fn temporary(&mut self, amount: usize) -> Result<(), ItemRefusal> {
        let next=self.state.checked_add(amount);
        self.state=next.filter(|n|*n<=self.limits.max_state_bytes)
            .ok_or(ItemRefusal::BudgetCheck {check:"compound live reconstruction/history state",used:next.map(|n|n as u64),limit:Some(self.limits.max_state_bytes as u64)})?;
        self.temporary_state = self
            .temporary_state
            .checked_add(amount)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }
    fn release_temporary(&mut self, amount:usize) {
        self.state-=amount;
        self.temporary_state-=amount;
    }
    fn release_temporary_since(&mut self, before: usize) {
        let released = self.temporary_state - before;
        self.state -= released;
        self.temporary_state = before;
    }
    fn decoded(&mut self, raw:&[u8])->Result<Value,ItemRefusal> {
        let available=self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?;
        let (value,state)=crate::record_biblio_cut::bounded_decoded_state(raw,limits(),available,self.limits.deadline,self.cancelled)?;
        self.temporary(state)?;
        Ok(value)
    }
    fn ordered_value(&mut self, raw:&[u8])->Result<JsonValue,ItemRefusal> {
        let available=self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?;
        let value=crate::record_biblio_cut::bounded_ordered(raw,limits(),available,self.limits.deadline,self.cancelled)?;
        self.temporary(crate::record_biblio_cut::ordered_state(&value)?)?;
        Ok(value)
    }
    // One actual canonicalization supplies the consumed digest and byte length.
    // Counting serialization admits its buffer; no canonical result is built
    // solely to estimate another execution of the same codec pipeline.
    fn canonical_buffer(&self,value:&Value)->Result<Vec<u8>,ItemRefusal> {
        let available=self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?;
        crate::record_biblio_cut::decoded_wire_size(value,available.saturating_sub(std::mem::size_of::<Vec<u8>>()))?;
        let raw=serde_json::to_vec(value).map_err(|_|bad("serialization"))?;
        let raw_state=raw.len().checked_add(std::mem::size_of::<Vec<u8>>()).ok_or(ItemRefusal::Budget)?;
        let remaining=available.checked_sub(raw_state).ok_or(ItemRefusal::BudgetCheck{check:"compound canonical input workspace",used:Some(raw_state as u64),limit:Some(available as u64)})?;
        let tree=crate::record_biblio_cut::bounded_ordered(&raw,limits(),remaining,self.limits.deadline,self.cancelled)?;
        let parse_peak=raw_state.checked_add(crate::record_biblio_cut::ordered_codec_state(&tree)?).ok_or(ItemRefusal::Budget)?;
        let emit_base=raw_state.checked_add(crate::record_biblio_cut::ordered_state(&tree)?).and_then(|n|n.checked_add(crate::record_biblio_cut::ordered_emit_state(&tree).ok()?)).and_then(|n|n.checked_add(std::mem::size_of::<Vec<u8>>())).ok_or(ItemRefusal::Budget)?;
        let room=available.checked_sub(emit_base).ok_or(ItemRefusal::BudgetCheck{check:"compound canonical emit indexes",used:Some(emit_base as u64),limit:Some(available as u64)})?;
        let mut emission=limits();emission.max_bytes=emission.max_bytes.min(room);
        let output=canonical_bytes_v1(&tree,CanonicalProfile::SourceCommandInputV1,emission).map_err(|error|if error.code==tos_foundation::FoundationErrorCode::BudgetExceeded {ItemRefusal::BudgetCheck{check:"compound canonical output workspace",used:None,limit:Some(room as u64)}}else{ItemRefusal::Unsupported(format!("compound canonical: {error:?}"))})?;
        let peak=parse_peak.max(emit_base.checked_add(output.len()).ok_or(ItemRefusal::Budget)?);
        if peak>available {return Err(ItemRefusal::BudgetCheck{check:"compound canonical codec workspace",used:Some(peak as u64),limit:Some(available as u64)});}
        check(self.limits.deadline,self.cancelled)?;
        Ok(output)
    }
    fn canonical_observation(&self,value:&Value)->Result<(String,usize),ItemRefusal> {
        let output=self.canonical_buffer(value)?;
        Ok((Digest256::of_bytes(&output).to_prefixed(),output.len()))
    }
    fn ordered_buffer(&mut self,value:&JsonValue)->Result<Vec<u8>,ItemRefusal> {
        let available=self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?;
        let base=std::mem::size_of::<Vec<u8>>()+crate::record_biblio_cut::ordered_emit_state(value)?;
        let room=available.checked_sub(base).ok_or(ItemRefusal::Budget)?;
        let mut emit=limits();emit.max_bytes=emit.max_bytes.min(room);
        let bytes=canonical_bytes_v1(value,CanonicalProfile::SourceCommandInputV1,emit).map_err(|error|if error.code==tos_foundation::FoundationErrorCode::BudgetExceeded {ItemRefusal::BudgetCheck{check:"membership ordered canonical buffer",used:None,limit:Some(room as u64)}}else{bad("membership ordered canonical buffer")})?;
        self.buffer(bytes)
    }
    fn advance_claim(&mut self,previous:&Value,request:&Value)->Result<Value,ItemRefusal> {
        // One previous clone and the exact incoming patch can coexist during
        // replacement. Removed subtrees are released after the real mutation.
        let admission=crate::record_biblio_cut::decoded_state(previous)?.checked_add(crate::record_biblio_cut::decoded_state(&request["fields"])?).ok_or(ItemRefusal::Budget)?;
        self.temporary(admission)?;
        let result=advance_membership_claim(previous,request)?;
        let retained=crate::record_biblio_cut::decoded_state(&result)?;
        if retained>admission {return Err(bad("membership correction shape accounting"));}
        self.release_temporary(admission-retained);Ok(result)
    }
    fn package_revision(&mut self,files:&Package)->Result<String,ItemRefusal> {
        let refs=file_refs(files);let tree=crate::record_biblio_cut::decoded_state(&refs)?;
        self.temporary(tree)?;
        let result=self.canonical_observation(&refs).map(|(digest,_)|digest);
        drop(refs);self.release_temporary(tree);result
    }
    fn reference_matches(&mut self,value:&Value,id:&str,version:&str,expected:&Value)->Result<bool,ItemRefusal> {
        let n=integer(value,version)?;if n==0 {return Err(bad("positive record version"));}
        let (digest,_)=self.canonical_observation(value)?;
        let reference=json!({"id":text(value,id)?,"version":n,"digest":digest});
        let tree=crate::record_biblio_cut::decoded_state(&reference)?;
        self.temporary(tree)?;let result=&reference==expected;drop(reference);self.release_temporary(tree);Ok(result)
    }
    fn value_copy(&mut self,value:&Value)->Result<Value,ItemRefusal> {
        self.temporary(crate::record_biblio_cut::decoded_state(value)?)?;
        Ok(value.clone())
    }
    fn ordered_copy(&mut self,value:&JsonValue)->Result<JsonValue,ItemRefusal> {
        self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;
        Ok(value.clone())
    }
    fn buffer(&mut self,raw:Vec<u8>)->Result<Vec<u8>,ItemRefusal> {
        self.temporary(std::mem::size_of::<Vec<u8>>().checked_add(raw.len()).ok_or(ItemRefusal::Budget)?)?;
        Ok(raw)
    }
    fn optional(&mut self, path: &str, cap: usize) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        if let Some(raw) = self.raw.get(path) {
            if raw.len() > cap {
                return Err(ItemRefusal::BudgetCheck {check:"compound cached member bytes",used:Some(raw.len() as u64),limit:Some(cap as u64)});
            }
            let size=raw.len()+std::mem::size_of::<Vec<u8>>();
            self.temporary(size)?;
            let copy = self.raw.get(path).expect("selected cache entry remains").clone();
            return Ok(Some(copy));
        }
        if !self.paths.contains(path) {
            return Ok(None);
        }
        let raw = current(self.cut, path, self.limits, self.cancelled, &mut self.bytes)?;
        if raw.len() > cap {
            return Err(ItemRefusal::BudgetCheck {check:"compound selected member bytes",used:Some(raw.len() as u64),limit:Some(cap as u64)});
        }
        // One retained cache buffer, one owned map key and one entry slot.
        // The returned buffer is a distinct scoped temporary, never a third copy.
        let raw_state = raw.len().checked_add(path.len())
            .and_then(|n|n.checked_add(std::mem::size_of::<(String,Vec<u8>)>())).ok_or(ItemRefusal::Budget)?;
        let cache_state = self.raw_cache_state.checked_add(raw_state).ok_or(ItemRefusal::Budget)?;
        reserve(&mut self.state, raw_state, self.limits.max_state_bytes)?;
        self.raw_cache_state = cache_state;
        self.temporary(raw.len()+std::mem::size_of::<Vec<u8>>())?;
        self.record_read(PredicateRead::ExactPath {path:path.into(),digest:Digest256::of_bytes(&raw).to_prefixed()})?;
        self.raw.insert(path.into(), raw.clone());
        Ok(Some(raw))
    }
    fn release_raw_cache(&mut self) {
        // Called only after verify_inner has returned: its borrowed/returned
        // raw buffers are gone. Transaction file copies and decoded histories
        // have independent charges; the directory and read observations remain.
        self.raw = BTreeMap::new();
        self.state -= self.raw_cache_state;
        self.raw_cache_state = 0;
    }
    fn required(&mut self, path: &str, cap: usize) -> Result<Vec<u8>, ItemRefusal> {
        self.optional(path, cap)?
            .ok_or_else(|| bad(&format!("missing selected member {path}")))
    }
    fn record_read(&mut self, read: PredicateRead) -> Result<(), ItemRefusal> {
        reserve(
            &mut self.state,
            crate::record_biblio_cut::predicate_state(&read)?,
            self.limits.max_state_bytes,
        )?;
        self.reads.push(read);
        Ok(())
    }
    fn selected(&mut self, path: &str) -> Result<Package, ItemRefusal> {
        metadata_path(path, false)?;
        let home = parent(path)?;
        let mut files = Package::new();
        let mut total = 0;
        for name in selected_names(path)? {
            if let Some(raw) = self.optional(&format!("{home}/{name}"), MAX_FILE)? {
                total += raw.len();
                if total > MAX_SIDE {
                    return Err(ItemRefusal::BudgetCheck {check:"compound selected package bytes",used:Some(total as u64),limit:Some(MAX_SIDE as u64)});
                }
                files.insert(name, raw);
            }
        }
        if !files.contains_key(path.rsplit('/').next().unwrap_or("")) {
            return Err(bad("selected record absent"));
        }
        Ok(files)
    }
    fn transaction(&mut self, id: &str) -> Result<std::sync::Arc<Transaction>, ItemRefusal> {
        let before=self.temporary_state;
        let result=self.transaction_inner(id);
        self.release_temporary_since(before);
        if let Ok(tx)=&result {if !self.transactions.contains_key(id) {
            let mut retained=std::mem::size_of::<Transaction>()+std::mem::size_of::<(String,std::sync::Arc<Transaction>)>()+2*std::mem::size_of::<usize>()+id.len()+tx.manifest_sha256.len()+tx.status.len();
            retained=retained.checked_add(crate::record_biblio_cut::decoded_state(&tx.manifest)?.checked_sub(std::mem::size_of::<Value>()).ok_or(ItemRefusal::Budget)?).ok_or(ItemRefusal::Budget)?;
            for (path,(before,after)) in &tx.files {retained=retained.checked_add(std::mem::size_of::<(String,(Option<Vec<u8>>,Option<Vec<u8>>))>()+path.len()).and_then(|n|n.checked_add(before.as_ref().map_or(0,Vec::len))).and_then(|n|n.checked_add(after.as_ref().map_or(0,Vec::len))).ok_or(ItemRefusal::Budget)?;}
            reserve(&mut self.state,retained,self.limits.max_state_bytes)?;
            self.transactions.insert(id.into(),tx.clone());
        }}
        result
    }
    fn transaction_inner(&mut self, id: &str) -> Result<std::sync::Arc<Transaction>, ItemRefusal> {
        hash(id)?;
        if let Some(tx) = self.transactions.get(id) {
            return Ok(tx.clone());
        }
        let directory = format!("{TRANSACTIONS}/{}", hash(id)?);
        let raw = self.required(&format!("{directory}/manifest.json"), MAX_MANIFEST)?;
        let manifest=self.decoded(&raw)?;
        keys(
            &manifest,
            &[
                "schema_version",
                "transaction_id",
                "base_publication",
                "plan",
                "parents",
            ],
        )?;
        if text(&manifest, "transaction_id")? != id {
            return Err(bad("native bibliographic transaction grammar"));
        }
        let base = &manifest["base_publication"];
        keys(base, &["token", "generation"])?;
        let generation = integer(base, "generation")?;
        if generation > MAX_GENERATION - 2 || base["token"].is_null() != (generation == 0) {
            return Err(bad("publication predecessor"));
        }
        if !base["token"].is_null() {
            hash(text(base, "token")?)?;
        }
        let plan = &manifest["plan"];
        let companion_home = if let Some(profile) = plan.get("path_profile") {
            keys(
                plan,
                &["authorization", "files", "new_directories", "path_profile"],
            )?;
            keys(profile, &["schema_version", "item_source_path"])?;
            let path = text(profile, "item_source_path")?;
            metadata_path(path, false)?;
            if text(profile, "schema_version")? != "tos_item_metadata_paths_v1"
                || !path.ends_with("/item.json")
                || !parent(path)?
                    .rsplit_once('/')
                    .is_some_and(|(p, _)| p.ends_with("/items"))
                || plan["authorization"]["schema_version"] != "tos_item_adoption_authorization_v1"
                || plan["authorization"]["scope"]["item_source_path"] != path
            {
                return Err(bad("exact Item transaction path profile"));
            }
            Some(parent(path)?.to_owned())
        } else {
            keys(plan, &["authorization", "files", "new_directories"])?;
            None
        };
        if text(&manifest, "schema_version")?
            != if companion_home.is_some() {
                "tos_selected_metadata_transaction_v2"
            } else {
                "tos_selected_metadata_transaction_v1"
            }
        {
            return Err(bad("transaction version/path profile mismatch"));
        }
        if !plan["authorization"].is_object() {return Err(bad("bounded authorization"));}
        let oversized=self.canonical_observation(&plan["authorization"])?.1>65536;
        if oversized {return Err(bad("bounded authorization"));}
        let directories = array(plan, "new_directories")?;
        if directories.len() > 64 {
            return Err(ItemRefusal::BudgetCheck {check:"compound new directory count",used:Some(directories.len() as u64),limit:Some(64)});
        }
        let dirs: Vec<_> = directories
            .iter()
            .map(|v| v.as_str().ok_or_else(|| bad("new directory")))
            .collect::<Result<_, _>>()?;
        // Three simultaneously live borrowed directory indexes: source Vec,
        // sorted Vec and uniqueness set; none owns another path payload.
        self.temporary(2*std::mem::size_of::<Vec<&str>>()+std::mem::size_of::<BTreeSet<&&str>>()+dirs.len()*(2*std::mem::size_of::<&str>()+std::mem::size_of::<&&str>()))?;
        let mut sorted_dirs = dirs.clone();
        sorted_dirs.sort_by_key(|v| (v.split('/').count(), *v));
        if dirs != sorted_dirs || dirs.iter().collect::<BTreeSet<_>>().len() != dirs.len() {
            return Err(bad("new directory order/uniqueness"));
        }
        for d in &dirs {
            metadata_path(d, true)?;
        }
        let rows = array(plan, "files")?;
        if !(1..=64).contains(&rows.len()) {
            return Err(ItemRefusal::BudgetCheck {check:"compound plan file count (minimum 1)",used:Some(rows.len() as u64),limit:Some(64)});
        }
        let mut files = BTreeMap::new();
        let mut blobs = BTreeMap::<String, Vec<u8>>::new();
        let mut sides = [0usize; 2];
        let mut total = 0usize;
        let mut changed = false;
        let mut last = "";
        for row in rows {
            check(self.limits.deadline, self.cancelled)?;
            keys(row, &["path", "before", "after"])?;
            let path = text(row, "path")?;
            if companion_home.as_ref().is_some_and(|home| {
                path == format!("{home}/fixity.sha256")
                    || path == format!("{home}/forensic-report.md")
            }) {
                // The home itself passed the exact public metadata path law.
            } else {
                metadata_path(path, false)?;
            }
            if path <= last {
                return Err(bad("file path order/uniqueness"));
            }
            last = path;
            if row["before"].is_null() && row["after"].is_null() {
                return Err(bad("absent to absent file"));
            }
            changed |= row["before"] != row["after"];
            let mut bytes = [None, None];
            for (i, side) in ["before", "after"].iter().enumerate() {
                let binding = &row[*side];
                if binding.is_null() {
                    continue;
                }
                keys(binding, &["sha256", "bytes"])?;
                let sha = text(binding, "sha256")?;
                hash(sha)?;
                let size =
                    usize::try_from(integer(binding, "bytes")?).map_err(|_| ItemRefusal::Budget)?;
                sides[i] = sides[i].checked_add(size).ok_or(ItemRefusal::Budget)?;
                if size > MAX_SIDE || sides[i] > MAX_SIDE {
                    return Err(ItemRefusal::BudgetCheck {check:"compound transaction side bytes",used:Some(sides[i].max(size) as u64),limit:Some(MAX_SIDE as u64)});
                }
                if !blobs.contains_key(sha) {
                    total = total.checked_add(size).ok_or(ItemRefusal::Budget)?;
                    if total > 2 * MAX_SIDE {
                        return Err(ItemRefusal::BudgetCheck {check:"compound unique transaction blob bytes",used:Some(total as u64),limit:Some((2*MAX_SIDE) as u64)});
                    }
                    let raw = self.required(&format!("{directory}/{}.blob", hash(sha)?), size)?;
                    if raw.len() != size || Digest256::of_bytes(&raw).to_prefixed() != sha {
                        return Err(bad("transaction blob fixity"));
                    }
                    self.temporary(std::mem::size_of::<(String,Vec<u8>)>()+sha.len())?;
                    blobs.insert(sha.into(), raw);
                }
                // One retained file buffer; the returned transaction shares it.
                self.temporary(size)?;
                bytes[i] = Some(blobs[sha].clone());
            }
            files.insert(path.into(), (bytes[0].take(), bytes[1].take()));
        }
        if !changed {
            return Err(bad("transaction has no change"));
        }
        for a in files.keys() {
            for b in files.keys().map(String::as_str).chain(dirs.iter().copied()) {
                if a.as_str() != b && is_ancestor(a, b) {
                    return Err(bad("target ancestor collision"));
                }
            }
        }
        for d in &dirs {
            if !files
                .iter()
                .any(|(p, (before, _))| is_ancestor(d, p) && before.is_none())
            {
                return Err(bad("new directory lacks new file"));
            }
        }
        let mut parents = BTreeSet::from([HOME.to_owned()]);
        self.temporary(std::mem::size_of::<String>()+HOME.len())?;
        for p in files.keys().map(String::as_str).chain(dirs.iter().copied()) {
            let mut p = parent(p)?;
            while p == HOME || p.starts_with(&format!("{HOME}/")) {
                if !parents.contains(p) {self.temporary(std::mem::size_of::<String>()+p.len())?;}
                parents.insert(p.into());
                if p == HOME {
                    break;
                }
                p = parent(p)?;
            }
        }
        let bindings = manifest["parents"]
            .as_object()
            .ok_or_else(|| bad("parent closure"))?;
        if bindings.keys().cloned().collect::<BTreeSet<_>>() != parents {
            return Err(bad("parent directory closure"));
        }
        for (p, binding) in bindings {
            if binding.is_null() {
                if !dirs.contains(&p.as_str()) {
                    return Err(bad("undeclared absent parent"));
                }
            } else {
                keys(binding, &["device", "inode", "mode", "uid"])?;
                for k in ["device", "inode", "mode", "uid"] {
                    integer(binding, k)?;
                }
                let mode = integer(binding, "mode")?;
                if mode & 0o170000 != 0o040000 || mode & 0o022 != 0 {
                    return Err(bad("historical directory posture"));
                }
            }
        }
        let sha = Digest256::of_bytes(&raw).to_prefixed();
        let completion = match self.optional(&format!("{directory}/completion.json"), 8192)? {
            Some(raw) => {
                let v = self.decoded(&raw)?;
                keys(&v, &["schema_version", "publication"])?;
                if text(&v, "schema_version")? != "tos_selected_metadata_completion_v1" {
                    return Err(bad("completion schema"));
                }
                let scratch=crate::record_biblio_cut::decoded_state(&v["publication"])?;
                self.temporary(scratch)?;state_valid_with(&v["publication"],&mut |value|self.canonical_observation(value))?;self.release_temporary(scratch);
                let s = &v["publication"];
                if text(s, "phase")? != "ready"
                    || text(s, "transaction_id")? != id
                    || text(s, "manifest_sha256")? != sha
                    || integer(s, "generation")? != generation + 2
                {
                    return Err(bad("terminal completion binding"));
                }
                Some(self.value_copy(s)?)
            }
            None => None,
        };
        let selected = self
            .publication
            .as_ref()
            .is_some_and(|s| s["transaction_id"] == id);
        let status = if selected {
            let s = self.publication.as_ref().unwrap();
            if text(s, "manifest_sha256")? != sha {
                return Err(bad("current terminal transaction drift"));
            }
            if text(s, "phase")? == "pending" {
                if integer(s, "generation")? != generation + 1 || completion.is_some() {
                    return Err(bad("exact pending transaction binding"));
                }
                "pending".into()
            } else {
                if integer(s, "generation")? != generation + 2
                    || completion.as_ref().is_some_and(|c| c != s)
                {
                    return Err(bad("terminal generation/completion drift"));
                }
                text(s, "outcome")?.into()
            }
        } else {
            completion
                .as_ref()
                .map(|s| text(s, "outcome").map(str::to_owned))
                .transpose()?
                .unwrap_or("orphan".into())
        };
        self.temporary(
            files
                .keys()
                .try_fold(0usize, |sum, path| {
                    sum.checked_add(path.len())?.checked_add(
                        std::mem::size_of::<(String, (Option<Vec<u8>>, Option<Vec<u8>>))>(),
                    )
                })
                .ok_or(ItemRefusal::Budget)?,
        )?;
        self.temporary(id.len()+std::mem::size_of::<(String,std::sync::Arc<Transaction>)>()+2*std::mem::size_of::<usize>()+std::mem::size_of::<Transaction>()+sha.len()+status.len())?;
        let tx = std::sync::Arc::new(Transaction {manifest,manifest_sha256:sha,status,files});
        Ok(tx)
    }
    fn archive(&mut self, path: &str, id: &str, receipt: &Value) -> Result<Package, ItemRefusal> {
        let before = self.temporary_state;
        let result = self.archive_inner(path, id, receipt);
        self.release_temporary_since(before);
        if let Ok(files)=&result {self.temporary(package_state(files)?)?;}
        result
    }
    fn archive_inner(
        &mut self,
        path: &str,
        id: &str,
        receipt: &Value,
    ) -> Result<Package, ItemRefusal> {
        self.archive_files_inner(path,id,receipt,true)
    }
    fn archive_files_inner(&mut self,path:&str,id:&str,receipt:&Value,record_semantics:bool)->Result<Package,ItemRefusal> {
        let rev = text(receipt, "previous_revision")?;
        let home = format!(
            "{HOME}/.record-revisions/{}-{}",
            Digest256::of_bytes(id.as_bytes()).to_hex(),
            hash(rev)?
        );
        if text(receipt, "archive_path")? != home {
            return Err(bad("archive exact locator"));
        }
        let raw = self.required(&format!("{home}/manifest.json"), MAX_FILE)?;
        let manifest = self.decoded(&raw)?;
        let v2 = text(&manifest, "schema_version")? == "tos_source_package_archive_v2";
        let mut wanted = vec![
            "schema_version",
            "source_path",
            "source",
            "revision",
            "files",
        ];
        // V1 Claim corrections bind a flat archive without a record basename;
        // selected V2 metadata archives retain their three-file scope.
        if v2 {
            wanted.push("publication_protocol");
        }
        keys(&manifest, &wanted)?;
        if !matches!(
            text(&manifest, "schema_version")?,
            "tos_source_package_archive_v1" | "tos_source_package_archive_v2"
        ) || v2 && text(&manifest, "publication_protocol")? != PROTOCOL
            || text(&manifest, "source_path")? != path
            || manifest["source"] != receipt["previous_source"]
            || manifest["revision"] != receipt["previous_revision"]
        {
            return Err(bad("archive metadata binding"));
        }
        let bindings = manifest["files"]
            .as_object()
            .ok_or_else(|| bad("archive files"))?;
        if bindings.len() > 64 {
            return Err(ItemRefusal::BudgetCheck {check:"compound archived package file count",used:Some(bindings.len() as u64),limit:Some(64)});
        }
        let mut files = Package::new();
        let mut expected = BTreeSet::from(["manifest.json".to_owned()]);
        let mut total = raw.len();
        for (name, b) in bindings {
            check(self.limits.deadline, self.cancelled)?;
            keys(b, &["blob", "sha256", "bytes"])?;
            if name.is_empty() || name.contains('/') || matches!(name.as_str(), "." | "..") {
                return Err(bad("flat archived filename"));
            }
            let sha = text(b, "sha256")?;
            let blob = format!("{}.blob", hash(sha)?);
            if text(b, "blob")? != blob {
                return Err(bad("archive blob name"));
            }
            let size = usize::try_from(integer(b, "bytes")?).map_err(|_| ItemRefusal::Budget)?;
            let raw = self.required(&format!("{home}/{blob}"), size.min(MAX_FILE))?;
            total = total.checked_add(raw.len()).ok_or(ItemRefusal::Budget)?;
            if total > MAX_SIDE + MAX_FILE {
                return Err(ItemRefusal::BudgetCheck {check:"compound archived package plus manifest bytes",used:Some(total as u64),limit:Some((MAX_SIDE+MAX_FILE) as u64)});
            }
            if raw.len() != size || Digest256::of_bytes(&raw).to_prefixed() != sha {
                return Err(bad("archive exact blob bytes"));
            }
            self.temporary(std::mem::size_of::<(String,Vec<u8>)>()+name.len()+std::mem::size_of::<String>()+blob.len())?;
            files.insert(name.clone(), raw);
            expected.insert(blob);
        }
        let prefix = format!("{home}/");
        let actual: BTreeSet<_> = self
            .paths
            .range::<str,_>((std::ops::Bound::Included(prefix.as_str()),std::ops::Bound::Unbounded))
            .take_while(|p| p.starts_with(&prefix))
            .map(|p| p[prefix.len()..].to_owned())
            .collect();
        self.temporary(actual.iter().try_fold(0usize,|n,p:&String|n.checked_add(std::mem::size_of::<String>()+p.len())).ok_or(ItemRefusal::Budget)?)?;
        if actual != expected {
            return Err(bad("archive extra/nested/unbound files"));
        }
        if v2 {
            let names = selected_names(path)?;
            if files.keys().any(|n| !names.contains(n)) || !files.contains_key(&names[0]) {
                return Err(bad("selected archive package scope"));
            }
        }
        if self.package_revision(&files)? != rev {
            return Err(bad("archive package revision"));
        }
        if !record_semantics {return Ok(files);}
        let names = selected_names(path)?;
        let old = self.decoded(files.get(&names[0]).ok_or_else(||bad("archive source missing"))?)?;
        if !self.reference_matches(&old,"record_id","record_version",&receipt["previous_source"])? {
            return Err(bad("archive previous source"));
        }
        if let Some(request) = receipt.get("request") {
            let mut revised = self.value_copy(&old)?;
            let object = revised
                .as_object_mut()
                .ok_or_else(|| bad("source object"))?;
            for (k, v) in request["fields"]
                .as_object()
                .ok_or_else(|| bad("revision fields"))?
            {
                object.insert(k.clone(), v.clone());
            }
            object.insert(
                "record_version".into(),
                json!(
                    integer(&old, "record_version")?
                        .checked_add(1)
                        .ok_or(ItemRefusal::Budget)?
                ),
            );
            self.temporary(crate::record_biblio_cut::decoded_state(&revised)?.saturating_sub(crate::record_biblio_cut::decoded_state(&old)?))?;
            if !self.reference_matches(&revised,"record_id","record_version",&receipt["source"])? {
                return Err(bad("retained request successor"));
            }
        }
        Ok(files)
    }
    fn flat_package(&mut self,home:&str)->Result<Package,ItemRefusal> {
        let prefix=format!("{home}/");
        self.temporary(std::mem::size_of::<String>()+prefix.len())?;
        let count=self.paths.range::<str,_>((std::ops::Bound::Included(prefix.as_str()),std::ops::Bound::Unbounded)).take_while(|p|p.starts_with(&prefix)).count();
        if !(1..=64).contains(&count) {return Err(bad("membership flat Claim package count"));}
        self.temporary(std::mem::size_of::<Vec<&str>>()+count*std::mem::size_of::<&str>())?;
        let names:Vec<&str>=self.paths.range::<str,_>((std::ops::Bound::Included(prefix.as_str()),std::ops::Bound::Unbounded)).take_while(|p|p.starts_with(&prefix)).map(|p|{let path=*p;&path[prefix.len()..]}).collect();
        if !(1..=64).contains(&names.len())||names.iter().any(|name|name.contains('/')) {return Err(bad("membership flat Claim package"));}
        let mut result=Package::new();self.temporary(std::mem::size_of::<Package>())?;let mut total=0usize;
        for name in names {let raw=self.required(&format!("{home}/{name}"),MAX_FILE)?;total=total.checked_add(raw.len()).ok_or(ItemRefusal::Budget)?;if total>MAX_SIDE {return Err(ItemRefusal::BudgetCheck{check:"membership current Claim package bytes",used:Some(total as u64),limit:Some(MAX_SIDE as u64)});}self.temporary(std::mem::size_of::<(String,Vec<u8>)>()+name.len())?;result.insert(name.into(),raw);}
        Ok(result)
    }
    fn single_membership_claim(&mut self,raw:&[u8],id:&str)->Result<Value,ItemRefusal> {
        if raw.len()>1_048_576 {return Err(ItemRefusal::BudgetCheck{check:"membership correction stream bytes",used:Some(raw.len() as u64),limit:Some(1_048_576)});}
        let mut result=None;
        for (line,_) in claim_lines(raw) {check(self.limits.deadline,self.cancelled)?;if line.iter().all(u8::is_ascii_whitespace) {continue;}if result.is_some() {return Err(bad("membership current stream has another Claim"));}let value=self.decoded(line)?;if text(&value,"claim_id")?!=id {return Err(bad("membership current stream Claim identity"));}result=Some(value);}
        result.ok_or_else(||bad("membership Claim stream empty"))
    }
    fn attachment_claim_initial(&mut self,path:&str,claim:&Value)->Result<Vec<u8>,ItemRefusal> {
        use crate::source_forms::source_copy_kernel as kernel;
        let files=self.flat_package(parent(path)?)?;let id=text(claim,"claim_id")?;
        let raw=files.get("source-claims.jsonl").ok_or_else(||bad("membership current stream missing"))?;
        let current=self.single_membership_claim(raw,id)?;
        if &current!=claim {return Err(bad("membership current stream differs from observed Claim"));}
        let history=match files.get("claim-revision-history.json") {Some(raw)=>self.decoded(raw)?,None=>{let value=json!({"schema_version":"tos_claim_revision_history_v1","source_path":path,"receipts":[]});self.temporary(crate::record_biblio_cut::decoded_state(&value)?)?;value}};
        keys(&history,&["schema_version","source_path","receipts"])?;
        let receipts=array(&history,"receipts")?;
        if history["schema_version"]!="tos_claim_revision_history_v1"||history["source_path"]!=path||receipts.len()>MAX_HISTORY {return Err(bad("membership correction history grammar"));}
        if receipts.is_empty() {if current["claim_version"]!=1 {return Err(bad("membership noninitial Claim lacks history"));}self.temporary(std::mem::size_of::<Vec<u8>>()+raw.len())?;return Ok(raw.clone());}
        let formname=format!("source-claims.{}.human-forms.json",Digest256::of_bytes(id.as_bytes()).to_hex());
        self.temporary(std::mem::size_of::<String>()+formname.len())?;
        let current_forms=self.ordered_value(files.get(&formname).ok_or_else(||bad("membership current correction forms absent"))?)?;
        let subject=current_forms.object_get("subject").ok_or_else(||bad("membership correction form subject"))?;
        let form_rows=current_forms.object_get("forms").and_then(JsonValue::as_array).ok_or_else(||bad("membership current forms"))?;
        let prior_rows=current_forms.object_get("prior_forms").and_then(JsonValue::as_array).ok_or_else(||bad("membership prior forms"))?;
        let history_scratch=(form_rows.len()+prior_rows.len())*std::mem::size_of::<((&str,u64),&JsonValue)>()+form_rows.len()*std::mem::size_of::<&str>();
        self.temporary(history_scratch)?;
        kernel::validate_history(&current_forms,subject).map_err(|_|bad("membership correction form history"))?;
        self.release_temporary(history_scratch);
        let mut commands=BTreeSet::new();self.temporary(std::mem::size_of::<BTreeSet<&str>>()+receipts.len()*std::mem::size_of::<&str>())?;
        let mut expected:Option<Vec<u8>>=None;let mut initial=None;
        for receipt in receipts {
            check(self.limits.deadline,self.cancelled)?;
            keys(receipt,&["command_id","request_digest","principal_id","authority_ref","owner_configuration","recorded_at","reason","previous_source","source","previous_revision","archive_path","dependencies","source_bindings","changed_fields","forms","grants_admission","request"])?;
            let request=&receipt["request"];
            crate::retirement_rules::observed_instant_order(text(receipt,"recorded_at")?,text(receipt,"recorded_at")?).map_err(|_|bad("membership correction aware instant"))?;
            if !commands.insert(text(receipt,"command_id")?)||request["operation"]!="claim.revise"||text(receipt,"request_digest")?!=self.canonical_observation(request)?.0||receipt["grants_admission"]!=false||receipt["source"]["id"]!=receipt["previous_source"]["id"]||integer(&receipt["source"],"version")?!=integer(&receipt["previous_source"],"version")?.checked_add(1).ok_or(ItemRefusal::Budget)? {return Err(bad("membership correction receipt binding"));}
            for (left,right) in [("command_id","command_id"),("previous_source","expected_source"),("previous_revision","expected_revision"),("owner_configuration","expected_configuration"),("dependencies","expected_dependencies"),("source_bindings","expected_inputs"),("reason","reason")] {if receipt[left]!=request[right] {return Err(bad("membership correction request binding"));}}
            let fields=request["fields"].as_object().ok_or_else(||bad("membership correction fields"))?;
            let changed=array(receipt,"changed_fields")?;
            if changed.len()!=fields.len()||!changed.iter().zip(fields.keys()).all(|(value,key)|value.as_str()==Some(key.as_str())) {return Err(bad("membership correction changed fields"));}
            let previous_survivors=initial.as_ref().map_or(0,|v:&Vec<u8>|std::mem::size_of::<Vec<u8>>()+v.len())+expected.as_ref().map_or(0,|v|std::mem::size_of::<Vec<u8>>()+v.len());
            let phase=self.temporary_state.checked_sub(previous_survivors).ok_or(ItemRefusal::Budget)?;
            let archived=self.archive_files_inner(path,text(&receipt["previous_source"],"id")?,receipt,false)?;
            let before=archived.get("source-claims.jsonl").ok_or_else(||bad("membership correction archived stream"))?;
            let previous=self.single_membership_claim(before,id)?;
            if initial.is_none() {if previous["claim_version"]!=1 {return Err(bad("membership correction lacks initial stream"));}self.temporary(std::mem::size_of::<Vec<u8>>()+before.len())?;initial=Some(before.clone());}
            if expected.as_ref().is_some_and(|raw|raw!=before)||!self.reference_matches(&previous,"claim_id","claim_version",&receipt["previous_source"])? {return Err(bad("membership correction predecessor bytes"));}
            let revised=self.advance_claim(&previous,request)?;
            if !self.reference_matches(&revised,"claim_id","claim_version",&receipt["source"])? {return Err(bad("membership correction successor reference"));}
            let selections=array(request,"forms")?;
            if !(1..=32).contains(&selections.len())||!selections.iter().any(|s|s["field_id"]=="claim.statement") {return Err(bad("membership correction statement form"));}
            let mut seen=BTreeSet::new();self.temporary(std::mem::size_of::<BTreeSet<&str>>()+selections.len()*std::mem::size_of::<&str>())?;
            for selection in selections {keys(selection,&["form_id","field_id"])?;if !seen.insert(text(selection,"form_id")?) {return Err(bad("membership correction duplicate form selection"));}}
            let prior=archived.get(&formname).map(|raw|self.ordered_value(raw)).transpose()?;
            let revised_raw=self.buffer(self.canonical_buffer(&revised)?)?;
            let revised_ordered=self.ordered_value(&revised_raw)?;
            let (result,result_refs)=forms(&revised_ordered,prior.as_ref(),&request["forms"],text(receipt,"principal_id")?,true,self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?)?;
            self.temporary(crate::record_biblio_cut::ordered_state(&result)?.checked_add(crate::record_biblio_cut::ordered_state(&result_refs)?).ok_or(ItemRefusal::Budget)?)?;
            let result_refs_raw=self.ordered_buffer(&result_refs)?;let expected_refs_raw=self.buffer(self.canonical_buffer(&receipt["forms"])?)?;
            if result_refs_raw!=expected_refs_raw {return Err(bad("membership correction archived form results"));}
            let reference_buffers=std::mem::size_of::<Vec<u8>>()*2+result_refs_raw.len()+expected_refs_raw.len();drop(result_refs_raw);drop(expected_refs_raw);self.release_temporary(reference_buffers);
            for reference in array(receipt,"forms")? {
                let mut retained=false;
                for form in form_rows.iter().chain(prior_rows) {
                    let before_form=self.temporary_state;let raw=self.ordered_buffer(form)?;let form=self.decoded(&raw)?;
                    let matches=self.reference_matches(&form,"form_id","form_version",reference)?;
                    drop(form);drop(raw);self.release_temporary_since(before_form);if matches {retained=true;break;}
                }
                if !retained {return Err(bad("membership correction result forms no longer retained"));}
            }
            let output_size=claim_lines(before).try_fold(0usize,|n,(line,ending)|n.checked_add(if line.iter().all(u8::is_ascii_whitespace){line.len()+ending.len()}else{revised_raw.len()+if ending==b"\r\n" {2}else if ending==b"\n" {1}else{0}})).ok_or(ItemRefusal::Budget)?;
            self.temporary(std::mem::size_of::<Vec<u8>>()+output_size)?;let mut output=Vec::with_capacity(output_size);
            for (line,ending) in claim_lines(before) {if line.iter().all(u8::is_ascii_whitespace) {output.extend_from_slice(line);output.extend_from_slice(ending);}else{output.extend_from_slice(&revised_raw);if ending==b"\r\n"||ending==b"\n" {output.extend_from_slice(ending);}}}
            if let Some(old)=expected.replace(output) {let cost=std::mem::size_of::<Vec<u8>>()+old.len();drop(old);self.release_temporary(cost);}
            // initial+expected survive the archive/reconstruction temporary phase.
            let survivors=initial.as_ref().map_or(0,|v|std::mem::size_of::<Vec<u8>>()+v.len())+expected.as_ref().map_or(0,|v|std::mem::size_of::<Vec<u8>>()+v.len());
            drop(archived);drop(previous);drop(revised);drop(prior);drop(revised_ordered);drop(result);drop(result_refs);drop(revised_raw);self.release_temporary_since(phase);self.temporary(survivors)?;
        }
        if expected.as_ref()!=Some(raw) {return Err(bad("membership current stream differs from correction head"));}
        initial.ok_or_else(||bad("membership initial stream absent"))
    }
    // The two maintained attachment owners resolve their existing endpoint
    // through current/continuously archived lineage; neither creates it.
    fn attachment_endpoint_binding(&mut self,scope:&Value,work:&Value,dependencies:&Value,kind:CompoundKind)->Result<JsonValue,ItemRefusal> {
        let start=self.temporary_state;
        let result=self.attachment_endpoint_binding_inner(scope,work,dependencies,kind);
        self.release_temporary_since(start);
        if let Ok(value)=&result {self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;}
        result
    }
    fn attachment_endpoint_binding_inner(&mut self,scope:&Value,work:&Value,dependencies:&Value,kind:CompoundKind)->Result<JsonValue,ItemRefusal> {
        let path=text(scope,kind.child_path())?;let id=text(scope,kind.child_key())?;
        let sha=text(&dependencies["catalog_and_sources"],path)?;
        let expected=Digest256::from_hex(sha).map_err(|_|bad("membership exact Work dependency raw hash"))?;
        let files=self.selected(path)?;let current=self.decoded(&files[kind.child_file()])?;
        if text(&current,"record_id")?!=id||text(&current,"record_type")?!=kind.child_kind() {return Err(bad("membership Work current typed identity"));}
        let history=self.history(path,&files)?;
        self.temporary(std::mem::size_of::<String>()+71)?;
        let work_digest=self.canonical_observation(work)?.0;
        let mut matched=None;
        if Digest256::of_bytes(&files[kind.child_file()])==expected {
            if self.canonical_observation(&current)?.0!=work_digest {return Err(bad("membership Work current payload binding"));}
            matched=Some(files[kind.child_file()].len());
        }
        for receipt in array(&history,"receipts")? {
            check(self.limits.deadline,self.cancelled)?;
            let request=&receipt["request"];
            if kind==CompoundKind::CollectionWork && text(request,"operation")?=="work.expression.create" {parent_receipt_shape_with(receipt,CompoundKind::WorkExpression,&mut |value|self.canonical_observation(value))?;}
            else if request["schema_version"]!="tos_local_source_command_v1"||request["operation"]!="record.revise"||request["fields"].as_object().is_none_or(|fields|fields.is_empty()||fields.keys().any(|key|!["preferred_label","notes","field_languages","source_refs"].contains(&key.as_str()))) {return Err(bad("membership Work undeclared metadata transition"));}
            let phase=self.temporary_state;
            let archived=self.archive(path,id,receipt)?;
            if let Some(publication)=receipt.get("publication") {
                let tx=self.transaction(text(publication,"transaction_id")?)?;
                let (before,after)=tx.files.get(path).ok_or_else(||bad("membership Work selected transition absent"))?;
                if tx.status!="committed"||before.as_ref()!=archived.get(kind.child_file()) {return Err(bad("membership Work committed selected before binding"));}
                let after=self.decoded(after.as_ref().ok_or_else(||bad("membership Work selected successor absent"))?)?;
                if !self.reference_matches(&after,"record_id","record_version",&receipt["source"])? {return Err(bad("membership Work committed selected successor binding"));}
            }
            let raw=&archived[kind.child_file()];
            if Digest256::of_bytes(raw)==expected {
                let previous=self.decoded(raw)?;
                if self.canonical_observation(&previous)?.0!=work_digest {return Err(bad("membership retained Work payload binding"));}
                matched=Some(raw.len());
            }
            drop(archived);self.release_temporary_since(phase);
        }
        let size=matched.ok_or_else(||bad("membership Work lacks current continuous raw binding"))?;
        // The consumed reference reuses the already compared canonical digest.
        // Admit its two actual ordered object containers and strings before
        // constructing them; the helper input Vecs coexist during collection.
        let version=integer(work,"record_version")?;
        let version_digits=work["record_version"].as_number().ok_or_else(||bad("membership Work version number"))?.as_str().len();let size_digits=if size==0{1}else{size.ilog10() as usize+1};
        let string_state=|s:&str|s.len()+s.encode_utf16().count()*std::mem::size_of::<u16>();
        let keys=["source_path","source","source_sha256","source_bytes","id","version","digest"];
        let retained=std::mem::size_of::<JsonValue>()+keys.len()*std::mem::size_of::<(tos_foundation::JsonString,JsonValue)>()+keys.iter().map(|key|string_state(key)).sum::<usize>()+string_state(path)+string_state(id)+2*string_state(&work_digest)+version_digits+size_digits;
        let scratch=2*std::mem::size_of::<Vec<(&str,JsonValue)>>()+keys.len()*std::mem::size_of::<(&str,JsonValue)>()+std::mem::size_of::<String>()+71+2*std::mem::size_of::<Value>()+version_digits+size_digits;
        self.temporary(retained.checked_add(scratch).ok_or(ItemRefusal::Budget)?)?;
        let result=object(vec![("source_path",string(path)),("source",object(vec![("id",string(id)),("version",j(&json!(version))?),("digest",string(&work_digest))])),("source_sha256",string(&expected.to_prefixed())),("source_bytes",j(&json!(size))?)]);
        self.release_temporary(scratch);Ok(result)
    }
    pub(crate) fn finish(self) -> (u64, Vec<PredicateRead>) {
        (self.bytes, self.reads)
    }
}
fn state_valid_with(v: &Value,observe:&mut impl FnMut(&Value)->Result<(String,usize),ItemRefusal>) -> Result<(), ItemRefusal> {
    keys(
        v,
        &[
            "schema_version",
            "generation",
            "transition_id",
            "phase",
            "transaction_id",
            "manifest_sha256",
            "outcome",
            "recovery_authorization",
            "token",
        ],
    )?;
    let generation = integer(v, "generation")?;
    let transition = text(v, "transition_id")?;
    let phase = text(v, "phase")?;
    if text(v, "schema_version")? != "tos_source_metadata_publication_v1"
        || !(1..=MAX_GENERATION).contains(&generation)
        || transition.len() != 32
        || !transition
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || !matches!(phase, "pending" | "ready")
        || phase == "pending" && (!v["outcome"].is_null() || !v["recovery_authorization"].is_null())
        || phase == "ready" && !matches!(text(v, "outcome")?, "committed" | "rolled-back")
    {
        return Err(bad("publication state grammar"));
    }
    for k in ["transaction_id", "manifest_sha256", "token"] {
        hash(text(v, k)?)?;
    }
    if !v["recovery_authorization"].is_null()
        && (!v["recovery_authorization"].is_object()
            || observe(&v["recovery_authorization"])?.1 > 4096)
    {
        return Err(bad("recovery evidence bound"));
    }
    let mut contents = v.clone();
    contents.as_object_mut().unwrap().remove("token");
    if text(v, "token")? != observe(&contents)?.0 {
        return Err(bad("publication token digest"));
    }
    Ok(())
}

fn request_valid_with(request: &Value, kind: CompoundKind,observe:&mut impl FnMut(&Value)->Result<(String,usize),ItemRefusal>) -> Result<(String,usize), ItemRefusal> {
    let mut request_keys = vec![
        "schema_version",
        "operation",
        kind.record_key(),
        "claim",
        "forms",
        kind.child_form_request(),
        "claim_forms",
        "reason",
        "command_id",
        "fields",
        "expected_configuration",
        "expected_source",
        "expected_revision",
        "expected_dependencies",
        "expected_publication",
    ];
    if kind.relation_attachment() {request_keys.retain(|key|*key!=kind.child_form_request());request_keys.push("forms");}
    if kind == CompoundKind::EditionItem {
        request_keys.extend([
            "rights",
            "item_kind",
            "inventory",
            "inventory_limitation",
            "fixity_verified_at",
        ]);
        let now = text(request, "fixity_verified_at")?;
        crate::retirement_rules::observed_instant_order(now, now)
            .map_err(|_| bad("Item fixity instant"))?;
        if request["inventory"].is_null() != !request["inventory_limitation"].is_null() {
            return Err(bad("explicit inventory completeness/limitation"));
        }
    }
    keys(request, &request_keys)?;
    if text(request, "schema_version")? != kind.request_schema()
        || text(request, "operation")? != kind.operation()
    {
        return Err(bad("native bibliographic compound request grammar"));
    }
    let observed=observe(request)?;
    if observed.1>1_048_576 {return Err(bad("native bibliographic compound request grammar"));}
    let reason = tos_foundation::python_strip_unicode16_v1(text(request, "reason")?, MAX_SIDE)
        .map_err(|_| ItemRefusal::Budget)?;
    if reason.is_empty()
        || reason.chars().count() > 4096
        || !(1..=256).contains(&text(request, "command_id")?.chars().count())
    {
        return Err(bad("request reason/command bounds"));
    }
    for k in [
        "expected_configuration",
        "expected_revision",
        "expected_dependencies",
    ] {
        hash(text(request, k)?)?;
    }
    if !request["expected_publication"].is_null() {
        hash(text(request, "expected_publication")?)?;
    }
    keys(&request["fields"], &[kind.field()])?;
    Ok(observed)
}
fn transaction_id_with(request:&Value,kind:CompoundKind,observe:&mut impl FnMut(&Value)->Result<(String,usize),ItemRefusal>)->Result<String,ItemRefusal> {
    let request_digest=observe(request)?.0;
    transaction_id_with_digest(request,kind,&request_digest,observe)
}
fn transaction_id_with_digest(request:&Value,kind:CompoundKind,request_digest:&str,observe:&mut impl FnMut(&Value)->Result<(String,usize),ItemRefusal>)->Result<String,ItemRefusal> {
    observe(&json!({"operation":kind.operation(),"command_id":request["command_id"],"owner_configuration":request["expected_configuration"],"request_digest":request_digest})).map(|(digest,_)|digest)
}
fn canonical_observation(value:&Value)->Result<(String,usize),ItemRefusal> {let bytes=canonical(value)?;Ok((Digest256::of_bytes(&bytes).to_prefixed(),bytes.len()))}
fn transaction_id(request:&Value,kind:CompoundKind)->Result<String,ItemRefusal> {transaction_id_with(request,kind,&mut canonical_observation)}
fn parent_receipt_shape_with(receipt: &Value, kind: CompoundKind,observe:&mut impl FnMut(&Value)->Result<(String,usize),ItemRefusal>) -> Result<(String,usize), ItemRefusal> {
    let request = &receipt["request"];
    let request_observation=request_valid_with(request, kind,observe)?;
    let publication = &receipt["publication"];
    keys(
        publication,
        &["protocol", "transaction_id", "selected_files"],
    )?;
    let refs = array(&request["fields"], kind.field())?;
    if text(publication, "protocol")? != PROTOCOL
        || text(publication, "transaction_id")? != transaction_id_with_digest(request, kind,&request_observation.0,observe)?
        || publication["selected_files"] != {
            let mut names = selected_names(kind.parent_file())?;
            names.sort();
            json!(names)
        }
        || receipt["changed_fields"] != json!([kind.field()])
        || request["claim"]["predicate"] != kind.predicate()
        || request["claim"]["subject_ref"] != receipt["previous_source"]["id"]
        || !kind.initial_backlink(&request[kind.record_key()], &receipt["previous_source"]["id"])
        || kind == CompoundKind::ExpressionEdition
            && request[kind.record_key()]["embodies_expression_refs"]
                != json!([receipt["previous_source"]["id"]])
        || request["claim"]["object"] != request[kind.record_key()]["record_id"]
        || refs.last() != request["claim"].get("claim_id")
    {
        return Err(bad("explicit compound parent lineage"));
    }
    Ok(request_observation)
}

/// Existing `_history` law over exact read-only packages. Callers own custody.
/// Other compound parent handlers remain explicit unsupported profiles.
pub fn inspect_record_history(
    files: &BTreeMap<String, Vec<u8>>,
    record_raw: &[u8],
    deadline: std::time::Instant,
    cancelled: &AtomicBool,
) -> Result<Value, ItemRefusal> {
    check(deadline, cancelled)?;
    let record = decode(record_raw)?;
    let history = match files.get(HISTORY) {
        Some(raw) => decode(raw)?,
        None => json!({"schema_version":"tos_source_revision_history_v1","record_id":record["record_id"],"receipts":[]}),
    };
    validate_record_history_values(files,&record,&history,deadline,cancelled,&mut canonical_observation)?;
    Ok(history)
}
fn validate_record_history_values(files:&Package,record:&Value,history:&Value,deadline:Instant,cancelled:&AtomicBool,observe:&mut impl FnMut(&Value)->Result<(String,usize),ItemRefusal>)->Result<(),ItemRefusal> {
    let version=integer(record,"record_version")?;if version==0{return Err(bad("positive record version"));}
    let subject=json!({"id":text(record,"record_id")?,"version":version,"digest":observe(record)?.0});
    keys(&history, &["schema_version", "record_id", "receipts"])?;
    if !matches!(
        text(&history, "schema_version")?,
        "tos_source_revision_history_v1" | "tos_source_revision_history_v2"
    ) || history["record_id"] != subject["id"]
    {
        return Err(bad("history subject/version"));
    }
    let receipts = array(&history, "receipts")?;
    if receipts.len() > MAX_HISTORY || files.contains_key(HISTORY) && receipts.is_empty() {
        return Err(bad("stored history capacity/empty chain"));
    }
    let mut commands = BTreeSet::new();
    let mut previous = None;
    for receipt in receipts {
        check(deadline, cancelled)?;
        let selected = receipt.get("publication").is_some();
        let mut fields = vec![
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "reason",
            "previous_source",
            "source",
            "previous_revision",
            "archive_path",
            "dependencies",
            "changed_fields",
            "forms",
            "grants_admission",
            "request",
        ];
        if selected {
            fields.push("publication");
        }
        keys(receipt, &fields)?;
        if selected {
            if text(&history, "schema_version")? != "tos_source_revision_history_v2" {
                return Err(bad("selected history v2 required"));
            }
            let p = &receipt["publication"];
            keys(p, &["protocol", "transaction_id", "selected_files"])?;
            let names = array(p, "selected_files")?;
            if text(p, "protocol")? != PROTOCOL
                || !p["transaction_id"].is_string()
                || names.len() != 3
                || names
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != 3
                || !names.iter().any(|n| n == HISTORY)
                || names.iter().any(|n| {
                    n.as_str()
                        .is_none_or(|s| s.contains('/') || s.is_empty() || matches!(s, "." | ".."))
                })
            {
                return Err(bad("selected history publication binding"));
            }
        }
        crate::retirement_rules::observed_instant_order(
            text(receipt, "recorded_at")?,
            text(receipt, "recorded_at")?,
        )
        .map_err(|_| bad("history aware instant"))?;
        let request = &receipt["request"];
        let observed_request=match text(request, "operation")? {
            "collection.work.attach" | "expression.responsibility.attach" | "work.expression.create" | "expression.edition.create" | "item.adopt" => {
                Some(parent_receipt_shape_with(
                    receipt,
                    CompoundKind::from_operation(text(request, "operation")?)?,observe,
                )?)
            }
            "record.revise" => None,
            other => {
                return Err(ItemRefusal::Unsupported(format!(
                    "retained compound parent handler {other}"
                )));
            }
        };
        let fields = request["fields"]
            .as_object()
            .ok_or_else(|| bad("retained request fields"))?;
        if !commands.insert(text(receipt, "command_id")?)
            || text(receipt, "request_digest")? != match observed_request {Some((digest,_))=>digest,None=>observe(request)?.0}
            || receipt["command_id"] != request["command_id"]
            || receipt["previous_source"] != request["expected_source"]
            || receipt["previous_revision"] != request["expected_revision"]
            || receipt["owner_configuration"] != request["expected_configuration"]
            || receipt["dependencies"] != request["expected_dependencies"]
            || receipt["reason"] != request["reason"]
            || receipt["changed_fields"] != json!(fields.keys().collect::<Vec<_>>())
            || receipt["grants_admission"] != false
            || receipt["source"]["id"] != subject["id"]
            || receipt["previous_source"]["id"] != subject["id"]
            || integer(&receipt["source"], "version")?
                != integer(&receipt["previous_source"], "version")?
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?
            || previous.is_some_and(|p| receipt.get("previous_source") != Some(p))
        {
            return Err(bad("broken source revision chain"));
        }
        previous = receipt.get("source");
    }
    if previous.is_some_and(|p| p != &subject) {
        return Err(bad("current source differs from history head"));
    }
    check(deadline, cancelled)?;
    Ok(())
}

impl NativeCompoundReader<'_> {
    fn history(&mut self, path: &str, files: &Package) -> Result<std::sync::Arc<Value>, ItemRefusal> {
        let before = self.temporary_state;
        let result = self.history_inner(path, files);
        self.release_temporary_since(before);
        result
    }
    fn history_inner(&mut self, path: &str, files: &Package) -> Result<std::sync::Arc<Value>, ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        // Bind memoized lineage to the whole selected package, not its subject
        // alone: source-copy forms and history bytes participate in revision.
        let key = (path.to_owned(), self.package_revision(files)?);
        let key_state=std::mem::size_of_val(&key).checked_add(key.0.len()).and_then(|n|n.checked_add(key.1.len())).ok_or(ItemRefusal::Budget)?;
        self.temporary(key_state)?;
        if let Some(history) = self.histories.get(&key) {
            return Ok(history.clone());
        }
        let name = path
            .rsplit('/')
            .next()
            .ok_or_else(|| bad("record basename"))?;
        let raw = files
            .get(name)
            .ok_or_else(|| bad("history source absent"))?;
        let record=self.decoded(raw)?;
        let id=text(&record,"record_id")?;
        let history=match files.get(HISTORY) {
            Some(raw)=>self.decoded(raw)?,
            None=>{let value=json!({"schema_version":"tos_source_revision_history_v1","record_id":id,"receipts":[]});self.temporary(crate::record_biblio_cut::decoded_state(&value)?)?;value}
        };
        // The commands index borrows retained history strings. Field-name
        // vectors contain only references; the exact subject is one owned value.
        let history_validation_state=reference_state(&record,"record_id","record_version")?+array(&history,"receipts")?.len()*std::mem::size_of::<&str>()+17*std::mem::size_of::<&str>();
        self.temporary(history_validation_state)?;
        validate_record_history_values(files,&record,&history,self.limits.deadline,self.cancelled,&mut |value|self.canonical_observation(value))?;
        self.release_temporary(history_validation_state);
        for (index, receipt) in array(&history, "receipts")?.iter().enumerate() {
            check(self.limits.deadline, self.cancelled)?;
            let previous_temporary = self.temporary_state;
            let archived = self.archive(path, id, receipt)?;
            let predecessor_record=self.decoded(archived.get(name).ok_or_else(||bad("archive source absent"))?)?;
            let predecessor=match archived.get(HISTORY) {
                Some(raw)=>self.decoded(raw)?,
                None=>{let value=json!({"schema_version":"tos_source_revision_history_v1","record_id":id,"receipts":[]});self.temporary(crate::record_biblio_cut::decoded_state(&value)?)?;value}
            };
            let predecessor_subject_state=reference_state(&predecessor_record,"record_id","record_version")?;
            let predecessor_indexes=array(&predecessor,"receipts")?.len()*std::mem::size_of::<&str>()+17*std::mem::size_of::<&str>();
            self.temporary(predecessor_subject_state.checked_add(predecessor_indexes).ok_or(ItemRefusal::Budget)?)?;
            validate_record_history_values(&archived,&predecessor_record,&predecessor,self.limits.deadline,self.cancelled,&mut |value|self.canonical_observation(value))?;
            if array(&predecessor, "receipts")? != &array(&history, "receipts")?[..index] {
                return Err(bad("retained predecessor receipt prefix"));
            }
            drop(predecessor);
            drop(predecessor_record);
            drop(archived);
            self.release_temporary_since(previous_temporary);
        }
        let history_state = crate::record_biblio_cut::decoded_state(&history)?
            .checked_add(key.0.len()+key.1.len()+std::mem::size_of::<((String,String),std::sync::Arc<Value>)>()+2*std::mem::size_of::<usize>()).ok_or(ItemRefusal::Budget)?;
        let tree_state=crate::record_biblio_cut::decoded_state(&history)?;
        self.state-=tree_state+key_state; self.temporary_state-=tree_state+key_state;
        reserve(&mut self.state, history_state, self.limits.max_state_bytes)?;
        let history=std::sync::Arc::new(history);
        self.histories.insert(key,history.clone());
        Ok(history)
    }
}

const SCOPE_KEYS: [&str; 9] = [
    "work_id",
    "work_source_path",
    "expression_id",
    "expression_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_work_form_ids",
    "allowed_expression_form_ids",
    "allowed_claim_form_ids",
];
const EDITION_SCOPE_KEYS: [&str; 11] = [
    "work_id",
    "work_source_path",
    "expression_id",
    "expression_source_path",
    "edition_id",
    "edition_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_expression_form_ids",
    "allowed_edition_form_ids",
    "allowed_claim_form_ids",
];
fn typed_id(id: &str, kind: &str) -> bool {
    id.strip_prefix(&format!("tos.{kind}."))
        .is_some_and(segment)
}
fn segment(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-'))
        && s.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && s.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
        && !s
            .as_bytes()
            .windows(2)
            .any(|p| matches!(p[0], b'.' | b'-') && matches!(p[1], b'.' | b'-'))
}
fn scope_valid(
    scope: &Value,
    request: &Value,
    authority: &Value,
    kind: CompoundKind,
) -> Result<(), ItemRefusal> {
    if kind.relation_attachment() {
        attachment_scope_valid(scope,request,kind)?;
    } else if kind == CompoundKind::EditionItem {
        item_scope_valid(scope, request)?;
    } else {
        keys(
            scope,
            if kind == CompoundKind::WorkExpression {
                &SCOPE_KEYS
            } else {
                &EDITION_SCOPE_KEYS
            },
        )?;
        for (k, kind) in [
            ("work_id", "work"),
            ("expression_id", "expression"),
            ("claim_id", "claim"),
            ("provenance_event_id", "event"),
        ] {
            if !typed_id(text(scope, k)?, kind) {
                return Err(bad("typed compound scope identities"));
            }
        }
        let work = text(scope, "work_source_path")?;
        let expression = text(scope, "expression_source_path")?;
        metadata_path(work, false)?;
        metadata_path(expression, false)?;
        if !work.starts_with("ToS/source-witnesses/works/")
            || work.split('/').count() < 5
            || !work.ends_with("/work.json")
            || !expression.ends_with("/expression.json")
        {
            return Err(bad("Work Expression home grammar"));
        }
        let child = parent(expression)?;
        let expected = format!("{}/expressions/", parent(work)?);
        if !child
            .strip_prefix(&expected)
            .is_some_and(|s| !s.contains('/') && segment(s))
        {
            return Err(bad("one exact child home"));
        }
        if kind == CompoundKind::ExpressionEdition {
            if !typed_id(text(scope, "edition_id")?, "edition") {
                return Err(bad("typed Edition identity"));
            }
            let edition = text(scope, "edition_source_path")?;
            metadata_path(edition, false)?;
            let expected = format!("{}/editions/", parent(expression)?);
            if !edition.ends_with("/edition.json")
                || !parent(edition)?
                    .strip_prefix(&expected)
                    .is_some_and(|s| !s.contains('/') && segment(s))
            {
                return Err(bad("one exact Edition home"));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for (field, allowed) in [
        (
            "forms",
            match kind {
                CompoundKind::CollectionWork => "allowed_collection_form_ids",
                CompoundKind::ExpressionResponsibility => "allowed_expression_form_ids",
                CompoundKind::WorkExpression => "allowed_work_form_ids",
                CompoundKind::ExpressionEdition => "allowed_expression_form_ids",
                CompoundKind::EditionItem => "allowed_edition_form_ids",
            },
        ),
        (
            kind.child_form_request(),
            match kind {
                CompoundKind::CollectionWork => "allowed_collection_form_ids",
                CompoundKind::ExpressionResponsibility => "allowed_expression_form_ids",
                CompoundKind::WorkExpression => "allowed_expression_form_ids",
                CompoundKind::ExpressionEdition => "allowed_edition_form_ids",
                CompoundKind::EditionItem => "allowed_item_form_ids",
            },
        ),
        ("claim_forms", "allowed_claim_form_ids"),
    ].into_iter().filter(|(field,_)|!kind.relation_attachment()||*field!="forms").chain((kind.relation_attachment()).then_some(("forms",kind.parent_form_grant()))) {
        let ids = array(scope, allowed)?;
        if !(1..=32).contains(&ids.len()) {
            return Err(bad("form identity bounds"));
        }
        let mut grant = BTreeSet::new();
        for id in ids {
            let id = id.as_str().ok_or_else(|| bad("form identity"))?;
            let tail = id
                .strip_prefix("tos.form.")
                .ok_or_else(|| bad("form identity prefix"))?;
            if tail.is_empty()
                || !tail.as_bytes()[0].is_ascii_alphanumeric()
                || !tail.bytes().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-' | b'_')
                })
                || !seen.insert(id)
                || !grant.insert(id)
            {
                return Err(bad("distinct subject form identities"));
            }
        }
        let selections = array(request, field)?;
        if !(1..=32).contains(&selections.len()) {
            return Err(bad("explicit form selections"));
        }
        let mut selected = BTreeSet::new();
        for item in selections {
            keys(item, &["form_id", "field_id"])?;
            if !grant.contains(text(item, "form_id")?) || !selected.insert(text(item, "form_id")?) {
                return Err(bad("form selection exceeds retained scope"));
            }
            text(item, "field_id")?;
        }
    }
    let claim = &request["claim"];
    let record = &request[kind.record_key()];
    if record["record_id"] != scope[kind.child_key()]
        || !kind.initial_backlink(record, &scope[kind.parent_key()])
        || kind == CompoundKind::ExpressionEdition
            && record["embodies_expression_refs"] != json!([scope["expression_id"]])
        || kind == CompoundKind::EditionItem
            && record["item_manifest_ref"]
                != format!(
                    "{}/item.manifest.json",
                    parent(text(scope, "item_source_path")?)?
                )
        || claim["claim_id"] != scope["claim_id"]
        || claim["subject_ref"] != scope[kind.parent_key()]
        || claim["object"] != scope[kind.child_key()]
        || claim["provenance_event_ref"] != scope["provenance_event_id"]
        || claim["maker"]
            != json!({"maker_type":authority["maker_type"],"agent_ref":authority["principal_id"]})
    {
        return Err(bad("exact scope/request endpoints and maker"));
    }
    Ok(())
}
const COLLECTION_SCOPE_KEYS:[&str;12]=["collection_id","collection_source_path","work_id","work_source_path","predicate","claim_id","claim_source_path","provenance_event_id","allowed_collection_form_ids","allowed_claim_form_ids","allowed_evidence_refs","retained_membership_provenance_refs"];
const RESPONSIBILITY_SCOPE_KEYS:[&str;11]=["expression_id","expression_source_path","agent_id","agent_source_path","predicate","claim_id","claim_source_path","provenance_event_id","allowed_expression_form_ids","allowed_claim_form_ids","allowed_evidence_refs"];
fn attachment_scope_valid(scope:&Value,request:&Value,kind:CompoundKind)->Result<(),ItemRefusal> {
    keys(scope,if kind==CompoundKind::CollectionWork {&COLLECTION_SCOPE_KEYS[..]}else{&RESPONSIBILITY_SCOPE_KEYS[..]})?;
    for (field,kind) in [(kind.parent_key(),kind.parent_kind()),(kind.child_key(),kind.child_kind()),("claim_id","claim"),("provenance_event_id","event")] {
        if !typed_id(text(scope,field)?,kind) {return Err(bad("membership typed scope identities"));}
    }
    let collection=text(scope,kind.parent_path())?;let work=text(scope,kind.child_path())?;let claim=text(scope,"claim_source_path")?;
    for path in [collection,work,claim] {metadata_path(path,false)?;}
    if text(scope,"predicate")?!=kind.predicate()||if kind==CompoundKind::CollectionWork {!collection.starts_with("ToS/source-witnesses/collections/")||collection.split('/').count()<5||!collection.ends_with("/collection.json")||!work.starts_with("ToS/source-witnesses/works/")||!work.ends_with("/work.json")}else{!collection.starts_with("ToS/source-witnesses/works/")||!collection.ends_with("/expression.json")||!collection.split('/').any(|part|part=="expressions")||!work.starts_with("ToS/source-witnesses/agents/")||!work.ends_with("/agent.json")}||!claim.starts_with("ToS/source-witnesses/relations/")||claim.split('/').count()!=5||!claim.ends_with("/source-claims.jsonl")||!segment(parent(claim)?.rsplit('/').next().unwrap()) {return Err(bad("membership separate exact public homes"));}
    if request[kind.record_key()]["record_type"]!=kind.child_kind()||request["claim"]["predicate"]!=scope["predicate"] {return Err(bad("membership exact Work/Claim route"));}
    let evidence=array(scope,"allowed_evidence_refs")?;
    if !(1..=128).contains(&evidence.len()) {return Err(bad("membership evidence bounds"));}
    let mut seen=BTreeSet::new();
    for value in evidence {let value=value.as_str().ok_or_else(||bad("membership evidence strings"))?;if value.chars().count()>4096||tos_foundation::python_strip_unicode16_v1(value,MAX_SIDE).map_err(|_|ItemRefusal::Budget)?.is_empty()||!seen.insert(value) {return Err(bad("membership distinct explicit evidence"));}}
    for field in ["evidence_refs","counterevidence_refs"] {if let Some(refs)=request["claim"].get(field) {for value in refs.as_array().ok_or_else(||bad("membership evidence array"))? {if !value.is_string()||!evidence.contains(value) {return Err(bad("membership evidence exceeds recorded scope"));}}}}
    if kind==CompoundKind::CollectionWork {
    let provenance=array(scope,"retained_membership_provenance_refs")?;
    if provenance.len()>32 {return Err(bad("membership retained provenance bounds"));}
    seen.clear();for value in provenance {let path=value.as_str().ok_or_else(||bad("membership provenance path"))?;metadata_path(path,false)?;let name=path.rsplit('/').next().unwrap();if !name.ends_with(".jsonl")||!name.contains("provenance")||!seen.insert(path) {return Err(bad("membership retained provenance grammar"));}}
    }
    Ok(())
}
const ITEM_SCOPE_KEYS: [&str; 18] = [
    "edition_id",
    "edition_source_path",
    "item_id",
    "item_source_path",
    "claim_id",
    "provenance_event_id",
    "allowed_edition_form_ids",
    "allowed_item_form_ids",
    "allowed_claim_form_ids",
    "file_id",
    "payload_basename",
    "original_basename",
    "media_type",
    "byte_size",
    "sha256",
    "rights_id",
    "acquisition_event_id",
    "inventory_event_id",
];
fn item_scope_valid(scope: &Value, request: &Value) -> Result<(), ItemRefusal> {
    keys(scope, &ITEM_SCOPE_KEYS)?;
    for (key, kind) in [
        ("edition_id", "edition"),
        ("item_id", "item"),
        ("file_id", "file"),
        ("claim_id", "claim"),
        ("provenance_event_id", "event"),
        ("rights_id", "rights"),
        ("acquisition_event_id", "event"),
        ("inventory_event_id", "event"),
    ] {
        if !typed_id(text(scope, key)?, kind) {
            return Err(bad("typed Item adoption identity"));
        }
    }
    if [
        text(scope, "provenance_event_id")?,
        text(scope, "acquisition_event_id")?,
        text(scope, "inventory_event_id")?,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>()
    .len()
        != 3
    {
        return Err(bad("separate Item copy/enumeration/serialization events"));
    }
    let edition = text(scope, "edition_source_path")?;
    let item = text(scope, "item_source_path")?;
    metadata_path(edition, false)?;
    metadata_path(item, false)?;
    let prefix = format!("{}/items/", parent(edition)?);
    if !edition.ends_with("/edition.json")
        || !edition.split('/').any(|p| p == "editions")
        || !item.ends_with("/item.json")
        || !parent(item)?
            .strip_prefix(&prefix)
            .is_some_and(|part| !part.contains('/') && segment(part))
    {
        return Err(bad("one exact Item child home"));
    }
    let payload = text(scope, "payload_basename")?;
    let original = text(scope, "original_basename")?;
    let media = text(scope, "media_type")?;
    let media_part = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'+' | b'-')
            })
    };
    if !(1..=201).contains(&payload.len())
        || !payload.as_bytes()[0].is_ascii_alphanumeric()
        || !payload
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        || !(1..=256).contains(&original.chars().count())
        || original
            .chars()
            .any(|c| matches!(c, '/' | '\\' | '\n' | '\r' | '\0'))
        || !media
            .split_once('/')
            .is_some_and(|(a, b)| media_part(a) && media_part(b))
        || !(1..=536_870_912).contains(&integer(scope, "byte_size")?)
    {
        return Err(bad("bounded Item File name/media/size"));
    }
    hash(&format!("sha256:{}", text(scope, "sha256")?))?;
    let rights = &request["rights"];
    let scopes = array(rights, "scope_refs")?
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    if rights["rights_id"] != scope["rights_id"]
        || scopes != BTreeSet::from([text(scope, "item_id")?, text(scope, "file_id")?])
        || rights["visibility"] != "local_only"
        || rights["review_status"] != "unreviewed"
        || !matches!(
            text(rights, "assessment_status")?,
            "not_assessed" | "copyright_not_evaluated" | "copyright_undetermined"
        )
        || rights["redistribution_posture"] != "not_authorized"
        || rights["derivative_posture"] != "local_research_only"
        || rights["permissions"] != json!([])
        || rights
            .get("layer_assessments")
            .is_some_and(|v| *v != json!([]))
        || !matches!(
            text(request, "item_kind")?,
            "born_digital" | "digitized_physical_copy" | "derived_publication" | "unknown"
        )
    {
        return Err(bad(
            "supplied Item rights remain separate unreviewed local-only observations",
        ));
    }
    Ok(())
}
fn item_payload(scope: &Value) -> Result<Value, ItemRefusal> {
    Ok(
        json!({"file_id":scope["file_id"],"relative_path":format!("payload/{}",text(scope,"payload_basename")?),
        "original_basename":scope["original_basename"],"media_type":scope["media_type"],
        "byte_size":scope["byte_size"],"sha256":scope["sha256"]}),
    )
}
fn item_companions(
    scope: &Value,
    request: &Value,
    byte_receipt: &Value,
    generator: &str,
    schemas: &mut impl CutSchemaExecutor,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> Result<Vec<(String, Vec<u8>)>, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    keys(
        byte_receipt,
        &[
            "schema_version",
            "transaction_id",
            "owner_configuration",
            "private_stage_digest",
            "recovery_configuration",
            "file",
            "started_at",
            "deposited_at",
            "observation_interval",
            "original_preserved",
            "metadata_committed",
            "grants_admission",
        ],
    )?;
    let identifier = transaction_id(request, CompoundKind::EditionItem)?;
    if byte_receipt["schema_version"] != "tos_item_deposit_receipt_v1"
        || byte_receipt["transaction_id"] != identifier
        || byte_receipt["owner_configuration"] != request["expected_configuration"]
        || byte_receipt["file"] != item_payload(scope)?
        || byte_receipt["original_preserved"] != true
        || byte_receipt["metadata_committed"] != false
        || byte_receipt["grants_admission"] != false
    {
        return Err(bad("public Item byte receipt exact File binding"));
    }
    hash(text(byte_receipt, "private_stage_digest")?)?;
    if !byte_receipt["recovery_configuration"].is_null() {
        hash(text(byte_receipt, "recovery_configuration")?)?;
    }
    let interval = &byte_receipt["observation_interval"];
    keys(interval, &["started_at", "ended_at"])?;
    let times = [
        text(interval, "started_at")?,
        text(interval, "ended_at")?,
        text(byte_receipt, "started_at")?,
        text(byte_receipt, "deposited_at")?,
    ];
    for pair in times.windows(2) {
        if crate::retirement_rules::observed_instant_order(pair[0], pair[1])
            .map_err(|_| bad("Item deposit aware chronology"))?
            == std::cmp::Ordering::Greater
        {
            return Err(bad("Item observation/deposit times reversed"));
        }
    }
    // A public retained receipt binds source-safe observations, not possession
    // of the private stage, current payload fixity or any publication grant.
    let boundary = match generator {
        "1" => {
            "resource enumeration, geometry, ordering, counts, and one-way fingerprints only; no source text, bibliographic acceptance, textual acceptance, rights clearance, translation, semantics, or canon authority"
        }
        "2" => {
            "This inventory records resource enumeration, geometry, ordering, counts and one-way fingerprints for the selected source."
        }
        _ => {
            return Err(ItemRefusal::Unsupported(
                "resource inventory generator version".into(),
            ));
        }
    };
    let home = parent(text(scope, "item_source_path")?)?;
    let locator = |name: &str| format!("{home}/{name}");
    let stamp = text(byte_receipt, "deposited_at")?;
    let payload = object(vec![
        ("file_id", j(&scope["file_id"])?),
        (
            "relative_path",
            string(&format!("payload/{}", text(scope, "payload_basename")?)),
        ),
        ("original_basename", j(&scope["original_basename"])?),
        ("media_type", j(&scope["media_type"])?),
        ("byte_size", j(&scope["byte_size"])?),
        ("sha256", j(&scope["sha256"])?),
        ("fixity_verified_at", string(stamp)),
    ]);
    let manifest = object(vec![
        ("schema_version", string("tos_source_item_manifest_v1")),
        ("item_id", j(&scope["item_id"])?),
        ("item_kind", j(&request["item_kind"])?),
        ("embodiment_ref", j(&scope["edition_id"])?),
        ("storage_posture", string("local_gitignored_payload")),
        ("payload_files", JsonValue::Array(vec![payload])),
        ("acquisition_event_ref", j(&scope["acquisition_event_id"])?),
        ("rights_ref", string(&locator("rights.json"))),
        ("provenance_ref", string(&locator("provenance.jsonl"))),
        (
            "forensic_report_ref",
            string(&locator("forensic-report.md")),
        ),
        (
            "resource_inventory_ref",
            string(&locator("resource-inventory.json")),
        ),
        ("visibility", string("local_only")),
        ("manifest_version", j(&json!(1))?),
    ]);
    let input = &request["inventory"];
    if input.is_null() || !request["inventory_limitation"].is_null() {
        return Err(bad("Item inventory unavailable"));
    }
    for (key, value) in [
        ("file_id", &scope["file_id"]),
        ("file_sha256", &scope["sha256"]),
        ("media_type", &scope["media_type"]),
    ] {
        if input.get(key) != Some(value) {
            return Err(bad("Item inventory exact granted File"));
        }
    }
    let inventory = object(vec![
        (
            "$schema",
            string(
                "https://tree-of-sophia.local/ToS/contracts/source-resource-inventory.schema.json",
            ),
        ),
        ("schema_version", string("tos_source_resource_inventory_v1")),
        ("item_id", j(&scope["item_id"])?),
        (
            "generated_from_manifest_ref",
            string(&locator("item.manifest.json")),
        ),
        ("inventory_authority", string("mechanical_metadata_only")),
        ("source_text_included", JsonValue::Bool(false)),
        ("files", JsonValue::Array(vec![j(input)?])),
        (
            "generator",
            object(vec![
                ("name", string("build_source_resource_inventories.py")),
                ("version", string(generator)),
            ]),
        ),
        ("provenance_event_ref", j(&scope["inventory_event_id"])?),
        ("inventory_version", j(&json!(1))?),
        ("supersedes_inventory_ref", JsonValue::Null),
        ("authority_boundary", string(boundary)),
    ]);
    let rights = j(&request["rights"])?;
    let guard=|used:usize|if used>limits.max_state_bytes {Err(ItemRefusal::BudgetCheck{check:"compound Item companion logical workspace",used:Some(used as u64),limit:Some(limits.max_state_bytes as u64)})}else{Ok(())};
    let mut workspace=0usize;
    for value in [&manifest,&inventory,&rights] {workspace=workspace.checked_add(crate::record_biblio_cut::ordered_state(value)?).ok_or(ItemRefusal::Budget)?;}
    guard(workspace)?;
    for (name, leaf, value) in [
        ("source-item-manifest", "item.manifest.json", &manifest),
        (
            "source-resource-inventory",
            "resource-inventory.json",
            &inventory,
        ),
        ("rights-record", "rights.json", &rights),
    ] {
        let raw=canonical_ordered(value)?;
        guard(workspace.checked_add(raw.len()).ok_or(ItemRefusal::Budget)?)?;
        if !schemas.check_reusing_scalar(
            &format!("{}#compound-reconstructed", locator(leaf)),
            &raw,
            &format!("ToS/contracts/{name}.schema.json"),
            limits.deadline,
            cancelled,
        )? {
            return Err(bad("Item companion selected schema"));
        }
    }
    let inventory_raw = pretty(&inventory)?;
    let event = json!({"schema_version":"tos_provenance_event_v1","event_id":scope["acquisition_event_id"],
        "event_type":"acquisition","started_at":byte_receipt["started_at"],"ended_at":stamp,
        "agent_refs":["software:tos-source-item-commands"],
        "inputs":[{"ref":scope["file_id"],"role":"previously_acquired_local_input","sha256":scope["sha256"]}],
        "outputs":[{"ref":locator(&format!("payload/{}",text(scope,"payload_basename")?)),"role":"retained_local_witness_bytes","sha256":scope["sha256"]}],
        "method":{"maker_type":"software","name":"bounded-local-file-adoption","version":"1","configuration":{
            "transaction_id":identifier,"owner_configuration":request["expected_configuration"],"byte_receipt_ref":locator("item-deposit-receipt.json")}},
        "status":"completed_with_warnings","warnings":["Local retention only; not rights, bibliographic or textual admission."],
        "receipt_refs":[locator("edition-item-receipt.json")],"rights_basis_ref":locator("rights.json"),"event_version":1});
    workspace=workspace.checked_add(inventory_raw.len()).and_then(|n|n.checked_add(crate::record_biblio_cut::decoded_state(&event).ok()?)).ok_or(ItemRefusal::Budget)?;
    guard(workspace.checked_add(crate::record_biblio_cut::decoded_state(&event)?).ok_or(ItemRefusal::Budget)?)?;
    let mut enumeration = event.clone();
    for (key, value) in [
        ("event_id", scope["inventory_event_id"].clone()),
        ("event_type", json!("forensic_inspection")),
        ("started_at", interval["started_at"].clone()),
        ("ended_at", interval["ended_at"].clone()),
        (
            "inputs",
            json!([{"ref":scope["file_id"],"role":"resource_inventory_input","sha256":scope["sha256"]}]),
        ),
        (
            "outputs",
            json!([{"ref":locator("resource-inventory.json"),"role":"tracked_text_free_resource_inventory","sha256":Digest256::of_bytes(&inventory_raw).to_hex()}]),
        ),
        (
            "method",
            json!({"maker_type":"software","name":"build_source_resource_inventories.py","version":generator,
            "configuration":{"scope":"resource enumeration only; no text extraction or semantic reading"}}),
        ),
    ] {
        enumeration
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
    }
    workspace=workspace.checked_add(crate::record_biblio_cut::decoded_state(&enumeration)?).ok_or(ItemRefusal::Budget)?;
    guard(workspace)?;
    let mut provenance = Vec::new();
    for (index, value) in [&event, &enumeration].into_iter().enumerate() {
        let raw = canonical(value)?;
        guard(workspace.checked_add(provenance.len()).and_then(|n|n.checked_add(raw.len())).ok_or(ItemRefusal::Budget)?)?;
        if !schemas.check_reusing_scalar(
            &format!(
                "{}:{}#compound-reconstructed",
                locator("provenance.jsonl"),
                index + 1
            ),
            &raw,
            "ToS/contracts/provenance-event.schema.json",
            limits.deadline,
            cancelled,
        )? {
            return Err(bad("Item acquisition/enumeration selected schema"));
        }
        provenance.extend(raw);
        provenance.push(b'\n');
    }
    let report = format!(
        "# Local Item adoption forensic boundary\n\nFile: {}\nSHA-256: {}\nRetains one unchanged previously acquired local file; the input is preserved.\nThe inventory enumerates resources only. No OCR, correction, translation, source reading,\ncopyright clearance, publication authorization or semantic acceptance was performed.\nCopy and metadata publication are separate stages; retained transaction evidence owns recovery.\n",
        text(scope, "file_id")?,
        text(scope, "sha256")?
    );
    let inventory_bytes=inventory_raw.len();
    let result=vec![
        ("item.manifest.json".into(), pretty(&manifest)?),
        ("rights.json".into(), pretty(&rights)?),
        ("resource-inventory.json".into(), inventory_raw),
        (
            "fixity.sha256".into(),
            format!(
                "{}  payload/{}\n",
                text(scope, "sha256")?,
                text(scope, "payload_basename")?
            )
            .into_bytes(),
        ),
        ("forensic-report.md".into(), report.into_bytes()),
        ("provenance.jsonl".into(), provenance),
    ];
    // inventory/provenance/report bytes move into these output rows, while the
    // manifest/rights/event trees above remain live until this return.
    let trees=workspace.checked_sub(inventory_bytes).unwrap_or(workspace);
    guard(trees.checked_add(rows_state(&result,true)?).ok_or(ItemRefusal::Budget)?)?;
    Ok(result)
}

fn claim_lines(raw:&[u8])->impl Iterator<Item=(&[u8],&[u8])> {
    let mut offset=0usize;
    std::iter::from_fn(move||{if offset>=raw.len(){return None;}let start=offset;while offset<raw.len()&&!matches!(raw[offset],b'\n'|b'\r'){offset+=1;}let end=offset;if offset<raw.len(){let marker=raw[offset];offset+=1;if marker==b'\r'&&offset<raw.len()&&raw[offset]==b'\n'{offset+=1;}}Some((&raw[start..end],&raw[end..offset]))})
}
fn advance_membership_claim(previous:&Value,request:&Value)->Result<Value,ItemRefusal> {
    let fields=request["fields"].as_object().ok_or_else(||bad("membership correction field patch"))?;
    let transition=request.get("layer_transition").filter(|v|!v.is_null());
    if fields.is_empty()||fields.keys().any(|key|if transition.is_some(){key!="assertion_layer"}else{!["qualifiers","evidence_refs","counterevidence_refs","alternative_claim_refs","supporting_quotes","epistemic_status","confidence","object"].contains(&key.as_str())}) {return Err(bad("membership correction cannot change identity/maker/endpoints"));}
    if let Some(transition)=transition {keys(transition,&["from","to"])?;let from=text(transition,"from")?;let to=text(transition,"to")?;let layer=|value:&str|!value.is_empty()&&value.len()<=64&&value.as_bytes()[0].is_ascii_lowercase()&&value.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'_');if !layer(from)||!layer(to)||from==to||previous["assertion_layer"]!=from||fields["assertion_layer"]!=to {return Err(bad("membership exact retained layer transition"));}}
    if fields.contains_key("object")&&(!previous["object"].is_object()||!fields["object"].is_object()) {return Err(bad("membership correction cannot change identity endpoint"));}
    let mut result=previous.clone();for (key,value) in fields {if key=="qualifiers" {let patch=value.as_object().ok_or_else(||bad("membership qualifier correction patch"))?;let target=result.as_object_mut().unwrap().entry(key.clone()).or_insert_with(||json!({})).as_object_mut().ok_or_else(||bad("membership qualifiers object"))?;target.extend(patch.iter().map(|(k,v)|(k.clone(),v.clone())));}else{result.as_object_mut().unwrap().insert(key.clone(),value.clone());}}
    if &result==previous {return Err(bad("membership correction did not change source"));}
    result.as_object_mut().unwrap().insert("claim_version".into(),json!(integer(previous,"claim_version")?.checked_add(1).ok_or(ItemRefusal::Budget)?));Ok(result)
}

fn j(value: &Value) -> Result<JsonValue, ItemRefusal> {
    ordered(&serde_json::to_vec(value).map_err(|_| bad("JSON value encoding"))?)
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (tos_foundation::JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn string(s: &str) -> JsonValue {
    JsonValue::String(tos_foundation::JsonString::from_utf8(s))
}
fn set(value: &mut JsonValue, key: &str, new: JsonValue) -> Result<(), ItemRefusal> {
    let JsonValue::Object(fields) = value else {
        return Err(bad("ordered object required"));
    };
    if let Some((_, old)) = fields.iter_mut().find(|(k, _)| k.as_str() == Some(key)) {
        *old = new;
    } else {
        fields.push((tos_foundation::JsonString::from_utf8(key), new));
    }
    Ok(())
}
fn ref_ordered(v: &Value, id: &str, version: &str) -> Result<JsonValue, ItemRefusal> {
    let r = reference(v, id, version)?;
    Ok(object(vec![
        ("id", j(&r["id"])?),
        ("version", j(&r["version"])?),
        ("digest", j(&r["digest"])?),
    ]))
}
fn refs_ordered<T:AsRef<[u8]>>(files: &[(String, T)]) -> JsonValue {
    JsonValue::Object(
        files
            .iter()
            .map(|(name, raw)| {
                (
                    tos_foundation::JsonString::from_utf8(name),
                    object(vec![
                        ("sha256", string(&Digest256::of_bytes(raw.as_ref()).to_prefixed())),
                        ("bytes", j(&json!(raw.as_ref().len())).expect("bounded byte length")),
                    ]),
                )
            })
            .collect(),
    )
}
fn forms(
    source: &JsonValue,
    previous: Option<&JsonValue>,
    selections: &Value,
    principal: &str,
    claim: bool,
    available:usize,
) -> Result<(JsonValue, JsonValue), ItemRefusal> {
    use crate::source_forms::source_copy_kernel as kernel;
    let fail = |e| ItemRefusal::Unsupported(format!("compound source-copy forms: {e:?}"));
    let selections = selections
        .as_array()
        .ok_or_else(|| bad("form selections"))?;
    if let Some(previous) = previous {
        let old = previous
            .object_get("forms")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| bad("previous forms"))?;
        for form in old {
            let id = form
                .object_get("form_id")
                .and_then(JsonValue::as_str)
                .ok_or_else(|| bad("previous form id"))?;
            if !selections.iter().any(|s| s["form_id"] == id)
                || form
                    .object_get("content")
                    .and_then(|v| v.object_get("kind"))
                    .and_then(JsonValue::as_str)
                    != Some("source-copy")
            {
                return Err(bad("parent explicitly rebinds all source-copy forms"));
            }
        }
    }
    let fields=kernel::metadata_fields(source).map_err(fail)?;
    let mut fields_state=std::mem::size_of::<Vec<kernel::FormField>>();
    for field in &fields {
        let strings=field.id.len()+field.pointer.len()+field.role.len();
        let context=field.context.iter().try_fold(0usize,|n,s|n.checked_add(std::mem::size_of::<String>()+s.len())).ok_or(ItemRefusal::Budget)?;
        fields_state=fields_state.checked_add(std::mem::size_of::<kernel::FormField>()).and_then(|n|n.checked_add(strings)).and_then(|n|n.checked_add(context))
            .and_then(|n|n.checked_add(crate::record_biblio_cut::ordered_state(&field.language).ok()?.checked_sub(std::mem::size_of::<JsonValue>())?)).and_then(|n|n.checked_add(crate::record_biblio_cut::ordered_state(&field.script).ok()?.checked_sub(std::mem::size_of::<JsonValue>())?)).ok_or(ItemRefusal::Budget)?;
    }
    let guard=|used:usize|if used>available {Err(ItemRefusal::BudgetCheck{check:"compound source-copy logical workspace",used:Some(used as u64),limit:Some(available as u64)})}else{Ok(())};
    // Canonical hash/equality helpers emit at most two independent buffers at
    // once. Price the actual selected source/prior bytes, not a raw multiplier.
    let mut codec_indexes=crate::record_biblio_cut::ordered_emit_state(source)?;
    guard(fields_state.checked_add(codec_indexes).ok_or(ItemRefusal::Budget)?)?;
    let mut codec_wire=canonical_count_v1(source,CanonicalProfile::SourceCommandInputV1,limits()).map_err(|error|ItemRefusal::Unsupported(format!("compound canonical: {error:?}")))?;
    let prior_codec=if let Some(previous)=previous {
        guard(fields_state.checked_add(crate::record_biblio_cut::ordered_emit_state(previous)?).ok_or(ItemRefusal::Budget)?)?;
        let bytes=canonical_count_v1(previous,CanonicalProfile::SourceCommandInputV1,limits()).map_err(|error|ItemRefusal::Unsupported(format!("compound canonical: {error:?}")))?;
        codec_wire=codec_wire.max(bytes);codec_indexes=codec_indexes.max(crate::record_biblio_cut::ordered_emit_state(previous)?);
        bytes.checked_add(crate::record_biblio_cut::ordered_codec_state(previous)?).ok_or(ItemRefusal::Budget)?
    }else{0};
    guard(fields_state.checked_add(codec_wire.checked_add(codec_indexes).ok_or(ItemRefusal::Budget)?).ok_or(ItemRefusal::Budget)?)?;
    let subject=kernel::metadata_subject(source).map_err(fail)?;
    let subject_state=crate::record_biblio_cut::ordered_state(&subject)?;
    let empty=if previous.is_none(){Some(kernel::empty_set(&subject))}else{None};
    let empty_state=empty.as_ref().map(crate::record_biblio_cut::ordered_state).transpose()?.unwrap_or(0);
    guard(fields_state.checked_add(subject_state).and_then(|n|n.checked_add(empty_state)).and_then(|n|n.checked_add(codec_wire.checked_add(codec_indexes)?)).ok_or(ItemRefusal::Budget)?)?;
    let mut changes = Vec::new();
    let mut changes_state=std::mem::size_of::<Vec<JsonValue>>();
    for selection in selections {
        let field_id=text(selection,"field_id")?;
        let selected=fields.iter().find(|field|field.id==field_id).ok_or_else(||fail(kernel::FormMechanicsError::Invalid("unknown source field selector")))?;
        let change = kernel::prepared_change(
                previous.or(empty.as_ref()).ok_or_else(||bad("form preparation base"))?,
                &subject,
                principal,
                text(selection, "form_id")?,
                selected,
            ).map_err(fail)?;
        changes_state=changes_state.checked_add(crate::record_biblio_cut::ordered_state(&change)?).ok_or(ItemRefusal::Budget)?;
        let indexes=crate::record_biblio_cut::ordered_emit_state(&change)?;
        guard(fields_state.checked_add(changes_state).and_then(|n|n.checked_add(subject_state)).and_then(|n|n.checked_add(empty_state)).and_then(|n|n.checked_add(indexes)).ok_or(ItemRefusal::Budget)?)?;
        let bytes=canonical_count_v1(&change,CanonicalProfile::SourceCommandInputV1,limits()).map_err(|error|ItemRefusal::Unsupported(format!("compound canonical: {error:?}")))?;
        codec_wire=codec_wire.max(bytes);codec_indexes=codec_indexes.max(crate::record_biblio_cut::ordered_emit_state(&change)?);
        guard(fields_state.checked_add(changes_state).and_then(|n|n.checked_add(subject_state)).and_then(|n|n.checked_add(empty_state)).and_then(|n|n.checked_add(codec_wire.checked_add(codec_indexes)?)).ok_or(ItemRefusal::Budget)?)?;
        changes.push(change);
    }
    drop(empty);
    // apply retains one successor clone plus its canonical predecessor parse;
    // newly prepared forms are already owned by changes above.
    guard(fields_state.checked_add(changes_state).and_then(|n|n.checked_add(subject_state)).and_then(|n|n.checked_add(prior_codec)).ok_or(ItemRefusal::Budget)?)?;
    let result = kernel::apply_form_changes(previous, &subject, &changes).map_err(fail)?;
    let result_state=crate::record_biblio_cut::ordered_state(&result)?;
    let base=fields_state.checked_add(changes_state).and_then(|n|n.checked_add(subject_state)).and_then(|n|n.checked_add(result_state)).ok_or(ItemRefusal::Budget)?;
    guard(base.checked_add(crate::record_biblio_cut::ordered_emit_state(&result)?).ok_or(ItemRefusal::Budget)?)?;
    let result_bytes=canonical_count_v1(&result,CanonicalProfile::SourceCommandInputV1,limits()).map_err(|error|ItemRefusal::Unsupported(format!("compound canonical: {error:?}")))?;
    codec_wire=codec_wire.max(result_bytes);codec_indexes=codec_indexes.max(crate::record_biblio_cut::ordered_emit_state(&result)?);
    let equality_buffers=codec_wire.checked_add(codec_wire.checked_add(codec_indexes).ok_or(ItemRefusal::Budget)?).ok_or(ItemRefusal::Budget)?;
    let forms=result.object_get("forms").and_then(JsonValue::as_array).map_or(0,|v|v.len());
    let prior=result.object_get("prior_forms").and_then(JsonValue::as_array).map_or(0,|v|v.len());
    // Borrowed history indexes, materializer subject and one field-match Vec.
    let indexes=(forms+prior)*std::mem::size_of::<((&str,u64),&JsonValue)>()+forms*std::mem::size_of::<&str>()+fields.len()*std::mem::size_of::<&kernel::FormField>();
    let inner=subject_state.checked_add(indexes).and_then(|n|n.checked_add(equality_buffers)).ok_or(ItemRefusal::Budget)?;
    guard(base.checked_add(inner).ok_or(ItemRefusal::Budget)?)?;
    let views = kernel::materialize_source_forms_from_fields(source, &result, &fields,available.checked_sub(base).and_then(|n|n.checked_sub(inner)).ok_or(ItemRefusal::Budget)?)?;
    let views_state=views.iter().try_fold(std::mem::size_of::<Vec<JsonValue>>(),|n,v|n.checked_add(crate::record_biblio_cut::ordered_state(v).ok()?)).ok_or(ItemRefusal::Budget)?;
    guard(base.checked_add(views_state).ok_or(ItemRefusal::Budget)?)?;
    if !views
        .iter()
        .all(|v| v.object_get("state").and_then(JsonValue::as_str) == Some("ready"))
        || !views.iter().any(|v| {
            v.object_get("role").and_then(JsonValue::as_str)
                == Some(if claim { "statement" } else { "name" })
        })
    {
        return Err(bad("ready source copies require name/statement"));
    }
    let refs = changes
        .iter()
        .map(|c| {
            kernel::form_reference(
                c.object_get("form")
                    .ok_or_else(|| bad("prepared form missing"))?,
            )
            .map_err(fail)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let refs=JsonValue::Array(refs);
    guard(base.checked_add(views_state).and_then(|n|n.checked_add(crate::record_biblio_cut::ordered_state(&refs).ok()?)).ok_or(ItemRefusal::Budget)?)?;
    Ok((result, refs))
}

fn slice_rows_state(rows:&[(String,&[u8])])->Result<usize,ItemRefusal> {
    rows.iter().try_fold(std::mem::size_of::<Vec<(String,&[u8])>>(),|sum,(name,_)|sum.checked_add(std::mem::size_of::<(String,&[u8])>())?.checked_add(name.len())).ok_or(ItemRefusal::Budget)
}
fn rows_state(rows:&[(String,Vec<u8>)], payloads:bool)->Result<usize,ItemRefusal> {
    rows.iter().try_fold(std::mem::size_of::<Vec<(String,Vec<u8>)>>(),|sum,(name,raw)|sum.checked_add(std::mem::size_of::<(String,Vec<u8>)>())?.checked_add(name.len())?.checked_add(if payloads {raw.len()} else {0})).ok_or(ItemRefusal::Budget)
}
fn package_state(files:&Package)->Result<usize,ItemRefusal> {
    files.iter().try_fold(std::mem::size_of::<Package>(),|sum,(name,raw)|sum.checked_add(std::mem::size_of::<(String,Vec<u8>)>())?.checked_add(name.len())?.checked_add(raw.len())).ok_or(ItemRefusal::Budget)
}
struct Reconstructed {
    scope: Value,
    request: Value,
    parent_receipt: Value,
    child: Package,
    receipt: Value,
}
impl NativeCompoundReader<'_> {
    fn reconstruct(&mut self,tx:&Transaction,kind:CompoundKind,schemas:&mut CutWorkerSchemaExecutor)->Result<Reconstructed,ItemRefusal> {
        let before=self.temporary_state;
        let result=self.reconstruct_inner(tx,kind,schemas);
        self.release_temporary_since(before);
        if let Ok(value)=&result {
            let mut amount=std::mem::size_of::<Reconstructed>();
            for tree in [&value.scope,&value.request,&value.parent_receipt,&value.receipt] {
                amount=amount.checked_add(crate::record_biblio_cut::decoded_state(tree)?.checked_sub(std::mem::size_of::<Value>()).ok_or(ItemRefusal::Budget)?).ok_or(ItemRefusal::Budget)?;
            }
            amount=amount.checked_add(package_state(&value.child)?.checked_sub(std::mem::size_of::<Package>()).ok_or(ItemRefusal::Budget)?).ok_or(ItemRefusal::Budget)?;
            self.temporary(amount)?;
        }
        result
    }
    fn reconstruct_inner(
        &mut self,
        tx: &Transaction,
        kind: CompoundKind,
        schemas: &mut CutWorkerSchemaExecutor,
    ) -> Result<Reconstructed, ItemRefusal> {
        let plan = &tx.manifest["plan"];
        let authority = &plan["authorization"];
        keys(
            authority,
            &[
                "schema_version",
                "scope",
                "principal_id",
                "maker_type",
                "authority_ref",
                "owner_configuration",
                "command_id",
                "request_digest",
                "dependency_bindings",
            ],
        )?;
        if text(authority, "schema_version")? != kind.authorization_schema() {
            return Err(bad("native bibliographic authorization profile"));
        }
        let scope = &authority["scope"];
        let work_path = text(scope, kind.parent_path())?;
        let expression_path = text(scope, kind.child_path())?;
        let home = kind.publication_home(scope)?;
        let work_home = parent(work_path)?;
        // Charge the representations this phase actually owns. No allowance
        // for future forms/events competes with the local Claim constructor.
        let after = |name: &str| {
            tx.files
                .get(&format!("{home}/{name}"))
                .and_then(|v| v.1.as_ref())
                .cloned()
                .ok_or_else(|| bad("missing compound after buffer"))
        };
        let request_raw = self.buffer(after("source-create-request.json")?)?;
        let request = self.decoded(&request_raw)?;
        let request_observation=request_valid_with(&request, kind,&mut |value|self.canonical_observation(value))?;
        self.temporary(std::mem::size_of::<String>()+request_observation.0.len())?;
        let scope_scratch=if kind.relation_attachment() {
            let evidence=array(scope,"allowed_evidence_refs")?;let provenance=if kind==CompoundKind::CollectionWork {array(scope,"retained_membership_provenance_refs")?.len()}else{0};
            let grants=array(scope,kind.parent_form_grant())?.len()+array(scope,"allowed_claim_form_ids")?.len();
            let selections=array(&request,"forms")?.len().max(array(&request,"claim_forms")?.len());
            // Collection evidence/provenance indexes are dropped before the
            // form pass. Form seen+current grant+selected indexes coexist;
            // Unicode strip is borrowed and allocates no string.
            let current_grant=array(scope,kind.parent_form_grant())?.len().max(array(scope,"allowed_claim_form_ids")?.len());
            3*std::mem::size_of::<BTreeSet<&str>>()+evidence.len().max(provenance).max(grants+current_grant+selections)*std::mem::size_of::<&str>()
        }else{0};
        self.temporary(scope_scratch)?;
        scope_valid(scope, &request, authority, kind)?;
        self.release_temporary(scope_scratch);
        let environment_raw = self.buffer(after("source-create-environment.json")?)?;
        let environment = self.decoded(&environment_raw)?;
        keys(
            &environment,
            &[
                "runtime",
                "runtime_version",
                "runtime_artifact_sha256",
                "backend",
                "hardware_target",
                "unicode_version",
                "argv_sha256",
            ],
        )?;
        for value in environment.as_object().unwrap().values() {
            if value.as_str().is_none_or(str::is_empty) {
                return Err(bad("retained environment fields"));
            }
        }
        for k in ["runtime_artifact_sha256", "argv_sha256"] {
            hash(&format!("sha256:{}", text(&environment, k)?))?;
        }
        let receipt_raw = self.buffer(after(kind.receipt_file())?)?;
        let actual_receipt = self.decoded(&receipt_raw)?;
        let recorded_at = text(&actual_receipt, "recorded_at")?;
        crate::retirement_rules::observed_instant_order(recorded_at, recorded_at)
            .map_err(|_| bad("compound recorded aware instant"))?;
        let mut before = Package::new();
        for name in selected_names(work_path)? {
            if let Some(raw) = tx
                .files
                .get(&format!("{work_home}/{name}"))
                .and_then(|s| s.0.clone())
            {
                let raw=self.buffer(raw)?;
                self.temporary(std::mem::size_of::<(String,Vec<u8>)>()+name.len())?;
                before.insert(name, raw);
            }
        }
        let old_raw = before
            .get(kind.parent_file())
            .ok_or_else(|| bad("retained parent input missing"))?;
        let old = self.decoded(old_raw)?;
        if !self.reference_matches(&old,"record_id","record_version",&request["expected_source"])?
            || self.package_revision(&before)? != text(&request, "expected_revision")? {
            return Err(bad("retained authorization/request/before binding"));
        }
        if authority["owner_configuration"] != request["expected_configuration"]
            || authority["command_id"] != request["command_id"]
            || text(authority, "request_digest")? != request_observation.0
            || self.canonical_observation(&authority["dependency_bindings"])?.0
                != text(&request, "expected_dependencies")?
        {
            return Err(bad("retained authorization/request/before binding"));
        }
        let dirs = array(plan, "new_directories")?;
        if kind.relation_attachment() && (dirs.len()!=1||dirs[0].as_str()!=Some(home))
            || !kind.relation_attachment() && !(kind == CompoundKind::EditionItem && dirs.is_empty())
            && *dirs != vec![json!(home)]
            && *dirs != vec![json!(parent(home)?), json!(home)]
        {
            return Err(bad("exact new child directories"));
        }
        let history = self.history(work_path, &before)?;
        if array(&history, "receipts")?.len() >= MAX_HISTORY {
            return Err(bad("parent history capacity"));
        }
        let mut revised = self.value_copy(&old)?;
        let map = revised
            .as_object_mut()
            .ok_or_else(|| bad("parent object"))?;
        for (k, v) in request["fields"].as_object().unwrap() {
            map.insert(k.clone(), v.clone());
        }
        map.insert(
            "record_version".into(),
            json!(
                integer(&old, "record_version")?
                    .checked_add(1)
                    .ok_or(ItemRefusal::Budget)?
            ),
        );
        // Inserts replace old subtrees; charge only positive growth of the
        // retained revised tree, not another full cloned profile.
        let old_state=crate::record_biblio_cut::decoded_state(&old)?;
        let revised_state=crate::record_biblio_cut::decoded_state(&revised)?;
        self.temporary(revised_state.saturating_sub(old_state))?;
        let mut revised_ordered = self.ordered_value(old_raw)?;
        let original_state=crate::record_biblio_cut::ordered_state(&revised_ordered)?;
        set(
            &mut revised_ordered,
            kind.field(),
            j(&request["fields"][kind.field()])?,
        )?;
        set(
            &mut revised_ordered,
            "record_version",
            j(&revised["record_version"])?,
        )?;
        self.temporary(crate::record_biblio_cut::ordered_state(&revised_ordered)?.saturating_sub(original_state))?;
        let expression = &request[kind.record_key()];
        let claim = &request["claim"];
        let ordered_request=self.ordered_value(&request_raw)?;
        let expression_ordered=self.ordered_copy(ordered_request.object_get(kind.record_key()).ok_or_else(||bad("ordered child record"))?)?;
        let claim_ordered=self.ordered_copy(ordered_request.object_get("claim").ok_or_else(||bad("ordered Claim"))?)?;
        let request_raw_state=std::mem::size_of::<Vec<u8>>()+request_raw.len();
        drop(request_raw);
        self.release_temporary(request_raw_state);
        let environment_raw_state=std::mem::size_of::<Vec<u8>>()+environment_raw.len();
        drop(environment_raw);
        self.release_temporary(environment_raw_state);
        let parent_raw = self.buffer(pretty(&revised_ordered)?)?;
        let expression_raw = self.buffer(pretty(&expression_ordered)?)?;
        let mut claim_raw = canonical_ordered(&claim_ordered)?;
        claim_raw.push(b'\n');
        let claim_raw=self.buffer(claim_raw)?;
        let mut delta_limits = self.limits;
        delta_limits.max_state_bytes = self
            .limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        delta_limits.max_total_bytes = self
            .limits
            .max_total_bytes
            .checked_sub(self.bytes)
            .ok_or(ItemRefusal::Budget)?;
        let delta = crate::biblio_rules::inspect_bibliographic_delta(
            crate::biblio_rules::BiblioDeltaInput {
                parent_path: work_path,
                parent_before_raw: old_raw,
                parent_after_raw: &parent_raw,
                endpoint_path: expression_path,
                endpoint_raw: &expression_raw,
                claim_path: &format!("{home}/source-claims.jsonl"),
                claim_raw: &claim_raw,
            },
            delta_limits,
            self.cancelled,
            schemas,
        )?;
        if !delta.issues.is_empty() {
            return Err(bad("native bibliographic append/delta mechanics"));
        }
        let delta_reads_state=delta.reads.iter().try_fold(0usize,|n,r|n.checked_add(crate::record_biblio_cut::predicate_state(r).ok()?)).ok_or(ItemRefusal::Budget)?;
        self.temporary(delta_reads_state)?;
        for read in delta.reads {
            self.release_temporary(crate::record_biblio_cut::predicate_state(&read)?);
            self.record_read(read)?;
        }
        if kind == CompoundKind::WorkExpression
            && (!text(expression, "language").is_ok_and(|v| !v.is_empty())
                || !text(expression, "expression_role").is_ok_and(|v| !v.is_empty()))
            || !kind.relation_attachment() && ["variant_labels", "external_identifiers"].iter().any(|k| {
                !expression[*k]
                    .as_array()
                    .is_some_and(|a| a.iter().all(|v| v["status"] == "unverified"))
            })
        {
            return Err(bad("child language/role/unverified variants"));
        }
        let mut local_limits = self.limits;
        local_limits.max_state_bytes = self
            .limits
            .max_state_bytes
            .checked_sub(self.state)
            .ok_or(ItemRefusal::Budget)?;
        local_limits.max_total_bytes = self
            .limits
            .max_total_bytes
            .checked_sub(self.bytes)
            .ok_or(ItemRefusal::Budget)?;
        let mut local = crate::record_rules::validate_source_claim_from_cut(
            self.cut,
            &claim_raw,
            schemas,
            local_limits,
            self.cancelled,
        )?;
        if !local.issues.is_empty() {
            return Err(bad("compound Claim local owner profile"));
        }
        let dependency_digests = std::mem::take(&mut local.dependency_digests);
        drop(local);
        let dependencies_state=dependency_digests.keys().try_fold(0usize,|n,path|n.checked_add(std::mem::size_of::<(String,Digest256)>()+path.len())).ok_or(ItemRefusal::Budget)?;
        self.temporary(dependencies_state)?;
        for (path, sha) in dependency_digests {
            let relative = RelativePath::parse(&path).map_err(|_| bad("Claim contract path"))?;
            let size = self
                .cut
                .current()
                .member(&relative)
                .ok_or_else(|| bad("Claim dependency membership"))?
                .size_bytes;
            account(
                &mut self.bytes,
                usize::try_from(size).map_err(|_| ItemRefusal::Budget)?,
                self.limits.max_total_bytes,
            )?;
            self.release_temporary(std::mem::size_of::<(String,Digest256)>()+path.len());
            self.record_read(PredicateRead::ExactPath {path,digest:sha.to_prefixed()})?;
        }
        // The constructor's decoded registries/routes have now been dropped.
        // Acquire the remainder before allocating any generated forms,
        // Item companions, provenance event or whole output maps.
        let form_name = kind.parent_forms();
        let prior_forms = before.get(form_name).map(|v| self.ordered_value(v)).transpose()?;
        let principal = text(authority, "principal_id")?;
        let (parent_forms, parent_refs) = forms(
            &revised_ordered,
            prior_forms.as_ref(),
            &request["forms"],
            principal,
            false,
            self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?,
        )?;
        for value in [&parent_forms,&parent_refs] {self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;}
        let (expression_forms, expression_refs) = if kind.relation_attachment() {(JsonValue::Null,JsonValue::Null)}else{forms(
            &expression_ordered,
            None,
            &request[kind.child_form_request()],
            principal,
            false,
            self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?,
        )?
        };
        for value in [&expression_forms,&expression_refs] {self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;}
        let (claim_forms, claim_refs) = forms(
            &claim_ordered,
            None,
            &request["claim_forms"],
            principal,
            true,
            self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?,
        )?;
        for value in [&claim_forms,&claim_refs] {
            self.temporary(crate::record_biblio_cut::ordered_state(value)?)?;
        }
        let id = transaction_id_with_digest(&request, kind,&request_observation.0,&mut |value|self.canonical_observation(value))?;
        if id != tx.manifest["transaction_id"] {
            return Err(bad("compound transaction request identity"));
        }
        let archive_path = format!(
            "{HOME}/.record-revisions/{}-{}",
            Digest256::of_bytes(text(scope, kind.parent_key())?.as_bytes()).to_hex(),
            hash(text(&request, "expected_revision")?)?
        );
        let parent_receipt_ordered = object(vec![
            ("command_id", j(&request["command_id"])?),
            ("request_digest", string(&request_observation.0)),
            ("principal_id", j(&authority["principal_id"])?),
            ("authority_ref", j(&authority["authority_ref"])?),
            (
                "owner_configuration",
                j(&request["expected_configuration"])?,
            ),
            ("recorded_at", string(recorded_at)),
            ("reason", j(&request["reason"])?),
            (
                "previous_source",
                ref_ordered(&old, "record_id", "record_version")?,
            ),
            (
                "source",
                ref_ordered(&revised, "record_id", "record_version")?,
            ),
            ("previous_revision", j(&request["expected_revision"])?),
            ("archive_path", string(&archive_path)),
            ("dependencies", j(&request["expected_dependencies"])?),
            ("changed_fields", j(&json!([kind.field()]))?),
            ("forms", parent_refs.clone()),
            ("grants_admission", JsonValue::Bool(false)),
            ("request", self.ordered_copy(&ordered_request)?),
            (
                "publication",
                object(vec![
                    ("protocol", string(PROTOCOL)),
                    ("transaction_id", string(&id)),
                    (
                        "selected_files",
                        j(&{
                            let mut names = selected_names(work_path)?;
                            names.sort();
                            json!(names)
                        })?,
                    ),
                ]),
            ),
        ]);
        self.temporary(crate::record_biblio_cut::ordered_state(&parent_receipt_ordered)?)?;
        let parent_receipt_raw=self.buffer(canonical_ordered(&parent_receipt_ordered)?)?;
        let parent_receipt = self.decoded(&parent_receipt_raw)?;
        parent_receipt_shape_with(&parent_receipt, kind,&mut |value|self.canonical_observation(value))?;
        let (mut receipts,mut receipt_array_state)=match before.get(HISTORY) {
            Some(raw)=>{
                let prior=self.ordered_value(raw)?;
                let prior_state=crate::record_biblio_cut::ordered_state(&prior)?;
                let rows=prior.object_get("receipts").and_then(JsonValue::as_array).ok_or_else(||bad("ordered receipt chain"))?;
                let cost=rows.iter().try_fold(std::mem::size_of::<Vec<JsonValue>>(),|n,row|n.checked_add(crate::record_biblio_cut::ordered_state(row).ok()?)).ok_or(ItemRefusal::Budget)?;
                self.temporary(cost)?;let rows=rows.to_vec();
                drop(prior);self.release_temporary(prior_state);(rows,cost)
            },
            None=>{let cost=std::mem::size_of::<Vec<JsonValue>>();self.temporary(cost)?;(vec![],cost)},
        };
        let new_receipt_state=crate::record_biblio_cut::ordered_state(&parent_receipt_ordered)?;
        self.temporary(new_receipt_state)?;receipt_array_state=receipt_array_state.checked_add(new_receipt_state).ok_or(ItemRefusal::Budget)?;
        receipts.push(parent_receipt_ordered.clone());
        // Maintained _compose creates this outer dict afresh in fixed order;
        // retained receipt object order, but not prior outer order, survives.
        let history_ordered = object(vec![
            ("schema_version", string("tos_source_revision_history_v2")),
            ("record_id", j(&scope[kind.parent_key()])?),
            ("receipts", JsonValue::Array(receipts)),
        ]);
        self.release_temporary(receipt_array_state);
        self.temporary(crate::record_biblio_cut::ordered_state(&history_ordered)?)?;
        let parent_files: Vec<(String, Vec<u8>)> = vec![
            (kind.parent_file().into(), parent_raw),
            (form_name.into(), self.buffer(pretty(&parent_forms)?)?),
            (HISTORY.into(), self.buffer(pretty(&history_ordered)?)?),
        ];
        let claim_form_name = format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(text(scope, "claim_id")?.as_bytes()).to_hex()
        );
        let mut child_files: Vec<(String, Vec<u8>)> = vec![
            ("source-claims.jsonl".into(), claim_raw),
            (claim_form_name, self.buffer(pretty(&claim_forms)?)?),
        ];
        if !kind.relation_attachment() {
            child_files.insert(0,(kind.child_file().into(),expression_raw));
            child_files.insert(1,(kind.child_forms().into(),self.buffer(pretty(&expression_forms)?)?));
        }else{let cost=std::mem::size_of::<Vec<u8>>()+expression_raw.len();drop(expression_raw);self.release_temporary(cost);}
        self.temporary(rows_state(&parent_files,false)?)?;
        self.temporary(rows_state(&child_files,false)?)?;
        if kind == CompoundKind::EditionItem {
            let byte_receipt_raw = self.buffer(after("item-deposit-receipt.json")?)?;
            let byte_receipt = self.decoded(&byte_receipt_raw)?;
            let inventory_raw=self.buffer(after("resource-inventory.json")?)?;
            let inventory = self.decoded(&inventory_raw)?;
            let mut companion_limits=self.limits;
            companion_limits.max_state_bytes=self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?;
            let companions=item_companions(
                scope,
                &request,
                &byte_receipt,
                text(&inventory["generator"], "version")?,
                schemas,
                companion_limits,
                self.cancelled,
            )?;
            self.temporary(rows_state(&companions,true)?)?;
            child_files.extend(companions);
            child_files.push((
                "item-deposit-receipt.json".into(),
                pretty(&ordered(&byte_receipt_raw)?)?,
            ));
        }
        let outputs: Vec<_> = parent_files
            .iter()
            .map(|(n, r)| (format!("{work_home}/{n}"), r.as_slice()))
            .chain(
                child_files
                    .iter()
                    .map(|(n, r)| (format!("{home}/{n}"), r.as_slice())),
            )
            .collect();
        self.temporary(slice_rows_state(&outputs)?)?;
        let event = compound_event(
            kind,
            scope,
            &request,
            &before,
            &outputs,
            &environment,
            &authority["dependency_bindings"],
            recorded_at,
            self.limits.max_state_bytes.checked_sub(self.state).ok_or(ItemRefusal::Budget)?,
        )?;
        let outputs_state=slice_rows_state(&outputs)?;
        drop(outputs);
        self.release_temporary(outputs_state);
        self.temporary(crate::record_biblio_cut::decoded_state(&event)?)?;
        let mut event_raw = canonical(&event)?;
        event_raw.push(b'\n');
        let event_raw=self.buffer(event_raw)?;
        if !schemas.check_reusing_scalar(
            &format!("{home}/source-create-provenance.jsonl"),
            &event_raw,
            "ToS/contracts/provenance-event-v2.schema.json",
            self.limits.deadline,
            self.cancelled,
        )? {
            return Err(bad("reconstructed provenance schema"));
        }
        for raw in [&parent_forms, &expression_forms, &claim_forms]
            .into_iter()
            .filter(|raw| !matches!(raw, JsonValue::Null))
        {
            if !schemas.check_reusing_scalar(
                "compound-reconstructed-human-form-set",
                &canonical_ordered(raw)?,
                "ToS/contracts/human-form-set.schema.json",
                self.limits.deadline,
                self.cancelled,
            )? {
                return Err(bad("compound forms schema"));
            }
        }
        child_files.extend([
            ("source-create-request.json".into(), {
                let mut r = canonical(&request)?;
                r.push(b'\n');
                self.buffer(r)?
            }),
            ("source-create-environment.json".into(), {
                let mut r = canonical(&environment)?;
                r.push(b'\n');
                self.buffer(r)?
            }),
            ("source-create-provenance.jsonl".into(), event_raw),
        ]);
        let files: Vec<_> = parent_files
            .iter()
            .map(|(n, r)| (format!("{work_home}/{n}"), r.as_slice()))
            .chain(
                child_files
                    .iter()
                    .map(|(n, r)| (format!("{home}/{n}"), r.as_slice())),
            )
            .collect();
        self.temporary(slice_rows_state(&files)?)?;
        let before_refs: Vec<_> = selected_names(work_path)?
            .iter()
            .filter_map(|n| before.get(n).map(|r| (n.clone(), r.as_slice())))
            .collect();
        self.temporary(slice_rows_state(&before_refs)?)?;
        let mut receipt_fields = vec![
            ("schema_version", string(kind.receipt_schema())),
            ("operation", string(kind.operation())),
            ("transaction_id", string(&id)),
            ("command_id", j(&request["command_id"])?),
            ("request_digest", string(&request_observation.0)),
            ("principal_id", j(&authority["principal_id"])?),
            ("authority_ref", j(&authority["authority_ref"])?),
            (
                "owner_configuration",
                j(&request["expected_configuration"])?,
            ),
            ("recorded_at", string(recorded_at)),
            ("scope", j(scope)?),
            ("dependencies", j(&request["expected_dependencies"])?),
            (
                "parent_before",
                ref_ordered(&old, "record_id", "record_version")?,
            ),
            (
                "parent_after",
                ref_ordered(&revised, "record_id", "record_version")?,
            ),
            ("parent_revision", j(&request["expected_revision"])?),
            ("parent_archive_ref", string(&archive_path)),
            (
                "parent_transition_sha256",
                string(
                    &Digest256::of_bytes(&canonical_ordered(&parent_receipt_ordered)?)
                        .to_prefixed(),
                ),
            ),
            ("parent_before_files", refs_ordered(&before_refs)),
            (
                kind.child_kind(),
                ref_ordered(expression, "record_id", "record_version")?,
            ),
            ("claim", ref_ordered(claim, "claim_id", "claim_version")?),
            (
                "forms",
                object(vec![
                    (kind.parent_kind(), parent_refs),
                    ("claim", claim_refs),
                ]),
            ),
            ("files", refs_ordered(&files)),
            ("grants_admission", JsonValue::Bool(false)),
        ];
        let mut binding_state=0;
        if !kind.relation_attachment() {
            let forms=receipt_fields.iter_mut().find(|(key,_)|*key=="forms").unwrap();
            if let JsonValue::Object(ref mut fields)=forms.1 {fields.insert(1,(tos_foundation::JsonString::from_utf8(kind.child_kind()),expression_refs));}
        }else{
            let binding=self.attachment_endpoint_binding(scope,expression,&authority["dependency_bindings"],kind)?;
            binding_state=crate::record_biblio_cut::ordered_state(&binding)?;
            let claim_position=receipt_fields.iter().position(|(key,_)|*key=="claim").unwrap();
            receipt_fields.insert(claim_position,(if kind==CompoundKind::CollectionWork {"work_source_binding"}else{"agent_source_binding"},binding));
        }
        let receipt_ordered=object(receipt_fields);
        let rows_state=slice_rows_state(&files)?.checked_add(slice_rows_state(&before_refs)?).ok_or(ItemRefusal::Budget)?;
        drop(files);drop(before_refs);
        self.release_temporary(rows_state);
        // The binding moved into this receipt; transfer its existing charge,
        // rather than counting a second retained tree for the same value.
        self.release_temporary(binding_state);
        self.temporary(crate::record_biblio_cut::ordered_state(&receipt_ordered)?)?;
        let expected_raw = self.buffer(pretty(&receipt_ordered)?)?;
        let receipt = self.decoded(&expected_raw)?;
        if actual_receipt != receipt || receipt_raw != expected_raw {
            return Err(bad("exact reconstructed compound receipt bytes"));
        }
        child_files.push((kind.receipt_file().into(), expected_raw));
        if parent_files
            .iter()
            .chain(child_files.iter())
            .any(|(_, raw)| raw.len() > MAX_FILE)
        {
            return Err(ItemRefusal::Budget);
        }
        let expected:BTreeMap<_,_>=parent_files.iter().map(|(name,raw)|(format!("{work_home}/{name}"),(before.get(name).map(Vec::as_slice),Some(raw.as_slice()))))
            .chain(child_files.iter().map(|(name,raw)|(format!("{home}/{name}"),(None,Some(raw.as_slice()))))).collect();
        let expected_state=expected.keys().try_fold(0usize,|sum,path|sum.checked_add(std::mem::size_of::<(String,(Option<&[u8]>,Option<&[u8]>))>()+path.len())).ok_or(ItemRefusal::Budget)?;
        self.temporary(expected_state)?;
        if tx.files.len()!=expected.len() || expected.iter().any(|(path,(before,after))|tx.files.get(path).is_none_or(|(old,new)|old.as_deref()!=*before || new.as_deref()!=*after)) {
            return Err(bad("exact whole retained before/after plan"));
        }
        drop(expected);
        self.release_temporary(expected_state);
        if self.archive(work_path, text(scope, kind.parent_key())?, &parent_receipt)? != before {
            return Err(bad("parent archive versus transaction inputs"));
        }
        Ok(Reconstructed {
            scope: self.value_copy(scope)?,
            request,
            parent_receipt,
            child: child_files.into_iter().collect(),
            receipt,
        })
    }
}

fn compound_event(
    kind: CompoundKind,
    scope: &Value,
    request: &Value,
    before: &Package,
    outputs: &[(String, &[u8])],
    environment: &Value,
    dependencies: &Value,
    recorded_at: &str,
    available:usize,
) -> Result<Value, ItemRefusal> {
    let module = kind.module();
    let home = kind.publication_home(scope)?;
    let request_ref = format!("{home}/source-create-request.json");
    let environment_ref = format!("{home}/source-create-environment.json");
    let mut request_raw = canonical(request)?;
    request_raw.push(b'\n');
    let mut environment_raw = canonical(environment)?;
    environment_raw.push(b'\n');
    let archive = format!(
        "{HOME}/.record-revisions/{}-{}",
        Digest256::of_bytes(text(scope, kind.parent_key())?.as_bytes()).to_hex(),
        hash(text(request, "expected_revision")?)?
    );
    let prior: BTreeMap<_, _> = before
        .values()
        .map(|raw| {
            (
                format!("{archive}/{}.blob", Digest256::of_bytes(raw).to_hex()),
                raw,
            )
        })
        .collect();
    let output: BTreeMap<_, _> = outputs.iter().map(|(p, r)| (p, r)).collect();
    let entity = |reference: &str, raw: &[u8], role: &str| json!({"entity_ref":reference,"role":role,"sha256":Digest256::of_bytes(raw).to_hex(),"size_bytes":raw.len(),"media_type":if reference.ends_with(".jsonl"){"application/x-ndjson"}else if kind==CompoundKind::EditionItem && reference.ends_with("/forensic-report.md"){"text/markdown"}else if kind==CompoundKind::EditionItem && reference.ends_with("/fixity.sha256"){"text/plain"}else{"application/json"},"availability":"owner_local","content_disclosure":"public_metadata_only","fixity_verified":false,"fixity_verified_at":null});
    let mut inputs = vec![entity(
        &request_ref,
        &request_raw,
        "caller-supplied-metadata-request",
    )];
    inputs.extend(
        prior
            .iter()
            .map(|(p, r)| entity(p, r, "retained-parent-metadata-input")),
    );
    let script = text(&dependencies["implementation"], module)?;
    hash(&format!("sha256:{script}"))?;
    let mut env = environment.clone();
    env.as_object_mut().unwrap().remove("argv_sha256");
    env.as_object_mut().unwrap().insert(
        "environment_profile_binding".into(),
        json!({"ref":environment_ref,"sha256":Digest256::of_bytes(&environment_raw).to_hex()}),
    );
    let derivation =
        text(scope, "provenance_event_id")?.replacen("tos.event.", "tos.derivation.", 1);
    let output_bytes = output
        .values()
        .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
        .ok_or(ItemRefusal::Budget)?;
    let mut scratch=request_raw.len().checked_add(environment_raw.len()).and_then(|n|n.checked_add(crate::record_biblio_cut::decoded_state(&env).ok()?)).ok_or(ItemRefusal::Budget)?;
    scratch=scratch.checked_add(inputs.iter().try_fold(0usize,|n,v|n.checked_add(crate::record_biblio_cut::decoded_state(v).ok()?)).ok_or(ItemRefusal::Budget)?).ok_or(ItemRefusal::Budget)?;
    scratch=scratch.checked_add(prior.keys().try_fold(0usize,|n,p|n.checked_add(std::mem::size_of::<(String,&Vec<u8>)>()+p.len())).ok_or(ItemRefusal::Budget)?).and_then(|n|n.checked_add(output.len()*std::mem::size_of::<(&String,&&[u8])>())).ok_or(ItemRefusal::Budget)?;
    let result=json!({
        "$schema":"https://tree-of-sophia.local/ToS/contracts/provenance-event-v2.schema.json","schema_version":"tos_provenance_event_v2","event_id":scope["provenance_event_id"],"event_version":1,"supersedes_event_ref":null,
        "record_binding":{"manifest_ref":format!("{home}/{}",kind.receipt_file()),"digest_algorithm":"sha256","digest_scope":"exact_event_record_bytes"},
        "activity":{"event_type":"annotation","started_at":recorded_at,"ended_at":recorded_at,"status":"completed_with_warnings","terminal_reason":null,"exit_code":0,"warnings":["Captured prepared metadata buffers; the committed transaction is a separate verification.",if kind==CompoundKind::ExpressionResponsibility {"A qualified attribution is supplied by the caller; serialization and URL presence do not prove source reading or its truth."}else if kind==CompoundKind::CollectionWork {"A qualified membership account is supplied by the caller; serialization and URL presence do not prove source reading or its truth."}else{"Observed denotes the declared record link, not accepted bibliographic or textual truth."}]},
        "entities":{"inputs":inputs,"outputs":output.iter().map(|(p,r)|entity(p,r,"prepared-compound-source-metadata")).collect::<Vec<_>>(),"byproducts":[entity(&environment_ref,&environment_raw,"runtime-description")]},
        "derivations":output.keys().enumerate().map(|(index,p)|json!({"derivation_id":format!("{derivation}.output-{index}"),"input_entity_ref":request_ref,"output_entity_ref":p,"relation":"was_derived_from","influence_asserted":true,"description":"Technical source metadata serialization; no historical influence or textual identity is asserted."})).collect::<Vec<_>>(),
        "responsibility":[{"agent_ref":kind.executor(),"agent_kind":"software","role":"executor","responsibility_posture":"performed","evidence_binding":{"ref":module,"sha256":script},"human_evidence_status":"not_applicable"}],
        "method":{"procedure":{"name":kind.procedure(),"version":"1","purpose":if kind==CompoundKind::ExpressionResponsibility {"Serialize one qualified translator Claim and an Expression responsibility reference without judging attribution."}else if kind==CompoundKind::CollectionWork {"Serialize one qualified membership Claim and a Collection membership reference without judging membership."}else{"Serialize one declared parent link and explicit source-copy forms without judging their content."}},"command_capture":{"disclosure":"withheld_digest_only","argv":null,"argv_sha256":environment["argv_sha256"],"withholding_reason":"Process arguments may contain a private owner-configuration path."},"configuration_binding":{"ref":request_ref,"sha256":Digest256::of_bytes(&request_raw).to_hex()},"software_components":[{"name":kind.component(),"version":"1","role":"serialization-runner","artifact_ref":module,"artifact_sha256":script,"verification_status":"verified"}],"model_invocations":[],"environment":env},
        "manual_changes":{"status":"none_declared","change_receipts":[],"statement":"Caller authorship precedes this operation; no manual edits are performed inside serialization."},
        "measurements":[{"metric":"output_bytes","status":"measured","value":output_bytes,"unit":"bytes","method":"Sum of prepared source record, form and parent history buffers; excludes capture and receipt.","evidence_binding":null}],
        "evidence_authentication":{"capture_posture":"tool_captured","signature_status":"unsigned","signature_bindings":[],"verification_status":"unverified","producer_control_boundary":"The same unsigned local process serializes and records; hashes do not authenticate execution truth."},
        "rights_and_visibility":{"rights_record_bindings":[],"intended_uses":["local_research","public_metadata"],"content_visibility":"tracked_public_metadata","publication_authorized":false,"publication_authority_bindings":[]},
        "review_and_authority":{"mechanical_validation":"not_run","human_review_status":"not_performed","review_bindings":[],"accepted_uses":[],"promotion_authorized":false,"competence_evidence_bindings":[]},
        "reproducibility":{"classification":"partially_specified","known_gaps":["Upstream research, source reading and model invocations are outside this operation.","Runtime metadata is captured, not a complete archived execution environment."],"replay_scope":"Exact retained request, metadata and source-copy buffer construction; not bibliographic truth."},
        "authority_boundary":{"validator_role":"mechanics_and_closure_only_not_truth","claims_not_established":["execution_truth","content_truth","source_fidelity","translation_quality","semantic_correctness","rights_clearance","human_review","publication_authority","canon_authority"]}
    });
    let used=scratch.checked_add(crate::record_biblio_cut::decoded_state(&result)?).ok_or(ItemRefusal::Budget)?;
    if used>available {return Err(ItemRefusal::BudgetCheck{check:"compound provenance logical workspace",used:Some(used as u64),limit:Some(available as u64)});}
    Ok(result)
}

impl NativeCompoundReader<'_> {
    pub(crate) fn verify(
        &mut self,
        path: &str,
        claim: &Value,
        schemas: &mut CutWorkerSchemaExecutor,
    ) -> Result<NativeCompoundObservation, ItemRefusal> {
        let before = self.temporary_state;
        let result = self.verify_inner(path, claim, schemas);
        self.release_temporary_since(before);
        self.release_raw_cache();
        result
    }
    fn verify_inner(
        &mut self,
        path: &str,
        claim: &Value,
        schemas: &mut CutWorkerSchemaExecutor,
    ) -> Result<NativeCompoundObservation, ItemRefusal> {
        check(self.limits.deadline, self.cancelled)?;
        let kind = CompoundKind::from_predicate(text(claim, "predicate")?)?;
        metadata_path(path, false)?;
        if !path.ends_with("/source-claims.jsonl") {
            return Err(bad("native compound Claim carrier"));
        }
        let home = parent(path)?;
        let receipt_raw = self.required(&format!("{home}/{}", kind.receipt_file()), MAX_FILE)?;
        let receipt = self.decoded(&receipt_raw)?;
        if text(&receipt, "schema_version")? != kind.receipt_schema() {
            return Err(bad("native Claim compound receipt"));
        }
        let id = text(&receipt, "transaction_id")?;
        let tx = self.transaction(id)?;
        let transport = match tx.status.as_str() {
            "committed" => NativeTransportState::Committed,
            "rolled-back" => NativeTransportState::RolledBack,
            "pending" => NativeTransportState::Pending,
            "orphan" => NativeTransportState::Orphan,
            _ => return Err(bad("transaction outcome")),
        };
        let observation = NativeCompoundObservation {
            claim_path: path.into(),
            claim_id: text(claim, "claim_id")?.into(),
            transaction_id: id.into(),
            manifest_sha256: tx.manifest_sha256.clone(),
            transport,
        };
        if transport != NativeTransportState::Committed {
            return Ok(observation);
        }
        if self
            .publication
            .as_ref()
            .is_some_and(|p| p["phase"] != "ready")
        {
            return Err(bad("current source snapshot is pending owner recovery"));
        }
        let reconstructed = self.reconstruct(&tx, kind, schemas)?;
        let scope = &reconstructed.scope;
        let work = text(scope, kind.parent_path())?;
        let expression = text(scope, kind.child_path())?;
        if path != format!("{}/source-claims.jsonl", kind.publication_home(scope)?)
            || !kind.relation_attachment() && claim != &reconstructed.request["claim"]
            || receipt != reconstructed.receipt
            || receipt_raw != reconstructed.child[kind.receipt_file()]
            || !kind.relation_attachment() && self.required(path, MAX_FILE)? != reconstructed.child["source-claims.jsonl"]
        {
            return Err(bad("exact current compound Claim/receipt bytes"));
        }
        for name in [
            "source-create-request.json",
            "source-create-environment.json",
            "source-create-provenance.jsonl",
        ] {
            if self.required(&format!("{home}/{name}"), MAX_FILE)? != reconstructed.child[name] {
                return Err(bad("immutable compound capture changed"));
            }
        }
        if kind == CompoundKind::EditionItem {
            for name in [
                "item-deposit-receipt.json",
                "item.manifest.json",
                "rights.json",
                "provenance.jsonl",
                "resource-inventory.json",
                "fixity.sha256",
                "forensic-report.md",
            ] {
                if self.required(&format!("{home}/{name}"), MAX_FILE)? != reconstructed.child[name]
                {
                    return Err(bad("immutable Item companion bytes changed"));
                }
            }
        }
        let parent_files = self.selected(work)?;
        let parent_record = self.decoded(&parent_files[kind.parent_file()])?;
        if parent_record["record_id"] != scope[kind.parent_key()]
            || parent_record["record_type"] != kind.parent_kind()
            || kind == CompoundKind::ExpressionEdition
                && parent_record["work_ref"] != scope["work_id"]
        {
            return Err(bad("current parent typed identity"));
        }
        let parent_history = self.history(work, &parent_files)?;
        if !array(&parent_history, "receipts")?.contains(&reconstructed.parent_receipt) {
            return Err(bad("compound transition missing in current parent lineage"));
        }
        if kind.relation_attachment() {
            let initial=self.attachment_claim_initial(path,claim)?;
            if initial!=reconstructed.child["source-claims.jsonl"] {return Err(bad("membership exact committed initial stream"));}
            for key in ["statement","statement_language","statement_script",if kind==CompoundKind::CollectionWork {"membership_scope"}else{"attribution_scope"}] {
                let value=text(&claim["qualifiers"],key)?;
                if tos_foundation::python_strip_unicode16_v1(value,MAX_SIDE).map_err(|_|ItemRefusal::Budget)?.is_empty() {return Err(bad("membership current qualified wording"));}
            }
            check(self.limits.deadline,self.cancelled)?;
            return Ok(observation);
        }
        let child_files = self.selected(expression)?;
        let child_record = self.decoded(&child_files[kind.child_file()])?;
        if child_record["record_id"] != scope[kind.child_key()]
            || child_record["record_type"] != kind.child_kind()
            || !kind.initial_backlink(&child_record, &scope[kind.parent_key()])
            || kind == CompoundKind::EditionItem
                && child_record["item_manifest_ref"]
                    != format!("{}/item.manifest.json", parent(expression)?)
        {
            return Err(bad("current compound child typed parent binding"));
        }
        let child_history = self.history(expression, &child_files)?;
        let mut initial = child_files[kind.child_file()] == reconstructed.child[kind.child_file()];
        for receipt in array(&child_history, "receipts")? {
            check(self.limits.deadline, self.cancelled)?;
            let before_archive=self.temporary_state;
            let archive = self.archive(expression, text(scope, kind.child_key())?, receipt)?;
            if receipt["previous_source"] == reconstructed.receipt[kind.child_kind()] {
                if archive[kind.child_file()] != reconstructed.child[kind.child_file()] {
                    return Err(bad("compound child initial archive bytes changed"));
                }
                initial = true;
            }
            drop(archive);
            self.release_temporary_since(before_archive);
        }
        if !initial {
            return Err(bad(
                "current compound child lacks committed initial lineage",
            ));
        }
        check(self.limits.deadline, self.cancelled)?;
        Ok(observation)
    }
}
