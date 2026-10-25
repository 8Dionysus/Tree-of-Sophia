//! Complete immediate-child source-index scan with original query DATA and
//! local owned-buffer/readset admission. No retained-subset EOF inference.
//! Driver/error/network/allocator capacity/native activation costs remain OPEN.
use super::{
    MAX_CHILDREN, OWNER, SourceRevisionReadSession, as_i64, as_u64, finish_source_revision_epoch,
    parse_hex, query_data,
};
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use crate::source_native_cli::private_assessment::OperationBudget;
use crate::{DurableError, DurableResult, PredicateKind, PredicateRead, PredicateToken};
use std::{mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::RelativePath;

const SETUP: &str = "SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'";
// Boolean projection bounds corrupt/non-normal maintenance strings without
// decoding them as a supposedly six-byte String.
const AUDIT: &str =
    "SELECT generation,maintenance_state='normal' FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE";
const LENGTHS: &str = "SELECT COALESCE(octet_length(pg_catalog.bpcharsend(source_definition_digest)),0),COALESCE(octet_length(pg_catalog.bpcharsend(contract_digest)),0) FROM cmd2_domain WHERE domain=$1 FOR SHARE";
const DOMAIN: &str = "SELECT rights_allowed,source_complete,source_generation,source_epoch,contract_digest,source_definition_digest FROM cmd2_domain WHERE domain=$1 FOR SHARE";
const PREDICATE_ENVELOPE: &str = "SELECT count(*)::bigint,COALESCE(sum(n),0)::bigint,COALESCE(max(n),0)::bigint FROM (SELECT octet_length(pg_catalog.textsend(definition_version))::bigint AS n FROM cmd2_predicate WHERE domain=$1 AND kind='range' AND owner=$2 AND scope='source-home' AND token=$3 FOR SHARE) AS selected";
const PREDICATE: &str = "SELECT generation,definition_version FROM cmd2_predicate WHERE domain=$1 AND kind='range' AND owner=$2 AND scope='source-home' AND token=$3 FOR SHARE";
const CHILD_ENVELOPE: &str = "SELECT count(*)::bigint,COALESCE(sum(n),0)::bigint,COALESCE(max(n),0)::bigint FROM (SELECT octet_length(pg_catalog.textsend(subject))::bigint AS n FROM cmd2_current WHERE domain=$1 AND subject COLLATE \"C\" >= $2 COLLATE \"C\" AND ($4 OR subject COLLATE \"C\" > $2 COLLATE \"C\") AND subject COLLATE \"C\" < $3 COLLATE \"C\" ORDER BY subject COLLATE \"C\" LIMIT 1 FOR SHARE) AS selected";
const CHILD: &str = "SELECT subject FROM cmd2_current WHERE domain=$1 AND subject COLLATE \"C\" >= $2 COLLATE \"C\" AND ($4 OR subject COLLATE \"C\" > $2 COLLATE \"C\") AND subject COLLATE \"C\" < $3 COLLATE \"C\" ORDER BY subject COLLATE \"C\" LIMIT 1 FOR SHARE";
fn overflow() -> DurableError {
    DurableError::Source(SourceCommandError::Unsupported(
        "PG directory data quote overflow",
    ))
}
fn add(a: usize, b: usize) -> DurableResult<usize> {
    a.checked_add(b).ok_or_else(overflow)
}
fn mul(a: usize, b: usize) -> DurableResult<usize> {
    a.checked_mul(b).ok_or_else(overflow)
}
fn debit(b: &mut OperationBudget, bytes: usize, work: usize) -> DurableResult<()> {
    b.charge_work(work).map_err(DurableError::Source)?;
    b.charge_bytes(bytes).map_err(DurableError::Source)
}
fn quote(
    b: &mut OperationBudget,
    sql: &'static str,
    params: &[usize],
    rows: usize,
    fields: usize,
    payload: usize,
    decoded: usize,
    strings: usize,
    d: Instant,
    c: &AtomicBool,
) -> DurableResult<()> {
    query_data::table_statement(
        b, sql, params, rows, fields, payload, decoded, strings, d, c,
    )
    .map_err(DurableError::Source)
}
fn length(value: i32) -> DurableResult<usize> {
    usize::try_from(value)
        .map_err(|_| DurableError::Corrupt("negative source directory sender length"))
}
fn envelope(row: &postgres::Row) -> DurableResult<(usize, usize)> {
    envelope_values(row.get(0), row.get(1), row.get(2))
}
fn envelope_values(count: i64, total: i64, maximum: i64) -> DurableResult<(usize, usize)> {
    let count = usize::try_from(count).map_err(|_| overflow())?;
    let total = usize::try_from(total).map_err(|_| overflow())?;
    let maximum = usize::try_from(maximum).map_err(|_| overflow())?;
    if count > 1 || (count == 0 && (total != 0 || maximum != 0)) || (count == 1 && total != maximum)
    {
        return Err(DurableError::Corrupt(
            "source directory single-row envelope differs",
        ));
    }
    Ok((count, total))
}
/// Structural classifier only, not permission/absence/coverage. Physical
/// cmd2_current EOF cannot certify these additional virtual member surfaces.
/// Keep this conservative until their own admitted enumerator is integrated.
pub(crate) fn physical_prefix_excludes_publication_virtuals(directory: &str) -> bool {
    fn ancestor(directory: &str, path: &str) -> bool {
        path.strip_prefix(directory)
            .is_some_and(|tail| tail.starts_with('/'))
    }
    let control = super::SOURCE_PUBLICATION_CONTROL_PATH;
    let journal = super::SOURCE_PUBLICATION_JOURNAL_ROOT;
    directory != control
        && !ancestor(directory, control)
        && directory != journal
        && !ancestor(directory, journal)
        && !ancestor(journal, directory)
}
pub(crate) fn publication_prefix_discriminator_work(directory_bytes: usize) -> Option<usize> {
    // Two equality and three ancestor comparisons: both complete operands
    // plus three slash probes and five comparison events, BEFORE any branch.
    directory_bytes
        .checked_mul(5)?
        .checked_add(
            super::SOURCE_PUBLICATION_CONTROL_PATH
                .len()
                .checked_mul(2)?,
        )?
        .checked_add(
            super::SOURCE_PUBLICATION_JOURNAL_ROOT
                .len()
                .checked_mul(3)?,
        )?
        .checked_add(8)
}
fn insert_child(
    children: &mut Vec<(String, bool)>,
    subject: String,
    prefix: &str,
    b: &mut OperationBudget,
) -> DurableResult<(String, bool)> {
    // Before strip_prefix/split/name-empty checks, including invalid rows.
    let carriers = 2 * size_of::<String>()
        + size_of::<DurableResult<(String, bool)>>()
        + size_of::<std::collections::TryReserveError>();
    debit(b, carriers, add(add(subject.len(), prefix.len())?, 1)?)?;
    let rest = subject
        .strip_prefix(prefix)
        .ok_or(DurableError::Corrupt("source child prefix differs"))?;
    let (name, directory) = match rest.split_once('/') {
        Some((n, _)) => (n, true),
        None => (rest, false),
    };
    if name.is_empty() {
        return Err(DurableError::Corrupt("source child name empty"));
    }
    // Explicit sorted Vec avoids an unpriced BTree allocation/search. Pay
    // every actual comparison; same duplicate/collision and64-child bound.
    let mut at = children.len();
    for (index, (existing, _)) in children.iter().enumerate() {
        debit(b, 0, add(add(existing.len(), name.len())?, 1)?)?;
        match existing.as_str().cmp(name) {
            std::cmp::Ordering::Equal => {
                return Err(DurableError::Corrupt(
                    "source child file and directory collide",
                ));
            }
            std::cmp::Ordering::Greater => {
                at = index;
                break;
            }
            std::cmp::Ordering::Less => (),
        }
    }
    if children.len() == MAX_CHILDREN {
        return Err(DurableError::Refused(
            "source revision immediate child budget exceeded",
        ));
    }
    // One retained name plus one directory cursor. File cursors MOVE the
    // already admitted subject String; they never skip foo.bar after foo.
    let cursor_bytes = if directory {
        add(add(prefix.len(), name.len())?, 1)?
    } else {
        0
    };
    let shifted = mul(children.len() - at, size_of::<(String, bool)>())?;
    debit(
        b,
        add(name.len(), cursor_bytes)?,
        add(add(name.len(), cursor_bytes)?, shifted)?,
    )?;
    let owned = name.to_owned();
    let next = if directory {
        let mut cursor = String::new();
        cursor
            .try_reserve_exact(cursor_bytes)
            .map_err(|_| overflow())?;
        cursor.push_str(prefix);
        cursor.push_str(name);
        cursor.push('0');
        cursor
    } else {
        subject
    };
    children.insert(at, (owned, directory));
    Ok((next, directory))
}

impl SourceRevisionReadSession<'_> {
    /// Complete PHYSICAL immediate children, not top-N. The separate prefix
    /// classifier refuses to promote this into virtual-publication coverage.
    /// Each directory's genuine range
    /// predicate is recorded only after the indexed scan reaches actual None.
    /// An error/cap/refusal returns no partial children or EOF carrier.
    pub(crate) fn list_immediate_children_with_data_admission(
        &mut self,
        parent: &RelativePath,
        b: &mut OperationBudget,
        d: Instant,
        c: &AtomicBool,
    ) -> DurableResult<Vec<(String, bool)>> {
        let result = (|| {
            active(d, c).map_err(DurableError::Source)?;
            let parent = parent.as_str();
            debit(b, 0, add(parent.len(), 1)?)?;
            if parent != "ToS" && !parent.starts_with("ToS/") {
                return Err(DurableError::Refused(
                    "bounded source revision directory outside authored namespace",
                ));
            }
            self.verify_fresh_with_query_data_admission(b, d, c)?;
            query_data::begin(b, d, c).map_err(DurableError::Source)?;
            let fixed = add(
                mul(MAX_CHILDREN, size_of::<(String, bool)>())?,
                add(
                    4 * size_of::<String>() + size_of::<Vec<(String, bool)>>(),
                    5 * size_of::<postgres::Row>()
                        + 3 * size_of::<Option<postgres::Row>>()
                        + size_of::<Result<Option<postgres::Row>, postgres::Error>>()
                        + 2 * size_of::<DurableResult<Vec<(String, bool)>>>(),
                )?,
            )?;
            let prefix_bytes = add(parent.len(), 1)?;
            debit(b, add(fixed, mul(3, prefix_bytes)?)?, mul(3, prefix_bytes)?)?;
            let mut prefix = String::new();
            prefix
                .try_reserve_exact(prefix_bytes)
                .map_err(|_| overflow())?;
            prefix.push_str(parent);
            prefix.push('/');
            let mut upper = String::new();
            upper
                .try_reserve_exact(prefix_bytes)
                .map_err(|_| overflow())?;
            upper.push_str(parent);
            upper.push('0');
            let mut lower = prefix.clone();
            let mut children = Vec::new();
            children
                .try_reserve_exact(MAX_CHILDREN)
                .map_err(|_| overflow())?;
            let domain = self.selection.cohort.domain.as_str();
            let mut tx = self.coordinator.client.transaction()?;
            query_data::statement(b, SETUP, &[], query_data::RowShape::None, d, c)
                .map_err(DurableError::Source)?;
            tx.batch_execute(SETUP)?;
            quote(b, AUDIT, &[domain.len()], 1, 2, 9, 0, 0, d, c)?;
            let fence = tx.query_one(AUDIT, &[&domain])?;
            if !fence.get::<_, bool>(1) || as_u64(fence.get(0))? != self.selection.audit_generation
            {
                return Err(DurableError::Conflict(
                    "bounded source directory audit fence changed",
                ));
            }
            quote(b, LENGTHS, &[domain.len()], 1, 2, 8, 0, 0, d, c)?;
            let lengths = tx.query_one(LENGTHS, &[&domain])?;
            let definition = length(lengths.get(0))?;
            let contract = length(lengths.get(1))?;
            quote(
                b,
                DOMAIN,
                &[domain.len()],
                1,
                6,
                add(18, add(definition, contract)?)?,
                add(add(mul(2, definition)?, contract)?, 128)?,
                5,
                d,
                c,
            )?;
            let gate = tx.query_one(DOMAIN, &[&domain])?;
            super::super::require_current_source_definition(parse_hex(
                gate.get::<_, Option<String>>(5)
                    .ok_or(DurableError::Corrupt("source definition absent"))?,
            )?)?;
            if !gate.get::<_, bool>(0)
                || !gate.get::<_, bool>(1)
                || gate.get::<_, Option<i64>>(2) != Some(as_i64(self.selection.source_generation)?)
                || gate.get::<_, Option<i64>>(3) != Some(as_i64(self.selection.epoch)?)
                || gate.get::<_, String>(4) != self.selection.contract.to_hex()
                || gate.get::<_, Option<String>>(5)
                    != Some(self.selection.cohort.definition.to_hex())
            {
                return Err(DurableError::Conflict(
                    "bounded source directory read epoch changed",
                ));
            }
            let predicate_params = [domain.len(), OWNER.len(), parent.len()];
            quote(
                b,
                PREDICATE_ENVELOPE,
                &predicate_params,
                1,
                3,
                24,
                0,
                0,
                d,
                c,
            )?;
            let ep = tx.query_one(PREDICATE_ENVELOPE, &[&domain, &OWNER, &parent])?;
            let (pn, pbytes) = envelope(&ep)?;
            quote(
                b,
                PREDICATE,
                &predicate_params,
                pn,
                2,
                add(mul(pn, 8)?, pbytes)?,
                add(pbytes, mul(pn, 64)?)?,
                2 * pn,
                d,
                c,
            )?;
            let predicate = tx.query_opt(PREDICATE, &[&domain, &OWNER, &parent])?;
            if usize::from(predicate.is_some()) != pn {
                return Err(DurableError::Conflict(
                    "source directory predicate envelope changed",
                ));
            }
            let generation = predicate
                .as_ref()
                .map(|row| as_u64(row.get(0)))
                .transpose()?
                .unwrap_or(0);
            if predicate.as_ref().is_some_and(|row| {
                row.get::<_, String>(1) != self.selection.cohort.definition.to_hex()
            }) {
                return Err(DurableError::Conflict(
                    "source directory predicate definition differs",
                ));
            }
            let mut inclusive = true;
            loop {
                active(d, c).map_err(DurableError::Source)?;
                let params = [domain.len(), lower.len(), upper.len(), 1];
                quote(b, CHILD_ENVELOPE, &params, 1, 3, 24, 0, 0, d, c)?;
                let ep = tx.query_one(CHILD_ENVELOPE, &[&domain, &lower, &upper, &inclusive])?;
                let (count, bytes) = envelope(&ep)?;
                quote(b, CHILD, &params, count, 1, bytes, bytes, count, d, c)?;
                let row = tx.query_opt(CHILD, &[&domain, &lower, &upper, &inclusive])?;
                if usize::from(row.is_some()) != count {
                    return Err(DurableError::Conflict(
                        "source directory child envelope changed",
                    ));
                }
                let Some(row) = row else { break };
                Self::charge_work(&mut self.work.directory_index_rows_examined, 1)?;
                let subject: String = row.get(0);
                if subject.len() != bytes {
                    return Err(DurableError::Conflict(
                        "source directory subject sender length changed",
                    ));
                }
                (lower, inclusive) = insert_child(&mut children, subject, &prefix, b)?;
            }
            tx.commit()?;
            self.record_directory_generation_admitted(parent, generation, b)?;
            Ok(children)
        })();
        finish_source_revision_epoch(result, || {
            self.verify_fresh_with_query_data_admission(b, d, c)
        })
    }

    fn record_directory_generation_admitted(
        &mut self,
        parent: &str,
        generation: u64,
        b: &mut OperationBudget,
    ) -> DurableResult<()> {
        // Actual readset duplicate/stability proof, no new BTree cache. The
        // legacy cache may add an identical later duplicate; it cannot erase
        // or weaken this range observation. New admitted calls dedup directly.
        debit(b, 64 + size_of::<String>(), 64)?;
        let definition = self.selection.cohort.definition.to_hex();
        for read in &self.reads.reads {
            debit(b, 0, 1)?;
            if let PredicateRead::Generation {
                predicate,
                observed_generation,
            } = read
            {
                let bytes = add(
                    add(predicate.owner.len(), OWNER.len())?,
                    add(
                        add(predicate.scope.len(), "source-home".len())?,
                        add(predicate.token.len(), parent.len())?,
                    )?,
                )?;
                debit(b, 0, add(bytes, 4)?)?;
                if predicate.kind == PredicateKind::Range
                    && predicate.owner == OWNER
                    && predicate.scope == "source-home"
                    && predicate.token == parent
                {
                    debit(
                        b,
                        0,
                        add(
                            add(predicate.definition_version.len(), definition.len())?,
                            1,
                        )?,
                    )?;
                    if predicate.definition_version != definition {
                        return Err(DurableError::Conflict(
                            "source directory observed predicate definition differs",
                        ));
                    }
                    if *observed_generation != generation {
                        return Err(DurableError::Conflict(
                            "bounded source predicate generation changed within read session",
                        ));
                    }
                    return Ok(());
                }
            }
        }
        let reads = &self.reads.reads;
        let needed = reads.len().checked_add(1).ok_or_else(overflow)?;
        let (allocation, relocation) = if needed > reads.capacity() {
            let cap = reads
                .capacity()
                .checked_mul(2)
                .ok_or_else(overflow)?
                .max(needed)
                .max(4);
            (
                mul(cap, size_of::<PredicateRead>())?,
                mul(reads.capacity(), size_of::<PredicateRead>())?,
            )
        } else {
            (0, 0)
        };
        // The already paid definition String is MOVED into this token.
        let text = add(add(OWNER.len(), "source-home".len())?, parent.len())?;
        debit(
            b,
            add(
                add(allocation, text)?,
                size_of::<PredicateRead>()
                    + size_of::<PredicateToken>()
                    + size_of::<SourceCommandResult<()>>(),
            )?,
            add(add(relocation, text)?, 1)?,
        )?;
        self.reads.reads.push(PredicateRead::Generation {
            predicate: PredicateToken {
                kind: PredicateKind::Range,
                owner: OWNER.to_owned(),
                scope: "source-home".to_owned(),
                token: parent.to_owned(),
                definition_version: definition,
            },
            observed_generation: generation,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, time::Duration};
    fn budget() -> OperationBudget {
        let d = Instant::now() + Duration::from_secs(30);
        OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            d,
        )
        .unwrap()
    }
    fn slots() -> Vec<(String, bool)> {
        Vec::with_capacity(MAX_CHILDREN)
    }
    #[test]
    fn directory_sender_envelope_requires_actual_zero_or_single_row_lengths() {
        assert_eq!(envelope_values(0, 0, 0).unwrap(), (0, 0));
        assert_eq!(envelope_values(1, 0, 0).unwrap(), (1, 0));
        assert_eq!(envelope_values(1, 4097, 4097).unwrap(), (1, 4097));
        for (count, total, maximum) in [(2, 8, 4), (0, 1, 1), (1, 8, 7), (-1, 0, 0), (1, -1, -1)] {
            assert!(envelope_values(count, total, maximum).is_err());
        }
    }
    #[test]
    fn physical_prefix_classifier_never_infers_virtual_publication_eof() {
        for prefix in [
            "ToS",
            "ToS/source-witnesses",
            "ToS/source-witnesses/.metadata-publication.json",
            "ToS/source-witnesses/.metadata-transactions",
            "ToS/source-witnesses/.metadata-transactions/selected",
        ] {
            assert!(!physical_prefix_excludes_publication_virtuals(prefix));
        }
        for prefix in [
            "ToS/contracts",
            "ToS/source-witnesses/agents",
            "ToS/source-witnesses/.metadata-transactions-other",
        ] {
            assert!(physical_prefix_excludes_publication_virtuals(prefix));
        }
        // Predicate on path spellings only: no source session or EOF token.
    }
    #[test]
    fn admitted_directory_cursor_keeps_prefixed_files_and_skips_only_directory_descendants() {
        // Local DATA helper only: no session, transaction, readset or EOF
        // token is constructed by these strings.
        let mut b = budget();
        let mut out = slots();
        assert_eq!(
            insert_child(&mut out, "ToS/x/foo".into(), "ToS/x/", &mut b).unwrap(),
            ("ToS/x/foo".into(), false)
        );
        assert_eq!(
            insert_child(&mut out, "ToS/x/foo.bar".into(), "ToS/x/", &mut b).unwrap(),
            ("ToS/x/foo.bar".into(), false)
        );
        assert_eq!(
            insert_child(&mut out, "ToS/x/dir/member.json".into(), "ToS/x/", &mut b).unwrap(),
            ("ToS/x/dir0".into(), true)
        );
        assert_eq!(
            out,
            vec![
                ("dir".into(), true),
                ("foo".into(), false),
                ("foo.bar".into(), false)
            ]
        );
    }
    #[test]
    fn admitted_directory_sorted_vec_matches_legacy_child_map_and_collision_rule() {
        let mut b = budget();
        let mut out = slots();
        let mut expected = BTreeMap::new();
        for subject in ["ToS/x/z", "ToS/x/a/member", "ToS/x/m", "ToS/x/é"] {
            let rest = subject.strip_prefix("ToS/x/").unwrap();
            let (name, dir) = match rest.split_once('/') {
                Some((n, _)) => (n, true),
                None => (rest, false),
            };
            expected.insert(name.to_owned(), dir);
            insert_child(&mut out, subject.into(), "ToS/x/", &mut b).unwrap();
        }
        assert_eq!(out, expected.into_iter().collect::<Vec<_>>());
        let before = out.clone();
        assert!(matches!(
            insert_child(&mut out, "ToS/x/a".into(), "ToS/x/", &mut b),
            Err(DurableError::Corrupt(
                "source child file and directory collide"
            ))
        ));
        assert_eq!(out, before);
    }
    #[test]
    fn admitted_directory_cap_and_invalid_rows_never_return_partial_success() {
        let mut b = budget();
        let mut out = slots();
        for i in 0..MAX_CHILDREN {
            out.push((format!("n{i:03}"), false));
        }
        let before = out.clone();
        assert!(matches!(
            insert_child(&mut out, "ToS/x/new".into(), "ToS/x/", &mut b),
            Err(DurableError::Refused(
                "source revision immediate child budget exceeded"
            ))
        ));
        assert_eq!(out, before);
        // Duplicate remains the earlier legacy error even when count is64.
        assert!(matches!(
            insert_child(&mut out, "ToS/x/n000".into(), "ToS/x/", &mut b),
            Err(DurableError::Corrupt(
                "source child file and directory collide"
            ))
        ));
        assert!(matches!(
            insert_child(&mut out, "ToS/x/".into(), "ToS/x/", &mut b),
            Err(DurableError::Corrupt("source child name empty"))
        ));
        assert!(matches!(
            insert_child(&mut out, "ToS/other/x".into(), "ToS/x/", &mut b),
            Err(DurableError::Corrupt("source child prefix differs"))
        ));
    }
    #[test]
    fn admitted_directory_refuses_before_child_mutation_and_keeps_the_spent_prefix() {
        let mut b = budget();
        b.lower_remaining(1_048_576, 25).unwrap();
        b.charge_work(10).unwrap();
        let mut out = slots();
        // Exact first inspection for subject9 + prefix6 + one visit =16;
        // remaining15 cannot pass it. The earlier10 stays spent.
        assert!(matches!(
            insert_child(&mut out, "ToS/x/foo".into(), "ToS/x/", &mut b),
            Err(DurableError::Source(SourceCommandError::Invalid(
                "assessment cumulative work budget"
            )))
        ));
        assert_eq!(b.test_work_remaining(), 15);
        assert!(out.is_empty());
        assert!(b.limits().is_err());
    }
}
