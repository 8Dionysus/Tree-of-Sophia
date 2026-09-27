//! Source-backed diagnostic only: no prepared admission, server or activation.
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use tos_foundation::{
    Digest256, JsonLimits, JsonMode, JsonNumber, JsonNumberKind, JsonString, JsonValue,
    emit_value_preserved_json, parse_json,
};
use tos_query::{
    AbortProbe, InspectBudget,
    philosophy_read::{PhilosophyReadBudget, compute_source_philosophy_view_diagnostic},
};
const SOURCE_BYTES: usize = 4 * 1024 * 1024;
const REPORT_BYTES: usize = 65536;
const CONTRACTS: [&[u8]; 3] = [
    include_bytes!("../../../../access/contracts/runtime-manifest.v1.json"),
    include_bytes!("../../../../access/contracts/runtime-data.v1.json"),
    include_bytes!("../../../../access/contracts/web-actions.v1.json"),
];
fn text(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}
fn number(value: usize) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}
fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn field<'a>(value: &'a JsonValue, key: &str) -> &'a JsonValue {
    value.object_get(key).unwrap_or(&JsonValue::Null)
}
fn limits() -> JsonLimits {
    JsonLimits {
        max_bytes: SOURCE_BYTES,
        max_depth: 64,
        max_visits: 200000,
        max_integer_digits: 4300,
    }
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    let file =
        tos_fd_open::open_absolute_regular(path, SOURCE_BYTES as u64).map_err(|e| e.to_string())?;
    let identity = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    let before = file.metadata().map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    (&file)
        .take(SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    if raw.len() > SOURCE_BYTES
        || identity(&before) != identity(&file.metadata().map_err(|e| e.to_string())?)
    {
        return Err("source changed or exceeds diagnostic byte bound".into());
    }
    Ok(raw)
}
fn document(raw: &[u8]) -> Result<JsonValue, String> {
    let value = parse_json(raw, JsonMode::RequestLastWins, limits())
        .map_err(|e| e.to_string())?
        .into_root();
    if value.as_object().is_none() {
        return Err("projection must be a JSON object".into());
    }
    Ok(value)
}
fn check(
    checks: &mut Vec<JsonValue>,
    id: &str,
    ok: bool,
    required: bool,
    details: Vec<(&str, JsonValue)>,
) {
    let mut fields = vec![
        ("check_id", text(id)),
        ("ok", JsonValue::Bool(ok)),
        ("required", JsonValue::Bool(required)),
    ];
    fields.extend(details);
    checks.push(object(fields));
}
fn selected_path(root: &Path, env: &str, relative: &str) -> PathBuf {
    let path = std::env::var_os(env)
        .filter(|raw| !raw.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(relative));
    let path = expand(&path);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}
fn expand(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| path.to_owned())
    } else if let Some(rest) = raw.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|| path.to_owned())
    } else {
        path.to_owned()
    }
}
fn absolute(path: &Path) -> Result<PathBuf, String> {
    let path = expand(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path))
    }
}
fn partitioned(value: &JsonValue) -> bool {
    ["schema_version", "schema"]
        .iter()
        .any(|key| field(value, key).as_str() == Some("tos_partitioned_projection_v1"))
}
struct NeverAbort;
impl AbortProbe for NeverAbort {
    fn reason(&self) -> Option<tos_query::AbortReason> {
        None
    }
}

pub fn doctor_report(
    root: &Path,
    profile: &str,
    require_mcp: bool,
    program_directory: &Path,
) -> Result<JsonValue, String> {
    if !matches!(profile, "standalone" | "abyssos") {
        return Err(format!("unknown access profile: {profile}"));
    }
    let root = absolute(root)?;
    // Explicit source-backed selection never searches another repository.
    let index = selected_path(
        &root,
        "TOS_CORPUS_INDEX_PATH",
        "ToS/derived-exports/tos_corpus_index.min.json",
    );
    let graph = selected_path(
        &root,
        "TOS_PHILOSOPHY_GRAPH_PROJECTION_PATH",
        "ToS/derived-exports/philosophy_graph_projection.min.json",
    );
    let evidence = selected_path(
        &root,
        "TOS_EVIDENCE_PROJECTION_PATH",
        "ToS/derived-exports/epistemic_evidence_projection.min.json",
    );
    let store = selected_path(
        &root,
        "TOS_QUERY_STORE_PATH",
        "ToS/derived-exports/runtime/knowledge.sqlite3",
    );
    let mut checks = vec![];
    let index_raw = if index.is_file() {
        Some(read(&index).and_then(|raw| document(&raw).map(|value| (raw, value))))
    } else {
        None
    };
    let graph_raw = if graph.is_file() {
        Some(read(&graph).and_then(|raw| document(&raw).map(|value| (raw, value))))
    } else {
        None
    };
    let source_partitioned = [index_raw.as_ref(), graph_raw.as_ref()]
        .into_iter()
        .flatten()
        .any(|raw| {
            raw.as_ref()
                .ok()
                .is_some_and(|(_, value)| partitioned(value))
        });
    let bibliographic = selected_path(
        &root,
        "TOS_BIBLIOGRAPHIC_GRAPH_PATH",
        "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    );
    let bibliography_partitioned = bibliographic.is_file()
        && read(&bibliographic)
            .and_then(|raw| document(&raw))
            .is_ok_and(|value| partitioned(&value));
    if std::env::var_os("TOS_QUERY_STORE_PATH").is_some_and(|raw| !raw.is_empty())
        || store.is_file()
        || source_partitioned
        || bibliography_partitioned
    {
        check(
            &mut checks,
            "query-store",
            false,
            true,
            vec![
                ("path", text(&store.to_string_lossy())),
                (
                    "error",
                    text(
                        "legacy Python query store is unsupported by the native source-backed diagnostic; no build is performed",
                    ),
                ),
            ],
        );
    }
    check(
        &mut checks,
        "corpus-index-present",
        index_raw.is_some(),
        true,
        vec![("path", text(&index.to_string_lossy()))],
    );
    if let Some(raw) = index_raw {
        match raw {
            Ok((raw, value)) => {
                let header = if partitioned(&value) {
                    field(&value, "header")
                } else {
                    &value
                };
                check(
                    &mut checks,
                    "corpus-index-schema",
                    field(header, "schema_version").as_str() == Some("tos_corpus_index_v1"),
                    true,
                    vec![
                        ("schema_version", field(header, "schema_version").clone()),
                        ("sha256", text(&Digest256::of_bytes(&raw).to_hex())),
                    ],
                );
            }
            Err(error) => check(
                &mut checks,
                "corpus-index-schema",
                false,
                true,
                vec![("schema_version", JsonValue::Null), ("error", text(&error))],
            ),
        }
    }
    check(
        &mut checks,
        "philosophy-graph-present",
        graph_raw.is_some(),
        true,
        vec![("path", text(&graph.to_string_lossy()))],
    );
    if let Some(raw) = graph_raw {
        match raw {
            Ok((raw, value)) => {
                let schema = field(&value, "schema_version");
                let supported = matches!(
                    schema.as_str(),
                    Some(
                        "tos_philosophy_graph_projection_v1" | "tos_philosophy_graph_projection_v2"
                    )
                );
                check(
                    &mut checks,
                    "philosophy-graph-schema",
                    supported,
                    true,
                    vec![
                        ("schema_version", schema.clone()),
                        ("sha256", text(&Digest256::of_bytes(&raw).to_hex())),
                    ],
                );
                if supported {
                    let first = field(&value, "views").as_array().and_then(|views| {
                        views.iter().find_map(|view| {
                            field(view, "view_id").as_str().filter(|id| !id.is_empty())
                        })
                    });
                    let packet = first
                        .ok_or_else(|| "projection has no graph views".to_owned())
                        .and_then(|view| {
                            let inspect = InspectBudget {
                                max_open_vm_steps: 1_000_000,
                                max_read_vm_steps: 1_000_000,
                                max_response_bytes: 1_048_576,
                                max_decoded_bytes: SOURCE_BYTES as u64,
                                max_matches: 100000,
                                max_rows: 100000,
                                max_field_bytes: 4096,
                                max_payload_bytes: SOURCE_BYTES,
                                json: limits(),
                            };
                            compute_source_philosophy_view_diagnostic(
                                &raw,
                                view,
                                PhilosophyReadBudget {
                                    inspect,
                                    max_work_steps: 1_000_000,
                                },
                                &NeverAbort,
                            )
                            .map_err(|e| e.message.to_owned())
                            .and_then(|bytes| document(&bytes))
                        });
                    match packet {
                        Ok(value) => {
                            let nodes = field(&value, "node_count");
                            let edges = field(&value, "edge_count");
                            let count = |v: &JsonValue| {
                                if let JsonValue::Number(n) = v {
                                    n.lexeme.parse::<usize>().unwrap_or(0)
                                } else {
                                    0
                                }
                            };
                            check(
                                &mut checks,
                                "graph-view-materialization",
                                count(nodes) > 0 && count(edges) > 0,
                                true,
                                vec![
                                    ("view_id", field(field(&value, "view"), "view_id").clone()),
                                    ("node_count", nodes.clone()),
                                    ("edge_count", edges.clone()),
                                ],
                            );
                        }
                        Err(error) => check(
                            &mut checks,
                            "graph-view-materialization",
                            false,
                            true,
                            vec![
                                ("view_id", first.map(text).unwrap_or(JsonValue::Null)),
                                ("node_count", number(0)),
                                ("edge_count", number(0)),
                                ("error", text(&error)),
                            ],
                        ),
                    }
                }
            }
            Err(error) => check(
                &mut checks,
                "philosophy-graph-schema",
                false,
                true,
                vec![("schema_version", JsonValue::Null), ("error", text(&error))],
            ),
        }
    }
    check(
        &mut checks,
        "evidence-projection-present",
        evidence.is_file(),
        true,
        vec![("path", text(&evidence.to_string_lossy()))],
    );
    if evidence.is_file() {
        match read(&evidence).and_then(|raw| document(&raw).map(|value| (raw, value))) {
            Ok((raw, value)) => {
                let scenes = field(&value, "scenes").as_array();
                check(
                    &mut checks,
                    "evidence-projection-schema",
                    field(&value, "schema_version").as_str()
                        == Some("tos_epistemic_evidence_projection_v1")
                        && scenes.is_some_and(|v| !v.is_empty()),
                    true,
                    vec![
                        ("schema_version", field(&value, "schema_version").clone()),
                        ("scene_count", number(scenes.map_or(0, |v| v.len()))),
                        ("sha256", text(&Digest256::of_bytes(&raw).to_hex())),
                    ],
                );
            }
            Err(error) => check(
                &mut checks,
                "evidence-projection-schema",
                false,
                true,
                vec![("schema_version", JsonValue::Null), ("error", text(&error))],
            ),
        }
    }
    let contracts = CONTRACTS
        .iter()
        .map(|raw| document(raw))
        .collect::<Result<Vec<_>, _>>();
    let contracts_ok = contracts.as_ref().is_ok_and(|values| {
        values
            .iter()
            .zip([
                "tos_access_runtime_manifest_v1",
                "tos_access_runtime_data_allowlist_v1",
                "tos_web_actions_v1",
            ])
            .all(|(value, schema)| field(value, "schema_version").as_str() == Some(schema))
    });
    check(
        &mut checks,
        "runtime-contracts",
        contracts_ok,
        true,
        vec![("path", text("embedded:access/contracts"))],
    );
    let web = program_directory.join("web_dist");
    let web_read = read(&web.join("assets/tos-graph.js"));
    let web_ok = web_read.as_ref().is_ok_and(|raw| !raw.is_empty());
    check(
        &mut checks,
        "web-assets",
        web_ok,
        true,
        vec![
            (
                "path",
                if web_ok {
                    text(&web.to_string_lossy())
                } else {
                    JsonValue::Null
                },
            ),
            (
                "error",
                if web_ok {
                    JsonValue::Null
                } else {
                    text(
                        "installed software web assets unavailable; selected data cannot supply executable assets",
                    )
                },
            ),
        ],
    );
    check(
        &mut checks,
        "native-mcp-dependency",
        crate::registered_operations().is_ok(),
        require_mcp,
        vec![
            ("install_hint", JsonValue::Null),
            ("implementation", text("built-in-rust-stdio")),
        ],
    );
    let abyss = std::env::var_os("TOS_ABYSSOS_ROOT")
        .filter(|raw| !raw.to_string_lossy().trim().is_empty())
        .map(|raw| PathBuf::from(raw.to_string_lossy().trim()))
        .map(|path| absolute(&path))
        .transpose()?;
    let available = abyss
        .as_ref()
        .is_some_and(|path| path.join("abyss-stack").is_dir());
    check(
        &mut checks,
        "abyssos-integration",
        available,
        profile == "abyssos",
        vec![
            ("available", JsonValue::Bool(available)),
            (
                "configured_root",
                abyss
                    .as_ref()
                    .map(|path| text(&path.to_string_lossy()))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "install_hint",
                if available {
                    JsonValue::Null
                } else {
                    text("set TOS_ABYSSOS_ROOT to an AbyssOS root containing abyss-stack")
                },
            ),
            ("posture", text("optional-adapter")),
        ],
    );
    if profile == "abyssos" {
        let posture = contracts
            .as_ref()
            .ok()
            .and_then(|values| values.first())
            .map(|value| field(value, "integration_posture"));
        let paused = posture.is_some_and(|value| {
            field(value, "state").as_str() == Some("paused")
                && field(value, "scope")
                    .as_array()
                    .is_some_and(|v| v.iter().any(|v| v.as_str() == Some("abyssos")))
        });
        check(
            &mut checks,
            "abyssos-integration-freeze",
            contracts_ok && !paused,
            true,
            vec![
                (
                    "state",
                    posture
                        .map(|v| field(v, "state").clone())
                        .unwrap_or(JsonValue::Null),
                ),
                (
                    "activation",
                    posture
                        .map(|v| field(v, "external_activation").clone())
                        .unwrap_or(JsonValue::Null),
                ),
                (
                    "reason",
                    if paused {
                        text(
                            "ToS has paused AbyssOS activation; an explicit ToS operator command is required",
                        )
                    } else {
                        JsonValue::Null
                    },
                ),
            ],
        );
    }
    let failures = checks
        .iter()
        .filter(|value| {
            field(value, "required") == &JsonValue::Bool(true)
                && field(value, "ok") == &JsonValue::Bool(false)
        })
        .map(|value| field(value, "check_id").clone())
        .collect::<Vec<_>>();
    Ok(object(vec![
        ("schema_version", text("tos_access_doctor_report_v1")),
        ("profile", text(profile)),
        ("ok", JsonValue::Bool(failures.is_empty())),
        ("tos_root", text(&root.to_string_lossy())),
        ("checks", JsonValue::Array(checks)),
        ("required_failures", JsonValue::Array(failures)),
        (
            "authority_limit",
            text(
                "This report proves local access mechanics only; ToS sources own meaning and review.",
            ),
        ),
    ]))
}
fn render(report: &JsonValue) -> String {
    let ready = field(report, "ok") == &JsonValue::Bool(true);
    let mut lines = vec![format!(
        "Tree of Sophia access: {} ({})",
        if ready { "ready" } else { "not ready" },
        field(report, "profile").as_str().unwrap()
    )];
    for item in field(report, "checks").as_array().unwrap() {
        let mark = if field(item, "ok") == &JsonValue::Bool(true) {
            "ok"
        } else if field(item, "required") == &JsonValue::Bool(false) {
            "optional"
        } else {
            "fail"
        };
        lines.push(format!(
            "[{mark}] {}",
            field(item, "check_id").as_str().unwrap()
        ));
    }
    let failures = field(report, "required_failures").as_array().unwrap();
    if !failures.is_empty() {
        lines.push(format!(
            "Required failures: {}",
            failures
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines.join("\n")
}
/// Intercept only a real diagnostic command after maintained global selectors.
/// Other native routes retain their own parser and authority selection.
pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    let mut at = 0;
    let mut root = None;
    let mut prepared = false;
    let mut explicit_release = false;
    while let Some(option) = args.get(at) {
        let (key, inline) = option
            .split_once('=')
            .map_or((option.as_str(), None), |(k, v)| (k, Some(v)));
        if !matches!(
            key,
            "--root" | "--prepared-read-model" | "--prepared-binding" | "--release-root"
        ) {
            break;
        }
        let value = if let Some(value) = inline {
            value
        } else {
            at += 1;
            args.get(at).map(String::as_str).unwrap_or("")
        };
        if value.is_empty() {
            let _ = writeln!(stderr, "invalid_request: {key} requires a path");
            return Some(2);
        }
        match key {
            "--root" => root = Some(PathBuf::from(value)),
            "--release-root" => explicit_release = true,
            _ => prepared = true,
        }
        at += 1;
    }
    let command = args.get(at)?.as_str();
    if !matches!(command, "doctor" | "verify") {
        return None;
    }
    if prepared
        || explicit_release
        || (root.is_none()
            && !std::env::var_os("TOS_DATA_ROOT").is_some_and(|raw| !raw.is_empty())
            && std::env::var_os("TOS_RELEASE_ROOT").is_some_and(|raw| !raw.is_empty()))
    {
        let _ = writeln!(
            stderr,
            "doctor/verify check the source-backed profile, not a prepared publication"
        );
        return Some(2);
    }
    let mut as_json = false;
    let mut profile = "standalone";
    at += 1;
    while at < args.len() {
        let option = &args[at];
        if option == "--json" {
            as_json = true;
            at += 1;
            continue;
        }
        if command == "verify" && (option == "--profile" || option.starts_with("--profile=")) {
            profile = if let Some((_, value)) = option.split_once('=') {
                value
            } else {
                at += 1;
                args.get(at).map(String::as_str).unwrap_or("")
            };
            if !matches!(profile, "standalone" | "abyssos") {
                let _ = writeln!(stderr, "invalid_request: unknown access profile: {profile}");
                return Some(2);
            }
            at += 1;
            continue;
        }
        let _ = writeln!(
            stderr,
            "invalid_request: unsupported {command} option: {option}"
        );
        return Some(2);
    }
    let result = (|| {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let program = executable
            .parent()
            .ok_or_else(|| "installed software directory unavailable".to_owned())?;
        let root = root
            .or_else(|| {
                std::env::var_os("TOS_DATA_ROOT")
                    .filter(|raw| !raw.is_empty())
                    .map(PathBuf::from)
            })
            .unwrap_or_else(|| program.join("runtime_data"));
        let report = doctor_report(&root, profile, command == "verify", program)?;
        let ok = field(&report, "ok") == &JsonValue::Bool(true);
        let bytes = if as_json {
            emit_value_preserved_json(
                &report,
                JsonLimits {
                    max_bytes: REPORT_BYTES,
                    ..JsonLimits::default()
                },
            )
            .map_err(|e| e.to_string())?
        } else {
            render(&report).into_bytes()
        };
        stdout
            .write_all(&bytes)
            .and_then(|_| stdout.write_all(b"\n"))
            .and_then(|_| stdout.flush())
            .map_err(|e| e.to_string())?;
        Ok::<i32, String>(if ok { 0 } else { 1 })
    })();
    Some(match result {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(stderr, "doctor failed: {error}");
            1
        }
    })
}
