//! Explicit mechanical read/restore of an immutable corpus carrier.
//! Native-v4 admission emits the same canonical snapshot-v1 carrier. This
//! consumer never selects another admission backend or grants current use.
use super::{absolute, capped, digest, exact, text};
use crate::source_admission_restore::{RestoreCommittedRefusal, RestoreLimits, restore_guarded};
use crate::source_admission_store::AdmissionStore;
use crate::source_command::{SourceCommandError as E, SourceCommandResult};
use crate::source_serialization::observe_executable;
use serde_json::{Value, json};
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{JsonLimits, SourceRevision};
use tos_source_store::{PinnedSqliteIoBudget, ReadLimits, Selector};

const MAX_MANIFEST_BYTES: u64 = 4_194_304;
const MAX_MANIFEST_ENTRIES: u64 = 32_768;
// State follows the same resident forecast as the admitted manifest profile.
// Each invocation still selects its own smaller bound under the outer quota.
const MAX_RETAINED_STATE_BYTES: u64 = 16 * MAX_MANIFEST_BYTES + 256 * MAX_MANIFEST_ENTRIES;

fn refuse(_: impl std::fmt::Debug) -> E {
    E::Invalid("corpus consumer mechanical verification")
}
fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
        return Err(E::Invalid("corpus consumer deadline or cancellation"));
    }
    Ok(())
}
/// The caller already read one protected invocation with the native strict
/// duplicate-key parser. The schema contains no owner_config or grant token.
pub(super) fn run(
    invocation: &Value,
    mut input: impl Read,
    deadline: Instant,
    cancelled: &AtomicBool,
    invocation_bytes: u64,
    invocation_fence: &dyn Fn() -> SourceCommandResult<()>,
) -> SourceCommandResult<Value> {
    let restore =
        text(invocation, "schema_version")? == "tos_local_native_corpus_restore_invocation_v1";
    let mut keys = vec![
        "schema_version",
        "native_executable_sha256",
        "corpus_store",
        "source_revision",
        "expected_current_revision",
        "budgets",
    ];
    if restore {
        keys.push("output");
    }
    exact(invocation, &keys)?;
    // No hidden request body or authority upgrade is accepted.
    let mut sentinel = [0u8; 1];
    if input.read(&mut sentinel).map_err(refuse)? != 0 {
        return Err(E::Invalid("corpus consumer does not accept stdin requests"));
    }
    let budgets = invocation
        .get("budgets")
        .ok_or(E::Invalid("corpus consumer budgets"))?;
    exact(
        budgets,
        &[
            "max_read_bytes",
            "max_write_bytes",
            "max_manifest_bytes",
            "max_manifest_entries",
            "max_object_bytes",
            "max_directories",
            "max_state_bytes",
            "max_json_depth",
            "max_json_visits",
            "max_integer_digits",
            "max_elapsed_ms",
        ],
    )?;
    let read_cap = capped(budgets, "max_read_bytes", 1_342_177_280)?;
    let write_cap = capped(budgets, "max_write_bytes", 268_435_456)?;
    let manifest_cap = capped(budgets, "max_manifest_bytes", MAX_MANIFEST_BYTES)? as usize;
    let max_entries = capped(budgets, "max_manifest_entries", MAX_MANIFEST_ENTRIES)? as usize;
    let object_cap = capped(budgets, "max_object_bytes", 16_777_216)?;
    let max_directories = capped(budgets, "max_directories", 2048)? as usize;
    let max_state = capped(budgets, "max_state_bytes", MAX_RETAINED_STATE_BYTES)? as usize;
    let millis = capped(budgets, "max_elapsed_ms", 60_000)?;
    let deadline = deadline.min(Instant::now() + std::time::Duration::from_millis(millis));
    // Conservative declared retained-state forecast for bounded JSON, snapshot
    // indexes and restore's second manifest projection. This is not a hard RSS
    // allocator bound; the separately admitted outer memory limit still owns it.
    let retained = manifest_cap
        .checked_mul(16)
        .and_then(|n| {
            max_entries
                .checked_mul(256)
                .and_then(|rows| n.checked_add(rows))
        })
        .filter(|n| *n <= max_state)
        .ok_or(E::Invalid(
            "corpus resident state reservation exceeds invocation",
        ))?;
    let _ = retained;
    let limits = ReadLimits {
        max_manifest_bytes: manifest_cap,
        max_manifest_entries: max_entries,
        max_selected_object_bytes: object_cap,
        json: JsonLimits {
            max_bytes: manifest_cap,
            max_depth: capped(budgets, "max_json_depth", 16)? as usize,
            max_visits: capped(budgets, "max_json_visits", 131_072)? as usize,
            max_integer_digits: capped(budgets, "max_integer_digits", 4300)? as usize,
        },
    };
    let root = absolute(text(invocation, "corpus_store")?)?;
    let revision = SourceRevision(digest(text(invocation, "source_revision")?)?);
    let expected_current = SourceRevision(digest(text(invocation, "expected_current_revision")?)?);
    let ledger = PinnedSqliteIoBudget::new(read_cap, write_cap).map_err(refuse)?;
    // Existing bounded protected invocation bootstrap read is charged now;
    // reserve both terminal guard rereads before any data or image work.
    ledger
        .charge_read(
            invocation_bytes
                .checked_add(1)
                .and_then(|n| n.checked_mul(3))
                .and_then(|n| n.checked_add(1))
                .ok_or(E::Invalid("corpus control charge overflow"))?,
        )
        .map_err(refuse)?;
    let store = AdmissionStore::open_existing(&root, deadline, cancelled).map_err(refuse)?;
    let reader = store.reader(limits).map_err(refuse)?;
    let current = reader
        .select_current_budgeted(&ledger, deadline, cancelled)
        .map_err(refuse)?;
    if current != Some(expected_current) {
        return Err(E::Conflict("corpus current revision changed"));
    }
    let image_bytes = std::fs::metadata("/proc/self/exe").map_err(refuse)?.len();
    if image_bytes == 0 || image_bytes > 536_870_912 {
        return Err(E::Invalid("corpus consumer executable length"));
    }
    ledger
        .charge_read(
            image_bytes
                .checked_add(1)
                .ok_or(E::Invalid("image charge overflow"))?,
        )
        .map_err(refuse)?;
    let observed = observe_executable(deadline, cancelled)?;
    let fence = || {
        active(deadline, cancelled)?;
        store.verify_layout().map_err(refuse)?;
        if reader
            .select_current_budgeted(&ledger, deadline, cancelled)
            .map_err(refuse)?
            != current
        {
            return Err(E::Conflict(
                "corpus current revision changed during consumer",
            ));
        }
        if observed.current_digest(deadline, cancelled)?
            != digest(text(invocation, "native_executable_sha256")?)?
        {
            return Err(E::Conflict("corpus consumer executable changed"));
        }
        invocation_fence()?;
        Ok(())
    };
    if observed.current_digest(deadline, cancelled)?
        != digest(text(invocation, "native_executable_sha256")?)?
    {
        return Err(E::Conflict("corpus consumer executable identity"));
    }
    let receipt = if restore {
        let output = absolute(text(invocation, "output")?)?;
        // The maintained restore has its own cumulative ledger. Partition
        // from this original cap after bootstrap/image/current costs and reserve
        // the remaining two pointer fences (bounded by the reader's envelope).
        let wrapper = ledger.snapshot().read_permitted_bytes;
        let tail = (manifest_cap as u64)
            .checked_add(1)
            .and_then(|n| n.checked_mul(2))
            .ok_or(E::Invalid("corpus restore final fence reserve overflow"))?;
        let restore_read_cap = read_cap
            .checked_sub(wrapper)
            .and_then(|n| n.checked_sub(tail))
            .filter(|n| *n > 0)
            .ok_or(E::Invalid("corpus restore remaining read budget"))?;
        // Reserve the delegated logical read envelope in this same parent ledger
        // before the maintained restore may consume it; callbacks cannot spend it.
        ledger.charge_read(restore_read_cap).map_err(refuse)?;
        let restore_fence =
            || fence().map_err(|_| io::Error::other("corpus current/invocation/executable fence"));
        let manifest = match restore_guarded(
            &store,
            revision.0,
            &output,
            RestoreLimits {
                reader: limits,
                max_read_bytes: restore_read_cap,
                max_write_bytes: write_cap,
                max_directories,
                max_state_bytes: max_state,
            },
            deadline,
            cancelled,
            &restore_fence,
        ) {
            Ok(manifest) => manifest,
            Err(error) => {
                if let Some(committed) = error
                    .get_ref()
                    .and_then(|e| e.downcast_ref::<RestoreCommittedRefusal>())
                {
                    return Ok(
                        json!({"schema_version":"tos_native_corpus_restore_committed_refusal_v1",
                        "status":"RESTORE_OUTPUT_COMMITTED_POSTCHECK_FAILED", "revision":committed.revision.to_hex(),
                        "output":committed.output, "output_dev":committed.output_dev, "output_ino":committed.output_ino,
                        "manifest_sha256":committed.manifest_sha256.to_hex(), "restored_files":committed.restored_files, "reason":committed.reason,
                        "current_use_grant":false, "semantic_admission":false, "rights_change":false}),
                    );
                }
                return Err(refuse(error));
            }
        };
        let files = manifest
            .get("files")
            .and_then(Value::as_array)
            .ok_or(E::Invalid("corpus restore returned manifest"))?;
        json!({"schema_version":"tos_native_corpus_restore_receipt_v1",
            "revision":revision.0.to_hex(), "output":output,
            "restored_files":files.len(), "current_pointer_changed":false,
            "semantic_admission":false, "rights_change":false})
    } else {
        if manifest_cap > max_state {
            return Err(E::Invalid("corpus reader manifest/state envelope"));
        }
        ledger
            .charge_read(manifest_cap as u64 + 1)
            .map_err(refuse)?;
        let snapshot = reader.load_exact(revision).map_err(refuse)?;
        let mut bytes = 0u64;
        for member in snapshot.members() {
            active(deadline, cancelled)?;
            ledger
                .charge_read(
                    member
                        .size_bytes
                        .checked_add(1)
                        .ok_or(E::Invalid("corpus object read reserve overflow"))?,
                )
                .map_err(refuse)?;
            let descriptor = reader
                .resolve(&snapshot, Selector::Path(&member.path))
                .map_err(refuse)?;
            let read = reader
                .read_selected(&snapshot, &descriptor, object_cap, &mut io::sink())
                .map_err(refuse)?;
            if read != member.size_bytes {
                return Err(E::Invalid("corpus reader exact member length"));
            }
            bytes = bytes
                .checked_add(read)
                .ok_or(E::Invalid("corpus reader byte overflow"))?;
        }
        json!({"schema_version":"tos_native_corpus_read_receipt_v1",
            "revision":revision.0.to_hex(), "validator_sha256":snapshot.validator_sha256().to_hex(),
            "verified_files":snapshot.member_count(), "verified_bytes":bytes,
            "identities":snapshot.identity_count(), "current_pointer_changed":false,
            "semantic_admission":false, "rights_change":false, "grants_current_use":false,
            "manifest_read_reserved_bytes":manifest_cap, "object_read_bytes":bytes})
    };
    if !restore {
        fence()?;
    }
    Ok(receipt)
}
