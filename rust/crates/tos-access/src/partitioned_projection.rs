//! Bounded assembly of selected partitioned projection collections.
//!
//! The compiler reader verifies the exact root manifest, member namespace,
//! member digests, row counts, keys, and caller limits. This adapter assembles
//! only collections requested by one access diagnostic.

use std::{io, path::Path, time::Instant};
use tos_compiler::projection_snapshot::{
    ProjectionAccounting, ProjectionLimits, ProjectionSnapshot,
};

pub(crate) fn materialize_collections(
    root_bytes: &[u8],
    root_path: &Path,
    collections: &[&str],
    limits: ProjectionLimits,
    deadline: Instant,
) -> Result<(Vec<u8>, ProjectionAccounting), String> {
    let snapshot = ProjectionSnapshot::new(root_bytes.to_vec(), root_path.to_owned())
        .map_err(|error| format!("partitioned projection root refused: {error}"))?;
    let mut projection = snapshot
        .metadata()
        .map_err(|error| format!("partitioned projection header refused: {error}"))?;
    let mut used = ProjectionAccounting::default();

    for name in collections {
        if Instant::now() >= deadline {
            return Err("partitioned projection deadline exceeded".into());
        }
        let remaining = ProjectionLimits {
            max_opened_parts: limits.max_opened_parts.saturating_sub(used.opened_parts),
            max_read_bytes: limits.max_read_bytes.saturating_sub(
                used.stored_bytes
                    .checked_add(used.decoded_bytes)
                    .ok_or("partitioned projection read accounting overflow")?,
            ),
            max_stored_read_bytes: limits
                .max_stored_read_bytes
                .saturating_sub(used.stored_bytes),
            max_decoded_bytes: limits.max_decoded_bytes.saturating_sub(used.decoded_bytes),
            max_keys: limits.max_keys.saturating_sub(used.keys),
            max_rows: limits.max_rows.saturating_sub(used.keys),
            max_changes: limits.max_changes.saturating_sub(used.changes),
            max_output_bytes: limits.max_output_bytes.saturating_sub(used.output_bytes),
        };
        let materialized = snapshot.read_collection(name, remaining).map_err(|error| {
            format!("partitioned projection collection {name} refused: {error}")
        })?;
        used.opened_parts = add(used.opened_parts, materialized.accounting.opened_parts)?;
        used.stored_bytes = add(used.stored_bytes, materialized.accounting.stored_bytes)?;
        used.decoded_bytes = add(used.decoded_bytes, materialized.accounting.decoded_bytes)?;
        used.keys = add(used.keys, materialized.accounting.keys)?;
        used.changes = add(used.changes, materialized.accounting.changes)?;
        used.output_bytes = add(used.output_bytes, materialized.accounting.output_bytes)?;
        let rows = materialized.rows.into_values().collect();
        let object = projection
            .as_object_mut()
            .ok_or("partitioned projection header must be an object")?;
        object.insert((*name).to_owned(), serde_json::Value::Array(rows));
    }
    if Instant::now() >= deadline {
        return Err("partitioned projection deadline exceeded".into());
    }

    struct Bounded(Vec<u8>, usize);
    impl io::Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.1.saturating_sub(self.0.len()) {
                return Err(io::Error::other(
                    "partitioned projection output budget exceeded",
                ));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output = Bounded(
        Vec::new(),
        limits.max_output_bytes.min(usize::MAX as u64) as usize,
    );
    serde_json::to_writer(&mut output, &projection)
        .map_err(|error| format!("partitioned projection output refused: {error}"))?;
    Ok((output.0, used))
}

fn add(left: u64, right: u64) -> Result<u64, String> {
    left.checked_add(right)
        .ok_or_else(|| "partitioned projection accounting overflow".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use serde_json::{Value, json};
    use std::{fs, io::Write, time::Duration};
    use tos_foundation::Digest256;

    fn write_part(directory: &Path, name: &str, row: &Value) -> Value {
        let decoded = format!("{{\"key\":\"{name}\",\"value\":{row}}}\n");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(decoded.as_bytes()).unwrap();
        let stored = encoder.finish().unwrap();
        let digest = Digest256::of_bytes(&stored).to_hex();
        let relative = format!("graph.parts/{}/{digest}.jsonl.gz", &digest[..2]);
        let path = directory.join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, &stored).unwrap();
        json!({
            "key_field":"id",
            "order_fields":["id"],
            "root":{
                "kind":"data",
                "prefix":"",
                "path":relative,
                "sha256":digest,
                "size_bytes":stored.len(),
                "decoded_bytes":decoded.len(),
                "decoded_sha256":Digest256::of_bytes(decoded.as_bytes()).to_hex(),
                "count":1
            }
        })
    }

    #[test]
    fn assembles_partition_members_when_header_omits_collection_fields() {
        let directory = std::env::temp_dir().join(format!(
            "tos-access-partitioned-projection-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let nodes = write_part(&directory, "node-1", &json!({"id":"node-1"}));
        let relations = write_part(&directory, "relation-1", &json!({"id":"relation-1"}));
        let root = json!({
            "schema_version":"tos_partitioned_projection_v1",
            "logical_schema":"tos_normalized_knowledge_graph_v1",
            "header":{
                "schema_version":"tos_normalized_knowledge_graph_v1",
                "authority_boundary":"fixture",
                "source_revision":"fixture-revision"
            },
            "limits":{"root_bytes":262144,"index_bytes":131072,"part_bytes":8388608,"key_bytes":4096},
            "collections":{"nodes":nodes,"relations":relations}
        });
        let root_bytes = serde_json::to_vec(&root).unwrap();
        let root_path = directory.join("graph.json");
        fs::write(&root_path, &root_bytes).unwrap();
        let (assembled, accounting) = materialize_collections(
            &root_bytes,
            &root_path,
            &["nodes", "relations"],
            ProjectionLimits::default(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&assembled).unwrap();
        assert_eq!(value["nodes"][0]["id"], "node-1");
        assert_eq!(value["relations"][0]["id"], "relation-1");
        assert_eq!(accounting.keys, 2);
        fs::remove_dir_all(directory).unwrap();
    }
}
