//! Standalone source-verified bibliographic query over the maintained Rust
//! corpus projection. The shared direct-source request and finite budget
//! remain owned by `managed_native_original_cli`.

use crate::managed_native_original_cli::{
    DirectRepositoryProjectionRequest, check_direct_projection_products,
    read_direct_repository_projection_request, validate_direct_projection_request,
    with_direct_repository_projection,
};
use crate::source_bibliographic_query::query_verified_projection;
use serde_json::{Map, Value};
use std::{
    ffi::OsString,
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};

const MAX_REQUEST_BYTES: u64 = 64 * 1024;
const MAX_ARGUMENTS: usize = 21;

#[derive(Default)]
struct QueryOptions {
    request_path: Option<PathBuf>,
    selectors: Map<String, Value>,
    limit: Option<u64>,
    pretty: bool,
}

fn refusal(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<QueryOptions, std::io::Error> {
    let mut args = args.into_iter().take(MAX_ARGUMENTS + 1).collect::<Vec<_>>();
    if args.len() > MAX_ARGUMENTS {
        return Err(refusal(
            "native bibliographic query exceeded its argument bound",
        ));
    }
    let mut options = QueryOptions::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index]
            .to_str()
            .ok_or_else(|| refusal("native bibliographic query arguments must be UTF-8"))?;
        index += 1;
        match flag {
            "--request" => {
                let value = args
                    .get(index)
                    .ok_or_else(|| refusal("--request requires an absolute request file"))?;
                let path = PathBuf::from(value);
                if !path.is_absolute() || options.request_path.replace(path).is_some() {
                    return Err(refusal(
                        "--request must be supplied once with an absolute path",
                    ));
                }
                index += 1;
            }
            "--claim-ref" | "--subject-ref" | "--object-ref" | "--normalized-ref"
            | "--predicate" | "--review-status" | "--visibility" => {
                let value = args
                    .get(index)
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| refusal(format!("{flag} requires one UTF-8 value")))?;
                let key = flag.trim_start_matches("--").replace('-', "_");
                if options
                    .selectors
                    .insert(key, Value::String(value.to_owned()))
                    .is_some()
                {
                    return Err(refusal(format!("{flag} may be supplied only once")));
                }
                index += 1;
            }
            "--limit" => {
                let value = args
                    .get(index)
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| refusal("--limit requires an integer from 1 to 100"))?;
                if options.limit.is_some() {
                    return Err(refusal("--limit may be supplied only once"));
                }
                let limit = value
                    .parse::<u64>()
                    .map_err(|_| refusal("--limit requires an integer from 1 to 100"))?;
                if !(1..=100).contains(&limit) {
                    return Err(refusal("--limit requires an integer from 1 to 100"));
                }
                options.limit = Some(limit);
                index += 1;
            }
            "--pretty" => {
                if options.pretty {
                    return Err(refusal("--pretty may be supplied only once"));
                }
                options.pretty = true;
            }
            _ => return Err(refusal(format!("unsupported native query option: {flag}"))),
        }
    }
    if options.request_path.is_none() {
        return Err(refusal("--request ABSOLUTE_JSON is required"));
    }
    if options.selectors.is_empty() {
        return Err(refusal("at least one exact query selector is required"));
    }
    Ok(options)
}

fn read_request(
    path: PathBuf,
) -> Result<DirectRepositoryProjectionRequest, Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)?;
    let metadata = file.metadata()?;
    let uid = rustix::process::getuid().as_raw();
    if !metadata.file_type().is_file()
        || metadata.uid() != uid
        || metadata.mode() & 0o022 != 0
        || metadata.len() > MAX_REQUEST_BYTES
    {
        return Err(refusal("request must be a bounded, owner-controlled regular file").into());
    }
    let request = read_direct_repository_projection_request(&mut file)?;
    validate_direct_projection_request(&request)?;
    Ok(request)
}

fn query(
    request: &DirectRepositoryProjectionRequest,
    options: &QueryOptions,
) -> Result<Value, Box<dyn std::error::Error>> {
    validate_direct_projection_request(request)?;
    let (_, result) = with_direct_repository_projection(request, |products, _| {
        check_direct_projection_products(request, products)?;
        query_verified_projection(
            &products.bibliographic_claims,
            &Value::Object(options.selectors.clone()),
            options.limit,
            request.limits.max_output_bytes,
        )
        .map_err(|reason| refusal(format!("bibliographic query refused: {reason}")).into())
    })?;
    Ok(result)
}

/// Run the argv query route. Query options retain the retired command's exact
/// selector names and bounds; `--request` supplies the shared direct-source
/// request envelope used by the native corpus parity checker.
pub fn run_args(
    args: impl IntoIterator<Item = OsString>,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> i32 {
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(error) => {
            let _ = writeln!(
                diagnostics,
                "native corpus projection query refused: {error}"
            );
            return 2;
        }
    };
    let pretty = options.pretty;
    let Some(path) = options.request_path.clone() else {
        let _ = writeln!(
            diagnostics,
            "native corpus projection query refused: --request ABSOLUTE_JSON is required"
        );
        return 2;
    };
    let request = match read_request(path) {
        Ok(request) => request,
        Err(error) => {
            let _ = writeln!(
                diagnostics,
                "native corpus projection query refused: {error}"
            );
            return 2;
        }
    };
    let max_output_bytes = request.limits.max_output_bytes;
    let result = match query(&request, &options) {
        Ok(result) => result,
        Err(error) => {
            let _ = writeln!(
                diagnostics,
                "native corpus projection query refused: {error}"
            );
            return 2;
        }
    };
    let encoded = if pretty {
        serde_json::to_vec_pretty(&result)
    } else {
        serde_json::to_vec(&result)
    };
    let result = match encoded {
        Ok(bytes) if (bytes.len() as u64).saturating_add(1) <= max_output_bytes => bytes,
        Ok(_) => {
            let _ = diagnostics.write_all(
                b"native corpus projection query refused: result exceeds output byte budget\n",
            );
            return 2;
        }
        Err(error) => {
            let _ = writeln!(
                diagnostics,
                "native corpus projection query refused: {error}"
            );
            return 2;
        }
    };
    if output.write_all(&result).is_err() || output.write_all(b"\n").is_err() {
        let _ = diagnostics.write_all(b"native corpus projection query output failed\n");
        2
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn legacy_selector_flags_and_defaults_are_preserved() {
        let options = parse_args(os(&[
            "--request",
            "/tmp/projection.json",
            "--claim-ref",
            "tos.claim.one",
            "--subject-ref",
            "tos.work.one",
            "--normalized-ref",
            "tos.place.one",
            "--limit",
            "9",
            "--pretty",
        ]))
        .unwrap();
        assert_eq!(options.limit, Some(9));
        assert!(options.pretty);
        assert_eq!(options.selectors["claim_ref"], "tos.claim.one");
        assert_eq!(options.selectors["subject_ref"], "tos.work.one");
        assert_eq!(options.selectors["normalized_ref"], "tos.place.one");
        assert_eq!(
            parse_args(os(&[
                "--request",
                "/tmp/p",
                "--predicate",
                "has_expression"
            ]))
            .unwrap()
            .limit,
            None
        );
    }

    #[test]
    fn query_flags_require_one_selector_and_reject_duplicates_or_unknowns() {
        assert!(parse_args(os(&["--request", "/tmp/p"])).is_err());
        assert!(
            parse_args(os(&[
                "--request",
                "/tmp/p",
                "--claim-ref",
                "a",
                "--claim-ref",
                "b"
            ]))
            .is_err()
        );
        assert!(
            parse_args(os(&[
                "--request",
                "/tmp/p",
                "--output",
                "/tmp/out",
                "--claim-ref",
                "a"
            ]))
            .is_err()
        );
        assert!(parse_args(os(&["--request", "relative", "--claim-ref", "a"])).is_err());
    }
}
