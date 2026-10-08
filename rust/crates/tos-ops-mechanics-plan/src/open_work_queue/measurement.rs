//! Native monotonic timing; curl is the bounded TLS/HTTP platform bridge.
use super::*;
use std::{
    io::Read,
    os::fd::AsRawFd,
    process::{Command, Stdio},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
const LIMIT: usize = 16_384;
const CLOCK: &str = "rust.std.time.Instant";
#[derive(Default)]
pub struct MeasureOptions<'a> {
    pub output: Option<&'a str>,
    pub instrumented_output: Option<&'a str>,
    pub superseding_output: Option<&'a str>,
    pub new_discovery_id: Option<&'a str>,
    pub supersedes_ref: Option<&'a str>,
    pub provenance_event_ref: Option<&'a str>,
    pub timeout_seconds: f64,
}
fn identifier(value: &str, prefix: &str) -> Result<()> {
    let suffix = value
        .strip_prefix(prefix)
        .ok_or_else(|| invalid("discovery/event identifier prefix differs"))?;
    require(
        !suffix.is_empty()
            && suffix.split(['.', '-']).all(|p| {
                !p.is_empty()
                    && p.bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            }),
        "invalid discovery/event identifier",
    )
}
fn now() -> Result<String> {
    crate::kag_downstream_status::observation_time()
}
struct ProbeProcess(std::process::Child);
impl Drop for ProbeProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn probe(url: &str, timeout: f64, cancel: &AtomicI32) -> Result<Value> {
    require(
        http_url(url, true),
        "probe requires credential-free HTTP(S)",
    )?;
    let started_at = now()?;
    let started = Instant::now();
    let timeout_string = timeout.to_string();
    let child = Command::new("/usr/bin/curl")
        .args([
            "-q",
            "--silent",
            "--include",
            "--suppress-connect-headers",
            "--location",
            "--max-redirs",
            "10",
            "--max-time",
            &timeout_string,
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--user-agent",
            "Tree-of-Sophia-open-work-discovery/1.0 (+source-research)",
            "--url",
            url,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut process = ProbeProcess(child);
    let mut pipe = process
        .0
        .stdout
        .take()
        .ok_or_else(|| invalid("HTTP probe pipe absent"))?;
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    require(
        flags >= 0 && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } >= 0,
        "cannot bound HTTP probe pipe",
    )?;
    let mut header = vec![];
    let mut response = None;
    let mut observed = 0usize;
    let mut header_bytes = 0usize;
    let mut failed = false;
    let mut completed = false;
    let deadline = started + Duration::from_secs_f64(timeout + 0.5);
    while !completed {
        require(
            cancel.load(Ordering::Relaxed) == 0,
            "discovery timing cancelled",
        )?;
        if Instant::now() >= deadline {
            failed = true;
            break;
        }
        let mut block = [0u8; 4096];
        match pipe.read(&mut block) {
            Ok(0) => {
                let status = process.0.wait()?;
                failed = !status.success();
                break;
            }
            Ok(n) => {
                for byte in &block[..n] {
                    if response.is_some() {
                        observed += 1;
                        if observed == LIMIT {
                            completed = true;
                            break;
                        }
                    } else {
                        header_bytes += 1;
                        require(header_bytes <= 64 * 1024, "HTTP probe header budget")?;
                        header.push(*byte);
                        if header.ends_with(b"\r\n\r\n") || header.ends_with(b"\n\n") {
                            let text = String::from_utf8_lossy(&header);
                            let first = text
                                .lines()
                                .next()
                                .ok_or_else(|| invalid("HTTP status line absent"))?;
                            let mut parts = first.split_ascii_whitespace();
                            require(
                                parts.next().is_some_and(|p| p.starts_with("HTTP/")),
                                "invalid HTTP status line",
                            )?;
                            let status = parts
                                .next()
                                .ok_or_else(|| invalid("HTTP status absent"))?
                                .parse::<u16>()
                                .map_err(io::Error::other)?;
                            require((100..=599).contains(&status), "invalid HTTP status")?;
                            let redirect = (300..400).contains(&status)
                                && text.lines().skip(1).any(|l| {
                                    l.split_once(':').is_some_and(|(k, v)| {
                                        k.eq_ignore_ascii_case("location") && !v.trim().is_empty()
                                    })
                                });
                            if status >= 200 && !redirect {
                                response = Some(status);
                            }
                            header.clear();
                        }
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => {
                failed = true;
                break;
            }
        }
    }
    // End the measured interval before stopping the unused remainder of a response.
    let elapsed = (started.elapsed().as_secs_f64() * 1_000_000.)
        .round()
        .max(1.)
        / 1_000_000.;
    let ended_at = now()?;
    drop(pipe);
    drop(process);
    let outcome = if failed || response.is_none() {
        observed = 0;
        "transport-error"
    } else if response.is_some_and(|s| !(200..300).contains(&s)) {
        "http-error"
    } else {
        "success"
    };
    Ok(
        json!({"method":"monotonic-http-request-v1","clock":CLOCK,"timing_scope":"request-through-first-16384-response-bytes","probe_url":url,"started_at":started_at,"ended_at":ended_at,"elapsed_seconds":elapsed,"outcome":outcome,"http_status":response,"response_bytes_observed":observed}),
    )
}
fn instrument(source: &Value, receipt: &Value) -> Result<Value> {
    let mut out = source.clone();
    let measurements: BTreeMap<&str, &Value> = arr(&receipt["measurements"])?
        .iter()
        .map(|v| Ok((req(v, "channel_id")?, &v["measurement"])))
        .collect::<Result<_>>()?;
    for channel in out["channels"]
        .as_array_mut()
        .ok_or_else(|| invalid("discovery channels absent"))?
    {
        let m = measurements
            .get(req(channel, "channel_id")?)
            .ok_or_else(|| invalid("channel measurement absent"))?;
        channel["queried_at"] = m["started_at"].clone();
        channel["elapsed_seconds"] = m["elapsed_seconds"].clone();
    }
    for row in out["channel_comparison"]
        .as_array_mut()
        .ok_or_else(|| invalid("discovery comparison absent"))?
    {
        let m = measurements
            .get(req(row, "channel_id")?)
            .ok_or_else(|| invalid("comparison measurement absent"))?;
        row["machine_seconds"] = m["elapsed_seconds"].clone();
        let mut notes = text(&row["notes"]);
        for delimiter in [
            " Timer sentinel",
            " Timing sentinel",
            " Per-channel timers",
            " Zero timing",
        ] {
            notes = notes.split(delimiter).next().unwrap_or("");
        }
        row["notes"] = json!(format!(
            "{notes} Machine time is the automatic monotonic HTTP measurement; human_minutes=0 means no real-human review was performed."
        ));
    }
    let rows = arr(&receipt["measurements"])?;
    require(!rows.is_empty(), "no channel measurements")?;
    out["started_at"] = rows[0]["measurement"]["started_at"].clone();
    out["ended_at"] = rows[rows.len() - 1]["measurement"]["ended_at"].clone();
    Ok(out)
}
pub fn measure(
    root: &Path,
    discovery: &str,
    options: MeasureOptions<'_>,
    cancel: &AtomicI32,
) -> Result<Value> {
    let mut repo = Repo::new(root, cancel)?;
    require(
        options.timeout_seconds.is_finite()
            && options.timeout_seconds > 0.
            && options.timeout_seconds <= 600.,
        "probe timeout must be within (0,600] seconds",
    )?;
    require(
        options.instrumented_output.is_none() || options.superseding_output.is_none(),
        "instrumented and superseding outputs are mutually exclusive",
    )?;
    let source_path = repo.path(discovery)?;
    let mut paths = BTreeSet::from([source_path]);
    for path in [
        options.output,
        options.instrumented_output,
        options.superseding_output,
    ]
    .into_iter()
    .flatten()
    {
        require(
            paths.insert(repo.path(path)?),
            "timing outputs must not alias discovery or each other",
        )?;
    }
    let source = repo.json(discovery)?;
    let original_id = req(&source, "discovery_id")?;
    identifier(original_id, "tos.discovery.")?;
    let new_id = if options.superseding_output.is_some() {
        require(
            options.supersedes_ref == Some(original_id),
            "supersedes_ref must match input discovery_id",
        )?;
        let id = options
            .new_discovery_id
            .ok_or_else(|| invalid("new-discovery-id is required"))?;
        identifier(id, "tos.discovery.")?;
        identifier(
            options
                .provenance_event_ref
                .ok_or_else(|| invalid("provenance-event-ref is required"))?,
            "tos.event.",
        )?;
        id
    } else {
        require(
            options.new_discovery_id.is_none()
                && options.supersedes_ref.is_none()
                && options.provenance_event_ref.is_none(),
            "superseding identifiers require superseding-output",
        )?;
        original_id
    };
    let channels = arr(&source["channels"])?;
    require(
        !channels.is_empty() && channels.len() <= 1024,
        "discovery requires 1..1024 channels",
    )?;
    let mut seen = Set::new();
    for channel in channels {
        require(
            seen.insert(req(channel, "channel_id")?.to_owned()),
            "duplicate discovery channel",
        )?;
        require(
            http_url(req(channel, "endpoint_url")?, true),
            "probe requires credential-free HTTP(S)",
        )?;
    }
    if options.instrumented_output.is_some() || options.superseding_output.is_some() {
        let comparisons = arr(&source["channel_comparison"])?;
        let mut compared = Set::new();
        for row in comparisons {
            require(
                compared.insert(req(row, "channel_id")?.to_owned()) && row["notes"].is_string(),
                "invalid discovery channel comparison",
            )?;
        }
        require(
            compared == seen,
            "comparison does not cover exact measured channels",
        )?;
    }
    let mut measurements = vec![];
    for channel in channels {
        measurements.push(json!({"channel_id":channel["channel_id"],"measurement":probe(req(channel,"endpoint_url")?,options.timeout_seconds,cancel)?}));
    }
    let reference = options
        .superseding_output
        .or(options.instrumented_output)
        .unwrap_or(discovery);
    let receipt = json!({"$schema":"https://tree-of-sophia.local/ToS/contracts/open-work-channel-timing-receipt.schema.json","schema_version":"tos_open_work_channel_timing_receipt_v1","timing_id":format!("open-work-channel-timing.{}",new_id.strip_prefix("tos.discovery.").unwrap()),"discovery_ref":reference,"discovery_id":new_id,"measured_at":now()?,"measurements":measurements,"claim_limit":"This receipt measures monotonic transport time through the first 16384 response bytes.","record_version":1});
    let derived = if let Some(path) = options.superseding_output.or(options.instrumented_output) {
        let mut value = instrument(&source, &receipt)?;
        if options.superseding_output.is_some() {
            value["discovery_id"] = json!(new_id);
            value["provenance_event_refs"] = json!([options.provenance_event_ref.unwrap()]);
            value["supersedes_discovery_ref"] = json!(original_id);
            value["record_version"] = json!(
                source["record_version"]
                    .as_i64()
                    .unwrap_or(1)
                    .checked_add(1)
                    .ok_or_else(|| invalid("record version overflow"))?
            );
        }
        Some((path, value))
    } else {
        None
    };
    require(
        cancel.load(Ordering::Relaxed) == 0,
        "discovery timing cancelled",
    )?;
    if let Some((path, value)) = derived {
        crate::route_cards::write_output(root, Path::new(path), &render(&value)?)?;
    }
    if let Some(path) = options.output {
        crate::route_cards::write_output(root, Path::new(path), &render(&receipt)?)?;
    }
    Ok(receipt)
}
