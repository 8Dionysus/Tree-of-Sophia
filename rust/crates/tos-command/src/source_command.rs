//! Maintained source command preparation, separate from transaction admission.
//!
//! Inputs are exact selected source bytes and owner configuration bytes. A plan
//! is neither an authority lease nor a VAL attestation. The real source delta
//! must be staged, checked against the complete source cut and committed under
//! the current owner fence before it may replace canonical files.

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonEmissionProfile, JsonLimits, JsonMode, JsonNumber,
    JsonNumberKind, JsonString, JsonValue, RelativePath, SourceRevision, canonical_bytes_v1,
    emit_json_profile, parse_json,
};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceCommandError {
    Invalid(&'static str),
    Conflict(&'static str),
    Denied(&'static str),
    /// Owned allow-listed public denial detail, retained through bounded output.
    DeniedWithReason(String),
    Unsupported(&'static str),
    SchemaExecution {
        path: String,
        root: String,
        reason: tos_validation::item_rules::ItemRefusal,
    },
    MissingProductionAdmission,
}
pub type SourceCommandResult<T> = Result<T, SourceCommandError>;

impl SourceCommandError {
    /// Preserve authored public reasons while keeping selected paths and roots
    /// inside their owner. The CLI's existing output envelope bounds emission.
    pub fn public_reason(&self) -> String {
        match self {
            Self::Invalid(reason) => format!("invalid: {reason}"),
            Self::Conflict(reason) => format!("conflict: {reason}"),
            Self::Denied(reason) => format!("denied: {reason}"),
            Self::DeniedWithReason(reason) => reason.clone(),
            Self::Unsupported(reason) => format!("unsupported: {reason}"),
            Self::SchemaExecution { reason, .. } => {
                crate::source_admission_spooled_index::receiver_refusal(reason.clone()).to_string()
            }
            Self::MissingProductionAdmission => "missing production admission".to_owned(),
        }
    }
}

impl std::fmt::Display for SourceCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.public_reason())
    }
}

impl std::error::Error for SourceCommandError {}

/// An IO carrier may contain an already bounded command/worker refusal. Other
/// IO text can contain private host paths; retain its kind and fingerprint.
pub(crate) fn public_io_reason(error: &std::io::Error) -> String {
    if let Some(reason) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<SourceCommandError>())
    {
        return reason.public_reason();
    }
    if let Some(reason) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<tos_validation::item_rules::ItemExecutorRefusal>())
    {
        return reason.summary();
    }
    let message = error.to_string();
    if crate::source_admission_spooled_index::is_bounded_source_cause(&message) {
        return message;
    }
    format!(
        "IO {:?}: {}",
        error.kind(),
        crate::source_admission_spooled_index::bounded_source_cause(
            "receiver-source",
            "command-io",
            &message,
        )
    )
}

/// Compiler refusal detail follows the same public/private boundary as command
/// errors: authored static guards survive, foreign source/SQL text is hashed.
pub(crate) fn public_compiler_reason(error: &tos_compiler::Error) -> String {
    use tos_compiler::Error;
    match error {
        Error::Invalid(reason) => format!("invalid compiler input: {reason}"),
        Error::PreparedUnsupported(reason) => {
            format!("unsupported local prepared carrier/profile: {reason}")
        }
        Error::ManagedSourceUnsupported(reason) => {
            format!("unsupported managed selected source: {reason}")
        }
        Error::Budget(reason) => format!("compiler budget exceeded: {reason}"),
        Error::SqliteVmBudget {
            phase,
            used_steps,
            max_steps,
        } => format!(
            "compiler budget exceeded: SQLite VM steps in {phase:?} (used {used_steps}, max {max_steps})"
        ),
        Error::FoundationJson { .. } => error.to_string(),
        Error::ControlledColdClose { close, .. } => compiler_sql_cause(error, "cold-close", close),
        Error::Io(error) => public_io_reason(error),
        Error::Source(reason)
            if crate::source_admission_spooled_index::is_bounded_source_cause(reason) =>
        {
            reason.clone()
        }
        Error::Sql(sql) => compiler_sql_cause(error, "sql", sql),
        Error::SqlitePhase { phase, error: sql } => {
            compiler_sql_cause(error, &format!("sql-{phase:?}"), sql)
        }
        Error::Source(reason) => {
            let site = tos_compiler::source_witness_catalog::source_refusal_stage(reason)
                .map(|stage| format!("compiler-{stage}"))
                .unwrap_or_else(|| "compiler-source".to_owned());
            crate::source_admission_spooled_index::bounded_source_cause(
                "receiver-source",
                &site,
                &error.to_string(),
            )
        }
    }
}

// Only the owned phase and SQLite's numeric result code cross this boundary.
// Preserve the fingerprint of the complete original compiler error for custody.
fn compiler_sql_cause(
    compiler: &tos_compiler::Error,
    phase: &str,
    sql: &rusqlite::Error,
) -> String {
    let site = match sql {
        rusqlite::Error::SqliteFailure(code, _) => {
            format!("compiler-{phase}-{}", code.extended_code)
        }
        rusqlite::Error::QueryReturnedNoRows => format!("compiler-{phase}-no-rows"),
        rusqlite::Error::InvalidColumnType(..) => format!("compiler-{phase}-column-type"),
        rusqlite::Error::InvalidQuery => format!("compiler-{phase}-query"),
        _ => format!("compiler-{phase}-other"),
    };
    crate::source_admission_spooled_index::bounded_source_cause(
        "receiver-source",
        &site,
        &compiler.to_string(),
    )
}

#[cfg(test)]
mod public_refusal_tests {
    use super::*;

    #[test]
    fn owned_reason_survives_io_while_foreign_paths_remain_private() {
        let owned = std::io::Error::other(SourceCommandError::Invalid("V2 profile absent"));
        assert!(public_io_reason(&owned).contains("V2 profile absent"));
        let foreign = std::io::Error::new(std::io::ErrorKind::NotFound, "/private/owner/input");
        let reason = public_io_reason(&foreign);
        assert!(reason.contains("NotFound"));
        assert!(!reason.contains("/private/owner/input"));
        let execution = SourceCommandError::SchemaExecution {
            path: "/private/member".into(),
            root: "/private/root".into(),
            reason: tos_validation::item_rules::ItemRefusal::BudgetCheck {
                check: "declared work",
                used: Some(11),
                limit: Some(10),
            },
        };
        let reason = public_io_reason(&std::io::Error::other(execution));
        assert!(reason.ends_with(":11:10"));
        assert!(!reason.contains("/private/"));
    }
}

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

pub(crate) const SELECTED_SOURCE_MAX_FILES: usize = 4096;
pub(crate) const SELECTED_SOURCE_MAX_BYTES: usize = 33_554_432;
pub(crate) const SELECTED_SOURCE_MAX_MEMBER_BYTES: usize = 8_388_608;

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

/// Shared proposal content; authored selection is supplied by its concrete owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandPlan {
    pub handler_id: String,
    pub operation: String,
    pub request_canonical_sha256: Digest256,
    pub configuration_raw_sha256: Digest256,
    pub configuration_canonical_sha256: Digest256,
    pub response: JsonValue,
    pub changes: Vec<SourceChange>,
    pub reads: Vec<SourceDependency>,
    pub replayed: bool,
}
impl CommandPlan {
    pub(crate) fn into_v1(self, base_revision: SourceRevision) -> PreparedCommand {
        PreparedCommand {
            base_revision,
            handler_id: self.handler_id,
            operation: self.operation,
            request_canonical_sha256: self.request_canonical_sha256,
            configuration_raw_sha256: self.configuration_raw_sha256,
            configuration_canonical_sha256: self.configuration_canonical_sha256,
            response: self.response,
            changes: self.changes,
            reads: self.reads,
            replayed: self.replayed,
        }
    }
}

impl CommandContext {
    /// Verify selected authored inputs and independently captured software
    /// inputs without merging their namespaces. This observes exact raw bytes;
    /// it establishes neither complete inventory nor account/admission authority.
    pub fn check_from_selected_captures(
        &self,
        source: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.check()?;
        if source.current().revision() != self.base_revision
            || components.capture() != software.selection()
        {
            return Err(SourceCommandError::Conflict(
                "selected command input carriers differ",
            ));
        }
        for input in &self.files {
            let digest = Digest256::of_bytes(&input.raw);
            let raw =
                if input.path.as_str().starts_with("ToS/") {
                    let member = source.current().member(&input.path).ok_or(
                        SourceCommandError::Unsupported(
                            "authored command input absent from selected source cut",
                        ),
                    )?;
                    if member.sha256 != digest || member.size_bytes != input.raw.len() as u64 {
                        return Err(SourceCommandError::Conflict(
                            "authored command input binding differs",
                        ));
                    }
                    source
                        .read_member(
                            self.base_revision,
                            &input.path,
                            SELECTED_SOURCE_MAX_MEMBER_BYTES as u64,
                            deadline,
                            cancelled,
                        )
                        .map_err(|_| {
                            SourceCommandError::Unsupported(
                                "authored command input custody read incomplete",
                            )
                        })?
                        .raw
                } else {
                    selected_software_input(software, components, input, deadline, cancelled)?
                };
            if raw != input.raw {
                return Err(SourceCommandError::Conflict(
                    "selected command input bytes differ",
                ));
            }
        }
        Ok(())
    }

    /// Only the bounded software/profile selection is checked here. The
    /// streamed cold owner must separately verify EVERY authored member and
    /// EOF; this helper carries no authored completeness or read authority.
    pub(crate) fn check_selected_software_inputs(
        &self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.check()?;
        if components.capture() != software.selection() {
            return Err(SourceCommandError::Conflict(
                "selected software capture differs",
            ));
        }
        for input in self
            .files
            .iter()
            .filter(|input| !input.path.as_str().starts_with("ToS/"))
        {
            let raw = selected_software_input(software, components, input, deadline, cancelled)?;
            if raw != input.raw {
                return Err(SourceCommandError::Conflict(
                    "selected software input bytes differ",
                ));
            }
        }
        Ok(())
    }

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
        if self.files.len() > SELECTED_SOURCE_MAX_FILES
            || total.is_none_or(|total| total > SELECTED_SOURCE_MAX_BYTES)
        {
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
        changes: Vec<SourceChange>,
        replayed: bool,
    ) -> SourceCommandResult<PreparedCommand> {
        Ok(self
            .plan_content(handler, response, changes, replayed)?
            .into_v1(self.base_revision))
    }
    pub(crate) fn plan_content(
        &self,
        handler: &str,
        response: JsonValue,
        mut changes: Vec<SourceChange>,
        replayed: bool,
    ) -> SourceCommandResult<CommandPlan> {
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
                .is_some_and(|raw| raw.len() > SELECTED_SOURCE_MAX_MEMBER_BYTES)
            {
                return Err(SourceCommandError::Invalid(
                    "proposed source member byte budget",
                ));
            }
        }
        let request = parse(&self.request_raw)?;
        let config = parse(&self.configuration_raw)?;
        Ok(CommandPlan {
            handler_id: handler.into(),
            operation: text(&request, "operation")?.into(),
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

fn selected_software_input(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    input: &SourceFile,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let digest = Digest256::of_bytes(&input.raw);
    let member = components
        .member(&input.path)
        .ok_or(SourceCommandError::Unsupported(
            "software command input absent from selected component subset",
        ))?;
    if member.sha256 != digest || member.size_bytes != input.raw.len() as u64 {
        return Err(SourceCommandError::Conflict(
            "software command input binding differs",
        ));
    }
    software
        .read_selected_component(
            components,
            &input.path,
            SELECTED_SOURCE_MAX_MEMBER_BYTES as u64,
            deadline,
            cancelled,
        )
        .map_err(|_| {
            SourceCommandError::Unsupported("software command input custody read incomplete")
        })
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
/// Preserve the maintained Python owner's Unicode 16 `str.strip()` test.
/// The scalar budget is bounded by the already selected UTF-8 byte length.
pub(crate) fn nonblank(value: &str) -> bool {
    tos_foundation::python_strip_unicode16_v1(value, value.len())
        .is_ok_and(|stripped| !stripped.is_empty())
}
pub(crate) fn reference(v: &JsonValue, id: &str, version: &str) -> SourceCommandResult<JsonValue> {
    Ok(object(vec![
        ("id", string(text(v, id)?)),
        ("version", number(integer(v, version)?)),
        ("digest", string(&record_digest(v)?.to_prefixed())),
    ]))
}
