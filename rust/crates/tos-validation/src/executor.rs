//! Operation-owned, fail-closed isolated JSON Schema worker boundary.
//!
//! This module does not create a validation trace or admission attestation.
//! The caller must pin the dedicated worker's exact ELF digest. Linux copies
//! that ELF to a sealed executable memfd before launching it; the selected-cut
//! adapter retains that immutable image within one bounded operation, so a path change
//! after verification cannot change the worker image. One bounded child retains
//! only its exact selected closure/profile and finite selector cache until explicit
//! finalization; uncertain exchanges poison it permanently.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tos_foundation::{Digest256, Digest256Hasher};

use crate::{FormatProfile, SchemaResource};

const MAX_URI_BYTES: usize = 4096;
const MAX_FRAME_BYTES: usize = 36 * 1024 * 1024;
const BATCH_REQUEST_MAGIC: &[u8; 8] = b"TOSV2BQ1";
const BATCH_ACK_MAGIC: &[u8; 8] = b"TOSV2BA1";
const BATCH_UNIT_MAGIC: &[u8; 8] = b"TOSV2BU1";
const BATCH_ACK_BYTES: usize = 8 + 32 + 32 + 4;
const BATCH_UNIT_BYTES: usize = 8 + 8 + 32 + 2;
const MAX_BATCH_UNITS: usize = 64;
const MAX_BATCH_RAW_BYTES: usize = 32 * 1024 * 1024;
const MAX_BATCH_FRAME_BYTES: usize = 68 * 1024 * 1024;
const OPERATION_REQUEST_MAGIC: &[u8; 8] = b"TOSV2OP1";
const OPERATION_END_MAGIC: &[u8; 8] = b"TOSV2OE1";
const OPERATION_END_BYTES: usize = 8 + 32 + 32 + 4;
const OPERATION_CLOSE_MAGIC: &[u8; 8] = b"TOSV2OC1";
const OPERATION_FINAL_MAGIC: &[u8; 8] = b"TOSV2OF1";
const OPERATION_FINAL_BYTES: usize = 8 + 32 + 32 + 8;
// nonce, selected schema digest/profile, finite aggregate envelope.
const OPERATION_HEADER_BYTES: usize = 8 + 16 + 32 + 1 + 8 * 8;
const MAX_MEMBER_ID_BYTES: usize = 512;
const MAX_PATH_BYTES: usize = 4096;
const DIAGNOSTIC_REQUEST_MAGIC: &[u8; 8] = b"TOSV2SD2";
const DIAGNOSTIC_ACK_MAGIC: &[u8; 8] = b"TOSV2DA2";
const DIAGNOSTIC_UNIT_MAGIC: &[u8; 8] = b"TOSV2DU2";
const DIAGNOSTIC_FINAL_MAGIC: &[u8; 8] = b"TOSV2DF2";
const DIAGNOSTIC_ACK_BYTES: usize = 8 + 2 + 32 * 4 + 4;
const DIAGNOSTIC_UNIT_HEADER_BYTES: usize = 8 + 2 + 8 + 32 + 1 + 1 + 8 + 1 + 4 + 4 + 32;
const DIAGNOSTIC_FINAL_BYTES: usize = 8 + 2 + 32 * 5 + 4;
const DIAGNOSTIC_EXCEPTIONAL_COUNTERS_BYTES: usize = 9 * 8;
const DIAGNOSTIC_FIXED_REQUEST_BYTES: usize = 8 + 2 + 4 + 4 + 2 + 4 + 32 + 32 + 1;
const MAX_DIAGNOSTIC_REQUEST_BYTES: usize = MAX_BATCH_FRAME_BYTES + 256;
const DIAGNOSTIC_EXTENDED_INPUT_MARKER: u8 = 0xff;
const DIAGNOSTIC_INPUT_LEGACY_PYTHON_OBSERVED: u8 = 1;
const DIAGNOSTIC_INPUT_MIXED_SOURCE_FOUNDATION: u8 = 2;
const DIAGNOSTIC_INPUT_FINITE_JSON_SELECTED: u8 = 3;
const DIAGNOSTIC_INPUT_LEGACY_PYTHON_OBSERVED_SELECTED: u8 = 4;
const LEGACY_SELECTED_DIAGNOSTIC_MAX_VISITS: u32 = 2_000_000;
const LEGACY_SELECTED_DIAGNOSTIC_MAX_STATE_BYTES: u64 = 384 * 1024 * 1024;
const LEGACY_SELECTED_DIAGNOSTIC_RUNTIME_HEADROOM_BYTES: u64 = 512 * 1024 * 1024;

/// Maximum ELF image size accepted by the Linux sealed-worker copier.
/// Callers that prepare one image for a multi-adapter operation can reserve
/// this bound before the single source read, then settle against the retained
/// image length after preparation succeeds.
pub const MAX_WORKER_IMAGE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiagnosticsInputProfile {
    FiniteJson,
    LegacyPythonObserved,
    MixedSourceFoundation,
    FiniteJsonSelected,
    LegacyPythonObservedSelected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiagnosticsUnitInputMode {
    FiniteJson,
    LegacyPythonObserved,
    /// Finite JSON using the caller's selected source-foundation ceiling.
    /// This keeps the standard finite parser and backend; it only raises the
    /// one-MiB probe admission limit up to the existing batch raw-byte cap.
    FiniteJsonSelected,
    /// Legacy Python-compatible grammar with request-bound parser and
    /// conversion state ceilings. Historical raw mode 2 remains unchanged.
    LegacyPythonObservedSelected,
}

/// Caller-selected finite limits for the LegacyPythonObserved diagnostics-v2
/// sibling. This does not change the historical 300,000-visit raw lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegacySelectedDiagnosticsLimits {
    pub max_instance_bytes: usize,
    pub max_visits: u32,
    pub parser_state_bytes: u64,
    pub conversion_state_bytes: u64,
}

impl LegacySelectedDiagnosticsLimits {
    pub const MAX_INSTANCE_BYTES: usize = MAX_BATCH_RAW_BYTES;
    pub const MAX_VISITS: u32 = LEGACY_SELECTED_DIAGNOSTIC_MAX_VISITS;
    pub const MAX_STATE_BYTES: u64 = LEGACY_SELECTED_DIAGNOSTIC_MAX_STATE_BYTES;
    pub const MAX_DEPTH: usize = 64;
    pub const MAX_INTEGER_DIGITS: usize = 4_300;
    pub const RUNTIME_HEADROOM_BYTES: u64 = LEGACY_SELECTED_DIAGNOSTIC_RUNTIME_HEADROOM_BYTES;

    pub(crate) fn validate(self) -> Result<(), ExecutorFailure> {
        if self.max_instance_bytes == 0
            || self.max_instance_bytes > Self::MAX_INSTANCE_BYTES
            || self.max_visits == 0
            || self.max_visits > Self::MAX_VISITS
            || self.parser_state_bytes == 0
            || self.parser_state_bytes > Self::MAX_STATE_BYTES
            || self.conversion_state_bytes == 0
            || self.conversion_state_bytes > Self::MAX_STATE_BYTES
        {
            return Err(ExecutorFailure::InputBudget);
        }
        Ok(())
    }

    fn update_digest(self, digest: &mut Digest256Hasher) {
        digest.update(&(self.max_instance_bytes as u64).to_be_bytes());
        digest.update(&self.max_visits.to_be_bytes());
        digest.update(&self.parser_state_bytes.to_be_bytes());
        digest.update(&self.conversion_state_bytes.to_be_bytes());
    }
}

/// Full selected-Legacy child address-space admission, computed by the
/// controller before request/input copies and spawn at both source-cut
/// admission and sealed-image request preparation. The existing OS child
/// address-space limit remains the enforcement boundary. The resource
/// component comes from the shared ToS resource-only preparation kernel; RSS
/// and allocator metadata are separate.
pub(crate) fn legacy_selected_child_address_space_required(
    resource_preparation_state_bytes: usize,
    request_frame_bytes: usize,
    response_buffer_bytes: usize,
    member_id_bytes: usize,
    path_bytes: usize,
    root_uri_bytes: usize,
    instance_bytes: usize,
    actual_worker_image_bytes: u64,
    limits: LegacySelectedDiagnosticsLimits,
) -> Result<u64, ExecutorFailure> {
    limits.validate()?;
    if actual_worker_image_bytes == 0 || actual_worker_image_bytes > MAX_WORKER_IMAGE_BYTES {
        return Err(ExecutorFailure::WorkerIdentity);
    }
    let unit_metadata_bytes = std::mem::size_of::<BatchUnit>()
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(member_id_bytes.checked_mul(2)?))
        .and_then(|bytes| bytes.checked_add(path_bytes.checked_mul(2)?))
        .and_then(|bytes| bytes.checked_add(root_uri_bytes.checked_mul(2)?))
        .ok_or(ExecutorFailure::InputBudget)?;
    let input_buffers = instance_bytes
        .checked_mul(2)
        .ok_or(ExecutorFailure::InputBudget)?;
    // Frame construction uses sequential Vec extensions. Count a conservative
    // three-frame envelope for the completed frame plus the old/new Vec
    // allocations that can overlap during growth.
    let request_buffers = request_frame_bytes
        .checked_mul(3)
        .ok_or(ExecutorFailure::InputBudget)?;
    let response_buffers = response_buffer_bytes
        .checked_mul(2)
        .ok_or(ExecutorFailure::InputBudget)?;
    [
        u64::try_from(resource_preparation_state_bytes)
            .map_err(|_| ExecutorFailure::InputBudget)?,
        u64::try_from(unit_metadata_bytes).map_err(|_| ExecutorFailure::InputBudget)?,
        u64::try_from(input_buffers).map_err(|_| ExecutorFailure::InputBudget)?,
        u64::try_from(request_buffers).map_err(|_| ExecutorFailure::InputBudget)?,
        u64::try_from(response_buffers).map_err(|_| ExecutorFailure::InputBudget)?,
        actual_worker_image_bytes,
        limits.parser_state_bytes,
        limits.conversion_state_bytes,
        LEGACY_SELECTED_DIAGNOSTIC_RUNTIME_HEADROOM_BYTES,
    ]
    .into_iter()
    .try_fold(0u64, u64::checked_add)
    .ok_or(ExecutorFailure::InputBudget)
}

/// Fixed profile-2 exceptional-evaluator budget vector. A request carries the
/// remaining amounts; the worker returns the amounts actually consumed. This
/// type also represents those returned per-chunk usage counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExceptionalSchemaUsage {
    pub schema_scan_work: u64,
    pub schema_scan_bytes: u64,
    pub pattern_compile_count: u64,
    pub pattern_bytes: u64,
    pub evaluation_work: u64,
    pub evaluation_bytes: u64,
    pub reference_steps: u64,
    pub regex_checks: u64,
    pub regex_bytes: u64,
}

impl ExceptionalSchemaUsage {
    /// The fixed whole-call ceiling bound by the profile-2 capability digest.
    pub const fn whole() -> Self {
        Self {
            schema_scan_work: 300_000,
            schema_scan_bytes: 32 * 1024 * 1024,
            pattern_compile_count: 4_096,
            pattern_bytes: 256 * 1024,
            evaluation_work: 1_000_000,
            evaluation_bytes: 64 * 1024 * 1024,
            reference_steps: 100_000,
            regex_checks: 100_000,
            regex_bytes: 64 * 1024 * 1024,
        }
    }

    pub(crate) fn fits_within(self, limit: Self) -> bool {
        self.schema_scan_work <= limit.schema_scan_work
            && self.schema_scan_bytes <= limit.schema_scan_bytes
            && self.pattern_compile_count <= limit.pattern_compile_count
            && self.pattern_bytes <= limit.pattern_bytes
            && self.evaluation_work <= limit.evaluation_work
            && self.evaluation_bytes <= limit.evaluation_bytes
            && self.reference_steps <= limit.reference_steps
            && self.regex_checks <= limit.regex_checks
            && self.regex_bytes <= limit.regex_bytes
    }

    pub(crate) fn checked_sub(self, used: Self) -> Option<Self> {
        Some(Self {
            schema_scan_work: self.schema_scan_work.checked_sub(used.schema_scan_work)?,
            schema_scan_bytes: self.schema_scan_bytes.checked_sub(used.schema_scan_bytes)?,
            pattern_compile_count: self
                .pattern_compile_count
                .checked_sub(used.pattern_compile_count)?,
            pattern_bytes: self.pattern_bytes.checked_sub(used.pattern_bytes)?,
            evaluation_work: self.evaluation_work.checked_sub(used.evaluation_work)?,
            evaluation_bytes: self.evaluation_bytes.checked_sub(used.evaluation_bytes)?,
            reference_steps: self.reference_steps.checked_sub(used.reference_steps)?,
            regex_checks: self.regex_checks.checked_sub(used.regex_checks)?,
            regex_bytes: self.regex_bytes.checked_sub(used.regex_bytes)?,
        })
    }

    fn write_be(self, output: &mut Vec<u8>) {
        for value in [
            self.schema_scan_work,
            self.schema_scan_bytes,
            self.pattern_compile_count,
            self.pattern_bytes,
            self.evaluation_work,
            self.evaluation_bytes,
            self.reference_steps,
            self.regex_checks,
            self.regex_bytes,
        ] {
            output.extend_from_slice(&value.to_be_bytes());
        }
    }

    fn read_be(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != DIAGNOSTIC_EXCEPTIONAL_COUNTERS_BYTES {
            return None;
        }
        let mut values = [0u64; 9];
        for (index, value) in values.iter_mut().enumerate() {
            let start = index.checked_mul(8)?;
            *value = u64::from_be_bytes(bytes.get(start..start + 8)?.try_into().ok()?);
        }
        Some(Self {
            schema_scan_work: values[0],
            schema_scan_bytes: values[1],
            pattern_compile_count: values[2],
            pattern_bytes: values[3],
            evaluation_work: values[4],
            evaluation_bytes: values[5],
            reference_steps: values[6],
            regex_checks: values[7],
            regex_bytes: values[8],
        })
    }
}

impl DiagnosticsUnitInputMode {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::FiniteJson => 1,
            Self::LegacyPythonObserved => 2,
            Self::FiniteJsonSelected => 3,
            Self::LegacyPythonObservedSelected => 4,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MixedDiagnosticsBatchUnit {
    pub unit: BatchUnit,
    pub input_mode: DiagnosticsUnitInputMode,
}

/// Structured, bounded diagnostics are an opt-in worker protocol. The v1
/// verdict exchange above remains byte-for-byte unchanged.
pub mod schema_diagnostics {
    use jsonschema::error::{TypeKind, ValidationErrorKind};
    use tos_foundation::{Digest256, Digest256Hasher};

    pub const PROTOCOL_VERSION: u16 = 2;
    pub const MAX_ISSUES_PER_UNIT: u32 = 128;
    pub const MAX_REPORT_BYTES_PER_UNIT: u32 = 128 * 1024;
    pub const MAX_PATH_SEGMENTS: u16 = 128;
    pub const MAX_PATH_BYTES: u32 = 16 * 1024;
    pub const MAX_RESPONSE_BYTES: usize = 20 * 1024 * 1024;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Caps {
        pub max_issues_per_unit: u32,
        pub max_report_bytes_per_unit: u32,
        pub max_path_segments: u16,
        pub max_path_bytes: u32,
    }

    impl Caps {
        pub const CURRENT: Self = Self {
            max_issues_per_unit: MAX_ISSUES_PER_UNIT,
            max_report_bytes_per_unit: MAX_REPORT_BYTES_PER_UNIT,
            max_path_segments: MAX_PATH_SEGMENTS,
            max_path_bytes: MAX_PATH_BYTES,
        };

        pub fn digest(self) -> Digest256 {
            let mut hash = Digest256Hasher::new();
            hash.update(b"tos-schema-diagnostics-caps-v2\0");
            hash.update(&PROTOCOL_VERSION.to_be_bytes());
            hash.update(&self.max_issues_per_unit.to_be_bytes());
            hash.update(&self.max_report_bytes_per_unit.to_be_bytes());
            hash.update(&self.max_path_segments.to_be_bytes());
            hash.update(&self.max_path_bytes.to_be_bytes());
            hash.finalize()
        }

        pub(crate) fn validate(self) -> bool {
            self.max_issues_per_unit > 0
                && self.max_issues_per_unit <= MAX_ISSUES_PER_UNIT
                && self.max_report_bytes_per_unit > 0
                && self.max_report_bytes_per_unit <= MAX_REPORT_BYTES_PER_UNIT
                && self.max_path_segments > 0
                && self.max_path_segments <= MAX_PATH_SEGMENTS
                && self.max_path_bytes > 0
                && self.max_path_bytes <= MAX_PATH_BYTES
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub enum PathSegment {
        Property(String),
        Index(u64),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    #[repr(u16)]
    pub enum Reason {
        AdditionalItems = 1,
        AdditionalProperties = 2,
        AnyOf = 3,
        Pattern = 4,
        Constant = 5,
        Contains = 6,
        ContentEncoding = 7,
        ContentMediaType = 8,
        CustomKeyword = 9,
        Enum = 10,
        ExclusiveMaximum = 11,
        ExclusiveMinimum = 12,
        FalseSchema = 13,
        Format = 14,
        MaximumItems = 15,
        Maximum = 16,
        MaximumLength = 17,
        MaximumProperties = 18,
        MinimumItems = 19,
        Minimum = 20,
        MinimumLength = 21,
        MinimumProperties = 22,
        MultipleOf = 23,
        Not = 24,
        OneOf = 25,
        PropertyNames = 26,
        Required = 27,
        Type = 28,
        UnevaluatedItems = 29,
        UnevaluatedProperties = 30,
        UniqueItems = 31,
        BacktrackLimit = 32,
        RegexEngineFailure = 33,
        ReferenceFailure = 34,
        UnknownValidationFailure = 35,
    }

    impl Reason {
        pub const fn code(self) -> &'static str {
            match self {
                Self::AdditionalItems => "additional_items",
                Self::AdditionalProperties => "additional_properties",
                Self::AnyOf => "any_of",
                Self::Pattern => "pattern",
                Self::Constant => "constant",
                Self::Contains => "contains",
                Self::ContentEncoding => "content_encoding",
                Self::ContentMediaType => "content_media_type",
                Self::CustomKeyword => "custom_keyword",
                Self::Enum => "enum",
                Self::ExclusiveMaximum => "exclusive_maximum",
                Self::ExclusiveMinimum => "exclusive_minimum",
                Self::FalseSchema => "false_schema",
                Self::Format => "format",
                Self::MaximumItems => "maximum_items",
                Self::Maximum => "maximum",
                Self::MaximumLength => "maximum_length",
                Self::MaximumProperties => "maximum_properties",
                Self::MinimumItems => "minimum_items",
                Self::Minimum => "minimum",
                Self::MinimumLength => "minimum_length",
                Self::MinimumProperties => "minimum_properties",
                Self::MultipleOf => "multiple_of",
                Self::Not => "not",
                Self::OneOf => "one_of",
                Self::PropertyNames => "property_names",
                Self::Required => "required",
                Self::Type => "type",
                Self::UnevaluatedItems => "unevaluated_items",
                Self::UnevaluatedProperties => "unevaluated_properties",
                Self::UniqueItems => "unique_items",
                Self::BacktrackLimit => "backtrack_limit",
                Self::RegexEngineFailure => "regex_engine_failure",
                Self::ReferenceFailure => "reference_failure",
                Self::UnknownValidationFailure => "unknown_validation_failure",
            }
        }

        pub const fn schema_keyword(self) -> &'static str {
            match self {
                Self::AdditionalItems => "additionalItems",
                Self::AdditionalProperties => "additionalProperties",
                Self::AnyOf => "anyOf",
                Self::Pattern | Self::BacktrackLimit | Self::RegexEngineFailure => "pattern",
                Self::Constant => "const",
                Self::Contains => "contains",
                Self::ContentEncoding => "contentEncoding",
                Self::ContentMediaType => "contentMediaType",
                Self::CustomKeyword => "custom",
                Self::Enum => "enum",
                Self::ExclusiveMaximum => "exclusiveMaximum",
                Self::ExclusiveMinimum => "exclusiveMinimum",
                Self::FalseSchema => "false",
                Self::Format => "format",
                Self::MaximumItems => "maxItems",
                Self::Maximum => "maximum",
                Self::MaximumLength => "maxLength",
                Self::MaximumProperties => "maxProperties",
                Self::MinimumItems => "minItems",
                Self::Minimum => "minimum",
                Self::MinimumLength => "minLength",
                Self::MinimumProperties => "minProperties",
                Self::MultipleOf => "multipleOf",
                Self::Not => "not",
                Self::OneOf => "oneOf",
                Self::PropertyNames => "propertyNames",
                Self::Required => "required",
                Self::Type => "type",
                Self::UnevaluatedItems => "unevaluatedItems",
                Self::UnevaluatedProperties => "unevaluatedProperties",
                Self::UniqueItems => "uniqueItems",
                Self::ReferenceFailure => "$ref",
                Self::UnknownValidationFailure => "unknown",
            }
        }

        pub(crate) fn from_wire(value: u16) -> Option<Self> {
            Some(match value {
                1 => Self::AdditionalItems,
                2 => Self::AdditionalProperties,
                3 => Self::AnyOf,
                4 => Self::Pattern,
                5 => Self::Constant,
                6 => Self::Contains,
                7 => Self::ContentEncoding,
                8 => Self::ContentMediaType,
                9 => Self::CustomKeyword,
                10 => Self::Enum,
                11 => Self::ExclusiveMaximum,
                12 => Self::ExclusiveMinimum,
                13 => Self::FalseSchema,
                14 => Self::Format,
                15 => Self::MaximumItems,
                16 => Self::Maximum,
                17 => Self::MaximumLength,
                18 => Self::MaximumProperties,
                19 => Self::MinimumItems,
                20 => Self::Minimum,
                21 => Self::MinimumLength,
                22 => Self::MinimumProperties,
                23 => Self::MultipleOf,
                24 => Self::Not,
                25 => Self::OneOf,
                26 => Self::PropertyNames,
                27 => Self::Required,
                28 => Self::Type,
                29 => Self::UnevaluatedItems,
                30 => Self::UnevaluatedProperties,
                31 => Self::UniqueItems,
                32 => Self::BacktrackLimit,
                33 => Self::RegexEngineFailure,
                34 => Self::ReferenceFailure,
                35 => Self::UnknownValidationFailure,
                _ => return None,
            })
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    #[repr(u8)]
    pub enum CompatibilityText {
        NullIsNotObject = 1,
        NullIsNotArray = 2,
    }

    impl CompatibilityText {
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::NullIsNotObject => "None is not of type 'object'",
                Self::NullIsNotArray => "None is not of type 'array'",
            }
        }

        pub(crate) fn from_wire(value: u8) -> Option<Option<Self>> {
            Some(match value {
                0 => None,
                1 => Some(Self::NullIsNotObject),
                2 => Some(Self::NullIsNotArray),
                _ => return None,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Issue {
        pub instance_path: Vec<PathSegment>,
        pub schema_keyword: String,
        pub reason: Reason,
        pub schema_path: Vec<PathSegment>,
        pub compatibility_text: Option<CompatibilityText>,
    }

    impl Issue {
        pub fn compatibility_text(&self) -> Option<&'static str> {
            self.compatibility_text.map(CompatibilityText::as_str)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum Status {
        Valid = 0,
        Invalid = 1,
        Truncated = 2,
        InputRejected = 3,
        Indeterminate = 4,
    }

    impl Status {
        pub(crate) fn from_wire(value: u8) -> Option<Self> {
            Some(match value {
                0 => Self::Valid,
                1 => Self::Invalid,
                2 => Self::Truncated,
                3 => Self::InputRejected,
                4 => Self::Indeterminate,
                _ => return None,
            })
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum Failure {
        None = 0,
        InvalidJson = 1,
        InputBudget = 2,
        ValidatorRuntime = 3,
        UnsupportedInputSemantics = 4,
    }

    impl Failure {
        pub(crate) fn from_wire(value: u8) -> Option<Self> {
            Some(match value {
                0 => Self::None,
                1 => Self::InvalidJson,
                2 => Self::InputBudget,
                3 => Self::ValidatorRuntime,
                4 => Self::UnsupportedInputSemantics,
                _ => return None,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Report {
        pub protocol_version: u16,
        pub worker_sha256: Digest256,
        pub request_sha256: Digest256,
        pub unit_sha256: Digest256,
        pub schema_set_sha256: Digest256,
        pub caps: Caps,
        pub status: Status,
        pub failure: Failure,
        pub total_issue_count: u64,
        pub truncated: bool,
        pub issues_sha256: Digest256,
        pub report_sha256: Digest256,
        pub issues: Vec<Issue>,
    }

    impl Report {
        /// Checks the complete structured report envelope, including canonical
        /// issue ordering and every report digest. This is transport evidence,
        /// not source admission.
        pub fn is_well_formed(&self) -> bool {
            if self.protocol_version != PROTOCOL_VERSION
                || !self.caps.validate()
                || !status_is_well_formed(
                    self.status,
                    self.failure,
                    self.total_issue_count,
                    self.truncated,
                    self.issues.len(),
                )
                || self.issues.windows(2).any(|pair| pair[0] > pair[1])
                || self.issues.iter().any(|issue| {
                    issue.schema_keyword != issue.reason.schema_keyword()
                        || issue.schema_keyword.len() > 64
                        || !path_within_caps(&issue.instance_path, self.caps)
                        || !path_within_caps(&issue.schema_path, self.caps)
                })
            {
                return false;
            }
            let Some(payload_bytes) = self.issues.iter().try_fold(0usize, |total, issue| {
                total.checked_add(issue_payload(issue)?.len())
            }) else {
                return false;
            };
            if payload_bytes
                .checked_add(super::DIAGNOSTIC_UNIT_HEADER_BYTES)
                .is_none_or(|bytes| bytes > self.caps.max_report_bytes_per_unit as usize)
            {
                return false;
            }
            let Some(issues_sha256) = issues_digest(&self.issues) else {
                return false;
            };
            self.issues_sha256 == issues_sha256
                && self.report_sha256
                    == report_digest(
                        self.worker_sha256,
                        self.request_sha256,
                        self.unit_sha256,
                        self.schema_set_sha256,
                        self.caps,
                        self.status,
                        self.failure,
                        self.total_issue_count,
                        self.truncated,
                        self.issues_sha256,
                    )
        }

        pub fn is_valid(&self) -> bool {
            self.is_well_formed()
                && self.status == Status::Valid
                && self.failure == Failure::None
                && !self.truncated
                && self.total_issue_count == 0
                && self.issues.is_empty()
        }

        pub fn caps_sha256(&self) -> Digest256 {
            self.caps.digest()
        }
    }

    fn path_within_caps(path: &[PathSegment], caps: Caps) -> bool {
        if path.len() > caps.max_path_segments as usize {
            return false;
        }
        path.iter()
            .try_fold(0usize, |total, segment| {
                let cost = match segment {
                    PathSegment::Property(value) => value.len(),
                    PathSegment::Index(_) => std::mem::size_of::<u64>(),
                };
                total.checked_add(cost)
            })
            .is_some_and(|bytes| bytes <= caps.max_path_bytes as usize)
    }

    pub(crate) fn reason_and_keyword(kind: &ValidationErrorKind) -> (Reason, &'static str) {
        match kind {
            ValidationErrorKind::AdditionalItems { .. } => {
                (Reason::AdditionalItems, "additionalItems")
            }
            ValidationErrorKind::AdditionalProperties { .. } => {
                (Reason::AdditionalProperties, "additionalProperties")
            }
            ValidationErrorKind::AnyOf { .. } => (Reason::AnyOf, "anyOf"),
            ValidationErrorKind::BacktrackLimitExceeded { .. } => {
                (Reason::BacktrackLimit, "pattern")
            }
            ValidationErrorKind::RegexEngineFailure { .. } => {
                (Reason::RegexEngineFailure, "pattern")
            }
            ValidationErrorKind::Pattern { .. } => (Reason::Pattern, "pattern"),
            ValidationErrorKind::Constant { .. } => (Reason::Constant, "const"),
            ValidationErrorKind::Contains => (Reason::Contains, "contains"),
            ValidationErrorKind::ContentEncoding { .. } | ValidationErrorKind::FromUtf8 { .. } => {
                (Reason::ContentEncoding, "contentEncoding")
            }
            ValidationErrorKind::ContentMediaType { .. } => {
                (Reason::ContentMediaType, "contentMediaType")
            }
            ValidationErrorKind::Custom { .. } => (Reason::CustomKeyword, "custom"),
            ValidationErrorKind::Enum { .. } => (Reason::Enum, "enum"),
            ValidationErrorKind::ExclusiveMaximum { .. } => {
                (Reason::ExclusiveMaximum, "exclusiveMaximum")
            }
            ValidationErrorKind::ExclusiveMinimum { .. } => {
                (Reason::ExclusiveMinimum, "exclusiveMinimum")
            }
            ValidationErrorKind::FalseSchema => (Reason::FalseSchema, "false"),
            ValidationErrorKind::Format { .. } => (Reason::Format, "format"),
            ValidationErrorKind::MaxItems { .. } => (Reason::MaximumItems, "maxItems"),
            ValidationErrorKind::Maximum { .. } => (Reason::Maximum, "maximum"),
            ValidationErrorKind::MaxLength { .. } => (Reason::MaximumLength, "maxLength"),
            ValidationErrorKind::MaxProperties { .. } => {
                (Reason::MaximumProperties, "maxProperties")
            }
            ValidationErrorKind::MinItems { .. } => (Reason::MinimumItems, "minItems"),
            ValidationErrorKind::Minimum { .. } => (Reason::Minimum, "minimum"),
            ValidationErrorKind::MinLength { .. } => (Reason::MinimumLength, "minLength"),
            ValidationErrorKind::MinProperties { .. } => {
                (Reason::MinimumProperties, "minProperties")
            }
            ValidationErrorKind::MultipleOf { .. } => (Reason::MultipleOf, "multipleOf"),
            ValidationErrorKind::Not { .. } => (Reason::Not, "not"),
            ValidationErrorKind::OneOfMultipleValid { .. }
            | ValidationErrorKind::OneOfNotValid { .. } => (Reason::OneOf, "oneOf"),
            ValidationErrorKind::PropertyNames { .. } => (Reason::PropertyNames, "propertyNames"),
            ValidationErrorKind::Required { .. } => (Reason::Required, "required"),
            ValidationErrorKind::Type { .. } => (Reason::Type, "type"),
            ValidationErrorKind::UnevaluatedItems { .. } => {
                (Reason::UnevaluatedItems, "unevaluatedItems")
            }
            ValidationErrorKind::UnevaluatedProperties { .. } => {
                (Reason::UnevaluatedProperties, "unevaluatedProperties")
            }
            ValidationErrorKind::UniqueItems => (Reason::UniqueItems, "uniqueItems"),
            ValidationErrorKind::Referencing(_) => (Reason::ReferenceFailure, "$ref"),
            _ => (Reason::UnknownValidationFailure, "unknown"),
        }
    }

    pub(crate) fn status_is_well_formed(
        status: Status,
        failure: Failure,
        total_issue_count: u64,
        truncated: bool,
        issue_count: usize,
    ) -> bool {
        match status {
            Status::Valid => {
                failure == Failure::None && total_issue_count == 0 && !truncated && issue_count == 0
            }
            Status::Invalid => {
                failure == Failure::None
                    && total_issue_count > 0
                    && !truncated
                    && total_issue_count == issue_count as u64
            }
            Status::Truncated => {
                failure == Failure::None && truncated && total_issue_count > issue_count as u64
            }
            Status::InputRejected => {
                matches!(failure, Failure::InvalidJson | Failure::InputBudget)
                    && total_issue_count == 0
                    && !truncated
                    && issue_count == 0
            }
            Status::Indeterminate => {
                matches!(
                    failure,
                    Failure::ValidatorRuntime | Failure::UnsupportedInputSemantics
                ) && issue_count as u64 <= total_issue_count
                    && if total_issue_count == 0 {
                        issue_count == 0 && !truncated
                    } else {
                        truncated || issue_count as u64 == total_issue_count
                    }
            }
        }
    }

    pub(crate) fn compatibility_text(
        kind: &ValidationErrorKind,
        instance: &serde_json::Value,
    ) -> Option<CompatibilityText> {
        if !instance.is_null() {
            return None;
        }
        let ValidationErrorKind::Type { kind } = kind else {
            return None;
        };
        let TypeKind::Single(expected) = kind else {
            return None;
        };
        match expected {
            jsonschema::JsonType::Object => Some(CompatibilityText::NullIsNotObject),
            jsonschema::JsonType::Array => Some(CompatibilityText::NullIsNotArray),
            _ => None,
        }
    }

    pub(crate) fn path_from_location<'a>(
        segments: impl Iterator<Item = jsonschema::paths::LocationSegment<'a>>,
        caps: Caps,
    ) -> Option<Vec<PathSegment>> {
        let mut output = Vec::new();
        let mut bytes = 0usize;
        for segment in segments {
            if output.len() >= caps.max_path_segments as usize {
                return None;
            }
            match segment {
                jsonschema::paths::LocationSegment::Property(property) => {
                    bytes = bytes.checked_add(property.len())?;
                    if bytes > caps.max_path_bytes as usize {
                        return None;
                    }
                    output.push(PathSegment::Property(property.into_owned()));
                }
                jsonschema::paths::LocationSegment::Index(index) => {
                    bytes = bytes.checked_add(std::mem::size_of::<u64>())?;
                    if bytes > caps.max_path_bytes as usize {
                        return None;
                    }
                    output.push(PathSegment::Index(u64::try_from(index).ok()?));
                }
            }
        }
        Some(output)
    }

    pub(crate) fn issue_from_validation_error(
        error: &jsonschema::ValidationError<'_>,
        caps: Caps,
    ) -> Option<(Issue, bool)> {
        let (reason, keyword) = reason_and_keyword(error.kind());
        let instance_path = path_from_location(error.instance_path().segments(), caps)?;
        let schema_path = path_from_location(error.schema_path().segments(), caps)?;
        let indeterminate = matches!(
            reason,
            Reason::BacktrackLimit
                | Reason::RegexEngineFailure
                | Reason::ReferenceFailure
                | Reason::UnknownValidationFailure
        );
        Some((
            Issue {
                instance_path,
                schema_keyword: keyword.to_owned(),
                reason,
                schema_path,
                compatibility_text: compatibility_text(error.kind(), error.instance().as_ref()),
            },
            indeterminate,
        ))
    }

    pub(crate) fn issue_payload(issue: &Issue) -> Option<Vec<u8>> {
        let mut output = Vec::new();
        encode_path(&mut output, &issue.instance_path)?;
        put_bytes(&mut output, issue.schema_keyword.as_bytes())?;
        output.extend_from_slice(&(issue.reason as u16).to_be_bytes());
        encode_path(&mut output, &issue.schema_path)?;
        output.push(issue.compatibility_text.map_or(0, |value| value as u8));
        Some(output)
    }

    fn put_bytes(output: &mut Vec<u8>, value: &[u8]) -> Option<()> {
        output.extend_from_slice(&u32::try_from(value.len()).ok()?.to_be_bytes());
        output.extend_from_slice(value);
        Some(())
    }

    fn encode_path(output: &mut Vec<u8>, path: &[PathSegment]) -> Option<()> {
        output.extend_from_slice(&u16::try_from(path.len()).ok()?.to_be_bytes());
        for segment in path {
            match segment {
                PathSegment::Property(value) => {
                    output.push(0);
                    put_bytes(output, value.as_bytes())?;
                }
                PathSegment::Index(value) => {
                    output.push(1);
                    output.extend_from_slice(&value.to_be_bytes());
                }
            }
        }
        Some(())
    }

    pub(crate) fn issues_digest(issues: &[Issue]) -> Option<Digest256> {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-schema-diagnostics-issues-v2\0");
        hash.update(&(issues.len() as u64).to_be_bytes());
        for issue in issues {
            let bytes = issue_payload(issue)?;
            hash.update(&(bytes.len() as u64).to_be_bytes());
            hash.update(&bytes);
        }
        Some(hash.finalize())
    }

    pub(crate) fn report_digest(
        worker_sha256: Digest256,
        request_sha256: Digest256,
        unit_sha256: Digest256,
        schema_set_sha256: Digest256,
        caps: Caps,
        status: Status,
        failure: Failure,
        total_issue_count: u64,
        truncated: bool,
        issues_sha256: Digest256,
    ) -> Digest256 {
        let mut hash = Digest256Hasher::new();
        hash.update(b"tos-schema-diagnostics-report-v2\0");
        hash.update(&PROTOCOL_VERSION.to_be_bytes());
        hash.update(worker_sha256.as_bytes());
        hash.update(request_sha256.as_bytes());
        hash.update(unit_sha256.as_bytes());
        hash.update(schema_set_sha256.as_bytes());
        hash.update(caps.digest().as_bytes());
        hash.update(&[status as u8, failure as u8, u8::from(truncated)]);
        hash.update(&total_issue_count.to_be_bytes());
        hash.update(issues_sha256.as_bytes());
        hash.finalize()
    }
}

/// One fully bound diagnostics-v2 result. `is_valid()` reports only the local
/// schema check for this unit; it is not source admission or acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDiagnosticUnit {
    pub ordinal: u64,
    pub member_id: String,
    pub relative_path: String,
    pub root_uri: String,
    pub raw_sha256: Digest256,
    pub unit_sha256: Digest256,
    pub report: schema_diagnostics::Report,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaDiagnosticsCheckpoint {
    pub worker_sha256: Digest256,
    pub request_sha256: Digest256,
    pub profile: FormatProfile,
    pub schema_set_sha256: Digest256,
    pub ordered_manifest_sha256: Digest256,
    pub caps_sha256: Digest256,
    pub completed_count: u64,
    pub result_stream_sha256: Digest256,
    /// Bytes the controller actually wrote to this worker's stdin. Zero means
    /// no diagnostic request bytes crossed the pipe.
    pub worker_request_bytes: u64,
    /// Bytes the controller actually received from this worker's stdout,
    /// including acknowledgement and terminal records.
    pub worker_response_bytes: u64,
    /// Actual wait4 user+system CPU time in microseconds. `None` means the
    /// worker was not reaped with a complete usage observation.
    pub worker_cpu_micros: Option<u64>,
    /// Remaining fixed whole-call allowance sent in this request, `None` for
    /// the unchanged finite-only and raw-only diagnostics profiles.
    pub exceptional_remaining: Option<ExceptionalSchemaUsage>,
    /// Present only when a mixed-profile worker's final record was fully
    /// parsed; it is actual worker-reported usage, never a parent estimate.
    pub exceptional_usage: Option<ExceptionalSchemaUsage>,
}

/// A transport-complete diagnostic batch or a fail-closed incomplete exchange.
/// Complete transport does not imply that any unit is schema-valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaDiagnosticsOutcome {
    Complete {
        units: Vec<SchemaDiagnosticUnit>,
        checkpoint: SchemaDiagnosticsCheckpoint,
    },
    Incomplete {
        checkpoint: SchemaDiagnosticsCheckpoint,
        reason: ExecutorFailure,
        exchange: Option<ExchangeFailureContext>,
    },
}

/// Bounded accounting returned with one diagnostics-v2 worker exchange.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct SchemaDiagnosticsExecutionCost {
    pub schema_resource_bytes: usize,
    pub schema_resource_buffer_bytes: usize,
    pub input_instance_buffer_bytes: usize,
    pub request_bytes: usize,
    /// Final encoded frame Vec capacity observed after construction. Selected
    /// Legacy peak admission separately uses a conservative three-frame bound.
    pub request_buffer_bytes: usize,
    pub response_bytes: usize,
    pub response_buffer_bytes: usize,
    pub worker_cpu_micros: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ExactWorkerIdentity {
    pub absolute_path: PathBuf,
    pub sha256: Digest256,
}

/// One verified immutable worker image shared by related schema adapters in a
/// single caller-owned operation. The file descriptor refers to the sealed
/// executable memfd; adapters clone that descriptor and never reopen the path.
pub struct VerifiedWorkerImageHandle {
    identity: ExactWorkerIdentity,
    operation_deadline: Instant,
    #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
    file: std::fs::File,
}

impl VerifiedWorkerImageHandle {
    /// Verify and seal the exact worker once for a caller operation.
    pub fn prepare(
        worker: ExactWorkerIdentity,
        budget: ExecutorBudget,
        operation_deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::VerifiedWorkerImage::prepare(&worker, budget, operation_deadline, cancelled)
                .map(native::VerifiedWorkerImage::into_handle)
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (worker, budget, operation_deadline, cancelled);
            Err(ExecutorFailure::UnsupportedHost)
        }
    }

    /// Identity whose exact path and digest were checked before sealing.
    pub fn identity(&self) -> &ExactWorkerIdentity {
        &self.identity
    }

    /// Deadline shared by every adapter created from this handle.
    pub fn operation_deadline(&self) -> Instant {
        self.operation_deadline
    }

    /// Length of the already verified sealed executable image.
    pub fn image_bytes(&self) -> Result<u64, ExecutorFailure> {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            let metadata = self
                .file
                .metadata()
                .map_err(|_| ExecutorFailure::WorkerIdentity)?;
            let bytes = metadata.len();
            if bytes == 0 || bytes > MAX_WORKER_IMAGE_BYTES {
                return Err(ExecutorFailure::WorkerIdentity);
            }
            Ok(bytes)
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            Err(ExecutorFailure::UnsupportedHost)
        }
    }

    /// Conservative retained-state charge for the handle and its sealed image.
    /// The path capacity and inline handle storage are included explicitly.
    pub fn retained_state_bytes(&self) -> Result<u64, ExecutorFailure> {
        let image_bytes = self.image_bytes()?;
        let metadata_bytes = std::mem::size_of::<Self>()
            .checked_add(self.identity.absolute_path.capacity())
            .ok_or(ExecutorFailure::ResourceLimitUnknown)?;
        image_bytes
            .checked_add(
                u64::try_from(metadata_bytes).map_err(|_| ExecutorFailure::ResourceLimitUnknown)?,
            )
            .ok_or(ExecutorFailure::ResourceLimitUnknown)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExecutorBudget {
    /// Deadline for image verification, fork, transfer and worker execution.
    pub execution_wall: Duration,
    /// Additional, caller-visible allowance for SIGKILL and WNOHANG reap.
    pub cleanup_grace: Duration,
    pub cpu_seconds: u64,
    pub address_space_bytes: u64,
}

impl ExecutorBudget {
    pub fn laboratory() -> Self {
        Self {
            execution_wall: Duration::from_secs(5),
            cleanup_grace: Duration::from_millis(200),
            cpu_seconds: 3,
            address_space_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionIdentity {
    pub worker_sha256: Digest256,
    pub request_sha256: Digest256,
    pub schema_set_sha256: Digest256,
    pub instance_sha256: Digest256,
    pub profile: FormatProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorFailure {
    UnsupportedHost,
    WorkerIdentity,
    InputBudget,
    ResourceLimitUnknown,
    Spawn,
    Timeout,
    Cancelled,
    CpuLimit,
    CrashSignal(i32),
    CrashExit(i32),
    ReapPending(i32),
    Protocol,
    Backend,
    ParseRejected,
    CoverageMismatch,
}

/// A child status observed before any parent cleanup signal. This is not an
/// inference from elapsed time or a status caused by parent-directed SIGKILL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildTermination {
    Exited(i32),
    Signalled(i32),
}

/// Context for the actual failed exchange guard; no acceptance is carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExchangeFailureContext {
    pub boundary: &'static str,
    pub failure: ExecutorFailure,
    pub natural_termination: Option<ChildTermination>,
    /// Actual wait4 CPU for a naturally reaped child; never elapsed-wall inference.
    pub child_cpu_micros: Option<u64>,
    pub child_pid: i32,
    /// One-based exchange count on this child; retained sessions share its CPU cap.
    pub child_exchange_ordinal: u64,
    pub retained_session: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorOutcome {
    SchemaValid(ExecutionIdentity),
    SchemaInvalid(ExecutionIdentity),
    InputRejected(ExecutionIdentity),
    Indeterminate {
        reason: ExecutorFailure,
        identity: Option<ExecutionIdentity>,
        exchange: Option<ExchangeFailureContext>,
    },
}

/// A bounded transport unit. The path is an exact identity label only; the
/// worker never opens it. Source membership and ownership remain external.
#[derive(Debug, Clone)]
pub struct BatchUnit {
    pub ordinal: u64,
    pub member_id: String,
    pub relative_path: String,
    pub root_uri: String,
    pub raw_instance: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct BatchCoverageExpectation {
    pub count: u64,
    pub ordered_manifest_sha256: Digest256,
}

#[derive(Debug, Clone, Copy)]
pub struct BatchBudget {
    pub total_execution_wall: Duration,
    pub startup_wall: Duration,
    pub per_unit_wall: Duration,
    pub cleanup_grace: Duration,
    pub cpu_seconds: u64,
    pub address_space_bytes: u64,
    /// Caller may lower these ceilings; the implementation never exceeds 64
    /// units or 32 MiB raw instances in one request frame.
    pub max_units: usize,
    pub max_total_raw_bytes: usize,
}

impl BatchBudget {
    pub(crate) fn validate(self) -> Result<(), ExecutorFailure> {
        let budget = self;
        if budget.total_execution_wall.is_zero()
            || budget.total_execution_wall > Duration::from_secs(3600)
            || budget.startup_wall.is_zero()
            || budget.startup_wall > budget.total_execution_wall
            || budget.per_unit_wall.is_zero()
            || budget.per_unit_wall > budget.total_execution_wall
            || budget.cleanup_grace > Duration::from_secs(1)
            || budget.cpu_seconds == 0
            || budget.cpu_seconds > 3600
            || budget.address_space_bytes < 64 * 1024 * 1024
            || budget.address_space_bytes > 8 * 1024 * 1024 * 1024
            || budget.max_units == 0
            || budget.max_units > MAX_BATCH_UNITS
            || budget.max_total_raw_bytes == 0
            || budget.max_total_raw_bytes > MAX_BATCH_RAW_BYTES
        {
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        Ok(())
    }
    pub const MAX_UNITS: usize = MAX_BATCH_UNITS;
    pub const MAX_RAW_BYTES: usize = MAX_BATCH_RAW_BYTES;
    pub fn laboratory() -> Self {
        Self {
            total_execution_wall: Duration::from_secs(60),
            startup_wall: Duration::from_secs(10),
            per_unit_wall: Duration::from_secs(5),
            cleanup_grace: Duration::from_millis(200),
            cpu_seconds: 30,
            address_space_bytes: 1024 * 1024 * 1024,
            max_units: MAX_BATCH_UNITS,
            max_total_raw_bytes: MAX_BATCH_RAW_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchUnitVerdict {
    SchemaValid,
    SchemaInvalid,
    InputRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchUnitReceipt {
    pub ordinal: u64,
    pub member_id: String,
    pub relative_path: String,
    pub root_uri: String,
    pub raw_sha256: Digest256,
    pub unit_sha256: Digest256,
    pub verdict: BatchUnitVerdict,
}

/// This proves only exact transport coverage of the caller-supplied manifest.
/// It is not a corpus membership root or a validation attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchCoverageCheckpoint {
    pub worker_sha256: Digest256,
    pub request_sha256: Digest256,
    pub profile: FormatProfile,
    pub schema_set_sha256: Digest256,
    pub ordered_manifest_sha256: Digest256,
    pub completed_count: u64,
    pub result_stream_sha256: Digest256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchOutcome {
    Complete {
        receipts: Vec<BatchUnitReceipt>,
        checkpoint: BatchCoverageCheckpoint,
    },
    Incomplete {
        receipts: Vec<BatchUnitReceipt>,
        checkpoint: BatchCoverageCheckpoint,
        reason: ExecutorFailure,
        exchange: Option<ExchangeFailureContext>,
    },
}

/// Finite transport profile for a sequence of disposable batch processes.
/// It is not a source-universe limit or source-membership authority.
#[derive(Debug, Clone, Copy)]
pub struct BatchStreamBudget {
    pub batch: BatchBudget,
    pub max_chunks: u64,
    pub max_total_units: u64,
    pub max_total_raw_bytes: u64,
    pub total_execution_wall: Duration,
    /// Cumulative limits for one operation-owned isolated child, not per frame.
    pub operation_cpu_seconds: u64,
    pub operation_address_space_bytes: u64,
    pub max_total_wire_bytes: u64,
    pub max_distinct_selectors: usize,
}

impl BatchStreamBudget {
    /// Check the finite operation envelope without launching or granting execution.
    pub fn validate(self) -> Result<(), ExecutorFailure> {
        let budget = self;
        budget.batch.validate()?;
        if budget.max_chunks == 0
            || budget.max_total_units == 0
            || budget.max_total_raw_bytes == 0
            || budget.max_total_wire_bytes == 0
            || budget.max_distinct_selectors == 0
            || budget.max_distinct_selectors == usize::MAX
            || budget.max_chunks == u64::MAX
            || budget.max_total_units == u64::MAX
            || budget.max_total_raw_bytes == u64::MAX
            || budget.max_total_wire_bytes == u64::MAX
            || budget.total_execution_wall.is_zero()
            || budget.total_execution_wall > Duration::from_secs(3600)
            || budget.operation_cpu_seconds == 0
            || budget.operation_cpu_seconds > 3600
            || budget.operation_address_space_bytes < 64 * 1024 * 1024
            || budget.operation_address_space_bytes > 8 * 1024 * 1024 * 1024
        {
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        Ok(())
    }
    pub fn laboratory() -> Self {
        Self {
            batch: BatchBudget::laboratory(),
            max_chunks: 64,
            max_total_units: 4096,
            max_total_raw_bytes: 128 * 1024 * 1024,
            total_execution_wall: Duration::from_secs(300),
            operation_cpu_seconds: 30,
            operation_address_space_bytes: 1024 * 1024 * 1024,
            max_total_wire_bytes: 2 * 128 * 1024 * 1024 + 32 * 1024 * 1024,
            max_distinct_selectors: MAX_BATCH_UNITS,
        }
    }
}

/// Opaque invocation-local budget shared by explicitly attached diagnostics-v2
/// schema executors. There is no default, global cache, reset, or wire field.
/// Long-lived owner executors attach before their own first request; a fresh
/// serial worker image may attach the same still-healthy handle later. Committed
/// usage is cumulative and attaching never changes it.
#[derive(Clone)]
pub struct SharedSchemaWorkerQuota {
    inner: Arc<Mutex<SharedSchemaWorkerQuotaState>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedSchemaWorkerQuotaUsage {
    pub max_total_cpu_micros: u64,
    pub max_total_wire_bytes: u64,
    pub max_total_units: u64,
    pub worker_cpu_micros: u64,
    pub worker_wire_bytes: u64,
    pub worker_units: u64,
}

struct SharedSchemaWorkerQuotaState {
    usage: SharedSchemaWorkerQuotaUsage,
    next_token: u64,
    in_flight: Option<u64>,
    poisoned: bool,
}

impl SharedSchemaWorkerQuota {
    /// Create the immutable whole-invocation ceilings. All dimensions are
    /// finite and positive; `max_total_cpu_micros` is exact accounting while
    /// each child still receives Linux's integer-second rlimit.
    pub fn new(
        max_total_cpu_micros: u64,
        max_total_wire_bytes: u64,
        max_total_units: u64,
    ) -> Result<Self, ExecutorFailure> {
        if max_total_cpu_micros == 0
            || max_total_cpu_micros > 3_600_000_000
            || max_total_wire_bytes == 0
            || max_total_wire_bytes == u64::MAX
            || max_total_units == 0
            || max_total_units == u64::MAX
        {
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(SharedSchemaWorkerQuotaState {
                usage: SharedSchemaWorkerQuotaUsage {
                    max_total_cpu_micros,
                    max_total_wire_bytes,
                    max_total_units,
                    worker_cpu_micros: 0,
                    worker_wire_bytes: 0,
                    worker_units: 0,
                },
                next_token: 1,
                in_flight: None,
                poisoned: false,
            })),
        })
    }

    /// Return actual committed terminal usage. Unknown, poisoned, or
    /// in-flight executions never appear as a zero-cost snapshot.
    pub fn usage(&self) -> Result<SharedSchemaWorkerQuotaUsage, ExecutorFailure> {
        let state = self
            .inner
            .lock()
            .map_err(|_| ExecutorFailure::ResourceLimitUnknown)?;
        if state.poisoned || state.in_flight.is_some() {
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        Ok(state.usage)
    }

    pub(crate) fn ensure_attachable(&self) -> Result<(), ExecutorFailure> {
        let state = self
            .inner
            .lock()
            .map_err(|_| ExecutorFailure::ResourceLimitUnknown)?;
        if state.poisoned || state.in_flight.is_some() {
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        Ok(())
    }

    fn child_cpu_seconds(&self, requested: u64) -> Result<u64, ExecutorFailure> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| ExecutorFailure::ResourceLimitUnknown)?;
        if state.poisoned || state.in_flight.is_some() || requested == 0 {
            state.poisoned = true;
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        let Some(remaining) = state
            .usage
            .max_total_cpu_micros
            .checked_sub(state.usage.worker_cpu_micros)
        else {
            state.poisoned = true;
            return Err(ExecutorFailure::ResourceLimitUnknown);
        };
        if remaining == 0 {
            state.poisoned = true;
            return Err(ExecutorFailure::CpuLimit);
        }
        Ok(requested.min(remaining.div_ceil(1_000_000).max(1)))
    }

    fn begin(
        &self,
        request_bytes: usize,
        minimum_response_bytes: usize,
        maximum_response_bytes: usize,
        requested_child_cpu_seconds: u64,
        units: u64,
    ) -> Result<SharedSchemaWorkerReservation, ExecutorFailure> {
        let request_bytes =
            u64::try_from(request_bytes).map_err(|_| ExecutorFailure::InputBudget)?;
        let minimum_response_bytes =
            u64::try_from(minimum_response_bytes).map_err(|_| ExecutorFailure::InputBudget)?;
        let maximum_response_bytes =
            u64::try_from(maximum_response_bytes).map_err(|_| ExecutorFailure::InputBudget)?;
        let mut state = self
            .inner
            .lock()
            .map_err(|_| ExecutorFailure::ResourceLimitUnknown)?;
        if state.poisoned || state.in_flight.is_some() {
            state.poisoned = true;
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        if units == 0 {
            state.poisoned = true;
            return Err(ExecutorFailure::InputBudget);
        }
        let remaining_cpu_micros = state
            .usage
            .max_total_cpu_micros
            .checked_sub(state.usage.worker_cpu_micros);
        let remaining_wire_bytes = state
            .usage
            .max_total_wire_bytes
            .checked_sub(state.usage.worker_wire_bytes);
        let remaining_units = state
            .usage
            .max_total_units
            .checked_sub(state.usage.worker_units);
        let admission_error = if remaining_units.is_none_or(|remaining| remaining < units) {
            Some(ExecutorFailure::InputBudget)
        } else if remaining_cpu_micros.is_none_or(|remaining| remaining == 0)
            || requested_child_cpu_seconds == 0
        {
            Some(ExecutorFailure::CpuLimit)
        } else if remaining_wire_bytes.is_none_or(|remaining| {
            request_bytes
                .checked_add(minimum_response_bytes)
                .and_then(|bytes| bytes.checked_add(1))
                .is_none_or(|minimum| minimum > remaining)
        }) {
            Some(ExecutorFailure::InputBudget)
        } else {
            None
        };
        if let Some(reason) = admission_error {
            state.poisoned = true;
            return Err(reason);
        }
        let remaining_cpu_micros = remaining_cpu_micros.unwrap();
        let remaining_wire_bytes = remaining_wire_bytes.unwrap();
        let response_capacity = remaining_wire_bytes
            .checked_sub(request_bytes)
            .and_then(|bytes| bytes.checked_sub(1))
            .ok_or(ExecutorFailure::InputBudget)?
            .min(maximum_response_bytes);
        if response_capacity < minimum_response_bytes {
            state.poisoned = true;
            return Err(ExecutorFailure::InputBudget);
        }
        let child_cpu_seconds =
            requested_child_cpu_seconds.min(remaining_cpu_micros.div_ceil(1_000_000).max(1));
        let token = state.next_token;
        let Some(next_token) = token.checked_add(1) else {
            state.poisoned = true;
            return Err(ExecutorFailure::ResourceLimitUnknown);
        };
        state.next_token = next_token;
        state.in_flight = Some(token);
        Ok(SharedSchemaWorkerReservation {
            quota: self.clone(),
            token,
            request_bytes,
            minimum_response_bytes,
            maximum_response_bytes: response_capacity,
            child_cpu_seconds,
            units,
            settled: false,
        })
    }

    pub(crate) fn poison(&self) {
        if let Ok(mut state) = self.inner.lock() {
            state.poisoned = true;
            state.in_flight = None;
        }
    }
}

struct SharedSchemaWorkerReservation {
    quota: SharedSchemaWorkerQuota,
    token: u64,
    request_bytes: u64,
    minimum_response_bytes: u64,
    maximum_response_bytes: u64,
    child_cpu_seconds: u64,
    units: u64,
    settled: bool,
}

impl SharedSchemaWorkerReservation {
    fn response_cap(&self) -> usize {
        usize::try_from(self.maximum_response_bytes).unwrap_or(usize::MAX)
    }

    fn child_cpu_seconds(&self) -> u64 {
        self.child_cpu_seconds
    }

    fn complete(
        mut self,
        request_bytes: usize,
        response_bytes: usize,
        cpu_micros: Option<u64>,
        completed_units: u64,
    ) -> Result<(), ExecutorFailure> {
        let request_bytes =
            u64::try_from(request_bytes).map_err(|_| ExecutorFailure::InputBudget)?;
        let response_bytes =
            u64::try_from(response_bytes).map_err(|_| ExecutorFailure::InputBudget)?;
        let mut state = self
            .quota
            .inner
            .lock()
            .map_err(|_| ExecutorFailure::ResourceLimitUnknown)?;
        let invalid = state.poisoned
            || state.in_flight != Some(self.token)
            || request_bytes != self.request_bytes
            || response_bytes < self.minimum_response_bytes
            || response_bytes > self.maximum_response_bytes
            || cpu_micros.is_none()
            || completed_units != self.units;
        if invalid {
            state.poisoned = true;
            state.in_flight = None;
            self.settled = true;
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        let cpu_micros = cpu_micros.unwrap();
        let next_cpu = state
            .usage
            .worker_cpu_micros
            .checked_add(cpu_micros)
            .filter(|used| *used <= state.usage.max_total_cpu_micros);
        let next_wire = request_bytes
            .checked_add(response_bytes)
            .and_then(|exchange| state.usage.worker_wire_bytes.checked_add(exchange))
            .filter(|used| *used <= state.usage.max_total_wire_bytes);
        let next_units = state
            .usage
            .worker_units
            .checked_add(completed_units)
            .filter(|used| *used <= state.usage.max_total_units);
        let (Some(next_cpu), Some(next_wire), Some(next_units)) = (next_cpu, next_wire, next_units)
        else {
            state.poisoned = true;
            state.in_flight = None;
            self.settled = true;
            return Err(if next_cpu.is_none() {
                ExecutorFailure::CpuLimit
            } else {
                ExecutorFailure::InputBudget
            });
        };
        state.usage.worker_cpu_micros = next_cpu;
        state.usage.worker_wire_bytes = next_wire;
        state.usage.worker_units = next_units;
        state.in_flight = None;
        self.settled = true;
        Ok(())
    }
}

impl Drop for SharedSchemaWorkerReservation {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        if let Ok(mut state) = self.quota.inner.lock() {
            if state.in_flight == Some(self.token) {
                state.in_flight = None;
                state.poisoned = true;
            }
        }
    }
}

pub(crate) fn validate_batch_unit(unit: &BatchUnit) -> Result<(), ExecutorFailure> {
    if unit.member_id.is_empty()
        || unit.member_id.len() > MAX_MEMBER_ID_BYTES
        || unit.relative_path.is_empty()
        || unit.relative_path.len() > MAX_PATH_BYTES
        || unit.relative_path.starts_with('/')
        || unit.relative_path.split('/').any(|part| part == "..")
        || unit.root_uri.len() > MAX_URI_BYTES
        || unit.raw_instance.len() > crate::SchemaBackendProbe::MAX_INSTANCE_BYTES
    {
        return Err(ExecutorFailure::InputBudget);
    }
    Ok(())
}

fn validate_diagnostics_batch_unit(
    unit: &BatchUnit,
    input_mode: DiagnosticsUnitInputMode,
) -> Result<(), ExecutorFailure> {
    validate_diagnostics_batch_unit_fields(
        &unit.member_id,
        &unit.relative_path,
        &unit.root_uri,
        unit.raw_instance.len(),
        input_mode,
    )
}

fn validate_diagnostics_batch_unit_fields(
    member_id: &str,
    relative_path: &str,
    root_uri: &str,
    raw_instance_bytes: usize,
    input_mode: DiagnosticsUnitInputMode,
) -> Result<(), ExecutorFailure> {
    let raw_limit = match input_mode {
        DiagnosticsUnitInputMode::FiniteJson => crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
        DiagnosticsUnitInputMode::LegacyPythonObserved
        | DiagnosticsUnitInputMode::FiniteJsonSelected
        | DiagnosticsUnitInputMode::LegacyPythonObservedSelected => MAX_BATCH_RAW_BYTES,
    };
    if member_id.is_empty()
        || member_id.len() > MAX_MEMBER_ID_BYTES
        || relative_path.is_empty()
        || relative_path.len() > MAX_PATH_BYTES
        || relative_path.starts_with('/')
        || relative_path.split('/').any(|part| part == "..")
        || root_uri.len() > MAX_URI_BYTES
        || raw_instance_bytes > raw_limit
    {
        return Err(ExecutorFailure::InputBudget);
    }
    Ok(())
}

pub(crate) fn batch_unit_digest(unit: &BatchUnit) -> Result<Digest256, ExecutorFailure> {
    validate_batch_unit(unit)?;
    let mut digest = tos_foundation::Digest256Hasher::new();
    digest.update(b"tos-val2-batch-unit-v1\0");
    digest.update(&unit.ordinal.to_be_bytes());
    for value in [
        unit.member_id.as_bytes(),
        unit.relative_path.as_bytes(),
        unit.root_uri.as_bytes(),
        unit.raw_instance.as_slice(),
    ] {
        digest.update(&(value.len() as u32).to_be_bytes());
        digest.update(value);
    }
    Ok(digest.finalize())
}

impl BatchCoverageExpectation {
    /// Exact transport manifest only; this does not establish source coverage.
    pub fn from_units(units: &[BatchUnit]) -> Result<Self, ExecutorFailure> {
        if units.is_empty() || units.len() > MAX_BATCH_UNITS {
            return Err(ExecutorFailure::InputBudget);
        }
        let mut manifest = tos_foundation::Digest256Hasher::new();
        manifest.update(b"tos-val2-batch-manifest-v1\0");
        for (ordinal, unit) in units.iter().enumerate() {
            if unit.ordinal != ordinal as u64 {
                return Err(ExecutorFailure::CoverageMismatch);
            }
            manifest.update(batch_unit_digest(unit)?.as_bytes());
        }
        Ok(Self {
            count: units.len() as u64,
            ordered_manifest_sha256: manifest.finalize(),
        })
    }

    pub(crate) fn from_diagnostics_units(
        units: &[BatchUnit],
        input_profile: DiagnosticsInputProfile,
        unit_modes: Option<&[DiagnosticsUnitInputMode]>,
    ) -> Result<Self, ExecutorFailure> {
        if units.is_empty()
            || units.len() > MAX_BATCH_UNITS
            || input_profile == DiagnosticsInputProfile::LegacyPythonObservedSelected
            || matches!(
                input_profile,
                DiagnosticsInputProfile::MixedSourceFoundation
            ) != unit_modes.is_some()
            || unit_modes.is_some_and(|modes| modes.len() != units.len())
        {
            return Err(ExecutorFailure::InputBudget);
        }
        let mut manifest = tos_foundation::Digest256Hasher::new();
        manifest.update(b"tos-val2-batch-manifest-v1\0");
        for (ordinal, unit) in units.iter().enumerate() {
            if unit.ordinal != ordinal as u64 {
                return Err(ExecutorFailure::CoverageMismatch);
            }
            let input_mode = match input_profile {
                DiagnosticsInputProfile::FiniteJson => DiagnosticsUnitInputMode::FiniteJson,
                DiagnosticsInputProfile::FiniteJsonSelected => {
                    DiagnosticsUnitInputMode::FiniteJsonSelected
                }
                DiagnosticsInputProfile::LegacyPythonObserved => {
                    DiagnosticsUnitInputMode::LegacyPythonObserved
                }
                DiagnosticsInputProfile::LegacyPythonObservedSelected => {
                    return Err(ExecutorFailure::InputBudget);
                }
                DiagnosticsInputProfile::MixedSourceFoundation => *unit_modes
                    .and_then(|modes| modes.get(ordinal))
                    .ok_or(ExecutorFailure::InputBudget)?,
            };
            manifest.update(diagnostics_batch_unit_digest(unit, input_mode)?.as_bytes());
        }
        Ok(Self {
            count: units.len() as u64,
            ordered_manifest_sha256: manifest.finalize(),
        })
    }

    pub(crate) fn from_selected_legacy_diagnostics_units(
        units: &[BatchUnit],
        limits: LegacySelectedDiagnosticsLimits,
    ) -> Result<Self, ExecutorFailure> {
        limits.validate()?;
        if units.is_empty() || units.len() > MAX_BATCH_UNITS {
            return Err(ExecutorFailure::InputBudget);
        }
        let mut manifest = Digest256Hasher::new();
        manifest.update(b"tos-val2-batch-manifest-v1\0");
        for (ordinal, unit) in units.iter().enumerate() {
            if unit.ordinal != ordinal as u64 {
                return Err(ExecutorFailure::CoverageMismatch);
            }
            manifest
                .update(selected_legacy_diagnostics_batch_unit_digest(unit, limits)?.as_bytes());
        }
        Ok(Self {
            count: units.len() as u64,
            ordered_manifest_sha256: manifest.finalize(),
        })
    }
}

pub(crate) fn diagnostics_batch_unit_digest(
    unit: &BatchUnit,
    input_mode: DiagnosticsUnitInputMode,
) -> Result<Digest256, ExecutorFailure> {
    if input_mode == DiagnosticsUnitInputMode::LegacyPythonObservedSelected {
        return Err(ExecutorFailure::InputBudget);
    }
    validate_diagnostics_batch_unit(unit, input_mode)?;
    let mut digest = tos_foundation::Digest256Hasher::new();
    digest.update(b"tos-val2-batch-unit-v1\0");
    digest.update(&unit.ordinal.to_be_bytes());
    for value in [
        unit.member_id.as_bytes(),
        unit.relative_path.as_bytes(),
        unit.root_uri.as_bytes(),
        unit.raw_instance.as_slice(),
    ] {
        digest.update(&(value.len() as u32).to_be_bytes());
        digest.update(value);
    }
    Ok(digest.finalize())
}

pub(crate) fn selected_legacy_diagnostics_batch_unit_digest(
    unit: &BatchUnit,
    limits: LegacySelectedDiagnosticsLimits,
) -> Result<Digest256, ExecutorFailure> {
    limits.validate()?;
    validate_diagnostics_batch_unit_fields(
        &unit.member_id,
        &unit.relative_path,
        &unit.root_uri,
        unit.raw_instance.len(),
        DiagnosticsUnitInputMode::LegacyPythonObservedSelected,
    )?;
    if unit.raw_instance.len() > limits.max_instance_bytes {
        return Err(ExecutorFailure::InputBudget);
    }
    Ok(selected_legacy_diagnostics_unit_digest_fields(
        unit.ordinal,
        &unit.member_id,
        &unit.relative_path,
        &unit.root_uri,
        &unit.raw_instance,
        limits,
    ))
}

fn selected_legacy_diagnostics_unit_digest_fields(
    ordinal: u64,
    member_id: &str,
    relative_path: &str,
    root_uri: &str,
    raw_instance: &[u8],
    limits: LegacySelectedDiagnosticsLimits,
) -> Digest256 {
    let mut digest = Digest256Hasher::new();
    digest.update(b"tos-val2-batch-unit-legacy-selected-v1\0");
    digest.update(&ordinal.to_be_bytes());
    digest.update(&[DiagnosticsUnitInputMode::LegacyPythonObservedSelected.wire_byte()]);
    limits.update_digest(&mut digest);
    for value in [
        member_id.as_bytes(),
        relative_path.as_bytes(),
        root_uri.as_bytes(),
        raw_instance,
    ] {
        digest.update(&(value.len() as u32).to_be_bytes());
        digest.update(value);
    }
    digest.finalize()
}

fn unknown(reason: ExecutorFailure, identity: Option<ExecutionIdentity>) -> ExecutorOutcome {
    ExecutorOutcome::Indeterminate {
        reason,
        identity,
        exchange: None,
    }
}

fn empty_diagnostics_outcome(
    worker_sha256: Digest256,
    profile: FormatProfile,
    schema_set_sha256: Digest256,
    reason: ExecutorFailure,
) -> SchemaDiagnosticsOutcome {
    SchemaDiagnosticsOutcome::Incomplete {
        checkpoint: SchemaDiagnosticsCheckpoint {
            worker_sha256,
            request_sha256: Digest256::of_bytes(b""),
            profile,
            schema_set_sha256,
            ordered_manifest_sha256: Digest256::of_bytes(b""),
            caps_sha256: schema_diagnostics::Caps::CURRENT.digest(),
            completed_count: 0,
            result_stream_sha256: Digest256::of_bytes(b""),
            worker_request_bytes: 0,
            worker_response_bytes: 0,
            worker_cpu_micros: None,
            exceptional_remaining: None,
            exceptional_usage: None,
        },
        reason,
        exchange: None,
    }
}

pub struct BoundedSchemaExecutor;

impl BoundedSchemaExecutor {
    pub fn evaluate(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
        budget: ExecutorBudget,
    ) -> ExecutorOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate(
                worker,
                resources,
                profile,
                root_uri,
                raw_instance,
                budget,
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (worker, resources, profile, root_uri, raw_instance, budget);
            unknown(ExecutorFailure::UnsupportedHost, None)
        }
    }

    /// Cooperative cancellation checked before and after image verification
    /// and at every nonblocking parent poll. It cannot interrupt a blocked
    /// host filesystem read; cleanup retains the explicit grace budget.
    pub fn evaluate_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
        budget: ExecutorBudget,
        cancelled: &AtomicBool,
    ) -> ExecutorOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate(
                worker,
                resources,
                profile,
                root_uri,
                raw_instance,
                budget,
                Some(cancelled),
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                worker,
                resources,
                profile,
                root_uri,
                raw_instance,
                budget,
                cancelled,
            );
            unknown(ExecutorFailure::UnsupportedHost, None)
        }
    }

    pub fn evaluate_batch(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
    ) -> BatchOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch(worker, resources, profile, units, expected, budget, None)
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (worker, resources, profile, units, expected, budget);
            BatchOutcome::Incomplete {
                receipts: Vec::new(),
                checkpoint: BatchCoverageCheckpoint {
                    worker_sha256: worker.sha256,
                    request_sha256: Digest256::of_bytes(b""),
                    profile,
                    schema_set_sha256: Digest256::of_bytes(b""),
                    ordered_manifest_sha256: Digest256::of_bytes(b""),
                    completed_count: 0,
                    result_stream_sha256: Digest256::of_bytes(b""),
                },
                reason: ExecutorFailure::UnsupportedHost,
                exchange: None,
            }
        }
    }

    /// Evaluates a bounded batch through the opt-in schema-diagnostics v2
    /// protocol. A `Valid` unit status is returned only after the exact worker,
    /// request, schema set, caps, unit order and final result stream all match.
    pub fn evaluate_batch_with_diagnostics(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_diagnostics(
                worker,
                resources,
                profile,
                DiagnosticsInputProfile::FiniteJson,
                units,
                expected,
                budget,
                None,
                None,
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (resources, units, expected, budget);
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Cancellation-aware variant of [`Self::evaluate_batch_with_diagnostics`].
    pub fn evaluate_batch_with_diagnostics_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_diagnostics(
                worker,
                resources,
                profile,
                DiagnosticsInputProfile::FiniteJson,
                units,
                expected,
                budget,
                None,
                Some(cancelled),
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (resources, units, expected, budget, cancelled);
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Source-foundation-only variant with one caller-owned remaining IPC byte
    /// allowance. Before spawning, the controller reserves the exact request,
    /// the minimum complete response, and one overflow-sentinel byte. It then
    /// bounds this chunk's response by the smaller of the worker cap and the
    /// remaining whole-call allowance; a response exceeding that bound is
    /// incomplete. The checkpoint records exact controller-observed bytes.
    pub(crate) fn evaluate_batch_with_diagnostics_wire_limited_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        remaining_worker_wire_bytes: u64,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_diagnostics(
                worker,
                resources,
                profile,
                DiagnosticsInputProfile::FiniteJson,
                units,
                expected,
                budget,
                Some(remaining_worker_wire_bytes),
                Some(cancelled),
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                units,
                expected,
                budget,
                remaining_worker_wire_bytes,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Source-foundation finite-json profile with an explicitly selected
    /// per-instance ceiling up to the existing 32 MiB request cap. Ordinary
    /// finite diagnostics retain their independent one-MiB probe ceiling.
    pub(crate) fn evaluate_batch_with_selected_finite_diagnostics_wire_limited_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        remaining_worker_wire_bytes: u64,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_diagnostics(
                worker,
                resources,
                profile,
                DiagnosticsInputProfile::FiniteJsonSelected,
                units,
                expected,
                budget,
                Some(remaining_worker_wire_bytes),
                Some(cancelled),
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                units,
                expected,
                budget,
                remaining_worker_wire_bytes,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Evaluates exact raw instances using the FND legacy-Python observed
    /// decoder inside the same diagnostics-v2 worker. The original bytes stay
    /// the unit payload and digest input; nonfinite or otherwise unrepresentable
    /// Python values return typed indeterminate diagnostics.
    pub fn evaluate_batch_with_legacy_python_diagnostics_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_diagnostics(
                worker,
                resources,
                profile,
                DiagnosticsInputProfile::LegacyPythonObserved,
                units,
                expected,
                budget,
                None,
                Some(cancelled),
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (resources, units, expected, budget, cancelled);
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Source-foundation-only raw-input variant with whole-call remaining IPC
    /// bytes. Existing raw diagnostics callers retain their original signature
    /// and uncoupled transport envelope.
    pub(crate) fn evaluate_batch_with_legacy_python_diagnostics_wire_limited_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        remaining_worker_wire_bytes: u64,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_diagnostics(
                worker,
                resources,
                profile,
                DiagnosticsInputProfile::LegacyPythonObserved,
                units,
                expected,
                budget,
                Some(remaining_worker_wire_bytes),
                Some(cancelled),
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                units,
                expected,
                budget,
                remaining_worker_wire_bytes,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Evaluates an encounter-ordered finite/raw batch through one diagnostics-v2
    /// worker and one request clock. Each unit carries a closed input mode inside
    /// the existing extended frame; finite units retain the ordinary finite
    /// parser/backend, while raw units use the FND LegacyPythonObserved decoder.
    pub(crate) fn evaluate_batch_with_mixed_source_foundation_diagnostics_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = MixedDiagnosticsBatchUnit>,
        expected: BatchCoverageExpectation,
        exceptional_remaining: ExceptionalSchemaUsage,
        budget: BatchBudget,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_mixed_source_foundation_diagnostics(
                worker,
                resources,
                profile,
                units,
                expected,
                exceptional_remaining,
                budget,
                None,
                Some(cancelled),
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                units,
                expected,
                exceptional_remaining,
                budget,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Mixed source-foundation profile-2 variant consuming the same controller
    /// wire allowance as the finite and raw-only sibling paths.
    pub(crate) fn evaluate_batch_with_mixed_source_foundation_diagnostics_wire_limited_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = MixedDiagnosticsBatchUnit>,
        expected: BatchCoverageExpectation,
        exceptional_remaining: ExceptionalSchemaUsage,
        budget: BatchBudget,
        remaining_worker_wire_bytes: u64,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_mixed_source_foundation_diagnostics(
                worker,
                resources,
                profile,
                units,
                expected,
                exceptional_remaining,
                budget,
                Some(remaining_worker_wire_bytes),
                Some(cancelled),
                None,
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                units,
                expected,
                exceptional_remaining,
                budget,
                remaining_worker_wire_bytes,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Source-foundation finite profile with one shared invocation quota.
    /// Existing callers keep their uncoupled finite API and exact wire bytes.
    pub(crate) fn evaluate_batch_with_diagnostics_wire_limited_shared_quota_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        remaining_worker_wire_bytes: u64,
        quota: &SharedSchemaWorkerQuota,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_diagnostics(
                worker,
                resources,
                profile,
                DiagnosticsInputProfile::FiniteJson,
                units,
                expected,
                budget,
                Some(remaining_worker_wire_bytes),
                Some(cancelled),
                Some(quota.clone()),
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                units,
                expected,
                budget,
                remaining_worker_wire_bytes,
                quota,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Source-foundation selected-finite profile attached to the shared
    /// invocation quota. The value remains ordinary finite JSON throughout.
    pub(crate) fn evaluate_batch_with_selected_finite_diagnostics_wire_limited_shared_quota_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        remaining_worker_wire_bytes: u64,
        quota: &SharedSchemaWorkerQuota,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_diagnostics(
                worker,
                resources,
                profile,
                DiagnosticsInputProfile::FiniteJsonSelected,
                units,
                expected,
                budget,
                Some(remaining_worker_wire_bytes),
                Some(cancelled),
                Some(quota.clone()),
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                units,
                expected,
                budget,
                remaining_worker_wire_bytes,
                quota,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Source-foundation raw profile with one shared invocation quota.
    pub(crate) fn evaluate_batch_with_legacy_python_diagnostics_wire_limited_shared_quota_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        remaining_worker_wire_bytes: u64,
        quota: &SharedSchemaWorkerQuota,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_diagnostics(
                worker,
                resources,
                profile,
                DiagnosticsInputProfile::LegacyPythonObserved,
                units,
                expected,
                budget,
                Some(remaining_worker_wire_bytes),
                Some(cancelled),
                Some(quota.clone()),
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                units,
                expected,
                budget,
                remaining_worker_wire_bytes,
                quota,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Source-foundation mixed profile with one shared invocation quota.
    pub(crate) fn evaluate_batch_with_mixed_source_foundation_diagnostics_wire_limited_shared_quota_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = MixedDiagnosticsBatchUnit>,
        expected: BatchCoverageExpectation,
        exceptional_remaining: ExceptionalSchemaUsage,
        budget: BatchBudget,
        remaining_worker_wire_bytes: u64,
        quota: &SharedSchemaWorkerQuota,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch_with_mixed_source_foundation_diagnostics(
                worker,
                resources,
                profile,
                units,
                expected,
                exceptional_remaining,
                budget,
                Some(remaining_worker_wire_bytes),
                Some(cancelled),
                Some(quota.clone()),
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                units,
                expected,
                exceptional_remaining,
                budget,
                remaining_worker_wire_bytes,
                quota,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// One source-foundation diagnostics-v2 exchange using the caller's
    /// already verified immutable worker image and invocation-wide quota.
    /// Unit order and modes are retained in this single request; the image
    /// handle contributes no schema closure or protocol identity of its own.
    pub(crate) fn evaluate_source_foundation_diagnostics_with_image(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        input_profile: DiagnosticsInputProfile,
        units: Vec<BatchUnit>,
        unit_modes: Option<Vec<DiagnosticsUnitInputMode>>,
        exceptional_remaining: Option<ExceptionalSchemaUsage>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        remaining_worker_wire_bytes: u64,
        quota: &SharedSchemaWorkerQuota,
        image: &VerifiedWorkerImageHandle,
        cancelled: &AtomicBool,
    ) -> SchemaDiagnosticsOutcome {
        if worker.sha256 != image.identity().sha256
            || worker.absolute_path != image.identity().absolute_path
        {
            quota.poison();
            return empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::WorkerIdentity,
            );
        }
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            let outcome = native::evaluate_batch_with_diagnostics_units(
                worker,
                resources,
                profile,
                input_profile,
                units,
                unit_modes,
                exceptional_remaining,
                Some(remaining_worker_wire_bytes),
                expected,
                budget,
                Some(cancelled),
                Some(quota.clone()),
                Some(image),
            );
            if matches!(&outcome, SchemaDiagnosticsOutcome::Incomplete { .. }) {
                quota.poison();
            }
            outcome
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                resources,
                input_profile,
                units,
                unit_modes,
                exceptional_remaining,
                expected,
                budget,
                remaining_worker_wire_bytes,
                quota,
                image,
                cancelled,
            );
            empty_diagnostics_outcome(
                worker.sha256,
                profile,
                Digest256::of_bytes(b""),
                ExecutorFailure::UnsupportedHost,
            )
        }
    }

    /// Same finite protocol, with cooperative cancellation during parent polls.
    pub fn evaluate_batch_cancellable(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        cancelled: &AtomicBool,
    ) -> BatchOutcome {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        {
            native::evaluate_batch(
                worker,
                resources,
                profile,
                units,
                expected,
                budget,
                Some(cancelled),
            )
        }
        #[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
        {
            let _ = (
                worker, resources, profile, units, expected, budget, cancelled,
            );
            BatchOutcome::Incomplete {
                receipts: Vec::new(),
                checkpoint: BatchCoverageCheckpoint {
                    worker_sha256: worker.sha256,
                    request_sha256: Digest256::of_bytes(b""),
                    profile,
                    schema_set_sha256: Digest256::of_bytes(b""),
                    ordered_manifest_sha256: Digest256::of_bytes(b""),
                    completed_count: 0,
                    result_stream_sha256: Digest256::of_bytes(b""),
                },
                reason: ExecutorFailure::UnsupportedHost,
                exchange: None,
            }
        }
    }
}

/// Called only by the dedicated executable. Its process limits are imposed by
/// the parent before `exec`; a direct call to this function has no such limit.
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub fn worker_once() -> std::io::Result<()> {
    native::worker_once()
}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub fn worker_once() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "bounded schema worker requires Linux process limits",
    ))
}

// Operation-local custody, used by the selected-cut and record-plan adapters.
// No path lookup or image preparation is repeated after successful creation.
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(crate) use native::{PreparedSchemaWorker, VerifiedWorkerImage};

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub(crate) struct VerifiedWorkerImage;
#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
impl VerifiedWorkerImage {
    pub(crate) fn image_bytes(&self) -> Result<u64, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn from_handle(
        _: &VerifiedWorkerImageHandle,
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn exchange_failure(&self) -> Option<ExchangeFailureContext> {
        None
    }
    pub(crate) fn preflight(&mut self, _: Instant, _: &AtomicBool) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn poison(&mut self, reason: ExecutorFailure) -> ExecutorFailure {
        reason
    }
    pub(crate) fn set_operation_budget(
        &mut self,
        _: BatchStreamBudget,
    ) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn set_shared_schema_worker_quota(
        &mut self,
        _: SharedSchemaWorkerQuota,
    ) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn poison_shared_schema_worker_quota(&self) {}
    pub(crate) fn finish(&mut self, _: Instant, _: &AtomicBool) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn set_operation_origin(&mut self, _: Instant) {}

    pub(crate) fn prepare(
        _: &ExactWorkerIdentity,
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn matches(&mut self, _: &ExactWorkerIdentity) -> bool {
        false
    }
    pub(crate) fn evaluate(
        &mut self,
        _: &[SchemaResource],
        _: FormatProfile,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> ExecutorOutcome {
        unknown(ExecutorFailure::UnsupportedHost, None)
    }
    pub(crate) fn evaluate_with_diagnostics(
        &mut self,
        _: &[SchemaResource],
        _: FormatProfile,
        _: &str,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    fn evaluate_with_diagnostics_encoded(
        &mut self,
        _: &[u8],
        _: Digest256,
        _: FormatProfile,
        _: DiagnosticsInputProfile,
        _: Option<LegacySelectedDiagnosticsLimits>,
        _: Option<usize>,
        _: &str,
        _: &str,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
}

#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
pub(crate) struct PreparedSchemaWorker;
#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))]
impl PreparedSchemaWorker {
    pub(crate) fn exchange_failure(&self) -> Option<ExchangeFailureContext> {
        None
    }
    pub(crate) fn preflight(&mut self, _: Instant, _: &AtomicBool) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn poison(&mut self, reason: ExecutorFailure) -> ExecutorFailure {
        reason
    }
    pub(crate) fn operation_budget(&self) -> BatchStreamBudget {
        BatchStreamBudget::laboratory()
    }
    pub(crate) fn release_child(
        &mut self,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn wire_cost(
        &self,
        _: usize,
        _: usize,
        _: usize,
        _: usize,
    ) -> Result<(u64, u64, u64), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn has_encoded_schema_resource(&self, _: &str, _: &[u8]) -> bool {
        false
    }
    pub(crate) fn max_encoded_schema_uri_bytes(&self) -> Result<usize, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn encoded_schema_resource_buffer_bytes(&self) -> usize {
        0
    }
    pub(crate) fn diagnostics_v2_request_frame_bytes_upper_bound(
        &self,
        _: usize,
        _: usize,
        _: usize,
        _: usize,
    ) -> Result<usize, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn diagnostics_v2_request_response_bytes_upper_bound(
        &self,
        _: usize,
    ) -> Result<(usize, usize), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn diagnostics_v2_selected_legacy_request_frame_bytes_upper_bound(
        &self,
        _: usize,
        _: usize,
        _: usize,
        _: usize,
    ) -> Result<usize, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn worker_image_bytes(&self) -> Result<u64, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn diagnostics_v2_selected_legacy_response_buffer_bytes_upper_bound(
        &self,
    ) -> Result<usize, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn set_operation_budget(
        &mut self,
        _: BatchStreamBudget,
    ) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn set_shared_schema_worker_quota(
        &mut self,
        _: SharedSchemaWorkerQuota,
    ) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn poison_shared_schema_worker_quota(&self) {}
    pub(crate) fn finish(&mut self, _: Instant, _: &AtomicBool) -> Result<(), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn set_operation_origin(&mut self, _: Instant) {}

    pub(crate) fn prepare(
        _: &ExactWorkerIdentity,
        _: &[SchemaResource],
        _: FormatProfile,
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn prepare_with_image(
        _: &VerifiedWorkerImageHandle,
        _: &[SchemaResource],
        _: FormatProfile,
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<Self, ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn evaluate(
        &mut self,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> ExecutorOutcome {
        unknown(ExecutorFailure::UnsupportedHost, None)
    }
    pub(crate) fn evaluate_with_diagnostics(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn evaluate_with_legacy_diagnostics(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn evaluate_with_selected_legacy_diagnostics(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: &[u8],
        _: LegacySelectedDiagnosticsLimits,
        _: usize,
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn evaluate_with_selected_finite_diagnostics(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: &[u8],
        _: ExecutorBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure> {
        Err(ExecutorFailure::UnsupportedHost)
    }
    pub(crate) fn evaluate_batch(
        &mut self,
        _: &[BatchUnit],
        _: BatchCoverageExpectation,
        _: BatchBudget,
        _: Instant,
        _: &AtomicBool,
    ) -> BatchOutcome {
        BatchOutcome::Incomplete {
            receipts: Vec::new(),
            checkpoint: BatchCoverageCheckpoint {
                worker_sha256: Digest256::of_bytes(b""),
                request_sha256: Digest256::of_bytes(b""),
                profile: FormatProfile::AssertedSourceCandidateV1,
                schema_set_sha256: Digest256::of_bytes(b""),
                ordered_manifest_sha256: Digest256::of_bytes(b""),
                completed_count: 0,
                result_stream_sha256: Digest256::of_bytes(b""),
            },
            reason: ExecutorFailure::UnsupportedHost,
            exchange: None,
        }
    }
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
#[path = "native"]
mod native {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::thread;
    use std::time::Instant;
    use tos_foundation::{
        Digest256Hasher, FoundationErrorCode, JsonLimits, JsonMode, JsonNumberKind, JsonValue,
        parse_json, parse_json_with_state_budget,
    };

    #[path = "source_foundation_exceptional_schema.rs"]
    mod exceptional_schema;

    // Linux UAPI MFD_EXEC. Requiring this flag fails closed on older kernels
    // or hosts that refuse executable anonymous files.
    const MFD_EXEC_FLAG: u32 = 0x0010;
    const MAX_WORKER_BYTES: u64 = super::MAX_WORKER_IMAGE_BYTES;

    #[cfg(test)]
    thread_local! {
        static TEST_CHILD_STDOUT_INODE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }

    fn profile_byte(profile: FormatProfile) -> u8 {
        match profile {
            FormatProfile::LegacyPythonObserved20260923 => 1,
            FormatProfile::AssertedSourceCandidateV1 => 2,
        }
    }

    fn operation_header(
        nonce: [u8; 16],
        schema_set: Digest256,
        profile: FormatProfile,
        limits: [u64; 8],
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(OPERATION_HEADER_BYTES);
        header.extend_from_slice(OPERATION_REQUEST_MAGIC);
        header.extend_from_slice(&nonce);
        header.extend_from_slice(schema_set.as_bytes());
        header.push(profile_byte(profile));
        for n in limits {
            header.extend_from_slice(&n.to_be_bytes());
        }
        header
    }

    fn parse_profile(value: u8) -> Option<FormatProfile> {
        match value {
            1 => Some(FormatProfile::LegacyPythonObserved20260923),
            2 => Some(FormatProfile::AssertedSourceCandidateV1),
            _ => None,
        }
    }

    fn put_batch_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), ExecutorFailure> {
        let size = u32::try_from(value.len()).map_err(|_| ExecutorFailure::InputBudget)?;
        if output
            .len()
            .checked_add(4)
            .and_then(|len| len.checked_add(value.len()))
            .filter(|len| *len <= MAX_BATCH_FRAME_BYTES)
            .is_none()
        {
            return Err(ExecutorFailure::InputBudget);
        }
        output.extend_from_slice(&size.to_be_bytes());
        output.extend_from_slice(value);
        Ok(())
    }

    fn schema_set_digest(resources: &[SchemaResource]) -> Result<Digest256, ExecutorFailure> {
        let mut members = BTreeMap::new();
        for resource in resources {
            if members
                .insert(resource.uri.as_str(), Digest256::of_bytes(&resource.raw))
                .is_some()
            {
                return Err(ExecutorFailure::Backend);
            }
        }
        let mut digest = Digest256Hasher::new();
        digest.update(b"tos-schema-set-v1\0");
        for (uri, raw_digest) in members {
            digest.update(&(uri.len() as u64).to_be_bytes());
            digest.update(uri.as_bytes());
            digest.update(raw_digest.as_bytes());
        }
        Ok(digest.finalize())
    }

    #[derive(Clone)]
    struct BatchUnitMeta {
        ordinal: u64,
        member_id: String,
        relative_path: String,
        root_uri: String,
        raw_sha256: Digest256,
        unit_sha256: Digest256,
    }

    #[derive(Clone)]
    struct BatchPrepared {
        frame: Vec<u8>,
        units: Vec<BatchUnitMeta>,
        worker_sha256: Digest256,
        profile: FormatProfile,
        schema_set_sha256: Digest256,
        request_sha256: Digest256,
        ordered_manifest_sha256: Digest256,
    }

    struct DiagnosticsPrepared {
        frame: Vec<u8>,
        units: Vec<BatchUnitMeta>,
        worker_sha256: Digest256,
        profile: FormatProfile,
        schema_set_sha256: Digest256,
        request_sha256: Digest256,
        ordered_manifest_sha256: Digest256,
        caps: schema_diagnostics::Caps,
        exceptional_remaining: Option<ExceptionalSchemaUsage>,
    }

    impl DiagnosticsPrepared {
        fn final_record_bytes(&self) -> usize {
            DIAGNOSTIC_FINAL_BYTES
                + if self.exceptional_remaining.is_some() {
                    DIAGNOSTIC_EXCEPTIONAL_COUNTERS_BYTES
                } else {
                    0
                }
        }
    }

    fn empty_batch_outcome(
        worker: Digest256,
        profile: FormatProfile,
        schema: Digest256,
        reason: ExecutorFailure,
    ) -> BatchOutcome {
        let mut results = Digest256Hasher::new();
        results.update(b"tos-val2-batch-results-v1\0");
        BatchOutcome::Incomplete {
            receipts: Vec::new(),
            reason,
            exchange: None,
            checkpoint: BatchCoverageCheckpoint {
                worker_sha256: worker,
                request_sha256: Digest256::of_bytes(b""),
                profile,
                schema_set_sha256: schema,
                ordered_manifest_sha256: Digest256::of_bytes(b""),
                completed_count: 0,
                result_stream_sha256: results.finalize(),
            },
        }
    }

    fn encode_resources(
        resources: &[SchemaResource],
    ) -> Result<(Vec<u8>, Digest256), ExecutorFailure> {
        if resources.len() > crate::SchemaBackendProbe::MAX_RESOURCES {
            return Err(ExecutorFailure::InputBudget);
        }
        let mut total = 0usize;
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&(resources.len() as u32).to_be_bytes());
        for resource in resources {
            total = total
                .checked_add(resource.raw.len())
                .ok_or(ExecutorFailure::InputBudget)?;
            if resource.uri.len() > MAX_URI_BYTES
                || resource.raw.len() > crate::SchemaBackendProbe::MAX_RESOURCE_BYTES
                || total > crate::SchemaBackendProbe::MAX_TOTAL_BYTES
            {
                return Err(ExecutorFailure::InputBudget);
            }
            put_batch_bytes(&mut encoded, resource.uri.as_bytes())?;
            put_batch_bytes(&mut encoded, &resource.raw)?;
        }
        Ok((encoded, schema_set_digest(resources)?))
    }

    fn take_encoded_resource_field<'a>(
        encoded: &'a [u8],
        offset: &mut usize,
        limit: usize,
    ) -> Option<&'a [u8]> {
        let length_end = (*offset).checked_add(4)?;
        let length_bytes = encoded.get(*offset..length_end)?;
        let length = u32::from_be_bytes(length_bytes.try_into().ok()?) as usize;
        if length > limit {
            return None;
        }
        *offset = length_end;
        let end = offset.checked_add(length)?;
        let field = encoded.get(*offset..end)?;
        *offset = end;
        Some(field)
    }

    /// Exact immutable image owned for one bounded operation; no global cache.
    /// The same FD may launch independent disposable children with different
    /// schema plans. Resource and request identities remain separate.
    pub(crate) struct VerifiedWorkerImage {
        file: File,
        identity: ExactWorkerIdentity,
        operation_deadline: Instant,
        operation_budget: BatchStreamBudget,
        shared_schema_worker_quota: Option<SharedSchemaWorkerQuota>,
        session: Option<OwnedSchemaSession>,
        poisoned: Option<ExecutorFailure>,
        poison_exchange: Option<ExchangeFailureContext>,
        operation_started: Option<Instant>,
        used_frames: u64,
        used_units: u64,
        used_raw: u64,
        used_wire: u64,
        used_cpu_micros: u64,
        selectors: std::collections::BTreeSet<(String, String)>,
        selected_profile: Option<FormatProfile>,
    }

    struct OwnedSchemaSession {
        child: OperationChild,
        header: Vec<u8>,
        schema_set: Digest256,
        sequence: u64,
    }

    /// One exact encoded resource closure used by the selected-cut adapter.
    pub(crate) struct PreparedSchemaWorker {
        image: VerifiedWorkerImage,
        encoded_resources: Vec<u8>,
        schema_set_sha256: Digest256,
        profile: FormatProfile,
    }

    fn scalar_budget(budget: ExecutorBudget) -> Result<(), ExecutorFailure> {
        if budget.execution_wall.is_zero()
            || budget.cleanup_grace > Duration::from_secs(1)
            || budget.cpu_seconds == 0
            || budget.cpu_seconds > 60
            || budget.address_space_bytes < 64 * 1024 * 1024
            || budget.address_space_bytes > 8 * 1024 * 1024 * 1024
        {
            return Err(ExecutorFailure::ResourceLimitUnknown);
        }
        Ok(())
    }

    impl VerifiedWorkerImage {
        pub(crate) fn image_bytes(&self) -> Result<u64, ExecutorFailure> {
            let bytes = self
                .file
                .metadata()
                .map_err(|_| ExecutorFailure::WorkerIdentity)?
                .len();
            if bytes == 0 || bytes > MAX_WORKER_BYTES {
                return Err(ExecutorFailure::WorkerIdentity);
            }
            Ok(bytes)
        }

        pub(crate) fn prepare(
            worker: &ExactWorkerIdentity,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            let operation_started = Instant::now();
            scalar_budget(budget)?;
            let deadline = Instant::now()
                .checked_add(budget.execution_wall)
                .ok_or(ExecutorFailure::ResourceLimitUnknown)?
                .min(operation_deadline);
            let file = sealed_worker_checked(worker, Some(deadline), Some(cancelled))?;
            preparation_check(Some(deadline), Some(cancelled))?;
            Self::from_file(
                file,
                worker.clone(),
                budget,
                operation_deadline,
                operation_started,
            )
        }

        pub(super) fn into_handle(self) -> VerifiedWorkerImageHandle {
            VerifiedWorkerImageHandle {
                file: self.file,
                identity: self.identity,
                operation_deadline: self.operation_deadline,
            }
        }

        pub(crate) fn from_handle(
            handle: &VerifiedWorkerImageHandle,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            scalar_budget(budget)?;
            let operation_deadline = operation_deadline.min(handle.operation_deadline);
            preparation_check(Some(operation_deadline), Some(cancelled))?;
            let file = handle
                .file
                .try_clone()
                .map_err(|_| ExecutorFailure::WorkerIdentity)?;
            preparation_check(Some(operation_deadline), Some(cancelled))?;
            Self::from_file(
                file,
                handle.identity.clone(),
                budget,
                operation_deadline,
                Instant::now(),
            )
        }

        fn from_file(
            file: File,
            identity: ExactWorkerIdentity,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            operation_started: Instant,
        ) -> Result<Self, ExecutorFailure> {
            let mut operation_budget = BatchStreamBudget::laboratory();
            operation_budget.batch.total_execution_wall = budget.execution_wall;
            operation_budget.batch.startup_wall = budget.execution_wall;
            operation_budget.batch.per_unit_wall = budget.execution_wall;
            operation_budget.batch.cleanup_grace = budget.cleanup_grace;
            operation_budget.operation_cpu_seconds = budget.cpu_seconds;
            operation_budget.operation_address_space_bytes = budget.address_space_bytes;
            Ok(Self {
                file,
                identity,
                operation_deadline,
                operation_budget,
                shared_schema_worker_quota: None,
                session: None,
                poisoned: None,
                poison_exchange: None,
                operation_started: Some(operation_started),
                used_frames: 0,
                used_units: 0,
                used_raw: 0,
                used_wire: 0,
                used_cpu_micros: 0,
                selectors: std::collections::BTreeSet::new(),
                selected_profile: None,
            })
        }

        pub(crate) fn set_operation_budget(
            &mut self,
            budget: BatchStreamBudget,
        ) -> Result<(), ExecutorFailure> {
            if self.used_frames != 0 || self.session.is_some() || self.poisoned.is_some() {
                return Err(ExecutorFailure::ResourceLimitUnknown);
            }
            budget.validate()?;
            self.operation_budget = budget;
            Ok(())
        }
        pub(crate) fn set_shared_schema_worker_quota(
            &mut self,
            quota: SharedSchemaWorkerQuota,
        ) -> Result<(), ExecutorFailure> {
            if self.shared_schema_worker_quota.is_some()
                || self.used_frames != 0
                || self.session.is_some()
                || self.poisoned.is_some()
            {
                return Err(ExecutorFailure::ResourceLimitUnknown);
            }
            quota.ensure_attachable()?;
            self.shared_schema_worker_quota = Some(quota);
            Ok(())
        }
        pub(crate) fn poison_shared_schema_worker_quota(&self) {
            if let Some(quota) = &self.shared_schema_worker_quota {
                quota.poison();
            }
        }
        pub(crate) fn exchange_failure(&self) -> Option<ExchangeFailureContext> {
            self.poison_exchange
        }
        pub(crate) fn poison(&mut self, mut reason: ExecutorFailure) -> ExecutorFailure {
            if let Some(mut session) = self.session.take() {
                if let Err(cleanup) = session.child.cleanup() {
                    reason = cleanup;
                }
            }
            self.poison_shared_schema_worker_quota();
            self.poisoned = Some(reason);
            reason
        }
        fn finish_session_inner(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            let deadline = deadline.min(self.operation_deadline).min(
                self.operation_started
                    .and_then(|origin| {
                        origin.checked_add(self.operation_budget.total_execution_wall)
                    })
                    .ok_or(ExecutorFailure::ResourceLimitUnknown)?,
            );
            preparation_check(Some(deadline), Some(cancelled))?;
            let Some(mut session) = self.session.take() else {
                return Ok(());
            };
            let result = (|| {
                let header_sha = Digest256::of_bytes(&session.header);
                let mut close = Vec::new();
                close.extend_from_slice(&session.sequence.to_be_bytes());
                close.extend_from_slice(OPERATION_CLOSE_MAGIC);
                close.extend_from_slice(header_sha.as_bytes());
                let mut hash = Digest256Hasher::new();
                hash.update(&session.header);
                hash.update(&close);
                let close_sha = hash.finalize();
                let mut request = Vec::new();
                request.extend_from_slice(&(close.len() as u32).to_be_bytes());
                request.extend_from_slice(&close);
                self.used_wire = self
                    .used_wire
                    .checked_add((request.len() + OPERATION_FINAL_BYTES) as u64)
                    .filter(|n| *n <= self.operation_budget.max_total_wire_bytes)
                    .ok_or(ExecutorFailure::InputBudget)?;
                let mut written = 0usize;
                let mut response = Vec::new();
                let mut eof = false;
                loop {
                    if let Err(reason) = preparation_check(Some(deadline), Some(cancelled)) {
                        return Err(self.poison(reason));
                    }
                    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
                    if session.child.status.is_none() {
                        let mut status = 0;
                        let result = unsafe {
                            libc::wait4(session.child.pid, &mut status, libc::WNOHANG, &mut usage)
                        };
                        if result == session.child.pid {
                            session.child.status = Some(status);
                            let micros = |v: libc::timeval| -> Option<u64> {
                                u64::try_from(v.tv_sec)
                                    .ok()?
                                    .checked_mul(1_000_000)?
                                    .checked_add(u64::try_from(v.tv_usec).ok()?)
                            };
                            self.used_cpu_micros = self
                                .used_cpu_micros
                                .checked_add(
                                    micros(usage.ru_utime)
                                        .and_then(|n| n.checked_add(micros(usage.ru_stime)?))
                                        .ok_or(ExecutorFailure::ResourceLimitUnknown)?,
                                )
                                .ok_or(ExecutorFailure::ResourceLimitUnknown)?;
                            if self.used_cpu_micros
                                > self
                                    .operation_budget
                                    .operation_cpu_seconds
                                    .checked_mul(1_000_000)
                                    .ok_or(ExecutorFailure::ResourceLimitUnknown)?
                            {
                                return Err(self.poison(ExecutorFailure::CpuLimit));
                            }
                        } else if result < 0
                            && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
                        {
                            return Err(self.poison(ExecutorFailure::ResourceLimitUnknown));
                        }
                    }
                    if written < request.len() {
                        let n = unsafe {
                            libc::send(
                                session.child.input.as_raw_fd(),
                                request[written..].as_ptr().cast(),
                                request.len() - written,
                                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                            )
                        };
                        if n > 0 {
                            written += n as usize;
                        } else if n == 0
                            || io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock
                        {
                            return Err(self.poison(ExecutorFailure::Protocol));
                        }
                    }
                    let mut extra = [0u8; OPERATION_FINAL_BYTES + 1];
                    let count = unsafe {
                        libc::recv(
                            session.child.output.as_raw_fd(),
                            extra.as_mut_ptr().cast(),
                            extra.len(),
                            libc::MSG_DONTWAIT,
                        )
                    };
                    if count > 0 {
                        response.extend_from_slice(&extra[..count as usize]);
                        if response.len() > OPERATION_FINAL_BYTES {
                            return Err(self.poison(ExecutorFailure::Protocol));
                        }
                    }
                    if count == 0 {
                        eof = true;
                    }
                    if count < 0 && io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                        return Err(self.poison(ExecutorFailure::Protocol));
                    }
                    if eof && session.child.status.is_some() {
                        if let Some(reason) = session.child.status.and_then(status_failure) {
                            return Err(self.poison(reason));
                        }
                        if written != request.len()
                            || response.len() != OPERATION_FINAL_BYTES
                            || &response[..8] != OPERATION_FINAL_MAGIC
                            || &response[8..40] != header_sha.as_bytes()
                            || &response[40..72] != close_sha.as_bytes()
                            || u64::from_be_bytes(response[72..80].try_into().unwrap())
                                != session.sequence
                        {
                            return Err(self.poison(ExecutorFailure::Protocol));
                        }
                        return Ok(());
                    }
                    let mut fd = libc::pollfd {
                        fd: session.child.output.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    unsafe {
                        libc::poll(&mut fd, 1, 2);
                    }
                }
            })();
            if let Err(reason) = result {
                let reason = session.child.cleanup().err().unwrap_or(reason);
                return Err(self.poison(reason));
            }
            result
        }
        fn finish_session(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            match self.finish_session_inner(deadline, cancelled) {
                Ok(()) => Ok(()),
                Err(reason) => Err(self.poison(reason)),
            }
        }
        pub(crate) fn finish(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            if let Some(reason) = self.poisoned {
                return Err(reason);
            }
            let result = self.finish_session(deadline, cancelled);
            // Finalized operations cannot accept another request or extend their envelope.
            if result.is_ok() {
                self.poisoned = Some(ExecutorFailure::CoverageMismatch);
            }
            result
        }
        fn exchange(
            &mut self,
            encoded: &[u8],
            schema_set: Digest256,
            profile: FormatProfile,
            units: &[BatchUnit],
            expected: BatchCoverageExpectation,
            mut budget: BatchBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> BatchOutcome {
            let start = Instant::now();
            budget.max_units = budget.max_units.min(self.operation_budget.batch.max_units);
            budget.max_total_raw_bytes = budget
                .max_total_raw_bytes
                .min(self.operation_budget.batch.max_total_raw_bytes);
            budget.total_execution_wall = budget
                .total_execution_wall
                .min(self.operation_budget.batch.total_execution_wall);
            budget.startup_wall = budget
                .startup_wall
                .min(self.operation_budget.batch.startup_wall)
                .min(budget.total_execution_wall);
            budget.per_unit_wall = budget
                .per_unit_wall
                .min(self.operation_budget.batch.per_unit_wall)
                .min(budget.total_execution_wall);
            budget.cleanup_grace = budget
                .cleanup_grace
                .min(self.operation_budget.batch.cleanup_grace);
            let empty_resources = 0u32.to_be_bytes();
            let resources = if self
                .session
                .as_ref()
                .is_some_and(|s| s.schema_set == schema_set)
            {
                &empty_resources[..]
            } else {
                encoded
            };
            let prepared = match make_batch_request_encoded(
                self.identity.sha256,
                resources,
                schema_set,
                profile,
                units,
                budget,
            ) {
                Ok(x) => x,
                Err(reason) => {
                    return empty_batch_outcome(self.identity.sha256, profile, schema_set, reason);
                }
            };
            self.exchange_prepared(
                prepared, units, expected, budget, deadline, cancelled, start,
            )
        }

        fn exchange_prepared(
            &mut self,
            mut prepared: BatchPrepared,
            units: &[BatchUnit],
            expected: BatchCoverageExpectation,
            mut budget: BatchBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
            start: Instant,
        ) -> BatchOutcome {
            let schema_set = prepared.schema_set_sha256;
            let profile = prepared.profile;
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let deadline = deadline.min(self.operation_deadline).min(
                self.operation_started
                    .get_or_insert(start)
                    .checked_add(self.operation_budget.total_execution_wall)
                    .unwrap_or(start),
            );
            let refusal = |p, r, why| batch_incomplete(p, Vec::new(), r, why);
            if self.shared_schema_worker_quota.is_some() {
                return refusal(prepared, results, self.poison(ExecutorFailure::Protocol));
            }
            if let Some(reason) = self.poisoned {
                let mut outcome = refusal(prepared, results, reason);
                if let BatchOutcome::Incomplete { exchange, .. } = &mut outcome {
                    *exchange = self.poison_exchange;
                }
                return outcome;
            }
            if let Err(reason) = preparation_check(Some(deadline), Some(cancelled)) {
                let why = self.poison(reason);
                return refusal(prepared, results, why);
            }
            if prepared.worker_sha256 != self.identity.sha256 {
                return refusal(
                    prepared,
                    results,
                    self.poison(ExecutorFailure::WorkerIdentity),
                );
            }
            if expected.count != prepared.units.len() as u64
                || expected.ordered_manifest_sha256 != prepared.ordered_manifest_sha256
            {
                return refusal(
                    prepared,
                    results,
                    self.poison(ExecutorFailure::CoverageMismatch),
                );
            }
            if self.selected_profile.is_some_and(|p| p != profile) {
                return refusal(prepared, results, self.poison(ExecutorFailure::Protocol));
            }
            self.selected_profile = Some(profile);
            let raw = units
                .iter()
                .try_fold(0u64, |sum, u| sum.checked_add(u.raw_instance.len() as u64));
            let Some(next_frames) = self
                .used_frames
                .checked_add(1)
                .filter(|n| *n <= self.operation_budget.max_chunks)
            else {
                return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
            };
            let Some(next_units) = self
                .used_units
                .checked_add(units.len() as u64)
                .filter(|n| *n <= self.operation_budget.max_total_units)
            else {
                return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
            };
            let Some(next_raw) = raw
                .and_then(|n| self.used_raw.checked_add(n))
                .filter(|n| *n <= self.operation_budget.max_total_raw_bytes)
            else {
                return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
            };
            for unit in units {
                self.selectors
                    .insert((schema_set.to_hex(), unit.root_uri.clone()));
                if self.selectors.len() > self.operation_budget.max_distinct_selectors {
                    return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
                }
            }
            if self
                .session
                .as_ref()
                .is_some_and(|s| s.schema_set != schema_set)
            {
                if let Err(reason) = self.finish_session(deadline, cancelled) {
                    return refusal(prepared, results, reason);
                }
            }
            let first = self.session.is_none();
            let amount = prepared.frame.len() as u64
                + 8
                + 4
                + if first {
                    OPERATION_HEADER_BYTES as u64
                } else {
                    0
                }
                + (BATCH_ACK_BYTES + prepared.units.len() * BATCH_UNIT_BYTES + OPERATION_END_BYTES)
                    as u64;
            let Some(next_wire) = self.used_wire.checked_add(amount).filter(|n| {
                n.checked_add((4 + 48 + OPERATION_FINAL_BYTES) as u64)
                    .is_some_and(|reserved| reserved <= self.operation_budget.max_total_wire_bytes)
            }) else {
                return refusal(prepared, results, self.poison(ExecutorFailure::InputBudget));
            };
            if first {
                let mut nonce = [0u8; 16];
                if File::open("/dev/urandom")
                    .and_then(|mut f| f.read_exact(&mut nonce))
                    .is_err()
                {
                    return refusal(prepared, results, self.poison(ExecutorFailure::Spawn));
                }
                let remaining_cpu = self
                    .operation_budget
                    .operation_cpu_seconds
                    .saturating_sub(self.used_cpu_micros.div_ceil(1_000_000));
                if remaining_cpu == 0 {
                    return refusal(prepared, results, self.poison(ExecutorFailure::CpuLimit));
                }
                let header = operation_header(
                    nonce,
                    schema_set,
                    profile,
                    [
                        self.operation_budget.max_chunks - self.used_frames,
                        self.operation_budget.max_total_units - self.used_units,
                        self.operation_budget.max_total_raw_bytes - self.used_raw,
                        self.operation_budget
                            .max_total_wire_bytes
                            .saturating_sub(self.used_wire),
                        self.operation_budget.max_distinct_selectors as u64,
                        remaining_cpu,
                        self.operation_budget.operation_address_space_bytes,
                        deadline
                            .saturating_duration_since(start)
                            .as_nanos()
                            .min(u64::MAX as u128) as u64,
                    ],
                );
                let argv = [
                    c"tos-schema-worker".as_ptr() as *mut libc::c_char,
                    std::ptr::null_mut(),
                ];
                let child = match spawn_operation_child(
                    &self.file,
                    ExecutorBudget {
                        execution_wall: deadline.saturating_duration_since(start),
                        cleanup_grace: budget.cleanup_grace,
                        cpu_seconds: remaining_cpu,
                        address_space_bytes: self.operation_budget.operation_address_space_bytes,
                    },
                    &argv,
                ) {
                    Ok(x) => x,
                    Err(reason) => return refusal(prepared, results, self.poison(reason)),
                };
                self.session = Some(OwnedSchemaSession {
                    child,
                    header,
                    schema_set,
                    sequence: 0,
                });
            }
            let session = self.session.as_mut().unwrap();
            // Add the operation framing in place instead of keeping full
            // batch/body/wire copies of every instance at the same time.
            let batch_len = prepared.frame.len();
            let header_len = if first { session.header.len() } else { 0 };
            let prefix = header_len + 4 + 8;
            prepared.frame.reserve(prefix);
            prepared.frame.resize(batch_len + prefix, 0);
            prepared.frame.copy_within(0..batch_len, prefix);
            if first {
                prepared.frame[..header_len].copy_from_slice(&session.header);
            }
            prepared.frame[header_len..header_len + 4]
                .copy_from_slice(&((batch_len + 8) as u32).to_be_bytes());
            prepared.frame[header_len + 4..prefix].copy_from_slice(&session.sequence.to_be_bytes());
            let mut request = Digest256Hasher::new();
            request.update(&session.header);
            request.update(&prepared.frame[header_len + 4..]);
            prepared.request_sha256 = request.finalize();
            self.used_frames = next_frames;
            self.used_units = next_units;
            self.used_raw = next_raw;
            self.used_wire = next_wire;
            budget.total_execution_wall = budget
                .total_execution_wall
                .min(deadline.saturating_duration_since(start));
            budget.startup_wall = budget.startup_wall.min(budget.total_execution_wall);
            budget.per_unit_wall = budget.per_unit_wall.min(budget.total_execution_wall);
            let outcome = run_batch_exchange(
                &mut session.child,
                prepared,
                results,
                budget,
                start,
                Some(cancelled),
                true,
            );
            match &outcome {
                BatchOutcome::Complete { .. } => session.sequence += 1,
                BatchOutcome::Incomplete {
                    reason, exchange, ..
                } => {
                    // A later preflight/finish refuses this poisoned operation,
                    // but must not replace its first actual exchange origin.
                    if self.poison_exchange.is_none() {
                        self.poison_exchange = *exchange;
                    }
                    let why = *reason;
                    self.poison(why);
                }
            }
            outcome
        }

        pub(crate) fn preflight(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            if let Some(reason) = self.poisoned {
                return Err(reason);
            }
            let deadline = deadline.min(self.operation_deadline).min(
                self.operation_started
                    .and_then(|origin| {
                        origin.checked_add(self.operation_budget.total_execution_wall)
                    })
                    .ok_or(ExecutorFailure::ResourceLimitUnknown)?,
            );
            preparation_check(Some(deadline), Some(cancelled)).map_err(|reason| self.poison(reason))
        }

        /// Runs one opt-in diagnostics-v2 unit through this exact sealed image
        /// and this operation's aggregate accounting. Any retained OP1 child
        /// is finalized before the disposable diagnostics child is started.
        pub(crate) fn evaluate_with_diagnostics(
            &mut self,
            resources: &[SchemaResource],
            profile: FormatProfile,
            location: &str,
            root_uri: &str,
            raw_instance: &[u8],
            budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure>
        {
            let start = Instant::now();
            let (encoded_resources, schema_set_sha256) = encode_resources(resources)?;
            self.evaluate_with_diagnostics_encoded(
                &encoded_resources,
                encoded_resources.capacity(),
                schema_set_sha256,
                profile,
                DiagnosticsInputProfile::FiniteJson,
                None,
                None,
                "biblio-record-schema-unit",
                location,
                root_uri,
                raw_instance,
                budget,
                deadline,
                start,
                cancelled,
            )
        }

        fn evaluate_with_diagnostics_encoded(
            &mut self,
            encoded_resources: &[u8],
            schema_resource_buffer_bytes: usize,
            schema_set_sha256: Digest256,
            profile: FormatProfile,
            input_profile: DiagnosticsInputProfile,
            selected_limits: Option<LegacySelectedDiagnosticsLimits>,
            selected_resource_preparation_state_bytes: Option<usize>,
            member_id: &str,
            location: &str,
            root_uri: &str,
            raw_instance: &[u8],
            mut budget: ExecutorBudget,
            deadline: Instant,
            start: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure>
        {
            let operation_deadline = deadline.min(self.operation_deadline).min(
                self.operation_started
                    .get_or_insert(start)
                    .checked_add(self.operation_budget.total_execution_wall)
                    .ok_or(ExecutorFailure::ResourceLimitUnknown)?,
            );
            self.preflight(operation_deadline, cancelled)?;
            scalar_budget(budget)?;
            budget.execution_wall = budget
                .execution_wall
                .min(self.operation_budget.batch.total_execution_wall)
                .min(operation_deadline.saturating_duration_since(start));
            if budget.execution_wall.is_zero() {
                return Err(self.poison(ExecutorFailure::Timeout));
            }
            if self
                .selected_profile
                .is_some_and(|selected| selected != profile)
            {
                return Err(self.poison(ExecutorFailure::Protocol));
            }
            if let Err(reason) = self.finish_session(operation_deadline, cancelled) {
                return Err(reason);
            }
            self.preflight(operation_deadline, cancelled)?;

            let input_mode = match input_profile {
                DiagnosticsInputProfile::FiniteJson => DiagnosticsUnitInputMode::FiniteJson,
                DiagnosticsInputProfile::FiniteJsonSelected => {
                    DiagnosticsUnitInputMode::FiniteJsonSelected
                }
                DiagnosticsInputProfile::LegacyPythonObserved => {
                    DiagnosticsUnitInputMode::LegacyPythonObserved
                }
                DiagnosticsInputProfile::LegacyPythonObservedSelected => {
                    DiagnosticsUnitInputMode::LegacyPythonObservedSelected
                }
                DiagnosticsInputProfile::MixedSourceFoundation => {
                    return Err(self.poison(ExecutorFailure::InputBudget));
                }
            };
            let selected_legacy =
                input_profile == DiagnosticsInputProfile::LegacyPythonObservedSelected;
            if selected_legacy != selected_limits.is_some()
                || selected_legacy != selected_resource_preparation_state_bytes.is_some()
                || (selected_legacy && profile != FormatProfile::LegacyPythonObserved20260923)
            {
                return Err(self.poison(ExecutorFailure::InputBudget));
            }
            if let Some(limits) = selected_limits {
                limits.validate().map_err(|reason| self.poison(reason))?;
                if raw_instance.len() > limits.max_instance_bytes {
                    return Err(self.poison(ExecutorFailure::InputBudget));
                }
            }
            validate_diagnostics_batch_unit_fields(
                member_id,
                location,
                root_uri,
                raw_instance.len(),
                input_mode,
            )
            .map_err(|reason| self.poison(reason))?;

            // Admit the borrowed source slice against every raw/frame ceiling
            // before making the BatchUnit-owned copy. This also keeps callers
            // of PreparedSchemaWorker on the same bounded path as the cut
            // adapter's earlier controller-side admission.
            let batch_raw_limit = self
                .operation_budget
                .batch
                .max_total_raw_bytes
                .min(MAX_BATCH_RAW_BYTES)
                .min(match input_mode {
                    DiagnosticsUnitInputMode::FiniteJson => {
                        crate::SchemaBackendProbe::MAX_INSTANCE_BYTES
                    }
                    DiagnosticsUnitInputMode::LegacyPythonObserved
                    | DiagnosticsUnitInputMode::FiniteJsonSelected => MAX_BATCH_RAW_BYTES,
                    DiagnosticsUnitInputMode::LegacyPythonObservedSelected => {
                        selected_limits
                            .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?
                            .max_instance_bytes
                    }
                });
            if raw_instance.len() > batch_raw_limit {
                return Err(self.poison(ExecutorFailure::InputBudget));
            }
            let raw_instance_bytes = u64::try_from(raw_instance.len())
                .map_err(|_| self.poison(ExecutorFailure::InputBudget))?;
            let next_frames = self
                .used_frames
                .checked_add(1)
                .filter(|count| *count <= self.operation_budget.max_chunks)
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            let next_units = self
                .used_units
                .checked_add(1)
                .filter(|count| *count <= self.operation_budget.max_total_units)
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            let next_raw = self
                .used_raw
                .checked_add(raw_instance_bytes)
                .filter(|count| *count <= self.operation_budget.max_total_raw_bytes)
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            let schema_key = schema_set_sha256.to_hex();
            let is_new_selector = !self
                .selectors
                .iter()
                .any(|(schema, selector)| schema == &schema_key && selector.as_str() == root_uri);
            if is_new_selector
                && self.selectors.len() >= self.operation_budget.max_distinct_selectors
            {
                return Err(self.poison(ExecutorFailure::InputBudget));
            }
            let projected_request_bytes = diagnostics_scalar_request_frame_bytes(
                encoded_resources.len(),
                input_profile,
                member_id.len(),
                location.len(),
                root_uri.len(),
                raw_instance.len(),
            )
            .filter(|bytes| *bytes <= MAX_DIAGNOSTIC_REQUEST_BYTES)
            .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            if let (Some(limits), Some(resource_state_bytes)) =
                (selected_limits, selected_resource_preparation_state_bytes)
            {
                let response_buffer_bytes = DIAGNOSTIC_ACK_BYTES
                    .checked_add(
                        schema_diagnostics::Caps::CURRENT.max_report_bytes_per_unit as usize,
                    )
                    .and_then(|bytes| bytes.checked_add(DIAGNOSTIC_FINAL_BYTES))
                    .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
                let required = legacy_selected_child_address_space_required(
                    resource_state_bytes,
                    projected_request_bytes,
                    response_buffer_bytes,
                    member_id.len(),
                    location.len(),
                    root_uri.len(),
                    raw_instance.len(),
                    self.image_bytes().map_err(|reason| self.poison(reason))?,
                    limits,
                )
                .map_err(|reason| self.poison(reason))?;
                let effective_child_limit = budget
                    .address_space_bytes
                    .min(self.operation_budget.batch.address_space_bytes)
                    .min(self.operation_budget.operation_address_space_bytes);
                if required > effective_child_limit {
                    return Err(self.poison(ExecutorFailure::InputBudget));
                }
            }
            let minimum_response_bytes = DIAGNOSTIC_ACK_BYTES
                .checked_add(DIAGNOSTIC_UNIT_HEADER_BYTES)
                .and_then(|bytes| bytes.checked_add(DIAGNOSTIC_FINAL_BYTES))
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            let local_remaining_wire = self
                .operation_budget
                .max_total_wire_bytes
                .checked_sub(self.used_wire)
                .filter(|remaining| *remaining != u64::MAX)
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            let minimum_exchange_wire = u64::try_from(projected_request_bytes)
                .ok()
                .and_then(|request| {
                    u64::try_from(minimum_response_bytes)
                        .ok()
                        .and_then(|response| request.checked_add(response))
                })
                .and_then(|bytes| bytes.checked_add(1)) // bounded-reader sentinel
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            if minimum_exchange_wire > local_remaining_wire {
                return Err(self.poison(ExecutorFailure::InputBudget));
            }
            if let Some(quota) = &self.shared_schema_worker_quota {
                let usage = quota.usage().map_err(|reason| self.poison(reason))?;
                if usage
                    .max_total_units
                    .checked_sub(usage.worker_units)
                    .is_none_or(|remaining| remaining == 0)
                {
                    return Err(self.poison(ExecutorFailure::InputBudget));
                }
                if usage
                    .max_total_cpu_micros
                    .checked_sub(usage.worker_cpu_micros)
                    .is_none_or(|remaining| remaining == 0)
                {
                    return Err(self.poison(ExecutorFailure::CpuLimit));
                }
                if usage
                    .max_total_wire_bytes
                    .checked_sub(usage.worker_wire_bytes)
                    .is_none_or(|remaining| minimum_exchange_wire > remaining)
                {
                    return Err(self.poison(ExecutorFailure::InputBudget));
                }
            }

            let unit = BatchUnit {
                ordinal: 0,
                member_id: member_id.to_owned(),
                relative_path: location.to_owned(),
                root_uri: root_uri.to_owned(),
                raw_instance: raw_instance.to_vec(),
            };
            validate_diagnostics_batch_unit(&unit, input_mode)
                .map_err(|reason| self.poison(reason))?;
            let input_instance_buffer_bytes = unit.raw_instance.capacity();
            let mut batch = self.operation_budget.batch;
            batch.max_units = 1;
            batch.max_total_raw_bytes = batch_raw_limit;
            batch.total_execution_wall = budget.execution_wall;
            batch.startup_wall = batch.startup_wall.min(batch.total_execution_wall);
            batch.per_unit_wall = batch.per_unit_wall.min(batch.total_execution_wall);
            batch.cleanup_grace = budget.cleanup_grace.min(batch.cleanup_grace);
            batch.address_space_bytes = budget
                .address_space_bytes
                .min(self.operation_budget.operation_address_space_bytes);
            let local_cpu_limit_micros = self
                .operation_budget
                .operation_cpu_seconds
                .checked_mul(1_000_000)
                .ok_or_else(|| self.poison(ExecutorFailure::ResourceLimitUnknown))?;
            let remaining_cpu_micros = local_cpu_limit_micros
                .checked_sub(self.used_cpu_micros)
                .ok_or_else(|| self.poison(ExecutorFailure::CpuLimit))?;
            if remaining_cpu_micros == 0 {
                return Err(self.poison(ExecutorFailure::CpuLimit));
            }
            let remaining_cpu_seconds = remaining_cpu_micros.div_ceil(1_000_000).max(1);
            batch.cpu_seconds = budget.cpu_seconds.min(remaining_cpu_seconds);
            let shared_quota = self.shared_schema_worker_quota.clone();
            if let Some(quota) = &shared_quota {
                batch.cpu_seconds = quota
                    .child_cpu_seconds(batch.cpu_seconds)
                    .map_err(|reason| self.poison(reason))?;
            }
            if batch.cpu_seconds == 0 {
                return Err(self.poison(ExecutorFailure::CpuLimit));
            }
            let expected = if let Some(limits) = selected_limits {
                BatchCoverageExpectation::from_selected_legacy_diagnostics_units(
                    std::slice::from_ref(&unit),
                    limits,
                )?
            } else {
                BatchCoverageExpectation::from_diagnostics_units(
                    std::slice::from_ref(&unit),
                    input_profile,
                    None,
                )?
            };
            let prepared = make_diagnostics_request_encoded_with_options(
                self.identity.sha256,
                &encoded_resources,
                schema_set_sha256,
                profile,
                input_profile,
                std::slice::from_ref(&unit),
                None,
                None,
                selected_limits,
                batch,
                schema_diagnostics::Caps::CURRENT,
            )?;
            if prepared.units.len() != 1
                || prepared.units[0].ordinal != 0
                || prepared.units[0].member_id != member_id
                || prepared.units[0].relative_path != location
                || prepared.units[0].root_uri != root_uri
                || expected.count != 1
                || expected.ordered_manifest_sha256 != prepared.ordered_manifest_sha256
            {
                return Err(self.poison(ExecutorFailure::CoverageMismatch));
            }
            self.preflight(operation_deadline, cancelled)?;

            if prepared.frame.len() != projected_request_bytes {
                return Err(self.poison(ExecutorFailure::Protocol));
            }
            let selector = (schema_key, root_uri.to_owned());
            let response_limit = DIAGNOSTIC_ACK_BYTES
                .checked_add(schema_diagnostics::Caps::CURRENT.max_report_bytes_per_unit as usize)
                .and_then(|bytes| bytes.checked_add(DIAGNOSTIC_FINAL_BYTES))
                .filter(|bytes| *bytes <= schema_diagnostics::MAX_RESPONSE_BYTES)
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            let request_bytes = prepared.frame.len();
            let minimum_response = diagnostics_minimum_response_bytes(&prepared)
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            let local_remaining_wire = self
                .operation_budget
                .max_total_wire_bytes
                .checked_sub(self.used_wire)
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            let local_response_cap =
                diagnostics_response_cap_with_wire_budget(&prepared, local_remaining_wire)
                    .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            if local_response_cap > response_limit || local_response_cap < minimum_response {
                return Err(self.poison(ExecutorFailure::InputBudget));
            }
            let shared_reservation = if let Some(quota) = shared_quota {
                let reservation = quota
                    .begin(
                        request_bytes,
                        minimum_response,
                        local_response_cap,
                        batch.cpu_seconds,
                        1,
                    )
                    .map_err(|reason| self.poison(reason))?;
                if reservation.child_cpu_seconds() != batch.cpu_seconds {
                    drop(reservation);
                    return Err(self.poison(ExecutorFailure::ResourceLimitUnknown));
                }
                Some(reservation)
            } else {
                None
            };
            let response_cap = shared_reservation.as_ref().map_or(
                local_response_cap,
                SharedSchemaWorkerReservation::response_cap,
            );
            if response_cap < minimum_response || response_cap > local_response_cap {
                drop(shared_reservation);
                return Err(self.poison(ExecutorFailure::InputBudget));
            }
            let reserved_wire = request_bytes
                .checked_add(response_cap)
                .and_then(|bytes| u64::try_from(bytes).ok())
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
            let next_wire = self
                .used_wire
                .checked_add(reserved_wire)
                .filter(|count| *count <= self.operation_budget.max_total_wire_bytes)
                .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;

            self.selected_profile = Some(profile);
            self.selectors.insert(selector);
            self.used_frames = next_frames;
            self.used_units = next_units;
            self.used_raw = next_raw;
            self.used_wire = next_wire;

            let argv = [
                c"tos-schema-worker".as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let (outcome, cost) = run_diagnostics_image_with_cost(
                &self.file,
                prepared,
                batch,
                start,
                &argv,
                Some(cancelled),
                Some(response_cap),
            );
            let mut cost = cost;
            cost.schema_resource_bytes = encoded_resources.len();
            cost.schema_resource_buffer_bytes = schema_resource_buffer_bytes;
            cost.input_instance_buffer_bytes = input_instance_buffer_bytes;
            if let SchemaDiagnosticsOutcome::Complete { .. } = &outcome {
                let Some(cpu_micros) = cost.worker_cpu_micros else {
                    return Err(self.poison(ExecutorFailure::ResourceLimitUnknown));
                };
                if cost.request_bytes != request_bytes
                    || cost.response_bytes < minimum_response
                    || cost.response_bytes > response_cap
                {
                    return Err(self.poison(ExecutorFailure::Protocol));
                }
                self.used_cpu_micros = self
                    .used_cpu_micros
                    .checked_add(cpu_micros)
                    .ok_or_else(|| self.poison(ExecutorFailure::ResourceLimitUnknown))?;
                let operation_cpu_micros = self
                    .operation_budget
                    .operation_cpu_seconds
                    .checked_mul(1_000_000)
                    .ok_or_else(|| self.poison(ExecutorFailure::ResourceLimitUnknown))?;
                if self.used_cpu_micros > operation_cpu_micros {
                    return Err(self.poison(ExecutorFailure::CpuLimit));
                }
                let actual_response = u64::try_from(cost.response_bytes)
                    .map_err(|_| self.poison(ExecutorFailure::InputBudget))?;
                let reserved_response = u64::try_from(response_cap)
                    .map_err(|_| self.poison(ExecutorFailure::InputBudget))?;
                self.used_wire = self
                    .used_wire
                    .checked_sub(reserved_response)
                    .and_then(|used| used.checked_add(actual_response))
                    .filter(|used| *used <= self.operation_budget.max_total_wire_bytes)
                    .ok_or_else(|| self.poison(ExecutorFailure::InputBudget))?;
                if let Err(reason) = self.preflight(operation_deadline, cancelled) {
                    return Err(reason);
                }
            } else {
                let reason = match &outcome {
                    SchemaDiagnosticsOutcome::Incomplete { reason, .. } => *reason,
                    SchemaDiagnosticsOutcome::Complete { .. } => ExecutorFailure::Protocol,
                };
                self.used_cpu_micros = self
                    .operation_budget
                    .operation_cpu_seconds
                    .saturating_mul(1_000_000);
                self.poison(reason);
            }
            if matches!(&outcome, SchemaDiagnosticsOutcome::Complete { .. }) {
                if let Some(reservation) = shared_reservation {
                    reservation.complete(
                        cost.request_bytes,
                        cost.response_bytes,
                        cost.worker_cpu_micros,
                        1,
                    )?;
                }
            }
            Ok((outcome, cost))
        }

        pub(crate) fn matches(&self, worker: &ExactWorkerIdentity) -> bool {
            self.identity.sha256 == worker.sha256
                && self.identity.absolute_path == worker.absolute_path
        }

        pub(crate) fn evaluate(
            &mut self,
            resources: &[SchemaResource],
            profile: FormatProfile,
            root_uri: &str,
            raw_instance: &[u8],
            budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> ExecutorOutcome {
            let start = Instant::now();
            let deadline = deadline.min(self.operation_deadline);
            if let Err(reason) = scalar_budget(budget)
                .and_then(|()| preparation_check(Some(deadline), Some(cancelled)))
            {
                return unknown(reason, None);
            }
            let (encoded, digest) = match encode_resources(resources) {
                Ok(value) => value,
                Err(reason) => return unknown(reason, None),
            };
            // Encoding belongs to this invocation's original wall budget.
            let remaining = budget.execution_wall.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return unknown(ExecutorFailure::Timeout, None);
            }
            self.evaluate_encoded(
                &encoded,
                digest,
                profile,
                root_uri,
                raw_instance,
                ExecutorBudget {
                    execution_wall: remaining,
                    ..budget
                },
                deadline,
                cancelled,
            )
        }

        fn evaluate_encoded(
            &mut self,
            encoded_resources: &[u8],
            schema_set_sha256: Digest256,
            profile: FormatProfile,
            root_uri: &str,
            raw_instance: &[u8],
            mut budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> ExecutorOutcome {
            let start = Instant::now();
            let deadline = deadline.min(self.operation_deadline);
            if let Err(reason) = preparation_check(Some(deadline), Some(cancelled)) {
                return unknown(reason, None);
            }
            if let Err(reason) = scalar_budget(budget) {
                return unknown(reason, None);
            }
            budget.execution_wall = budget
                .execution_wall
                .min(deadline.saturating_duration_since(start));
            let units = [BatchUnit {
                ordinal: 0,
                member_id: "scalar-instance".into(),
                relative_path: "scalar-instance".into(),
                root_uri: root_uri.into(),
                raw_instance: raw_instance.to_vec(),
            }];
            let expected = match BatchCoverageExpectation::from_units(&units) {
                Ok(x) => x,
                Err(reason) => return unknown(reason, None),
            };
            let batch = BatchBudget {
                total_execution_wall: budget.execution_wall,
                startup_wall: budget.execution_wall,
                per_unit_wall: budget.execution_wall,
                cleanup_grace: budget.cleanup_grace,
                cpu_seconds: budget.cpu_seconds,
                address_space_bytes: budget.address_space_bytes,
                max_units: 1,
                max_total_raw_bytes: crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
            };
            match self.exchange(
                encoded_resources,
                schema_set_sha256,
                profile,
                &units,
                expected,
                batch,
                deadline,
                cancelled,
            ) {
                BatchOutcome::Complete {
                    receipts,
                    checkpoint,
                } => {
                    let identity = ExecutionIdentity {
                        worker_sha256: checkpoint.worker_sha256,
                        request_sha256: checkpoint.request_sha256,
                        schema_set_sha256,
                        instance_sha256: Digest256::of_bytes(raw_instance),
                        profile,
                    };
                    match receipts[0].verdict {
                        BatchUnitVerdict::SchemaValid => ExecutorOutcome::SchemaValid(identity),
                        BatchUnitVerdict::SchemaInvalid => ExecutorOutcome::SchemaInvalid(identity),
                        BatchUnitVerdict::InputRejected => ExecutorOutcome::InputRejected(identity),
                    }
                }
                BatchOutcome::Incomplete {
                    reason,
                    checkpoint,
                    exchange,
                    ..
                } => {
                    let identity = ExecutionIdentity {
                        worker_sha256: checkpoint.worker_sha256,
                        request_sha256: checkpoint.request_sha256,
                        schema_set_sha256,
                        instance_sha256: Digest256::of_bytes(raw_instance),
                        profile,
                    };
                    if reason == ExecutorFailure::ParseRejected {
                        ExecutorOutcome::InputRejected(identity)
                    } else {
                        ExecutorOutcome::Indeterminate {
                            reason,
                            identity: Some(identity),
                            exchange,
                        }
                    }
                }
            }
        }
    }

    impl PreparedSchemaWorker {
        pub(crate) fn prepare(
            worker: &ExactWorkerIdentity,
            resources: &[SchemaResource],
            profile: FormatProfile,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            Self::prepare_inner(
                worker,
                None,
                resources,
                profile,
                budget,
                operation_deadline,
                cancelled,
            )
        }

        pub(crate) fn prepare_with_image(
            handle: &VerifiedWorkerImageHandle,
            resources: &[SchemaResource],
            profile: FormatProfile,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            Self::prepare_inner(
                handle.identity(),
                Some(handle),
                resources,
                profile,
                budget,
                operation_deadline.min(handle.operation_deadline()),
                cancelled,
            )
        }

        fn prepare_inner(
            worker: &ExactWorkerIdentity,
            handle: Option<&VerifiedWorkerImageHandle>,
            resources: &[SchemaResource],
            profile: FormatProfile,
            budget: ExecutorBudget,
            operation_deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<Self, ExecutorFailure> {
            scalar_budget(budget)?;
            let deadline = Instant::now()
                .checked_add(budget.execution_wall)
                .ok_or(ExecutorFailure::ResourceLimitUnknown)?
                .min(operation_deadline);
            preparation_check(Some(deadline), Some(cancelled))?;
            let (encoded_resources, schema_set_sha256) = encode_resources(resources)?;
            preparation_check(Some(deadline), Some(cancelled))?;
            let remaining = ExecutorBudget {
                execution_wall: deadline.saturating_duration_since(Instant::now()),
                ..budget
            };
            let image = match handle {
                Some(handle) => VerifiedWorkerImage::from_handle(
                    handle,
                    remaining,
                    operation_deadline,
                    cancelled,
                ),
                None => {
                    VerifiedWorkerImage::prepare(worker, remaining, operation_deadline, cancelled)
                }
            }?;
            preparation_check(Some(deadline), Some(cancelled))?;
            Ok(Self {
                image,
                encoded_resources,
                schema_set_sha256,
                profile,
            })
        }

        pub(crate) fn set_operation_budget(
            &mut self,
            budget: BatchStreamBudget,
        ) -> Result<(), ExecutorFailure> {
            self.image.set_operation_budget(budget)
        }
        pub(crate) fn set_shared_schema_worker_quota(
            &mut self,
            quota: SharedSchemaWorkerQuota,
        ) -> Result<(), ExecutorFailure> {
            self.image.set_shared_schema_worker_quota(quota)
        }
        pub(crate) fn poison_shared_schema_worker_quota(&self) {
            self.image.poison_shared_schema_worker_quota();
        }
        pub(crate) fn operation_budget(&self) -> BatchStreamBudget {
            self.image.operation_budget
        }
        pub(crate) fn exchange_failure(&self) -> Option<ExchangeFailureContext> {
            self.image.exchange_failure()
        }
        pub(crate) fn preflight(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            self.image.preflight(deadline, cancelled)
        }

        pub(crate) fn poison(&mut self, reason: ExecutorFailure) -> ExecutorFailure {
            self.image.poison(reason)
        }

        pub(crate) fn release_child(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            if let Some(reason) = self.image.poisoned {
                return Err(reason);
            }
            self.image.finish_session(deadline, cancelled)
        }

        pub(crate) fn finish(
            &mut self,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(), ExecutorFailure> {
            self.image.finish(deadline, cancelled)
        }
        pub(crate) fn set_operation_origin(&mut self, origin: Instant) {
            self.image.operation_started = Some(origin);
        }
        pub(crate) fn wire_cost(
            &self,
            member_bytes: usize,
            path_bytes: usize,
            selector_bytes: usize,
            instance_bytes: usize,
        ) -> Result<(u64, u64, u64), ExecutorFailure> {
            let operation = (OPERATION_HEADER_BYTES + self.encoded_resources.len() - 4
                + 4
                + 48
                + OPERATION_FINAL_BYTES) as u64;
            let frame = (4
                + 8
                + BATCH_REQUEST_MAGIC.len()
                + 16
                + 1
                + 4
                + 4
                + BATCH_ACK_BYTES
                + OPERATION_END_BYTES) as u64;
            let unit = [
                8usize,
                16,
                member_bytes,
                path_bytes,
                selector_bytes,
                instance_bytes,
                BATCH_UNIT_BYTES,
            ]
            .into_iter()
            .try_fold(0usize, |sum, n| sum.checked_add(n))
            .ok_or(ExecutorFailure::InputBudget)?;
            Ok((operation, frame, unit as u64))
        }

        pub(crate) fn evaluate(
            &mut self,
            root_uri: &str,
            raw_instance: &[u8],
            budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> ExecutorOutcome {
            self.image.evaluate_encoded(
                &self.encoded_resources,
                self.schema_set_sha256,
                self.profile,
                root_uri,
                raw_instance,
                budget,
                deadline,
                cancelled,
            )
        }

        pub(crate) fn evaluate_with_diagnostics(
            &mut self,
            member_id: &str,
            location: &str,
            root_uri: &str,
            raw_instance: &[u8],
            budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure>
        {
            self.image.evaluate_with_diagnostics_encoded(
                &self.encoded_resources,
                self.encoded_resources.capacity(),
                self.schema_set_sha256,
                self.profile,
                DiagnosticsInputProfile::FiniteJson,
                None,
                None,
                member_id,
                location,
                root_uri,
                raw_instance,
                budget,
                deadline,
                Instant::now(),
                cancelled,
            )
        }

        pub(crate) fn evaluate_with_legacy_diagnostics(
            &mut self,
            member_id: &str,
            location: &str,
            root_uri: &str,
            raw_instance: &[u8],
            budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure>
        {
            if self.profile != FormatProfile::LegacyPythonObserved20260923 {
                return Err(ExecutorFailure::InputBudget);
            }
            self.image.evaluate_with_diagnostics_encoded(
                &self.encoded_resources,
                self.encoded_resources.capacity(),
                self.schema_set_sha256,
                self.profile,
                DiagnosticsInputProfile::LegacyPythonObserved,
                None,
                None,
                member_id,
                location,
                root_uri,
                raw_instance,
                budget,
                deadline,
                Instant::now(),
                cancelled,
            )
        }

        pub(crate) fn evaluate_with_selected_finite_diagnostics(
            &mut self,
            member_id: &str,
            location: &str,
            root_uri: &str,
            raw_instance: &[u8],
            budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure>
        {
            self.image.evaluate_with_diagnostics_encoded(
                &self.encoded_resources,
                self.encoded_resources.capacity(),
                self.schema_set_sha256,
                self.profile,
                DiagnosticsInputProfile::FiniteJsonSelected,
                None,
                None,
                member_id,
                location,
                root_uri,
                raw_instance,
                budget,
                deadline,
                Instant::now(),
                cancelled,
            )
        }

        pub(crate) fn evaluate_with_selected_legacy_diagnostics(
            &mut self,
            member_id: &str,
            location: &str,
            root_uri: &str,
            raw_instance: &[u8],
            selected_limits: LegacySelectedDiagnosticsLimits,
            resource_preparation_state_bytes: usize,
            budget: ExecutorBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> Result<(SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost), ExecutorFailure>
        {
            if self.profile != FormatProfile::LegacyPythonObserved20260923 {
                return Err(ExecutorFailure::InputBudget);
            }
            self.image.evaluate_with_diagnostics_encoded(
                &self.encoded_resources,
                self.encoded_resources.capacity(),
                self.schema_set_sha256,
                self.profile,
                DiagnosticsInputProfile::LegacyPythonObservedSelected,
                Some(selected_limits),
                Some(resource_preparation_state_bytes),
                member_id,
                location,
                root_uri,
                raw_instance,
                budget,
                deadline,
                Instant::now(),
                cancelled,
            )
        }

        /// Allocation-free upper bound for one diagnostics-v2 request frame
        /// using this exact retained schema closure. It includes the two
        /// extended-profile bytes, so it also bounds the Legacy raw lane. This
        /// does not start a worker or allocate request buffers.
        pub(crate) fn diagnostics_v2_request_frame_bytes_upper_bound(
            &self,
            member_id_bytes: usize,
            location_bytes: usize,
            root_uri_bytes: usize,
            instance_bytes: usize,
        ) -> Result<usize, ExecutorFailure> {
            if member_id_bytes == 0
                || member_id_bytes > MAX_MEMBER_ID_BYTES
                || location_bytes == 0
                || location_bytes > MAX_PATH_BYTES
                || root_uri_bytes > MAX_URI_BYTES
                || instance_bytes > MAX_BATCH_RAW_BYTES
            {
                return Err(ExecutorFailure::InputBudget);
            }
            let strings = [
                member_id_bytes,
                location_bytes,
                root_uri_bytes,
                instance_bytes,
            ];
            let base = DIAGNOSTIC_FIXED_REQUEST_BYTES
                .checked_add(self.encoded_resources.len())
                .and_then(|bytes| bytes.checked_add(4)) // one-unit count
                .ok_or(ExecutorFailure::InputBudget)?;
            strings
                .iter()
                .try_fold(base, |total, len| total.checked_add(4)?.checked_add(*len))
                .and_then(|bytes| bytes.checked_add(8)) // ordinal
                .and_then(|bytes| bytes.checked_add(2)) // extended profile marker + input mode
                .filter(|bytes| *bytes <= MAX_DIAGNOSTIC_REQUEST_BYTES)
                .ok_or(ExecutorFailure::InputBudget)
        }

        /// Encoded frame-length bound for the selected Legacy sibling,
        /// including its four request-bound ceilings after the extended-profile
        /// marker. Whole-operation selected Legacy admission separately counts
        /// a conservative three-frame growth envelope.
        pub(crate) fn diagnostics_v2_selected_legacy_request_frame_bytes_upper_bound(
            &self,
            member_id_bytes: usize,
            location_bytes: usize,
            root_uri_bytes: usize,
            instance_bytes: usize,
        ) -> Result<usize, ExecutorFailure> {
            self.diagnostics_v2_request_frame_bytes_upper_bound(
                member_id_bytes,
                location_bytes,
                root_uri_bytes,
                instance_bytes,
            )?
            .checked_add(8 + 4 + 8 + 8)
            .filter(|bytes| *bytes <= MAX_DIAGNOSTIC_REQUEST_BYTES)
            .ok_or(ExecutorFailure::InputBudget)
        }

        pub(crate) fn worker_image_bytes(&self) -> Result<u64, ExecutorFailure> {
            self.image.image_bytes()
        }

        pub(crate) fn diagnostics_v2_selected_legacy_response_buffer_bytes_upper_bound(
            &self,
        ) -> Result<usize, ExecutorFailure> {
            DIAGNOSTIC_ACK_BYTES
                .checked_add(schema_diagnostics::Caps::CURRENT.max_report_bytes_per_unit as usize)
                .and_then(|bytes| bytes.checked_add(DIAGNOSTIC_FINAL_BYTES))
                .filter(|bytes| *bytes <= schema_diagnostics::MAX_RESPONSE_BYTES)
                .ok_or(ExecutorFailure::InputBudget)
        }

        /// Exact raw membership check against the immutable encoded closure.
        /// The walk is allocation-free and bounded by the selected resource
        /// count and per-resource byte ceilings.
        pub(crate) fn has_encoded_schema_resource(&self, uri: &str, raw: &[u8]) -> bool {
            let bytes = &self.encoded_resources;
            let Some(count_raw) = bytes.get(..4) else {
                return false;
            };
            let count = u32::from_be_bytes(count_raw.try_into().unwrap()) as usize;
            if count == 0 || count > crate::SchemaBackendProbe::MAX_RESOURCES {
                return false;
            }
            let mut offset = 4usize;
            let mut found = false;
            for _ in 0..count {
                let Some(resource_uri) =
                    take_encoded_resource_field(bytes, &mut offset, MAX_URI_BYTES)
                else {
                    return false;
                };
                let Some(resource_raw) = take_encoded_resource_field(
                    bytes,
                    &mut offset,
                    crate::SchemaBackendProbe::MAX_RESOURCE_BYTES,
                ) else {
                    return false;
                };
                found |= resource_uri == uri.as_bytes() && resource_raw == raw;
            }
            offset == bytes.len() && found
        }

        /// Maximum selected root-URI byte length from the retained closure.
        /// No strings are copied while deriving this preflight input bound.
        pub(crate) fn max_encoded_schema_uri_bytes(&self) -> Result<usize, ExecutorFailure> {
            let bytes = &self.encoded_resources;
            let count_raw = bytes.get(..4).ok_or(ExecutorFailure::Protocol)?;
            let count = u32::from_be_bytes(count_raw.try_into().unwrap()) as usize;
            if count == 0 || count > crate::SchemaBackendProbe::MAX_RESOURCES {
                return Err(ExecutorFailure::Protocol);
            }
            let mut offset = 4usize;
            let mut maximum = 0usize;
            for _ in 0..count {
                let uri = take_encoded_resource_field(bytes, &mut offset, MAX_URI_BYTES)
                    .ok_or(ExecutorFailure::Protocol)?;
                let _raw = take_encoded_resource_field(
                    bytes,
                    &mut offset,
                    crate::SchemaBackendProbe::MAX_RESOURCE_BYTES,
                )
                .ok_or(ExecutorFailure::Protocol)?;
                maximum = maximum.max(uri.len());
            }
            if offset != bytes.len() || maximum == 0 {
                return Err(ExecutorFailure::Protocol);
            }
            Ok(maximum)
        }

        /// Capacity of the exact encoded schema closure already retained by
        /// this prepared worker. Controller admission counts it separately
        /// from the additional request-frame copy used during an exchange.
        pub(crate) fn encoded_schema_resource_buffer_bytes(&self) -> usize {
            self.encoded_resources.capacity()
        }

        /// Upper bound for the frame and bounded response buffers that may
        /// coexist in one finite diagnostics-v2 exchange. A one-byte sentinel
        /// is reserved exactly as in the controller's bounded reader.
        pub(crate) fn diagnostics_v2_request_response_bytes_upper_bound(
            &self,
            request_bytes: usize,
        ) -> Result<(usize, usize), ExecutorFailure> {
            let full_response = DIAGNOSTIC_ACK_BYTES
                .checked_add(schema_diagnostics::Caps::CURRENT.max_report_bytes_per_unit as usize)
                .and_then(|bytes| bytes.checked_add(DIAGNOSTIC_FINAL_BYTES))
                .filter(|bytes| *bytes <= schema_diagnostics::MAX_RESPONSE_BYTES)
                .ok_or(ExecutorFailure::InputBudget)?;
            let minimum_response = DIAGNOSTIC_ACK_BYTES
                .checked_add(DIAGNOSTIC_UNIT_HEADER_BYTES)
                .and_then(|bytes| bytes.checked_add(DIAGNOSTIC_FINAL_BYTES))
                .ok_or(ExecutorFailure::InputBudget)?;
            let response = if self.image.operation_budget.max_total_wire_bytes == u64::MAX {
                full_response
            } else {
                let remaining = self
                    .image
                    .operation_budget
                    .max_total_wire_bytes
                    .checked_sub(self.image.used_wire)
                    .ok_or(ExecutorFailure::InputBudget)?;
                let response_ceiling = remaining
                    .checked_sub(
                        u64::try_from(request_bytes).map_err(|_| ExecutorFailure::InputBudget)?,
                    )
                    .and_then(|bytes| bytes.checked_sub(1))
                    .and_then(|bytes| usize::try_from(bytes).ok())
                    .ok_or(ExecutorFailure::InputBudget)?;
                full_response.min(response_ceiling)
            };
            if response < minimum_response {
                return Err(ExecutorFailure::InputBudget);
            }
            Ok((request_bytes, response))
        }

        pub(crate) fn evaluate_batch(
            &mut self,
            units: &[BatchUnit],
            expected: BatchCoverageExpectation,
            mut budget: BatchBudget,
            deadline: Instant,
            cancelled: &AtomicBool,
        ) -> BatchOutcome {
            self.image.exchange(
                &self.encoded_resources,
                self.schema_set_sha256,
                self.profile,
                units,
                expected,
                budget,
                deadline,
                cancelled,
            )
        }
    }

    #[cfg(test)]
    fn make_batch_request(
        worker_sha256: Digest256,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        budget: BatchBudget,
    ) -> Result<BatchPrepared, ExecutorFailure> {
        let (encoded, schema_set_sha256) = encode_resources(resources)?;
        make_batch_request_encoded(
            worker_sha256,
            &encoded,
            schema_set_sha256,
            profile,
            units,
            budget,
        )
    }

    fn make_batch_request_encoded<U: std::borrow::Borrow<BatchUnit>>(
        worker_sha256: Digest256,
        encoded_resources: &[u8],
        schema_set_sha256: Digest256,
        profile: FormatProfile,
        units: impl IntoIterator<Item = U>,
        budget: BatchBudget,
    ) -> Result<BatchPrepared, ExecutorFailure> {
        budget.validate()?;
        let mut frame = Vec::new();
        frame.extend_from_slice(BATCH_REQUEST_MAGIC);
        let mut nonce = [0u8; 16];
        File::open("/dev/urandom")
            .and_then(|mut random| random.read_exact(&mut nonce))
            .map_err(|_| ExecutorFailure::Spawn)?;
        frame.extend_from_slice(&nonce);
        frame.push(profile_byte(profile));
        frame.extend_from_slice(encoded_resources);
        let count_offset = frame.len();
        frame.extend_from_slice(&0u32.to_be_bytes());
        let mut metas = Vec::new();
        let mut raw_total = 0usize;
        let mut manifest = Digest256Hasher::new();
        manifest.update(b"tos-val2-batch-manifest-v1\0");
        for unit in units {
            let unit = unit.borrow();
            if metas.len() >= budget.max_units || unit.ordinal != metas.len() as u64 {
                return Err(ExecutorFailure::InputBudget);
            }
            validate_batch_unit(unit)?;
            raw_total = raw_total
                .checked_add(unit.raw_instance.len())
                .ok_or(ExecutorFailure::InputBudget)?;
            if raw_total > budget.max_total_raw_bytes {
                return Err(ExecutorFailure::InputBudget);
            }
            let start = frame.len();
            frame.extend_from_slice(&unit.ordinal.to_be_bytes());
            put_batch_bytes(&mut frame, unit.member_id.as_bytes())?;
            put_batch_bytes(&mut frame, unit.relative_path.as_bytes())?;
            put_batch_bytes(&mut frame, unit.root_uri.as_bytes())?;
            put_batch_bytes(&mut frame, &unit.raw_instance)?;
            let mut hasher = Digest256Hasher::new();
            hasher.update(b"tos-val2-batch-unit-v1\0");
            hasher.update(&frame[start..]);
            let unit_sha256 = hasher.finalize();
            manifest.update(unit_sha256.as_bytes());
            metas.push(BatchUnitMeta {
                ordinal: unit.ordinal,
                member_id: unit.member_id.clone(),
                relative_path: unit.relative_path.clone(),
                root_uri: unit.root_uri.clone(),
                raw_sha256: Digest256::of_bytes(&unit.raw_instance),
                unit_sha256,
            });
        }
        if metas.is_empty() {
            return Err(ExecutorFailure::InputBudget);
        }
        frame[count_offset..count_offset + 4].copy_from_slice(&(metas.len() as u32).to_be_bytes());
        let request_sha256 = Digest256::of_bytes(&frame);
        Ok(BatchPrepared {
            frame,
            units: metas,
            worker_sha256,
            profile,
            schema_set_sha256,
            request_sha256,
            ordered_manifest_sha256: manifest.finalize(),
        })
    }

    fn put_diagnostic_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), ExecutorFailure> {
        let size = u32::try_from(value.len()).map_err(|_| ExecutorFailure::InputBudget)?;
        if output
            .len()
            .checked_add(4)
            .and_then(|len| len.checked_add(value.len()))
            .filter(|len| *len <= MAX_DIAGNOSTIC_REQUEST_BYTES)
            .is_none()
        {
            return Err(ExecutorFailure::InputBudget);
        }
        output.extend_from_slice(&size.to_be_bytes());
        output.extend_from_slice(value);
        Ok(())
    }

    fn make_diagnostics_request_encoded<U: std::borrow::Borrow<BatchUnit>>(
        worker_sha256: Digest256,
        encoded_resources: &[u8],
        schema_set_sha256: Digest256,
        profile: FormatProfile,
        input_profile: DiagnosticsInputProfile,
        units: impl IntoIterator<Item = U>,
        budget: BatchBudget,
        caps: schema_diagnostics::Caps,
    ) -> Result<DiagnosticsPrepared, ExecutorFailure> {
        make_diagnostics_request_encoded_with_unit_modes(
            worker_sha256,
            encoded_resources,
            schema_set_sha256,
            profile,
            input_profile,
            units,
            None,
            None,
            budget,
            caps,
        )
    }

    fn make_diagnostics_request_encoded_with_unit_modes<U: std::borrow::Borrow<BatchUnit>>(
        worker_sha256: Digest256,
        encoded_resources: &[u8],
        schema_set_sha256: Digest256,
        profile: FormatProfile,
        input_profile: DiagnosticsInputProfile,
        units: impl IntoIterator<Item = U>,
        unit_modes: Option<&[DiagnosticsUnitInputMode]>,
        exceptional_remaining: Option<ExceptionalSchemaUsage>,
        budget: BatchBudget,
        caps: schema_diagnostics::Caps,
    ) -> Result<DiagnosticsPrepared, ExecutorFailure> {
        make_diagnostics_request_encoded_with_options(
            worker_sha256,
            encoded_resources,
            schema_set_sha256,
            profile,
            input_profile,
            units,
            unit_modes,
            exceptional_remaining,
            None,
            budget,
            caps,
        )
    }

    fn make_diagnostics_request_encoded_with_options<U: std::borrow::Borrow<BatchUnit>>(
        worker_sha256: Digest256,
        encoded_resources: &[u8],
        schema_set_sha256: Digest256,
        profile: FormatProfile,
        input_profile: DiagnosticsInputProfile,
        units: impl IntoIterator<Item = U>,
        unit_modes: Option<&[DiagnosticsUnitInputMode]>,
        exceptional_remaining: Option<ExceptionalSchemaUsage>,
        selected_limits: Option<LegacySelectedDiagnosticsLimits>,
        budget: BatchBudget,
        caps: schema_diagnostics::Caps,
    ) -> Result<DiagnosticsPrepared, ExecutorFailure> {
        budget.validate()?;
        if !caps.validate() {
            return Err(ExecutorFailure::InputBudget);
        }
        if matches!(
            input_profile,
            DiagnosticsInputProfile::MixedSourceFoundation
        ) != unit_modes.is_some()
            || matches!(
                input_profile,
                DiagnosticsInputProfile::MixedSourceFoundation
            ) != exceptional_remaining.is_some()
            || matches!(
                input_profile,
                DiagnosticsInputProfile::LegacyPythonObservedSelected
            ) != selected_limits.is_some()
            || exceptional_remaining
                .is_some_and(|remaining| !remaining.fits_within(ExceptionalSchemaUsage::whole()))
        {
            return Err(ExecutorFailure::InputBudget);
        }
        if let Some(limits) = selected_limits {
            limits.validate()?;
            if profile != FormatProfile::LegacyPythonObserved20260923
                || unit_modes.is_some()
                || exceptional_remaining.is_some()
            {
                return Err(ExecutorFailure::InputBudget);
            }
        }
        let mut frame = Vec::new();
        frame.extend_from_slice(DIAGNOSTIC_REQUEST_MAGIC);
        frame.extend_from_slice(&schema_diagnostics::PROTOCOL_VERSION.to_be_bytes());
        frame.extend_from_slice(&caps.max_issues_per_unit.to_be_bytes());
        frame.extend_from_slice(&caps.max_report_bytes_per_unit.to_be_bytes());
        frame.extend_from_slice(&caps.max_path_segments.to_be_bytes());
        frame.extend_from_slice(&caps.max_path_bytes.to_be_bytes());
        frame.extend_from_slice(worker_sha256.as_bytes());
        frame.extend_from_slice(schema_set_sha256.as_bytes());
        match input_profile {
            DiagnosticsInputProfile::FiniteJson => frame.push(profile_byte(profile)),
            DiagnosticsInputProfile::FiniteJsonSelected => {
                frame.push(DIAGNOSTIC_EXTENDED_INPUT_MARKER);
                frame.push(profile_byte(profile));
                frame.push(DIAGNOSTIC_INPUT_FINITE_JSON_SELECTED);
            }
            DiagnosticsInputProfile::LegacyPythonObserved => {
                frame.push(DIAGNOSTIC_EXTENDED_INPUT_MARKER);
                frame.push(profile_byte(profile));
                frame.push(DIAGNOSTIC_INPUT_LEGACY_PYTHON_OBSERVED);
            }
            DiagnosticsInputProfile::LegacyPythonObservedSelected => {
                frame.push(DIAGNOSTIC_EXTENDED_INPUT_MARKER);
                frame.push(profile_byte(profile));
                frame.push(DIAGNOSTIC_INPUT_LEGACY_PYTHON_OBSERVED_SELECTED);
                let limits = selected_limits.ok_or(ExecutorFailure::InputBudget)?;
                frame.extend_from_slice(
                    &u64::try_from(limits.max_instance_bytes)
                        .map_err(|_| ExecutorFailure::InputBudget)?
                        .to_be_bytes(),
                );
                frame.extend_from_slice(&limits.max_visits.to_be_bytes());
                frame.extend_from_slice(&limits.parser_state_bytes.to_be_bytes());
                frame.extend_from_slice(&limits.conversion_state_bytes.to_be_bytes());
            }
            DiagnosticsInputProfile::MixedSourceFoundation => {
                frame.push(DIAGNOSTIC_EXTENDED_INPUT_MARKER);
                frame.push(profile_byte(profile));
                frame.push(DIAGNOSTIC_INPUT_MIXED_SOURCE_FOUNDATION);
                frame.extend_from_slice(exceptional_schema::caps_sha256().as_bytes());
                exceptional_remaining
                    .ok_or(ExecutorFailure::InputBudget)?
                    .write_be(&mut frame);
            }
        }
        frame.extend_from_slice(encoded_resources);
        let count_offset = frame.len();
        frame.extend_from_slice(&0u32.to_be_bytes());
        let mut metas = Vec::new();
        let mut raw_total = 0usize;
        let mut manifest = Digest256Hasher::new();
        manifest.update(b"tos-val2-batch-manifest-v1\0");
        let mut ordinal = 0usize;
        for unit in units {
            let unit = unit.borrow();
            if metas.len() >= budget.max_units || unit.ordinal != metas.len() as u64 {
                return Err(ExecutorFailure::InputBudget);
            }
            let input_mode = match input_profile {
                DiagnosticsInputProfile::FiniteJson => DiagnosticsUnitInputMode::FiniteJson,
                DiagnosticsInputProfile::FiniteJsonSelected => {
                    DiagnosticsUnitInputMode::FiniteJsonSelected
                }
                DiagnosticsInputProfile::LegacyPythonObserved => {
                    DiagnosticsUnitInputMode::LegacyPythonObserved
                }
                DiagnosticsInputProfile::LegacyPythonObservedSelected => {
                    DiagnosticsUnitInputMode::LegacyPythonObservedSelected
                }
                DiagnosticsInputProfile::MixedSourceFoundation => *unit_modes
                    .and_then(|modes| modes.get(ordinal))
                    .ok_or(ExecutorFailure::InputBudget)?,
            };
            validate_diagnostics_batch_unit(unit, input_mode)?;
            if selected_limits
                .is_some_and(|limits| unit.raw_instance.len() > limits.max_instance_bytes)
            {
                return Err(ExecutorFailure::InputBudget);
            }
            raw_total = raw_total
                .checked_add(unit.raw_instance.len())
                .filter(|total| *total <= budget.max_total_raw_bytes)
                .ok_or(ExecutorFailure::InputBudget)?;
            frame.extend_from_slice(&unit.ordinal.to_be_bytes());
            let mode = matches!(
                input_profile,
                DiagnosticsInputProfile::MixedSourceFoundation
            )
            .then_some(input_mode);
            if let Some(mode) = mode {
                frame.push(mode.wire_byte());
            }
            put_diagnostic_bytes(&mut frame, unit.member_id.as_bytes())?;
            put_diagnostic_bytes(&mut frame, unit.relative_path.as_bytes())?;
            put_diagnostic_bytes(&mut frame, unit.root_uri.as_bytes())?;
            put_diagnostic_bytes(&mut frame, &unit.raw_instance)?;
            let unit_sha256 = if let Some(limits) = selected_limits {
                selected_legacy_diagnostics_batch_unit_digest(unit, limits)?
            } else {
                diagnostics_batch_unit_digest(unit, input_mode)?
            };
            manifest.update(unit_sha256.as_bytes());
            metas.push(BatchUnitMeta {
                ordinal: unit.ordinal,
                member_id: unit.member_id.clone(),
                relative_path: unit.relative_path.clone(),
                root_uri: unit.root_uri.clone(),
                raw_sha256: Digest256::of_bytes(&unit.raw_instance),
                unit_sha256,
            });
            ordinal += 1;
        }
        if unit_modes.is_some_and(|modes| modes.len() != metas.len()) {
            return Err(ExecutorFailure::InputBudget);
        }
        if metas.is_empty() {
            return Err(ExecutorFailure::InputBudget);
        }
        frame[count_offset..count_offset + 4].copy_from_slice(&(metas.len() as u32).to_be_bytes());
        let request_sha256 = Digest256::of_bytes(&frame);
        Ok(DiagnosticsPrepared {
            frame,
            units: metas,
            worker_sha256,
            profile,
            schema_set_sha256,
            request_sha256,
            ordered_manifest_sha256: manifest.finalize(),
            caps,
            exceptional_remaining,
        })
    }

    pub(super) fn evaluate_batch(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        cancelled: Option<&AtomicBool>,
    ) -> BatchOutcome {
        let local_cancelled = AtomicBool::new(false);
        let cancelled = cancelled.unwrap_or(&local_cancelled);
        let start = Instant::now();
        let deadline = start
            .checked_add(budget.total_execution_wall)
            .unwrap_or(start);
        let units: Vec<_> = units
            .into_iter()
            .take(MAX_BATCH_UNITS.saturating_add(1))
            .collect();
        // Keep request/manifest preflight before image lookup.
        let (encoded, set) = match encode_resources(resources) {
            Ok(x) => x,
            Err(reason) => {
                return empty_batch_outcome(
                    worker.sha256,
                    profile,
                    Digest256::of_bytes(b""),
                    reason,
                );
            }
        };
        let prepared =
            match make_batch_request_encoded(worker.sha256, &encoded, set, profile, &units, budget)
            {
                Ok(x) => x,
                Err(reason) => return empty_batch_outcome(worker.sha256, profile, set, reason),
            };
        let mut results = Digest256Hasher::new();
        results.update(b"tos-val2-batch-results-v1\0");
        if prepared.units.len() as u64 != expected.count
            || prepared.ordered_manifest_sha256 != expected.ordered_manifest_sha256
        {
            return batch_incomplete(
                prepared,
                Vec::new(),
                results,
                ExecutorFailure::CoverageMismatch,
            );
        }
        let scalar = ExecutorBudget {
            execution_wall: budget.total_execution_wall,
            cleanup_grace: budget.cleanup_grace,
            cpu_seconds: budget.cpu_seconds.min(60),
            address_space_bytes: budget.address_space_bytes,
        };
        let mut image = match VerifiedWorkerImage::prepare(worker, scalar, deadline, cancelled) {
            Ok(x) => x,
            Err(reason) => return batch_incomplete(prepared, Vec::new(), results, reason),
        };
        let mut work = BatchStreamBudget::laboratory();
        work.batch = budget;
        work.max_chunks = 1;
        work.max_total_units = budget.max_units as u64;
        work.max_total_raw_bytes = budget.max_total_raw_bytes as u64;
        work.max_total_wire_bytes = (MAX_BATCH_FRAME_BYTES
            + BATCH_ACK_BYTES
            + MAX_BATCH_UNITS * BATCH_UNIT_BYTES
            + OPERATION_END_BYTES
            + OPERATION_HEADER_BYTES
            + OPERATION_FINAL_BYTES
            + 128) as u64;
        work.max_distinct_selectors = budget.max_units;
        work.operation_cpu_seconds = budget.cpu_seconds;
        work.operation_address_space_bytes = budget.address_space_bytes;
        work.total_execution_wall = budget.total_execution_wall;
        if let Err(reason) = image.set_operation_budget(work) {
            return batch_incomplete(prepared, Vec::new(), results, reason);
        }
        // The same preflighted immutable frame now enters the owned path;
        // do not encode/hash/copy the whole request a second time.
        let outcome = image.exchange_prepared(
            prepared, &units, expected, budget, deadline, cancelled, start,
        );
        match outcome {
            BatchOutcome::Complete {
                receipts,
                checkpoint,
            } => match image.finish(deadline, cancelled) {
                Ok(()) => BatchOutcome::Complete {
                    receipts,
                    checkpoint,
                },
                Err(reason) => BatchOutcome::Incomplete {
                    receipts: Vec::new(),
                    checkpoint: BatchCoverageCheckpoint {
                        completed_count: 0,
                        ..checkpoint
                    },
                    reason,
                    exchange: None,
                },
            },
            other => other,
        }
    }

    pub(super) fn evaluate_batch_with_diagnostics(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        input_profile: DiagnosticsInputProfile,
        units: impl IntoIterator<Item = BatchUnit>,
        expected: BatchCoverageExpectation,
        budget: BatchBudget,
        remaining_worker_wire_bytes: Option<u64>,
        cancelled: Option<&AtomicBool>,
        shared_quota: Option<SharedSchemaWorkerQuota>,
    ) -> SchemaDiagnosticsOutcome {
        let units: Vec<_> = units
            .into_iter()
            .take(MAX_BATCH_UNITS.saturating_add(1))
            .collect();
        let poison_on_failure = shared_quota.clone();
        let outcome = evaluate_batch_with_diagnostics_units(
            worker,
            resources,
            profile,
            input_profile,
            units,
            None,
            None,
            remaining_worker_wire_bytes,
            expected,
            budget,
            cancelled,
            shared_quota,
            None,
        );
        if matches!(&outcome, SchemaDiagnosticsOutcome::Incomplete { .. }) {
            if let Some(quota) = poison_on_failure {
                quota.poison();
            }
        }
        outcome
    }

    pub(super) fn evaluate_batch_with_mixed_source_foundation_diagnostics(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        units: impl IntoIterator<Item = MixedDiagnosticsBatchUnit>,
        expected: BatchCoverageExpectation,
        exceptional_remaining: ExceptionalSchemaUsage,
        budget: BatchBudget,
        remaining_worker_wire_bytes: Option<u64>,
        cancelled: Option<&AtomicBool>,
        shared_quota: Option<SharedSchemaWorkerQuota>,
    ) -> SchemaDiagnosticsOutcome {
        let mixed: Vec<_> = units
            .into_iter()
            .take(MAX_BATCH_UNITS.saturating_add(1))
            .collect();
        let modes = mixed.iter().map(|unit| unit.input_mode).collect();
        let units = mixed.into_iter().map(|unit| unit.unit).collect();
        let poison_on_failure = shared_quota.clone();
        let outcome = evaluate_batch_with_diagnostics_units(
            worker,
            resources,
            profile,
            DiagnosticsInputProfile::MixedSourceFoundation,
            units,
            Some(modes),
            Some(exceptional_remaining),
            remaining_worker_wire_bytes,
            expected,
            budget,
            cancelled,
            shared_quota,
            None,
        );
        if matches!(&outcome, SchemaDiagnosticsOutcome::Incomplete { .. }) {
            if let Some(quota) = poison_on_failure {
                quota.poison();
            }
        }
        outcome
    }

    pub(super) fn evaluate_batch_with_diagnostics_units(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        input_profile: DiagnosticsInputProfile,
        units: Vec<BatchUnit>,
        unit_modes: Option<Vec<DiagnosticsUnitInputMode>>,
        exceptional_remaining: Option<ExceptionalSchemaUsage>,
        remaining_worker_wire_bytes: Option<u64>,
        expected: BatchCoverageExpectation,
        mut budget: BatchBudget,
        cancelled: Option<&AtomicBool>,
        shared_quota: Option<SharedSchemaWorkerQuota>,
        prepared_image: Option<&VerifiedWorkerImageHandle>,
    ) -> SchemaDiagnosticsOutcome {
        let local_cancelled = AtomicBool::new(false);
        let cancelled = cancelled.unwrap_or(&local_cancelled);
        let start = Instant::now();
        let deadline = start
            .checked_add(budget.total_execution_wall)
            .unwrap_or(start);
        if let Some(quota) = &shared_quota {
            budget.cpu_seconds = match quota.child_cpu_seconds(budget.cpu_seconds.min(60)) {
                Ok(seconds) => seconds,
                Err(reason) => {
                    quota.poison();
                    return empty_diagnostics_outcome(
                        worker.sha256,
                        profile,
                        Digest256::of_bytes(b""),
                        reason,
                    );
                }
            };
        }
        let (encoded, schema_set) = match encode_resources(resources) {
            Ok(value) => value,
            Err(reason) => {
                return empty_diagnostics_outcome(
                    worker.sha256,
                    profile,
                    Digest256::of_bytes(b""),
                    reason,
                );
            }
        };
        let prepared = match make_diagnostics_request_encoded_with_unit_modes(
            worker.sha256,
            &encoded,
            schema_set,
            profile,
            input_profile,
            &units,
            unit_modes.as_deref(),
            exceptional_remaining,
            budget,
            schema_diagnostics::Caps::CURRENT,
        ) {
            Ok(prepared) => prepared,
            Err(reason) => {
                return empty_diagnostics_outcome(worker.sha256, profile, schema_set, reason);
            }
        };
        if prepared.units.len() as u64 != expected.count
            || prepared.ordered_manifest_sha256 != expected.ordered_manifest_sha256
        {
            return diagnostics_incomplete(prepared, ExecutorFailure::CoverageMismatch, None);
        }
        let response_cap = if let Some(remaining_wire_bytes) = remaining_worker_wire_bytes {
            let Some(response_cap) =
                diagnostics_response_cap_with_wire_budget(&prepared, remaining_wire_bytes)
            else {
                return diagnostics_incomplete(prepared, ExecutorFailure::InputBudget, None);
            };
            Some(response_cap)
        } else {
            None
        };
        let scalar = ExecutorBudget {
            execution_wall: budget.total_execution_wall,
            cleanup_grace: budget.cleanup_grace,
            cpu_seconds: budget.cpu_seconds.min(60),
            address_space_bytes: budget.address_space_bytes,
        };
        let image_result = match prepared_image {
            Some(handle) => VerifiedWorkerImage::from_handle(handle, scalar, deadline, cancelled),
            None => VerifiedWorkerImage::prepare(worker, scalar, deadline, cancelled),
        };
        let mut image = match image_result {
            Ok(image) => image,
            Err(reason) => {
                if let Some(quota) = &shared_quota {
                    quota.poison();
                }
                return diagnostics_incomplete(prepared, reason, None);
            }
        };
        if let Some(quota) = &shared_quota {
            if let Err(reason) = image.set_shared_schema_worker_quota(quota.clone()) {
                quota.poison();
                return diagnostics_incomplete(prepared, reason, None);
            }
        }
        let shared_reservation = if let Some(quota) = &shared_quota {
            let maximum_response = match diagnostics_response_cap(&prepared) {
                Some(cap) => response_cap.map_or(cap, |local| local.min(cap)),
                None => {
                    quota.poison();
                    return diagnostics_incomplete(prepared, ExecutorFailure::InputBudget, None);
                }
            };
            let minimum_response = match diagnostics_minimum_response_bytes(&prepared) {
                Some(bytes) => bytes,
                None => {
                    quota.poison();
                    return diagnostics_incomplete(prepared, ExecutorFailure::InputBudget, None);
                }
            };
            match quota.begin(
                prepared.frame.len(),
                minimum_response,
                maximum_response,
                budget.cpu_seconds,
                expected.count,
            ) {
                Ok(reservation) if reservation.child_cpu_seconds() == budget.cpu_seconds => {
                    Some(reservation)
                }
                Ok(reservation) => {
                    drop(reservation);
                    quota.poison();
                    return diagnostics_incomplete(
                        prepared,
                        ExecutorFailure::ResourceLimitUnknown,
                        None,
                    );
                }
                Err(reason) => {
                    quota.poison();
                    return diagnostics_incomplete(prepared, reason, None);
                }
            }
        } else {
            None
        };
        let effective_response_cap = shared_reservation
            .as_ref()
            .map_or(response_cap, |reservation| Some(reservation.response_cap()));
        if effective_response_cap.is_some_and(|cap| {
            cap < diagnostics_minimum_response_bytes(&prepared).unwrap_or(usize::MAX)
        }) {
            drop(shared_reservation);
            if let Some(quota) = &shared_quota {
                quota.poison();
            }
            return diagnostics_incomplete(prepared, ExecutorFailure::InputBudget, None);
        }
        let argv = [
            c"tos-schema-worker".as_ptr() as *mut libc::c_char,
            std::ptr::null_mut(),
        ];
        let outcome = run_diagnostics_image(
            &image.file,
            prepared,
            budget,
            start,
            &argv,
            Some(cancelled),
            effective_response_cap,
        );
        let outcome = match outcome {
            SchemaDiagnosticsOutcome::Complete { units, checkpoint } => {
                match image.finish(deadline, cancelled) {
                    Ok(()) => SchemaDiagnosticsOutcome::Complete { units, checkpoint },
                    Err(reason) => {
                        let mut empty = Digest256Hasher::new();
                        empty.update(b"tos-schema-diagnostics-results-v2\0");
                        SchemaDiagnosticsOutcome::Incomplete {
                            checkpoint: SchemaDiagnosticsCheckpoint {
                                completed_count: 0,
                                result_stream_sha256: empty.finalize(),
                                ..checkpoint
                            },
                            reason,
                            exchange: None,
                        }
                    }
                }
            }
            incomplete => incomplete,
        };
        match (outcome, shared_reservation) {
            (SchemaDiagnosticsOutcome::Complete { units, checkpoint }, Some(reservation)) => {
                if checkpoint.completed_count != expected.count
                    || checkpoint.worker_request_bytes
                        != u64::try_from(reservation.request_bytes).unwrap_or(u64::MAX)
                {
                    reservation.quota.poison();
                    diagnostics_incomplete_from_checkpoint(
                        checkpoint,
                        ExecutorFailure::CoverageMismatch,
                    )
                } else {
                    match reservation.complete(
                        usize::try_from(checkpoint.worker_request_bytes).unwrap_or(usize::MAX),
                        usize::try_from(checkpoint.worker_response_bytes).unwrap_or(usize::MAX),
                        checkpoint.worker_cpu_micros,
                        checkpoint.completed_count,
                    ) {
                        Ok(()) => SchemaDiagnosticsOutcome::Complete { units, checkpoint },
                        Err(reason) => diagnostics_incomplete_from_checkpoint(checkpoint, reason),
                    }
                }
            }
            (incomplete @ SchemaDiagnosticsOutcome::Incomplete { .. }, Some(reservation)) => {
                drop(reservation);
                incomplete
            }
            (outcome, None) => outcome,
        }
    }

    fn batch_checkpoint(
        prepared: &BatchPrepared,
        completed_count: usize,
        results: Digest256Hasher,
    ) -> BatchCoverageCheckpoint {
        BatchCoverageCheckpoint {
            worker_sha256: prepared.worker_sha256,
            request_sha256: prepared.request_sha256,
            profile: prepared.profile,
            schema_set_sha256: prepared.schema_set_sha256,
            ordered_manifest_sha256: prepared.ordered_manifest_sha256,
            completed_count: completed_count as u64,
            result_stream_sha256: results.finalize(),
        }
    }

    fn batch_incomplete(
        prepared: BatchPrepared,
        receipts: Vec<BatchUnitReceipt>,
        results: Digest256Hasher,
        reason: ExecutorFailure,
    ) -> BatchOutcome {
        let checkpoint = batch_checkpoint(&prepared, receipts.len(), results);
        BatchOutcome::Incomplete {
            receipts,
            checkpoint,
            reason,
            exchange: None,
        }
    }

    fn diagnostics_checkpoint(
        prepared: &DiagnosticsPrepared,
        completed_count: usize,
        result_stream_sha256: Digest256,
        exceptional_usage: Option<ExceptionalSchemaUsage>,
    ) -> SchemaDiagnosticsCheckpoint {
        SchemaDiagnosticsCheckpoint {
            worker_sha256: prepared.worker_sha256,
            request_sha256: prepared.request_sha256,
            profile: prepared.profile,
            schema_set_sha256: prepared.schema_set_sha256,
            ordered_manifest_sha256: prepared.ordered_manifest_sha256,
            caps_sha256: prepared.caps.digest(),
            completed_count: completed_count as u64,
            result_stream_sha256,
            worker_request_bytes: 0,
            worker_response_bytes: 0,
            worker_cpu_micros: None,
            exceptional_remaining: prepared.exceptional_remaining,
            exceptional_usage,
        }
    }

    fn diagnostics_incomplete(
        prepared: DiagnosticsPrepared,
        reason: ExecutorFailure,
        exchange: Option<ExchangeFailureContext>,
    ) -> SchemaDiagnosticsOutcome {
        let mut empty = Digest256Hasher::new();
        empty.update(b"tos-schema-diagnostics-results-v2\0");
        SchemaDiagnosticsOutcome::Incomplete {
            checkpoint: diagnostics_checkpoint(&prepared, 0, empty.finalize(), None),
            reason,
            exchange,
        }
    }

    fn diagnostics_incomplete_with_observed_cost(
        prepared: DiagnosticsPrepared,
        reason: ExecutorFailure,
        exchange: Option<ExchangeFailureContext>,
        cost: &SchemaDiagnosticsExecutionCost,
    ) -> SchemaDiagnosticsOutcome {
        let mut outcome = diagnostics_incomplete(prepared, reason, exchange);
        if let SchemaDiagnosticsOutcome::Incomplete { checkpoint, .. } = &mut outcome {
            checkpoint.worker_request_bytes = u64::try_from(cost.request_bytes).unwrap_or(u64::MAX);
            checkpoint.worker_response_bytes =
                u64::try_from(cost.response_bytes).unwrap_or(u64::MAX);
            checkpoint.worker_cpu_micros = cost.worker_cpu_micros;
        }
        outcome
    }

    fn diagnostics_incomplete_from_checkpoint(
        checkpoint: SchemaDiagnosticsCheckpoint,
        reason: ExecutorFailure,
    ) -> SchemaDiagnosticsOutcome {
        SchemaDiagnosticsOutcome::Incomplete {
            checkpoint,
            reason,
            exchange: None,
        }
    }

    fn parse_diagnostic_path(
        cursor: &mut Cursor<'_>,
        caps: schema_diagnostics::Caps,
    ) -> io::Result<Vec<schema_diagnostics::PathSegment>> {
        let count = u16::from_be_bytes(cursor.take(2)?.try_into().unwrap()) as usize;
        if count > caps.max_path_segments as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "diagnostic path segments",
            ));
        }
        let mut path = Vec::with_capacity(count);
        let mut bytes = 0usize;
        for _ in 0..count {
            match cursor.take(1)?[0] {
                0 => {
                    let raw = cursor.bytes(caps.max_path_bytes as usize)?;
                    bytes = bytes
                        .checked_add(raw.len())
                        .filter(|size| *size <= caps.max_path_bytes as usize)
                        .ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "diagnostic path bytes")
                        })?;
                    let property = std::str::from_utf8(raw).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "diagnostic path utf8")
                    })?;
                    path.push(schema_diagnostics::PathSegment::Property(
                        property.to_owned(),
                    ));
                }
                1 => {
                    bytes = bytes
                        .checked_add(std::mem::size_of::<u64>())
                        .filter(|size| *size <= caps.max_path_bytes as usize)
                        .ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "diagnostic path bytes")
                        })?;
                    path.push(schema_diagnostics::PathSegment::Index(u64::from_be_bytes(
                        cursor.take(8)?.try_into().unwrap(),
                    )));
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "diagnostic path segment kind",
                    ));
                }
            }
        }
        Ok(path)
    }

    fn parse_diagnostic_issue(
        cursor: &mut Cursor<'_>,
        caps: schema_diagnostics::Caps,
    ) -> io::Result<schema_diagnostics::Issue> {
        let instance_path = parse_diagnostic_path(cursor, caps)?;
        let keyword_raw = cursor.bytes(64)?;
        let schema_keyword = std::str::from_utf8(keyword_raw)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "diagnostic schema keyword"))?;
        let reason = schema_diagnostics::Reason::from_wire(u16::from_be_bytes(
            cursor.take(2)?.try_into().unwrap(),
        ))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "diagnostic reason"))?;
        if schema_keyword != reason.schema_keyword() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "diagnostic keyword binding",
            ));
        }
        let schema_path = parse_diagnostic_path(cursor, caps)?;
        let compatibility_text =
            schema_diagnostics::CompatibilityText::from_wire(cursor.take(1)?[0]).ok_or_else(
                || io::Error::new(io::ErrorKind::InvalidData, "diagnostic compatibility text"),
            )?;
        Ok(schema_diagnostics::Issue {
            instance_path,
            schema_keyword: schema_keyword.to_owned(),
            reason,
            schema_path,
            compatibility_text,
        })
    }

    fn parse_diagnostic_ack(
        response: &[u8],
        prepared: &DiagnosticsPrepared,
    ) -> Result<(), ExecutorFailure> {
        if response.len() < DIAGNOSTIC_ACK_BYTES {
            return Err(ExecutorFailure::Protocol);
        }
        let ack = &response[..DIAGNOSTIC_ACK_BYTES];
        if &ack[..8] != DIAGNOSTIC_ACK_MAGIC
            || u16::from_be_bytes(ack[8..10].try_into().unwrap())
                != schema_diagnostics::PROTOCOL_VERSION
            || &ack[10..42] != prepared.request_sha256.as_bytes()
            || &ack[42..74] != prepared.worker_sha256.as_bytes()
            || &ack[74..106] != prepared.schema_set_sha256.as_bytes()
            || &ack[106..138] != prepared.caps.digest().as_bytes()
            || u32::from_be_bytes(ack[138..142].try_into().unwrap()) as usize
                != prepared.units.len()
        {
            return Err(ExecutorFailure::Protocol);
        }
        Ok(())
    }

    fn parse_diagnostics_response(
        response: &[u8],
        prepared: &DiagnosticsPrepared,
    ) -> Result<(Vec<SchemaDiagnosticUnit>, Option<ExceptionalSchemaUsage>), ExecutorFailure> {
        if response.len() > schema_diagnostics::MAX_RESPONSE_BYTES {
            return Err(ExecutorFailure::InputBudget);
        }
        parse_diagnostic_ack(response, prepared)?;
        let mut cursor = Cursor {
            bytes: response,
            offset: DIAGNOSTIC_ACK_BYTES,
        };
        let mut result_stream = Digest256Hasher::new();
        result_stream.update(b"tos-schema-diagnostics-results-v2\0");
        let mut units = Vec::with_capacity(prepared.units.len());
        for meta in &prepared.units {
            let fixed = cursor
                .take(DIAGNOSTIC_UNIT_HEADER_BYTES)
                .map_err(|_| ExecutorFailure::Protocol)?;
            if &fixed[..8] != DIAGNOSTIC_UNIT_MAGIC
                || u16::from_be_bytes(fixed[8..10].try_into().unwrap())
                    != schema_diagnostics::PROTOCOL_VERSION
                || u64::from_be_bytes(fixed[10..18].try_into().unwrap()) != meta.ordinal
                || &fixed[18..50] != meta.unit_sha256.as_bytes()
            {
                return Err(ExecutorFailure::Protocol);
            }
            let status = schema_diagnostics::Status::from_wire(fixed[50])
                .ok_or(ExecutorFailure::Protocol)?;
            let failure = schema_diagnostics::Failure::from_wire(fixed[51])
                .ok_or(ExecutorFailure::Protocol)?;
            let total_issue_count = u64::from_be_bytes(fixed[52..60].try_into().unwrap());
            let truncated = match fixed[60] {
                0 => false,
                1 => true,
                _ => return Err(ExecutorFailure::Protocol),
            };
            let issue_count = u32::from_be_bytes(fixed[61..65].try_into().unwrap()) as usize;
            let payload_len = u32::from_be_bytes(fixed[65..69].try_into().unwrap()) as usize;
            let issues_sha256 = Digest256::from_bytes(fixed[69..101].try_into().unwrap());
            if issue_count > prepared.caps.max_issues_per_unit as usize
                || payload_len
                    .checked_add(DIAGNOSTIC_UNIT_HEADER_BYTES)
                    .is_none_or(|size| size > prepared.caps.max_report_bytes_per_unit as usize)
            {
                return Err(ExecutorFailure::Protocol);
            }
            let payload = cursor
                .take(payload_len)
                .map_err(|_| ExecutorFailure::Protocol)?;
            let mut issue_cursor = Cursor {
                bytes: payload,
                offset: 0,
            };
            let mut issues = Vec::with_capacity(issue_count);
            for _ in 0..issue_count {
                issues.push(
                    parse_diagnostic_issue(&mut issue_cursor, prepared.caps)
                        .map_err(|_| ExecutorFailure::Protocol)?,
                );
            }
            if issue_cursor.offset != payload.len()
                || issues.windows(2).any(|pair| pair[0] > pair[1])
                || schema_diagnostics::issues_digest(&issues) != Some(issues_sha256)
                || !schema_diagnostics::status_is_well_formed(
                    status,
                    failure,
                    total_issue_count,
                    truncated,
                    issues.len(),
                )
            {
                return Err(ExecutorFailure::Protocol);
            }
            let report_sha256 = schema_diagnostics::report_digest(
                prepared.worker_sha256,
                prepared.request_sha256,
                meta.unit_sha256,
                prepared.schema_set_sha256,
                prepared.caps,
                status,
                failure,
                total_issue_count,
                truncated,
                issues_sha256,
            );
            let report = schema_diagnostics::Report {
                protocol_version: schema_diagnostics::PROTOCOL_VERSION,
                worker_sha256: prepared.worker_sha256,
                request_sha256: prepared.request_sha256,
                unit_sha256: meta.unit_sha256,
                schema_set_sha256: prepared.schema_set_sha256,
                caps: prepared.caps,
                status,
                failure,
                total_issue_count,
                truncated,
                issues_sha256,
                report_sha256,
                issues,
            };
            update_diagnostic_result_stream(&mut result_stream, meta.unit_sha256, &report);
            units.push(SchemaDiagnosticUnit {
                ordinal: meta.ordinal,
                member_id: meta.member_id.clone(),
                relative_path: meta.relative_path.clone(),
                root_uri: meta.root_uri.clone(),
                raw_sha256: meta.raw_sha256,
                unit_sha256: meta.unit_sha256,
                report,
            });
        }
        let final_bytes = prepared.final_record_bytes();
        let final_record = cursor
            .take(final_bytes)
            .map_err(|_| ExecutorFailure::Protocol)?;
        let result_sha256 = result_stream.finalize();
        if &final_record[..8] != DIAGNOSTIC_FINAL_MAGIC
            || u16::from_be_bytes(final_record[8..10].try_into().unwrap())
                != schema_diagnostics::PROTOCOL_VERSION
            || &final_record[10..42] != prepared.request_sha256.as_bytes()
            || &final_record[42..74] != prepared.worker_sha256.as_bytes()
            || &final_record[74..106] != prepared.schema_set_sha256.as_bytes()
            || &final_record[106..138] != prepared.caps.digest().as_bytes()
            || u32::from_be_bytes(final_record[138..142].try_into().unwrap()) as usize
                != prepared.units.len()
            || &final_record[142..174] != result_sha256.as_bytes()
            || cursor.offset != response.len()
        {
            return Err(ExecutorFailure::Protocol);
        }
        let exceptional_usage = match prepared.exceptional_remaining {
            Some(remaining) => {
                let usage =
                    ExceptionalSchemaUsage::read_be(&final_record[DIAGNOSTIC_FINAL_BYTES..])
                        .ok_or(ExecutorFailure::Protocol)?;
                if !usage.fits_within(remaining) {
                    return Err(ExecutorFailure::Protocol);
                }
                Some(usage)
            }
            None => None,
        };
        Ok((units, exceptional_usage))
    }

    fn run_diagnostics_image(
        image: &File,
        prepared: DiagnosticsPrepared,
        budget: BatchBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
        cancelled: Option<&AtomicBool>,
        response_cap: Option<usize>,
    ) -> SchemaDiagnosticsOutcome {
        run_diagnostics_image_with_cost(
            image,
            prepared,
            budget,
            start,
            argv,
            cancelled,
            response_cap,
        )
        .0
    }

    fn run_diagnostics_image_with_cost(
        image: &File,
        prepared: DiagnosticsPrepared,
        budget: BatchBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
        cancelled: Option<&AtomicBool>,
        response_cap: Option<usize>,
    ) -> (SchemaDiagnosticsOutcome, SchemaDiagnosticsExecutionCost) {
        let mut cost = SchemaDiagnosticsExecutionCost {
            request_buffer_bytes: prepared.frame.capacity(),
            ..SchemaDiagnosticsExecutionCost::default()
        };
        let outcome = match spawn_operation_child(
            image,
            ExecutorBudget {
                execution_wall: budget.total_execution_wall,
                cleanup_grace: budget.cleanup_grace,
                cpu_seconds: budget.cpu_seconds.min(60),
                address_space_bytes: budget.address_space_bytes,
            },
            argv,
        ) {
            Ok(mut child) => run_diagnostics_exchange(
                &mut child,
                prepared,
                budget,
                start,
                cancelled,
                response_cap,
                &mut cost,
            ),
            Err(reason) => diagnostics_incomplete(prepared, reason, None),
        };
        (diagnostics_with_wire_cost(outcome, &cost), cost)
    }

    fn diagnostics_with_wire_cost(
        outcome: SchemaDiagnosticsOutcome,
        cost: &SchemaDiagnosticsExecutionCost,
    ) -> SchemaDiagnosticsOutcome {
        let request_bytes = u64::try_from(cost.request_bytes).unwrap_or(u64::MAX);
        let response_bytes = u64::try_from(cost.response_bytes).unwrap_or(u64::MAX);
        match outcome {
            SchemaDiagnosticsOutcome::Complete {
                units,
                mut checkpoint,
            } => {
                checkpoint.worker_request_bytes = request_bytes;
                checkpoint.worker_response_bytes = response_bytes;
                checkpoint.worker_cpu_micros = cost.worker_cpu_micros;
                SchemaDiagnosticsOutcome::Complete { units, checkpoint }
            }
            SchemaDiagnosticsOutcome::Incomplete {
                mut checkpoint,
                reason,
                exchange,
            } => {
                checkpoint.worker_request_bytes = request_bytes;
                checkpoint.worker_response_bytes = response_bytes;
                checkpoint.worker_cpu_micros = cost.worker_cpu_micros;
                SchemaDiagnosticsOutcome::Incomplete {
                    checkpoint,
                    reason,
                    exchange,
                }
            }
        }
    }

    fn diagnostics_response_cap(prepared: &DiagnosticsPrepared) -> Option<usize> {
        (prepared.caps.max_report_bytes_per_unit as usize)
            .checked_mul(prepared.units.len())
            .and_then(|bytes| bytes.checked_add(DIAGNOSTIC_ACK_BYTES))
            .and_then(|bytes| bytes.checked_add(prepared.final_record_bytes()))
            .filter(|bytes| *bytes <= schema_diagnostics::MAX_RESPONSE_BYTES)
    }

    fn diagnostics_minimum_response_bytes(prepared: &DiagnosticsPrepared) -> Option<usize> {
        DIAGNOSTIC_UNIT_HEADER_BYTES
            .checked_mul(prepared.units.len())
            .and_then(|units| units.checked_add(DIAGNOSTIC_ACK_BYTES))
            .and_then(|bytes| bytes.checked_add(prepared.final_record_bytes()))
    }

    fn diagnostics_scalar_request_frame_bytes(
        encoded_resource_bytes: usize,
        input_profile: DiagnosticsInputProfile,
        member_id_bytes: usize,
        path_bytes: usize,
        root_uri_bytes: usize,
        instance_bytes: usize,
    ) -> Option<usize> {
        let extended_profile_bytes = match input_profile {
            DiagnosticsInputProfile::FiniteJson => 0,
            DiagnosticsInputProfile::LegacyPythonObserved
            | DiagnosticsInputProfile::FiniteJsonSelected => 2,
            DiagnosticsInputProfile::LegacyPythonObservedSelected => 2 + 8 + 4 + 8 + 8,
            DiagnosticsInputProfile::MixedSourceFoundation => return None,
        };
        let base = DIAGNOSTIC_FIXED_REQUEST_BYTES
            .checked_add(extended_profile_bytes)?
            .checked_add(encoded_resource_bytes)?
            .checked_add(4)? // unit count
            .checked_add(8)?; // unit ordinal
        [member_id_bytes, path_bytes, root_uri_bytes, instance_bytes]
            .into_iter()
            .try_fold(base, |total, bytes| {
                total.checked_add(4)?.checked_add(bytes)
            })
    }

    fn diagnostics_response_cap_with_wire_budget(
        prepared: &DiagnosticsPrepared,
        remaining_wire_bytes: u64,
    ) -> Option<usize> {
        if remaining_wire_bytes == u64::MAX {
            return None;
        }
        let full_response_cap = diagnostics_response_cap(prepared)?;
        let frame_bytes = u64::try_from(prepared.frame.len()).ok()?;
        let response_bytes_with_sentinel = remaining_wire_bytes.checked_sub(frame_bytes)?;
        let response_cap_u64 = response_bytes_with_sentinel.checked_sub(1)?;
        let response_cap = usize::try_from(response_cap_u64).ok()?;
        let response_cap = full_response_cap.min(response_cap);
        let minimum_response_bytes = diagnostics_minimum_response_bytes(prepared)?;
        (response_cap >= minimum_response_bytes).then_some(response_cap)
    }

    fn run_diagnostics_exchange(
        child: &mut OperationChild,
        prepared: DiagnosticsPrepared,
        budget: BatchBudget,
        start: Instant,
        cancelled: Option<&AtomicBool>,
        response_cap: Option<usize>,
        cost: &mut SchemaDiagnosticsExecutionCost,
    ) -> SchemaDiagnosticsOutcome {
        let mut written = 0usize;
        let full_response_cap = diagnostics_response_cap(&prepared);
        let Some(full_response_cap) = full_response_cap else {
            let _ = child.cleanup();
            return diagnostics_incomplete(prepared, ExecutorFailure::InputBudget, None);
        };
        let response_cap = response_cap.unwrap_or(full_response_cap);
        if response_cap > full_response_cap
            || diagnostics_minimum_response_bytes(&prepared)
                .is_none_or(|minimum| response_cap < minimum)
        {
            let _ = child.cleanup();
            return diagnostics_incomplete(prepared, ExecutorFailure::InputBudget, None);
        }
        let mut response = Vec::with_capacity(response_cap);
        cost.response_buffer_bytes = response.capacity();
        let mut output_eof = false;
        let mut status = child.status;
        let mut input = Some(&child.input);
        let mut last_progress = start;
        let mut failure: Option<(ExecutorFailure, &'static str)> = None;
        let mut ack = false;
        loop {
            if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                failure = Some((ExecutorFailure::Cancelled, "diagnostics-cancellation"));
                break;
            }
            let now = Instant::now();
            if now.duration_since(start) >= budget.total_execution_wall {
                failure = Some((ExecutorFailure::Timeout, "diagnostics-total-wall"));
                break;
            }
            if !ack && now.duration_since(start) >= budget.startup_wall {
                failure = Some((ExecutorFailure::Timeout, "diagnostics-startup-wall"));
                break;
            }
            if ack && now.duration_since(last_progress) >= budget.per_unit_wall {
                failure = Some((ExecutorFailure::Timeout, "diagnostics-unit-wall"));
                break;
            }
            match poll_exit_with_usage(child.pid, &mut status) {
                Ok(Some(cpu_micros)) => cost.worker_cpu_micros = Some(cpu_micros),
                Ok(None) => {}
                Err(reason) => {
                    failure = Some((reason, "diagnostics-child-status"));
                    break;
                }
            }
            if !output_eof && written == prepared.frame.len() {
                if let Some(fd) = input.take() {
                    unsafe { libc::shutdown(fd.as_raw_fd(), libc::SHUT_WR) };
                }
            }
            let mut fds = [
                libc::pollfd {
                    fd: if written < prepared.frame.len() {
                        input.map_or(-1, |fd| fd.as_raw_fd())
                    } else {
                        -1
                    },
                    events: libc::POLLOUT,
                    revents: 0,
                },
                libc::pollfd {
                    fd: if output_eof {
                        -1
                    } else {
                        child.output.as_raw_fd()
                    },
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            if unsafe { libc::poll(fds.as_mut_ptr(), 2, 2) } < 0 {
                if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                    failure = Some((ExecutorFailure::Protocol, "diagnostics-poll"));
                    break;
                }
                continue;
            }
            if fds[0].revents & libc::POLLOUT != 0 {
                let count = unsafe {
                    libc::send(
                        fds[0].fd,
                        prepared.frame[written..].as_ptr().cast(),
                        prepared.frame.len() - written,
                        libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                    )
                };
                if count > 0 {
                    written += count as usize;
                    cost.request_bytes = written;
                } else if count == 0
                    || (count < 0 && io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock)
                {
                    failure = Some((ExecutorFailure::Protocol, "diagnostics-request-send"));
                    break;
                }
            }
            if fds[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                let mut buffer = [0u8; 4096];
                let remaining_with_overflow = response_cap
                    .saturating_sub(response.len())
                    .saturating_add(1)
                    .min(buffer.len());
                let count = unsafe {
                    libc::recv(
                        fds[1].fd,
                        buffer.as_mut_ptr().cast(),
                        remaining_with_overflow,
                        libc::MSG_DONTWAIT,
                    )
                };
                if count == 0 {
                    output_eof = true;
                } else if count > 0 {
                    response.extend_from_slice(&buffer[..count as usize]);
                    cost.response_bytes = response.len();
                    last_progress = Instant::now();
                    if response.len() > response_cap {
                        failure = Some((ExecutorFailure::InputBudget, "diagnostics-response-cap"));
                        break;
                    }
                    if !ack && response.len() >= DIAGNOSTIC_ACK_BYTES {
                        if let Err(reason) = parse_diagnostic_ack(&response, &prepared) {
                            failure = Some((reason, "diagnostics-ack"));
                            break;
                        }
                        ack = true;
                        last_progress = Instant::now();
                    }
                } else if io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                    failure = Some((ExecutorFailure::Protocol, "diagnostics-response-receive"));
                    break;
                }
            }
            if fds
                .iter()
                .any(|fd| fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0)
            {
                failure = Some((ExecutorFailure::Protocol, "diagnostics-socket-events"));
                break;
            }
            if output_eof && !ack {
                failure = Some((ExecutorFailure::Protocol, "diagnostics-missing-ack"));
                break;
            }
            if output_eof && written == prepared.frame.len() && status.is_some() {
                break;
            }
            if status.is_some() && written != prepared.frame.len() {
                failure = Some((ExecutorFailure::Protocol, "diagnostics-early-exit"));
                break;
            }
        }
        child.status = status;
        if let Some((reason, boundary)) = failure {
            let cleanup = if reason == ExecutorFailure::Protocol && output_eof {
                child.cleanup_after_eof()
            } else {
                child.cleanup()
            };
            let natural_termination = child.natural_status.map(|status| {
                let signal = status & 0x7f;
                if signal == 0 {
                    ChildTermination::Exited((status >> 8) & 0xff)
                } else {
                    ChildTermination::Signalled(signal)
                }
            });
            return diagnostics_incomplete_with_observed_cost(
                prepared,
                cleanup
                    .err()
                    .unwrap_or_else(|| status.and_then(status_failure).unwrap_or(reason)),
                Some(ExchangeFailureContext {
                    boundary,
                    failure: reason,
                    natural_termination,
                    child_cpu_micros: child.natural_cpu_micros.or(cost.worker_cpu_micros),
                    child_pid: child.pid,
                    child_exchange_ordinal: 1,
                    retained_session: false,
                }),
                cost,
            );
        }
        if let Some(reason) = status.and_then(status_failure) {
            return diagnostics_incomplete_with_observed_cost(prepared, reason, None, cost);
        }
        if let Err(reason) = child.cleanup_after_eof() {
            return diagnostics_incomplete_with_observed_cost(prepared, reason, None, cost);
        }
        let (units, exceptional_usage) = match parse_diagnostics_response(&response, &prepared) {
            Ok(parsed) => parsed,
            Err(reason) => {
                return diagnostics_incomplete_with_observed_cost(prepared, reason, None, cost);
            }
        };
        let mut result_stream = Digest256Hasher::new();
        result_stream.update(b"tos-schema-diagnostics-results-v2\0");
        for unit in &units {
            update_diagnostic_result_stream(&mut result_stream, unit.unit_sha256, &unit.report);
        }
        let checkpoint = diagnostics_checkpoint(
            &prepared,
            units.len(),
            result_stream.finalize(),
            exceptional_usage,
        );
        SchemaDiagnosticsOutcome::Complete { units, checkpoint }
    }

    /// Snapshot the verified worker into an executable, sealed in-memory file.
    /// Hashing the *copy* removes the in-place mutation race of path+hash+exec.
    fn sealed_worker(worker: &ExactWorkerIdentity) -> Result<File, ExecutorFailure> {
        sealed_worker_checked(worker, None, None)
    }

    fn preparation_check(
        deadline: Option<Instant>,
        cancelled: Option<&AtomicBool>,
    ) -> Result<(), ExecutorFailure> {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Err(ExecutorFailure::Cancelled);
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(ExecutorFailure::Timeout);
        }
        Ok(())
    }

    fn sealed_worker_checked(
        worker: &ExactWorkerIdentity,
        deadline: Option<Instant>,
        cancelled: Option<&AtomicBool>,
    ) -> Result<File, ExecutorFailure> {
        preparation_check(deadline, cancelled)?;
        if !worker.absolute_path.is_absolute() {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let mut source = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&worker.absolute_path)
            .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        let metadata = source
            .metadata()
            .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        if !metadata.file_type().is_file()
            || metadata.permissions().mode() & 0o111 == 0
            || metadata.len() == 0
            || metadata.len() > MAX_WORKER_BYTES
        {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let fd = unsafe {
            libc::memfd_create(
                c"tos-schema-worker".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | MFD_EXEC_FLAG,
            )
        };
        if fd < 0 {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let mut sealed = unsafe { File::from_raw_fd(fd) };
        let mut digest = Digest256Hasher::new();
        let mut copied = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            preparation_check(deadline, cancelled)?;
            let count = source
                .read(&mut buffer)
                .map_err(|_| ExecutorFailure::WorkerIdentity)?;
            if count == 0 {
                break;
            }
            copied = copied
                .checked_add(count as u64)
                .ok_or(ExecutorFailure::WorkerIdentity)?;
            if copied > MAX_WORKER_BYTES {
                return Err(ExecutorFailure::WorkerIdentity);
            }
            digest.update(&buffer[..count]);
            sealed
                .write_all(&buffer[..count])
                .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        }
        preparation_check(deadline, cancelled)?;
        if copied != metadata.len() || digest.finalize() != worker.sha256 {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let seals =
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        if unsafe { libc::fcntl(sealed.as_raw_fd(), libc::F_ADD_SEALS, seals) } != 0 {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        let after = source
            .metadata()
            .map_err(|_| ExecutorFailure::WorkerIdentity)?;
        if metadata.dev() != after.dev() || metadata.ino() != after.ino() {
            return Err(ExecutorFailure::WorkerIdentity);
        }
        Ok(sealed)
    }

    // After fork, use only libc calls until exec. No Rust allocator, lock, or
    // destructor may run in a possibly multithreaded parent process's child.
    unsafe fn child_exec(
        worker_fd: i32,
        input_fd: i32,
        output_fd: i32,
        null_fd: i32,
        budget: ExecutorBudget,
        parent_pid: libc::pid_t,
        argv: *const *mut libc::c_char,
    ) -> ! {
        let as_limit = libc::rlimit {
            rlim_cur: budget.address_space_bytes,
            rlim_max: budget.address_space_bytes,
        };
        let cpu_limit = libc::rlimit {
            rlim_cur: budget.cpu_seconds.saturating_sub(1).max(1),
            rlim_max: budget.cpu_seconds,
        };
        if unsafe { libc::setpgid(0, 0) } != 0
            || unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) } != 0
            || unsafe { libc::getppid() } != parent_pid
            || unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
            || unsafe { libc::setrlimit(libc::RLIMIT_AS, &as_limit) } != 0
            || unsafe { libc::setrlimit(libc::RLIMIT_CPU, &cpu_limit) } != 0
            || unsafe { libc::chdir(c"/".as_ptr()) } != 0
            || unsafe { libc::dup2(input_fd, 0) } < 0
            || unsafe { libc::dup2(output_fd, 1) } < 0
            || unsafe { libc::dup2(null_fd, 2) } < 0
        {
            unsafe { libc::_exit(126) };
        }
        // The executable fd remains usable for execveat; all ambient fds
        // (including socket peers and the memfd) close on successful exec.
        const CLOSE_RANGE_CLOEXEC_FLAG: libc::c_int = 4;
        if unsafe { libc::close_range(3, u32::MAX, CLOSE_RANGE_CLOEXEC_FLAG) } != 0 {
            unsafe { libc::_exit(126) };
        }
        let env: [*mut libc::c_char; 1] = [std::ptr::null_mut()];
        unsafe {
            libc::execveat(
                worker_fd,
                c"".as_ptr(),
                argv,
                env.as_ptr(),
                libc::AT_EMPTY_PATH,
            );
            libc::_exit(126)
        }
    }

    fn socket_pair() -> io::Result<(File, File)> {
        let mut fds = [-1, -1];
        if unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_CLOEXEC,
                0,
                fds.as_mut_ptr(),
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) })
    }

    fn poll_exit(pid: i32, status: &mut Option<i32>) -> Result<(), ExecutorFailure> {
        if status.is_some() {
            return Ok(());
        }
        let mut raw = 0;
        let observed = unsafe { libc::waitpid(pid, &mut raw, libc::WNOHANG) };
        if observed == pid {
            *status = Some(raw);
            Ok(())
        } else if observed == 0 {
            Ok(())
        } else {
            Err(ExecutorFailure::ResourceLimitUnknown)
        }
    }

    fn poll_exit_with_usage(
        pid: i32,
        status: &mut Option<i32>,
    ) -> Result<Option<u64>, ExecutorFailure> {
        if status.is_some() {
            return Ok(None);
        }
        let mut raw = 0;
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        let observed = unsafe { libc::wait4(pid, &mut raw, libc::WNOHANG, &mut usage) };
        if observed == pid {
            *status = Some(raw);
            let micros = |value: libc::timeval| -> Option<u64> {
                u64::try_from(value.tv_sec)
                    .ok()?
                    .checked_mul(1_000_000)?
                    .checked_add(u64::try_from(value.tv_usec).ok()?)
            };
            micros(usage.ru_utime)
                .and_then(|user| micros(usage.ru_stime).and_then(|system| user.checked_add(system)))
                .map(Some)
                .ok_or(ExecutorFailure::ResourceLimitUnknown)
        } else if observed == 0
            || (observed < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted)
        {
            Ok(None)
        } else {
            Err(ExecutorFailure::ResourceLimitUnknown)
        }
    }

    fn kill_and_reap(
        pid: i32,
        status: &mut Option<i32>,
        reap_deadline: Instant,
    ) -> Result<(), ExecutorFailure> {
        // waitpid already released this PID/PGID for reuse. Never signal it
        // after that point, even if a descendant kept a protocol socket open.
        if status.is_some() {
            return Ok(());
        }
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
            libc::kill(pid, libc::SIGKILL);
        }
        // Always attempt a nonblocking reap after signalling, even when the
        // shared cleanup deadline has just elapsed.
        loop {
            poll_exit(pid, status)?;
            if status.is_some() || Instant::now() >= reap_deadline {
                break;
            }
            thread::sleep(
                Duration::from_millis(1)
                    .min(reap_deadline.saturating_duration_since(Instant::now())),
            );
        }
        if status.is_none() {
            // A kernel-uninterruptible child cannot be reaped on a deadline.
            // Report the exact residual PID to the caller/host supervisor;
            // no unbounded blocking or detached request thread is hidden.
            return Err(ExecutorFailure::ReapPending(pid));
        }
        Ok(())
    }

    fn status_failure(status: i32) -> Option<ExecutorFailure> {
        let signal = status & 0x7f;
        if signal != 0 {
            return Some(if signal == libc::SIGXCPU {
                ExecutorFailure::CpuLimit
            } else {
                ExecutorFailure::CrashSignal(signal)
            });
        }
        let exit_code = (status >> 8) & 0xff;
        (exit_code != 0).then_some(ExecutorFailure::CrashExit(exit_code))
    }

    pub(super) fn evaluate(
        worker: &ExactWorkerIdentity,
        resources: &[SchemaResource],
        profile: FormatProfile,
        root_uri: &str,
        raw_instance: &[u8],
        budget: ExecutorBudget,
        cancelled: Option<&AtomicBool>,
    ) -> ExecutorOutcome {
        let local_cancelled = AtomicBool::new(false);
        let cancelled = cancelled.unwrap_or(&local_cancelled);
        let start = Instant::now();
        let deadline = start.checked_add(budget.execution_wall).unwrap_or(start);
        if let Err(reason) =
            scalar_budget(budget).and_then(|_| preparation_check(Some(deadline), Some(cancelled)))
        {
            return unknown(reason, None);
        }
        if raw_instance.len() > crate::SchemaBackendProbe::MAX_INSTANCE_BYTES
            || root_uri.len() > MAX_URI_BYTES
        {
            return unknown(ExecutorFailure::InputBudget, None);
        }
        let (encoded, set) = match encode_resources(resources) {
            Ok(x) => x,
            Err(reason) => return unknown(reason, None),
        };
        let mut image = match VerifiedWorkerImage::prepare(worker, budget, deadline, cancelled) {
            Ok(x) => x,
            Err(reason) => return unknown(reason, None),
        };
        let mut work = BatchStreamBudget::laboratory();
        work.batch = BatchBudget {
            total_execution_wall: budget.execution_wall,
            startup_wall: budget.execution_wall,
            per_unit_wall: budget.execution_wall,
            cleanup_grace: budget.cleanup_grace,
            cpu_seconds: budget.cpu_seconds,
            address_space_bytes: budget.address_space_bytes,
            max_units: 1,
            max_total_raw_bytes: crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
        };
        work.max_chunks = 1;
        work.max_total_units = 1;
        work.max_total_raw_bytes = crate::SchemaBackendProbe::MAX_INSTANCE_BYTES as u64;
        work.max_total_wire_bytes = (MAX_FRAME_BYTES
            + MAX_URI_BYTES
            + OPERATION_HEADER_BYTES
            + 2 * OPERATION_FINAL_BYTES
            + 1024) as u64;
        work.max_distinct_selectors = 1;
        work.operation_cpu_seconds = budget.cpu_seconds;
        work.operation_address_space_bytes = budget.address_space_bytes;
        work.total_execution_wall = budget.execution_wall;
        if let Err(reason) = image.set_operation_budget(work) {
            return unknown(reason, None);
        }
        let outcome = image.evaluate_encoded(
            &encoded,
            set,
            profile,
            root_uri,
            raw_instance,
            budget,
            deadline,
            cancelled,
        );
        if matches!(
            outcome,
            ExecutorOutcome::SchemaValid(_) | ExecutorOutcome::SchemaInvalid(_)
        ) {
            if let Err(reason) = image.finish(deadline, cancelled) {
                return unknown(
                    reason,
                    match outcome {
                        ExecutorOutcome::SchemaValid(x) | ExecutorOutcome::SchemaInvalid(x) => {
                            Some(x)
                        }
                        _ => None,
                    },
                );
            }
        }
        outcome
    }

    #[cfg(test)]
    fn run_image(
        image: &File,
        request: Vec<u8>,
        identity: ExecutionIdentity,
        budget: ExecutorBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
        cancelled: Option<&AtomicBool>,
    ) -> ExecutorOutcome {
        // Existing process-custody faults use the same parent I/O/cleanup path.
        let prepared = BatchPrepared {
            frame: request,
            units: vec![BatchUnitMeta {
                ordinal: 0,
                member_id: "custody-probe".into(),
                relative_path: "custody-probe".into(),
                root_uri: "custody-probe".into(),
                raw_sha256: identity.instance_sha256,
                unit_sha256: identity.instance_sha256,
            }],
            worker_sha256: identity.worker_sha256,
            profile: identity.profile,
            schema_set_sha256: identity.schema_set_sha256,
            request_sha256: identity.request_sha256,
            ordered_manifest_sha256: identity.instance_sha256,
        };
        let mut results = Digest256Hasher::new();
        results.update(b"tos-val2-batch-results-v1\0");
        let batch = BatchBudget {
            total_execution_wall: budget.execution_wall,
            startup_wall: budget.execution_wall,
            per_unit_wall: budget.execution_wall,
            cleanup_grace: budget.cleanup_grace,
            cpu_seconds: budget.cpu_seconds,
            address_space_bytes: budget.address_space_bytes,
            max_units: 1,
            max_total_raw_bytes: crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
        };
        match run_batch_image_cancellable(image, prepared, results, batch, start, argv, cancelled) {
            BatchOutcome::Incomplete {
                reason, exchange, ..
            } => ExecutorOutcome::Indeterminate {
                reason,
                identity: Some(identity),
                exchange,
            },
            BatchOutcome::Complete { .. } => unknown(ExecutorFailure::Protocol, Some(identity)),
        }
    }

    #[cfg(test)]
    fn run_batch_image(
        image: &File,
        prepared: BatchPrepared,
        results: Digest256Hasher,
        budget: BatchBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
    ) -> BatchOutcome {
        run_batch_image_cancellable(image, prepared, results, budget, start, argv, None)
    }

    struct OperationChild {
        pid: libc::pid_t,
        input: File,
        output: File,
        status: Option<i32>,
        natural_status: Option<i32>,
        natural_cpu_micros: Option<u64>,
        exchanges_started: u64,
        cleanup_grace: Duration,
        cleanup_result: Option<Result<(), ExecutorFailure>>,
    }
    impl OperationChild {
        fn cleanup(&mut self) -> Result<(), ExecutorFailure> {
            self.cleanup_inner(false)
        }
        fn cleanup_after_eof(&mut self) -> Result<(), ExecutorFailure> {
            self.cleanup_inner(true)
        }
        fn cleanup_inner(&mut self, observe_eof_exit: bool) -> Result<(), ExecutorFailure> {
            if let Some(result) = self.cleanup_result {
                return result;
            }
            let started = Instant::now();
            let reap_deadline = started + self.cleanup_grace;
            // EOF can precede a waitable status. Reserve at least half of the
            // same cleanup grace for forced termination/reaping; this is a
            // best-effort natural observation, not a promised cause capture.
            let observation_deadline = started + self.cleanup_grace / 2;
            let mut observed = poll_exit_with_usage(self.pid, &mut self.status).map(|cpu| {
                if let Some(cpu) = cpu {
                    self.natural_cpu_micros = Some(cpu);
                }
            });
            while observe_eof_exit
                && observed.is_ok()
                && self.status.is_none()
                && Instant::now() < observation_deadline
            {
                thread::sleep(
                    Duration::from_millis(1)
                        .min(observation_deadline.saturating_duration_since(Instant::now())),
                );
                observed = poll_exit_with_usage(self.pid, &mut self.status).map(|cpu| {
                    if let Some(cpu) = cpu {
                        self.natural_cpu_micros = Some(cpu);
                    }
                });
            }
            // Only status collected before any parent kill belongs to origin.
            self.natural_status = self.status;
            let reaped = kill_and_reap(self.pid, &mut self.status, reap_deadline);
            let result = reaped.and(observed);
            self.cleanup_result = Some(result);
            result
        }
    }
    impl Drop for OperationChild {
        fn drop(&mut self) {
            let _ = self.cleanup();
        }
    }
    fn spawn_operation_child(
        image: &File,
        budget: ExecutorBudget,
        argv: &[*mut libc::c_char],
    ) -> Result<OperationChild, ExecutorFailure> {
        let (input_parent, input_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => {
                return Err(ExecutorFailure::Spawn);
            }
        };
        let (output_parent, output_child) = match socket_pair() {
            Ok(pair) => pair,
            Err(_) => {
                return Err(ExecutorFailure::Spawn);
            }
        };
        #[cfg(test)]
        TEST_CHILD_STDOUT_INODE.with(|cell| cell.set(output_child.metadata().unwrap().ino()));
        let null = match OpenOptions::new().write(true).open("/dev/null") {
            Ok(file) => file,
            Err(_) => {
                return Err(ExecutorFailure::Spawn);
            }
        };
        let parent_pid = unsafe { libc::getpid() };
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(ExecutorFailure::Spawn);
        }
        if pid == 0 {
            unsafe {
                child_exec(
                    image.as_raw_fd(),
                    input_child.as_raw_fd(),
                    output_child.as_raw_fd(),
                    null.as_raw_fd(),
                    budget,
                    parent_pid,
                    argv.as_ptr(),
                )
            }
        }
        drop(input_child);
        drop(output_child);
        drop(null);

        Ok(OperationChild {
            pid,
            input: input_parent,
            output: output_parent,
            status: None,
            natural_status: None,
            natural_cpu_micros: None,
            exchanges_started: 0,
            cleanup_grace: budget.cleanup_grace,
            cleanup_result: None,
        })
    }

    #[cfg(test)]
    fn run_batch_image_cancellable(
        image: &File,
        prepared: BatchPrepared,
        mut results: Digest256Hasher,
        budget: BatchBudget,
        start: Instant,
        argv: &[*mut libc::c_char],
        cancelled: Option<&AtomicBool>,
    ) -> BatchOutcome {
        let mut child = match spawn_operation_child(
            image,
            ExecutorBudget {
                execution_wall: budget.total_execution_wall,
                cleanup_grace: budget.cleanup_grace,
                cpu_seconds: budget.cpu_seconds,
                address_space_bytes: budget.address_space_bytes,
            },
            argv,
        ) {
            Ok(child) => child,
            Err(reason) => return batch_incomplete(prepared, Vec::new(), results, reason),
        };
        run_batch_exchange(
            &mut child, prepared, results, budget, start, cancelled, false,
        )
    }

    fn run_batch_exchange(
        child: &mut OperationChild,
        prepared: BatchPrepared,
        mut results: Digest256Hasher,
        budget: BatchBudget,
        start: Instant,
        cancelled: Option<&AtomicBool>,
        retained: bool,
    ) -> BatchOutcome {
        child.exchanges_started = child.exchanges_started.saturating_add(1);
        let pid = child.pid;
        let input_parent = &child.input;
        let output_parent = &child.output;
        let mut input = Some(input_parent);
        let output = output_parent;
        let mut written = 0usize;
        let mut response =
            Vec::with_capacity(BATCH_ACK_BYTES + prepared.units.len() * BATCH_UNIT_BYTES);
        let mut parsed = 0usize;
        let mut ack = false;
        let mut unit_deadline = None;
        let mut receipts = Vec::with_capacity(prepared.units.len());
        let mut output_eof = false;
        let mut status = child.status;
        let mut failure = None;
        let mut terminal = false;
        let expected_bytes = BATCH_ACK_BYTES
            + prepared.units.len() * BATCH_UNIT_BYTES
            + if retained { OPERATION_END_BYTES } else { 0 };
        while if retained {
            !terminal
        } else {
            !output_eof || status.is_none() || receipts.len() != prepared.units.len()
        } {
            if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                failure = Some((ExecutorFailure::Cancelled, "cancellation"));
                break;
            }
            let now = Instant::now();
            let timeout_boundary = if now.duration_since(start) >= budget.total_execution_wall {
                Some("exchange-wall")
            } else if !ack && now.duration_since(start) >= budget.startup_wall {
                Some("ack-startup-wall")
            } else if unit_deadline.is_some_and(|deadline| now >= deadline) {
                Some("unit-wall")
            } else {
                None
            };
            if let Some(boundary) = timeout_boundary {
                failure = Some((ExecutorFailure::Timeout, boundary));
                break;
            }
            match poll_exit_with_usage(pid, &mut status) {
                Ok(Some(cpu)) => child.natural_cpu_micros = Some(cpu),
                Ok(None) => {}
                Err(reason) => {
                    failure = Some((reason, "child-status-poll"));
                    break;
                }
            }
            if !retained && written == prepared.frame.len() {
                if let Some(fd) = input.take() {
                    unsafe { libc::shutdown(fd.as_raw_fd(), libc::SHUT_WR) };
                }
            }
            let mut fds = [
                libc::pollfd {
                    fd: if written < prepared.frame.len() {
                        input.as_ref().map_or(-1, |fd| fd.as_raw_fd())
                    } else {
                        -1
                    },
                    events: libc::POLLOUT,
                    revents: 0,
                },
                libc::pollfd {
                    fd: if output_eof { -1 } else { output.as_raw_fd() },
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            if unsafe { libc::poll(fds.as_mut_ptr(), 2, 2) } < 0 {
                if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                    failure = Some((ExecutorFailure::Protocol, "poll-system-call"));
                    break;
                }
                continue;
            }
            if fds[0].revents & libc::POLLOUT != 0 {
                let count = unsafe {
                    libc::send(
                        fds[0].fd,
                        prepared.frame[written..].as_ptr().cast(),
                        prepared.frame.len() - written,
                        libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                    )
                };
                if count > 0 {
                    written += count as usize;
                } else if count == 0
                    || (count < 0 && io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock)
                {
                    failure = Some((ExecutorFailure::Protocol, "request-send"));
                    break;
                }
            }
            if fds[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
                let mut buffer = [0u8; 1024];
                let count = unsafe {
                    libc::recv(
                        fds[1].fd,
                        buffer.as_mut_ptr().cast(),
                        buffer.len(),
                        libc::MSG_DONTWAIT,
                    )
                };
                if count == 0 {
                    output_eof = true;
                } else if count > 0 {
                    response.extend_from_slice(&buffer[..count as usize]);
                    if response.len() > expected_bytes {
                        failure = Some((ExecutorFailure::Protocol, "response-byte-count"));
                        break;
                    }
                    loop {
                        if parsed == response.len() {
                            break;
                        }
                        if !ack {
                            if response.len() - parsed < BATCH_ACK_BYTES {
                                break;
                            }
                            let bytes = &response[parsed..parsed + BATCH_ACK_BYTES];
                            if &bytes[..8] != BATCH_ACK_MAGIC
                                || &bytes[8..40] != prepared.request_sha256.as_bytes()
                                || &bytes[40..72] != prepared.schema_set_sha256.as_bytes()
                                || u32::from_be_bytes(bytes[72..76].try_into().unwrap()) as usize
                                    != prepared.units.len()
                            {
                                failure = Some((ExecutorFailure::Protocol, "ack-identity"));
                                break;
                            }
                            parsed += BATCH_ACK_BYTES;
                            ack = true;
                            unit_deadline = Some(Instant::now() + budget.per_unit_wall);
                        } else if receipts.len() < prepared.units.len() {
                            if response.len() - parsed < BATCH_UNIT_BYTES {
                                break;
                            }
                            let bytes = &response[parsed..parsed + BATCH_UNIT_BYTES];
                            let meta = &prepared.units[receipts.len()];
                            if &bytes[..8] != BATCH_UNIT_MAGIC
                                || u64::from_be_bytes(bytes[8..16].try_into().unwrap())
                                    != meta.ordinal
                                || &bytes[16..48] != meta.unit_sha256.as_bytes()
                            {
                                failure = Some((ExecutorFailure::Protocol, "unit-identity"));
                                break;
                            }
                            let verdict = match (bytes[48], bytes[49]) {
                                (0, 0) => BatchUnitVerdict::SchemaValid,
                                (1, 0) => BatchUnitVerdict::SchemaInvalid,
                                (2, 1) => BatchUnitVerdict::InputRejected,
                                (3, 1) => {
                                    failure = Some((
                                        ExecutorFailure::InputBudget,
                                        "worker-unit-input-budget",
                                    ));
                                    break;
                                }
                                (3, 2) => {
                                    failure =
                                        Some((ExecutorFailure::Backend, "worker-unit-backend"));
                                    break;
                                }
                                _ => {
                                    failure = Some((ExecutorFailure::Protocol, "unit-verdict"));
                                    break;
                                }
                            };
                            parsed += BATCH_UNIT_BYTES;
                            results.update(meta.unit_sha256.as_bytes());
                            results.update(&[bytes[48], bytes[49]]);
                            receipts.push(BatchUnitReceipt {
                                ordinal: meta.ordinal,
                                member_id: meta.member_id.clone(),
                                relative_path: meta.relative_path.clone(),
                                root_uri: meta.root_uri.clone(),
                                raw_sha256: meta.raw_sha256,
                                unit_sha256: meta.unit_sha256,
                                verdict,
                            });
                            if verdict == BatchUnitVerdict::InputRejected {
                                failure =
                                    Some((ExecutorFailure::ParseRejected, "worker-unit-parse"));
                                break;
                            }
                            unit_deadline = Some(Instant::now() + budget.per_unit_wall);
                        } else if retained && !terminal {
                            if response.len() - parsed < OPERATION_END_BYTES {
                                break;
                            }
                            let end = &response[parsed..parsed + OPERATION_END_BYTES];
                            if &end[..8] != OPERATION_END_MAGIC
                                || &end[8..40] != prepared.request_sha256.as_bytes()
                                || &end[40..72] != results.clone().finalize().as_bytes()
                                || u32::from_be_bytes(end[72..76].try_into().unwrap()) as usize
                                    != receipts.len()
                            {
                                failure = Some((ExecutorFailure::Protocol, "terminal-identity"));
                                break;
                            }
                            parsed += OPERATION_END_BYTES;
                            terminal = true;
                        } else {
                            failure = Some((ExecutorFailure::Protocol, "trailing-response"));
                            break;
                        }
                    }
                    if failure.is_some() {
                        break;
                    }
                } else if io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                    failure = Some((ExecutorFailure::Protocol, "response-receive"));
                    break;
                }
            }
            if fds
                .iter()
                .any(|fd| fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0)
            {
                failure = Some((ExecutorFailure::Protocol, "socket-events"));
                break;
            }
            if retained && (output_eof || status.is_some()) {
                failure = Some((
                    ExecutorFailure::Protocol,
                    if output_eof {
                        "early-output-eof"
                    } else {
                        "early-child-exit"
                    },
                ));
                break;
            }
            if output_eof
                && (parsed != response.len() || !ack || receipts.len() != prepared.units.len())
            {
                failure = Some((ExecutorFailure::Protocol, "incomplete-eof"));
                break;
            }
        }
        if retained && failure.is_none() && written != prepared.frame.len() {
            failure = Some((ExecutorFailure::Protocol, "incomplete-request-write"));
        }
        if retained && failure.is_none() {
            let mut extra = [0u8; 1];
            let count = unsafe {
                libc::recv(
                    output.as_raw_fd(),
                    extra.as_mut_ptr().cast(),
                    1,
                    libc::MSG_DONTWAIT,
                )
            };
            if count >= 0 || io::Error::last_os_error().kind() != io::ErrorKind::WouldBlock {
                output_eof |= count == 0;
                failure = Some((
                    ExecutorFailure::Protocol,
                    if count == 0 {
                        "post-terminal-eof"
                    } else if count > 0 {
                        "post-terminal-extra-byte"
                    } else {
                        "post-terminal-receive"
                    },
                ));
            }
        }
        child.status = status;
        if let Some((reason, boundary)) = failure {
            // Timeout/cancellation and non-EOF failures keep prompt cleanup.
            let cleanup = if reason == ExecutorFailure::Protocol && output_eof {
                child.cleanup_after_eof()
            } else {
                child.cleanup()
            };
            let observed_failure = child.natural_status.and_then(status_failure);
            let natural_termination = child.natural_status.map(|status| {
                let signal = status & 0x7f;
                if signal == 0 {
                    ChildTermination::Exited((status >> 8) & 0xff)
                } else {
                    ChildTermination::Signalled(signal)
                }
            });
            let mut outcome = batch_incomplete(
                prepared,
                receipts,
                results,
                cleanup
                    .err()
                    .unwrap_or_else(|| observed_failure.unwrap_or(reason)),
            );
            if let BatchOutcome::Incomplete { exchange, .. } = &mut outcome {
                *exchange = Some(ExchangeFailureContext {
                    boundary,
                    failure: reason,
                    natural_termination,
                    child_cpu_micros: child.natural_cpu_micros,
                    child_pid: child.pid,
                    child_exchange_ordinal: child.exchanges_started,
                    retained_session: retained,
                });
            }
            return outcome;
        }
        if status.and_then(status_failure).is_some() {
            return batch_incomplete(
                prepared,
                receipts,
                results,
                status.and_then(status_failure).unwrap(),
            );
        }
        let checkpoint = batch_checkpoint(&prepared, receipts.len(), results);
        BatchOutcome::Complete {
            receipts,
            checkpoint,
        }
    }

    struct Cursor<'a> {
        bytes: &'a [u8],
        offset: usize,
    }

    impl<'a> Cursor<'a> {
        fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
            let end = self
                .offset
                .checked_add(count)
                .filter(|end| *end <= self.bytes.len())
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "short request"))?;
            let part = &self.bytes[self.offset..end];
            self.offset = end;
            Ok(part)
        }

        fn bytes(&mut self, limit: usize) -> io::Result<&'a [u8]> {
            let len = u32::from_be_bytes(self.take(4)?.try_into().unwrap()) as usize;
            if len > limit {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "oversize field"));
            }
            self.take(len)
        }
    }

    pub(super) fn worker_once() -> io::Result<()> {
        if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut stdin = io::stdin();
        let mut magic = [0u8; 8];
        stdin.read_exact(&mut magic)?;
        if &magic == OPERATION_REQUEST_MAGIC {
            operation_worker_once(stdin, io::stdout(), magic)
        } else if &magic == DIAGNOSTIC_REQUEST_MAGIC {
            diagnostics_worker_once(stdin, io::stdout())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "worker protocol version",
            ))
        }
    }

    struct DiagnosticsParsedUnit<'a> {
        ordinal: u64,
        member_id: &'a str,
        relative_path: &'a str,
        root_uri: &'a str,
        raw: &'a [u8],
        input_mode: DiagnosticsUnitInputMode,
        unit_sha256: Digest256,
    }

    fn diagnostics_worker_once(mut input: impl Read, mut output: impl Write) -> io::Result<()> {
        use jsonschema::{Registry, Validator};

        let bad = || io::Error::new(io::ErrorKind::InvalidData, "schema diagnostics request");
        let mut tail = Vec::new();
        input
            .take((MAX_DIAGNOSTIC_REQUEST_BYTES as u64) + 1)
            .read_to_end(&mut tail)?;
        if tail.len() + 8 < DIAGNOSTIC_FIXED_REQUEST_BYTES
            || tail.len() + 8 > MAX_DIAGNOSTIC_REQUEST_BYTES
        {
            return Err(bad());
        }
        let mut cursor = Cursor {
            bytes: &tail,
            offset: 0,
        };
        let version = u16::from_be_bytes(cursor.take(2)?.try_into().unwrap());
        let caps = schema_diagnostics::Caps {
            max_issues_per_unit: u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()),
            max_report_bytes_per_unit: u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()),
            max_path_segments: u16::from_be_bytes(cursor.take(2)?.try_into().unwrap()),
            max_path_bytes: u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()),
        };
        let worker_sha256 = Digest256::from_bytes(cursor.take(32)?.try_into().unwrap());
        let expected_schema = Digest256::from_bytes(cursor.take(32)?.try_into().unwrap());
        let profile_byte = cursor.take(1)?[0];
        let (profile, input_profile, exceptional_remaining, selected_limits) = if profile_byte
            == DIAGNOSTIC_EXTENDED_INPUT_MARKER
        {
            let profile = parse_profile(cursor.take(1)?[0]).ok_or_else(bad)?;
            let input_profile = cursor.take(1)?[0];
            let (input_profile, exceptional_remaining, selected_limits) = match input_profile {
                DIAGNOSTIC_INPUT_LEGACY_PYTHON_OBSERVED => {
                    (DiagnosticsInputProfile::LegacyPythonObserved, None, None)
                }
                DIAGNOSTIC_INPUT_MIXED_SOURCE_FOUNDATION => {
                    let received = Digest256::from_bytes(cursor.take(32)?.try_into().unwrap());
                    if received != exceptional_schema::caps_sha256() {
                        return Err(bad());
                    }
                    let remaining = ExceptionalSchemaUsage::read_be(
                        cursor.take(DIAGNOSTIC_EXCEPTIONAL_COUNTERS_BYTES)?,
                    )
                    .filter(|remaining| remaining.fits_within(ExceptionalSchemaUsage::whole()))
                    .ok_or_else(bad)?;
                    (
                        DiagnosticsInputProfile::MixedSourceFoundation,
                        Some(remaining),
                        None,
                    )
                }
                DIAGNOSTIC_INPUT_FINITE_JSON_SELECTED => {
                    (DiagnosticsInputProfile::FiniteJsonSelected, None, None)
                }
                DIAGNOSTIC_INPUT_LEGACY_PYTHON_OBSERVED_SELECTED => {
                    let max_instance_bytes =
                        usize::try_from(u64::from_be_bytes(cursor.take(8)?.try_into().unwrap()))
                            .map_err(|_| bad())?;
                    let limits = LegacySelectedDiagnosticsLimits {
                        max_instance_bytes,
                        max_visits: u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()),
                        parser_state_bytes: u64::from_be_bytes(cursor.take(8)?.try_into().unwrap()),
                        conversion_state_bytes: u64::from_be_bytes(
                            cursor.take(8)?.try_into().unwrap(),
                        ),
                    };
                    limits.validate().map_err(|_| bad())?;
                    if profile != FormatProfile::LegacyPythonObserved20260923 {
                        return Err(bad());
                    }
                    (
                        DiagnosticsInputProfile::LegacyPythonObservedSelected,
                        None,
                        Some(limits),
                    )
                }
                _ => return Err(bad()),
            };
            if !matches!(
                input_profile,
                DiagnosticsInputProfile::LegacyPythonObserved
                    | DiagnosticsInputProfile::MixedSourceFoundation
                    | DiagnosticsInputProfile::FiniteJsonSelected
                    | DiagnosticsInputProfile::LegacyPythonObservedSelected
            ) {
                return Err(bad());
            }
            (
                profile,
                input_profile,
                exceptional_remaining,
                selected_limits,
            )
        } else {
            (
                parse_profile(profile_byte).ok_or_else(bad)?,
                DiagnosticsInputProfile::FiniteJson,
                None,
                None,
            )
        };
        if matches!(
            input_profile,
            DiagnosticsInputProfile::MixedSourceFoundation
        ) != exceptional_remaining.is_some()
        {
            return Err(bad());
        }
        if matches!(
            input_profile,
            DiagnosticsInputProfile::LegacyPythonObservedSelected
        ) != selected_limits.is_some()
        {
            return Err(bad());
        }
        if version != schema_diagnostics::PROTOCOL_VERSION || !caps.validate() {
            return Err(bad());
        }
        let resources = parse_batch_resources(&mut cursor)?;
        let (units, raw_total) =
            parse_diagnostics_units(&mut cursor, input_profile, selected_limits)?;
        if matches!(
            input_profile,
            DiagnosticsInputProfile::LegacyPythonObservedSelected
        ) && units.len() != 1
        {
            return Err(bad());
        }
        if cursor.offset != tail.len() || raw_total > MAX_BATCH_RAW_BYTES {
            return Err(bad());
        }
        let schema_set = schema_set_digest(&resources).map_err(|_| bad())?;
        if schema_set != expected_schema {
            return Err(bad());
        }
        let probe = crate::SchemaBackendProbe::prepare_diagnostics_resources(resources, profile)
            .map_err(|_| bad())?;
        if probe.schema_set_digest() != schema_set {
            return Err(bad());
        }
        let registry = Registry::new()
            .extend(
                probe
                    .resources
                    .iter()
                    .map(|(uri, value)| (uri.as_str(), value.clone())),
            )
            .map_err(|_| bad())?
            .prepare()
            .map_err(|_| bad())?;
        let exceptional_limits =
            exceptional_remaining.unwrap_or_else(ExceptionalSchemaUsage::whole);
        let mut exceptional_preparation_budget =
            exceptional_schema::PreparationBudget::new(exceptional_limits).map_err(|_| bad())?;
        let mut exceptional_plans = BTreeMap::new();

        let mut request_hash = Digest256Hasher::new();
        request_hash.update(DIAGNOSTIC_REQUEST_MAGIC);
        request_hash.update(&tail);
        let request_sha256 = request_hash.finalize();
        let caps_sha256 = caps.digest();
        let mut ack = Vec::with_capacity(DIAGNOSTIC_ACK_BYTES);
        ack.extend_from_slice(DIAGNOSTIC_ACK_MAGIC);
        ack.extend_from_slice(&version.to_be_bytes());
        ack.extend_from_slice(request_sha256.as_bytes());
        ack.extend_from_slice(worker_sha256.as_bytes());
        ack.extend_from_slice(schema_set.as_bytes());
        ack.extend_from_slice(caps_sha256.as_bytes());
        ack.extend_from_slice(&(units.len() as u32).to_be_bytes());
        output.write_all(&ack)?;
        output.flush()?;
        let mut validators = BTreeMap::<String, Validator>::new();
        let mut unsupported_roots = std::collections::BTreeSet::new();
        for unit in &units {
            if validators.contains_key(unit.root_uri) || unsupported_roots.contains(unit.root_uri) {
                continue;
            }
            // Framing and exact resource identity have already been ACKed.
            // Catch only the typed semantic refusal; malformed resources,
            // resolution and backend failures remain incomplete protocol runs.
            match crate::check_selected_schema_keywords(&registry, unit.root_uri, |bytes| {
                exceptional_preparation_budget
                    .charge_schema_scan()
                    .and_then(|()| exceptional_preparation_budget.charge_schema_bytes(bytes))
                    .map_err(|_| crate::SchemaProbeError::BudgetExceeded)
            }) {
                Ok(()) => {
                    validators.insert(
                        unit.root_uri.to_owned(),
                        compile_selected_validator(&probe, &registry, profile, unit.root_uri)?,
                    );
                }
                Err(crate::SchemaProbeError::UnknownKeyword(_)) => {
                    exceptional_preparation_budget
                        .charge_schema_scan()
                        .and_then(|()| {
                            exceptional_preparation_budget
                                .charge_schema_bytes(256 + unit.root_uri.len())
                        })
                        .map_err(|_| bad())?;
                    unsupported_roots.insert(unit.root_uri.to_owned());
                }
                Err(_) => return Err(bad()),
            }
        }
        let mut result_stream = Digest256Hasher::new();
        result_stream.update(b"tos-schema-diagnostics-results-v2\0");
        let mut response_bytes = ack.len();
        let mut exceptional_evaluation_context =
            exceptional_schema::EvaluationContext::new(exceptional_limits).map_err(|_| bad())?;
        for unit in &units {
            let report = match unit.input_mode {
                _ if unsupported_roots.contains(unit.root_uri) => diagnostic_input_report(
                    worker_sha256,
                    request_sha256,
                    unit.unit_sha256,
                    schema_set,
                    caps,
                    schema_diagnostics::Status::Indeterminate,
                    schema_diagnostics::Failure::UnsupportedInputSemantics,
                )?,
                DiagnosticsUnitInputMode::FiniteJson => match crate::published_value(
                    unit.raw,
                    crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
                ) {
                    Ok(value) => collect_diagnostic_report(
                        &validators[unit.root_uri],
                        &value,
                        worker_sha256,
                        request_sha256,
                        unit.unit_sha256,
                        schema_set,
                        caps,
                    )?,
                    Err(crate::SchemaProbeError::InvalidPublishedJson(_))
                    | Err(crate::SchemaProbeError::InvalidJson) => diagnostic_input_report(
                        worker_sha256,
                        request_sha256,
                        unit.unit_sha256,
                        schema_set,
                        caps,
                        schema_diagnostics::Status::InputRejected,
                        schema_diagnostics::Failure::InvalidJson,
                    )?,
                    Err(crate::SchemaProbeError::BudgetExceeded) => diagnostic_input_report(
                        worker_sha256,
                        request_sha256,
                        unit.unit_sha256,
                        schema_set,
                        caps,
                        schema_diagnostics::Status::InputRejected,
                        schema_diagnostics::Failure::InputBudget,
                    )?,
                    Err(_) => diagnostic_input_report(
                        worker_sha256,
                        request_sha256,
                        unit.unit_sha256,
                        schema_set,
                        caps,
                        schema_diagnostics::Status::Indeterminate,
                        schema_diagnostics::Failure::ValidatorRuntime,
                    )?,
                },
                DiagnosticsUnitInputMode::FiniteJsonSelected => {
                    match crate::published_value(unit.raw, MAX_BATCH_RAW_BYTES) {
                        Ok(value) => collect_diagnostic_report(
                            &validators[unit.root_uri],
                            &value,
                            worker_sha256,
                            request_sha256,
                            unit.unit_sha256,
                            schema_set,
                            caps,
                        )?,
                        Err(crate::SchemaProbeError::InvalidPublishedJson(_))
                        | Err(crate::SchemaProbeError::InvalidJson) => diagnostic_input_report(
                            worker_sha256,
                            request_sha256,
                            unit.unit_sha256,
                            schema_set,
                            caps,
                            schema_diagnostics::Status::InputRejected,
                            schema_diagnostics::Failure::InvalidJson,
                        )?,
                        Err(crate::SchemaProbeError::BudgetExceeded) => diagnostic_input_report(
                            worker_sha256,
                            request_sha256,
                            unit.unit_sha256,
                            schema_set,
                            caps,
                            schema_diagnostics::Status::InputRejected,
                            schema_diagnostics::Failure::InputBudget,
                        )?,
                        Err(_) => diagnostic_input_report(
                            worker_sha256,
                            request_sha256,
                            unit.unit_sha256,
                            schema_set,
                            caps,
                            schema_diagnostics::Status::Indeterminate,
                            schema_diagnostics::Failure::ValidatorRuntime,
                        )?,
                    }
                }
                DiagnosticsUnitInputMode::LegacyPythonObserved => {
                    match parse_legacy_python_observed_tree(unit.raw) {
                        Ok(value) => match legacy_json_value(&value) {
                            Some(finite_value) => collect_diagnostic_report(
                                &validators[unit.root_uri],
                                &finite_value,
                                worker_sha256,
                                request_sha256,
                                unit.unit_sha256,
                                schema_set,
                                caps,
                            )?,
                            None => {
                                if !exceptional_plans.contains_key(unit.root_uri) {
                                    exceptional_plans.insert(
                                        unit.root_uri.to_owned(),
                                        exceptional_schema::Plan::prepare(
                                            &probe.resources,
                                            unit.root_uri,
                                            &mut exceptional_preparation_budget,
                                        ),
                                    );
                                }
                                exceptional_diagnostic_report(
                                    exceptional_plans.get(unit.root_uri).ok_or_else(bad)?,
                                    &value,
                                    worker_sha256,
                                    request_sha256,
                                    unit.unit_sha256,
                                    schema_set,
                                    caps,
                                    &mut exceptional_evaluation_context,
                                )?
                            }
                        },
                        Err(LegacyInputFailure::InvalidJson) => diagnostic_input_report(
                            worker_sha256,
                            request_sha256,
                            unit.unit_sha256,
                            schema_set,
                            caps,
                            schema_diagnostics::Status::InputRejected,
                            schema_diagnostics::Failure::InvalidJson,
                        )?,
                        Err(LegacyInputFailure::InputBudget) => diagnostic_input_report(
                            worker_sha256,
                            request_sha256,
                            unit.unit_sha256,
                            schema_set,
                            caps,
                            schema_diagnostics::Status::InputRejected,
                            schema_diagnostics::Failure::InputBudget,
                        )?,
                    }
                }
                DiagnosticsUnitInputMode::LegacyPythonObservedSelected => {
                    let limits = selected_limits.ok_or_else(bad)?;
                    match parse_convert_selected_legacy(unit.raw, limits) {
                        Ok(value) => collect_diagnostic_report(
                            &validators[unit.root_uri],
                            &value,
                            worker_sha256,
                            request_sha256,
                            unit.unit_sha256,
                            schema_set,
                            caps,
                        )?,
                        Err(LegacySelectedFailure::InvalidJson) => diagnostic_input_report(
                            worker_sha256,
                            request_sha256,
                            unit.unit_sha256,
                            schema_set,
                            caps,
                            schema_diagnostics::Status::InputRejected,
                            schema_diagnostics::Failure::InvalidJson,
                        )?,
                        Err(LegacySelectedFailure::InputBudget) => diagnostic_input_report(
                            worker_sha256,
                            request_sha256,
                            unit.unit_sha256,
                            schema_set,
                            caps,
                            schema_diagnostics::Status::InputRejected,
                            schema_diagnostics::Failure::InputBudget,
                        )?,
                        Err(LegacySelectedFailure::UnsupportedInputSemantics) => {
                            diagnostic_input_report(
                                worker_sha256,
                                request_sha256,
                                unit.unit_sha256,
                                schema_set,
                                caps,
                                schema_diagnostics::Status::Indeterminate,
                                schema_diagnostics::Failure::UnsupportedInputSemantics,
                            )?
                        }
                    }
                }
            };
            let record =
                encode_diagnostic_unit(unit.ordinal, unit.unit_sha256, &report).ok_or_else(bad)?;
            response_bytes = response_bytes
                .checked_add(record.len())
                .filter(|bytes| *bytes <= schema_diagnostics::MAX_RESPONSE_BYTES)
                .ok_or_else(bad)?;
            update_diagnostic_result_stream(&mut result_stream, unit.unit_sha256, &report);
            output.write_all(&record)?;
            output.flush()?;
        }
        let result_sha256 = result_stream.finalize();
        let mut final_record = Vec::with_capacity(DIAGNOSTIC_FINAL_BYTES);
        final_record.extend_from_slice(DIAGNOSTIC_FINAL_MAGIC);
        final_record.extend_from_slice(&version.to_be_bytes());
        final_record.extend_from_slice(request_sha256.as_bytes());
        final_record.extend_from_slice(worker_sha256.as_bytes());
        final_record.extend_from_slice(schema_set.as_bytes());
        final_record.extend_from_slice(caps_sha256.as_bytes());
        final_record.extend_from_slice(&(units.len() as u32).to_be_bytes());
        final_record.extend_from_slice(result_sha256.as_bytes());
        if exceptional_remaining.is_some() {
            let preparation_usage = exceptional_preparation_budget.usage();
            let evaluation_usage = exceptional_evaluation_context.usage();
            ExceptionalSchemaUsage {
                schema_scan_work: preparation_usage.schema_scan_work,
                schema_scan_bytes: preparation_usage.schema_scan_bytes,
                pattern_compile_count: preparation_usage.pattern_compile_count,
                pattern_bytes: preparation_usage.pattern_bytes,
                evaluation_work: evaluation_usage.evaluation_work,
                evaluation_bytes: evaluation_usage.evaluation_bytes,
                reference_steps: evaluation_usage.reference_steps,
                regex_checks: evaluation_usage.regex_checks,
                regex_bytes: evaluation_usage.regex_bytes,
            }
            .write_be(&mut final_record);
        }
        response_bytes = response_bytes
            .checked_add(final_record.len())
            .filter(|bytes| *bytes <= schema_diagnostics::MAX_RESPONSE_BYTES)
            .ok_or_else(bad)?;
        output.write_all(&final_record)?;
        output.flush()?;
        Ok(())
    }

    #[derive(Debug)]
    enum LegacyInputFailure {
        InvalidJson,
        InputBudget,
    }

    fn parse_legacy_python_observed_tree(raw: &[u8]) -> Result<JsonValue, LegacyInputFailure> {
        let limits = JsonLimits::new(MAX_BATCH_RAW_BYTES, 64, 300_000, 4_300)
            .map_err(|_| LegacyInputFailure::InputBudget)?;
        let document =
            parse_json(raw, JsonMode::LegacyPythonObserved, limits).map_err(|error| {
                if error.code == FoundationErrorCode::BudgetExceeded {
                    LegacyInputFailure::InputBudget
                } else {
                    LegacyInputFailure::InvalidJson
                }
            })?;
        Ok(document.into_root())
    }

    fn legacy_json_value(value: &JsonValue) -> Option<serde_json::Value> {
        match value {
            JsonValue::Null => Some(serde_json::Value::Null),
            JsonValue::Bool(value) => Some(serde_json::Value::Bool(*value)),
            JsonValue::Number(number) => {
                let number = match number.kind {
                    JsonNumberKind::Int => number.lexeme.parse::<serde_json::Number>().ok()?,
                    JsonNumberKind::Float => {
                        let value = number.as_python_float()?;
                        if !value.is_finite() {
                            return None;
                        }
                        serde_json::Number::from_f64(value)?
                    }
                };
                Some(serde_json::Value::Number(number))
            }
            JsonValue::String(value) => Some(serde_json::Value::String(value.as_str()?.to_owned())),
            JsonValue::Array(items) => items
                .iter()
                .map(legacy_json_value)
                .collect::<Option<Vec<_>>>()
                .map(serde_json::Value::Array),
            JsonValue::Object(entries) => {
                let mut object = serde_json::Map::new();
                for (key, value) in entries {
                    object.insert(key.as_str()?.to_owned(), legacy_json_value(value)?);
                }
                Some(serde_json::Value::Object(object))
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum LegacySelectedFailure {
        InvalidJson,
        InputBudget,
        UnsupportedInputSemantics,
    }

    #[derive(Default)]
    struct LegacySelectedConversionShape {
        values: usize,
        object_entries: usize,
        array_elements: usize,
        cloned_text_bytes: usize,
        number_storage_bytes: usize,
    }

    impl LegacySelectedConversionShape {
        fn total_state_bytes(&self) -> Result<usize, LegacySelectedFailure> {
            let map_nodes = self
                .object_entries
                .checked_mul(768)
                .ok_or(LegacySelectedFailure::InputBudget)?;
            let value_slots = self
                .values
                .checked_mul(std::mem::size_of::<serde_json::Value>())
                .ok_or(LegacySelectedFailure::InputBudget)?;
            let frame_bytes = std::mem::size_of::<JsonValue>()
                .checked_add(std::mem::size_of::<serde_json::Value>())
                .and_then(|bytes| bytes.checked_add(64))
                .ok_or(LegacySelectedFailure::InputBudget)?;
            let conversion_stack = (LegacySelectedDiagnosticsLimits::MAX_DEPTH + 1)
                .checked_mul(frame_bytes)
                .ok_or(LegacySelectedFailure::InputBudget)?;
            [
                map_nodes,
                self.cloned_text_bytes,
                self.number_storage_bytes,
                value_slots,
                conversion_stack,
            ]
            .into_iter()
            .try_fold(0usize, usize::checked_add)
            .ok_or(LegacySelectedFailure::InputBudget)
        }
    }

    fn legacy_selected_conversion_shape(
        value: &JsonValue,
        depth: usize,
        shape: &mut LegacySelectedConversionShape,
    ) -> Result<(), LegacySelectedFailure> {
        if depth > LegacySelectedDiagnosticsLimits::MAX_DEPTH {
            return Err(LegacySelectedFailure::InputBudget);
        }
        shape.values = shape
            .values
            .checked_add(1)
            .ok_or(LegacySelectedFailure::InputBudget)?;
        match value {
            JsonValue::Null | JsonValue::Bool(_) => {}
            JsonValue::Number(number) => {
                // Integer Numbers may retain their arbitrary-precision lexeme;
                // float conversion has a bounded shortest representation.
                let bytes = number
                    .lexeme
                    .len()
                    .checked_add(if number.kind == JsonNumberKind::Float {
                        32
                    } else {
                        0
                    })
                    .ok_or(LegacySelectedFailure::InputBudget)?;
                shape.number_storage_bytes = shape
                    .number_storage_bytes
                    .checked_add(bytes)
                    .ok_or(LegacySelectedFailure::InputBudget)?;
            }
            JsonValue::String(string) => {
                let value = string
                    .as_str()
                    .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
                shape.cloned_text_bytes = shape
                    .cloned_text_bytes
                    .checked_add(value.len())
                    .ok_or(LegacySelectedFailure::InputBudget)?;
            }
            JsonValue::Array(items) => {
                shape.array_elements = shape
                    .array_elements
                    .checked_add(items.len())
                    .ok_or(LegacySelectedFailure::InputBudget)?;
                for item in items {
                    legacy_selected_conversion_shape(item, depth + 1, shape)?;
                }
            }
            JsonValue::Object(entries) => {
                shape.object_entries = shape
                    .object_entries
                    .checked_add(entries.len())
                    .ok_or(LegacySelectedFailure::InputBudget)?;
                for (key, item) in entries {
                    let key = key
                        .as_str()
                        .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
                    shape.cloned_text_bytes = shape
                        .cloned_text_bytes
                        .checked_add(key.len())
                        .ok_or(LegacySelectedFailure::InputBudget)?;
                    legacy_selected_conversion_shape(item, depth + 1, shape)?;
                }
            }
        }
        Ok(())
    }

    struct LegacySelectedConversionMeter {
        max_bytes: u64,
        shape: LegacySelectedConversionShape,
        used_bytes: u64,
        values_seen: usize,
        entries_charged: usize,
        arrays_reserved: usize,
        text_bytes_charged: usize,
        number_bytes_charged: usize,
    }

    impl LegacySelectedConversionMeter {
        fn new(
            shape: LegacySelectedConversionShape,
            max_bytes: u64,
        ) -> Result<Self, LegacySelectedFailure> {
            let value_slots = shape
                .values
                .checked_mul(std::mem::size_of::<serde_json::Value>())
                .ok_or(LegacySelectedFailure::InputBudget)?;
            let frame_bytes = std::mem::size_of::<JsonValue>()
                .checked_add(std::mem::size_of::<serde_json::Value>())
                .and_then(|bytes| bytes.checked_add(64))
                .ok_or(LegacySelectedFailure::InputBudget)?;
            let stack_bytes = (LegacySelectedDiagnosticsLimits::MAX_DEPTH + 1)
                .checked_mul(frame_bytes)
                .ok_or(LegacySelectedFailure::InputBudget)?;
            let reserved = u64::try_from(
                value_slots
                    .checked_add(stack_bytes)
                    .ok_or(LegacySelectedFailure::InputBudget)?,
            )
            .map_err(|_| LegacySelectedFailure::InputBudget)?;
            if reserved > max_bytes {
                return Err(LegacySelectedFailure::InputBudget);
            }
            Ok(Self {
                max_bytes,
                shape,
                used_bytes: reserved,
                values_seen: 0,
                entries_charged: 0,
                arrays_reserved: 0,
                text_bytes_charged: 0,
                number_bytes_charged: 0,
            })
        }

        fn charge(&mut self, bytes: usize) -> Result<(), LegacySelectedFailure> {
            self.used_bytes = self
                .used_bytes
                .checked_add(u64::try_from(bytes).map_err(|_| LegacySelectedFailure::InputBudget)?)
                .filter(|used| *used <= self.max_bytes)
                .ok_or(LegacySelectedFailure::InputBudget)?;
            Ok(())
        }

        fn visit_value(&mut self) -> Result<(), LegacySelectedFailure> {
            self.values_seen = self
                .values_seen
                .checked_add(1)
                .filter(|seen| *seen <= self.shape.values)
                .ok_or(LegacySelectedFailure::InputBudget)?;
            Ok(())
        }

        fn charge_text(&mut self, bytes: usize) -> Result<(), LegacySelectedFailure> {
            self.text_bytes_charged = self
                .text_bytes_charged
                .checked_add(bytes)
                .filter(|charged| *charged <= self.shape.cloned_text_bytes)
                .ok_or(LegacySelectedFailure::InputBudget)?;
            self.charge(bytes)
        }

        fn charge_number(
            &mut self,
            number: &tos_foundation::JsonNumber,
        ) -> Result<(), LegacySelectedFailure> {
            let bytes = number
                .lexeme
                .len()
                .checked_add(if number.kind == JsonNumberKind::Float {
                    32
                } else {
                    0
                })
                .ok_or(LegacySelectedFailure::InputBudget)?;
            self.number_bytes_charged = self
                .number_bytes_charged
                .checked_add(bytes)
                .filter(|charged| *charged <= self.shape.number_storage_bytes)
                .ok_or(LegacySelectedFailure::InputBudget)?;
            self.charge(bytes)
        }

        fn before_array_reserve(&mut self, elements: usize) -> Result<(), LegacySelectedFailure> {
            self.arrays_reserved = self
                .arrays_reserved
                .checked_add(elements)
                .filter(|reserved| *reserved <= self.shape.array_elements)
                .ok_or(LegacySelectedFailure::InputBudget)?;
            Ok(())
        }

        fn charge_map_entry(&mut self) -> Result<(), LegacySelectedFailure> {
            self.entries_charged = self
                .entries_charged
                .checked_add(1)
                .filter(|charged| *charged <= self.shape.object_entries)
                .ok_or(LegacySelectedFailure::InputBudget)?;
            self.charge(768)
        }

        fn finish(&self) -> Result<(), LegacySelectedFailure> {
            if self.values_seen != self.shape.values
                || self.entries_charged != self.shape.object_entries
                || self.arrays_reserved != self.shape.array_elements
                || self.text_bytes_charged != self.shape.cloned_text_bytes
                || self.number_bytes_charged != self.shape.number_storage_bytes
            {
                return Err(LegacySelectedFailure::InputBudget);
            }
            let planned = self.shape.total_state_bytes()?;
            if u64::try_from(planned).map_err(|_| LegacySelectedFailure::InputBudget)?
                != self.used_bytes
            {
                return Err(LegacySelectedFailure::InputBudget);
            }
            Ok(())
        }
    }

    fn clone_legacy_selected_string(
        value: &str,
        meter: &mut LegacySelectedConversionMeter,
    ) -> Result<String, LegacySelectedFailure> {
        meter.charge_text(value.len())?;
        let mut cloned = String::new();
        cloned
            .try_reserve_exact(value.len())
            .map_err(|_| LegacySelectedFailure::InputBudget)?;
        cloned.push_str(value);
        Ok(cloned)
    }

    fn convert_legacy_selected_value(
        value: &JsonValue,
        meter: &mut LegacySelectedConversionMeter,
    ) -> Result<serde_json::Value, LegacySelectedFailure> {
        meter.visit_value()?;
        match value {
            JsonValue::Null => Ok(serde_json::Value::Null),
            JsonValue::Bool(value) => Ok(serde_json::Value::Bool(*value)),
            JsonValue::Number(number) => {
                meter.charge_number(number)?;
                let number = match number.kind {
                    JsonNumberKind::Int => number
                        .lexeme
                        .parse::<serde_json::Number>()
                        .map_err(|_| LegacySelectedFailure::UnsupportedInputSemantics)?,
                    JsonNumberKind::Float => {
                        let value = number
                            .as_python_float()
                            .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
                        if !value.is_finite() {
                            return Err(LegacySelectedFailure::UnsupportedInputSemantics);
                        }
                        serde_json::Number::from_f64(value)
                            .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?
                    }
                };
                Ok(serde_json::Value::Number(number))
            }
            JsonValue::String(value) => {
                let value = value
                    .as_str()
                    .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
                Ok(serde_json::Value::String(clone_legacy_selected_string(
                    value, meter,
                )?))
            }
            JsonValue::Array(items) => {
                // The complete tree's `values * size_of::<Value>()` slots are
                // prepaid by the meter before conversion. Reserve exactly
                // this array's share before visiting its children, so Vec
                // growth cannot exceed that already-admitted slot budget.
                meter.before_array_reserve(items.len())?;
                let mut converted = Vec::new();
                converted
                    .try_reserve_exact(items.len())
                    .map_err(|_| LegacySelectedFailure::InputBudget)?;
                for item in items {
                    converted.push(convert_legacy_selected_value(item, meter)?);
                }
                Ok(serde_json::Value::Array(converted))
            }
            JsonValue::Object(entries) => {
                let mut converted = serde_json::Map::new();
                for (key, item) in entries {
                    let key = key
                        .as_str()
                        .ok_or(LegacySelectedFailure::UnsupportedInputSemantics)?;
                    let key = clone_legacy_selected_string(key, meter)?;
                    let item = convert_legacy_selected_value(item, meter)?;
                    meter.charge_map_entry()?;
                    converted.insert(key, item);
                }
                Ok(serde_json::Value::Object(converted))
            }
        }
    }

    fn parse_convert_selected_legacy(
        raw: &[u8],
        limits: LegacySelectedDiagnosticsLimits,
    ) -> Result<serde_json::Value, LegacySelectedFailure> {
        if raw.len() > limits.max_instance_bytes {
            return Err(LegacySelectedFailure::InputBudget);
        }
        let parser_state_bytes = usize::try_from(limits.parser_state_bytes)
            .map_err(|_| LegacySelectedFailure::InputBudget)?;
        let json_limits = JsonLimits::new(
            limits.max_instance_bytes,
            LegacySelectedDiagnosticsLimits::MAX_DEPTH,
            limits.max_visits as usize,
            LegacySelectedDiagnosticsLimits::MAX_INTEGER_DIGITS,
        )
        .map_err(|_| LegacySelectedFailure::InputBudget)?;
        let document = parse_json_with_state_budget(
            raw,
            JsonMode::LegacyPythonObserved,
            json_limits,
            parser_state_bytes,
        )
        .map_err(|error| {
            if error.code == FoundationErrorCode::BudgetExceeded {
                LegacySelectedFailure::InputBudget
            } else {
                LegacySelectedFailure::InvalidJson
            }
        })?;
        let value = document.into_root();
        let mut shape = LegacySelectedConversionShape::default();
        legacy_selected_conversion_shape(&value, 0, &mut shape)?;
        let conversion_state_bytes = shape.total_state_bytes()?;
        if u64::try_from(conversion_state_bytes).map_err(|_| LegacySelectedFailure::InputBudget)?
            > limits.conversion_state_bytes
        {
            return Err(LegacySelectedFailure::InputBudget);
        }
        let mut meter = LegacySelectedConversionMeter::new(shape, limits.conversion_state_bytes)?;
        let converted = convert_legacy_selected_value(&value, &mut meter)?;
        meter.finish()?;
        Ok(converted)
    }

    fn parse_diagnostics_units<'a>(
        cursor: &mut Cursor<'a>,
        input_profile: DiagnosticsInputProfile,
        selected_limits: Option<LegacySelectedDiagnosticsLimits>,
    ) -> io::Result<(Vec<DiagnosticsParsedUnit<'a>>, usize)> {
        let unit_count = u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()) as usize;
        if unit_count == 0 || unit_count > MAX_BATCH_UNITS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "diagnostic unit count",
            ));
        }
        let mut units = Vec::with_capacity(unit_count);
        let mut raw_total = 0usize;
        for ordinal in 0..unit_count {
            let start = cursor.offset;
            let observed_ordinal = u64::from_be_bytes(cursor.take(8)?.try_into().unwrap());
            let (input_mode, payload_start) = match input_profile {
                DiagnosticsInputProfile::FiniteJson => {
                    (DiagnosticsUnitInputMode::FiniteJson, cursor.offset)
                }
                DiagnosticsInputProfile::LegacyPythonObserved => (
                    DiagnosticsUnitInputMode::LegacyPythonObserved,
                    cursor.offset,
                ),
                DiagnosticsInputProfile::FiniteJsonSelected => {
                    (DiagnosticsUnitInputMode::FiniteJsonSelected, cursor.offset)
                }
                DiagnosticsInputProfile::LegacyPythonObservedSelected => (
                    DiagnosticsUnitInputMode::LegacyPythonObservedSelected,
                    cursor.offset,
                ),
                DiagnosticsInputProfile::MixedSourceFoundation => {
                    let mode = match cursor.take(1)?[0] {
                        1 => DiagnosticsUnitInputMode::FiniteJson,
                        2 => DiagnosticsUnitInputMode::LegacyPythonObserved,
                        3 => DiagnosticsUnitInputMode::FiniteJsonSelected,
                        _ => {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "diagnostic input mode",
                            ));
                        }
                    };
                    (mode, cursor.offset)
                }
            };
            let member_id = std::str::from_utf8(cursor.bytes(MAX_MEMBER_ID_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "diagnostic member id"))?;
            let relative_path =
                std::str::from_utf8(cursor.bytes(MAX_PATH_BYTES)?).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "diagnostic member path")
                })?;
            let root_uri = std::str::from_utf8(cursor.bytes(MAX_URI_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "diagnostic root uri"))?;
            let unit_raw_limit = match input_mode {
                DiagnosticsUnitInputMode::FiniteJson => {
                    crate::SchemaBackendProbe::MAX_INSTANCE_BYTES
                }
                DiagnosticsUnitInputMode::LegacyPythonObserved
                | DiagnosticsUnitInputMode::FiniteJsonSelected => MAX_BATCH_RAW_BYTES,
                DiagnosticsUnitInputMode::LegacyPythonObservedSelected => {
                    selected_limits
                        .ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "selected Legacy limits")
                        })?
                        .max_instance_bytes
                }
            };
            let raw = cursor.bytes(unit_raw_limit)?;
            raw_total = raw_total
                .checked_add(raw.len())
                .filter(|total| *total <= MAX_BATCH_RAW_BYTES)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "diagnostic raw bytes")
                })?;
            if observed_ordinal != ordinal as u64
                || member_id.is_empty()
                || relative_path.is_empty()
                || relative_path.starts_with('/')
                || relative_path.split('/').any(|part| part == "..")
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "diagnostic unit identity",
                ));
            }
            let unit_sha256 =
                if input_mode == DiagnosticsUnitInputMode::LegacyPythonObservedSelected {
                    selected_legacy_diagnostics_unit_digest_fields(
                        observed_ordinal,
                        member_id,
                        relative_path,
                        root_uri,
                        raw,
                        selected_limits.ok_or_else(|| {
                            io::Error::new(io::ErrorKind::InvalidData, "selected Legacy limits")
                        })?,
                    )
                } else {
                    let mut digest = Digest256Hasher::new();
                    digest.update(b"tos-val2-batch-unit-v1\0");
                    // Preserve the historical mixed-mode digest: its mode byte is
                    // request-bound but deliberately absent from unit identity.
                    digest.update(&cursor.bytes[start..start + 8]);
                    digest.update(&cursor.bytes[payload_start..cursor.offset]);
                    digest.finalize()
                };
            units.push(DiagnosticsParsedUnit {
                ordinal: observed_ordinal,
                member_id,
                relative_path,
                root_uri,
                raw,
                input_mode,
                unit_sha256,
            });
        }
        Ok((units, raw_total))
    }

    fn exceptional_diagnostic_report(
        plan: &Result<exceptional_schema::Plan<'_>, exceptional_schema::PrepareFailure>,
        instance: &JsonValue,
        worker_sha256: Digest256,
        request_sha256: Digest256,
        unit_sha256: Digest256,
        schema_set: Digest256,
        caps: schema_diagnostics::Caps,
        context: &mut exceptional_schema::EvaluationContext,
    ) -> io::Result<schema_diagnostics::Report> {
        let plan = match plan {
            Ok(plan) => plan,
            Err(exceptional_schema::PrepareFailure::Unsupported) => {
                return diagnostic_input_report(
                    worker_sha256,
                    request_sha256,
                    unit_sha256,
                    schema_set,
                    caps,
                    schema_diagnostics::Status::Indeterminate,
                    schema_diagnostics::Failure::UnsupportedInputSemantics,
                );
            }
            Err(exceptional_schema::PrepareFailure::Budget) => {
                return diagnostic_input_report(
                    worker_sha256,
                    request_sha256,
                    unit_sha256,
                    schema_set,
                    caps,
                    schema_diagnostics::Status::Indeterminate,
                    schema_diagnostics::Failure::ValidatorRuntime,
                );
            }
        };
        let evaluation = plan.evaluate(instance, caps, context);
        let (status, failure) = match evaluation.failure {
            Some(exceptional_schema::EvaluationFailure::Unsupported) => (
                schema_diagnostics::Status::Indeterminate,
                schema_diagnostics::Failure::UnsupportedInputSemantics,
            ),
            Some(exceptional_schema::EvaluationFailure::Budget) => (
                schema_diagnostics::Status::Indeterminate,
                schema_diagnostics::Failure::ValidatorRuntime,
            ),
            None if evaluation.truncated => (
                schema_diagnostics::Status::Truncated,
                schema_diagnostics::Failure::None,
            ),
            None if evaluation.total_issue_count == 0 => (
                schema_diagnostics::Status::Valid,
                schema_diagnostics::Failure::None,
            ),
            None => (
                schema_diagnostics::Status::Invalid,
                schema_diagnostics::Failure::None,
            ),
        };
        make_diagnostic_report(
            worker_sha256,
            request_sha256,
            unit_sha256,
            schema_set,
            caps,
            status,
            failure,
            evaluation.total_issue_count,
            evaluation.truncated,
            evaluation.issues,
        )
    }

    fn collect_diagnostic_report(
        validator: &jsonschema::Validator,
        instance: &serde_json::Value,
        worker_sha256: Digest256,
        request_sha256: Digest256,
        unit_sha256: Digest256,
        schema_set: Digest256,
        caps: schema_diagnostics::Caps,
    ) -> io::Result<schema_diagnostics::Report> {
        use std::collections::BinaryHeap;

        let bad = || io::Error::new(io::ErrorKind::InvalidData, "diagnostic report bound");
        let mut retained = BinaryHeap::<schema_diagnostics::Issue>::new();
        let mut total = 0u64;
        let mut truncated = false;
        let mut indeterminate = false;
        for error in validator.iter_errors(instance) {
            total = total.saturating_add(1);
            let Some((issue, evaluation_failure)) =
                schema_diagnostics::issue_from_validation_error(&error, caps)
            else {
                truncated = true;
                continue;
            };
            indeterminate |= evaluation_failure;
            if retained.len() < caps.max_issues_per_unit as usize {
                retained.push(issue);
            } else {
                truncated = true;
                if retained
                    .peek()
                    .is_some_and(|largest| issue.cmp(largest) == std::cmp::Ordering::Less)
                {
                    let _ = retained.pop();
                    retained.push(issue);
                }
            }
        }
        let mut issues = retained.into_vec();
        issues.sort();
        let mut kept = Vec::with_capacity(issues.len());
        let mut report_bytes = DIAGNOSTIC_UNIT_HEADER_BYTES;
        for issue in issues {
            let payload = schema_diagnostics::issue_payload(&issue).ok_or_else(bad)?;
            let next = report_bytes.checked_add(payload.len()).ok_or_else(bad)?;
            if next > caps.max_report_bytes_per_unit as usize {
                truncated = true;
                break;
            }
            report_bytes = next;
            kept.push(issue);
        }
        let status = if indeterminate {
            schema_diagnostics::Status::Indeterminate
        } else if truncated {
            schema_diagnostics::Status::Truncated
        } else if total == 0 {
            schema_diagnostics::Status::Valid
        } else {
            schema_diagnostics::Status::Invalid
        };
        let failure = if indeterminate {
            schema_diagnostics::Failure::ValidatorRuntime
        } else {
            schema_diagnostics::Failure::None
        };
        make_diagnostic_report(
            worker_sha256,
            request_sha256,
            unit_sha256,
            schema_set,
            caps,
            status,
            failure,
            total,
            truncated,
            kept,
        )
    }

    fn diagnostic_input_report(
        worker_sha256: Digest256,
        request_sha256: Digest256,
        unit_sha256: Digest256,
        schema_set: Digest256,
        caps: schema_diagnostics::Caps,
        status: schema_diagnostics::Status,
        failure: schema_diagnostics::Failure,
    ) -> io::Result<schema_diagnostics::Report> {
        make_diagnostic_report(
            worker_sha256,
            request_sha256,
            unit_sha256,
            schema_set,
            caps,
            status,
            failure,
            0,
            false,
            Vec::new(),
        )
    }

    fn make_diagnostic_report(
        worker_sha256: Digest256,
        request_sha256: Digest256,
        unit_sha256: Digest256,
        schema_set: Digest256,
        caps: schema_diagnostics::Caps,
        status: schema_diagnostics::Status,
        failure: schema_diagnostics::Failure,
        total_issue_count: u64,
        truncated: bool,
        issues: Vec<schema_diagnostics::Issue>,
    ) -> io::Result<schema_diagnostics::Report> {
        if !schema_diagnostics::status_is_well_formed(
            status,
            failure,
            total_issue_count,
            truncated,
            issues.len(),
        ) || issues.windows(2).any(|pair| pair[0] > pair[1])
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "diagnostic report state",
            ));
        }
        let issues_sha256 = schema_diagnostics::issues_digest(&issues)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "diagnostic issue digest"))?;
        let report_sha256 = schema_diagnostics::report_digest(
            worker_sha256,
            request_sha256,
            unit_sha256,
            schema_set,
            caps,
            status,
            failure,
            total_issue_count,
            truncated,
            issues_sha256,
        );
        Ok(schema_diagnostics::Report {
            protocol_version: schema_diagnostics::PROTOCOL_VERSION,
            worker_sha256,
            request_sha256,
            unit_sha256,
            schema_set_sha256: schema_set,
            caps,
            status,
            failure,
            total_issue_count,
            truncated,
            issues_sha256,
            report_sha256,
            issues,
        })
    }

    fn encode_diagnostic_unit(
        ordinal: u64,
        unit_sha256: Digest256,
        report: &schema_diagnostics::Report,
    ) -> Option<Vec<u8>> {
        let mut payload = Vec::new();
        for issue in &report.issues {
            payload.extend_from_slice(&schema_diagnostics::issue_payload(issue)?);
        }
        let mut unit = Vec::with_capacity(DIAGNOSTIC_UNIT_HEADER_BYTES + payload.len());
        unit.extend_from_slice(DIAGNOSTIC_UNIT_MAGIC);
        unit.extend_from_slice(&report.protocol_version.to_be_bytes());
        unit.extend_from_slice(&ordinal.to_be_bytes());
        unit.extend_from_slice(unit_sha256.as_bytes());
        unit.push(report.status as u8);
        unit.push(report.failure as u8);
        unit.extend_from_slice(&report.total_issue_count.to_be_bytes());
        unit.push(u8::from(report.truncated));
        unit.extend_from_slice(&(report.issues.len() as u32).to_be_bytes());
        unit.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        unit.extend_from_slice(report.issues_sha256.as_bytes());
        unit.extend_from_slice(&payload);
        (unit.len() <= report.caps.max_report_bytes_per_unit as usize).then_some(unit)
    }

    fn update_diagnostic_result_stream(
        hash: &mut Digest256Hasher,
        unit_sha256: Digest256,
        report: &schema_diagnostics::Report,
    ) {
        hash.update(unit_sha256.as_bytes());
        hash.update(&[
            report.status as u8,
            report.failure as u8,
            u8::from(report.truncated),
        ]);
        hash.update(&report.total_issue_count.to_be_bytes());
        hash.update(&(report.issues.len() as u32).to_be_bytes());
        hash.update(report.report_sha256.as_bytes());
    }

    struct BatchParsedUnit<'a> {
        ordinal: u64,
        root_uri: &'a str,
        raw: &'a [u8],
        unit_sha256: Digest256,
    }

    fn parse_batch_resources(cursor: &mut Cursor<'_>) -> io::Result<Vec<SchemaResource>> {
        let resource_count = u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()) as usize;
        if resource_count > crate::SchemaBackendProbe::MAX_RESOURCES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "batch resources",
            ));
        }
        let mut resource_bytes = 0usize;
        let mut resources = Vec::with_capacity(resource_count);
        for _ in 0..resource_count {
            let uri = std::str::from_utf8(cursor.bytes(MAX_URI_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "schema uri"))?;
            let raw = cursor.bytes(crate::SchemaBackendProbe::MAX_RESOURCE_BYTES)?;
            resource_bytes = resource_bytes
                .checked_add(raw.len())
                .filter(|total| *total <= crate::SchemaBackendProbe::MAX_TOTAL_BYTES)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "schema bytes"))?;
            resources.push(SchemaResource {
                uri: uri.to_owned(),
                raw: raw.to_vec(),
            });
        }
        Ok(resources)
    }
    fn parse_batch_units<'a>(
        cursor: &mut Cursor<'a>,
    ) -> io::Result<(Vec<BatchParsedUnit<'a>>, usize)> {
        let unit_count = u32::from_be_bytes(cursor.take(4)?.try_into().unwrap()) as usize;
        if unit_count == 0 || unit_count > MAX_BATCH_UNITS {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "unit count"));
        }
        let mut units = Vec::with_capacity(unit_count);
        let mut raw_total = 0usize;
        for ordinal in 0..unit_count {
            let start = cursor.offset;
            let observed_ordinal = u64::from_be_bytes(cursor.take(8)?.try_into().unwrap());
            let member = std::str::from_utf8(cursor.bytes(MAX_MEMBER_ID_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "member id"))?;
            let path = std::str::from_utf8(cursor.bytes(MAX_PATH_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "member path"))?;
            let root_uri = std::str::from_utf8(cursor.bytes(MAX_URI_BYTES)?)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "root uri"))?;
            let raw = cursor.bytes(crate::SchemaBackendProbe::MAX_INSTANCE_BYTES)?;
            raw_total = raw_total
                .checked_add(raw.len())
                .filter(|total| *total <= MAX_BATCH_RAW_BYTES)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "batch raw bytes"))?;
            if observed_ordinal != ordinal as u64
                || member.is_empty()
                || path.is_empty()
                || path.starts_with('/')
                || path.split('/').any(|part| part == "..")
            {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "unit identity"));
            }
            let mut digest = Digest256Hasher::new();
            digest.update(b"tos-val2-batch-unit-v1\0");
            digest.update(&cursor.bytes[start..cursor.offset]);
            units.push(BatchParsedUnit {
                ordinal: observed_ordinal,
                root_uri,
                raw,
                unit_sha256: digest.finalize(),
            });
        }
        Ok((units, raw_total))
    }

    fn read_operation_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
        let mut length = [0u8; 4];
        if reader.read(&mut length[..1])? == 0 {
            return Ok(None);
        }
        reader.read_exact(&mut length[1..])?;
        let count = u32::from_be_bytes(length) as usize;
        if count < 8 || count > MAX_BATCH_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "operation frame length",
            ));
        }
        let mut frame = vec![0u8; count];
        reader.read_exact(&mut frame)?;
        Ok(Some(frame))
    }

    fn operation_worker_once(
        mut input: impl Read,
        mut output: impl Write,
        magic: [u8; 8],
    ) -> io::Result<()> {
        use jsonschema::{Registry, Validator};
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "operation identity or budget");
        let mut header = vec![0u8; OPERATION_HEADER_BYTES];
        header[..8].copy_from_slice(&magic);
        input.read_exact(&mut header[8..])?;
        let mut h = Cursor {
            bytes: &header,
            offset: 24,
        };
        let expected_schema = h.take(32)?.to_vec();
        let profile = parse_profile(h.take(1)?[0]).ok_or_else(bad)?;
        let mut number =
            || -> io::Result<u64> { Ok(u64::from_be_bytes(h.take(8)?.try_into().unwrap())) };
        let max_frames = number()?;
        let max_units = number()?;
        let max_raw = number()?;
        let max_wire = number()?;
        let max_selectors = usize::try_from(number()?).map_err(|_| bad())?;
        let cpu = number()?;
        let memory = number()?;
        let wall_nanos = number()?;
        if max_frames == 0
            || max_units == 0
            || max_raw == 0
            || max_wire == 0
            || max_selectors == 0
            || cpu == 0
            || cpu > 3600
            || memory < 64 * 1024 * 1024
            || memory > 8 * 1024 * 1024 * 1024
            || wall_nanos == 0
            || wall_nanos > Duration::from_secs(3600).as_nanos() as u64
        {
            return Err(bad());
        }
        let start = Instant::now();
        let mut frame = read_operation_frame(&mut input)?.ok_or_else(bad)?;
        let mut first = Cursor {
            bytes: &frame,
            offset: 8,
        };
        if first.take(8)? != BATCH_REQUEST_MAGIC {
            return Err(bad());
        }
        first.take(16)?;
        if parse_profile(first.take(1)?[0]) != Some(profile) {
            return Err(bad());
        }
        if (header.len()
            + frame.len()
            + 4
            + BATCH_ACK_BYTES
            + OPERATION_END_BYTES
            + 4
            + 48
            + OPERATION_FINAL_BYTES) as u64
            > max_wire
        {
            return Err(bad());
        }
        let resources = parse_batch_resources(&mut first)?;
        let first_units_offset = first.offset;
        let schema_set = schema_set_digest(&resources).map_err(|_| bad())?;
        if expected_schema.as_slice() != schema_set.as_bytes() {
            return Err(bad());
        }
        let probe = crate::SchemaBackendProbe::new(resources, profile).map_err(|_| bad())?;
        if probe.schema_set_digest() != schema_set {
            return Err(bad());
        }
        let registry = Registry::new()
            .extend(
                probe
                    .resources
                    .iter()
                    .map(|(uri, value)| (uri.as_str(), value.clone())),
            )
            .map_err(|_| bad())?
            .prepare()
            .map_err(|_| bad())?;
        let mut validators = BTreeMap::<String, Validator>::new();
        let mut total_units = 0u64;
        let mut total_raw = 0u64;
        let mut wire = header.len() as u64;
        let mut sequence = 0u64;
        loop {
            if start.elapsed().as_nanos() >= wall_nanos as u128 {
                return Err(bad());
            }
            if frame.len() == 48 && &frame[8..16] == OPERATION_CLOSE_MAGIC {
                let header_sha = Digest256::of_bytes(&header);
                if u64::from_be_bytes(frame[..8].try_into().unwrap()) != sequence
                    || &frame[16..48] != header_sha.as_bytes()
                {
                    return Err(bad());
                }
                wire = wire
                    .checked_add((frame.len() + 4 + OPERATION_FINAL_BYTES) as u64)
                    .filter(|n| *n <= max_wire)
                    .ok_or_else(bad)?;
                let mut request = Digest256Hasher::new();
                request.update(&header);
                request.update(&frame);
                let mut final_ack = Vec::with_capacity(OPERATION_FINAL_BYTES);
                final_ack.extend_from_slice(OPERATION_FINAL_MAGIC);
                final_ack.extend_from_slice(header_sha.as_bytes());
                final_ack.extend_from_slice(request.finalize().as_bytes());
                final_ack.extend_from_slice(&sequence.to_be_bytes());
                output.write_all(&final_ack)?;
                output.flush()?;
                return Ok(());
            }

            if sequence >= max_frames || start.elapsed().as_nanos() >= wall_nanos as u128 {
                return Err(bad());
            }
            wire = wire
                .checked_add(frame.len() as u64 + 4)
                .filter(|n| *n <= max_wire)
                .ok_or_else(bad)?;
            let mut cursor = Cursor {
                bytes: &frame,
                offset: 0,
            };
            if u64::from_be_bytes(cursor.take(8)?.try_into().unwrap()) != sequence {
                return Err(bad());
            }
            if sequence == 0 {
                cursor.offset = first_units_offset;
            } else {
                if cursor.take(8)? != BATCH_REQUEST_MAGIC {
                    return Err(bad());
                }
                cursor.take(16)?;
                if parse_profile(cursor.take(1)?[0]) != Some(profile)
                    || !parse_batch_resources(&mut cursor)?.is_empty()
                {
                    return Err(bad());
                }
            }
            let (units, raw_total) = parse_batch_units(&mut cursor)?;
            if cursor.offset != frame.len() {
                return Err(bad());
            }
            total_units = total_units
                .checked_add(units.len() as u64)
                .filter(|n| *n <= max_units)
                .ok_or_else(bad)?;
            total_raw = total_raw
                .checked_add(raw_total as u64)
                .filter(|n| *n <= max_raw)
                .ok_or_else(bad)?;
            let response_bytes =
                BATCH_ACK_BYTES + units.len() * BATCH_UNIT_BYTES + OPERATION_END_BYTES;
            wire = wire
                .checked_add(response_bytes as u64)
                .filter(|n| *n <= max_wire)
                .ok_or_else(bad)?;
            for unit in &units {
                if !validators.contains_key(unit.root_uri) {
                    if validators.len() >= max_selectors {
                        return Err(bad());
                    }
                    validators.insert(
                        unit.root_uri.to_owned(),
                        compile_selected_validator(&probe, &registry, profile, unit.root_uri)?,
                    );
                }
            }
            let mut request = Digest256Hasher::new();
            request.update(&header);
            request.update(&frame);
            let request_sha = request.finalize();
            let mut ack = Vec::with_capacity(BATCH_ACK_BYTES);
            ack.extend_from_slice(BATCH_ACK_MAGIC);
            ack.extend_from_slice(request_sha.as_bytes());
            ack.extend_from_slice(schema_set.as_bytes());
            ack.extend_from_slice(&(units.len() as u32).to_be_bytes());
            output.write_all(&ack)?;
            output.flush()?;
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            for unit in &units {
                let result = match crate::published_value(
                    unit.raw,
                    crate::SchemaBackendProbe::MAX_INSTANCE_BYTES,
                ) {
                    Ok(value) => {
                        if validators[unit.root_uri].is_valid(&value) {
                            (0u8, 0u8)
                        } else {
                            (1, 0)
                        }
                    }
                    Err(crate::SchemaProbeError::InvalidPublishedJson(_))
                    | Err(crate::SchemaProbeError::InvalidJson) => (2, 1),
                    Err(crate::SchemaProbeError::BudgetExceeded) => (3, 1),
                    Err(_) => (3, 2),
                };
                let mut response = Vec::with_capacity(BATCH_UNIT_BYTES);
                response.extend_from_slice(BATCH_UNIT_MAGIC);
                response.extend_from_slice(&unit.ordinal.to_be_bytes());
                response.extend_from_slice(unit.unit_sha256.as_bytes());
                response.extend_from_slice(&[result.0, result.1]);
                output.write_all(&response)?;
                output.flush()?;
                if !matches!(result, (0, 0) | (1, 0)) {
                    // The request-bound typed unit refusal remains the failure
                    // reason. Exit without END/FINAL, so coverage cannot pass.
                    return Ok(());
                }
                results.update(unit.unit_sha256.as_bytes());
                results.update(&[result.0, result.1]);
            }
            let mut end = Vec::with_capacity(OPERATION_END_BYTES);
            end.extend_from_slice(OPERATION_END_MAGIC);
            end.extend_from_slice(request_sha.as_bytes());
            end.extend_from_slice(results.finalize().as_bytes());
            end.extend_from_slice(&(units.len() as u32).to_be_bytes());
            output.write_all(&end)?;
            output.flush()?;
            drop(units);
            sequence = sequence.checked_add(1).ok_or_else(bad)?;
            frame = match read_operation_frame(&mut input)? {
                Some(next) => next,
                None => return Err(bad()),
            };
        }
    }

    fn compile_selected_validator<'a>(
        probe: &crate::SchemaBackendProbe,
        registry: &'a jsonschema::Registry<'a>,
        profile: FormatProfile,
        root_uri: &str,
    ) -> io::Result<jsonschema::Validator> {
        let schema = probe
            .selected_schema(root_uri)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "missing root selector"))?;
        let mut options = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .with_registry(registry)
            .offline()
            .should_validate_formats(true)
            .should_ignore_unknown_formats(false);
        if profile == FormatProfile::LegacyPythonObserved20260923 {
            options = options
                .with_format("date-time", |_| true)
                .with_format("uri", |_| true)
                .with_format("uri-reference", |_| true);
        }
        options
            .build(schema.as_ref())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "schema compile"))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn batch_schema() -> Vec<SchemaResource> {
            vec![SchemaResource {
                uri: "https://treeofsophia.local/tests/batch-integer.schema.json".to_owned(),
                raw: br#"{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"https://treeofsophia.local/tests/batch-integer.schema.json","type":"integer"}"#.to_vec(),
            }]
        }

        fn batch_unit(ordinal: u64, raw: &[u8]) -> BatchUnit {
            BatchUnit {
                ordinal,
                member_id: format!("test-member-{ordinal}"),
                relative_path: format!("synthetic/{ordinal}.json"),
                root_uri: batch_schema()[0].uri.clone(),
                raw_instance: raw.to_vec(),
            }
        }

        fn operation_fixture(prepared: &[&BatchPrepared]) -> (Vec<u8>, Vec<Digest256>) {
            let budget = BatchStreamBudget::laboratory();
            let first = prepared[0];
            let header = operation_header(
                [0; 16],
                first.schema_set_sha256,
                first.profile,
                [
                    budget.max_chunks,
                    budget.max_total_units,
                    budget.max_total_raw_bytes,
                    budget.max_total_wire_bytes,
                    budget.max_distinct_selectors as u64,
                    budget.operation_cpu_seconds,
                    budget.operation_address_space_bytes,
                    budget.total_execution_wall.as_nanos() as u64,
                ],
            );
            let mut input = header.clone();
            let mut requests = Vec::new();
            for (sequence, frame) in prepared.iter().enumerate() {
                let mut body = (sequence as u64).to_be_bytes().to_vec();
                body.extend_from_slice(&frame.frame);
                let mut digest = Digest256Hasher::new();
                digest.update(&header);
                digest.update(&body);
                requests.push(digest.finalize());
                input.extend_from_slice(&(body.len() as u32).to_be_bytes());
                input.extend_from_slice(&body);
            }
            let mut close = (prepared.len() as u64).to_be_bytes().to_vec();
            close.extend_from_slice(OPERATION_CLOSE_MAGIC);
            close.extend_from_slice(Digest256::of_bytes(&header).as_bytes());
            input.extend_from_slice(&(close.len() as u32).to_be_bytes());
            input.extend_from_slice(&close);
            (input, requests)
        }

        fn batch_stream_output(units: Vec<BatchUnit>) -> Vec<u8> {
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                units,
                BatchBudget::laboratory(),
            )
            .unwrap();
            let mut output = Vec::new();
            let (input, request_sha) = operation_fixture(&[&prepared]);
            operation_worker_once(
                std::io::Cursor::new(&input[8..]),
                &mut output,
                *OPERATION_REQUEST_MAGIC,
            )
            .unwrap();
            assert_eq!(&output[8..40], request_sha[0].as_bytes());
            output
        }

        fn fixture_image(path: &str) -> File {
            let path = std::fs::canonicalize(path).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            sealed_worker(&ExactWorkerIdentity {
                absolute_path: path,
                sha256: Digest256::of_bytes(&bytes),
            })
            .unwrap()
        }

        fn fixture_identity() -> ExecutionIdentity {
            ExecutionIdentity {
                worker_sha256: Digest256::of_bytes(b"fixture-worker"),
                request_sha256: Digest256::of_bytes(b"fixture-request"),
                schema_set_sha256: Digest256::of_bytes(b"fixture-schemas"),
                instance_sha256: Digest256::of_bytes(b"fixture-instance"),
                profile: FormatProfile::AssertedSourceCandidateV1,
            }
        }

        #[test]
        fn oversized_and_duplicate_inputs_never_launch_worker() {
            let one = SchemaResource {
                uri: "https://example.invalid/schema".to_owned(),
                raw: b"{}".to_vec(),
            };
            let absent = ExactWorkerIdentity {
                absolute_path: "/absent-worker".into(),
                sha256: Digest256::of_bytes(b""),
            };
            assert!(matches!(
                BoundedSchemaExecutor::evaluate(
                    &absent,
                    &[one.clone()],
                    FormatProfile::AssertedSourceCandidateV1,
                    "root",
                    &vec![0; crate::SchemaBackendProbe::MAX_INSTANCE_BYTES + 1],
                    ExecutorBudget::laboratory()
                ),
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::InputBudget,
                    ..
                }
            ));
            assert!(matches!(
                BoundedSchemaExecutor::evaluate(
                    &absent,
                    &[one.clone(), one],
                    FormatProfile::AssertedSourceCandidateV1,
                    "root",
                    b"null",
                    ExecutorBudget::laboratory()
                ),
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Backend,
                    ..
                }
            ));
        }

        #[test]
        fn worker_that_never_reads_stdin_is_killed_without_waiting_for_a_writer() {
            let image = fixture_image("/usr/bin/sleep");
            let arg = c"2";
            let argv = [
                c"sleep".as_ptr() as *mut libc::c_char,
                arg.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let budget = ExecutorBudget {
                execution_wall: Duration::from_millis(200),
                cleanup_grace: Duration::from_millis(200),
                cpu_seconds: 2,
                address_space_bytes: 1024 * 1024 * 1024,
            };
            let start = Instant::now();
            let result = run_image(
                &image,
                vec![b'x'; 2 * 1024 * 1024],
                fixture_identity(),
                budget,
                start,
                &argv,
                None,
            );
            assert!(matches!(
                result,
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Timeout,
                    ..
                }
            ));
            assert!(start.elapsed() < Duration::from_millis(600));
            // Reuse the same immutable image after the first child's reap.
            // Its second isolated invocation must honor live cancellation.
            let cancelled = AtomicBool::new(false);
            let result = std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(Duration::from_millis(20));
                    cancelled.store(true, Ordering::Relaxed);
                });
                run_image(
                    &image,
                    vec![b'x'; 2 * 1024 * 1024],
                    fixture_identity(),
                    ExecutorBudget {
                        execution_wall: Duration::from_secs(2),
                        ..budget
                    },
                    Instant::now(),
                    &argv,
                    Some(&cancelled),
                )
            });
            assert!(matches!(
                result,
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Cancelled,
                    ..
                }
            ));
        }

        #[test]
        fn escaped_descendant_retaining_stdout_cannot_hold_parent_after_deadline() {
            use std::os::unix::ffi::OsStrExt;
            use std::time::{SystemTime, UNIX_EPOCH};

            let image = fixture_image("/bin/sh");
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir()
                .join(format!("tos-val2-escaped-{}-{unique}", std::process::id()));
            std::fs::create_dir(&dir).unwrap();
            let pid_file = dir.join("child.pid");
            let pid_arg = std::ffi::CString::new(pid_file.as_os_str().as_bytes()).unwrap();
            // The inner shell records its exact PID, then becomes sleep. It
            // has a new session and retains the worker's stdout socket.
            let script = c"/usr/bin/setsid /bin/sh -c 'echo $$ > \"$1\"; exec /usr/bin/sleep 1.2' child \"$1\" & echo ready; wait";
            let argv = [
                c"sh".as_ptr() as *mut libc::c_char,
                c"-c".as_ptr() as *mut libc::c_char,
                script.as_ptr() as *mut libc::c_char,
                c"fixture".as_ptr() as *mut libc::c_char,
                pid_arg.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let budget = ExecutorBudget {
                execution_wall: Duration::from_millis(600),
                cleanup_grace: Duration::from_millis(200),
                cpu_seconds: 2,
                address_space_bytes: 1024 * 1024 * 1024,
            };
            let start = Instant::now();
            let result = run_image(
                &image,
                b"request".to_vec(),
                fixture_identity(),
                budget,
                start,
                &argv,
                None,
            );
            let child_pid: i32 = std::fs::read_to_string(&pid_file)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let pidfd_raw = unsafe { libc::syscall(libc::SYS_pidfd_open, child_pid, 0) as i32 };
            assert!(pidfd_raw >= 0);
            let pidfd = unsafe { File::from_raw_fd(pidfd_raw) };
            let expected_inode = TEST_CHILD_STDOUT_INODE.with(std::cell::Cell::get);
            let fd1 = std::fs::read_link(format!("/proc/{child_pid}/fd/1")).unwrap();
            assert_eq!(fd1.to_string_lossy(), format!("socket:[{expected_inode}]"));
            assert!(matches!(
                result,
                ExecutorOutcome::Indeterminate {
                    reason: ExecutorFailure::Timeout,
                    ..
                }
            ));
            assert!(start.elapsed() < Duration::from_millis(950));
            let mut exit_poll = libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            assert_eq!(unsafe { libc::poll(&mut exit_poll, 1, 1500) }, 1);
            assert_ne!(exit_poll.revents & libc::POLLIN, 0);
            std::fs::remove_file(pid_file).unwrap();
            std::fs::remove_dir(dir).unwrap();
        }

        #[test]
        fn batch_reuses_compiled_validator_without_prior_instance_state() {
            let first = batch_stream_output(vec![
                batch_unit(0, b"7"),
                batch_unit(1, br#""x""#),
                batch_unit(2, b"7"),
            ]);
            assert_eq!(
                first.len(),
                BATCH_ACK_BYTES
                    + 3 * BATCH_UNIT_BYTES
                    + OPERATION_END_BYTES
                    + OPERATION_FINAL_BYTES
            );
            assert_eq!(first[BATCH_ACK_BYTES + 48], 0);
            assert_eq!(first[BATCH_ACK_BYTES + BATCH_UNIT_BYTES + 48], 1);
            assert_eq!(first[BATCH_ACK_BYTES + 2 * BATCH_UNIT_BYTES + 48], 0);
            let permuted = batch_stream_output(vec![
                batch_unit(0, br#""x""#),
                batch_unit(1, b"7"),
                batch_unit(2, b"7"),
            ]);
            assert_eq!(permuted[BATCH_ACK_BYTES + 48], 1);
            assert_eq!(permuted[BATCH_ACK_BYTES + BATCH_UNIT_BYTES + 48], 0);
            assert_eq!(permuted[BATCH_ACK_BYTES + 2 * BATCH_UNIT_BYTES + 48], 0);
        }

        #[test]
        fn batch_selected_fragments_keep_scope_and_independent_unit_outcomes() {
            let root = "https://treeofsophia.local/tests/root.json";
            let target = "https://treeofsophia.local/tests/nested/types.json";
            let resources = vec![SchemaResource {
                uri: root.into(),
                raw: format!(r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{root}","$defs":{{"scoped":{{"$id":"nested/child.json","properties":{{"a/b~c":{{"$ref":"types.json#/$defs/code"}}}}}},"deny":false}}}}"#).into_bytes(),
            }, SchemaResource {
                uri: target.into(),
                raw: format!(r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{target}","$defs":{{"code":{{"const":"owned"}}}}}}"#).into_bytes(),
            }];
            let mut units = vec![
                batch_unit(0, br#""owned""#),
                batch_unit(1, br#""other""#),
                batch_unit(2, b"3"),
            ];
            units[0].root_uri = format!("{root}#/$defs/scoped/properties/a~1b~0c");
            units[1].root_uri = units[0].root_uri.clone();
            units[2].root_uri = format!("{root}#/$defs/deny");
            let expected = BatchCoverageExpectation::from_units(&units).unwrap();
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &resources,
                FormatProfile::AssertedSourceCandidateV1,
                units,
                BatchBudget::laboratory(),
            )
            .unwrap();
            assert_eq!(
                prepared.ordered_manifest_sha256,
                expected.ordered_manifest_sha256
            );
            let mut output = Vec::new();
            let mut later_unit = batch_unit(0, br#""owned""#);
            later_unit.root_uri = format!("{root}#/$defs/scoped/properties/a~1b~0c");
            let later = make_batch_request_encoded(
                prepared.worker_sha256,
                &0u32.to_be_bytes(),
                prepared.schema_set_sha256,
                prepared.profile,
                [later_unit],
                BatchBudget::laboratory(),
            )
            .unwrap();
            let (input, request_sha) = operation_fixture(&[&prepared, &later]);
            operation_worker_once(
                std::io::Cursor::new(&input[8..]),
                &mut output,
                *OPERATION_REQUEST_MAGIC,
            )
            .unwrap();
            assert_eq!(&output[8..40], request_sha[0].as_bytes());
            assert_eq!(
                output.len(),
                2 * BATCH_ACK_BYTES
                    + 4 * BATCH_UNIT_BYTES
                    + 2 * OPERATION_END_BYTES
                    + OPERATION_FINAL_BYTES
            );
            assert_eq!(&output[8..40], request_sha[0].as_bytes());
            assert_eq!(&output[40..72], prepared.schema_set_sha256.as_bytes());
            for (ordinal, verdict) in [0u8, 1, 1].into_iter().enumerate() {
                let start = BATCH_ACK_BYTES + ordinal * BATCH_UNIT_BYTES;
                assert_eq!(
                    &output[start + 8..start + 16],
                    &(ordinal as u64).to_be_bytes()
                );
                assert_eq!(
                    &output[start + 16..start + 48],
                    prepared.units[ordinal].unit_sha256.as_bytes()
                );
                assert_eq!(output[start + 48], verdict);
            }
            let second = BATCH_ACK_BYTES + 3 * BATCH_UNIT_BYTES + OPERATION_END_BYTES;
            assert_eq!(&output[second + 8..second + 40], request_sha[1].as_bytes());
            assert_ne!(request_sha[0], request_sha[1]);
            assert_eq!(output[second + BATCH_ACK_BYTES + 48], 0);
            assert_eq!(
                &output[output.len() - OPERATION_FINAL_BYTES
                    ..output.len() - OPERATION_FINAL_BYTES + 8],
                OPERATION_FINAL_MAGIC
            );
            // A later malformed frame or omitted CLOSE cannot finalize this operation.
            let second_input = OPERATION_HEADER_BYTES + 4 + 8 + prepared.frame.len();
            let mut out_of_order = input.clone();
            out_of_order[second_input + 4..second_input + 12].copy_from_slice(&0u64.to_be_bytes());
            for broken in [
                out_of_order,
                input[..input.len() - 52].to_vec(),
                input[..second_input + 6].to_vec(),
            ] {
                let mut partial = Vec::new();
                assert!(
                    operation_worker_once(
                        std::io::Cursor::new(&broken[8..]),
                        &mut partial,
                        *OPERATION_REQUEST_MAGIC
                    )
                    .is_err()
                );
                assert!(!partial.windows(8).any(|w| w == OPERATION_FINAL_MAGIC));
            }
        }

        #[test]
        fn shared_verified_image_preserves_seals_deadline_and_independent_operation_state() {
            use std::os::unix::fs::{FileExt, PermissionsExt};
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("fixture-image");
            let raw = b"Synthetic executable custody fixture; no worker execution.";
            std::fs::write(&path, raw).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
            let cancel = AtomicBool::new(false);
            let deadline = Instant::now() + Duration::from_secs(60);
            let handle = VerifiedWorkerImageHandle::prepare(
                ExactWorkerIdentity {
                    absolute_path: path.clone(),
                    sha256: Digest256::of_bytes(raw),
                },
                ExecutorBudget::laboratory(),
                deadline,
                &cancel,
            )
            .unwrap();
            // Subsequent adapters use only this actual sealed image, never a
            // reopened path or a caller-supplied assertion of verification.
            std::fs::remove_file(&path).unwrap();
            let mut first = VerifiedWorkerImage::from_handle(
                &handle,
                ExecutorBudget::laboratory(),
                deadline,
                &cancel,
            )
            .unwrap();
            let second = VerifiedWorkerImage::from_handle(
                &handle,
                ExecutorBudget::laboratory(),
                deadline,
                &cancel,
            )
            .unwrap();
            let mut observed = vec![0; raw.len()];
            second.file.read_exact_at(&mut observed, 0).unwrap();
            assert_eq!(observed, raw);
            let seals = unsafe { libc::fcntl(second.file.as_raw_fd(), libc::F_GET_SEALS) };
            let required =
                libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
            assert_eq!(seals & required, required);
            first.used_frames = 1;
            first.poisoned = Some(ExecutorFailure::Backend);
            assert_eq!(second.used_frames, 0);
            assert!(second.poisoned.is_none() && second.session.is_none());
            assert_eq!(first.identity.sha256, second.identity.sha256);
            assert_eq!(second.operation_deadline, deadline);
            assert!(matches!(
                VerifiedWorkerImage::from_handle(
                    &handle,
                    ExecutorBudget::laboratory(),
                    Instant::now(),
                    &cancel
                ),
                Err(ExecutorFailure::Timeout)
            ));
            assert!(matches!(
                VerifiedWorkerImage::from_handle(
                    &handle,
                    ExecutorBudget::laboratory(),
                    deadline,
                    &AtomicBool::new(true)
                ),
                Err(ExecutorFailure::Cancelled)
            ));
        }

        #[test]
        fn batch_cancellation_before_image_and_during_poll_is_incomplete() {
            // The selected-cut and varying-plan consumers prepare the same
            // image primitive. Cancelled/expired operations refuse before any
            // lookup, rather than turning setup into an unbounded extra phase.
            let absent = ExactWorkerIdentity {
                absolute_path: PathBuf::from("/absent-worker"),
                sha256: Digest256::of_bytes(b""),
            };
            assert!(matches!(
                VerifiedWorkerImage::prepare(
                    &absent,
                    ExecutorBudget::laboratory(),
                    Instant::now() + Duration::from_secs(1),
                    &AtomicBool::new(true)
                ),
                Err(ExecutorFailure::Cancelled)
            ));
            assert!(matches!(
                VerifiedWorkerImage::prepare(
                    &absent,
                    ExecutorBudget::laboratory(),
                    Instant::now(),
                    &AtomicBool::new(false)
                ),
                Err(ExecutorFailure::Timeout)
            ));
            let units = vec![batch_unit(0, b"7")];
            let expected = BatchCoverageExpectation::from_units(&units).unwrap();
            let outcome = BoundedSchemaExecutor::evaluate_batch_cancellable(
                &ExactWorkerIdentity {
                    absolute_path: PathBuf::from("/absent-worker"),
                    sha256: Digest256::of_bytes(b""),
                },
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                units,
                expected,
                BatchBudget::laboratory(),
                &AtomicBool::new(true),
            );
            assert!(
                matches!(outcome, BatchOutcome::Incomplete { reason: ExecutorFailure::Cancelled, receipts, .. } if receipts.is_empty())
            );
            let image = fixture_image("/usr/bin/sleep");
            let argv = [
                c"sleep".as_ptr() as *mut libc::c_char,
                c"2".as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let budget = BatchBudget::laboratory();
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                budget,
            )
            .unwrap();
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let cancelled = AtomicBool::new(false);
            let start = Instant::now();
            let outcome = std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(Duration::from_millis(20));
                    cancelled.store(true, Ordering::Relaxed);
                });
                run_batch_image_cancellable(
                    &image,
                    prepared,
                    results,
                    budget,
                    start,
                    &argv,
                    Some(&cancelled),
                )
            });
            assert!(
                matches!(outcome, BatchOutcome::Incomplete { reason: ExecutorFailure::Cancelled, receipts, .. } if receipts.is_empty())
            );
            assert!(start.elapsed() < Duration::from_millis(700));
        }

        #[test]
        fn independent_fixed_manifest_oracle_catches_unit_framing_drift() {
            let uri = "https://treeofsophia.local/tests/batch-integer.schema.json";
            let units = [
                BatchUnit {
                    ordinal: 0,
                    member_id: "unit-0".to_owned(),
                    relative_path: "synthetic/0.json".to_owned(),
                    root_uri: uri.to_owned(),
                    raw_instance: b"7".to_vec(),
                },
                BatchUnit {
                    ordinal: 1,
                    member_id: "unit-1".to_owned(),
                    relative_path: "synthetic/1.json".to_owned(),
                    root_uri: uri.to_owned(),
                    raw_instance: b"\"x\"".to_vec(),
                },
            ];
            // Values computed independently with Python hashlib+struct using
            // the published binary framing, not `make_batch_request`.
            let expected_units = [
                "71b39a8481983557b3a69c93c8657c166cbc1baeb5f0668bb15089a07e935af6",
                "d934c3e71a992a994c57209d0f39f106e460cb2ccc06d45aa8ceec2ee69e4213",
            ];
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                units,
                BatchBudget::laboratory(),
            )
            .unwrap();
            for (unit, expected) in prepared.units.iter().zip(expected_units) {
                assert_eq!(unit.unit_sha256.to_hex(), expected);
            }
            assert_eq!(
                prepared.ordered_manifest_sha256.to_hex(),
                "fb0a7be8c7c650bf92309ffe2da54cf36ed2f1a492732518353950b30a4945a0"
            );
        }

        #[test]
        fn batch_missing_trailing_and_budget_violations_refuse_without_receipts() {
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                BatchBudget::laboratory(),
            )
            .unwrap();
            for changed in [prepared.frame[..prepared.frame.len() - 1].to_vec(), {
                let mut value = prepared.frame.clone();
                value.push(0);
                value
            }] {
                let mut output = Vec::new();
                assert!(
                    {
                        let changed_prepared = BatchPrepared {
                            frame: changed,
                            ..prepared.clone()
                        };
                        let (input, _) = operation_fixture(&[&changed_prepared]);
                        operation_worker_once(
                            std::io::Cursor::new(&input[8..]),
                            &mut output,
                            *OPERATION_REQUEST_MAGIC,
                        )
                    }
                    .is_err()
                );
                assert!(output.is_empty());
            }
            let mut tiny = BatchBudget::laboratory();
            tiny.max_total_raw_bytes = 1;
            assert!(matches!(
                make_batch_request(
                    Digest256::of_bytes(b"fixture-worker"),
                    &batch_schema(),
                    FormatProfile::AssertedSourceCandidateV1,
                    [batch_unit(0, b"77")],
                    tiny,
                ),
                Err(ExecutorFailure::InputBudget)
            ));
            let too_many = (0..65).map(|ordinal| batch_unit(ordinal, b"7"));
            assert!(matches!(
                make_batch_request(
                    Digest256::of_bytes(b"fixture-worker"),
                    &batch_schema(),
                    FormatProfile::AssertedSourceCandidateV1,
                    too_many,
                    BatchBudget::laboratory(),
                ),
                Err(ExecutorFailure::InputBudget)
            ));
        }

        #[test]
        fn batch_expected_manifest_mismatch_refuses_before_worker_lookup() {
            let outcome = BoundedSchemaExecutor::evaluate_batch(
                &ExactWorkerIdentity {
                    absolute_path: "/does/not/exist/tos-schema-worker".into(),
                    sha256: Digest256::of_bytes(b"fixture-worker"),
                },
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                BatchCoverageExpectation {
                    count: 1,
                    ordered_manifest_sha256: Digest256::of_bytes(b"wrong manifest"),
                },
                BatchBudget::laboratory(),
            );
            assert!(matches!(
                outcome,
                BatchOutcome::Incomplete {
                    reason: ExecutorFailure::CoverageMismatch,
                    receipts,
                    ..
                } if receipts.is_empty()
            ));
        }

        #[test]
        fn batch_missing_or_trailing_worker_receipt_refuses_full_coverage() {
            let image = fixture_image("/usr/bin/python3");
            let budget = BatchBudget::laboratory();
            for trailing in [false, true] {
                let prepared = make_batch_request(
                    Digest256::of_bytes(b"fixture-worker"),
                    &batch_schema(),
                    FormatProfile::AssertedSourceCandidateV1,
                    [batch_unit(0, b"7")],
                    budget,
                )
                .unwrap();
                let mut output = Vec::new();
                output.extend_from_slice(BATCH_ACK_MAGIC);
                output.extend_from_slice(prepared.request_sha256.as_bytes());
                output.extend_from_slice(prepared.schema_set_sha256.as_bytes());
                output.extend_from_slice(&1u32.to_be_bytes());
                if trailing {
                    output.extend_from_slice(BATCH_UNIT_MAGIC);
                    output.extend_from_slice(&0u64.to_be_bytes());
                    output.extend_from_slice(prepared.units[0].unit_sha256.as_bytes());
                    output.extend_from_slice(&[0, 0, 0]);
                }
                let output_hex: String = output.iter().map(|byte| format!("{byte:02x}")).collect();
                let output_arg = std::ffi::CString::new(output_hex).unwrap();
                let script = c"import sys; sys.stdout.buffer.write(bytes.fromhex(sys.argv[1])); sys.stdout.buffer.flush()";
                let argv = [
                    c"python3".as_ptr() as *mut libc::c_char,
                    c"-c".as_ptr() as *mut libc::c_char,
                    script.as_ptr() as *mut libc::c_char,
                    output_arg.as_ptr() as *mut libc::c_char,
                    std::ptr::null_mut(),
                ];
                let mut results = Digest256Hasher::new();
                results.update(b"tos-val2-batch-results-v1\0");
                let outcome =
                    run_batch_image(&image, prepared, results, budget, Instant::now(), &argv);
                assert!(matches!(
                    outcome,
                    BatchOutcome::Incomplete {
                        reason: ExecutorFailure::Protocol,
                        ..
                    }
                ));
            }
            // Force EOF while the child is still blocked on input, then let
            // it exit naturally. EOF must not make cleanup immediately kill it.
            let script = c"import os,sys; os.close(1); sys.stdin.buffer.read(1); sys.exit(17)";
            let argv = [
                c"python3".as_ptr() as *mut libc::c_char,
                c"-c".as_ptr() as *mut libc::c_char,
                script.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let mut child =
                spawn_operation_child(&image, ExecutorBudget::laboratory(), &argv).unwrap();
            let eof_deadline = Instant::now() + Duration::from_millis(700);
            loop {
                let mut byte = [0u8; 1];
                let count = unsafe {
                    libc::recv(
                        child.output.as_raw_fd(),
                        byte.as_mut_ptr().cast(),
                        1,
                        libc::MSG_DONTWAIT,
                    )
                };
                if count == 0 {
                    break;
                }
                assert!(
                    count < 0 && io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock
                );
                assert!(Instant::now() < eof_deadline, "child did not close output");
                thread::sleep(
                    Duration::from_millis(1)
                        .min(eof_deadline.saturating_duration_since(Instant::now())),
                );
            }
            poll_exit(child.pid, &mut child.status).unwrap();
            assert!(
                child.status.is_none(),
                "EOF fixture must still be awaiting input"
            );
            assert_eq!(
                unsafe {
                    libc::send(
                        child.input.as_raw_fd(),
                        b"x".as_ptr().cast(),
                        1,
                        libc::MSG_NOSIGNAL,
                    )
                },
                1
            );
            let cleanup_started = Instant::now();
            child.cleanup_after_eof().unwrap();
            assert_eq!(
                child.natural_status.map(status_failure),
                Some(Some(ExecutorFailure::CrashExit(17)))
            );
            assert_eq!(child.status, child.natural_status);
            assert!(cleanup_started.elapsed() < Duration::from_millis(700));
            child.cleanup().unwrap();
            assert_eq!(child.status, child.natural_status);

            // Closing stdout does not imply process exit. A live EOF child
            // receives only the original cleanup envelope and then is killed;
            // that SIGKILL is cleanup evidence, never natural termination.
            let script = c"import os,time; os.close(1); time.sleep(2)";
            let argv = [
                c"python3".as_ptr() as *mut libc::c_char,
                c"-c".as_ptr() as *mut libc::c_char,
                script.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let mut child =
                spawn_operation_child(&image, ExecutorBudget::laboratory(), &argv).unwrap();
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                budget,
            )
            .unwrap();
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let started = Instant::now();
            let outcome =
                run_batch_exchange(&mut child, prepared, results, budget, started, None, true);
            match outcome {
                BatchOutcome::Incomplete {
                    reason: ExecutorFailure::Protocol,
                    receipts,
                    exchange: Some(context),
                    ..
                } => {
                    assert!(receipts.is_empty());
                    assert_eq!(context.boundary, "early-output-eof");
                    assert_eq!(context.failure, ExecutorFailure::Protocol);
                    assert_eq!(context.natural_termination, None);
                }
                other => panic!("live EOF child did not preserve failure origin: {other:?}"),
            }
            assert!(started.elapsed() < Duration::from_millis(700));
            assert_eq!(
                child.status.map(status_failure),
                Some(Some(ExecutorFailure::CrashSignal(libc::SIGKILL)))
            );
            assert_eq!(child.natural_status, None);
            child.cleanup_after_eof().unwrap();
            assert_eq!(child.natural_status, None);

            // A status observed before cleanup is source of the termination
            // detail; unlike a cleanup SIGKILL, it may explain missing output.
            for (script, expected) in [
                (c"import sys; sys.exit(17)", ChildTermination::Exited(17)),
                (
                    c"import os,signal; os.kill(os.getpid(),signal.SIGTERM)",
                    ChildTermination::Signalled(libc::SIGTERM),
                ),
            ] {
                let budget = BatchBudget::laboratory();
                let argv = [
                    c"python3".as_ptr() as *mut libc::c_char,
                    c"-c".as_ptr() as *mut libc::c_char,
                    script.as_ptr() as *mut libc::c_char,
                    std::ptr::null_mut(),
                ];
                let mut child =
                    spawn_operation_child(&image, ExecutorBudget::laboratory(), &argv).unwrap();
                let start = Instant::now();
                while child.status.is_none() && start.elapsed() < Duration::from_millis(700) {
                    poll_exit(child.pid, &mut child.status).unwrap();
                    if child.status.is_none() {
                        thread::sleep(Duration::from_millis(1));
                    }
                }
                assert!(
                    child.status.is_some(),
                    "natural child exit was not observed"
                );
                let prepared = make_batch_request(
                    Digest256::of_bytes(b"fixture-worker"),
                    &batch_schema(),
                    FormatProfile::AssertedSourceCandidateV1,
                    [batch_unit(0, b"7")],
                    budget,
                )
                .unwrap();
                let mut results = Digest256Hasher::new();
                results.update(b"tos-val2-batch-results-v1\0");
                let outcome = run_batch_exchange(
                    &mut child,
                    prepared,
                    results,
                    budget,
                    Instant::now(),
                    None,
                    true,
                );
                match outcome {
                    BatchOutcome::Incomplete {
                        reason,
                        receipts,
                        exchange: Some(context),
                        ..
                    } => {
                        assert_eq!(
                            reason,
                            match expected {
                                ChildTermination::Exited(code) => ExecutorFailure::CrashExit(code),
                                ChildTermination::Signalled(signal) =>
                                    ExecutorFailure::CrashSignal(signal),
                            }
                        );
                        assert!(receipts.is_empty());
                        assert_eq!(context.failure, ExecutorFailure::Protocol);
                        assert_eq!(context.natural_termination, Some(expected));
                        assert!(!context.boundary.is_empty());
                    }
                    other => panic!("missing natural termination context: {other:?}"),
                }
                // Latched cleanup cannot overwrite the independently observed
                // original status on a second call or signal a reused PID.
                child.cleanup().unwrap();
                assert_eq!(child.natural_status, child.status);
            }
        }

        #[test]
        fn batch_startup_timeout_kills_real_worker_and_is_incomplete() {
            let image = fixture_image("/usr/bin/sleep");
            let argv = [
                c"sleep".as_ptr() as *mut libc::c_char,
                c"2".as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let mut budget = BatchBudget::laboratory();
            budget.total_execution_wall = Duration::from_millis(250);
            budget.startup_wall = Duration::from_millis(200);
            budget.per_unit_wall = Duration::from_millis(100);
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                budget,
            )
            .unwrap();
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let start = Instant::now();
            let outcome = run_batch_image(&image, prepared, results, budget, start, &argv);
            assert!(matches!(
                &outcome,
                BatchOutcome::Incomplete {
                    reason: ExecutorFailure::Timeout,
                    receipts,
                    ..
                } if receipts.is_empty()
            ));
            if let BatchOutcome::Incomplete {
                exchange: Some(context),
                ..
            } = outcome
            {
                assert_eq!(context.failure, ExecutorFailure::Timeout);
                assert_eq!(context.boundary, "ack-startup-wall");
                assert_eq!(
                    context.natural_termination, None,
                    "cleanup SIGKILL is not natural termination"
                );
            } else {
                panic!("missing timeout exchange context");
            }
            assert!(start.elapsed() < Duration::from_millis(700));
        }

        #[test]
        fn batch_per_unit_timeout_after_real_ack_kills_and_refuses() {
            let image = fixture_image("/usr/bin/python3");
            let mut budget = BatchBudget::laboratory();
            budget.total_execution_wall = Duration::from_secs(1);
            budget.startup_wall = Duration::from_millis(700);
            budget.per_unit_wall = Duration::from_millis(100);
            let prepared = make_batch_request(
                Digest256::of_bytes(b"fixture-worker"),
                &batch_schema(),
                FormatProfile::AssertedSourceCandidateV1,
                [batch_unit(0, b"7")],
                budget,
            )
            .unwrap();
            let mut ack = Vec::new();
            ack.extend_from_slice(BATCH_ACK_MAGIC);
            ack.extend_from_slice(prepared.request_sha256.as_bytes());
            ack.extend_from_slice(prepared.schema_set_sha256.as_bytes());
            ack.extend_from_slice(&1u32.to_be_bytes());
            let ack_hex: String = ack.iter().map(|byte| format!("{byte:02x}")).collect();
            let ack_arg = std::ffi::CString::new(ack_hex).unwrap();
            let script = c"import sys,time; sys.stdout.buffer.write(bytes.fromhex(sys.argv[1])); sys.stdout.buffer.flush(); time.sleep(2)";
            let argv = [
                c"python3".as_ptr() as *mut libc::c_char,
                c"-c".as_ptr() as *mut libc::c_char,
                script.as_ptr() as *mut libc::c_char,
                ack_arg.as_ptr() as *mut libc::c_char,
                std::ptr::null_mut(),
            ];
            let mut results = Digest256Hasher::new();
            results.update(b"tos-val2-batch-results-v1\0");
            let start = Instant::now();
            let outcome = run_batch_image(&image, prepared, results, budget, start, &argv);
            assert!(matches!(
                outcome,
                BatchOutcome::Incomplete {
                    reason: ExecutorFailure::Timeout,
                    receipts,
                    ..
                } if receipts.is_empty()
            ));
            assert!(start.elapsed() < Duration::from_millis(700));
        }
    }
}
