//! Whole exact predecessor normalization for the maintained additive layer
//! operation. The caller retains input bytes and constructs the explicit edit;
//! this pure transform neither inherits quality nor grants publication.

use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use unicode_normalization::UnicodeNormalization;

const MAX_DERIVED_TEXT_BYTES: usize = 131_072;
const UNICODE_DATABASE_VERSION: &str = "16.0.0";

pub(crate) fn normalize_whole_predecessor(
    input: &str,
    form: &str,
    declared_unicode_version: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    if input.is_empty()
        || input.len() > MAX_DERIVED_TEXT_BYTES
        || input.contains('\0')
        || declared_unicode_version != UNICODE_DATABASE_VERSION
    {
        return Err(SourceCommandError::Unsupported(
            "native TextLayer Unicode input or database version",
        ));
    }
    match form {
        "NFC" => collect_bounded(input.nfc(), deadline, cancelled),
        "NFD" => collect_bounded(input.nfd(), deadline, cancelled),
        "NFKC" => collect_bounded(input.nfkc(), deadline, cancelled),
        "NFKD" => collect_bounded(input.nfkd(), deadline, cancelled),
        _ => Err(SourceCommandError::Invalid("native TextLayer Unicode form")),
    }
}

fn collect_bounded(
    iter: impl Iterator<Item = char>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<String> {
    let mut output = String::new();
    for (index, ch) in iter.enumerate() {
        if index % 1024 == 0 {
            active(deadline, cancelled)?;
        }
        if output
            .len()
            .checked_add(ch.len_utf8())
            .is_none_or(|size| size > MAX_DERIVED_TEXT_BYTES)
            || ch == '\0'
        {
            return Err(SourceCommandError::Unsupported(
                "native TextLayer normalized output budget",
            ));
        }
        output.push(ch);
    }
    active(deadline, cancelled)?;
    if output.is_empty() {
        return Err(SourceCommandError::Invalid(
            "native TextLayer normalized output absent",
        ));
    }
    Ok(output)
}
