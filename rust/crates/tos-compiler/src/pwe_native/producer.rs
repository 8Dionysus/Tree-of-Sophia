//! B1/Q4. Resolve exact records first, then wrapper -> /body -> each item.
use super::{
    PreparedAccount,
    basis::{Error, Result, Session},
};
use serde_json::Value;
use tos_foundation::{
    ExecutionStatus, JsonValue, OperationBindingLimits, ValueDisposition, check_operation_binding,
};

/// Exactly projected borrowed codomain. It has no semantic-consumer method and
/// cannot be used as a current admission, identity edge or completeness receipt.
pub struct CheckedPreservedCandidate<'v> {
    record: &'v Value,
    body: &'v Value,
    values: Vec<PreparedAccount<'v>>,
}
impl<'v> CheckedPreservedCandidate<'v> {
    pub fn record(&self) -> &'v Value {
        self.record
    }
    pub fn body(&self) -> &'v Value {
        self.body
    }
    pub fn values(&self) -> &[PreparedAccount<'v>] {
        &self.values
    }
    pub fn semantic_resolution_executed(&self) -> bool {
        false
    }
}

const SCHEMAS: [(&str, &str, &str); 3] = [
    (
        "pwe.local.prepared-values-record-schema-specification",
        "sha256:429d58a8b8d2b2d92ec9204ec813880b232ba0defb0a7d8e4a7f9ac1d7107ddd",
        super::WRAPPER_SCHEMA,
    ),
    (
        "pwe.local.preserved-candidate-schema-specification",
        "sha256:4c163d2d2deb6f7827b17396e7f614dfb51ab9b1a573cdccfeb9b9698139a5a6",
        super::CONTAINER_SCHEMA,
    ),
    (
        "pwe.local.single-value-schema-specification",
        "sha256:1c6f7003a3c6d0381257461c44e3a4d6ade1899fcd7e7a4672a656aaaf2fc262",
        super::SINGLE_VALUE_SCHEMA,
    ),
];

impl Session<'_> {
    fn native_field<'v>(&mut self, v: &'v JsonValue, key: &str) -> Result<&'v JsonValue> {
        let xs = v.as_object().ok_or(Error::Malformed("native object"))?;
        for (k, v) in xs {
            let k = k.as_str().ok_or(Error::Malformed("native UTF-8 key"))?;
            if self.same_text(k, key)? {
                return Ok(v);
            }
        }
        Err(Error::Malformed("native field"))
    }
    fn native_ref_matches(&mut self, native: &JsonValue, record: &Value) -> Result<()> {
        let id = self
            .native_field(native, "id")?
            .as_str()
            .ok_or(Error::Malformed("native ID"))?;
        let version = self
            .native_field(native, "version")?
            .as_u64()
            .ok_or(Error::Malformed("native version"))?;
        let digest = self
            .native_field(native, "digest")?
            .as_str()
            .ok_or(Error::Malformed("native digest"))?;
        let actual = self.str_field(record, "record_id")?;
        if !self.same_text(actual, id)?
            || self.field(record, "record_version")?.as_u64() != Some(version)
            || self.digest(record)?
                != tos_foundation::Digest256::from_prefixed(digest)
                    .map_err(|_| Error::Malformed("native digest"))?
        {
            return Err(Error::ExactMismatch);
        }
        Ok(())
    }
    /// This bounded adapter supports the accepted *local* output profile v2.
    /// It does not register its urn schemas as native production source schemas.
    /// `schemas` are the actual exact records in wrapper/container/item order.
    pub fn preserved_candidate<'v>(
        &mut self,
        envelope: &JsonValue,
        k0: OperationBindingLimits,
        profile: &Value,
        record: &'v Value,
        schemas: [&Value; 3],
    ) -> Result<CheckedPreservedCandidate<'v>> {
        // K0 does not expose usage counters; precharge its complete declared
        // allowance before re-admission, instead of resetting a hidden budget.
        self.prepay_k0(k0)?;
        let checked = check_operation_binding(envelope, k0)
            .map_err(|_| Error::Upstream("K0 operation binding"))?;
        self.check()?;
        if checked.execution_status() != ExecutionStatus::Completed
            || checked.value_disposition() != ValueDisposition::Produced
        {
            return Err(Error::Unsupported("producer is not Completed/Produced"));
        }
        for v in [profile, record, schemas[0], schemas[1], schemas[2]] {
            self.admit(v)?;
        }
        self.native_ref_matches(
            checked
                .output_profile_ref()
                .ok_or(Error::Malformed("output profile"))?,
            profile,
        )?;
        self.native_ref_matches(
            checked.output_ref().ok_or(Error::Malformed("output ref"))?,
            record,
        )?;
        self.closed(profile, &["record_id", "record_version", "status", "body"])?;
        self.constant(profile, "record_id", "pwe.local.prepared-values-profile")?;
        if self.field(profile, "record_version")?.as_u64() != Some(2) {
            return Err(Error::Unsupported("output profile version"));
        }
        self.constant(profile, "status", "local-source-only-draft")?;
        let p = self.field(profile, "body")?;
        self.closed(
            p,
            &[
                "kind",
                "record_representation",
                "projection",
                "codomain",
                "item_contract",
                "authority_boundary",
            ],
        )?;
        self.constant(p, "kind", "output-profile")?;
        self.constant(p,"authority_boundary","output shape/projection only; no semantic resolution, native binding, admission or proof verification")?;
        let representation = self.field(p, "record_representation")?;
        self.closed(representation, &["schema_ref", "schema_id"])?;
        let projection = self.field(p, "projection")?;
        self.closed(
            projection,
            &[
                "syntax",
                "from",
                "pointer",
                "total_on_record_schema_ref",
                "output_schema_ref",
                "operation",
            ],
        )?;
        for (key, want) in [
            ("syntax", "json-pointer"),
            ("from", "resolved-exact-record-value"),
            ("pointer", "/body"),
            ("operation", "select property body without rewrite"),
        ] {
            self.constant(projection, key, want)?;
        }
        let codomain = self.field(p, "codomain")?;
        self.closed(codomain, &["kind", "schema_id", "schema_ref"])?;
        self.constant(codomain, "kind", "PreservedCandidate")?;
        let items = self.field(p, "item_contract")?;
        self.closed(
            items,
            &["array_pointer", "item_selection", "schema_id", "schema_ref"],
        )?;
        self.constant(items, "array_pointer", "/values")?;
        self.constant(items, "item_selection", "each-array-element")?;
        for (i, node) in [representation, codomain, items].into_iter().enumerate() {
            let (id, digest, schema_id) = SCHEMAS[i];
            self.constant(node, "schema_id", schema_id)?;
            let r = self.field(node, "schema_ref")?;
            let r = self.exact(r)?;
            if !self.same_text(r.id, id)?
                || r.version != 1
                || r.digest
                    != tos_foundation::Digest256::from_prefixed(digest)
                        .map_err(|_| Error::Malformed("compiled schema pin"))?
            {
                return Err(Error::Unsupported("unbound schema/profile revision"));
            }
            self.record_matches(schemas[i], r, "record_id", "record_version")?;
            // Actual schema bytes are digest-bound, not a self-declared schema_id.
        }
        let a = self.field(projection, "total_on_record_schema_ref")?;
        let b = self.field(representation, "schema_ref")?;
        if !self.equal(a, b)? {
            return Err(Error::ExactMismatch);
        }
        let a = self.field(projection, "output_schema_ref")?;
        let b = self.field(codomain, "schema_ref")?;
        if !self.equal(a, b)? {
            return Err(Error::ExactMismatch);
        }
        // Wrapper check and projection precede container and individual checks.
        self.closed(record, &["record_id", "record_version", "status", "body"])?;
        self.constant(record, "record_id", "pwe.local.prepared-pwe-values")?;
        if self
            .field(record, "record_version")?
            .as_u64()
            .filter(|v| *v > 0)
            .is_none()
        {
            return Err(Error::Malformed("wrapper version"));
        }
        self.constant(record, "status", "local-source-only-draft")?;
        let body = self.field(record, "body")?;
        self.closed(body, &["kind", "values", "semantic_resolution_executed"])?;
        self.constant(body, "kind", "PreservedCandidate")?;
        if self.field(body, "semantic_resolution_executed")?.as_bool() != Some(false) {
            return Err(Error::Malformed(
                "producer cannot declare semantic resolution",
            ));
        }
        let xs = self.field(body, "values")?;
        let xs = self.array(xs)?;
        let mut values = self.vec(xs.len())?;
        for value in xs {
            values.push(self.prepare_admitted(value)?);
        }
        self.check()?;
        Ok(CheckedPreservedCandidate {
            record,
            body,
            values,
        })
    }
    pub(super) fn constant(&mut self, v: &Value, key: &str, want: &str) -> Result<()> {
        let actual = self.str_field(v, key)?;
        if !self.same_text(actual, want)? {
            return Err(Error::Malformed("fixed profile constant"));
        }
        Ok(())
    }
}
