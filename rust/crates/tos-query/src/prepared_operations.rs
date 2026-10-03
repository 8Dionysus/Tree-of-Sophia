//! Remaining local prepared consumers of the maintained query domain.
//! These adapters own physical selected reads only; caller custody/currentness
//! and the existing semantic kernels retain their respective authority.
use crate::compressed_search_sqlite::Read;
use crate::prepared_inspect::{budget, codec, corrupt, lookup, storage_error};
use crate::search_v2::{SearchKind, SearchV2Error, SearchV2ErrorCode};
use tos_compiler::local_prepared::PreparedReadTransaction;
use tos_foundation::{JsonValue, emit_python_compact_json};
type Result<T> = std::result::Result<T, SearchV2Error>;

fn admit_request(request: &JsonValue, read: &Read<'_>) -> Result<()> {
    emit_python_compact_json(request, codec(65_536.min(read.limits.max_row_bytes)))
        .map_err(|_| budget())?;
    read.check_abort().map_err(storage_error)
}
pub(crate) fn temporal(
    read: &mut Read<'_>,
    view: &PreparedReadTransaction<'_>,
    request: &JsonValue,
) -> Result<JsonValue> {
    admit_request(request, read)?;
    let revision = view
        .top()
        .object_get("source_revision")
        .and_then(JsonValue::as_str)
        .ok_or_else(corrupt)?;
    let output_limits = codec(read.limits.max_response_bytes);
    let field_cap = 65_536.min(read.limits.max_row_bytes);
    // This is the existing portable prepared/published Claim profile. The
    // descriptor-selected native owner adapter remains a separate consumer.
    let packet = crate::compare_temporal_operands(
        revision,
        request,
        "source-claims",
        |identifier| {
            read.check_abort().map_err(storage_error)?;
            lookup(read, SearchKind::Nodes, "id", identifier, 1, field_cap)
        },
        output_limits,
    )?;
    emit_python_compact_json(&packet, output_limits).map_err(|_| budget())?;
    read.check_abort().map_err(storage_error)?;
    Ok(packet)
}
pub(crate) fn focus_spec(
    read: &Read<'_>,
    request: &crate::knowledge_focus::KnowledgeFocusRequest,
) -> Result<JsonValue> {
    read.check_abort().map_err(storage_error)?;
    let cap = 65_536.min(read.limits.max_row_bytes);
    let bytes = request
        .sources
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .chain(request.predicate_ids.iter())
        .try_fold(request.node_id.len(), |sum, item| {
            sum.checked_add(item.len()).filter(|n| *n <= cap)
        })
        .ok_or_else(budget)?;
    if bytes > cap {
        return Err(budget());
    }
    let spec = crate::knowledge_focus::focus_lens_spec(
        request,
        &crate::knowledge_lens_spec::LensVocabulary::published_shape(),
    )?;
    emit_python_compact_json(&spec, codec(cap)).map_err(|_| budget())?;
    read.check_abort().map_err(storage_error)?;
    Ok(spec)
}
pub(crate) fn stored_spec(catalog: &JsonValue, identifier: &str) -> Result<JsonValue> {
    // An admitted <=128-codepoint valid UTF8 identifier can never exceed512B.
    // Avoid constructing an unbounded JsonString before the maintained parser.
    if identifier.len() > 512 {
        return Err(SearchV2Error {
            code: SearchV2ErrorCode::InvalidRequest,
            message: "invalid stored lens identifier",
        });
    }
    let identifier = crate::knowledge_lens_spec::validate_stored_lens_identifier(
        &crate::compressed_search_state::string(identifier),
    )?;
    crate::knowledge_lens_spec::stored_lens_spec(catalog, &identifier)
}
