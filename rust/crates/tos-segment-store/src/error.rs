use std::fmt;
use std::io;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SegmentErrorCode {
    InvalidRoot,
    UnsupportedPlatform,
    UnsupportedOversized,
    UnsafePath,
    InvalidFormat,
    CorruptBytes,
    BudgetExceeded,
    InvalidReceipt,
    PinConflict,
    Io,
}

#[derive(Debug)]
pub struct SegmentError {
    pub code: SegmentErrorCode,
    pub detail: &'static str,
    pub source: Option<io::Error>,
}

impl SegmentError {
    pub const fn new(code: SegmentErrorCode, detail: &'static str) -> Self {
        Self {
            code,
            detail,
            source: None,
        }
    }

    pub fn io(detail: &'static str, source: io::Error) -> Self {
        Self {
            code: SegmentErrorCode::Io,
            detail,
            source: Some(source),
        }
    }
}

impl fmt::Display for SegmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.detail)
    }
}

impl std::error::Error for SegmentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

pub type Result<T> = std::result::Result<T, SegmentError>;
