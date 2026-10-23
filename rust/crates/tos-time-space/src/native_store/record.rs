use crate::DomainError;
use tos_foundation::{
    CanonicalProfile, ExactRecordRef, FoundationErrorCode, JsonDocument, JsonLimits,
    JsonMode, JsonValue, canonical_digest_v1,
};

/// Select the actual source owner's identity fields explicitly. No fallback
/// from Claim identity to Record identity or from current to latest is allowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeIdentityFields { Record, Claim }

/// Content/identity checked, NOT source-authorized, schema-admitted or a TS
/// value/profile. The borrowed original document keeps every field/provenance.
pub struct NativeTsRecord<'a> {
    exact: &'a ExactRecordRef,
    document: &'a JsonDocument,
    fields: NativeIdentityFields,
}
impl<'a> NativeTsRecord<'a> {
    pub fn exact(&self) -> &'a ExactRecordRef { self.exact }
    pub fn document(&self) -> &'a JsonDocument { self.document }
    pub fn identity_fields(&self) -> NativeIdentityFields { self.fields }
}

/// Actual native source-content seam: PublishedStrict document + exact source
/// id/version + the maintained SourceRecordDigestV1, rather than a raw-file SHA.
/// The caller retains parsing/source holders and supplies the foundation codec
/// limits for this operation; this helper does not reset a shared host budget.
/// It makes no claim of source-cut membership/completeness or TS applicability.
pub fn check_native_ts_record<'a>(
    document: &'a JsonDocument,
    expected: &'a ExactRecordRef,
    fields: NativeIdentityFields,
    digest_profile: CanonicalProfile,
    codec_limits: JsonLimits,
) -> Result<NativeTsRecord<'a>, DomainError> {
    if digest_profile != CanonicalProfile::SourceRecordDigestV1 {
        return Err(DomainError::Unsupported("native source record digest profile"));
    }
    if document.mode() != JsonMode::PublishedStrict {
        return Err(DomainError::Invalid("native source must preserve strict duplicate rejection"));
    }
    if expected.id.as_str().len() > codec_limits.max_bytes {
        return Err(DomainError::ResourceLimit);
    }
    // Perform the bounded common canonical traversal before follow-up field
    // scans, so its visits/bytes/depth/integer limits also bound those scans.
    let actual = canonical_digest_v1(document.root(), digest_profile, codec_limits)
        .map_err(codec_error)?;
    if actual != expected.digest {
        return Err(DomainError::Incompatible("native source canonical digest"));
    }
    finish_binding(document, expected, fields)
}
pub(super) fn codec_error(e: tos_foundation::FoundationError) -> DomainError {
    match e.code {
        FoundationErrorCode::BudgetExceeded => DomainError::ResourceLimit,
        FoundationErrorCode::UnsupportedCanonicalNumber | FoundationErrorCode::UnsupportedFormat =>
            DomainError::Unsupported("native source canonical representation"),
        _ => DomainError::Invalid("native source canonical record"),
    }
}
pub(super) fn finish_binding<'a>(document: &'a JsonDocument, expected: &'a ExactRecordRef,
    fields: NativeIdentityFields) -> Result<NativeTsRecord<'a>, DomainError> {
    let entries = document.root().as_object()
        .ok_or(DomainError::Invalid("native source record object"))?;
    let (id_key, version_key) = match fields {
        NativeIdentityFields::Record => ("record_id", "record_version"),
        NativeIdentityFields::Claim => ("claim_id", "claim_version"),
    };
    let mut id = None;
    let mut version = None;
    for (key, value) in entries {
        if key.as_str() == Some(id_key) { id = value.as_str(); }
        if key.as_str() == Some(version_key) {
            // as_u64 alone accepts spellings such as +1 if constructed by hand;
            // PublishedStrict source and the common codec own number grammar.
            version = match value { JsonValue::Number(_) => value.as_u64(), _ => None };
        }
    }
    if id != Some(expected.id.as_str()) || version != Some(expected.version.get()) {
        return Err(DomainError::Incompatible("native source identity/version"));
    }
    Ok(NativeTsRecord { exact: expected, document, fields })
}
