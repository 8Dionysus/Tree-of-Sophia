//! Native Lens keysets, using the existing normalized indexes and cold owner.
use super::*;
use rusqlite::types::Value;

#[derive(Clone, Copy)]
pub enum ControlledLensKeySelection<'a> {
    Candidates { kind: ControlledSearchKind, sources_json: &'a str,
        after_source: &'a str, after_position: i64 },
    Identity { identifier: &'a str, sources_json: &'a str, after: &'a str },
    Incident { identifier: &'a str, after: &'a str },
}
pub struct ControlledLensKeyRow {
    pub source_graph: String,
    pub position: u64,
    pub id: String,
}
impl ControlledKnowledgeModel<'_, '_, '_> {
    pub fn with_controlled_lens_keys<E>(&mut self,
        selected: ControlledLensKeySelection<'_>, limit: usize,
        max_field_bytes: usize, max_decoded_bytes: u64, max_vm_steps: u64,
        consume: impl FnOnce(&[ControlledLensKeyRow]) -> std::result::Result<(), E>,
    ) -> Result<std::result::Result<ControlledLegacySearchScan, E>> {
        self.check_pin()?;
        if limit == 0 || limit > i64::MAX as usize || max_field_bytes == 0
            || max_field_bytes > i64::MAX as usize || max_vm_steps == 0 {
            return Err(Error::Budget("controlled Lens keyset admission"));
        }
        let (sql, inputs, positions): (&str, [&str; 3], [i64; 2]) = match selected {
            ControlledLensKeySelection::Candidates { kind, sources_json, after_source, after_position } => {
                let sql = if kind == ControlledSearchKind::Nodes {
                    "SELECT CASE WHEN length(CAST(source_graph AS BLOB))<=?5 THEN source_graph END,source_order,CASE WHEN length(CAST(id AS BLOB))<=?5 THEN id END FROM knowledge_nodes INDEXED BY knowledge_nodes_source_order WHERE source_graph IN (SELECT value FROM json_each(?1)) AND (source_graph,source_order)>(?2,?3) ORDER BY source_graph,source_order LIMIT ?4"
                } else {
                    "SELECT CASE WHEN length(CAST(source_graph AS BLOB))<=?5 THEN source_graph END,source_order,CASE WHEN length(CAST(id AS BLOB))<=?5 THEN id END FROM knowledge_relations INDEXED BY knowledge_relations_source_order WHERE source_graph IN (SELECT value FROM json_each(?1)) AND (source_graph,source_order)>(?2,?3) ORDER BY source_graph,source_order LIMIT ?4"
                };
                (sql, [sources_json, after_source, ""], [after_position, 0])
            }
            ControlledLensKeySelection::Identity { identifier, sources_json, after } => (
                "SELECT '',0,CASE WHEN length(CAST(id AS BLOB))<=?5 THEN id END FROM knowledge_nodes INDEXED BY knowledge_nodes_entity_id WHERE entity_id=?1 AND id>?2 AND source_graph IN (SELECT value FROM json_each(?3)) ORDER BY id LIMIT ?4",
                [identifier, after, sources_json], [0, 0]),
            ControlledLensKeySelection::Incident { identifier, after } => (
                "SELECT '',0,CASE WHEN length(CAST(id AS BLOB))<=?4 THEN id END FROM (SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_from_id WHERE from_id=?1 AND id>?2 ORDER BY id LIMIT ?3) UNION SELECT id FROM (SELECT id FROM knowledge_relations INDEXED BY knowledge_relations_to_id WHERE to_id=?1 AND id>?2 ORDER BY id LIMIT ?3)) ORDER BY id LIMIT ?3",
                [identifier, after, ""], [0, 0]),
        };
        let input_bytes = inputs.iter().try_fold(0usize, |n, text| n.checked_add(text.len())
            .ok_or(Error::Budget("controlled Lens key inputs")))?;
        let row_bound = max_field_bytes.checked_mul(2)
            .and_then(|n| n.checked_add(std::mem::size_of::<ControlledLensKeyRow>()))
            .ok_or(Error::Budget("controlled Lens row bound"))?;
        let forecast = limit.checked_mul(row_bound)
            .and_then(|n| n.checked_add(input_bytes))
            .and_then(|n| n.checked_add(5 * std::mem::size_of::<Value>()))
            .and_then(|n| n.checked_add(std::mem::size_of_val(&consume)))
            .and_then(|n| n.checked_add(tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()))
            .ok_or(Error::Budget("controlled Lens keyset state"))?;
        let _hold = self.context.owned_state().hold(forecast)?;
        self.charge_query_work(input_bytes.checked_add(sql.len())
            .ok_or(Error::Budget("controlled Lens SQL work"))?)?;
        let args = match selected {
            ControlledLensKeySelection::Candidates { .. } => vec![Value::Text(inputs[0].into()),
                Value::Text(inputs[1].into()), Value::Integer(positions[0]),
                Value::Integer(limit as i64), Value::Integer(max_field_bytes as i64)],
            ControlledLensKeySelection::Identity { .. } => vec![Value::Text(inputs[0].into()),
                Value::Text(inputs[1].into()), Value::Text(inputs[2].into()),
                Value::Integer(limit as i64), Value::Integer(max_field_bytes as i64)],
            ControlledLensKeySelection::Incident { .. } => vec![Value::Text(inputs[0].into()),
                Value::Text(inputs[1].into()), Value::Integer(limit as i64), Value::Integer(max_field_bytes as i64)],
        };
        let mut rows = Vec::new();
        rows.try_reserve_exact(limit).map_err(|_| Error::Budget("controlled Lens keyset allocation"))?;
        let mut decoded = 0u64;
        let (_, vm) = crate::knowledge_payload_read::with_query_vm_window(self.context,
            &self.connection, max_vm_steps, || {
                let mut statement = self.connection.prepare(sql)?;
                let mut cursor = statement.query(rusqlite::params_from_iter(args.iter()))?;
                while let Some(row) = cursor.next()? {
                    self.check_pin()?;
                    let source = row.get::<_, Option<String>>(0)?
                        .ok_or(Error::Budget("controlled Lens source field"))?;
                    let position = u64::try_from(row.get::<_, i64>(1)?)
                        .map_err(|_| Error::Invalid("controlled Lens source order"))?;
                    let id = row.get::<_, Option<String>>(2)?
                        .ok_or(Error::Budget("controlled Lens id field"))?;
                    let bytes = source.len().checked_add(id.len()).and_then(|n| n.checked_add(8))
                        .ok_or(Error::Budget("controlled Lens key decoded"))?;
                    decoded = decoded.checked_add(bytes as u64)
                        .filter(|n| *n <= max_decoded_bytes).ok_or(Error::Budget("controlled Lens decoded cap"))?;
                    self.charge_query_work(bytes)?;
                    rows.push(ControlledLensKeyRow { source_graph: source, position, id });
                }
                Ok(())
            })?;
        self.check_pin()?;
        let consumed = consume(&rows);
        self.check_pin()?;
        Ok(consumed.map(|_| ControlledLegacySearchScan {
            rows: rows.len() as u64, decoded_bytes: decoded, vm_steps: vm }))
    }

    pub fn controlled_lens_scope_count(&mut self, kind: ControlledSearchKind,
        sources_json: &str, max_vm_steps: u64) -> Result<(u64,u64)> {
        self.check_pin()?;
        self.charge_query_work(sources_json.len())?;
        let _hold = self.context.owned_state().hold(
            tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
            .checked_add(std::mem::size_of::<(i64,u64)>()).ok_or(Error::Budget("Lens count frame"))?)?;
        let sql = if kind == ControlledSearchKind::Nodes {
            "SELECT count(*) FROM knowledge_nodes WHERE source_graph IN (SELECT value FROM json_each(?1))"
        } else { "SELECT count(*) FROM knowledge_relations WHERE source_graph IN (SELECT value FROM json_each(?1))" };
        let (count, vm) = crate::knowledge_payload_read::with_query_vm_window(self.context,
            &self.connection, max_vm_steps, || self.connection.query_row(sql, [sources_json],
                |row| row.get::<_,i64>(0)).map_err(Error::from))?;
        self.check_pin()?;
        Ok((u64::try_from(count).map_err(|_| Error::Invalid("Lens count shape"))?, vm))
    }
    pub fn controlled_lens_key_geometry(&mut self, sources_json: &str,
        max_vm_steps: u64) -> Result<(usize, u64)> {
        self.check_pin()?;
        self.charge_query_work(sources_json.len())?;
        let _hold = self.context.owned_state().hold(
            tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())?;
        let (length, vm) = crate::knowledge_payload_read::with_query_vm_window(self.context,
            &self.connection, max_vm_steps, || self.connection.query_row(
                "SELECT coalesce(max(length(CAST(id AS BLOB))),0) FROM (SELECT id,source_graph FROM knowledge_nodes UNION ALL SELECT id,source_graph FROM knowledge_relations) WHERE source_graph IN (SELECT value FROM json_each(?1))",
                [sources_json], |row| row.get::<_,i64>(0)).map_err(Error::from))?;
        self.check_pin()?;
        Ok((usize::try_from(length).map_err(|_| Error::Invalid("Lens id geometry"))?, vm))
    }

}
