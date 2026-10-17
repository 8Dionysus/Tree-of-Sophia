//! Durable PostgreSQL source-text search needs the native source-owner adapter.
//! Keep the transport surface type-correct while failing closed.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use tos_query::{AbortProbe, source_text_search::SourceTextSearchRequest};

use crate::{AccessError, AccessErrorCode, AccessProfile, PreparedPacket};

#[derive(Clone, Debug)]
pub struct SelectedPgSourceTextSearch;

impl SelectedPgSourceTextSearch {
    pub fn new(_selection_path: &Path) -> Result<Self, AccessError> {
        Err(unavailable())
    }

    pub fn execute(
        &self,
        _request_json: &[u8],
        _profile: AccessProfile,
        _probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        Err(unavailable())
    }

    pub fn execute_request(
        &self,
        _request: SourceTextSearchRequest,
        _profile: AccessProfile,
        _probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
        Err(unavailable())
    }
}

pub fn run_cli(
    _args: &[String],
    _selected: Option<&SelectedPgSourceTextSearch>,
    _profile: AccessProfile,
    _stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let _ = writeln!(stderr, "unavailable: {}", unavailable_message());
    3
}

fn unavailable() -> AccessError {
    AccessError::new(AccessErrorCode::Unavailable, unavailable_message())
}

fn unavailable_message() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "durable PostgreSQL source-text search is native-only"
    } else {
        "durable PostgreSQL source-text search is not enabled in this build"
    }
}
