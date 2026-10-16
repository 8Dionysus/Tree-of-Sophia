//! Exact code-point coordinates for the owner-local TextUnit creator.
//! Input bytes remain immutable; a bounded set of requested endpoints is
//! resolved in one UTF-8 scan rather than rescanning the text for every unit.

use crate::source_command::{SourceCommandError, SourceCommandResult};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::Digest256;

const MAX_ENDPOINTS: usize = 2 * (1 + 256 + 257);
const MAX_TEXT_BYTES: usize = 8_388_608;

pub(crate) struct TextCodepointOffsets {
    offsets: BTreeMap<usize, usize>,
}

impl TextCodepointOffsets {
    pub(crate) fn select(
        text: &str,
        endpoints: &[usize],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        if text.is_empty()
            || text.len() > MAX_TEXT_BYTES
            || endpoints.is_empty()
            || endpoints.len() > MAX_ENDPOINTS
        {
            return Err(SourceCommandError::Unsupported(
                "native TextUnit text or endpoint budget",
            ));
        }
        let mut wanted = endpoints.to_vec();
        wanted.sort_unstable();
        wanted.dedup();
        let mut offsets = BTreeMap::new();
        let mut next = 0usize;
        let mut count = 0usize;
        for (ordinal, (byte, _)) in text.char_indices().enumerate() {
            if ordinal % 65_536 == 0 {
                crate::source_creation_store::active(deadline, cancelled)?;
            }
            if wanted.get(next) == Some(&ordinal) {
                offsets.insert(ordinal, byte);
                next += 1;
            }
            count = ordinal + 1;
        }
        if wanted.get(next) == Some(&count) {
            offsets.insert(count, text.len());
            next += 1;
        }
        crate::source_creation_store::active(deadline, cancelled)?;
        if next != wanted.len() {
            return Err(SourceCommandError::Invalid(
                "native TextUnit endpoint outside exact text",
            ));
        }
        Ok(Self { offsets })
    }

    pub(crate) fn exact_sha256(
        &self,
        text: &str,
        start: usize,
        end: usize,
    ) -> SourceCommandResult<Digest256> {
        let left = *self.offsets.get(&start).ok_or(SourceCommandError::Invalid(
            "native TextUnit start endpoint",
        ))?;
        let right = *self
            .offsets
            .get(&end)
            .ok_or(SourceCommandError::Invalid("native TextUnit end endpoint"))?;
        if start >= end || left >= right || right > text.len() {
            return Err(SourceCommandError::Invalid("native TextUnit interval"));
        }
        Ok(Digest256::of_bytes(&text.as_bytes()[left..right]))
    }
}
