//! Maintained source command preparation, separate from transaction admission.
//!
//! Inputs are exact selected source bytes and owner configuration bytes. A plan
//! is neither an authority lease nor a VAL attestation. The real source delta
//! must be staged, checked against the complete source cut and committed under
//! the current owner fence before it may replace canonical files.

use std::collections::BTreeSet;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonEmissionProfile, JsonLimits, JsonMode, JsonNumber,
    JsonNumberKind, JsonString, JsonValue, RelativePath, SourceRevision, canonical_bytes_v1,
    emit_json_profile, parse_json,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceCommandError {
    Invalid(&'static str),
    Conflict(&'static str),
    Denied(&'static str),
    Unsupported(&'static str),
    MissingProductionAdmission,
}
pub type SourceCommandResult<T> = Result<T, SourceCommandError>;

/// Match the source owner's aware Python `_instant` comparison. The parser
/// remains with VAL's existing source chronology implementation; this supplies
/// no clock authority and does not reread a protected configuration.
pub fn validate_expiry(expires_at: &str, observed_now: &str) -> SourceCommandResult<()> {
    let order = tos_validation::retirement_rules::observed_instant_order(expires_at, observed_now)
        .map_err(|_| {
            SourceCommandError::Invalid("owner instant requires explicit valid timezone")
        })?;
    if order != std::cmp::Ordering::Greater {
        return Err(SourceCommandError::Denied("owner delegation expired"));
    }
    Ok(())
}

pub fn validate_instant(value: &str) -> SourceCommandResult<()> {
    tos_validation::retirement_rules::observed_instant_order(value, value)
        .map(|_| ())
        .map_err(|_| SourceCommandError::Invalid("instant requires explicit valid timezone"))
}

/// Exact bytes of an explicitly selected canonical member. No path discovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    pub path: RelativePath,
    pub raw: Vec<u8>,
}

/// The independently selected protected configuration remains outside the
/// authored source cut. This value records observations, not account authority.
#[derive(Clone, Debug)]
pub struct CommandContext {
    pub base_revision: SourceRevision,
    pub configuration_raw: Vec<u8>,
    pub request_raw: Vec<u8>,
    pub recorded_at: String,
    pub effective_uid: u64,
    pub files: Vec<SourceFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceChange {
    pub path: RelativePath,
    pub before: Option<Digest256>,
    pub after: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDependency {
    pub path: RelativePath,
    pub raw_sha256: Digest256,
}

/// Owner-computed proposal bytes, including retained predecessors and receipts.
/// Public fields intentionally permit proposal construction, never admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedCommand {
    pub handler_id: String,
    pub operation: String,
    pub base_revision: SourceRevision,
    pub request_canonical_sha256: Digest256,
    pub configuration_raw_sha256: Digest256,
    pub configuration_canonical_sha256: Digest256,
    pub response: JsonValue,
    pub changes: Vec<SourceChange>,
    pub reads: Vec<SourceDependency>,
    pub replayed: bool,
}

impl CommandContext {
    pub fn file(&self, path: &RelativePath) -> SourceCommandResult<Option<&[u8]>> {
        let mut selected = self.files.iter().filter(|f| f.path == *path);
        let result = selected.next().map(|f| f.raw.as_slice());
        if selected.next().is_some() {
            return Err(SourceCommandError::Invalid(
                "duplicate selected source path",
            ));
        }
        Ok(result)
    }
    pub fn check(&self) -> SourceCommandResult<()> {
        if self.configuration_raw.len() > 1_048_576 || self.request_raw.len() > 1_048_576 {
            return Err(SourceCommandError::Invalid("command input byte budget"));
        }
        let total = self
            .files
            .iter()
            .try_fold(0usize, |total, file| total.checked_add(file.raw.len()));
        if self.files.len() > 4096 || total.is_none_or(|total| total > 33_554_432) {
            return Err(SourceCommandError::Invalid("selected source byte budget"));
        }
        let paths: BTreeSet<_> = self.files.iter().map(|f| &f.path).collect();
        if paths.len() != self.files.len() {
            return Err(SourceCommandError::Invalid(
                "duplicate selected source path",
            ));
        }
        if self.recorded_at.is_empty() {
            return Err(SourceCommandError::Invalid(
                "missing observed receipt instant",
            ));
        }
        Ok(())
    }
    pub fn plan(
        &self,
        handler: &str,
        response: JsonValue,
        mut changes: Vec<SourceChange>,
        replayed: bool,
    ) -> SourceCommandResult<PreparedCommand> {
        self.check()?;
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        if changes.windows(2).any(|p| p[0].path == p[1].path) {
            return Err(SourceCommandError::Invalid("duplicate proposed write path"));
        }
        for change in &changes {
            if self.file(&change.path)?.map(Digest256::of_bytes) != change.before {
                return Err(SourceCommandError::Conflict(
                    "proposed before bytes differ from selected source",
                ));
            }
            if change
                .after
                .as_ref()
                .is_some_and(|raw| raw.len() > 8_388_608)
            {
                return Err(SourceCommandError::Invalid(
                    "proposed source member byte budget",
                ));
            }
        }
        let request = parse(&self.request_raw)?;
        let config = parse(&self.configuration_raw)?;
        Ok(PreparedCommand {
            handler_id: handler.into(),
            operation: text(&request, "operation")?.into(),
            base_revision: self.base_revision,
            request_canonical_sha256: Digest256::of_bytes(&canonical(&request)?),
            configuration_raw_sha256: Digest256::of_bytes(&self.configuration_raw),
            configuration_canonical_sha256: Digest256::of_bytes(&canonical(&config)?),
            response,
            changes,
            reads: self
                .files
                .iter()
                .map(|f| SourceDependency {
                    path: f.path.clone(),
                    raw_sha256: Digest256::of_bytes(&f.raw),
                })
                .collect(),
            replayed,
        })
    }
}

impl PreparedCommand {
    /// Explicit refusal until a genuine full-rule issuer and current protected
    /// owner fence can be consumed by the canonical source coordinator. Neither
    /// the synthetic PG attestation nor a family report can select this writer.
    pub fn commit(&self) -> SourceCommandResult<()> {
        Err(SourceCommandError::MissingProductionAdmission)
    }
}

pub(crate) fn parse(raw: &[u8]) -> SourceCommandResult<JsonValue> {
    parse_json(
        raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map(|doc| doc.into_root())
    .map_err(|_| SourceCommandError::Invalid("strict JSON input"))
}
pub(crate) fn canonical(value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Invalid("source canonical input"))
}
pub(crate) fn published(value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
    emit_json_profile(
        value,
        JsonEmissionProfile::SourceFormSetPublishedV1,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map(|encoded| encoded.bytes)
    .map_err(|_| SourceCommandError::Invalid("published source bytes"))
}
pub(crate) fn record_digest(value: &JsonValue) -> SourceCommandResult<Digest256> {
    Ok(Digest256::of_bytes(&canonical(value)?))
}
pub(crate) fn object(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
pub(crate) fn string(s: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(s))
}
pub(crate) fn number(n: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
pub(crate) fn field<'a>(v: &'a JsonValue, k: &str) -> SourceCommandResult<&'a JsonValue> {
    v.object_get(k)
        .ok_or(SourceCommandError::Invalid("missing object field"))
}
pub(crate) fn text<'a>(v: &'a JsonValue, k: &str) -> SourceCommandResult<&'a str> {
    field(v, k)?
        .as_str()
        .ok_or(SourceCommandError::Invalid("expected text field"))
}
pub(crate) fn integer(v: &JsonValue, k: &str) -> SourceCommandResult<u64> {
    field(v, k)?
        .as_u64()
        .ok_or(SourceCommandError::Invalid("expected integer field"))
}
pub(crate) fn array<'a>(v: &'a JsonValue, k: &str) -> SourceCommandResult<&'a [JsonValue]> {
    field(v, k)?
        .as_array()
        .ok_or(SourceCommandError::Invalid("expected array field"))
}
pub(crate) fn exact_keys(v: &JsonValue, keys: &[&str]) -> SourceCommandResult<()> {
    let members = v
        .as_object()
        .ok_or(SourceCommandError::Invalid("expected object"))?;
    if members.len() != keys.len()
        || members
            .iter()
            .any(|(k, _)| !keys.contains(&k.as_str().unwrap_or("")))
    {
        return Err(SourceCommandError::Invalid("unexpected object fields"));
    }
    Ok(())
}
pub(crate) fn set(v: &mut JsonValue, k: &str, new: JsonValue) -> SourceCommandResult<()> {
    let JsonValue::Object(members) = v else {
        return Err(SourceCommandError::Invalid("expected object"));
    };
    if let Some((_, old)) = members.iter_mut().find(|(key, _)| key.as_str() == Some(k)) {
        *old = new;
    } else {
        members.push((JsonString::from_utf8(k), new));
    }
    Ok(())
}
pub(crate) fn same(a: &JsonValue, b: &JsonValue) -> SourceCommandResult<bool> {
    Ok(canonical(a)? == canonical(b)?)
}
pub(crate) fn reference(v: &JsonValue, id: &str, version: &str) -> SourceCommandResult<JsonValue> {
    Ok(object(vec![
        ("id", string(text(v, id)?)),
        ("version", number(integer(v, version)?)),
        ("digest", string(&record_digest(v)?.to_prefixed())),
    ]))
}
