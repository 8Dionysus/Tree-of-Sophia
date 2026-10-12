//! Explicit source-owner current-use policy for PG source navigation.
//!
//! A protected owner receipt and a current PG row jointly authorize only the
//! public metadata carrier. CMD rights, Item payload access, publication, and
//! source admission remain independent.

use postgres::Transaction;
use serde::Deserialize;
use serde_json::Value;
use tos_foundation::{Digest256, JsonLimits, JsonMode, parse_json};
use tos_query::{
    Binding, Charged, CurrentPolicy, DisclosureLease, DisclosureScope, QueryError, QueryErrorCode,
    RawRecord,
};

use crate::source_command as cmd;

pub const RECEIPT_SCHEMA: &str = "tos_source_navigation_public_metadata_policy_receipt_v1";
pub const CARRIER_LAYER: &str = "tos_source_navigation_public_metadata_v1";
pub const INTENDED_USE: &str = "read_only_public_metadata_navigation_v1";

const MAX_ISSUER_BYTES: usize = 4096;
const MAX_RECEIPT_ID_BYTES: usize = 512;
const MAX_PROFILE_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceNavigationPolicyOperationV1 {
    Install,
    Revoke,
}

impl SourceNavigationPolicyOperationV1 {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Revoke => "revoke",
        }
    }
}

pub struct SourceNavigationPolicyMutationV1<'a> {
    pub domain: &'a str,
    pub consumer_id: &'a str,
    pub selected_profile: &'a str,
    pub command_id: &'a str,
    pub operation: SourceNavigationPolicyOperationV1,
    pub expected_policy_version: Option<u64>,
    pub source_contract_sha256: Digest256,
    pub expected_rule_version: u64,
    pub expected_rights_version: u64,
    pub job_id: &'a str,
    pub job_fence: u64,
    pub bootstrap_source_revision: Digest256,
    pub source_epoch: u64,
    pub source_definition_digest: Digest256,
    pub bootstrap_membership_digest: Digest256,
    pub bootstrap_membership_count: u64,
    pub domain_digest: Digest256,
    pub database_oid: u64,
    pub anchor_through_sequence: u64,
    pub request_sha256: Digest256,
    pub owner_configuration_sha256: Digest256,
    pub receipt_sha256: Digest256,
    pub receipt: &'a SourceNavigationPolicyReceiptV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceNavigationPolicyMutationReceiptV1 {
    /// Immutable result of this command; on replay this can be older than the
    /// current policy row and therefore must not be treated as current-use
    /// authorization.
    pub policy_version: u64,
    pub withdrawal_generation: u64,
    pub allowed: bool,
    pub replayed: bool,
    pub current_policy_version: u64,
    pub current_withdrawal_generation: u64,
    pub current_allowed: bool,
    pub current_receipt_sha256: Digest256,
    pub current_owner_configuration_sha256: Digest256,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceNavigationPolicyReceiptV1 {
    pub schema_version: String,
    pub issuer_ref: String,
    pub receipt_id: String,
    pub export_receipt_id: String,
    pub model_receipt_id: String,
    pub carrier_layer: String,
    pub intended_use: String,
    pub model_abi: String,
    pub selection_profile: String,
    pub policy_epoch: u64,
    pub withdrawal_generation: u64,
}

impl SourceNavigationPolicyReceiptV1 {
    /// Parse a strict, explicit allow receipt from the protected local owner
    /// configuration. There is deliberately no default or wildcard receipt.
    pub fn parse(value: &Value) -> Result<(Self, Digest256), &'static str> {
        let raw =
            serde_json::to_vec(value).map_err(|_| "source navigation policy receipt codec")?;
        let parsed = cmd::parse(&raw).map_err(|_| "source navigation policy receipt JSON")?;
        let digest =
            cmd::record_digest(&parsed).map_err(|_| "source navigation policy receipt digest")?;
        let receipt: Self = serde_json::from_slice(
            &cmd::canonical(&parsed)
                .map_err(|_| "source navigation policy receipt canonicalization")?,
        )
        .map_err(|_| "source navigation policy receipt fields")?;
        receipt.validate()?;
        Ok((receipt, digest))
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != RECEIPT_SCHEMA
            || self.carrier_layer != CARRIER_LAYER
            || self.intended_use != INTENDED_USE
            || self.policy_epoch == 0
            || !valid_text(&self.issuer_ref, MAX_ISSUER_BYTES)
            || !self.issuer_ref.starts_with("ToS/")
            || !valid_text(&self.receipt_id, MAX_RECEIPT_ID_BYTES)
            || !valid_text(&self.export_receipt_id, MAX_RECEIPT_ID_BYTES)
            || !valid_text(&self.model_receipt_id, MAX_RECEIPT_ID_BYTES)
            || !valid_text(&self.model_abi, MAX_PROFILE_BYTES)
            || !valid_text(&self.selection_profile, MAX_PROFILE_BYTES)
        {
            return Err("source navigation policy receipt scope");
        }
        Ok(())
    }

    /// Recompute the receipt identity from exactly the fields accepted by the
    /// strict owner-config parser. The durable writer uses this to reject a
    /// caller-supplied digest that does not bind the stored receipt fields.
    pub fn digest(&self) -> Result<Digest256, &'static str> {
        let value = self.to_value()?;
        let raw =
            serde_json::to_vec(&value).map_err(|_| "source navigation policy receipt codec")?;
        let parsed = cmd::parse(&raw).map_err(|_| "source navigation policy receipt JSON")?;
        cmd::record_digest(&parsed).map_err(|_| "source navigation policy receipt digest")
    }

    /// Emit the canonical owner-facing JSON value for this validated receipt.
    pub fn to_value(&self) -> Result<Value, &'static str> {
        self.validate()?;
        Ok(serde_json::json!({
            "schema_version": self.schema_version,
            "issuer_ref": self.issuer_ref,
            "receipt_id": self.receipt_id,
            "export_receipt_id": self.export_receipt_id,
            "model_receipt_id": self.model_receipt_id,
            "carrier_layer": self.carrier_layer,
            "intended_use": self.intended_use,
            "model_abi": self.model_abi,
            "selection_profile": self.selection_profile,
            "policy_epoch": self.policy_epoch,
            "withdrawal_generation": self.withdrawal_generation,
        }))
    }
}

fn valid_text(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value.bytes().any(|byte| byte.is_ascii_control())
        && value.trim() == value
}

/// Read and lock the current policy row in the already owner-held repeatable
/// read transaction. The `FOR SHARE` lock survives until the caller's complete
/// response has been written and the transaction commits.
pub fn read_locked_v1(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    owner_configuration_sha256: Digest256,
    receipt: &SourceNavigationPolicyReceiptV1,
    receipt_sha256: Digest256,
    binding: &Binding,
    selected_model_receipt_id: &str,
    corpus_revision: &str,
) -> Result<(DisclosureScope, PgSourceNavigationCurrentPolicyV1), QueryError> {
    let row = tx
        .query_opt(
            "SELECT policy_issuer_ref,policy_receipt_id,export_receipt_id,model_receipt_id,
                    carrier_layer,intended_use,model_abi,selection_profile,
                    policy_epoch,withdrawal_generation,policy_version,allowed,
                    receipt_sha256,owner_configuration_sha256
             FROM cmd2_source_navigation_policy_current
             WHERE domain=$1 AND consumer_id=$2 FOR SHARE",
            &[&domain, &consumer_id],
        )
        .map_err(|_| policy_error("current source navigation policy unavailable"))?
        .ok_or_else(|| policy_error("current source navigation policy is absent"))?;
    let policy_epoch = row.get::<_, i64>(8);
    let withdrawal_generation = row.get::<_, i64>(9);
    let policy_version = row.get::<_, i64>(10);
    let allowed = row.get::<_, bool>(11);
    let row_receipt_sha256: String = row.get(12);
    let row_owner_sha256: String = row.get(13);
    if !allowed
        || policy_epoch <= 0
        || withdrawal_generation < 0
        || policy_version <= 0
        || row.get::<_, String>(0).trim_end() != receipt.issuer_ref.as_str()
        || row.get::<_, String>(1).trim_end() != receipt.receipt_id.as_str()
        || row.get::<_, String>(2).trim_end() != receipt.export_receipt_id.as_str()
        || row.get::<_, String>(3).trim_end() != receipt.model_receipt_id.as_str()
        || row.get::<_, String>(4).trim_end() != CARRIER_LAYER
        || row.get::<_, String>(5).trim_end() != INTENDED_USE
        || row.get::<_, String>(6).trim_end() != receipt.model_abi.as_str()
        || row.get::<_, String>(7).trim_end() != receipt.selection_profile.as_str()
        || u64::try_from(policy_epoch).ok() != Some(receipt.policy_epoch)
        || u64::try_from(withdrawal_generation).ok() != Some(receipt.withdrawal_generation)
        || row_receipt_sha256.trim_end() != receipt_sha256.to_hex()
        || row_owner_sha256.trim_end() != owner_configuration_sha256.to_hex()
        || receipt.model_receipt_id.as_str() != selected_model_receipt_id
        || receipt.model_abi.as_str() != binding.model_abi
        || receipt.selection_profile.as_str() != binding.selection_profile
        || receipt.export_receipt_id.is_empty()
        || corpus_revision.is_empty()
    {
        return Err(policy_error(
            "current source navigation policy scope differs",
        ));
    }
    let scope = DisclosureScope {
        operation_id: "tos.source.descend".into(),
        carrier_layer: CARRIER_LAYER.into(),
        intended_use: INTENDED_USE.into(),
        selected_binding: binding.clone(),
        corpus_revision: corpus_revision.to_owned(),
        data_revision: binding.index_root.to_hex(),
        selected_export_receipt_id: receipt.export_receipt_id.clone(),
        selected_model_receipt_id: receipt.model_receipt_id.clone(),
        policy_issuer_ref: receipt.issuer_ref.clone(),
        policy_receipt_id: receipt.receipt_id.clone(),
        policy_epoch: policy_epoch.to_string(),
        withdrawal_generation: withdrawal_generation.to_string(),
    };
    Ok((
        scope.clone(),
        PgSourceNavigationCurrentPolicyV1 {
            scope,
            policy_version: u64::try_from(policy_version)
                .map_err(|_| policy_error("current source navigation policy version invalid"))?,
        },
    ))
}

pub struct PgSourceNavigationCurrentPolicyV1 {
    scope: DisclosureScope,
    policy_version: u64,
}

impl CurrentPolicy for PgSourceNavigationCurrentPolicyV1 {
    fn authorize_current(
        &mut self,
        scope: &DisclosureScope,
        record: &RawRecord,
    ) -> Result<Charged, QueryError> {
        if scope != &self.scope
            || record.binding != self.scope.selected_binding
            || Digest256::of_bytes(&record.raw) != record.sha256
        {
            return Err(policy_error("source navigation policy input changed"));
        }
        Ok(Charged::default())
    }

    fn acquire_disclosure(
        &mut self,
        scope: &DisclosureScope,
        selected: &[RawRecord],
    ) -> Result<Box<dyn DisclosureLease>, QueryError> {
        if scope != &self.scope
            || selected.iter().any(|record| {
                record.binding != self.scope.selected_binding
                    || Digest256::of_bytes(&record.raw) != record.sha256
            })
        {
            return Err(policy_error("source navigation disclosure set changed"));
        }
        Ok(Box::new(PgSourceNavigationDisclosureLeaseV1 {
            scope: self.scope.clone(),
            policy_version: self.policy_version,
        }))
    }
}

struct PgSourceNavigationDisclosureLeaseV1 {
    scope: DisclosureScope,
    policy_version: u64,
}

impl DisclosureLease for PgSourceNavigationDisclosureLeaseV1 {
    fn recheck(&mut self) -> Result<(), QueryError> {
        // The current-row FOR SHARE lock is held by the surrounding RR
        // transaction; a policy writer cannot commit a withdrawal until this
        // lease and final transport callback return.
        if self.policy_version == 0
            || self.scope.policy_epoch.is_empty()
            || self.scope.withdrawal_generation.is_empty()
        {
            return Err(policy_error(
                "source navigation disclosure fence is invalid",
            ));
        }
        Ok(())
    }
}

fn policy_error(message: &'static str) -> QueryError {
    QueryError {
        code: QueryErrorCode::PolicyDenied,
        message,
    }
}

/// Validate a receipt's exact selected export id using the native original
/// navigation receipt. The component root commits the original header, rows,
/// rights and source-member index as one selected export.
pub fn selected_export_receipt_id(component_root_sha256: &str) -> Result<String, QueryError> {
    let digest = Digest256::from_hex(component_root_sha256)
        .map_err(|_| policy_error("selected source export receipt is invalid"))?;
    Ok(format!("sha256:{}", digest.to_hex()))
}

/// Parse the protected nested policy object with the published strict JSON
/// grammar before serde denies unknown policy fields.
pub fn parse_policy_object(
    value: &Value,
) -> Result<(SourceNavigationPolicyReceiptV1, Digest256), QueryError> {
    let raw = serde_json::to_vec(value)
        .map_err(|_| policy_error("source navigation policy receipt encoding failed"))?;
    let parsed = parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: 16 * 1024,
            max_depth: 16,
            max_visits: 256,
            max_integer_digits: 20,
        },
    )
    .map_err(|_| policy_error("source navigation policy receipt invalid"))?;
    let _ = parsed;
    SourceNavigationPolicyReceiptV1::parse(value)
        .map_err(|_| policy_error("source navigation policy receipt invalid"))
}
