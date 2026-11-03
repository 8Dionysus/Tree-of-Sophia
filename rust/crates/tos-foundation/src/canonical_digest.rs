//! Tagged canonical JSON digests for forward writers and historical readers.

use crate::{
    CanonicalProfile, Digest256, FoundationError, FoundationErrorCode, JsonLimits, JsonValue,
    Result, jcs_bytes_v1, legacy_profiles,
};

/// The wire tag for RFC 8785 canonical JSON with the ToS admission policy.
pub const JCS_DIGEST_TAG: &str = "jcs1";

/// A digest of JCS bytes without a carrier line feed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalDigestV1 {
    pub profile: &'static str,
    pub digest: Digest256,
}

/// Produce a tagged digest for a new canonical JSON value.
pub fn jcs_digest_v1(value: &JsonValue, limits: JsonLimits) -> Result<CanonicalDigestV1> {
    let bytes = jcs_bytes_v1(value, limits)?;
    require_safe_integers(value)?;
    Ok(CanonicalDigestV1 {
        profile: JCS_DIGEST_TAG,
        digest: Digest256::of_bytes(&bytes),
    })
}

fn require_safe_integers(value: &JsonValue) -> Result<()> {
    match value {
        JsonValue::Number(number)
            if number
                .lexeme
                .parse::<f64>()
                .is_ok_and(|n| n.is_finite() && n.abs() > 9_007_199_254_740_992.0) =>
        {
            Err(FoundationError::new(
                FoundationErrorCode::UnsupportedCanonicalNumber,
                "numbers beyond 2^53 must be encoded as strings in new records",
            ))
        }
        JsonValue::Array(items) => {
            for item in items {
                require_safe_integers(item)?;
            }
            Ok(())
        }
        JsonValue::Object(fields) => {
            for (_, item) in fields {
                require_safe_integers(item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Verify a digest whose untagged historical profile is known by its owner.
///
/// A tag selects exactly one profile; verification never retries another
/// profile after a mismatch. The legacy corpus snapshot profile includes LF;
/// `jcs1` covers JCS bytes alone.
pub fn verify_canonical_digest_v1(
    value: &JsonValue,
    profile: Option<&str>,
    historical_profile: CanonicalProfile,
    expected: Digest256,
    limits: JsonLimits,
) -> Result<bool> {
    match profile {
        Some(JCS_DIGEST_TAG) => Ok(jcs_digest_v1(value, limits)?.digest == expected),
        None => legacy_profiles::verify(value, historical_profile, expected, limits),
        Some(tag) if tag == historical_profile.as_str() => {
            legacy_profiles::verify(value, historical_profile, expected, limits)
        }
        Some(_) => Err(FoundationError::new(
            FoundationErrorCode::UnsupportedFormat,
            "unknown canonical digest profile tag",
        )),
    }
}
