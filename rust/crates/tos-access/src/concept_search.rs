//! Direct concept cards, preserving the maintained standalone query packet.
use crate::{AccessError, AccessErrorCode, AccessExecutor, AccessProfile};
use std::io::Write;
use tos_query::reading_search::ConceptSearchRequest;

pub(crate) fn run_cli(
    args: &[String],
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let result = (|| {
        let mut request = ConceptSearchRequest {
            query: String::new(),
            language: String::new(),
            limit: "20".into(),
            include_semantic_neighbors: false,
            request_ref: None,
        };
        let mut has_query = false;
        let mut at = 1;
        while at < args.len() {
            if args[at] == "--include-semantic-neighbors" {
                request.include_semantic_neighbors = true;
                at += 1;
                continue;
            }
            let value = args.get(at + 1).ok_or_else(|| {
                AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "concept-search option requires a value",
                )
            })?;
            match args[at].as_str() {
                "--query" => {
                    request.query = value.clone();
                    has_query = true;
                }
                "--language" => request.language = value.clone(),
                "--limit" => request.limit = value.clone(),
                "--request" => request.request_ref = Some(value.clone()),
                _ => {
                    return Err(AccessError::new(
                        AccessErrorCode::InvalidRequest,
                        "unsupported concept-search option",
                    ));
                }
            }
            at += 2;
        }
        if !has_query || request.language.is_empty() {
            return Err(AccessError::new(
                AccessErrorCode::InvalidRequest,
                "concept-search requires query and language",
            ));
        }
        crate::checked_execute(profile.deadline_probe(), |probe| {
            executor.concept_search(request, probe)
        })
    })();
    match result {
        Ok(packet) => crate::cli::write_packet(packet, profile, stdout, stderr),
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            match error.code {
                AccessErrorCode::InvalidRequest => 2,
                AccessErrorCode::Unavailable => 3,
                _ => 1,
            }
        }
    }
}
