//! Record-only custom owner composition. Provider bytes are observations, not grants.
//! The platform retains actual owner objects; only this kernel mints wire handles.
//! Callbacks happen between fully joined native phases, never inside a child owner.
use crate::source_claim_publication_bytes as bytes;
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_read_contract as wire;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tos_compiler::prepared_source_binding::PreparedSourceInputs;
use tos_foundation::Digest256;

pub const STATE_BYTES: usize = 4 * 1024 * 1024;
pub const PHASE_INPUT_BYTES: usize = wire::RESPONSE_BYTES + 4096;
pub const PHASE_OUTPUT_BYTES: usize = wire::RESPONSE_BYTES + 4096;
const WORK_BYTES: u64 = 128 * 1024 * 1024;
const PHASE_WORK: u64 = 16 * 1024 * 1024;
const MAX_PHASES: u64 = 8;
const STATE_SCHEMA: &str = "tos_source_owner_continuation_v1";
const RESPONSE_SCHEMA: &str = "tos_source_owner_provider_response_v1";

fn invalid() -> Error {
    Error::Invalid("source owner phase framing")
}
fn budget() -> Error {
    Error::Unsupported("source owner cumulative budget exceeded")
}
fn parse(raw: &[u8], cap: usize) -> Result<Value> {
    if raw.len() > cap {
        return Err(budget());
    }
    bytes::parse(raw, cap).map_err(|_| invalid())
}
fn encode(value: &Value, cap: usize) -> Result<Vec<u8>> {
    wire::canonical(value, cap)
}
fn bare(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn exact(value: &Value, keys: &[&str]) -> Result<()> {
    wire::exact(value, keys)
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    wire::text(value, key)
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Limits {
    max_handle_bytes: usize,
    max_request_bytes: usize,
    max_record_bytes: usize,
    max_response_bytes: usize,
}
impl Limits {
    fn validate(&self) -> Result<()> {
        if self.max_handle_bytes > wire::HANDLE_BYTES
            || self.max_request_bytes > wire::REQUEST_BYTES
            || self.max_record_bytes > wire::RECORD_BYTES
            || self.max_response_bytes > wire::RESPONSE_BYTES
        {
            return Err(budget());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Role {
    role: String,
    identity: String,
    epoch: Value,
    custody: Value,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Binding {
    binding_kind: String,
    roles: Vec<Role>,
    metadata_record_types: Vec<String>,
    source_inputs_sha256: Option<String>,
}
impl Binding {
    fn validate(&self) -> Result<Value> {
        if !["owner-issued-reader", "prepared-source-vector"].contains(&self.binding_kind.as_str())
            || self.roles.is_empty()
            || self.roles.len() > 5
        {
            return Err(invalid());
        }
        let mut roles = BTreeSet::new();
        for r in &self.roles {
            if !["metadata", "claim", "slot", "issuer", "authored"].contains(&r.role.as_str())
                || !roles.insert(&r.role)
                || !bare(&r.identity)
            {
                return Err(invalid());
            }
            wire::epoch(&r.epoch)?;
            if r.epoch != self.roles[0].epoch {
                return Err(Error::Conflict(
                    "owner readers issue different source epochs",
                ));
            }
        }
        if !self.roles.iter().any(|r| r.role != "issuer")
            || self.metadata_record_types.len() > wire::RESPONSE_BYTES / 2
        {
            return Err(invalid());
        }
        let mut kinds = BTreeSet::new();
        for k in &self.metadata_record_types {
            if k.is_empty()
                || k.len() > 64
                || !k.as_bytes()[0].is_ascii_lowercase()
                || !k
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                || !kinds.insert(k)
            {
                return Err(invalid());
            }
        }
        if !self.has("metadata") && !self.metadata_record_types.is_empty() {
            return Err(invalid());
        }
        Ok(self.roles[0].epoch.clone())
    }
    fn has(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r.role == role)
    }
}
/// Properties copied from the SAME retained objects, never a Python verdict.
/// Fresh observations are normalized and compared by Rust at every fence.
fn binding_observation(v: &Value) -> Result<Binding> {
    exact(
        v,
        &["binding_kind", "roles", "metadata_record_types", "prepared"],
    )?;
    let kind = text(v, "binding_kind")?;
    let rows = v["roles"].as_array().ok_or_else(invalid)?;
    if rows.is_empty() || rows.len() > 5 {
        return Err(invalid());
    }
    let mut epoch = None;
    let mut inputs_digest = None;
    let mut catalog_identity = None;
    let mut catalog_header = None;
    let mut catalog_namespace = None;
    let mut authored_view = None;
    if kind == "prepared-source-vector" {
        let p = &v["prepared"];
        exact(p, &["source_inputs_raw", "catalog_snapshot"])?;
        let raw = text(p, "source_inputs_raw")?.as_bytes();
        if raw.len() > wire::RECORD_BYTES {
            return Err(budget());
        }
        let inputs = PreparedSourceInputs::parse(raw, Default::default()).map_err(|_| invalid())?;
        let full = crate::source_agent_publication::require_vector(&inputs, wire::RECORD_BYTES)
            .map_err(|_| invalid())?;
        let root = inputs.roots().get("source-catalog").ok_or_else(invalid)?;
        let snapshot = &p["catalog_snapshot"];
        exact(
            snapshot,
            &["identity", "root_sha256", "header", "namespace_path"],
        )?;
        if !bare(text(snapshot, "identity")?)
            || snapshot["root_sha256"] != root.snapshot_sha256
            || snapshot["namespace_path"] != root.namespace_path
        {
            return Err(invalid());
        }
        let root_value = parse(&root.root_bytes, 262144)?;
        let header = &root_value["header"];
        if snapshot["header"] != *header
            || header["schema_version"] != "tos_source_catalog_projection_v2"
        {
            return Err(Error::Conflict("prepared catalog observation differs"));
        }
        let e = json!({"source_revision":inputs.source_revision(),"catalog_root_sha256":root.snapshot_sha256,"catalog_namespace":header["catalog_namespace"],"source_publication":header["source_publication"]});
        wire::epoch(&e)?;
        if full["source_publication"] != e["source_publication"]["token"] {
            return Err(invalid());
        }
        epoch = Some(e);
        inputs_digest = Some(inputs.digest().to_owned());
        catalog_identity = Some(snapshot["identity"].clone());
        catalog_header = Some(snapshot["header"].clone());
        catalog_namespace = Some(snapshot["namespace_path"].clone());
        if let Some(root) = inputs.roots().get("authored-corpus") {
            authored_view = Some(
                json!({"namespace_path":root.namespace_path,"root_json":std::str::from_utf8(&root.root_bytes).map_err(|_|invalid())?,"snapshot_sha256":root.snapshot_sha256}),
            );
        }
    } else if kind != "owner-issued-reader" || !v["prepared"].is_null() {
        return Err(invalid());
    }
    let mut roles = Vec::new();
    for r in rows {
        exact(
            r,
            &[
                "role",
                "identity",
                "epoch",
                "snapshot",
                "view",
                "authored_identity",
            ],
        )?;
        let role = text(r, "role")?;
        if !bare(text(r, "identity")?) {
            return Err(invalid());
        }
        let (issued, custody) = if kind == "owner-issued-reader" {
            if role == "authored"
                || !r["snapshot"].is_null()
                || !r["view"].is_null()
                || !r["authored_identity"].is_null()
            {
                return Err(invalid());
            }
            wire::epoch(&r["epoch"])?;
            (r["epoch"].clone(), Value::Null)
        } else if role == "authored" {
            let e = epoch.as_ref().ok_or_else(invalid)?;
            if r["epoch"] != *e
                || Some(&r["view"]) != authored_view.as_ref()
                || !r["snapshot"].is_null()
                || !r["authored_identity"].is_null()
            {
                return Err(Error::Conflict("authored owner view or epoch differs"));
            }
            (
                e.clone(),
                json!({"view_sha256":Digest256::of_bytes(&encode(&r["view"],wire::RECORD_BYTES)?).to_hex()}),
            )
        } else {
            if !r["epoch"].is_null() || !r["view"].is_null() {
                return Err(invalid());
            }
            let snap = &r["snapshot"];
            exact(
                snap,
                &["identity", "root_sha256", "header", "namespace_path"],
            )?;
            if !bare(text(snap, "identity")?)
                || snap["root_sha256"] != epoch.as_ref().ok_or_else(invalid)?["catalog_root_sha256"]
                || Some(&snap["header"]) != catalog_header.as_ref()
                || (role == "issuer"
                    && (Some(&snap["namespace_path"]) != catalog_namespace.as_ref()
                        || Some(&snap["identity"]) != catalog_identity.as_ref()))
            {
                return Err(Error::Conflict("prepared reader catalog differs"));
            }
            if role != "issuer" && !r["authored_identity"].is_null() {
                return Err(invalid());
            }
            if role == "issuer" {
                let expected = rows
                    .iter()
                    .find(|a| a["role"] == "authored")
                    .map(|a| a["identity"].clone())
                    .unwrap_or(Value::Null);
                if r["authored_identity"] != expected {
                    return Err(Error::Conflict("issuer authored object differs"));
                }
            }
            let header_sha256 = Digest256::of_bytes(&encode(&snap["header"], 262144)?).to_hex();
            // Generic readers promise the catalog root/header, not an identity
            // or namespace on their transient snapshot wrapper. The issuer
            // alone promises the selected snapshot object and its actual view.
            let custody = if role == "issuer" {
                json!({"snapshot_identity":snap["identity"],"header_sha256":header_sha256,"namespace_path":snap["namespace_path"],"authored_identity":r["authored_identity"]})
            } else {
                json!({"header_sha256":header_sha256})
            };
            (epoch.as_ref().ok_or_else(invalid)?.clone(), custody)
        };
        roles.push(Role {
            role: role.into(),
            identity: text(r, "identity")?.into(),
            epoch: issued,
            custody,
        });
    }
    roles.sort_by(|a, b| a.role.cmp(&b.role));
    let mut types: Vec<String> =
        serde_json::from_value(v["metadata_record_types"].clone()).map_err(|_| invalid())?;
    types.sort();
    let b = Binding {
        binding_kind: kind.into(),
        roles,
        metadata_record_types: types,
        source_inputs_sha256: inputs_digest,
    };
    b.validate()?;
    Ok(b)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema_version: String,
    session: String,
    sequence: u64,
    stage: String,
    operation: String,
    request: Value,
    request_sha256: String,
    limits: Limits,
    binding: Option<Binding>,
    epoch: Option<Value>,
    expected_initial_epoch: Option<Value>,
    target: Option<Value>,
    descriptor: Option<Value>,
    resolved: Option<Value>,
    packet: Option<Value>,
    started_ns: u64,
    callback_elapsed_ns: u64,
    native_budget_ns: u64,
    completed_native_ns: u64,
    phase_started_ns: u64,
    caller_deadline_ns: Option<u64>,
    remaining_work_bytes: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InitializedBinding {
    schema_version: String,
    session: String,
    binding: Binding,
    epoch: Value,
    limits: Limits,
}
/// State bytes must only be received from the authenticated native-produced sealed FD.
/// This type has no caller-supplied epoch/rights constructor.
pub struct PhaseResult {
    pub output: Vec<u8>,
    pub continuation: Option<Vec<u8>>,
    pub continuation_marker: &'static [u8],
    clock: ClockFence,
}
struct ClockFence {
    started_ns: u64,
    callback_ns: u64,
    native_budget_ns: u64,
    completed_native_ns: u64,
    phase_started_ns: u64,
    caller_deadline_ns: Option<u64>,
}
impl ClockFence {
    fn check(&self, now: u64) -> Result<()> {
        let execution = now
            .checked_sub(self.started_ns)
            .and_then(|n| n.checked_sub(self.callback_ns))
            .ok_or_else(invalid)?;
        let domain = self
            .completed_native_ns
            .checked_add(now.checked_sub(self.phase_started_ns).ok_or_else(invalid)?)
            .ok_or_else(budget)?;
        if self.started_ns > self.phase_started_ns
            || execution >= 45_000_000_000
            || domain >= self.native_budget_ns
            || self.caller_deadline_ns.is_some_and(|n| now >= n)
        {
            return Err(budget());
        }
        Ok(())
    }
    fn value(&self) -> Value {
        json!({"execution_started_ns":self.started_ns,"callback_elapsed_ns":self.callback_ns,"completed_native_ns":self.completed_native_ns,"phase_started_ns":self.phase_started_ns,"native_budget_ns":self.native_budget_ns,"work_budget_ns":45_000_000_000u64,"execution_budget_ns":50_000_000_000u64,"caller_deadline_ns":self.caller_deadline_ns})
    }
}
impl PhaseResult {
    /// Parent additionally charges observed terminal-entry after authenticated
    /// WNOWAIT; output/FD from a failed join is never accepted.
    pub fn check_clock(&self, now: u64) -> Result<()> {
        self.clock.check(now)
    }
}

impl State {
    fn fence(&self) -> ClockFence {
        ClockFence {
            started_ns: self.started_ns,
            callback_ns: self.callback_elapsed_ns,
            native_budget_ns: self.native_budget_ns,
            completed_native_ns: self.completed_native_ns,
            phase_started_ns: self.phase_started_ns,
            caller_deadline_ns: self.caller_deadline_ns,
        }
    }
    fn clock(&self, now: u64) -> Result<()> {
        self.fence().check(now)
    }
    fn binding(&self) -> Result<&Binding> {
        self.binding.as_ref().ok_or_else(invalid)
    }
    fn epoch(&self) -> Result<&Value> {
        self.epoch.as_ref().ok_or_else(invalid)
    }
    fn target(&self) -> Result<&Value> {
        self.target.as_ref().ok_or_else(invalid)
    }
    fn reserve(&mut self) -> Result<()> {
        self.remaining_work_bytes = self
            .remaining_work_bytes
            .checked_sub(PHASE_WORK)
            .ok_or_else(budget)?;
        Ok(())
    }
    fn plan(&self) -> Result<Value> {
        let (operation, arguments) = match self.stage.as_str() {
            "binding" => ("binding", json!({})),
            "verify_lookup" | "verify_mint" | "verify_final" => (
                "verify_current",
                json!({"role_identities": self.binding()?.roles.iter().map(|r|json!({"role":r.role,"identity":r.identity})).collect::<Vec<_>>() }),
            ),
            "issue" => (
                "issue_target",
                json!({"selector": self.request["selector"]}),
            ),
            "descriptor" => (
                "slot_descriptor",
                json!({"kind": self.target()?["slot_kind"], "identity": self.target()?["identity"]}),
            ),
            "resolve" => match text(self.target()?, "layer")? {
                "metadata_record" => (
                    "resolve_metadata",
                    json!({"record_ref": self.target()?["record_ref"]}),
                ),
                "claim_record" => (
                    "resolve_claim",
                    json!({"record_ref": self.target()?["record_ref"]}),
                ),
                "source_slot" => (
                    "read_slot",
                    json!({"kind": self.target()?["slot_kind"], "identity": self.target()?["identity"], "expected_row_sha256": self.target()?["row_sha256"]}),
                ),
                "authored_csv_record" => ("resolve_authored_csv", json!({"target":self.target()?})),
                _ => return Err(invalid()),
            },
            _ => return Err(invalid()),
        };
        Ok(
            json!({"schema_version":"tos_source_owner_provider_request_v1","session":self.session,"request_id":self.sequence,"operation":operation,"arguments":arguments}),
        )
    }
    fn finish(mut self, now: u64) -> Result<PhaseResult> {
        self.clock(now)?;
        let clock = self.fence();
        if self.stage == "done" {
            let initialized = InitializedBinding {
                schema_version: "tos_source_owner_initialized_binding_v1".into(),
                session: self.session.clone(),
                binding: self.binding()?.clone(),
                epoch: self.epoch()?.clone(),
                limits: self.limits.clone(),
            };
            let packet = self.packet.take().ok_or_else(invalid)?;
            // Successful handle/record disclosure already checks the selected
            // cap in resolved_packet. Capabilities and fixed refusal packets
            // retain their ordinary wire ceiling, including zero profiles.
            encode(&packet, wire::RESPONSE_BYTES)?;
            return Ok(PhaseResult {
                output: encode(
                    &json!({"schema_version":"tos_source_owner_phase_v1","session":self.session,"request_id":self.sequence,"status":"complete","callback":null,"packet":packet,"clock":clock.value()}),
                    PHASE_OUTPUT_BYTES,
                )?,
                continuation: Some(encode(
                    &serde_json::to_value(initialized).map_err(|_| invalid())?,
                    STATE_BYTES,
                )?),
                continuation_marker: b"tos-source-owner-binding-v1",
                clock,
            });
        }
        if self.sequence == 0 || self.sequence >= MAX_PHASES {
            return Err(budget());
        }
        let output = encode(
            &json!({"schema_version":"tos_source_owner_phase_v1","session":self.session,"request_id":self.sequence,"status":"callback","callback":self.plan()?,"packet":null,"clock":clock.value()}),
            PHASE_OUTPUT_BYTES,
        )?;
        let continuation = encode(
            &serde_json::to_value(&self).map_err(|_| invalid())?,
            STATE_BYTES,
        )?;
        Ok(PhaseResult {
            output,
            continuation: Some(continuation),
            continuation_marker: b"tos-source-owner-state-v1",
            clock,
        })
    }
    fn base(&self) -> Result<Value> {
        let epoch = self.epoch()?;
        let target = self.target.as_ref();
        if self.operation == "discover" {
            return Ok(
                json!({"schema_version":"tos_source_handle_discovery_v1","status":"unsupported","reason":"owner-reader-not-configured","target":target,"handle":null,"source_revision":epoch["source_revision"],"content_revision":target.map(|t|&t["content_revision"]),"provenance":null,"access":null,"grants_current_use":false,"performs_assessment":false,"writes_to_source":false}),
            );
        }
        let handle = &self.request["handle"];
        let t = &handle["target"];
        Ok(
            json!({"schema_version":"tos_source_read_result_v1","status":"unsupported","reason":"owner-reader-not-configured","handle":handle,"source_revision":epoch["source_revision"],"content_revision":t["content_revision"],"layer":t["layer"],"record_kind":match t["layer"].as_str(){Some("metadata_record")=>"metadata",Some("claim_record")=>"claim",Some("authored_csv_record")=>"authored_csv",_=>"source_slot"},"record_ref":t.get("record_ref"),"record":null,"provenance":null,"access":handle["access"],"grants_current_use":false,"performs_assessment":false,"writes_to_source":false}),
        )
    }
    fn close_packet(&mut self, status: &str, reason: &str) -> Result<()> {
        let mut p = self.base()?;
        p["status"] = json!(status);
        p["reason"] = json!(reason);
        if self.operation == "read" && status != "stale" && status != "over-budget" {
            p["access"] = Value::Null;
        }
        if self.operation == "discover"
            && self.target.is_none()
            && self.request.get("selector").is_some()
        {
            p["selector"] = self.request["selector"].clone();
        }
        self.packet = Some(p);
        self.stage = "verify_final".into();
        Ok(())
    }
    fn lookup(&mut self) -> Result<()> {
        let target = self.target()?;
        wire::target(target)?;
        let role = match text(target, "layer")? {
            "metadata_record" => "metadata",
            "claim_record" => "claim",
            "source_slot" => "slot",
            _ => "authored",
        };
        if !self.binding()?.has(role) {
            return self.close_packet(
                "unsupported",
                match role {
                    "metadata" => "metadata-owner-reader-unconfigured",
                    "claim" => "claim-owner-reader-unconfigured",
                    "slot" => "source-slot-owner-reader-unconfigured",
                    _ => "authored-owner-reader-unconfigured",
                },
            );
        }
        if role == "metadata"
            && !self
                .binding()?
                .metadata_record_types
                .iter()
                .any(|k| target["record_type"] == *k)
        {
            return self.close_packet("unsupported", "metadata-type-not-owner-declared");
        }
        self.stage = if role == "slot" {
            "descriptor"
        } else {
            "resolve"
        }
        .into();
        Ok(())
    }
    fn resolved_packet(&mut self) -> Result<()> {
        let r = self.resolved.as_ref().ok_or_else(invalid)?;
        let mut p = self.base()?;
        p["status"] = json!("available");
        p["access"] = r["access"].clone();
        p["provenance"] = r["provenance"].clone();
        if self.operation == "discover" {
            if self.target()?["layer"] == "authored_csv_record" {
                p["provenance"]
                    .as_object_mut()
                    .ok_or_else(invalid)?
                    .remove("raw_record");
            }
            let handle = wire::make_handle(self.epoch()?, self.target()?, &r["access"])?;
            encode(&handle, self.limits.max_handle_bytes)?;
            p["handle"] = handle;
            p["reason"] = json!("owner-issued-exact-source-handle");
        } else {
            p["record"] = r["record"].clone();
            p["reason"] = json!("exact-owner-source-record");
        }
        encode(&p, self.limits.max_response_bytes)?;
        self.packet = Some(p);
        self.stage = "verify_final".into();
        Ok(())
    }
    fn resolved_or_budget(&mut self) -> Result<()> {
        match self.resolved_packet() {
            Err(Error::Unsupported("source response byte budget" | "source-read-budget")) => {
                self.close_packet("over-budget", "source-read-budget")
            }
            result => result,
        }
    }
    fn verify_roles(&self, v: &Value) -> Result<()> {
        let binding = binding_observation(v)?;
        if binding != *self.binding()? {
            return Err(Error::Conflict("source owner epoch or identity withdrawn"));
        }
        Ok(())
    }
}
fn selector(v: &Value) -> Result<()> {
    match text(v, "layer")? {
        "metadata_record" => {
            exact(v, &["layer", "record_type", "record_id"])?;
            if !wire::kind(text(v, "record_type")?)
                || !wire::identity(text(v, "record_id")?, Some(false))
            {
                return Err(invalid());
            }
        }
        "claim_record" => {
            exact(v, &["layer", "claim_id"])?;
            if !wire::identity(text(v, "claim_id")?, Some(true)) {
                return Err(invalid());
            }
        }
        "source_slot" => {
            exact(v, &["layer", "slot_kind", "identity"])?;
            let kind = text(v, "slot_kind")?;
            if !["claim", "provenance_event", "anchor"].contains(&kind)
                || !wire::identity(
                    text(v, "identity")?,
                    if kind == "claim" { Some(true) } else { None },
                )
            {
                return Err(invalid());
            }
        }
        "authored_csv_record" => {
            exact(v, &["layer", "pack_id", "edge_id"])?;
            let mut target = v.clone();
            target["source_row"] = json!(1);
            target["source_file_sha256"] = json!("0".repeat(64));
            target["content_revision"] = json!(format!("sha256:{}", "0".repeat(64)));
            wire::target(&target)?;
        }
        _ => return Err(invalid()),
    }
    Ok(())
}
/// Begin only captures a request and plans actual owner binding; it issues no handle.
pub fn begin(raw: &[u8], now_ns: u64, native_budget_ns: u64) -> Result<PhaseResult> {
    begin_with_binding(raw, None, now_ns, native_budget_ns)
}
/// The binding descriptor has the same authenticated native-produced custody
/// requirement as a phase descriptor. Seals or caller JSON are not sufficient.
pub fn begin_with_binding(
    raw: &[u8],
    initialized_raw: Option<&[u8]>,
    now_ns: u64,
    native_budget_ns: u64,
) -> Result<PhaseResult> {
    let v = parse(raw, wire::REQUEST_BYTES + 4096)?;
    exact(
        &v,
        &[
            "schema_version",
            "session",
            "operation",
            "request",
            "limits",
            "caller_deadline_ns",
            "execution_started_ns",
            "expected_initial_epoch",
        ],
    )?;
    if text(&v, "schema_version")? != "tos_source_owner_begin_v1"
        || !bare(text(&v, "session")?)
        || native_budget_ns == 0
        || native_budget_ns > 5_000_000_000
    {
        return Err(invalid());
    }
    let limits: Limits = serde_json::from_value(v["limits"].clone()).map_err(|_| invalid())?;
    limits.validate()?;
    let expected_initial_epoch = if v["expected_initial_epoch"].is_null() {
        None
    } else {
        wire::epoch(&v["expected_initial_epoch"])?;
        Some(v["expected_initial_epoch"].clone())
    };
    let initialized = initialized_raw
        .map(|raw| {
            let value: InitializedBinding =
                serde_json::from_value(parse(raw, STATE_BYTES)?).map_err(|_| invalid())?;
            value.limits.validate()?;
            if value.schema_version != "tos_source_owner_initialized_binding_v1"
                || value.session != text(&v, "session")?
                || value.limits != limits
                || value.binding.validate()? != value.epoch
            {
                return Err(invalid());
            }
            Ok(value)
        })
        .transpose()?;
    let request = v["request"].clone();
    let operation = text(&v, "operation")?.to_owned();
    if initialized.is_none() && operation != "capabilities" {
        return Err(Error::Invalid("native owner initialization is required"));
    }
    let request_raw = encode(&request, wire::REQUEST_BYTES)?;
    let request_over_budget =
        operation != "capabilities" && request_raw.len() > limits.max_request_bytes;
    match operation.as_str() {
        "capabilities" => exact(&request, &[])?,
        "discover" if !request_over_budget => {
            if request.get("target").is_some() {
                exact(&request, &["target"])?;
                wire::target(&request["target"])?;
            } else {
                exact(&request, &["selector"])?;
                selector(&request["selector"])?;
            }
        }
        "read" if !request_over_budget => {
            exact(&request, &["handle", "representation"])?;
            wire::validate_handle(&request["handle"])?;
            if !["record", "native_public_unit", "native_local_unit"]
                .contains(&text(&request, "representation")?)
            {
                return Err(invalid());
            }
        }
        "discover" | "read" if request_over_budget => {}
        _ => return Err(invalid()),
    }
    let caller_deadline_ns = if v["caller_deadline_ns"].is_null() {
        None
    } else {
        Some(v["caller_deadline_ns"].as_u64().ok_or_else(invalid)?)
    };
    let mut state = State {
        schema_version: STATE_SCHEMA.into(),
        session: text(&v, "session")?.into(),
        sequence: 1,
        stage: "binding".into(),
        operation,
        request,
        request_sha256: Digest256::of_bytes(&request_raw).to_hex(),
        limits,
        binding: initialized.as_ref().map(|b| b.binding.clone()),
        epoch: initialized.as_ref().map(|b| b.epoch.clone()),
        expected_initial_epoch,
        target: None,
        descriptor: None,
        resolved: None,
        packet: None,
        started_ns: v["execution_started_ns"]
            .as_u64()
            .filter(|n| *n <= now_ns)
            .ok_or_else(invalid)?,
        callback_elapsed_ns: 0,
        native_budget_ns,
        completed_native_ns: 0,
        phase_started_ns: now_ns,
        caller_deadline_ns,
        remaining_work_bytes: WORK_BYTES,
    };
    state.reserve()?;
    state.finish(now_ns)
}
/// Consume one response from the SAME retained provider. State custody is enforced
/// by the Access sealed-FD adapter, never by a caller-provided state hash.
pub fn advance(
    state_raw: &[u8],
    response_raw: &[u8],
    phase_started_ns: u64,
    now_ns: u64,
) -> Result<PhaseResult> {
    let mut s: State =
        serde_json::from_value(parse(state_raw, STATE_BYTES)?).map_err(|_| invalid())?;
    if s.schema_version != STATE_SCHEMA
        || !bare(&s.session)
        || s.sequence == 0
        || s.sequence >= MAX_PHASES
        || s.remaining_work_bytes > WORK_BYTES
        || s.native_budget_ns == 0
        || s.native_budget_ns > 5_000_000_000
    {
        return Err(invalid());
    }
    s.limits.validate()?;
    if s.request_sha256 != Digest256::of_bytes(&encode(&s.request, wire::REQUEST_BYTES)?).to_hex() {
        return Err(invalid());
    }
    let v = parse(response_raw, PHASE_INPUT_BYTES)?;
    exact(
        &v,
        &[
            "schema_version",
            "session",
            "request_id",
            "operation",
            "status",
            "value",
            "error",
            "callback_elapsed_ns",
            "previous_phase_terminal_ns",
        ],
    )?;
    let plan = s.plan()?;
    if text(&v, "schema_version")? != RESPONSE_SCHEMA
        || v["session"] != s.session
        || v["request_id"] != s.sequence
        || v["operation"] != plan["operation"]
    {
        return Err(invalid());
    }
    let terminal = v["previous_phase_terminal_ns"]
        .as_u64()
        .ok_or_else(invalid)?;
    let callback = v["callback_elapsed_ns"].as_u64().ok_or_else(invalid)?;
    if terminal < s.phase_started_ns
        || terminal > phase_started_ns
        || phase_started_ns > now_ns
        || callback > phase_started_ns.checked_sub(terminal).ok_or_else(invalid)?
    {
        return Err(invalid());
    }
    // Conservative WNOWAIT observation includes scheduling delay after exit.
    // It is supplied only by the platform's held native child owner, not by a
    // provider result, and cannot reduce previously charged Rust time.
    s.completed_native_ns = s
        .completed_native_ns
        .checked_add(
            terminal
                .checked_sub(s.phase_started_ns)
                .ok_or_else(invalid)?,
        )
        .ok_or_else(budget)?;
    s.callback_elapsed_ns = s
        .callback_elapsed_ns
        .checked_add(callback)
        .ok_or_else(budget)?;
    s.phase_started_ns = phase_started_ns;
    s.clock(now_ns)?;
    s.reserve()?;
    if text(&v, "status")? == "error" {
        if !v["value"].is_null() {
            return Err(invalid());
        }
        exact(&v["error"], &["status", "reason"])?;
        let status = text(&v["error"], "status")?;
        let reason = text(&v["error"], "reason")?;
        if ![
            "access-restricted",
            "corrupt",
            "missing",
            "over-budget",
            "stale",
            "unsupported",
        ]
        .contains(&status)
            || reason.chars().count() > 256
        {
            return Err(invalid());
        }
        if s.stage == "binding" || s.stage.starts_with("verify_") {
            return Err(Error::Denied("source owner callback refused or withdrew"));
        }
        let status = if status == "corrupt"
            && (reason.ends_with("-unconfigured") || reason == "metadata-type-not-owner-declared")
        {
            "unsupported"
        } else {
            status
        };
        s.close_packet(status, reason)?;
        s.sequence = s.sequence.checked_add(1).ok_or_else(budget)?;
        return s.finish(now_ns);
    }
    if text(&v, "status")? != "ok" || !v["error"].is_null() {
        return Err(invalid());
    }
    let value = &v["value"];
    match s.stage.as_str() {
        "binding" => {
            let b = binding_observation(value)?;
            let epoch = b.validate()?;
            if s.expected_initial_epoch
                .as_ref()
                .is_some_and(|e| *e != epoch)
            {
                return Err(Error::Conflict(
                    "captured legacy source owner epoch differs",
                ));
            }
            if let Some(original) = &s.binding {
                if *original != b || s.epoch.as_ref() != Some(&epoch) {
                    return Err(Error::Conflict(
                        "initialized source owner binding withdrawn",
                    ));
                }
            } else {
                s.epoch = Some(epoch);
                s.binding = Some(b);
            }
            s.stage = "verify_lookup".into();
        }
        "verify_lookup" => {
            s.verify_roles(value)?;
            if s.operation == "capabilities" {
                s.packet = Some(capabilities(s.binding()?, s.epoch()?, &s.limits)?);
                s.stage = "verify_final".into();
            } else if encode(&s.request, wire::REQUEST_BYTES)?.len() > s.limits.max_request_bytes
                || (s.operation == "read"
                    && encode(&s.request["handle"], wire::HANDLE_BYTES)?.len()
                        > s.limits.max_handle_bytes)
            {
                s.close_packet("over-budget", "source-read-budget")?;
            } else if s.operation == "read" {
                s.target = Some(s.request["handle"]["target"].clone());
                if s.request["handle"]["epoch"] != *s.epoch()? {
                    s.close_packet("stale", "source-epoch-differs")?;
                } else if s.request["representation"] != "record" {
                    let mut p = s.base()?;
                    p["schema_version"] = json!("tos_source_native_unit_read_result_v1");
                    p["native_unit"] = Value::Null;
                    p["text_access"] = Value::Null;
                    p["reason"] = json!("native-unit-owner-unconfigured");
                    s.packet = Some(p);
                    s.stage = "verify_final".into();
                } else {
                    s.lookup()?;
                }
            } else if s.request.get("selector").is_some() {
                if !s.binding()?.has("issuer") {
                    s.close_packet("unsupported", "owner target resolver is not configured")?;
                } else {
                    s.stage = "issue".into();
                }
            } else {
                s.target = Some(s.request["target"].clone());
                s.lookup()?;
            }
        }
        "issue" => {
            wire::target(value)?;
            s.target = Some(value.clone());
            s.lookup()?;
        }
        "descriptor" => {
            check_descriptor(value, s.target()?)?;
            let mut normalized = value.clone();
            if json_falsey(&normalized["provenance"]) {
                normalized["provenance"] = json!({});
            }
            s.descriptor = Some(normalized);
            s.stage = "resolve".into();
        }
        "resolve" => {
            if s.target()?["layer"] != "source_slot" {
                let (status, reason) = owner_status(value);
                if status != "available" {
                    s.close_packet(status, reason)?;
                    s.sequence = s.sequence.checked_add(1).ok_or_else(budget)?;
                    return s.finish(now_ns);
                }
            }
            match resolve_record(&s, value) {
                Ok(record) => {
                    s.resolved = Some(record);
                    if s.operation == "discover" {
                        s.stage = "verify_mint".into();
                    } else {
                        s.resolved_or_budget()?;
                    }
                }
                Err(Error::Denied(_) | Error::DeniedWithReason(_)) => {
                    s.close_packet("access-restricted", "owner-source-path-restricted")?
                }
                Err(Error::Conflict(reason)) => s.close_packet("stale", reason)?,
                Err(Error::Unsupported("source response byte budget" | "source-read-budget")) => {
                    s.close_packet("over-budget", "source-read-budget")?
                }
                Err(Error::Unsupported(reason)) => s.close_packet("unsupported", reason)?,
                Err(Error::Invalid(reason)) => s.close_packet("corrupt", reason)?,
                Err(_) => return Err(invalid()),
            }
        }
        "verify_mint" => {
            s.verify_roles(value)?;
            s.resolved_or_budget()?;
        }
        "verify_final" => {
            s.verify_roles(value)?;
            s.stage = "done".into();
        }
        _ => return Err(invalid()),
    }
    s.sequence = s.sequence.checked_add(1).ok_or_else(budget)?;
    s.finish(now_ns)
}
fn capabilities(b: &Binding, epoch: &Value, limits: &Limits) -> Result<Value> {
    let mut types = b.metadata_record_types.clone();
    types.sort();
    Ok(
        json!({"schema_version":"tos_source_read_capabilities_v1","available":true,"issuer":wire::ISSUER,"layers":{"metadata_record":types,"claim_record":b.has("claim"),"source_slot":b.has("slot"),"authored_csv_record":b.has("authored")},"limits":limits,"source_epoch":epoch,"binding":{"kind":b.binding_kind,"catalog_root_sha256":epoch["catalog_root_sha256"],"source_inputs_sha256":b.source_inputs_sha256},"authority":{"is_source":false,"writes_to_source":false,"grants_current_use":false,"native_text_payload":false,"note":"Owner readers retain source, rights, and currentness authority; this adapter discloses only owner-declared public metadata and grants no use rights."}}),
    )
}
fn json_falsey(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Bool(b) => !*b,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
    }
}
fn owner_status(v: &Value) -> (&str, &str) {
    if !v.is_object() {
        return ("corrupt", "owner-reader-returned-no-envelope");
    }
    let Some(status) = v["status"].as_str().filter(|s| {
        [
            "access-restricted",
            "available",
            "corrupt",
            "missing",
            "over-budget",
            "stale",
            "unsupported",
        ]
        .contains(s)
    }) else {
        return ("corrupt", "owner-reader-returned-unknown-status");
    };
    let Some(reason) = v["reason"]
        .as_str()
        .filter(|s| !s.is_empty() && s.chars().count() <= 256)
    else {
        return ("corrupt", "owner-reader-returned-invalid-reason");
    };
    (status, reason)
}
fn check_descriptor(v: &Value, t: &Value) -> Result<()> {
    exact(
        v,
        &["row_sha256", "canonical_sha256", "visibility", "provenance"],
    )?;
    if !bare(text(v, "row_sha256")?)
        || !bare(text(v, "canonical_sha256")?)
        || v["row_sha256"] != t["row_sha256"]
        || format!("sha256:{}", text(v, "canonical_sha256")?) != t["content_revision"]
    {
        return Err(Error::Conflict("source-slot-descriptor-differs"));
    }
    wire::access("public-source-slot-metadata", text(v, "visibility")?)?;
    if !json_falsey(&v["provenance"]) && !v["provenance"].is_object() {
        return Err(invalid());
    }
    Ok(())
}
fn visibility(record: &Value, descriptor: Option<&Value>) -> Result<String> {
    let public = |s: &str| ["public", "public_metadata_only"].contains(&s);
    let own = record.get("visibility");
    if own.is_some_and(|v| !v.as_str().is_some_and(public)) {
        return Err(Error::Denied("owner metadata visibility"));
    }
    if let Some(d) = descriptor {
        if let Some(scope) = d.get("source_scope") {
            let scope = scope
                .as_str()
                .filter(|s| public(s))
                .ok_or(Error::Denied("owner descriptor visibility"))?;
            if own.is_some_and(|v| v != scope) {
                return Err(Error::Denied("owner descriptor visibility differs"));
            }
            return Ok(scope.into());
        }
        if own.is_none() && d["adapter"] == "native-corpus" {
            return Ok("public_metadata_only".into());
        }
    }
    own.and_then(Value::as_str)
        .map(String::from)
        .ok_or(Error::Denied("owner metadata visibility absent"))
}
fn provenance(v: &Value, limits: &Limits) -> Result<Value> {
    if !v.is_object() {
        return Err(Error::Invalid("owner provenance is not an object"));
    }
    encode(v, limits.max_response_bytes)?;
    Ok(v.clone())
}
fn resolve_record(s: &State, v: &Value) -> Result<Value> {
    let t = s.target()?;
    let layer = text(t, "layer")?;
    let (record, prov, access) = if layer == "source_slot" {
        exact(v, &["row_sha256", "payload", "raw_bytes_len", "provenance"])?;
        let d = s.descriptor.as_ref().ok_or_else(invalid)?;
        check_descriptor(d, t)?;
        if v["row_sha256"] != t["row_sha256"] {
            return Err(Error::Invalid("owner source-slot row digest differs"));
        }
        if !v["raw_bytes_len"].is_null()
            && !v["raw_bytes_len"]
                .as_u64()
                .is_some_and(|n| n <= s.limits.max_record_bytes as u64)
        {
            return Err(Error::Unsupported("source-read-budget"));
        }
        let record = &v["payload"];
        if !record.is_object() {
            return Err(Error::Invalid(
                "owner source-slot reader returned no exact payload",
            ));
        }
        let field = match text(t, "slot_kind")? {
            "claim" => "claim_id",
            "provenance_event" => "event_id",
            "anchor" => "anchor_id",
            _ => return Err(invalid()),
        };
        if record[field] != t["identity"] {
            return Err(Error::Invalid("owner source-slot identity differs"));
        }
        if record
            .get("visibility")
            .is_some_and(|visibility| visibility != &d["visibility"])
        {
            return Err(Error::Denied(
                "source-slot descriptor and payload visibility differ",
            ));
        }
        let p = if json_falsey(&v["provenance"]) {
            &d["provenance"]
        } else {
            &v["provenance"]
        };
        (
            record.clone(),
            provenance(p, &s.limits)?,
            wire::access("public-source-slot-metadata", text(d, "visibility")?)?,
        )
    } else if layer == "authored_csv_record" {
        exact(v, &["status", "reason", "record", "provenance"])?;
        let record = &v["record"];
        if !record.is_object()
            || record.get("edge_id").is_some_and(|id| {
                id.as_str().is_some_and(|id| !id.is_empty()) && id != &t["edge_id"]
            })
        {
            return Err(Error::Invalid("authored CSV exact record differs"));
        }
        (
            record.clone(),
            provenance(&v["provenance"], &s.limits)?,
            wire::access("public-authored-csv-record", "public_metadata_only")?,
        )
    } else {
        let status = text(v, "status")?;
        let reason = text(v, "reason")?;
        if ![
            "available",
            "access-restricted",
            "corrupt",
            "missing",
            "over-budget",
            "stale",
            "unsupported",
        ]
        .contains(&status)
            || reason.is_empty()
            || reason.chars().count() > 256
        {
            return Err(Error::Invalid("owner-reader-returned-invalid-status"));
        }
        if status != "available" {
            return Err(Error::Unsupported("owner-source-record-unavailable"));
        }
        let record = &v["record"];
        if !record.is_object() {
            return Err(Error::Invalid("owner reader returned no exact record"));
        }
        if v["exact_ref"] != t["record_ref"] {
            return Err(Error::Invalid(
                "owner reader did not return requested exact reference",
            ));
        }
        let (field, version, descriptor, scope) = if layer == "claim_record" {
            ("claim_id", "claim_version", None, "public-claim-record")
        } else {
            let d = &v["descriptor"];
            if !d.is_object() || d["record_type"] != t["record_type"] {
                return Err(Error::Invalid("owner metadata descriptor kind differs"));
            }
            let (kind, field) = match record["schema_version"].as_str() {
                Some("tos_scholarly_composite_witness_v1") => ("composite", "composite_id"),
                Some("tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2") => {
                    ("artifact", "artifact_id")
                }
                _ => (
                    record["record_type"]
                        .as_str()
                        .ok_or(Error::Invalid("owner metadata record type differs"))?,
                    "record_id",
                ),
            };
            if kind != text(t, "record_type")?
                || (field != "record_id"
                    && (record.get("record_id").is_some()
                        || record.get("record_type").is_some()
                        || d["adapter"] != "native-witness"
                        || d["identity_field"] != field))
            {
                return Err(Error::Invalid(
                    "owner native metadata descriptor identity differs",
                ));
            }
            (field, "record_version", Some(d), "public-metadata-record")
        };
        if record[field] != t["record_ref"]["id"] || record[version] != t["record_ref"]["version"] {
            return Err(Error::Invalid("owner record identity or version differs"));
        }
        let visibility = visibility(record, descriptor)?;
        (
            record.clone(),
            provenance(&v["provenance"], &s.limits)?,
            wire::access(scope, &visibility)?,
        )
    };
    let raw = encode(&record, s.limits.max_record_bytes)?;
    if Digest256::of_bytes(&raw).to_prefixed() != text(t, "content_revision")? {
        return Err(Error::Invalid("owner source content digest differs"));
    }
    Ok(json!({"record":record,"provenance":prov,"access":access}))
}
