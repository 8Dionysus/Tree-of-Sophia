//! Dedicated one-shot process entry for the bounded VAL schema probe.
//! The parent must verify this exact binary and impose process limits before
//! exec. Running this directly has no CPU/memory/time bound and grants nothing.

fn main() {
    if tos_validation::executor::worker_once().is_err() {
        std::process::exit(2);
    }
}
