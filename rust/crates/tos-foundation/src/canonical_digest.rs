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
        JsonValue::Number(number) if crate::jcs::requires_jcs1_string(number) => {
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

/// Profile carried with an in-memory canonical digest through owner validation.
/// Only JCS1 has a current wire tag; legacy values resolve through their field
/// family's registry entry when a stored untagged digest is read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalDigestProfileV1 {
    Jcs1,
    Legacy(CanonicalProfile),
}

impl CanonicalDigestProfileV1 {
    /// The profile tag to persist with a newly written digest.
    pub const fn wire_tag(self) -> Option<&'static str> {
        match self {
            Self::Jcs1 => Some(JCS_DIGEST_TAG),
            Self::Legacy(_) => None,
        }
    }

    /// Resolve a stored tag using the registry's historical profile for this
    /// field family. Unknown and conflicting tags fail closed.
    pub fn from_stored_tag(
        tag: Option<&str>,
        historical_profile: CanonicalProfile,
    ) -> Result<Self> {
        match tag {
            None => Ok(Self::Legacy(historical_profile)),
            Some(JCS_DIGEST_TAG) => Ok(Self::Jcs1),
            Some(tag) if tag == historical_profile.as_str() => Ok(Self::Legacy(historical_profile)),
            Some(_) => Err(FoundationError::new(
                FoundationErrorCode::UnsupportedFormat,
                "canonical digest profile differs from its field family",
            )),
        }
    }

    /// Verify with this exact profile, without trying a fallback after a
    /// mismatch. A legacy digest always uses its registry-selected owner.
    pub fn verify(
        self,
        value: &JsonValue,
        stored_tag: Option<&str>,
        historical_profile: CanonicalProfile,
        expected: Digest256,
        limits: JsonLimits,
    ) -> Result<bool> {
        match self {
            Self::Jcs1 if stored_tag == Some(JCS_DIGEST_TAG) => {
                verify_canonical_digest_v1(value, stored_tag, historical_profile, expected, limits)
            }
            Self::Jcs1 => Err(FoundationError::new(
                FoundationErrorCode::UnsupportedFormat,
                "JCS digest profile tag is absent or mismatched",
            )),
            Self::Legacy(profile)
                if profile == historical_profile
                    && stored_tag.is_none_or(|tag| tag == profile.as_str()) =>
            {
                verify_canonical_digest_v1(value, stored_tag, profile, expected, limits)
            }
            Self::Legacy(_) => Err(FoundationError::new(
                FoundationErrorCode::UnsupportedFormat,
                "legacy digest profile or tag differs from its field family",
            )),
        }
    }
}

#[cfg(test)]
mod profile_tests {
    use super::{CanonicalDigestProfileV1, JCS_DIGEST_TAG, jcs_digest_v1};
    use crate::{
        CanonicalProfile, Digest256, JsonLimits, JsonMode, canonical_bytes_v1, jcs_bytes_v1,
        parse_json,
    };

    #[test]
    fn typed_profiles_select_jcs_or_one_registry_legacy_profile() {
        let limits = JsonLimits::default();
        let document = parse_json(
            r#"{"😀":1,"\uE000":2}"#.as_bytes(),
            JsonMode::PublishedStrict,
            limits,
        )
        .unwrap();
        let legacy =
            CanonicalDigestProfileV1::from_stored_tag(None, CanonicalProfile::SourceCommandInputV1)
                .unwrap();
        let old_bytes = canonical_bytes_v1(
            document.root(),
            CanonicalProfile::SourceCommandInputV1,
            limits,
        )
        .unwrap();
        let jcs_bytes = jcs_bytes_v1(document.root(), limits).unwrap();
        assert_ne!(old_bytes, jcs_bytes);
        let legacy_digest = Digest256::of_bytes(&old_bytes);
        assert!(
            legacy
                .verify(
                    document.root(),
                    None,
                    CanonicalProfile::SourceCommandInputV1,
                    legacy_digest,
                    limits
                )
                .unwrap()
        );
        assert_eq!(legacy.wire_tag(), None);

        let tagged = CanonicalDigestProfileV1::from_stored_tag(
            Some(JCS_DIGEST_TAG),
            CanonicalProfile::SourceCommandInputV1,
        )
        .unwrap();
        let jcs_digest = jcs_digest_v1(document.root(), limits).unwrap().digest;
        assert!(
            tagged
                .verify(
                    document.root(),
                    Some(JCS_DIGEST_TAG),
                    CanonicalProfile::SourceCommandInputV1,
                    jcs_digest,
                    limits
                )
                .unwrap()
        );
        assert!(
            !tagged
                .verify(
                    document.root(),
                    Some(JCS_DIGEST_TAG),
                    CanonicalProfile::SourceCommandInputV1,
                    legacy_digest,
                    limits
                )
                .unwrap()
        );
        assert_eq!(tagged.wire_tag(), Some(JCS_DIGEST_TAG));

        assert!(
            CanonicalDigestProfileV1::from_stored_tag(
                Some("unrecognized"),
                CanonicalProfile::SourceCommandInputV1,
            )
            .is_err()
        );
        assert!(
            CanonicalDigestProfileV1::from_stored_tag(
                Some("tos_source_record_digest_v1"),
                CanonicalProfile::SourceCommandInputV1,
            )
            .is_err()
        );
        assert!(
            tagged
                .verify(
                    document.root(),
                    Some("tos_source_command_input_v1"),
                    CanonicalProfile::SourceCommandInputV1,
                    jcs_digest,
                    limits,
                )
                .is_err()
        );
    }
}
