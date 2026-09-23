use crate::error::{FoundationError, FoundationErrorCode, Result};

/// A normalized lexical corpus path; the caller must still check filesystem objects.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct RelativePath(String);

impl RelativePath {
    pub fn parse(value: &str) -> Result<Self> {
        if value.is_empty() || value.starts_with('/') || value.contains('\\')
            || value.chars().any(|ch| (ch as u32) < 32)
            || value.split('/').any(|part| part.is_empty() || matches!(part, "." | ".." | ".git"))
        {
            return Err(FoundationError::new(FoundationErrorCode::UnsafePath, "path must be normalized, relative and outside .git"));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str { &self.0 }
    pub fn into_string(self) -> String { self.0 }
}
