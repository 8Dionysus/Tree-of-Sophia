//! Optional PostgreSQL evidence for one selected record revision.
//!
//! The v1 table and receipt format originated with Work and remain compatible
//! with those rows. Authored revision history remains in ToS source members;
//! this table is only the PG adapter's per-record commit/replay proof, never
//! projected as authored source or treated as semantic admission.

use crate::durable_adapter::{DurableError, DurableResult};
use crate::source_cohort::{OWNER as SOURCE_OWNER, SourceRevisionWrite};
use crate::source_command::{
    self as cmd, CommandContext, PreparedCommand, SourceCommandError, SourceCommandResult,
};
use crate::source_revisions::{
    self as revision, RetainedRevisionTransaction, RevisionFamily, RevisionTransactionStatus,
};
use postgres::Transaction;
use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};

pub(crate) const PREDICATE_SCOPE: &str = "pg-work-revision-v1";
const TABLE: &str = "cmd2_work_revision_history_v1";
pub(crate) const MAX_TRANSACTIONS: usize = 128;
const MAX_AUTHORIZATION_BYTES: usize = 65_536;
const MAX_RETAINED_BYTES: usize = 1_048_576;
const MAX_PREPARE_ID_BYTES: usize = u16::MAX as usize;
const REF_MAGIC: &[u8; 8] = b"TOSWREF1";

#[derive(Clone, Debug)]
pub(crate) struct MemberVersionRef {
    pub(crate) path: String,
    pub(crate) revision: Option<u64>,
    pub(crate) digest: Option<Digest256>,
}

#[derive(Clone, Debug)]
pub(crate) struct SelectedTransactionRef {
    pub(crate) transaction_id: String,
    pub(crate) command_id: String,
    pub(crate) authorization_raw: Vec<u8>,
    pub(crate) before: Vec<MemberVersionRef>,
    pub(crate) after: Vec<MemberVersionRef>,
}

#[derive(Clone, Debug)]
pub(crate) struct Observation {
    pub(crate) domain: String,
    pub(crate) store_id: [u8; 16],
    pub(crate) source_path: String,
    pub(crate) record_id: String,
    pub(crate) generation: u64,
    /// Digest of the current record-scoped extension head, if any.
    pub(crate) predecessor_receipt_digest: Option<Digest256>,
    /// Only the transaction named by this invocation is selected for replay.
    pub(crate) selected_refs: Option<SelectedTransactionRef>,
    pub(crate) selected: Option<RetainedRevisionTransaction>,
    pub(crate) observation_digest: Digest256,
    pub(crate) retained_bytes: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct CommitEvidence {
    pub(crate) observation: Observation,
    pub(crate) source_path: String,
    pub(crate) record_id: String,
    pub(crate) transaction_id: String,
    pub(crate) command_id: String,
    pub(crate) owner_configuration_raw_digest: Digest256,
    pub(crate) owner_configuration_canonical_digest: Digest256,
    pub(crate) request_raw_digest: Digest256,
    pub(crate) request_canonical_digest: Digest256,
    pub(crate) expected_publication: Option<String>,
    pub(crate) publication_observation_digest: Digest256,
    pub(crate) compound_members_digest: Digest256,
    pub(crate) authorization_raw: Vec<u8>,
    pub(crate) before: Vec<MemberVersionRef>,
    pub(crate) after: Vec<MemberVersionRef>,
}

pub(crate) fn publication_digest(publication: &revision::RevisionPublication) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-pg-work-publication-observation-v1");
    if let Some(token) = &publication.token {
        part(&mut hasher, b"token-present");
        part(&mut hasher, token.as_bytes());
    } else {
        part(&mut hasher, b"token-absent");
    }
    for transaction in &publication.transactions {
        part(&mut hasher, transaction.transaction_id.as_bytes());
        part(
            &mut hasher,
            match transaction.status {
                RevisionTransactionStatus::Pending => b"pending",
                RevisionTransactionStatus::Committed => b"committed",
                RevisionTransactionStatus::RolledBack => b"rolled-back",
            },
        );
        part(
            &mut hasher,
            Digest256::of_bytes(&transaction.authorization_raw).as_bytes(),
        );
        for (tag, files) in [
            (b"before".as_slice(), &transaction.before),
            (b"after", &transaction.after),
        ] {
            part(&mut hasher, tag);
            let mut files = files.iter().collect::<Vec<_>>();
            files.sort_by(|left, right| left.path.cmp(&right.path));
            for file in files {
                part(&mut hasher, file.path.as_str().as_bytes());
                part(&mut hasher, Digest256::of_bytes(&file.raw).as_bytes());
                part(&mut hasher, &(file.raw.len() as u64).to_be_bytes());
            }
        }
    }
    hasher.finalize()
}

pub(crate) fn predicate_token(store_id: &[u8; 16], source_path: &str, record_id: &str) -> String {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-pg-work-revision-predicate-v1");
    part(&mut hasher, store_id);
    part(&mut hasher, source_path.as_bytes());
    part(&mut hasher, record_id.as_bytes());
    hasher.finalize().to_hex()
}

pub(crate) fn require_schema_v1(
    tx: &mut Transaction<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    schema_budget(tx, deadline, cancelled)?;
    let relation = tx
        .query_opt(
            "SELECT c.oid::bigint FROM pg_catalog.pg_class c
         JOIN pg_catalog.pg_class d ON d.relname='cmd2_domain'
           AND d.relnamespace=c.relnamespace AND d.relkind='r'
         WHERE c.relname=$1 AND c.relkind='r' AND c.relpersistence='p'
           AND c.relnamespace=current_schema()::regnamespace",
            &[&TABLE],
        )?
        .ok_or(DurableError::Refused(
            "PG Work revision extension absent or unknown",
        ))?;
    let oid: i64 = relation.try_get(0)?;
    schema_budget(tx, deadline, cancelled)?;
    let columns = tx.query(
        "SELECT a.attname::text,pg_catalog.format_type(a.atttypid,a.atttypmod),a.attnotnull
         FROM pg_catalog.pg_attribute a
         WHERE a.attrelid=$1::bigint::oid AND a.attnum>0 AND NOT a.attisdropped
         ORDER BY a.attnum",
        &[&oid],
    )?;
    let expected = [
        ("domain", "text", true),
        ("store_id", "bytea", true),
        ("source_path", "text", true),
        ("record_id", "text", true),
        ("generation", "bigint", true),
        ("previous_receipt_digest", "character(64)", false),
        ("transaction_id", "text", true),
        ("command_id", "text", true),
        ("owner_configuration_raw_digest", "character(64)", true),
        (
            "owner_configuration_canonical_digest",
            "character(64)",
            true,
        ),
        ("request_raw_digest", "character(64)", true),
        ("request_canonical_digest", "character(64)", true),
        ("expected_publication", "text", false),
        ("publication_observation_digest", "character(64)", true),
        ("compound_members_digest", "character(64)", true),
        ("authorization_raw", "bytea", true),
        ("before_refs", "bytea", true),
        ("after_refs", "bytea", true),
        ("receipt_digest", "character(64)", true),
        ("prepare_id", "bytea", true),
        ("commit_seq", "bigint", true),
    ];
    if columns.len() != expected.len() {
        return Err(DurableError::Refused(
            "unknown PG Work revision extension columns",
        ));
    }
    for (row, (name, ty, not_null)) in columns.iter().zip(expected) {
        if row.try_get::<_, String>(0)? != name
            || row.try_get::<_, String>(1)? != ty
            || row.try_get::<_, bool>(2)? != not_null
        {
            return Err(DurableError::Refused(
                "unknown PG Work revision extension columns",
            ));
        }
    }
    schema_budget(tx, deadline, cancelled)?;
    let constraints = tx.query(
        "SELECT c.contype::text,c.conkey,c.confrelid::bigint,c.confkey,c.convalidated,
                c.condeferrable,c.condeferred,pg_catalog.pg_get_expr(c.conbin,c.conrelid)::text,
                c.confupdtype::text,c.confdeltype::text,c.confmatchtype::text
         FROM pg_catalog.pg_constraint c WHERE c.conrelid=$1::bigint::oid",
        &[&oid],
    )?;
    if constraints.len() != 10 {
        return Err(DurableError::Refused(
            "unknown PG Work revision extension constraints",
        ));
    }
    let domain_oid: i64 = tx
        .query_one("SELECT 'cmd2_domain'::regclass::oid::bigint", &[])?
        .try_get(0)?;
    let domain_attnum: i16 = tx
        .query_one(
            "SELECT attnum FROM pg_catalog.pg_attribute
         WHERE attrelid='cmd2_domain'::regclass AND attname='domain' AND NOT attisdropped",
            &[],
        )?
        .try_get(0)?;
    let mut seen = BTreeSet::new();
    let mut key_count = 0usize;
    for constraint in constraints {
        let validated: bool = constraint.try_get(4)?;
        let deferrable: bool = constraint.try_get(5)?;
        let deferred: bool = constraint.try_get(6)?;
        if !validated || deferrable || deferred {
            return Err(DurableError::Refused(
                "deferred PG Work revision constraint",
            ));
        }
        let kind: String = constraint.try_get(0)?;
        let keys: Vec<i16> = constraint.try_get(1)?;
        if matches!(kind.as_str(), "p" | "u") {
            key_count += 1;
        }
        let valid = match kind.as_str() {
            "p" => keys == [1, 3, 4, 5],
            "u" => keys == [1, 3, 4, 7] || keys == [1, 20] || keys == [1, 3, 4, 8],
            "f" => {
                keys == [1]
                    && constraint.try_get::<_, i64>(2)? == domain_oid
                    && constraint.try_get::<_, Vec<i16>>(3)? == [domain_attnum]
                    && constraint.try_get::<_, String>(8)? == "a"
                    && constraint.try_get::<_, String>(9)? == "a"
                    && constraint.try_get::<_, String>(10)? == "s"
            }
            "c" => {
                let expression: Option<String> = constraint.try_get(7)?;
                match (keys.as_slice(), expression.as_deref()) {
                    ([5], Some("(generation > 0)")) => true,
                    ([2], Some("(octet_length(store_id) = 16)")) => true,
                    ([16], Some("(octet_length(authorization_raw) <= 65536)")) => true,
                    ([17], Some("(octet_length(before_refs) <= 65536)")) => true,
                    ([18], Some("(octet_length(after_refs) <= 65536)")) => true,
                    _ => false,
                }
            }
            _ => false,
        };
        if !valid || !seen.insert((kind, keys)) {
            return Err(DurableError::Refused(
                "unknown PG Work revision extension constraint",
            ));
        }
    }
    if key_count != 4 {
        return Err(DurableError::Refused("PG Work revision key set differs"));
    }
    schema_budget(tx, deadline, cancelled)?;
    let extras: i64 = tx
        .query_one(
            "SELECT (SELECT count(*) FROM pg_catalog.pg_attrdef WHERE adrelid=$1::bigint::oid)
              +(SELECT count(*) FROM pg_catalog.pg_rewrite WHERE ev_class=$1::bigint::oid)",
            &[&oid],
        )?
        .try_get(0)?;
    if extras != 0 {
        return Err(DurableError::Refused(
            "unknown PG Work revision extension default/rule",
        ));
    }
    let triggers = tx.query(
        "SELECT t.tgname::text,t.tgenabled::text,t.tgtype::integer,p.proname::text
         FROM pg_catalog.pg_trigger t JOIN pg_catalog.pg_proc p ON p.oid=t.tgfoid
         WHERE t.tgrelid=$1::bigint::oid AND NOT t.tgisinternal ORDER BY t.tgname",
        &[&oid],
    )?;
    let expected_triggers = [
        (
            "cmd2_audit_cmd2_work_revision_history_v1",
            "O",
            29,
            "cmd2_bump_audit_fence",
        ),
        (
            "cmd2_refuse_cmd2_work_revision_history_v1_truncate",
            "O",
            34,
            "cmd2_refuse_audited_truncate",
        ),
    ];
    if triggers.len() != expected_triggers.len() {
        return Err(DurableError::Refused(
            "unknown PG Work revision extension triggers",
        ));
    }
    for (row, (name, enabled, kind, function)) in triggers.iter().zip(expected_triggers) {
        if row.try_get::<_, String>(0)? != name
            || row.try_get::<_, String>(1)? != enabled
            || row.try_get::<_, i32>(2)? != kind
            || row.try_get::<_, String>(3)? != function
        {
            return Err(DurableError::Refused(
                "unknown PG Work revision extension triggers",
            ));
        }
    }
    let index_count: i64 = tx
        .query_one(
            "SELECT count(*) FROM pg_catalog.pg_index i WHERE i.indrelid=$1::bigint::oid",
            &[&oid],
        )?
        .try_get(0)?;
    if index_count != 4 {
        return Err(DurableError::Refused(
            "unknown PG Work revision extension indexes",
        ));
    }
    Ok(())
}

pub(crate) fn schema_budget(
    tx: &mut Transaction<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(DurableError::Refused(
            "PG Work revision schema check deadline",
        ));
    }
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_millis();
    let statement = format!("{}ms", remaining.min(15_000));
    let lock = format!("{}ms", remaining.min(5_000));
    tx.query_one(
        "SELECT set_config('statement_timeout',$1,true),set_config('lock_timeout',$2,true)",
        &[&statement, &lock],
    )?;
    Ok(())
}

pub(crate) fn observe(
    tx: &mut Transaction<'_>,
    domain: &str,
    store_id: [u8; 16],
    source_path: &str,
    record_id: &str,
    predicate_generation: u64,
    requested_command_id: &str,
) -> DurableResult<Observation> {
    if requested_command_id.len() > 256 {
        return Err(DurableError::Invalid("PG Work revision command identity"));
    }
    let domain = domain.to_owned();
    let chain = verify_receipt_chain(tx, &domain, &store_id, source_path, record_id)?;
    let generation = chain.generation;
    if generation != predicate_generation {
        return Err(DurableError::Conflict(
            "PG Work revision history census differs",
        ));
    }
    let head_row = tx.query_opt(
        "SELECT store_id,source_path,record_id,generation,previous_receipt_digest,transaction_id,
                command_id,owner_configuration_raw_digest,owner_configuration_canonical_digest,
                request_raw_digest,request_canonical_digest,expected_publication,
                publication_observation_digest,compound_members_digest,authorization_raw,
                before_refs,after_refs,receipt_digest,prepare_id,commit_seq
         FROM cmd2_work_revision_history_v1
         WHERE domain=$1 AND store_id=$2 AND source_path=$3 AND record_id=$4
         ORDER BY generation DESC LIMIT 1",
        &[&domain, &&store_id[..], &source_path, &record_id],
    )?;
    let selected_row = if requested_command_id.is_empty() {
        None
    } else {
        tx.query_opt(
        "SELECT store_id,source_path,record_id,generation,previous_receipt_digest,transaction_id,
                command_id,owner_configuration_raw_digest,owner_configuration_canonical_digest,
                request_raw_digest,request_canonical_digest,expected_publication,
                publication_observation_digest,compound_members_digest,authorization_raw,
                before_refs,after_refs,receipt_digest,prepare_id,commit_seq
         FROM cmd2_work_revision_history_v1
         WHERE domain=$1 AND store_id=$2 AND source_path=$3 AND record_id=$4 AND command_id=$5",
        &[&domain, &&store_id[..], &source_path, &record_id, &requested_command_id],
    )?
    };
    let head = head_row
        .as_ref()
        .map(|row| decode_receipt_row(row, &domain, &store_id, source_path, record_id))
        .transpose()?;
    let selected = selected_row
        .as_ref()
        .map(|row| decode_receipt_row(row, &domain, &store_id, source_path, record_id))
        .transpose()?;
    if head
        .as_ref()
        .is_some_and(|row| row.generation != generation)
        || selected
            .as_ref()
            .is_some_and(|row| row.command_id != requested_command_id)
    {
        return Err(DurableError::Conflict(
            "PG Work revision head or replay row changed",
        ));
    }
    let head_receipt_digest = head.as_ref().map(|row| row.receipt_digest);
    if head_receipt_digest != chain.head_receipt_digest {
        return Err(DurableError::Conflict("PG Work revision head changed"));
    }
    let selected_refs = selected.map(|row| row.into_selected());
    let observation_digest = observation_digest(
        &domain,
        &store_id,
        source_path,
        record_id,
        generation,
        head_receipt_digest,
    );
    let retained_bytes = chain
        .retained_bytes
        .checked_add(std::mem::size_of::<Observation>())
        .and_then(|bytes| {
            bytes.checked_add(selected_refs.as_ref().map_or(0, selected_ref_state_bytes))
        })
        .ok_or(DurableError::Refused(
            "PG Work revision retained state overflow",
        ))?;
    if retained_bytes > MAX_RETAINED_BYTES {
        return Err(DurableError::Refused(
            "PG Work revision retained state budget",
        ));
    }
    Ok(Observation {
        domain,
        store_id,
        source_path: source_path.to_owned(),
        record_id: record_id.to_owned(),
        generation,
        predecessor_receipt_digest: head_receipt_digest,
        selected_refs,
        selected: None,
        observation_digest,
        retained_bytes,
    })
}

struct StoredReceipt {
    generation: u64,
    previous_receipt_digest: Option<Digest256>,
    transaction_id: String,
    command_id: String,
    authorization_raw: Vec<u8>,
    before: Vec<MemberVersionRef>,
    after: Vec<MemberVersionRef>,
    receipt_digest: Digest256,
    prepare_id: Vec<u8>,
    commit_seq: u64,
}

impl StoredReceipt {
    fn into_selected(self) -> SelectedTransactionRef {
        SelectedTransactionRef {
            transaction_id: self.transaction_id,
            command_id: self.command_id,
            authorization_raw: self.authorization_raw,
            before: self.before,
            after: self.after,
        }
    }
}

fn decode_receipt_row(
    row: &postgres::Row,
    domain: &str,
    store_id: &[u8; 16],
    source_path: &str,
    record_id: &str,
) -> DurableResult<StoredReceipt> {
    if row.get::<_, Vec<u8>>(0) != store_id
        || row.get::<_, String>(1) != source_path
        || row.get::<_, String>(2) != record_id
    {
        return Err(DurableError::Corrupt("PG Work revision row identity"));
    }
    let generation = as_u64(row.get(3))?;
    let previous_receipt_digest = row
        .get::<_, Option<String>>(4)
        .map(parse_digest)
        .transpose()?;
    let transaction_id: String = row.get(5);
    let command_id: String = row.get(6);
    let owner_raw = parse_digest(row.get::<_, String>(7))?;
    let owner_canonical = parse_digest(row.get::<_, String>(8))?;
    let request_raw = parse_digest(row.get::<_, String>(9))?;
    let request_canonical = parse_digest(row.get::<_, String>(10))?;
    let expected_publication: Option<String> = row.get(11);
    let publication_observation = parse_digest(row.get::<_, String>(12))?;
    let compound_members = parse_digest(row.get::<_, String>(13))?;
    let authorization_raw: Vec<u8> = row.get(14);
    let before_raw: Vec<u8> = row.get(15);
    let after_raw: Vec<u8> = row.get(16);
    let before = decode_refs(&before_raw, source_path)?;
    let after = decode_refs(&after_raw, source_path)?;
    let stored_receipt_digest = parse_digest(row.get::<_, String>(17))?;
    let prepare_id: Vec<u8> = row.get(18);
    let commit_seq = as_u64(row.get(19))?;
    if authorization_raw.is_empty()
        || authorization_raw.len() > MAX_AUTHORIZATION_BYTES
        || prepare_id.is_empty()
        || prepare_id.len() > MAX_PREPARE_ID_BYTES
        || commit_seq == 0
        || generation == 0
    {
        return Err(DurableError::Corrupt("PG Work revision receipt fields"));
    }
    validate_selected_record_authorization_binding(
        &authorization_raw,
        source_path,
        record_id,
        &transaction_id,
        &command_id,
        owner_canonical,
        request_canonical,
        expected_publication.as_deref(),
    )?;
    let computed = receipt_digest(
        domain,
        store_id,
        source_path,
        record_id,
        generation,
        previous_receipt_digest,
        &transaction_id,
        &command_id,
        owner_raw,
        owner_canonical,
        request_raw,
        request_canonical,
        expected_publication.as_deref(),
        publication_observation,
        compound_members,
        &authorization_raw,
        &before_raw,
        &after_raw,
        &prepare_id,
        commit_seq,
    );
    if computed != stored_receipt_digest {
        return Err(DurableError::Corrupt("PG Work revision receipt digest"));
    }
    Ok(StoredReceipt {
        generation,
        previous_receipt_digest,
        transaction_id,
        command_id,
        authorization_raw,
        before,
        after,
        receipt_digest: stored_receipt_digest,
        prepare_id,
        commit_seq,
    })
}

fn validate_selected_record_authorization_binding(
    authorization_raw: &[u8],
    source_path: &str,
    record_id: &str,
    transaction_id: &str,
    command_id: &str,
    owner_canonical: Digest256,
    request_canonical: Digest256,
    expected_publication: Option<&str>,
) -> DurableResult<()> {
    let authorization = cmd::parse(authorization_raw).map_err(DurableError::Source)?;
    cmd::exact_keys(
        &authorization,
        &[
            "schema_version",
            "principal_id",
            "authority_ref",
            "source_path",
            "record_id",
            "record_type",
            "request",
        ],
    )
    .map_err(DurableError::Source)?;
    let request = cmd::field(&authorization, "request").map_err(DurableError::Source)?;
    if cmd::text(&authorization, "schema_version").map_err(DurableError::Source)?
        != "tos_selected_metadata_revision_authorization_v1"
        || cmd::text(&authorization, "source_path").map_err(DurableError::Source)? != source_path
        || cmd::text(&authorization, "record_id").map_err(DurableError::Source)? != record_id
        || cmd::text(&authorization, "record_type")
            .map_err(DurableError::Source)?
            .is_empty()
        || cmd::text(request, "operation").map_err(DurableError::Source)? != "record.revise"
        || cmd::text(request, "command_id").map_err(DurableError::Source)? != command_id
        || revision::transaction_id(request).map_err(DurableError::Source)? != transaction_id
        || cmd::record_digest(request).map_err(DurableError::Source)? != request_canonical
        || cmd::text(request, "expected_configuration").map_err(DurableError::Source)?
            != owner_canonical.to_prefixed()
        || selected_record_publication(
            cmd::text(&authorization, "record_type").map_err(DurableError::Source)?,
            request,
        )?
        .as_deref()
            != expected_publication
    {
        return Err(DurableError::Corrupt(
            "PG selected record authorization binding",
        ));
    }
    Ok(())
}

/// Work keeps its independent null predecessor. Other selected records can
/// bind an existing publication token; retain it verbatim in their PG receipt.
fn selected_record_publication(
    record_type: &str,
    request: &tos_foundation::JsonValue,
) -> DurableResult<Option<String>> {
    let value = cmd::field(request, "expected_publication").map_err(DurableError::Source)?;
    if value == &tos_foundation::JsonValue::Null {
        return Ok(None);
    }
    if record_type == "work" {
        return Err(DurableError::Conflict(
            "PG Work publication predecessor must be null",
        ));
    }
    let token = cmd::text(request, "expected_publication").map_err(DurableError::Source)?;
    Digest256::from_prefixed(token)
        .map_err(|_| DurableError::Invalid("PG selected publication predecessor digest"))?;
    Ok(Some(token.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_record_v1_authorization_binding_accepts_work_and_agent_profiles() {
        for (source_path, record_id, record_type, command_id) in [
            (
                "ToS/source-witnesses/works/zarathustra/work.json",
                "tos.work.zarathustra",
                "work",
                "test:work-revision",
            ),
            (
                "ToS/source-witnesses/agents/friedrich-nietzsche/agent.json",
                "tos.agent.friedrich-nietzsche",
                "agent",
                "test:agent-revision",
            ),
        ] {
            let token = Digest256::of_bytes(b"existing-publication").to_prefixed();
            for expected_publication in [None, Some(token.as_str())] {
                let owner_canonical = Digest256::of_bytes(record_type.as_bytes());
                let request = cmd::object(vec![
                    ("schema_version", cmd::string("tos_local_source_command_v1")),
                    ("operation", cmd::string("record.revise")),
                    ("command_id", cmd::string(command_id)),
                    (
                        "expected_configuration",
                        cmd::string(&owner_canonical.to_prefixed()),
                    ),
                    (
                        "expected_publication",
                        expected_publication
                            .map(cmd::string)
                            .unwrap_or(tos_foundation::JsonValue::Null),
                    ),
                    ("fields", cmd::object(Vec::new())),
                ]);
                let authorization = cmd::object(vec![
                    (
                        "schema_version",
                        cmd::string("tos_selected_metadata_revision_authorization_v1"),
                    ),
                    ("principal_id", cmd::string("test-principal")),
                    ("authority_ref", cmd::string("test-authority")),
                    ("source_path", cmd::string(source_path)),
                    ("record_id", cmd::string(record_id)),
                    ("record_type", cmd::string(record_type)),
                    ("request", request.clone()),
                ]);
                let authorization_raw = cmd::published(&authorization).unwrap();
                let request_canonical = cmd::record_digest(&request).unwrap();
                let transaction_id = revision::transaction_id(&request).unwrap();

                assert_eq!(
                    validate_selected_record_authorization_binding(
                        &authorization_raw,
                        source_path,
                        record_id,
                        &transaction_id,
                        command_id,
                        owner_canonical,
                        request_canonical,
                        expected_publication,
                    )
                    .is_ok(),
                    record_type != "work" || expected_publication.is_none()
                );
                assert!(
                    validate_selected_record_authorization_binding(
                        &authorization_raw,
                        "ToS/source-witnesses/other.json",
                        record_id,
                        &transaction_id,
                        command_id,
                        owner_canonical,
                        request_canonical,
                        expected_publication,
                    )
                    .is_err()
                );
                let mismatched = if expected_publication.is_some() {
                    None
                } else {
                    Some(token.as_str())
                };
                assert!(
                    validate_selected_record_authorization_binding(
                        &authorization_raw,
                        source_path,
                        record_id,
                        &transaction_id,
                        command_id,
                        owner_canonical,
                        request_canonical,
                        mismatched,
                    )
                    .is_err()
                );
            }
        }
    }
}

struct ReceiptChainSummary {
    generation: u64,
    retained_bytes: usize,
    head_receipt_digest: Option<Digest256>,
}

/// Validate every compact receipt in this one-record chain. The SQL preflight
/// caps rows and encoded metadata before the detailed read; source bodies stay
/// lazy and only the exact requested replay transaction is hydrated from STO.
fn verify_receipt_chain(
    tx: &mut Transaction<'_>,
    domain: &str,
    store_id: &[u8; 16],
    source_path: &str,
    record_id: &str,
) -> DurableResult<ReceiptChainSummary> {
    let limit = as_i64((MAX_TRANSACTIONS + 1) as u64)?;
    let census = tx.query_one(
        "SELECT count(*)::bigint,coalesce(max(generation),0),
                coalesce(sum(
                    octet_length(domain)::bigint+octet_length(source_path)::bigint+
                    octet_length(record_id)::bigint+octet_length(transaction_id)::bigint+
                    octet_length(command_id)::bigint+coalesce(octet_length(expected_publication)::bigint,0)+
                    octet_length(authorization_raw)::bigint+octet_length(before_refs)::bigint+
                    octet_length(after_refs)::bigint+octet_length(prepare_id)::bigint
                ),0)::bigint,
                coalesce(bool_or(octet_length(expected_publication)>71
                    OR octet_length(source_path)>4096 OR octet_length(record_id)>1024
                    OR octet_length(transaction_id)>128 OR octet_length(command_id)>1024
                    OR octet_length(prepare_id)<1 OR octet_length(prepare_id)>65535),false)
         FROM (
             SELECT domain,source_path,record_id,generation,transaction_id,command_id,
                    expected_publication,authorization_raw,before_refs,after_refs,prepare_id
             FROM cmd2_work_revision_history_v1
             WHERE domain=$1 AND store_id=$2 AND source_path=$3 AND record_id=$4
             ORDER BY generation DESC LIMIT $5
         ) bounded",
        &[&domain, &&store_id[..], &source_path, &record_id, &limit],
    )?;
    let count = as_u64(census.get(0))?;
    let generation = as_u64(census.get(1))?;
    let retained_bytes = usize::try_from(as_u64(census.get(2))?)
        .map_err(|_| DurableError::Refused("PG Work revision retained evidence budget"))?;
    let invalid_shape: bool = census.get(3);
    if count > MAX_TRANSACTIONS as u64
        || generation != count
        || retained_bytes > MAX_RETAINED_BYTES
        || invalid_shape
    {
        return Err(DurableError::Conflict(
            "PG Work revision history census differs",
        ));
    }
    let rows = tx.query(
        "SELECT store_id,source_path,record_id,generation,previous_receipt_digest,transaction_id,
                command_id,owner_configuration_raw_digest,owner_configuration_canonical_digest,
                request_raw_digest,request_canonical_digest,expected_publication,
                publication_observation_digest,compound_members_digest,authorization_raw,
                before_refs,after_refs,receipt_digest,prepare_id,commit_seq
         FROM cmd2_work_revision_history_v1
         WHERE domain=$1 AND store_id=$2 AND source_path=$3 AND record_id=$4
         ORDER BY generation LIMIT $5",
        &[&domain, &&store_id[..], &source_path, &record_id, &limit],
    )?;
    if rows.len() as u64 != count {
        return Err(DurableError::Conflict(
            "PG Work revision chain row count changed",
        ));
    }
    let mut expected_generation = 1u64;
    let mut previous = None;
    for row in &rows {
        let receipt = decode_receipt_row(row, domain, store_id, source_path, record_id)?;
        if receipt.generation != expected_generation || receipt.previous_receipt_digest != previous
        {
            return Err(DurableError::Corrupt("PG Work revision receipt chain"));
        }
        previous = Some(receipt.receipt_digest);
        expected_generation = expected_generation
            .checked_add(1)
            .ok_or(DurableError::Corrupt(
                "PG Work revision generation overflow",
            ))?;
    }
    if expected_generation.saturating_sub(1) != generation {
        return Err(DurableError::Corrupt("PG Work revision chain generation"));
    }
    Ok(ReceiptChainSummary {
        generation,
        retained_bytes,
        head_receipt_digest: previous,
    })
}

fn selected_ref_state_bytes(selected: &SelectedTransactionRef) -> usize {
    std::mem::size_of::<SelectedTransactionRef>()
        .saturating_add(selected.transaction_id.capacity())
        .saturating_add(selected.command_id.capacity())
        .saturating_add(selected.authorization_raw.capacity())
        .saturating_add(
            selected
                .before
                .iter()
                .chain(&selected.after)
                .map(|reference| {
                    std::mem::size_of::<MemberVersionRef>() + reference.path.capacity()
                })
                .sum::<usize>(),
        )
}

pub(crate) fn make_commit_evidence(
    context: &CommandContext,
    proposal: &PreparedCommand,
    observation: &Observation,
    publication_observation_digest: Digest256,
    writes: &[SourceRevisionWrite],
) -> DurableResult<CommitEvidence> {
    let (configuration, family) = revision::configuration(context).map_err(DurableError::Source)?;
    let work_owner = family == RevisionFamily::CorpusSelectedV3
        && cmd::text(&configuration, "record_type").map_err(DurableError::Source)? == "work";
    if !family.selected()
        || cmd::text(&configuration, "source_path").map_err(DurableError::Source)?
            != observation.source_path
        || cmd::text(&configuration, "record_id").map_err(DurableError::Source)?
            != observation.record_id
    {
        return Err(DurableError::Refused(
            "PG revision evidence requires exact selected record owner",
        ));
    }
    if work_owner {
        revision::validate_pg_work_revision_archive_changes(context, proposal)
            .map_err(DurableError::Source)?;
    } else {
        revision::validate_pg_selected_revision_proposal(context, proposal)
            .map_err(DurableError::Source)?;
    }
    let member_writes = selected_member_writes(&observation.source_path, writes)?;
    let request = cmd::parse(&context.request_raw).map_err(DurableError::Source)?;
    let expected_publication = selected_record_publication(
        cmd::text(&configuration, "record_type").map_err(DurableError::Source)?,
        &request,
    )?;
    if observation.selected.is_some()
        || observation.selected_refs.is_some()
        || proposal.operation != "record.revise"
    {
        return Err(DurableError::Conflict(
            "PG Work revision publication observation differs",
        ));
    }
    let transaction_id = revision::transaction_id(&request).map_err(DurableError::Source)?;
    let command_id = cmd::text(&request, "command_id")
        .map_err(DurableError::Source)?
        .to_owned();
    let owner_raw = Digest256::of_bytes(&context.configuration_raw);
    let owner_canonical = cmd::record_digest(&configuration).map_err(DurableError::Source)?;
    if owner_raw != proposal.configuration_raw_sha256
        || owner_canonical != proposal.configuration_canonical_sha256
        || cmd::text(&request, "expected_configuration").map_err(DurableError::Source)?
            != owner_canonical.to_prefixed()
    {
        return Err(DurableError::Conflict(
            "PG Work revision owner configuration digest differs",
        ));
    }
    let source_path = observation.source_path.as_str();
    let (parent, _) = source_path
        .rsplit_once('/')
        .ok_or(DurableError::Invalid("PG Work source path parent"))?;
    let names = revision::names(source_path).map_err(DurableError::Source)?;
    let mut before = Vec::new();
    let mut after = Vec::new();
    for name in names {
        let path_text = format!("{parent}/{name}");
        let path = RelativePath::parse(&path_text)
            .map_err(|_| DurableError::Invalid("PG Work selected member path"))?;
        let change = proposal
            .changes
            .iter()
            .find(|change| change.path == path)
            .ok_or(DurableError::Conflict(
                "PG Work revision selected member change absent",
            ))?;
        let write = member_writes
            .iter()
            .copied()
            .find(|write| write.subject == path_text)
            .ok_or(DurableError::Conflict(
                "PG Work selected member write absent",
            ))?;
        let expected_before = write
            .expected_predecessor
            .map(|(revision, digest)| MemberVersionRef {
                path: path_text.clone(),
                revision: Some(revision),
                digest: Some(digest),
            })
            .unwrap_or_else(|| MemberVersionRef {
                path: path_text.clone(),
                revision: None,
                digest: None,
            });
        let raw = change.after.as_ref().ok_or(DurableError::Refused(
            "PG Work revision selected member deletion",
        ))?;
        if raw != &write.exact_bytes
            || change.before != write.expected_predecessor.map(|(_, digest)| digest)
        {
            return Err(DurableError::Conflict(
                "PG Work selected member write differs from proposal",
            ));
        }
        before.push(expected_before);
        after.push(MemberVersionRef {
            path: path_text,
            revision: Some(write.proposed_revision),
            digest: Some(Digest256::of_bytes(&write.exact_bytes)),
        });
    }
    let before_raw = encode_refs(&before, source_path)?;
    let after_raw = encode_refs(&after, source_path)?;
    let source_raw = after
        .iter()
        .find(|file| file.path == source_path)
        .and_then(|reference| writes.iter().find(|write| write.subject == reference.path))
        .map(|write| write.exact_bytes.as_slice())
        .ok_or(DurableError::Corrupt(
            "PG selected successor record missing",
        ))?;
    let source_record = cmd::parse(source_raw).map_err(DurableError::Source)?;
    let history_path = format!("{parent}/{}", revision::HISTORY);
    let history_raw = writes
        .iter()
        .find(|write| write.subject == history_path)
        .ok_or(DurableError::Corrupt("PG Work successor history missing"))?;
    let history = cmd::parse(&history_raw.exact_bytes).map_err(DurableError::Source)?;
    let receipts = cmd::array(&history, "receipts").map_err(DurableError::Source)?;
    let latest = receipts
        .last()
        .ok_or(DurableError::Corrupt("PG Work successor history empty"))?;
    if cmd::text(latest, "command_id").map_err(DurableError::Source)? != command_id
        || cmd::text(
            cmd::field(latest, "publication").map_err(DurableError::Source)?,
            "transaction_id",
        )
        .map_err(DurableError::Source)?
            != transaction_id
    {
        return Err(DurableError::Conflict(
            "PG Work successor authored receipt differs",
        ));
    }
    let authorization = crate::source_creation_store::revision_publication::authorization(
        context,
        &configuration,
        family,
        &source_record,
    )
    .map_err(DurableError::Source)?;
    let authorization_raw = cmd::published(&authorization).map_err(DurableError::Source)?;
    if authorization_raw.len() > MAX_AUTHORIZATION_BYTES {
        return Err(DurableError::Refused("PG Work authorization byte budget"));
    }
    let compound_members_digest = compound_members_digest(writes);
    Ok(CommitEvidence {
        observation: observation.clone(),
        source_path: source_path.to_owned(),
        record_id: observation.record_id.clone(),
        transaction_id,
        command_id,
        owner_configuration_raw_digest: owner_raw,
        owner_configuration_canonical_digest: owner_canonical,
        request_raw_digest: Digest256::of_bytes(&context.request_raw),
        request_canonical_digest: cmd::record_digest(&request).map_err(DurableError::Source)?,
        expected_publication,
        publication_observation_digest,
        compound_members_digest,
        authorization_raw,
        before,
        after,
    })
}

pub(crate) fn compound_members_digest(writes: &[SourceRevisionWrite]) -> Digest256 {
    let mut writes = writes.iter().collect::<Vec<_>>();
    writes.sort_by(|left, right| left.subject.cmp(&right.subject));
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-pg-work-revision-compound-members-v1");
    for write in writes {
        part(&mut hasher, write.subject.as_bytes());
        if let Some((revision, digest)) = write.expected_predecessor {
            part(&mut hasher, b"present");
            part(&mut hasher, &revision.to_be_bytes());
            part(&mut hasher, digest.as_bytes());
        } else {
            part(&mut hasher, b"absent");
        }
        part(&mut hasher, &write.proposed_revision.to_be_bytes());
        part(
            &mut hasher,
            Digest256::of_bytes(&write.exact_bytes).as_bytes(),
        );
        part(&mut hasher, &(write.exact_bytes.len() as u64).to_be_bytes());
    }
    hasher.finalize()
}

pub(crate) fn commit_delta_digest(evidence: &CommitEvidence) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-pg-work-revision-evidence-delta-v1");
    part(
        &mut hasher,
        evidence.observation.observation_digest.as_bytes(),
    );
    part(&mut hasher, evidence.source_path.as_bytes());
    part(&mut hasher, evidence.record_id.as_bytes());
    part(&mut hasher, evidence.transaction_id.as_bytes());
    part(&mut hasher, evidence.command_id.as_bytes());
    part(
        &mut hasher,
        evidence.owner_configuration_raw_digest.as_bytes(),
    );
    part(
        &mut hasher,
        evidence.owner_configuration_canonical_digest.as_bytes(),
    );
    part(&mut hasher, evidence.request_raw_digest.as_bytes());
    part(&mut hasher, evidence.request_canonical_digest.as_bytes());
    part(
        &mut hasher,
        evidence
            .expected_publication
            .as_deref()
            .unwrap_or("<null>")
            .as_bytes(),
    );
    part(
        &mut hasher,
        evidence.publication_observation_digest.as_bytes(),
    );
    part(&mut hasher, evidence.compound_members_digest.as_bytes());
    part(
        &mut hasher,
        Digest256::of_bytes(&evidence.authorization_raw).as_bytes(),
    );
    hasher.finalize()
}

/// Rebind recovered or live typed evidence to the exact durable source attempt
/// before the replay short-circuit can return a committed result.
pub(crate) fn validate_attempt_evidence(
    evidence: &CommitEvidence,
    domain: &str,
    store_id: [u8; 16],
    command_id: &str,
    request_raw_digest: Digest256,
    owner_configuration_raw_digest: Digest256,
    owner_configuration_canonical_digest: Digest256,
    observed_generation: u64,
    writes: &[SourceRevisionWrite],
) -> DurableResult<()> {
    validate_selected_record_authorization_binding(
        &evidence.authorization_raw,
        &evidence.source_path,
        &evidence.record_id,
        &evidence.transaction_id,
        &evidence.command_id,
        evidence.owner_configuration_canonical_digest,
        evidence.request_canonical_digest,
        evidence.expected_publication.as_deref(),
    )?;
    let observation = &evidence.observation;
    if observation.domain != domain
        || observation.store_id != store_id
        || observation.source_path != evidence.source_path
        || !evidence.source_path.starts_with("ToS/source-witnesses/")
        || !evidence.source_path.ends_with(".json")
        || observation.record_id != evidence.record_id
        || observation.generation != observed_generation
        || observed_generation >= MAX_TRANSACTIONS as u64
        || observation.observation_digest
            != observation_digest(
                domain,
                &store_id,
                &evidence.source_path,
                &evidence.record_id,
                observed_generation,
                observation.predecessor_receipt_digest,
            )
        || evidence.command_id != command_id
        || evidence.request_raw_digest != request_raw_digest
        || evidence.owner_configuration_raw_digest != owner_configuration_raw_digest
        || evidence.owner_configuration_canonical_digest != owner_configuration_canonical_digest
        || evidence.compound_members_digest != compound_members_digest(writes)
    {
        return Err(DurableError::Conflict(
            "PG Work revision attempt evidence binding differs",
        ));
    }
    let (parent, _) = evidence
        .source_path
        .rsplit_once('/')
        .ok_or(DurableError::Corrupt("PG Work source path parent"))?;
    let member_writes = selected_member_writes(&evidence.source_path, writes)?;
    let mut expected_before = Vec::new();
    let mut expected_after = Vec::new();
    for name in revision::names(&evidence.source_path).map_err(DurableError::Source)? {
        let path = format!("{parent}/{name}");
        let write = member_writes
            .iter()
            .copied()
            .find(|write| write.subject == path)
            .ok_or(DurableError::Conflict(
                "PG Work attempt member write absent",
            ))?;
        expected_before.push(match write.expected_predecessor {
            Some((revision, digest)) => MemberVersionRef {
                path: path.clone(),
                revision: Some(revision),
                digest: Some(digest),
            },
            None => MemberVersionRef {
                path: path.clone(),
                revision: None,
                digest: None,
            },
        });
        expected_after.push(MemberVersionRef {
            path,
            revision: Some(write.proposed_revision),
            digest: Some(Digest256::of_bytes(&write.exact_bytes)),
        });
    }
    if encode_refs(&evidence.before, &evidence.source_path)?
        != encode_refs(&expected_before, &evidence.source_path)?
        || encode_refs(&evidence.after, &evidence.source_path)?
            != encode_refs(&expected_after, &evidence.source_path)?
    {
        return Err(DurableError::Conflict(
            "PG Work revision member locators differ from attempt",
        ));
    }
    Ok(())
}

/// Select exactly the three current Work members whose before/after locators
/// the compact Work receipt records. The complete durable write set still
/// includes the predecessor archive: its exact authored bytes remain bound by
/// `compound_members_digest`, the typed `retain_archive` proposal, the source
/// readset and the source-revision path indexes. Ready recovery independently
/// reconstructs and validates that whole archive package.
fn selected_member_writes<'a>(
    source_path: &str,
    writes: &'a [SourceRevisionWrite],
) -> DurableResult<Vec<&'a SourceRevisionWrite>> {
    let (parent, _) = source_path
        .rsplit_once('/')
        .ok_or(DurableError::Corrupt("PG Work source path parent"))?;
    let names = revision::names(source_path).map_err(DurableError::Source)?;
    if names.iter().collect::<BTreeSet<_>>().len() != 3 {
        return Err(DurableError::Corrupt("PG Work selected member names"));
    }
    names
        .into_iter()
        .map(|name| {
            let path = format!("{parent}/{name}");
            let mut matches = writes.iter().filter(|write| write.subject == path);
            let write = matches.next().ok_or(DurableError::Conflict(
                "PG Work selected member write absent",
            ))?;
            if matches.next().is_some() {
                return Err(DurableError::Conflict(
                    "PG Work selected member write is duplicated",
                ));
            }
            Ok(write)
        })
        .collect()
}

pub(crate) fn apply_source_change(
    tx: &mut Transaction<'_>,
    domain: &str,
    store_id: [u8; 16],
    prepare_id: &[u8],
    commit_seq: u64,
    expected_predicate_generation: u64,
    definition: Digest256,
    evidence: &CommitEvidence,
    actual_compound_digest: Digest256,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    if prepare_id.is_empty() || prepare_id.len() > MAX_PREPARE_ID_BYTES {
        return Err(DurableError::Invalid("PG Work revision prepare ID length"));
    }
    require_schema_v1(tx, deadline, cancelled)?;
    if evidence.observation.domain != domain
        || evidence.observation.store_id != store_id
        || evidence.observation.source_path != evidence.source_path
        || evidence.observation.record_id != evidence.record_id
        || evidence.observation.generation != expected_predicate_generation
        || evidence.observation.selected_refs.is_some()
        || evidence.observation.selected.is_some()
        || actual_compound_digest != evidence.compound_members_digest
        || evidence.owner_configuration_raw_digest == Digest256::from_bytes([0; 32])
        || evidence.owner_configuration_canonical_digest == Digest256::from_bytes([0; 32])
        || evidence.request_raw_digest == Digest256::from_bytes([0; 32])
        || evidence.request_canonical_digest == Digest256::from_bytes([0; 32])
        || commit_seq == 0
    {
        return Err(DurableError::Conflict(
            "PG Work revision commit evidence differs",
        ));
    }
    let chain = verify_receipt_chain(
        tx,
        domain,
        &store_id,
        &evidence.source_path,
        &evidence.record_id,
    )?;
    if chain.generation != expected_predicate_generation
        || chain.head_receipt_digest != evidence.observation.predecessor_receipt_digest
    {
        return Err(DurableError::Conflict(
            "PG Work revision receipt head changed",
        ));
    }
    let token = predicate_token(&store_id, &evidence.source_path, &evidence.record_id);
    let generation_row = tx
        .query_opt(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind='range' AND owner=$2
           AND scope=$3 AND token=$4 FOR UPDATE",
            &[&domain, &SOURCE_OWNER, &PREDICATE_SCOPE, &token],
        )?
        .ok_or(DurableError::Refused("PG Work revision predicate absent"))?;
    if as_u64(generation_row.get(0))? != expected_predicate_generation
        || !generation_row.get::<_, bool>(1)
        || generation_row.get::<_, String>(2) != definition.to_hex()
    {
        return Err(DurableError::Conflict(
            "PG Work revision predicate generation changed",
        ));
    }
    let latest = tx.query_opt(
        "SELECT generation,receipt_digest FROM cmd2_work_revision_history_v1
         WHERE domain=$1 AND store_id=$2 AND source_path=$3 AND record_id=$4
         ORDER BY generation DESC LIMIT 1 FOR UPDATE",
        &[
            &domain,
            &&store_id[..],
            &evidence.source_path,
            &evidence.record_id,
        ],
    )?;
    let (generation, predecessor) = match latest {
        Some(row) => (
            as_u64(row.get(0))?,
            Some(parse_digest(row.get::<_, String>(1))?),
        ),
        None => (0, None),
    };
    if generation != expected_predicate_generation
        || predecessor != evidence.observation.predecessor_receipt_digest
    {
        return Err(DurableError::Conflict(
            "PG Work revision latest receipt changed",
        ));
    }
    let next_generation = generation
        .checked_add(1)
        .filter(|next| *next <= i64::MAX as u64)
        .ok_or(DurableError::Refused(
            "PG Work revision generation exhausted",
        ))?;
    if next_generation > MAX_TRANSACTIONS as u64 {
        return Err(DurableError::Refused(
            "PG Work revision history capacity reached",
        ));
    }
    let before_raw = encode_refs(&evidence.before, &evidence.source_path)?;
    let after_raw = encode_refs(&evidence.after, &evidence.source_path)?;
    // Match the SQL census exactly: all variable-width fields represented by
    // the receipt row count toward the same bounded retained-evidence budget.
    let new_retained_bytes = [
        domain.len(),
        evidence.source_path.len(),
        evidence.record_id.len(),
        evidence.transaction_id.len(),
        evidence.command_id.len(),
        evidence.expected_publication.as_deref().map_or(0, str::len),
        evidence.authorization_raw.len(),
        before_raw.len(),
        after_raw.len(),
        prepare_id.len(),
    ]
    .into_iter()
    .try_fold(chain.retained_bytes, usize::checked_add)
    .ok_or(DurableError::Refused(
        "PG Work revision retained evidence overflow",
    ))?;
    if new_retained_bytes > MAX_RETAINED_BYTES {
        return Err(DurableError::Refused(
            "PG Work revision retained evidence budget",
        ));
    }
    let receipt_digest = receipt_digest(
        domain,
        &store_id,
        &evidence.source_path,
        &evidence.record_id,
        next_generation,
        predecessor,
        &evidence.transaction_id,
        &evidence.command_id,
        evidence.owner_configuration_raw_digest,
        evidence.owner_configuration_canonical_digest,
        evidence.request_raw_digest,
        evidence.request_canonical_digest,
        evidence.expected_publication.as_deref(),
        evidence.publication_observation_digest,
        evidence.compound_members_digest,
        &evidence.authorization_raw,
        &before_raw,
        &after_raw,
        prepare_id,
        commit_seq,
    );
    let inserted = tx.execute(
        "INSERT INTO cmd2_work_revision_history_v1(
           domain,store_id,source_path,record_id,generation,previous_receipt_digest,
           transaction_id,command_id,owner_configuration_raw_digest,
           owner_configuration_canonical_digest,request_raw_digest,request_canonical_digest,
           expected_publication,publication_observation_digest,compound_members_digest,
           authorization_raw,before_refs,after_refs,receipt_digest,prepare_id,commit_seq)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)",
        &[
            &domain,
            &&store_id[..],
            &evidence.source_path,
            &evidence.record_id,
            &as_i64(next_generation)?,
            &predecessor.map(|digest| digest.to_hex()),
            &evidence.transaction_id,
            &evidence.command_id,
            &evidence.owner_configuration_raw_digest.to_hex(),
            &evidence.owner_configuration_canonical_digest.to_hex(),
            &evidence.request_raw_digest.to_hex(),
            &evidence.request_canonical_digest.to_hex(),
            &evidence.expected_publication,
            &evidence.publication_observation_digest.to_hex(),
            &evidence.compound_members_digest.to_hex(),
            &evidence.authorization_raw,
            &before_raw,
            &after_raw,
            &receipt_digest.to_hex(),
            &prepare_id,
            &as_i64(commit_seq)?,
        ],
    )?;
    if inserted != 1 {
        return Err(DurableError::Conflict(
            "PG Work revision receipt insertion changed",
        ));
    }
    let advanced = tx.execute(
        "UPDATE cmd2_predicate SET generation=generation+1
         WHERE domain=$1 AND kind='range' AND owner=$2 AND scope=$3 AND token=$4
           AND complete AND definition_version=$5 AND generation=$6
           AND generation<9223372036854775807",
        &[
            &domain,
            &SOURCE_OWNER,
            &PREDICATE_SCOPE,
            &token,
            &definition.to_hex(),
            &as_i64(generation)?,
        ],
    )?;
    if advanced != 1 {
        return Err(DurableError::Conflict(
            "PG Work revision predicate advance changed",
        ));
    }
    Ok(())
}

pub(crate) fn load_committed(
    tx: &mut Transaction<'_>,
    domain: &str,
    prepare_id: &[u8],
    store_id: [u8; 16],
) -> DurableResult<CommitEvidence> {
    let row = tx.query_opt(
        "SELECT store_id,source_path,record_id,generation,previous_receipt_digest,transaction_id,
                command_id,owner_configuration_raw_digest,owner_configuration_canonical_digest,
                request_raw_digest,request_canonical_digest,expected_publication,
                publication_observation_digest,compound_members_digest,authorization_raw,
                before_refs,after_refs,receipt_digest,prepare_id,commit_seq
         FROM cmd2_work_revision_history_v1 WHERE domain=$1 AND prepare_id=$2 FOR SHARE",
        &[&domain, &prepare_id],
    )?.ok_or(DurableError::Refused("PG Work revision committed evidence absent"))?;
    let receipt = decode_receipt_row(
        &row,
        domain,
        &store_id,
        &row.get::<_, String>(1),
        &row.get::<_, String>(2),
    )?;
    if row.get::<_, Vec<u8>>(18) != prepare_id {
        return Err(DurableError::Corrupt(
            "PG Work revision committed prepare identity",
        ));
    }
    let chain = verify_receipt_chain(
        tx,
        domain,
        &store_id,
        &row.get::<_, String>(1),
        &row.get::<_, String>(2),
    )?;
    if chain.generation < receipt.generation {
        return Err(DurableError::Corrupt(
            "PG Work committed receipt is outside chain",
        ));
    }
    let source_path: String = row.get(1);
    let record_id: String = row.get(2);
    let generation = receipt.generation;
    let predecessor = receipt.previous_receipt_digest;
    let owner_raw = parse_digest(row.get::<_, String>(7))?;
    let owner_canonical = parse_digest(row.get::<_, String>(8))?;
    let request_raw = parse_digest(row.get::<_, String>(9))?;
    let request_canonical = parse_digest(row.get::<_, String>(10))?;
    let expected_publication = row.get::<_, Option<String>>(11);
    let publication_observation = parse_digest(row.get::<_, String>(12))?;
    let compound_members = parse_digest(row.get::<_, String>(13))?;
    let before_raw: Vec<u8> = row.get(15);
    let after_raw: Vec<u8> = row.get(16);
    let observation = Observation {
        domain: domain.to_owned(),
        store_id,
        source_path: source_path.clone(),
        record_id: record_id.clone(),
        generation: generation - 1,
        predecessor_receipt_digest: predecessor,
        selected_refs: None,
        selected: None,
        observation_digest: observation_digest(
            domain,
            &store_id,
            &source_path,
            &record_id,
            generation - 1,
            predecessor,
        ),
        retained_bytes: 0,
    };
    Ok(CommitEvidence {
        observation,
        source_path,
        record_id,
        transaction_id: receipt.transaction_id,
        command_id: receipt.command_id,
        owner_configuration_raw_digest: owner_raw,
        owner_configuration_canonical_digest: owner_canonical,
        request_raw_digest: request_raw,
        request_canonical_digest: request_canonical,
        expected_publication,
        publication_observation_digest: publication_observation,
        compound_members_digest: compound_members,
        authorization_raw: receipt.authorization_raw,
        before: decode_refs(&before_raw, &row.get::<_, String>(1))?,
        after: decode_refs(&after_raw, &row.get::<_, String>(1))?,
    })
}

fn receipt_digest(
    domain: &str,
    store_id: &[u8; 16],
    source_path: &str,
    record_id: &str,
    generation: u64,
    predecessor: Option<Digest256>,
    transaction_id: &str,
    command_id: &str,
    owner_raw: Digest256,
    owner_canonical: Digest256,
    request_raw: Digest256,
    request_canonical: Digest256,
    expected_publication: Option<&str>,
    publication_observation: Digest256,
    compound_members: Digest256,
    authorization: &[u8],
    before: &[u8],
    after: &[u8],
    prepare_id: &[u8],
    commit_seq: u64,
) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-pg-work-revision-receipt-v1");
    part(&mut hasher, domain.as_bytes());
    part(&mut hasher, store_id);
    part(&mut hasher, source_path.as_bytes());
    part(&mut hasher, record_id.as_bytes());
    part(&mut hasher, &generation.to_be_bytes());
    part(
        &mut hasher,
        predecessor
            .as_ref()
            .map_or(&[][..], |digest| digest.as_bytes()),
    );
    part(&mut hasher, transaction_id.as_bytes());
    part(&mut hasher, command_id.as_bytes());
    part(&mut hasher, owner_raw.as_bytes());
    part(&mut hasher, owner_canonical.as_bytes());
    part(&mut hasher, request_raw.as_bytes());
    part(&mut hasher, request_canonical.as_bytes());
    part(
        &mut hasher,
        expected_publication.unwrap_or("<null>").as_bytes(),
    );
    part(&mut hasher, publication_observation.as_bytes());
    part(&mut hasher, compound_members.as_bytes());
    part(&mut hasher, Digest256::of_bytes(authorization).as_bytes());
    part(&mut hasher, Digest256::of_bytes(before).as_bytes());
    part(&mut hasher, Digest256::of_bytes(after).as_bytes());
    part(&mut hasher, prepare_id);
    part(&mut hasher, &commit_seq.to_be_bytes());
    hasher.finalize()
}

fn observation_digest(
    domain: &str,
    store_id: &[u8; 16],
    source_path: &str,
    record_id: &str,
    generation: u64,
    head_receipt: Option<Digest256>,
) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    part(&mut hasher, b"tos-pg-work-revision-observation-v1");
    part(&mut hasher, domain.as_bytes());
    part(&mut hasher, store_id);
    part(&mut hasher, source_path.as_bytes());
    part(&mut hasher, record_id.as_bytes());
    part(&mut hasher, &generation.to_be_bytes());
    if let Some(digest) = head_receipt.as_ref() {
        part(&mut hasher, digest.as_bytes());
    } else {
        part(&mut hasher, &[]);
    }
    hasher.finalize()
}

fn encode_refs(refs: &[MemberVersionRef], source_path: &str) -> DurableResult<Vec<u8>> {
    let (parent, _) = source_path
        .rsplit_once('/')
        .ok_or(DurableError::Invalid("PG Work source path"))?;
    let mut expected = revision::names(source_path)
        .map_err(DurableError::Source)?
        .into_iter()
        .map(|name| format!("{parent}/{name}"))
        .collect::<Vec<_>>();
    expected.sort();
    if refs.len() != expected.len() || refs.len() != 3 {
        return Err(DurableError::Corrupt("PG Work reference member count"));
    }
    let mut ordered = refs.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.path.cmp(&right.path));
    let mut out = REF_MAGIC.to_vec();
    out.extend_from_slice(&(ordered.len() as u32).to_be_bytes());
    let mut total = out.len();
    for (reference, wanted) in ordered.into_iter().zip(expected) {
        if reference.path != wanted {
            return Err(DurableError::Corrupt("PG Work reference member path"));
        }
        let path_len = u32::try_from(reference.path.len())
            .map_err(|_| DurableError::Refused("PG Work reference path length"))?;
        total = total
            .checked_add(4 + 1 + 8 + 64 + reference.path.len())
            .filter(|bytes| *bytes <= 65_536)
            .ok_or(DurableError::Refused("PG Work reference envelope budget"))?;
        out.extend_from_slice(&path_len.to_be_bytes());
        match (reference.revision, reference.digest) {
            (None, None) => {
                out.push(0);
                out.extend_from_slice(&0u64.to_be_bytes());
                out.extend_from_slice(&[b'0'; 64]);
            }
            (Some(revision), Some(digest)) if revision > 0 => {
                out.push(1);
                out.extend_from_slice(&revision.to_be_bytes());
                out.extend_from_slice(digest.to_hex().as_bytes());
            }
            _ => return Err(DurableError::Corrupt("PG Work reference version binding")),
        }
        out.extend_from_slice(reference.path.as_bytes());
    }
    debug_assert_eq!(out.len(), total);
    Ok(out)
}

fn decode_refs(raw: &[u8], source_path: &str) -> DurableResult<Vec<MemberVersionRef>> {
    if raw.len() > 65_536 || raw.len() < REF_MAGIC.len() + 4 || !raw.starts_with(REF_MAGIC) {
        return Err(DurableError::Corrupt("PG Work reference envelope"));
    }
    let mut offset = REF_MAGIC.len();
    let count = take_u32(raw, &mut offset)? as usize;
    if count != 3 {
        return Err(DurableError::Corrupt("PG Work reference count"));
    }
    let (parent, _) = source_path
        .rsplit_once('/')
        .ok_or(DurableError::Invalid("PG Work source path"))?;
    let mut expected = revision::names(source_path)
        .map_err(DurableError::Source)?
        .into_iter()
        .map(|name| format!("{parent}/{name}"))
        .collect::<Vec<_>>();
    expected.sort();
    let mut output = Vec::with_capacity(count);
    let mut previous: Option<String> = None;
    for wanted in expected {
        let path_len = take_u32(raw, &mut offset)? as usize;
        let flag_end = offset
            .checked_add(1)
            .ok_or(DurableError::Corrupt("PG Work reference offset"))?;
        let flag = *raw
            .get(offset)
            .ok_or(DurableError::Corrupt("PG Work reference flag"))?;
        offset = flag_end;
        let revision = take_u64(raw, &mut offset)?;
        let digest_end = offset
            .checked_add(64)
            .ok_or(DurableError::Corrupt("PG Work reference offset"))?;
        let digest_text = std::str::from_utf8(
            raw.get(offset..digest_end)
                .ok_or(DurableError::Corrupt("PG Work reference digest"))?,
        )
        .map_err(|_| DurableError::Corrupt("PG Work reference digest UTF-8"))?;
        offset = digest_end;
        let path_end = offset
            .checked_add(path_len)
            .filter(|end| *end <= raw.len())
            .ok_or(DurableError::Corrupt("PG Work reference path envelope"))?;
        let path = std::str::from_utf8(&raw[offset..path_end])
            .map_err(|_| DurableError::Corrupt("PG Work reference path UTF-8"))?
            .to_owned();
        offset = path_end;
        if path != wanted || previous.as_ref().is_some_and(|old| old >= &path) {
            return Err(DurableError::Corrupt("PG Work reference path order"));
        }
        RelativePath::parse(&path).map_err(|_| DurableError::Corrupt("PG Work reference path"))?;
        let (revision, digest) = match flag {
            0 if revision == 0 && digest_text == "0".repeat(64) => (None, None),
            1 if revision > 0 => (
                Some(revision),
                Some(
                    Digest256::from_hex(digest_text)
                        .map_err(|_| DurableError::Corrupt("PG Work reference digest"))?,
                ),
            ),
            _ => return Err(DurableError::Corrupt("PG Work reference presence binding")),
        };
        output.push(MemberVersionRef {
            path: path.clone(),
            revision,
            digest,
        });
        previous = Some(path);
    }
    if offset != raw.len() {
        return Err(DurableError::Corrupt("PG Work reference trailing bytes"));
    }
    Ok(output)
}

fn take_u32(raw: &[u8], offset: &mut usize) -> DurableResult<u32> {
    let end = offset
        .checked_add(4)
        .ok_or(DurableError::Corrupt("PG Work envelope offset"))?;
    let bytes: [u8; 4] = raw
        .get(*offset..end)
        .ok_or(DurableError::Corrupt("PG Work envelope u32"))?
        .try_into()
        .map_err(|_| DurableError::Corrupt("PG Work envelope u32"))?;
    *offset = end;
    Ok(u32::from_be_bytes(bytes))
}

fn take_u64(raw: &[u8], offset: &mut usize) -> DurableResult<u64> {
    let end = offset
        .checked_add(8)
        .ok_or(DurableError::Corrupt("PG Work envelope offset"))?;
    let bytes: [u8; 8] = raw
        .get(*offset..end)
        .ok_or(DurableError::Corrupt("PG Work envelope u64"))?
        .try_into()
        .map_err(|_| DurableError::Corrupt("PG Work envelope u64"))?;
    *offset = end;
    Ok(u64::from_be_bytes(bytes))
}

fn parse_digest(value: String) -> DurableResult<Digest256> {
    Digest256::from_hex(value.trim_end())
        .map_err(|_| DurableError::Corrupt("PG Work revision digest"))
}

fn as_u64(value: i64) -> DurableResult<u64> {
    u64::try_from(value).map_err(|_| DurableError::Corrupt("PG Work revision integer"))
}

fn as_i64(value: u64) -> DurableResult<i64> {
    i64::try_from(value).map_err(|_| DurableError::Refused("PG Work revision integer range"))
}

fn part(hasher: &mut Digest256Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}
