//! Participating metadata-publication epoch. This does not certify source
//! membership or protect against an independently mutating same-UID writer.
//! Callers own held-descriptor reads, identity/mode checks and the 8 KiB input bound.
use crate::{Result, StoreError, StoreErrorCode};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, JsonValue, canonical_bytes_v1};

const STATE_SCHEMA: &str = "tos_source_metadata_publication_v1";
const MAX_GENERATION: u64 = 9_007_199_254_740_991;
fn invalid(detail: &'static str) -> StoreError {
    StoreError::new(StoreErrorCode::InvalidCanonicalSnapshot, detail)
}
fn canonical(value: &JsonValue) -> Result<Vec<u8>> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| invalid("source canonical input"))
}
fn digest(value: &JsonValue) -> Result<String> {
    Ok(Digest256::of_bytes(&canonical(value)?).to_prefixed())
}
fn hash(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn field<'a>(v: &'a JsonValue, k: &str) -> Result<&'a JsonValue> {
    v.object_get(k).ok_or(invalid("missing object field"))
}
fn text<'a>(v: &'a JsonValue, k: &str) -> Result<&'a str> {
    field(v, k)?.as_str().ok_or(invalid("expected text field"))
}
fn integer(v: &JsonValue, k: &str) -> Result<u64> {
    field(v, k)?
        .as_u64()
        .ok_or(invalid("expected integer field"))
}
fn exact_keys(v: &JsonValue, keys: &[&str]) -> Result<()> {
    let members = v.as_object().ok_or(invalid("expected object"))?;
    if members.len() != keys.len()
        || members
            .iter()
            .any(|(k, _)| !keys.contains(&k.as_str().unwrap_or("")))
    {
        return Err(invalid("unexpected object fields"));
    }
    Ok(())
}
/// Validate the existing pending or ready wire state, including its canonical token.
pub fn validate_metadata_publication(value: &JsonValue) -> Result<()> {
    exact_keys(
        value,
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
    let generation = integer(value, "generation")?;
    let transition = text(value, "transition_id")?;
    let phase = text(value, "phase")?;
    let token = text(value, "token")?;
    if text(value, "schema_version")? != STATE_SCHEMA
        || !(1..=MAX_GENERATION).contains(&generation)
        || transition.len() != 32
        || !transition
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || !matches!(phase, "pending" | "ready")
        || !hash(text(value, "transaction_id")?)
        || !hash(text(value, "manifest_sha256")?)
        || !hash(token)
    {
        return Err(invalid("selected metadata publication control"));
    }
    let outcome = field(value, "outcome")?;
    let renewal = field(value, "recovery_authorization")?;
    if phase == "pending" {
        if outcome != &JsonValue::Null || renewal != &JsonValue::Null {
            return Err(invalid("pending publication terminal fields"));
        }
    } else if !matches!(outcome.as_str(), Some("committed" | "rolled-back")) {
        return Err(invalid("terminal publication outcome"));
    }
    if renewal != &JsonValue::Null
        && (renewal.as_object().is_none() || canonical(renewal)?.len() > 4096)
    {
        return Err(invalid("publication recovery authorization"));
    }
    let without_token = JsonValue::Object(
        value
            .as_object()
            .ok_or(invalid("publication object"))?
            .iter()
            .filter(|(key, _)| key.as_str() != Some("token"))
            .cloned()
            .collect(),
    );
    if digest(&without_token)? != token {
        return Err(invalid("publication token digest"));
    }
    Ok(())
}

/// A ready epoch selected before reading participating live source members.
/// Absence is the existing legacy epoch, not proof of a complete snapshot.
#[derive(Clone)]
pub struct MetadataPublicationEpoch {
    state: Option<JsonValue>,
    token: Option<String>,
    generation: u64,
}
impl MetadataPublicationEpoch {
    pub fn select(state: Option<JsonValue>) -> Result<Self> {
        if let Some(value) = state.as_ref() {
            validate_metadata_publication(value)?;
            if text(value, "phase")? != "ready" {
                return Err(StoreError::new(
                    StoreErrorCode::RevisionMismatch,
                    "selected metadata publication pending",
                ));
            }
        }
        let token = state
            .as_ref()
            .map(|v| text(v, "token").map(str::to_owned))
            .transpose()?;
        let generation = state
            .as_ref()
            .map(|v| integer(v, "generation"))
            .transpose()?
            .unwrap_or(0);
        Ok(Self {
            state,
            token,
            generation,
        })
    }
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn member_binding(&self) -> Result<Option<(Digest256, u64)>> {
        self.state
            .as_ref()
            .map(|value| {
                let mut bytes = canonical(value)?;
                bytes.push(b'\n');
                Ok((Digest256::of_bytes(&bytes), bytes.len() as u64))
            })
            .transpose()
    }
    /// Revalidate the physical reader's newly selected control after the read.
    pub fn verify_current(&self, current: Option<JsonValue>) -> Result<()> {
        let current = Self::select(current).map_err(|error| {
            if error.code == StoreErrorCode::RevisionMismatch {
                StoreError::new(
                    StoreErrorCode::RevisionMismatch,
                    "selected metadata publication changed",
                )
            } else {
                error
            }
        })?;
        let same = match (&self.state, &current.state) {
            (None, None) => true,
            (Some(a), Some(b)) => canonical(a)? == canonical(b)?,
            _ => false,
        };
        if !same {
            return Err(StoreError::new(
                StoreErrorCode::RevisionMismatch,
                "selected metadata publication changed",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tos_foundation::{JsonNumber, JsonNumberKind, JsonString};

    fn text_value(value: &str) -> JsonValue {
        JsonValue::String(JsonString::from_utf8(value))
    }
    fn control(generation: u64, pending: bool) -> JsonValue {
        let hash = Digest256::of_bytes(b"publication").to_prefixed();
        let mut members = vec![
            ("schema_version", text_value(STATE_SCHEMA)),
            (
                "generation",
                JsonValue::Number(JsonNumber {
                    kind: JsonNumberKind::Int,
                    lexeme: generation.to_string(),
                }),
            ),
            (
                "transition_id",
                text_value("0123456789abcdef0123456789abcdef"),
            ),
            (
                "phase",
                text_value(if pending { "pending" } else { "ready" }),
            ),
            ("transaction_id", text_value(&hash)),
            ("manifest_sha256", text_value(&hash)),
            (
                "outcome",
                if pending {
                    JsonValue::Null
                } else {
                    text_value("committed")
                },
            ),
            ("recovery_authorization", JsonValue::Null),
        ]
        .into_iter()
        .map(|(key, value)| (JsonString::from_utf8(key), value))
        .collect::<Vec<_>>();
        let token = digest(&JsonValue::Object(members.clone())).unwrap();
        members.push((JsonString::from_utf8("token"), text_value(&token)));
        JsonValue::Object(members)
    }

    #[test]
    fn cooperating_reader_rejects_pending_replacement_and_disappearance() {
        let epoch = MetadataPublicationEpoch::select(Some(control(1, false))).unwrap();
        epoch.verify_current(Some(control(1, false))).unwrap();
        for changed in [Some(control(2, false)), Some(control(2, true)), None] {
            assert_eq!(
                epoch.verify_current(changed).unwrap_err().code,
                StoreErrorCode::RevisionMismatch
            );
        }
        let absent = MetadataPublicationEpoch::select(None).unwrap();
        absent.verify_current(None).unwrap();
        assert!(absent.verify_current(Some(control(1, false))).is_err());
        assert!(MetadataPublicationEpoch::select(Some(control(1, true))).is_err());
        // Writers may validate a pending head; ordinary readers may not select it.
        validate_metadata_publication(&control(1, true)).unwrap();
    }

    #[test]
    fn token_authenticates_state_and_member_binding_keeps_wire_newline() {
        let state = control(1, false);
        let epoch = MetadataPublicationEpoch::select(Some(state.clone())).unwrap();
        let mut wire = canonical(&state).unwrap();
        wire.push(b'\n');
        assert_eq!(
            epoch.member_binding().unwrap(),
            Some((Digest256::of_bytes(&wire), wire.len() as u64))
        );
        let JsonValue::Object(mut members) = state else {
            unreachable!()
        };
        members
            .iter_mut()
            .find(|(key, _)| key.as_str() == Some("outcome"))
            .unwrap()
            .1 = text_value("rolled-back");
        assert_eq!(
            validate_metadata_publication(&JsonValue::Object(members))
                .unwrap_err()
                .detail,
            "publication token digest"
        );
    }
}
