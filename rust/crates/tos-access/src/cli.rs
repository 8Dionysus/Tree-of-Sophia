//! Additive one-shot CLI route for the first native query family.

use std::io::Write;

use crate::common::validate_packet;
use crate::{AccessExecutor, AccessProfile, Params};

/// Exit code: 0 success, 2 request syntax, 3 selected capability unavailable,
/// 1 query/disclosure or output failure. Diagnostics stay on stderr.
pub fn run_cli(
    args: &[String],
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    if args.len() < 3 || args[0] != "source" || args[1] != "descend" {
        let _ = writeln!(
            stderr,
            "usage: tos-access source descend NODE_ID [--max-depth 1..8] [--limit 1..300]"
        );
        return 2;
    }
    let mut max_depth = 8;
    let mut limit = 300;
    let mut at = 3;
    while at < args.len() {
        let Some(value) = args.get(at + 1) else {
            let _ = writeln!(stderr, "missing value for {}", args[at]);
            return 2;
        };
        let parsed = match value.parse::<usize>() {
            Ok(value) => value,
            Err(_) => {
                let _ = writeln!(stderr, "invalid value for {}", args[at]);
                return 2;
            }
        };
        match args[at].as_str() {
            "--max-depth" if (1..=8).contains(&parsed) => max_depth = parsed as u8,
            "--limit" if (1..=300).contains(&parsed) => limit = parsed,
            _ => {
                let _ = writeln!(stderr, "unsupported option or range: {}", args[at]);
                return 2;
            }
        }
        at += 2;
    }
    let params = match Params::new(args[2].clone(), max_depth, limit) {
        Ok(value) => value,
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            return 2;
        }
    };
    if !executor.source_descend_available() {
        let _ = writeln!(
            stderr,
            "source descent unavailable: no owner-selected read model and current-policy fence"
        );
        return 3;
    }
    let mut packet = match executor.source_descend(params) {
        Ok(packet) => packet,
        Err(error) => {
            let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
            return 1;
        }
    };
    if packet.body.len() > profile.max_response_bytes {
        let _ = writeln!(
            stderr,
            "budget_exceeded: source descent response budget exceeded"
        );
        return 1;
    }
    if let Err(error) = validate_packet(&packet.body, profile.max_response_bytes) {
        let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
        return 1;
    }
    if let Err(error) = packet.fence.recheck() {
        let _ = writeln!(stderr, "{}: {}", error.code_str(), error.message);
        return 1;
    }
    let result = stdout
        .write_all(&packet.body)
        .and_then(|_| stdout.write_all(b"\n"))
        .and_then(|_| stdout.flush());
    drop(packet);
    if let Err(error) = result {
        let _ = writeln!(stderr, "output failed: {error}");
        return 1;
    }
    0
}
