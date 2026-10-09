//! Physical byte dictionaries owned by the selected V3 model. They do not
//! identify sources or share row authority. Readers resolve only a frame's
//! exact digest from the same held SQLite connection and original budget.
use crate::knowledge_byte_codec::{DICTIONARY_BYTES, DICTIONARY_WINDOW_BYTES};
use crate::knowledge_stage::KnowledgePayloadLayout;
mod v4;
use crate::{
    Error, Result,
    d1_public_capture::{CreationState, CreationStateHold},
};
use rusqlite::{Connection, types::ValueRef};
use tos_foundation::Digest256;
use tos_source_store::PinnedBoundedStatement;

// Maximum auxiliary work for one source or normalized-row write. The
// collector and published dictionary share the same Stage transaction owner.
pub(crate) const MAX_GRAPH_BYTES: usize = 4096;
pub(crate) const MAX_WRITE_ROWS: usize = 2;
pub(crate) const MAX_WRITE_BYTES: usize = DICTIONARY_BYTES + MAX_GRAPH_BYTES + 8 + 256;

pub(crate) const DDL: &str = "CREATE TABLE knowledge_byte_dictionaries(dictionary_sha256 BLOB PRIMARY KEY NOT NULL CHECK(length(dictionary_sha256)=32),dictionary BLOB NOT NULL CHECK(length(dictionary) BETWEEN 1 AND 4096));";
pub(crate) const PREPARATION_SCHEMA: crate::knowledge_stage::PreparationSchema = crate::knowledge_stage::preparation_schema!(
    table "knowledge_byte_dictionary_pending(dictionary_kind TEXT NOT NULL,source_graph TEXT NOT NULL,samples INTEGER NOT NULL CHECK(samples BETWEEN 1 AND 32),dictionary BLOB NOT NULL CHECK(length(dictionary)<=4096),dictionary_sha256 BLOB CHECK(dictionary_sha256 IS NULL OR length(dictionary_sha256)=32),PRIMARY KEY(dictionary_kind,source_graph)) WITHOUT ROWID"
);

/// A digest established over these immutable dictionary bytes. The view
/// conveys byte integrity only, with no row/source or publication authority.
#[derive(Clone, Copy)]
pub(crate) struct VerifiedDictionary<'a> {
    bytes: &'a [u8],
    digest: Digest256,
}
impl<'a> VerifiedDictionary<'a> {
    pub(crate) fn from_bytes(
        bytes: &'a [u8],
        mut checkpoint: impl FnMut(usize) -> Result<()>,
    ) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > DICTIONARY_WINDOW_BYTES {
            return Err(Error::Invalid("byte dictionary length"));
        }
        checkpoint(bytes.len())?;
        Ok(Self {
            bytes,
            digest: Digest256::of_bytes(bytes),
        })
    }
    pub(crate) fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
    pub(crate) fn digest(self) -> Digest256 {
        self.digest
    }
}
pub(crate) struct OwnedDictionary<'s, 'budget> {
    bytes: Vec<u8>,
    digest: Option<Digest256>,
    _hold: CreationStateHold<'s, 'budget>,
}
impl OwnedDictionary<'_, '_> {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn verified(&self) -> Result<VerifiedDictionary<'_>> {
        Ok(VerifiedDictionary {
            bytes: &self.bytes,
            digest: self
                .digest
                .ok_or(Error::Invalid("unfinished dictionary has no digest"))?,
        })
    }
}
fn sql_error(e: tos_source_store::StoreError) -> Error {
    if e.code == tos_source_store::StoreErrorCode::BudgetExceeded {
        Error::Budget("byte dictionary SQL budget")
    } else {
        Error::Invalid("byte dictionary SQL refusal")
    }
}
fn allocate<'s, 'b>(state: &'s CreationState<'b>) -> Result<OwnedDictionary<'s, 'b>> {
    allocate_with_capacity(state, DICTIONARY_BYTES)
}
fn allocate_with_capacity<'s, 'b>(
    state: &'s CreationState<'b>,
    capacity: usize,
) -> Result<OwnedDictionary<'s, 'b>> {
    let hold = state.hold(capacity + std::mem::size_of::<OwnedDictionary<'s, 'b>>())?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| Error::Budget("byte dictionary allocation"))?;
    if bytes.capacity() != capacity {
        return Err(Error::Budget("byte dictionary capacity"));
    }
    Ok(OwnedDictionary {
        bytes,
        digest: None,
        _hold: hold,
    })
}
fn with_statement<T>(
    db: &Connection,
    state: &CreationState<'_>,
    sql: &std::ffi::CStr,
    consume: impl FnOnce(&mut PinnedBoundedStatement<'_>) -> Result<T>,
) -> Result<T> {
    state.active()?;
    let _hold = state.hold(
        PinnedBoundedStatement::owned_connection_rust_workspace_upper_bound()
            + std::mem::size_of_val(&consume),
    )?;
    state.charge_work(sql.to_bytes().len())?;
    let mut statement =
        PinnedBoundedStatement::prepare_on_owned_connection(db, sql).map_err(sql_error)?;
    let result = consume(&mut statement);
    state.active()?;
    result
}
fn packet<'a>(statement: &'a PinnedBoundedStatement<'_>, column: i32) -> Result<&'a [u8]> {
    match statement.value_ref(column).map_err(sql_error)? {
        ValueRef::Blob(raw) if !raw.is_empty() && raw.len() <= DICTIONARY_BYTES => Ok(raw),
        _ => Err(Error::Invalid("byte dictionary row type/length")),
    }
}

pub(crate) fn read<'s, 'b>(
    db: &Connection,
    state: &'s CreationState<'b>,
    stored: &[u8],
    max_bytes: usize,
) -> Result<Option<OwnedDictionary<'s, 'b>>> {
    if !crate::knowledge_byte_codec::is_dictionary_frame(stored) {
        return Ok(None);
    }
    let (_, digest) =
        crate::knowledge_byte_codec::dictionary_frame_metadata(stored, None, max_bytes)?;
    with_statement(db,state,c"SELECT CASE WHEN typeof(dictionary)='blob' AND length(dictionary) BETWEEN 1 AND 4096 THEN dictionary END FROM knowledge_byte_dictionaries WHERE dictionary_sha256=?1",|statement|{
        statement.bind_blob(1,digest.as_bytes()).map_err(sql_error)?;
        if !statement.step().map_err(sql_error)? {return Err(Error::Invalid("byte dictionary missing"));}
        let raw=packet(statement,0)?;
        state.charge_work(raw.len())?;
        if Digest256::of_bytes(raw)!=digest {return Err(Error::Invalid("byte dictionary digest differs"));}
        let mut owned=allocate(state)?;
        state.charge_work(raw.len())?;owned.bytes.extend_from_slice(raw);owned.digest=Some(digest);
        if statement.step().map_err(sql_error)? {return Err(Error::Invalid("byte dictionary duplicate"));}
        Ok(Some(owned))
    })
}

/// Collect only the first 4 KiB for one producer family. Early rows use V2
/// until the dictionary is sealed. Published dictionaries never change; the
/// unfinished collector is TEMP and is removed before model selection.
pub(crate) fn prepare<'s, 'b>(
    db: &Connection,
    state: &'s CreationState<'b>,
    kind: &str,
    graph: &str,
    raw: &[u8],
) -> Result<(Option<OwnedDictionary<'s, 'b>>, u64, u64)> {
    if !matches!(kind, "source" | "node" | "relation")
        || graph.is_empty()
        || graph.len() > MAX_GRAPH_BYTES
        || raw.is_empty()
    {
        return Err(Error::Invalid("byte dictionary producer family"));
    }
    let published=with_statement(db,state,c"SELECT d.dictionary_sha256,CASE WHEN typeof(d.dictionary)='blob' AND length(d.dictionary) BETWEEN 1 AND 4096 THEN d.dictionary END FROM knowledge_byte_dictionary_pending p JOIN knowledge_byte_dictionaries d ON d.dictionary_sha256=p.dictionary_sha256 WHERE p.dictionary_kind=?1 AND p.source_graph=?2",|statement|{
        statement.bind_text(1,kind).map_err(sql_error)?;statement.bind_text(2,graph).map_err(sql_error)?;
        if !statement.step().map_err(sql_error)? {return Ok(None);}
        let raw=packet(statement,1)?;
        let sha=match statement.value_ref(0).map_err(sql_error)? {ValueRef::Blob(v) if v.len()==32=>v,_=>return Err(Error::Invalid("byte dictionary stored digest"))};
        state.charge_work(raw.len())?;
        let digest=Digest256::of_bytes(raw);
        if digest.as_bytes().as_slice()!=sha {return Err(Error::Invalid("byte dictionary stored bytes"));}
        let mut owned=allocate(state)?;state.charge_work(raw.len())?;owned.bytes.extend_from_slice(raw);owned.digest=Some(digest);
        if statement.step().map_err(sql_error)? {return Err(Error::Invalid("byte dictionary family duplicate"));}
        Ok(Some(owned))
    })?;
    if published.is_some() {
        return Ok((published, 0, 0));
    }
    let mut owned = allocate(state)?;
    let samples=with_statement(db,state,c"SELECT samples,CASE WHEN typeof(dictionary)='blob' AND length(dictionary) BETWEEN 1 AND 4096 THEN dictionary END FROM knowledge_byte_dictionary_pending WHERE dictionary_kind=?1 AND source_graph=?2",|statement|{
        statement.bind_text(1,kind).map_err(sql_error)?;statement.bind_text(2,graph).map_err(sql_error)?;
        if !statement.step().map_err(sql_error)? {return Ok(0);}
        let count=statement.integer(0).map_err(sql_error)?;
        if !(1..=31).contains(&count) {return Err(Error::Invalid("byte dictionary sample count"));}
        let packet=packet(statement,1)?;state.charge_work(packet.len())?;owned.bytes.extend_from_slice(packet);
        if statement.step().map_err(sql_error)? {return Err(Error::Invalid("byte dictionary pending duplicate"));}
        Ok(count)
    })?;
    let next = raw.len().min(DICTIONARY_BYTES - owned.bytes.len());
    state.charge_work(next)?;
    owned.bytes.extend_from_slice(&raw[..next]);
    if owned.bytes.len() == DICTIONARY_BYTES || samples == 31 {
        state.charge_work(owned.bytes.len())?;
        let digest = Digest256::of_bytes(&owned.bytes);
        owned.digest = Some(digest);
        with_statement(db,state,c"INSERT INTO knowledge_byte_dictionaries(dictionary_sha256,dictionary) VALUES (?1,?2) ON CONFLICT(dictionary_sha256) DO NOTHING",|statement|{
            statement.bind_blob(1,digest.as_bytes()).map_err(sql_error)?;statement.bind_blob(2,&owned.bytes).map_err(sql_error)?;
            if statement.step().map_err(sql_error)? {return Err(Error::Invalid("byte dictionary insertion row"));}Ok(())
        })?;
        with_statement(
            db,
            state,
            c"SELECT dictionary FROM knowledge_byte_dictionaries WHERE dictionary_sha256=?1",
            |statement| {
                statement
                    .bind_blob(1, digest.as_bytes())
                    .map_err(sql_error)?;
                if !statement.step().map_err(sql_error)? {
                    return Err(Error::Invalid("byte dictionary inserted row absent"));
                }
                let selected = packet(statement, 0)?;
                state.charge_work(selected.len())?;
                if selected != owned.bytes {
                    return Err(Error::Invalid("byte dictionary digest collision"));
                }
                Ok(())
            },
        )?;
        with_statement(db,state,c"INSERT INTO knowledge_byte_dictionary_pending(dictionary_kind,source_graph,samples,dictionary,dictionary_sha256) VALUES (?1,?2,32,x'',?3) ON CONFLICT(dictionary_kind,source_graph) DO UPDATE SET samples=32,dictionary=x'',dictionary_sha256=excluded.dictionary_sha256",|statement|{
            statement.bind_text(1,kind).map_err(sql_error)?;statement.bind_text(2,graph).map_err(sql_error)?;
            statement.bind_blob(3,digest.as_bytes()).map_err(sql_error)?;
            if statement.step().map_err(sql_error)? {return Err(Error::Invalid("byte dictionary selection row"));}Ok(())
        })?;
        let written = (owned.bytes.len() + kind.len() + graph.len() + 256) as u64;
        Ok((Some(owned), 1 + u64::from(samples == 0), written))
    } else {
        with_statement(db,state,c"INSERT INTO knowledge_byte_dictionary_pending(dictionary_kind,source_graph,samples,dictionary) VALUES (?1,?2,?3,?4) ON CONFLICT(dictionary_kind,source_graph) DO UPDATE SET samples=excluded.samples,dictionary=excluded.dictionary",|statement|{
            statement.bind_text(1,kind).map_err(sql_error)?;statement.bind_text(2,graph).map_err(sql_error)?;
            statement.bind_i64(3,samples+1).map_err(sql_error)?;statement.bind_blob(4,&owned.bytes).map_err(sql_error)?;
            if statement.step().map_err(sql_error)? {return Err(Error::Invalid("byte dictionary collection row"));}Ok(())
        })?;
        Ok((
            None,
            u64::from(samples == 0),
            (owned.bytes.len() + kind.len() + graph.len() + 128) as u64,
        ))
    }
}

pub(crate) fn ddl(layout: KnowledgePayloadLayout) -> &'static str {
    if layout == KnowledgePayloadLayout::CarrierOnceV4 {
        v4::DDL
    } else {
        DDL
    }
}
pub(crate) fn preparation_schema(
    layout: KnowledgePayloadLayout,
) -> crate::knowledge_stage::PreparationSchema {
    if layout == KnowledgePayloadLayout::CarrierOnceV4 {
        v4::PREPARATION_SCHEMA
    } else {
        PREPARATION_SCHEMA
    }
}
pub(crate) fn write_bytes(layout: KnowledgePayloadLayout) -> usize {
    if layout == KnowledgePayloadLayout::CarrierOnceV4 {
        v4::MAX_WRITE_BYTES
    } else {
        MAX_WRITE_BYTES
    }
}
pub(crate) fn read_selected<'s, 'b>(
    db: &Connection,
    state: &'s CreationState<'b>,
    stored: &[u8],
    max_bytes: usize,
    layout: KnowledgePayloadLayout,
) -> Result<Option<OwnedDictionary<'s, 'b>>> {
    if layout == KnowledgePayloadLayout::CarrierOnceV4 {
        v4::read(db, state, stored, max_bytes)
    } else {
        read(db, state, stored, max_bytes)
    }
}
pub(crate) fn prepare_selected<'s, 'b>(
    db: &Connection,
    state: &'s CreationState<'b>,
    kind: &str,
    graph: &str,
    raw: &[u8],
    layout: KnowledgePayloadLayout,
) -> Result<(Option<OwnedDictionary<'s, 'b>>, u64, u64)> {
    if layout == KnowledgePayloadLayout::CarrierOnceV4 {
        v4::prepare(db, state, kind, graph, raw)
    } else {
        prepare(db, state, kind, graph, raw)
    }
}
