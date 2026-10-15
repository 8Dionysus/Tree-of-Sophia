//! Transactional authenticated map for current event-cohort metadata.
//!
//! This PG adapter uses a compressed radix tree over owner-key digests. Exact
//! owner keys stay in leaves; hashing is an index, never an identity rewrite.
//! One mutation reads/writes its paths, a point read proves its selected owners,
//! and a full audit separately checks coverage of the whole current table.
//! Immutable nodes and the root pointer commit in the same PG transaction as
//! the cohort rows and publication cursor. They carry no source/rights authority.

use super::*;
use serde::{Deserialize, Serialize};

const NODE_BYTES: usize = 65_536;
const ROOT_BYTES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeRef {
    prefix: String,
    digest: String,
    count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Node {
    prefix: String,
    key: Option<String>,
    value: Option<String>,
    children: BTreeMap<String, NodeRef>,
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn digest(domain: &str, consumer: &str, tag: &[u8], raw: &[u8]) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    hash.update(tag);
    for part in [domain.as_bytes(), consumer.as_bytes(), raw] {
        navigation_hash_part(&mut hash, part);
    }
    hash.finalize()
}

fn owner_key(path: &str, id: &str) -> DurableResult<String> {
    validate_relative_path(path)?;
    validate_identifier(
        id,
        MAX_NAVIGATION_RECORD_ID_BYTES,
        "navigation cohort tree owner",
    )?;
    serde_json::to_string(&(path, id)).map_err(|_| DurableError::Invalid("cohort tree owner key"))
}

fn key_hash(key: &str) -> String {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-navigation-cohort-key-v1");
    navigation_hash_part(&mut hash, key.as_bytes());
    hash.finalize().to_hex()
}

impl Node {
    fn count(&self) -> DurableResult<u64> {
        if self.prefix.len() > 64 || !is_hex(&self.prefix, self.prefix.len()) {
            return Err(DurableError::Corrupt("cohort tree prefix"));
        }
        match (&self.key, &self.value) {
            (Some(key), Some(value)) if self.children.is_empty() => {
                if self.prefix.len() != 64 || key_hash(key) != self.prefix || !is_hex(value, 64) {
                    return Err(DurableError::Corrupt("cohort tree leaf"));
                }
                Ok(1)
            }
            (None, None) if (2..=16).contains(&self.children.len()) && self.prefix.len() < 64 => {
                self.children.iter().try_fold(0u64, |total, (slot, child)| {
                    if !is_hex(slot, 1)
                        || !is_hex(&child.digest, 64)
                        || child.count == 0
                        || child.prefix.len() <= self.prefix.len()
                        || child.prefix.len() > 64
                        || !is_hex(&child.prefix, child.prefix.len())
                        || !child.prefix.starts_with(&format!("{}{slot}", self.prefix))
                    {
                        return Err(DurableError::Corrupt("cohort tree child"));
                    }
                    total
                        .checked_add(child.count)
                        .ok_or(DurableError::Corrupt("cohort tree count"))
                })
            }
            _ => Err(DurableError::Corrupt("cohort tree shape")),
        }
    }
}

#[derive(Clone)]
pub(super) struct Checkpoint {
    root: Option<NodeRef>,
    work: (u64, u64, u64),
}

impl Checkpoint {
    pub(super) fn work(&self) -> (u64, u64, u64) {
        self.work
    }
    pub(super) fn count(&self) -> u64 {
        self.root.as_ref().map_or(0, |root| root.count)
    }
}

struct Tree<'a, C: GenericClient> {
    tx: &'a mut C,
    domain: &'a str,
    consumer: &'a str,
    limits: DurableNavigationEventLimits,
    bytes: u64,
    nodes: usize,
    probes: u64,
    rows: u64,
    max_probes: u64,
    max_rows: u64,
    deadline: Instant,
    cache: BTreeMap<String, Node>,
}

impl<'a, C: GenericClient> Tree<'a, C> {
    fn new(
        tx: &'a mut C,
        domain: &'a str,
        consumer: &'a str,
        limits: DurableNavigationEventLimits,
    ) -> Self {
        Self {
            tx,
            domain,
            consumer,
            limits,
            bytes: 0,
            nodes: 0,
            probes: 0,
            rows: 0,
            max_probes: (limits
                .max_current_cohorts
                .saturating_add(1)
                .saturating_mul(130)) as u64,
            max_rows: (limits
                .max_current_cohorts
                .saturating_add(1)
                .saturating_mul(130)) as u64,
            deadline: Instant::now() + limits.statement_timeout,
            cache: BTreeMap::new(),
        }
    }

    fn probe(&mut self) -> DurableResult<()> {
        active_navigation(self.deadline)?;
        // Every lookup returns at most one row; reserve it before issuing SQL.
        if self.probes >= self.max_probes || self.rows >= self.max_rows {
            return Err(DurableError::Refused("cohort tree SQL work budget"));
        }
        self.probes += 1;
        Ok(())
    }

    fn charge(&mut self, bytes: usize) -> DurableResult<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes as u64)
            .ok_or(DurableError::Refused("cohort tree byte budget"))?;
        self.nodes += 1;
        // At most 64 compressed nibble levels per changed/selected key, with
        // read and replacement nodes charged separately. This is a call cap.
        if self.bytes > self.limits.max_total_bytes
            || self.nodes
                > self
                    .limits
                    .max_current_cohorts
                    .saturating_add(1)
                    .saturating_mul(130)
        {
            return Err(DurableError::Refused("cohort tree operation budget"));
        }
        Ok(())
    }

    fn read_root(&mut self) -> DurableResult<Checkpoint> {
        self.probe()?;
        let row = self
            .tx
            .query_opt(
                "SELECT CASE WHEN octet_length(root_json)<=$3 THEN root_json END AS root_json
               FROM cmd2_projection_navigation_cohort_tree WHERE domain=$1 AND consumer_id=$2",
                &[&self.domain, &self.consumer, &(ROOT_BYTES as i32)],
            )?
            .ok_or(DurableError::Conflict(
                "navigation cohort tree requires an explicit bootstrap",
            ))?;
        self.rows += 1;
        let raw: String = row
            .get::<_, Option<String>>(0)
            .ok_or(DurableError::Corrupt("cohort tree root size"))?;
        self.charge(raw.len())?;
        let root: Option<NodeRef> =
            serde_json::from_str(&raw).map_err(|_| DurableError::Corrupt("cohort tree root"))?;
        if serde_json::to_string(&root)
            .map_err(|_| DurableError::Corrupt("cohort tree root encoding"))?
            != raw
        {
            return Err(DurableError::Corrupt("cohort tree root encoding"));
        }
        if let Some(root) = &root {
            self.load(root)?;
        }
        Ok(Checkpoint {
            root,
            work: (self.probes, self.rows, self.bytes),
        })
    }

    fn load(&mut self, reference: &NodeRef) -> DurableResult<Node> {
        let node =
            if let Some(node) = self.cache.get(&reference.digest) {
                node.clone()
            } else {
                if !is_hex(&reference.digest, 64) {
                    return Err(DurableError::Corrupt("cohort tree node address"));
                }
                let cap =
                    NODE_BYTES.min(self.limits.max_total_bytes.saturating_sub(self.bytes) as usize);
                self.probe()?;
                let row = self.tx.query_opt(
                "SELECT CASE WHEN octet_length(node_json)<=$4 THEN node_json END AS node_json
                   FROM cmd2_projection_navigation_cohort_tree_node
                  WHERE domain=$1 AND consumer_id=$2 AND node_sha256=$3",
                &[&self.domain, &self.consumer, &reference.digest, &(cap as i32)],
            )?.ok_or(DurableError::Corrupt("cohort tree node absent"))?;
                self.rows += 1;
                let raw: String = row
                    .get::<_, Option<String>>(0)
                    .ok_or(DurableError::Refused("cohort tree node bytes"))?;
                self.charge(raw.len())?;
                if digest(
                    self.domain,
                    self.consumer,
                    b"tos-navigation-cohort-node-v1",
                    raw.as_bytes(),
                )
                .to_hex()
                    != reference.digest
                {
                    return Err(DurableError::Corrupt("cohort tree node digest"));
                }
                let node: Node = serde_json::from_str(&raw)
                    .map_err(|_| DurableError::Corrupt("cohort tree node JSON"))?;
                if serde_json::to_string(&node)
                    .map_err(|_| DurableError::Corrupt("cohort tree node encoding"))?
                    != raw
                {
                    return Err(DurableError::Corrupt("cohort tree node encoding"));
                }
                self.cache.insert(reference.digest.clone(), node.clone());
                node
            };
        if node.prefix != reference.prefix || node.count()? != reference.count {
            return Err(DurableError::Corrupt("cohort tree authenticated summary"));
        }
        Ok(node)
    }

    fn put(&mut self, node: Node) -> DurableResult<NodeRef> {
        let count = node.count()?;
        let raw = serde_json::to_string(&node)
            .map_err(|_| DurableError::Invalid("cohort tree node encoding"))?;
        if raw.len() > NODE_BYTES {
            return Err(DurableError::Refused("cohort tree node size"));
        }
        active_navigation(self.deadline)?;
        self.charge(raw.len())?;
        let reference = NodeRef {
            prefix: node.prefix.clone(),
            count,
            digest: digest(
                self.domain,
                self.consumer,
                b"tos-navigation-cohort-node-v1",
                raw.as_bytes(),
            )
            .to_hex(),
        };
        let inserted = self.tx.execute(
            "INSERT INTO cmd2_projection_navigation_cohort_tree_node(domain,consumer_id,node_sha256,node_json)
             VALUES($1,$2,$3,$4) ON CONFLICT(domain,consumer_id,node_sha256) DO NOTHING",
            &[&self.domain, &self.consumer, &reference.digest, &raw],
        )?;
        if inserted == 0 {
            self.load(&reference)?;
        }
        self.cache.insert(reference.digest.clone(), node);
        Ok(reference)
    }

    fn leaf(&mut self, key: &str, value: String) -> DurableResult<NodeRef> {
        self.put(Node {
            prefix: key_hash(key),
            key: Some(key.into()),
            value: Some(value),
            children: BTreeMap::new(),
        })
    }

    fn lookup(&mut self, root: &Option<NodeRef>, key: &str) -> DurableResult<Option<String>> {
        let hash = key_hash(key);
        let mut next = root.clone();
        let mut prior_depth = None;
        while let Some(reference) = next {
            if prior_depth.is_some_and(|depth| reference.prefix.len() <= depth) {
                return Err(DurableError::Corrupt("cohort tree non-progressing path"));
            }
            let node = self.load(&reference)?;
            if !hash.starts_with(&node.prefix) {
                return Ok(None);
            }
            if let Some(stored_key) = node.key {
                if stored_key != key {
                    return Err(DurableError::Corrupt("cohort tree owner key collision"));
                }
                return Ok(node.value);
            }
            let depth = node.prefix.len();
            next = node.children.get(&hash[depth..depth + 1]).cloned();
            prior_depth = Some(depth);
        }
        Ok(None)
    }

    fn change(
        &mut self,
        root: Option<NodeRef>,
        key: &str,
        value: Option<String>,
        depth: usize,
    ) -> DurableResult<Option<NodeRef>> {
        if depth > 64 {
            return Err(DurableError::Corrupt("cohort tree depth"));
        }
        let Some(reference) = root else {
            return value.map(|value| self.leaf(key, value)).transpose();
        };
        let mut node = self.load(&reference)?;
        let hash = key_hash(key);
        let shared = hash
            .bytes()
            .zip(node.prefix.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        if shared < node.prefix.len() {
            let Some(value) = value else {
                return Ok(Some(reference));
            };
            let leaf = self.leaf(key, value)?;
            let children = BTreeMap::from([
                (node.prefix[shared..shared + 1].into(), reference),
                (hash[shared..shared + 1].into(), leaf),
            ]);
            return self
                .put(Node {
                    prefix: hash[..shared].into(),
                    key: None,
                    value: None,
                    children,
                })
                .map(Some);
        }
        if let Some(stored_key) = node.key.as_ref() {
            if stored_key != key {
                return Err(DurableError::Corrupt("cohort tree owner key collision"));
            }
            if node.value == value {
                return Ok(Some(reference));
            }
            return value.map(|value| self.leaf(key, value)).transpose();
        }
        let slot = &hash[node.prefix.len()..node.prefix.len() + 1];
        let child = node.children.remove(slot);
        if let Some(changed) = self.change(child, key, value, depth + 1)? {
            node.children.insert(slot.into(), changed);
        }
        match node.children.len() {
            0 => Ok(None),
            1 => Ok(node.children.into_values().next()),
            _ => self.put(node).map(Some),
        }
    }

    fn row_digest(&mut self, path: &str, id: &str) -> DurableResult<Option<String>> {
        // PostgreSQL forms bounded metadata only; no source or carrier payload
        // is in this table. Gate its returned JSON before client allocation.
        let cap = self
            .limits
            .max_row_bytes
            .min(self.limits.max_total_bytes.saturating_sub(self.bytes) as usize);
        self.probe()?;
        let row = self.tx.query_opt(
            "SELECT CASE WHEN octet_length(row_to_json(c)::text)<=$5 THEN row_to_json(c)::text END AS row_json
               FROM cmd2_projection_navigation_cohort c
              WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4",
            &[&self.domain, &self.consumer, &path, &id, &(cap as i32)],
        )?;
        let Some(row) = row else {
            return Ok(None);
        };
        self.rows += 1;
        let raw: String = row
            .get::<_, Option<String>>(0)
            .ok_or(DurableError::Refused("cohort tree selected row bytes"))?;
        self.charge(raw.len())?;
        let value: Value = serde_json::from_str(&raw)
            .map_err(|_| DurableError::Corrupt("cohort tree selected row JSON"))?;
        let canonical = canonical_value(&value, cap)?;
        self.charge(canonical.len())?;
        Ok(Some(
            digest(
                self.domain,
                self.consumer,
                b"tos-navigation-cohort-value-v1",
                &canonical,
            )
            .to_hex(),
        ))
    }
}

pub(super) fn initialize<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
) -> DurableResult<()> {
    tx.execute("INSERT INTO cmd2_projection_navigation_cohort_tree(domain,consumer_id,root_json)
                VALUES($1,$2,'null') ON CONFLICT(domain,consumer_id) DO UPDATE SET root_json='null'",
        &[&domain, &consumer])?;
    Ok(())
}

/// Only the first event of a new, unbootstrapped consumer can initialize an
/// empty map. Existing v1/v2 state always requires its explicit source bootstrap.
pub(super) fn initialize_empty_if_absent<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
) -> DurableResult<()> {
    if tx.query_opt("SELECT 1 FROM cmd2_projection_navigation_cohort_tree WHERE domain=$1 AND consumer_id=$2", &[&domain, &consumer])?.is_none() {
        if tx.query_opt("SELECT 1 FROM cmd2_projection_navigation_cohort WHERE domain=$1 AND consumer_id=$2 LIMIT 1", &[&domain, &consumer])?.is_some() {
            return Err(DurableError::Conflict("existing cohort metadata requires explicit bootstrap"));
        }
        initialize(tx, domain, consumer)?;
    }
    Ok(())
}

fn commitment(domain: &str, consumer: &str, checkpoint: &Checkpoint) -> DurableResult<Digest256> {
    let raw = serde_json::to_vec(&checkpoint.root)
        .map_err(|_| DurableError::Corrupt("cohort tree root encoding"))?;
    Ok(digest(
        domain,
        consumer,
        b"tos-navigation-current-cohort-tree-v1",
        &raw,
    ))
}

pub(super) fn current_root<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Digest256> {
    let checkpoint = Tree::new(tx, domain, consumer, limits).read_root()?;
    commitment(domain, consumer, &checkpoint)
}

pub(super) fn verify_owners<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
    owners: &BTreeSet<(String, String)>,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Checkpoint> {
    verify_owners_with_work(tx, domain, consumer, owners, limits, u64::MAX, u64::MAX)
}

pub(super) fn verify_owners_with_work<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
    owners: &BTreeSet<(String, String)>,
    limits: DurableNavigationEventLimits,
    max_probes: u64,
    max_rows: u64,
) -> DurableResult<Checkpoint> {
    if owners.len() > limits.max_current_cohorts {
        return Err(DurableError::Refused("cohort tree selected owner budget"));
    }
    let mut tree = Tree::new(tx, domain, consumer, limits);
    tree.max_probes = tree.max_probes.min(max_probes);
    tree.max_rows = tree.max_rows.min(max_rows);
    let mut checkpoint = tree.read_root()?;
    for (path, id) in owners {
        let key = owner_key(path, id)?;
        tree.charge(key.len())?;
        let expected = tree.lookup(&checkpoint.root, &key)?;
        if tree.row_digest(path, id)? != expected {
            return Err(DurableError::Corrupt(
                "cohort tree selected metadata differs",
            ));
        }
    }
    checkpoint.work = (tree.probes, tree.rows, tree.bytes);
    Ok(checkpoint)
}

pub(super) fn update_owners<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
    owners: &BTreeSet<(String, String)>,
    previous: &Checkpoint,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Digest256> {
    if owners.len() > limits.max_current_cohorts {
        return Err(DurableError::Refused("cohort tree changed owner budget"));
    }
    let mut tree = Tree::new(tx, domain, consumer, limits);
    let mut checkpoint = tree.read_root()?;
    if checkpoint.root != previous.root {
        return Err(DurableError::Conflict("cohort tree predecessor changed"));
    }
    for (path, id) in owners {
        let value = tree.row_digest(path, id)?;
        checkpoint.root = tree.change(checkpoint.root, &owner_key(path, id)?, value, 0)?;
    }
    let old = serde_json::to_string(&previous.root)
        .map_err(|_| DurableError::Corrupt("cohort tree root encoding"))?;
    let new = serde_json::to_string(&checkpoint.root)
        .map_err(|_| DurableError::Corrupt("cohort tree root encoding"))?;
    if tree.tx.execute(
        "UPDATE cmd2_projection_navigation_cohort_tree SET root_json=$3
                        WHERE domain=$1 AND consumer_id=$2 AND root_json=$4",
        &[&domain, &consumer, &new, &old],
    )? != 1
    {
        return Err(DurableError::Conflict("cohort tree root compare and set"));
    }
    commitment(domain, consumer, &checkpoint)
}

#[cfg(all(test, feature = "postgres-lab"))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires explicit isolated TOS_CMD_POSTGRES_URL"]
    fn navigation_cohort_tree_updates_selected_paths_and_proves_exact_metadata() {
        let url = std::env::var("TOS_CMD_POSTGRES_URL").expect("explicit private PG lab URL");
        let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
        let mut tx = client.transaction().unwrap();
        tx.batch_execute(
            "CREATE TEMP TABLE cmd2_projection_navigation_cohort (
               domain text,consumer_id text,source_path text,record_id text,value text,
               PRIMARY KEY(domain,consumer_id,source_path,record_id));
             CREATE TEMP TABLE cmd2_projection_navigation_cohort_tree (
               domain text,consumer_id text,root_json text,PRIMARY KEY(domain,consumer_id));
             CREATE TEMP TABLE cmd2_projection_navigation_cohort_tree_node (
               domain text,consumer_id text,node_sha256 text,node_json text,
               PRIMARY KEY(domain,consumer_id,node_sha256));",
        )
        .unwrap();
        let limits = DurableNavigationEventLimits {
            statement_timeout: Duration::from_secs(120),
            ..Default::default()
        };
        initialize(&mut tx, "lab", "nav").unwrap();
        let owners = (0..2048)
            .map(|n| (format!("ToS/{n}.json"), format!("record:{n}")))
            .collect::<BTreeSet<_>>();
        let empty = verify_owners(&mut tx, "lab", "nav", &owners, limits).unwrap();
        tx.batch_execute("INSERT INTO cmd2_projection_navigation_cohort
            SELECT 'lab','nav','ToS/'||n||'.json','record:'||n,'original' FROM generate_series(0,2047) n").unwrap();
        let original_root = update_owners(&mut tx, "lab", "nav", &owners, &empty, limits).unwrap();
        let all = verify_owners(&mut tx, "lab", "nav", &owners, limits).unwrap();
        assert_eq!(all.count(), 2048);
        let selected = BTreeSet::from([("ToS/99.json".into(), "record:99".into())]);
        let seq_reads = |tx: &mut postgres::Transaction<'_>| -> i64 {
            tx.query_one(
                "SELECT coalesce(sum(seq_tup_read),0)::bigint FROM pg_stat_xact_user_tables
                          WHERE schemaname LIKE 'pg_temp_%'",
                &[],
            )
            .unwrap()
            .get(0)
        };
        tx.batch_execute("ANALYZE cmd2_projection_navigation_cohort; ANALYZE cmd2_projection_navigation_cohort_tree_node").unwrap();
        let scans_before = seq_reads(&mut tx);
        let previous = verify_owners(&mut tx, "lab", "nav", &selected, limits).unwrap();
        let work = previous.work();
        assert!(
            work.0 < 24 && work.1 < 24 && work.2 < 65536,
            "selected proof must stay local: {work:?}"
        );
        tx.execute("UPDATE cmd2_projection_navigation_cohort SET value='changed'
                    WHERE domain='lab' AND consumer_id='nav' AND source_path='ToS/99.json' AND record_id='record:99'", &[]).unwrap();
        assert!(verify_owners(&mut tx, "lab", "nav", &selected, limits).is_err());
        let changed_root =
            update_owners(&mut tx, "lab", "nav", &selected, &previous, limits).unwrap();
        assert_ne!(original_root, changed_root);
        let changed = verify_owners(&mut tx, "lab", "nav", &selected, limits).unwrap();
        assert_eq!(changed.count(), 2048);
        assert!(
            seq_reads(&mut tx) - scans_before < 100,
            "one owner must not scan all current metadata or immutable nodes"
        );
        assert!(matches!(
            verify_owners_with_work(&mut tx, "lab", "nav", &selected, limits, 1, 1),
            Err(DurableError::Refused(_))
        ));
        tx.execute("UPDATE cmd2_projection_navigation_cohort SET value='original'
                    WHERE domain='lab' AND consumer_id='nav' AND source_path='ToS/99.json' AND record_id='record:99'", &[]).unwrap();
        assert_eq!(
            update_owners(&mut tx, "lab", "nav", &selected, &changed, limits).unwrap(),
            original_root,
            "commitment depends on exact current map, not event history"
        );
        let previous = verify_owners(&mut tx, "lab", "nav", &selected, limits).unwrap();
        tx.execute("DELETE FROM cmd2_projection_navigation_cohort
                    WHERE domain='lab' AND consumer_id='nav' AND source_path='ToS/99.json' AND record_id='record:99'", &[]).unwrap();
        assert!(verify_owners(&mut tx, "lab", "nav", &selected, limits).is_err());
        update_owners(&mut tx, "lab", "nav", &selected, &previous, limits).unwrap();
        let absent = verify_owners(&mut tx, "lab", "nav", &selected, limits).unwrap();
        assert_eq!(absent.count(), 2047);
        let surviving = BTreeSet::from([("ToS/100.json".into(), "record:100".into())]);
        verify_owners(&mut tx, "lab", "nav", &surviving, limits).unwrap();
        let root = Tree::new(&mut tx, "lab", "nav", limits)
            .read_root()
            .unwrap()
            .root
            .unwrap();
        tx.execute(
            "UPDATE cmd2_projection_navigation_cohort_tree_node SET node_json=node_json||' '
                    WHERE domain='lab' AND consumer_id='nav' AND node_sha256=$1",
            &[&root.digest],
        )
        .unwrap();
        assert!(
            current_root(&mut tx, "lab", "nav", limits).is_err(),
            "stored proof bytes must authenticate"
        );
        eprintln!(
            "cohort_tree_selected_work probes={} rows={} bytes={} current_cohorts=2048",
            work.0, work.1, work.2
        );
        tx.rollback().unwrap();
    }
}
