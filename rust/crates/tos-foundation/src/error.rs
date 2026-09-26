use std::fmt;

/// Stable mechanical error classes. A transport maps these to its own status codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FoundationErrorCode {
    InvalidUtf8,
    InvalidJson,
    InvalidUnicodeScalar,
    DuplicateMember,
    InvalidNumber,
    NonfiniteFloat,
    InvalidIdentifier,
    InvalidVersion,
    InvalidDigest,
    UnsafePath,
    InvalidCoordinate,
    InvalidDescriptor,
    DuplicateDescriptor,
    BudgetExceeded,
    UnsupportedFormat,
    UnsupportedCanonicalNumber,
}

impl FoundationErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "invalid_utf8",
            Self::InvalidJson => "invalid_json",
            Self::InvalidUnicodeScalar => "invalid_unicode_scalar",
            Self::DuplicateMember => "duplicate_member",
            Self::InvalidNumber => "invalid_number",
            Self::NonfiniteFloat => "nonfinite_float",
            Self::InvalidIdentifier => "invalid_identifier",
            Self::InvalidVersion => "invalid_version",
            Self::InvalidDigest => "invalid_digest",
            Self::UnsafePath => "unsafe_path",
            Self::InvalidCoordinate => "invalid_coordinate",
            Self::InvalidDescriptor => "invalid_descriptor",
            Self::DuplicateDescriptor => "duplicate_descriptor",
            Self::BudgetExceeded => "budget_exceeded",
            Self::UnsupportedFormat => "unsupported_format",
            Self::UnsupportedCanonicalNumber => "unsupported_canonical_number",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundationError {
    pub code: FoundationErrorCode,
    pub byte_offset: Option<usize>,
    pub detail: String,
}

impl FoundationError {
    pub fn new(code: FoundationErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            byte_offset: None,
            detail: detail.into(),
        }
    }

    pub fn at(mut self, byte_offset: usize) -> Self {
        self.byte_offset = Some(byte_offset);
        self
    }
}

impl fmt::Display for FoundationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.byte_offset {
            Some(offset) => write!(
                f,
                "{} at byte {offset}: {}",
                self.code.as_str(),
                self.detail
            ),
            None => write!(f, "{}: {}", self.code.as_str(), self.detail),
        }
    }
}

impl std::error::Error for FoundationError {}

pub type Result<T> = std::result::Result<T, FoundationError>;
