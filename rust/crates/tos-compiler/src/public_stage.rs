use crate::{
    Error, Result,
    knowledge_stage::{KnowledgeStage, WritePhase},
    public_capture::{MAX_ROW_BYTES, PublicCapture, compact},
    public_rows::portable,
};
use rusqlite::params;
use std::path::Path;
use tos_foundation::Digest256;

pub(crate) fn portabilize_public_stage(
    stage: &mut KnowledgeStage<'_>,
    capture: &PublicCapture,
    root: &Path,
) -> Result<()> {
    if !stage.public_build() {
        return Err(Error::Invalid(
            "public D1 path adaptation on selected stage",
        ));
    }
    let root = root
        .to_str()
        .ok_or(Error::Invalid("public D1 root UTF-8"))?;
    let creation = stage.owned_creation_state();
    if let Some(owner) = creation {
        for relation in [false, true] {
            let mut after = -1;
            loop {
                let mut next = None;
                stage.with_write_page(
                    WritePhase::Normalized,
                    1,
                    MAX_ROW_BYTES as u64,
                    |stage| {
                        let (_, cursor) = stage.with_normalized_rows_owned(
                            relation,
                            after,
                            1,
                            MAX_ROW_BYTES,
                            |stage, metadata, raw, source| {
                                capture.charge_work(raw.len() as u64)?;
                                let mut value = crate::public_capture::foundation_scoped(
                                    raw,
                                    MAX_ROW_BYTES,
                                    Some(owner),
                                )?;
                                portable(&mut value, root);
                                owner.with_foundation_compact_bytes(
                                    &value,
                                    MAX_ROW_BYTES,
                                    raw.len(),
                                    |bytes| {
                                        capture.charge_work(bytes.len() as u64)?;
                                        if bytes != raw {
                                            if relation {
                                                stage.replace_relation_logical_payload_if_current(
                                                    metadata.id,
                                                    bytes,
                                                    source,
                                                    Some(metadata.logical_digest),
                                                )?;
                                            } else {
                                                stage.replace_node_logical_payload_if_current(
                                                    metadata.id,
                                                    bytes,
                                                    source,
                                                    Some(metadata.logical_digest),
                                                )?;
                                            }
                                        }
                                        Ok(())
                                    },
                                )
                            },
                        )?;
                        next = cursor;
                        Ok(())
                    },
                )?;
                match next {
                    Some(next) => after = next,
                    None => break,
                }
            }
        }
        return Ok(());
    }
    for table in ["knowledge_nodes", "knowledge_relations"] {
        let mut after = -1i64;
        loop {
            let (page, _page_hold) = stage.with_connection(
                WritePhase::Normalized,
                |db| {
                    let mut statement = db.prepare(&format!(
                        "SELECT source_order,id,payload FROM {table} WHERE source_order>?1 ORDER BY source_order LIMIT 8"
                    ))?;
                    let mut rows = statement.query([after])?;
                    let mut bytes = 8 * std::mem::size_of::<(i64, String, Vec<u8>)>();
                    while let Some(row) = rows.next()? {
                        let id = row.get_ref(1)?.as_str().map_err(|_| Error::Invalid("public D1 path row id"))?;
                        let raw = row.get_ref(2)?.as_blob().map_err(|_| Error::Invalid("public D1 path row payload"))?;
                        if raw.len() > MAX_ROW_BYTES { return Err(Error::Budget("public D1 normalized row bytes")); }
                        bytes = bytes.checked_add(id.len()).and_then(|n| n.checked_add(raw.len()))
                            .ok_or(Error::Budget("public D1 path page state"))?;
                    }
                    drop(rows);
                    let hold = creation.map(|owner| owner.hold(bytes)).transpose()?;
                    let mut page = Vec::with_capacity(8);
                    let mut rows = statement.query([after])?;
                    while let Some(row) = rows.next()? {
                        if page.len() >= 8 { return Err(Error::Invalid("public D1 path page changed")); }
                        page.push((row.get::<_,i64>(0)?, row.get::<_,String>(1)?, row.get::<_,Vec<u8>>(2)?));
                    }
                    Ok((page, hold))
                },
            )?;
            if page.is_empty() {
                break;
            }
            let _changed_hold = creation
                .map(|owner| {
                    owner.hold(
                        8 * (std::mem::size_of::<(String, Vec<u8>)>()
                            + std::mem::size_of::<crate::public_capture::CreationStateHold<'_, '_>>(
                            )),
                    )
                })
                .transpose()?;
            let mut changed = Vec::with_capacity(8);
            let mut changed_holds = Vec::with_capacity(8);
            let mut page_bytes = 0u64;
            for (position, id, raw) in page {
                if position <= after || raw.len() > MAX_ROW_BYTES {
                    return Err(Error::Invalid("public D1 normalized row order/bytes"));
                }
                after = position;
                capture.charge_work(raw.len() as u64)?;
                let mut value =
                    crate::public_capture::foundation_scoped(&raw, MAX_ROW_BYTES, creation)?;
                portable(&mut value, root);
                let mut keep = |bytes: &[u8]| -> Result<()> {
                    capture.charge_work(bytes.len() as u64)?;
                    if bytes != raw {
                        page_bytes = page_bytes
                            .checked_add(bytes.len() as u64)
                            .filter(|total| *total <= 64 * 1024 * 1024)
                            .ok_or(Error::Budget("public D1 normalized page bytes"))?;
                        let hold = creation
                            .map(|owner| owner.hold(bytes.len() + id.len()))
                            .transpose()?;
                        changed_holds.push(hold);
                        changed.push((id.clone(), bytes.to_vec()));
                    }
                    Ok(())
                };
                match creation {
                    Some(owner) => owner.with_foundation_compact_bytes(
                        &value,
                        MAX_ROW_BYTES,
                        raw.len(),
                        &mut keep,
                    )?,
                    None => keep(&compact(&value, MAX_ROW_BYTES)?)?,
                }
            }
            if !changed.is_empty() {
                stage.with_write_page(WritePhase::Normalized, 8, page_bytes, |stage| {
                    stage.charge_materialized(changed.len() as u64, page_bytes)?;
                    stage.with_connection(WritePhase::Normalized, |db| {
                        let mut statement = db.prepare(&format!(
                            "UPDATE {table} SET payload=?1,payload_len=?2,payload_sha256=?3 WHERE id=?4"
                        ))?;
                        for (id, bytes) in &changed {
                            let digest = Digest256::of_bytes(bytes);
                            let count = statement.execute(params![
                                bytes,
                                bytes.len() as i64,
                                digest.as_bytes().as_slice(),
                                id
                            ])?;
                            if count != 1 {
                                return Err(Error::Invalid("public D1 normalized row changed"));
                            }
                        }
                        Ok(())
                    })
                })?;
            }
        }
    }
    Ok(())
}
