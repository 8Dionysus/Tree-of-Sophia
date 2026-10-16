//! Native peer for `tests/source-text-search-wasm-host.mjs`.
//!
//! The host sends the request, selection, and candidate JSON as three compact
//! newline-delimited records. Including the adapter module keeps its byte
//! admissions, fixed semantic budget, and error-code mapping identical to WASM.

use std::io::{self, Read, Write};

#[path = "../src/source_text_search.rs"]
mod source_text_search;

const MAX_STDIN_BYTES: u64 = 9 * 1024 * 1024;

fn line_without_cr(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\r").unwrap_or(line)
}

fn main() {
    let stdin = io::stdin();
    let mut input = Vec::new();
    if let Err(error) = stdin
        .lock()
        .take(MAX_STDIN_BYTES + 1)
        .read_to_end(&mut input)
    {
        let _ = writeln!(
            io::stderr(),
            "native parity host could not read input: {error}"
        );
        std::process::exit(2);
    }
    if input.len() as u64 > MAX_STDIN_BYTES {
        let _ = io::stdout()
            .lock()
            .write_all(b"error:budget_exceeded: native parity packet exceeds byte limit");
        return;
    }

    let mut lines = input.split(|byte| *byte == b'\n');
    let Some(request) = lines.next() else {
        let _ = io::stdout()
            .lock()
            .write_all(b"error:invalid_request: missing request");
        return;
    };
    let Some(selection) = lines.next() else {
        let _ = io::stdout()
            .lock()
            .write_all(b"error:invalid_request: missing selection");
        return;
    };
    let Some(candidates) = lines.next() else {
        let _ = io::stdout()
            .lock()
            .write_all(b"error:invalid_request: missing candidate set");
        return;
    };
    if lines.any(|line| !line.is_empty()) {
        let _ = io::stdout()
            .lock()
            .write_all(b"error:invalid_request: unexpected parity input");
        return;
    }

    let mut stdout = io::stdout().lock();
    match source_text_search::execute_source_text_search_wasm_v1(
        line_without_cr(request),
        line_without_cr(selection),
        line_without_cr(candidates),
    ) {
        Ok(output) => {
            let _ = stdout.write_all(&output);
        }
        Err(error) => {
            let _ = write!(stdout, "error:{error}");
        }
    }
}
