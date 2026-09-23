use tos_foundation::JsonLimits;

use crate::error::{Result, StoreError, StoreErrorCode};

/// Explicit finite resource profile for one v1 corpus reader.
///
/// A caller may choose a larger profile after capacity review. No implicit
/// default raises the limit based on untrusted manifest or object metadata.
#[derive(Clone, Copy, Debug)]
pub struct ReadLimits {
    pub max_manifest_bytes: usize,
    pub max_manifest_entries: usize,
    pub max_selected_object_bytes: u64,
    pub json: JsonLimits,
}

impl ReadLimits {
    pub fn validate(self) -> Result<Self> {
        if self.max_manifest_bytes == 0
            || self.max_manifest_entries == 0
            || self.max_selected_object_bytes == 0
            || self.max_manifest_bytes == usize::MAX
            || self.max_manifest_entries == usize::MAX
            || self.max_selected_object_bytes == u64::MAX
            || self.json.max_bytes == 0
            || self.json.max_depth == 0
            || self.json.max_visits == 0
            || self.json.max_integer_digits == 0
        {
            return Err(StoreError::new(
                StoreErrorCode::BudgetExceeded,
                "invalid or unbounded source read limits",
            ));
        }
        Ok(self)
    }
}
