use std::fmt;
use std::io;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreErrorCode {
    InvalidRoot,
    UnsupportedFormat,
    InvalidCanonicalSnapshot,
    RevisionMismatch,
    InvalidMemberIndex,
    InvalidIdentityIndex,
    InvalidDependencyIndex,
    InvalidRetirementIndex,
    MissingRevision,
    MissingMember,
    InvalidSelector,
    UnsafePath,
    CorruptSelectedObject,
    BudgetExceeded,
    DescriptorMismatch,
    Io,
}

#[derive(Debug)]
pub struct StoreError {
    pub code: StoreErrorCode,
    pub detail: &'static str,
    pub source: Option<io::Error>,
}

impl StoreError {
    pub const fn new(code: StoreErrorCode, detail: &'static str) -> Self {
        Self {
            code,
            detail,
            source: None,
        }
    }

    pub fn io(detail: &'static str, source: io::Error) -> Self {
        Self {
            code: StoreErrorCode::Io,
            detail,
            source: Some(source),
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.detail)
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;
