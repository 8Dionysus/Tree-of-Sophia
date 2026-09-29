//! Published exploration over one held prepared SQLite snapshot and meter.
//! The host stages opaque process checkpoints, then commits them only after
//! the same currentness fence that admits the response. No source authority is issued.
use crate::{
    InspectBudget,
    compressed_search_sqlite::Read,
    exploration_plan::{
        ExplorationInput, ExplorationNeed, ExplorationPlan, ExplorationReply,
        PublishedExplorationBudget,
    },
    knowledge_exploration::{
        self as rules, ExplorationBudget, ExplorationCheckpoint, ExplorationCheckpoints,
        PreparedExplorationCheckpoint,
    },
    prepared_inspect::{budget, codec, corrupt, storage_error},
    search_v2::{SearchKind, SearchV2Error},
};
use tos_compiler::local_prepared::PreparedReadTransaction;
use tos_foundation::{Digest256, JsonValue, emit_python_compact_json};
type Result<T> = std::result::Result<T, SearchV2Error>;

/// The host must stage the exact encoded response before disclosure and commit
/// this preparation only after its final source/currentness check. Dropping an
/// uncommitted preparation releases reservations through its existing owner.
pub struct PreparedExploration {
    pub packet: JsonValue,
    pub checkpoint: Option<Box<dyn PreparedExplorationCheckpoint>>,
}
fn field<'a>(value: &'a JsonValue, key: &str) -> Result<&'a str> {
    value
        .object_get(key)
        .and_then(JsonValue::as_str)
        .ok_or_else(corrupt)
}
fn encode(value: &JsonValue, max: usize) -> Result<Vec<u8>> {
    emit_python_compact_json(value, codec(max)).map_err(|_| budget())
}
/// Actual prepared exploration limits, shared with software capability discovery.
pub fn exploration_budget(
    limits: tos_compiler::local_prepared::PreparedReadLimits,
) -> ExplorationBudget {
    let inspect = InspectBudget {
        max_open_vm_steps: limits.max_vm_steps,
        max_read_vm_steps: limits.max_vm_steps,
        max_matches: 128,
        max_rows: limits.max_rows as u64,
        max_field_bytes: 65_536.min(limits.max_row_bytes),
        max_payload_bytes: limits.max_row_bytes,
        max_decoded_bytes: limits.max_bytes as u64,
        max_response_bytes: limits.max_response_bytes,
        json: codec(limits.max_row_bytes),
    };
    ExplorationBudget {
        read: inspect,
        max_work_units: 512,
        max_session_nodes: 10_000,
        max_session_relations: 20_000,
        max_state_bytes: 1_000_000,
        max_checkpoint_bytes: 32 * 1024 * 1024,
        max_checkpoints: 128,
    }
}
pub(crate) fn explore(
    read: &mut Read<'_>,
    view: &PreparedReadTransaction<'_>,
    request: &JsonValue,
    checkpoints: &mut dyn ExplorationCheckpoints,
) -> Result<PreparedExploration> {
    let limits = read.limits;
    encode(request, 65_536.min(limits.max_row_bytes))?;
    let cursor = rules::validate_published_exploration_request(request)?;
    let binding = view.binding();
    let top = view.top();
    let source = field(top, "source_revision")?;
    let data = field(binding, "data_revision")?;
    let epoch = binding
        .object_get("publication_epoch")
        .and_then(|v| {
            if let JsonValue::Number(n) = v {
                (n.kind == tos_foundation::JsonNumberKind::Int)
                    .then(|| n.lexeme.parse::<u64>().ok())
                    .flatten()
            } else {
                None
            }
        })
        .ok_or_else(corrupt)?;
    let exploration = exploration_budget(limits);
    let inspect = exploration.read;
    let snapshot = rules::published_exploration_snapshot(data, epoch, inspect.json)?;
    read.check_abort().map_err(storage_error)?;
    let checkpoint = cursor
        .as_deref()
        .map(|c| checkpoints.load(c, &snapshot))
        .transpose()?;
    if let Some(ExplorationCheckpoint::Replay {
        packet,
        packet_sha256,
    }) = &checkpoint
    {
        let bytes = encode(packet, limits.max_response_bytes)?;
        rules::validate_published_exploration_replay(&bytes, codec(limits.max_response_bytes))?;
        if Digest256::of_bytes(&bytes) != *packet_sha256
            || field(packet, "snapshot_revision")? != snapshot
            || field(packet, "source_revision")? != source
        {
            return Err(corrupt());
        }
        // Re-read cached carriers under this operation's held snapshot/meter;
        // a valid cache token does not substitute current source observation.
        for (name, kind) in [
            ("nodes", SearchKind::Nodes),
            ("relations", SearchKind::Relations),
        ] {
            let carriers = packet
                .object_get(name)
                .and_then(JsonValue::as_array)
                .ok_or_else(corrupt)?;
            for old in carriers {
                let id = field(old, "id")?.to_owned();
                let reply = crate::prepared_exploration_rows::produce(
                    read,
                    &ExplorationNeed::Rows {
                        kind,
                        ids: vec![id],
                        allow_missing: false,
                        allow_ambiguous: false,
                    },
                    inspect.max_field_bytes,
                )?;
                let ExplorationReply::Rows { rows, .. } = reply else {
                    return Err(corrupt());
                };
                if rows.len() != 1
                    || rows[0].object_get("content_revision") != old.object_get("content_revision")
                {
                    return Err(corrupt());
                }
            }
        }
        read.check_abort().map_err(storage_error)?;
        return Ok(PreparedExploration {
            packet: packet.clone(),
            checkpoint: None,
        });
    }
    let input = match checkpoint {
        Some(ExplorationCheckpoint::State(state)) => ExplorationInput::Continue(state),
        None => ExplorationInput::Start(request.clone()),
        Some(ExplorationCheckpoint::Replay { .. }) => unreachable!(),
    };
    let boundary = top
        .object_get("authority_boundary")
        .cloned()
        .ok_or_else(corrupt)?;
    let mut plan = ExplorationPlan::published(
        input,
        source,
        data,
        epoch,
        boundary,
        PublishedExplorationBudget {
            exploration,
            max_cache_bytes: (2 * 1024 * 1024).min(limits.max_bytes),
            max_cache_entries: 64.min(limits.max_rows),
        },
        read.abort_handle(),
    )?;
    loop {
        read.check_abort().map_err(storage_error)?;
        if plan.advance()? {
            break;
        }
        let need = plan.need().ok_or_else(corrupt)?;
        let reply =
            crate::prepared_exploration_rows::produce(read, &need, inspect.max_field_bytes)?;
        drop(need);
        plan.resume(reply)?;
    }
    let output = plan.finish()?;
    let mut packet = output.packet;
    let successor = (field(&packet, "status")? == "paused").then_some(&output.state);
    let staged = checkpoints.prepare(
        cursor.as_deref(),
        &snapshot,
        successor,
        &packet,
        exploration,
    )?;
    rules::set_exploration_cursor(&mut packet, staged.next_cursor())?;
    encode(&packet, limits.max_response_bytes)?;
    read.check_abort().map_err(storage_error)?;
    Ok(PreparedExploration {
        packet,
        checkpoint: Some(staged),
    })
}
