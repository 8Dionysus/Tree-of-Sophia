use std::collections::HashMap;
use std::time::{Duration, Instant};

use postgres::{Client, IsolationLevel, NoTls, Transaction};
use tos_foundation::{Digest256, Digest256Hasher};

use crate::{
    AuthorityFence, Candidate, CommitReceipt, Error, PredicateRead, PredicateToken, Result,
    synthetic_delta_digest,
};

const MAX_LAB_WRITES: usize = 64;
const MAX_LAB_READS: usize = 1024;
const MAX_LAB_INVALIDATIONS_PER_WRITE: usize = 1024;
const MAX_LAB_CUT_EVENTS: u64 = 100_000;

/// Measured on the client: lock wait includes network round-trip, so it is an
/// upper-bound observation rather than PostgreSQL's internal wait metric.
#[derive(Clone, Debug)]
pub struct Timing {
    pub lock_wait: Duration,
    pub lock_held: Duration,
    pub transaction: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cut {
    pub domain: String,
    pub through_commit_seq: u64,
    pub log_digest: Digest256,
    pub command_ids: Vec<String>,
}

/// Single-connection adapter. Callers use separate instances for concurrent
/// clients. All methods target a dedicated synthetic laboratory database.
pub struct PgCoordinator {
    client: Client,
}

impl PgCoordinator {
    pub fn connect(database_url: &str) -> Result<Self> {
        Ok(Self {
            client: Client::connect(database_url, NoTls)?,
        })
    }

    /// Identifies this connection in PostgreSQL's lock-wait observation view.
    pub fn backend_pid(&mut self) -> Result<i32> {
        Ok(self
            .client
            .query_one("SELECT pg_backend_pid()", &[])?
            .get(0))
    }

    pub fn init_lab_schema(&mut self) -> Result<()> {
        self.client.batch_execute(include_str!("schema.sql"))?;
        Ok(())
    }

    pub fn create_domain(&mut self, domain: &str, contract_digest: Digest256) -> Result<()> {
        self.client.execute(
            "INSERT INTO cmd1_coordinator(domain,contract_digest) VALUES($1,$2)
             ON CONFLICT DO NOTHING",
            &[&domain, &contract_digest.to_hex()],
        )?;
        self.client.execute(
            "INSERT INTO cmd1_publication(domain) VALUES($1) ON CONFLICT DO NOTHING",
            &[&domain],
        )?;
        Ok(())
    }

    pub fn set_job_epoch(&mut self, domain: &str, job_id: &str, epoch: u64) -> Result<()> {
        let epoch = to_i64(epoch)?;
        let changed = self.client.execute(
            "INSERT INTO cmd1_job(domain, job_id, fence_epoch) VALUES($1,$2,$3)
             ON CONFLICT(domain,job_id) DO UPDATE SET fence_epoch=EXCLUDED.fence_epoch
             WHERE EXCLUDED.fence_epoch >= cmd1_job.fence_epoch",
            &[&domain, &job_id, &epoch],
        )?;
        if changed != 1 {
            return Err(Error::Refused("job fence epoch cannot decrease"));
        }
        Ok(())
    }

    pub fn register_predicate(
        &mut self,
        domain: &str,
        predicate: &PredicateToken,
        complete: bool,
    ) -> Result<u64> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        let row = tx.query_one(
            "SELECT head_seq FROM cmd1_coordinator WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        let seq = from_i64(row.get::<_, i64>(0))? + 1;
        let seq_db = to_i64(seq)?;
        tx.execute(
            "INSERT INTO cmd1_predicate(domain,kind,owner,scope,token,definition_version,complete)
             VALUES($1,$2,$3,$4,$5,$6,$7)
             ON CONFLICT(domain,kind,owner,scope,token) DO UPDATE
             SET definition_version=EXCLUDED.definition_version,
                 complete=EXCLUDED.complete,
                 generation=cmd1_predicate.generation+1",
            &[
                &domain,
                &predicate.kind.as_str(),
                &predicate.owner,
                &predicate.scope,
                &predicate.token,
                &predicate.definition_version,
                &complete,
            ],
        )?;
        tx.execute(
            "UPDATE cmd1_coordinator SET head_seq=$2 WHERE domain=$1",
            &[&domain, &seq_db],
        )?;
        let event = format!("predicate.update:{seq}");
        tx.execute(
            "INSERT INTO cmd1_commit_log(domain,commit_seq,event_kind,command_id,delta_digest,members)
             VALUES($1,$2,'rule',$3,$4,$5)",
            &[&domain, &seq_db, &event, &zero_digest(), &Vec::<String>::new()],
        )?;
        tx.execute(
            "INSERT INTO cmd1_outbox(domain,commit_seq,event_id) VALUES($1,$2,$3)",
            &[&domain, &seq_db, &event],
        )?;
        tx.commit()?;
        Ok(seq)
    }

    pub fn predicate_generation(&mut self, domain: &str, token: &PredicateToken) -> Result<u64> {
        let row = self.client.query_opt(
            "SELECT generation FROM cmd1_predicate WHERE domain=$1 AND kind=$2 AND owner=$3
             AND scope=$4 AND token=$5 AND definition_version=$6 AND complete",
            &[
                &domain,
                &token.kind.as_str(),
                &token.owner,
                &token.scope,
                &token.token,
                &token.definition_version,
            ],
        )?;
        match row {
            Some(row) => from_i64(row.get(0)),
            None => Err(Error::Refused("predicate coverage incomplete")),
        }
    }

    pub fn head_seq(&mut self, domain: &str) -> Result<u64> {
        let row = self.client.query_one(
            "SELECT head_seq FROM cmd1_coordinator WHERE domain=$1",
            &[&domain],
        )?;
        from_i64(row.get(0))
    }

    pub fn local_rights_version(&mut self, domain: &str) -> Result<u64> {
        let row = self.client.query_one(
            "SELECT rights_version FROM cmd1_coordinator WHERE domain=$1",
            &[&domain],
        )?;
        from_i64(row.get(0))
    }

    pub fn revoke_local(&mut self, domain: &str) -> Result<u64> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        let row = tx.query_one(
            "SELECT head_seq FROM cmd1_coordinator WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        let seq = from_i64(row.get::<_, i64>(0))? + 1;
        let seq_db = to_i64(seq)?;
        tx.execute(
            "UPDATE cmd1_coordinator SET head_seq=$2, rights_version=rights_version+1,
             rights_allowed=false WHERE domain=$1",
            &[&domain, &seq_db],
        )?;
        let event = format!("rights.revoke:{seq}");
        tx.execute(
            "INSERT INTO cmd1_commit_log(domain,commit_seq,event_kind,command_id,delta_digest,members)
             VALUES($1,$2,'rights',$3,$4,$5)",
            &[
                &domain,
                &seq_db,
                &event,
                &zero_digest(),
                &Vec::<String>::new(),
            ],
        )?;
        tx.execute(
            "INSERT INTO cmd1_outbox(domain,commit_seq,event_id) VALUES($1,$2,$3)",
            &[&domain, &seq_db, &event],
        )?;
        tx.commit()?;
        Ok(seq)
    }

    pub fn set_rule_version(&mut self, domain: &str, version: u64) -> Result<u64> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        let row = tx.query_one(
            "SELECT head_seq FROM cmd1_coordinator WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        let seq = from_i64(row.get::<_, i64>(0))? + 1;
        let seq_db = to_i64(seq)?;
        tx.execute(
            "UPDATE cmd1_coordinator SET head_seq=$2,rule_version=$3 WHERE domain=$1",
            &[&domain, &seq_db, &to_i64(version)?],
        )?;
        let event = format!("rule.update:{seq}");
        tx.execute(
            "INSERT INTO cmd1_commit_log(domain,commit_seq,event_kind,command_id,delta_digest,members)
             VALUES($1,$2,'rule',$3,$4,$5)",
            &[
                &domain,
                &seq_db,
                &event,
                &zero_digest(),
                &Vec::<String>::new(),
            ],
        )?;
        tx.execute(
            "INSERT INTO cmd1_outbox(domain,commit_seq,event_id) VALUES($1,$2,$3)",
            &[&domain, &seq_db, &event],
        )?;
        tx.commit()?;
        Ok(seq)
    }

    pub fn set_contract_digest(&mut self, domain: &str, digest: Digest256) -> Result<u64> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        let row = tx.query_one(
            "SELECT head_seq FROM cmd1_coordinator WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        let seq = from_i64(row.get::<_, i64>(0))? + 1;
        let seq_db = to_i64(seq)?;
        tx.execute(
            "UPDATE cmd1_coordinator SET head_seq=$2,contract_digest=$3 WHERE domain=$1",
            &[&domain, &seq_db, &digest.to_hex()],
        )?;
        let event = format!("contract.update:{seq}");
        tx.execute(
            "INSERT INTO cmd1_commit_log(domain,commit_seq,event_kind,command_id,delta_digest,members)
             VALUES($1,$2,'rule',$3,$4,$5)",
            &[&domain, &seq_db, &event, &digest.to_hex(), &Vec::<String>::new()],
        )?;
        tx.execute(
            "INSERT INTO cmd1_outbox(domain,commit_seq,event_id) VALUES($1,$2,$3)",
            &[&domain, &seq_db, &event],
        )?;
        tx.commit()?;
        Ok(seq)
    }

    pub fn commit(&mut self, candidate: &Candidate) -> Result<(CommitReceipt, Timing)> {
        if candidate.domain.is_empty()
            || candidate.command_id.is_empty()
            || candidate.input_profile_id != "raw-lab-v1"
            || candidate.writes.is_empty()
            || candidate.writes.len() > MAX_LAB_WRITES
            || candidate.reads.len() > MAX_LAB_READS
            || candidate.attestation.prepare_overlay_id.is_empty()
            || candidate.attestation.prepare_delta_digest != candidate.delta_digest
            || synthetic_delta_digest(&candidate.writes) != candidate.delta_digest
            || candidate
                .writes
                .iter()
                .any(|write| write.invalidates.len() > MAX_LAB_INVALIDATIONS_PER_WRITE)
        {
            return Err(Error::InvalidInput(
                "invalid synthetic command identity/profile/delta",
            ));
        }
        if matches!(
            &candidate.authority,
            AuthorityFence::ExternalUnsupported { .. }
        ) {
            return Err(Error::UnsupportedExternalAuthority);
        }
        let tx_start = Instant::now();
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        let lock_start = Instant::now();
        // First data read acquires the row lock. PostgreSQL can take the
        // locking statement's snapshot before waiting, so the authoritative
        // state is fetched by a separate READ COMMITTED statement afterwards.
        tx.query_one(
            "SELECT 1 FROM cmd1_coordinator WHERE domain=$1 FOR UPDATE",
            &[&candidate.domain],
        )?;
        let lock_wait = lock_start.elapsed();
        let lock_acquired = Instant::now();
        let row = tx.query_one(
            "SELECT head_seq,rights_version,rights_allowed,rule_version,contract_digest
             FROM cmd1_coordinator WHERE domain=$1",
            &[&candidate.domain],
        )?;
        let head = from_i64(row.get::<_, i64>(0))?;
        if head >= MAX_LAB_CUT_EVENTS {
            return Err(Error::Refused("laboratory publication cut budget exceeded"));
        }
        let rights_version = from_i64(row.get::<_, i64>(1))?;
        let rights_allowed: bool = row.get(2);
        let rule_version = from_i64(row.get::<_, i64>(3))?;
        let contract_digest: String = row.get(4);
        if !rights_allowed {
            return Err(Error::Refused("current local rights revoked"));
        }
        match &candidate.authority {
            AuthorityFence::Local { expected_version } if *expected_version == rights_version => {}
            AuthorityFence::Local { .. } => return Err(Error::Refused("local rights changed")),
            AuthorityFence::ExternalUnsupported { .. } => {
                return Err(Error::UnsupportedExternalAuthority);
            }
        }
        let previous = tx.query_opt(
            "SELECT commit_seq,raw_request_digest,delta_digest,attestation_digest,input_profile_id
             FROM cmd1_receipt WHERE domain=$1 AND command_id=$2",
            &[&candidate.domain, &candidate.command_id],
        )?;
        if let Some(previous) = previous {
            let digest: String = previous.get(1);
            let delta: String = previous.get(2);
            let profile: String = previous.get(4);
            if digest != candidate.raw_request_digest.to_hex()
                || delta != candidate.delta_digest.to_hex()
                || profile != candidate.input_profile_id
            {
                return Err(Error::Conflict("command identity collision"));
            }
            let receipt = CommitReceipt {
                domain: candidate.domain.clone(),
                command_id: candidate.command_id.clone(),
                commit_seq: from_i64(previous.get(0))?,
                raw_request_digest: candidate.raw_request_digest,
                delta_digest: parse_digest(delta)?,
                attestation_digest: parse_digest(previous.get(3))?,
                replayed: true,
            };
            tx.commit()?;
            return Ok((
                receipt,
                Timing {
                    lock_wait,
                    lock_held: lock_acquired.elapsed(),
                    transaction: tx_start.elapsed(),
                },
            ));
        }
        if candidate.expected_rule_version != rule_version {
            return Err(Error::Conflict("rule version changed"));
        }
        if candidate.expected_contract_digest.to_hex() != contract_digest {
            return Err(Error::Conflict("schema/registry/backend contract changed"));
        }
        if candidate.full_base_seq.is_some_and(|base| base != head) {
            return Err(Error::Conflict("FullOnly base changed; audit outside lock"));
        }
        let lease = tx.query_opt(
            "SELECT fence_epoch FROM cmd1_job WHERE domain=$1 AND job_id=$2 FOR UPDATE",
            &[&candidate.domain, &candidate.job_id],
        )?;
        if lease.map(|r| r.get::<_, i64>(0)) != Some(to_i64(candidate.fence_epoch)?) {
            return Err(Error::Refused("stale or missing job fence"));
        }
        for read in &candidate.reads {
            check_read(&mut tx, &candidate.domain, read)?;
        }
        let seq = head
            .checked_add(1)
            .ok_or(Error::Corrupt("sequence overflow"))?;
        let seq_db = to_i64(seq)?;
        let mut members = Vec::with_capacity(candidate.writes.len());
        for write in &candidate.writes {
            if write.bytes.lab_marker.is_empty() || write.bytes.length == 0 {
                return Err(Error::InvalidInput("synthetic byte marker/length missing"));
            }
            if write.expected_version.is_none()
                && tx
                    .query_opt(
                        "SELECT 1 FROM cmd1_record WHERE domain=$1 AND namespace=$2 AND key=$3",
                        &[&candidate.domain, &write.namespace, &write.key],
                    )?
                    .is_some()
            {
                return Err(Error::Conflict("new record key already exists"));
            }
            let next_version = match write.expected_version {
                None => {
                    tx.execute(
                        "INSERT INTO cmd1_record(domain,namespace,key,version,digest,byte_length)
                         VALUES($1,$2,$3,1,$4,$5)",
                        &[
                            &candidate.domain,
                            &write.namespace,
                            &write.key,
                            &write.bytes.digest.to_hex(),
                            &to_i64(write.bytes.length)?,
                        ],
                    )?;
                    1
                }
                Some(expected) => {
                    let next = expected
                        .checked_add(1)
                        .ok_or(Error::Corrupt("version overflow"))?;
                    let count = tx.execute(
                        "UPDATE cmd1_record SET version=$5,digest=$6,byte_length=$7
                         WHERE domain=$1 AND namespace=$2 AND key=$3 AND version=$4",
                        &[
                            &candidate.domain,
                            &write.namespace,
                            &write.key,
                            &to_i64(expected)?,
                            &to_i64(next)?,
                            &write.bytes.digest.to_hex(),
                            &to_i64(write.bytes.length)?,
                        ],
                    )?;
                    if count != 1 {
                        return Err(Error::Conflict("write version changed"));
                    }
                    next
                }
            };
            tx.execute(
                "INSERT INTO cmd1_record_history(domain,namespace,key,version,digest,byte_length,commit_seq)
                 VALUES($1,$2,$3,$4,$5,$6,$7)",
                &[
                    &candidate.domain,
                    &write.namespace,
                    &write.key,
                    &to_i64(next_version)?,
                    &write.bytes.digest.to_hex(),
                    &to_i64(write.bytes.length)?,
                    &seq_db,
                ],
            )?;
            members.push(member_commitment(
                &write.namespace,
                &write.key,
                next_version,
                write.bytes.digest,
                write.bytes.length,
            ));
            for token in &write.invalidates {
                let count = tx.execute(
                    "UPDATE cmd1_predicate SET generation=generation+1
                     WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5
                     AND definition_version=$6 AND complete",
                    &[
                        &candidate.domain,
                        &token.kind.as_str(),
                        &token.owner,
                        &token.scope,
                        &token.token,
                        &token.definition_version,
                    ],
                )?;
                if count != 1 {
                    return Err(Error::Refused("write invalidation coverage incomplete"));
                }
            }
        }
        let attestation = attestation_digest(candidate, seq, head);
        tx.execute(
            "UPDATE cmd1_coordinator SET head_seq=$2 WHERE domain=$1",
            &[&candidate.domain, &seq_db],
        )?;
        tx.execute(
            "INSERT INTO cmd1_receipt(domain,command_id,commit_seq,raw_request_digest,
             delta_digest,attestation_digest,input_profile_id)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
            &[
                &candidate.domain,
                &candidate.command_id,
                &seq_db,
                &candidate.raw_request_digest.to_hex(),
                &candidate.delta_digest.to_hex(),
                &attestation.to_hex(),
                &candidate.input_profile_id,
            ],
        )?;
        tx.execute(
            "INSERT INTO cmd1_commit_log(domain,commit_seq,event_kind,command_id,delta_digest,members)
             VALUES($1,$2,'command',$3,$4,$5)",
            &[
                &candidate.domain,
                &seq_db,
                &candidate.command_id,
                &candidate.delta_digest.to_hex(),
                &members,
            ],
        )?;
        let event_id = format!("{}:{seq}", candidate.domain);
        tx.execute(
            "INSERT INTO cmd1_outbox(domain,commit_seq,event_id) VALUES($1,$2,$3)",
            &[&candidate.domain, &seq_db, &event_id],
        )?;
        tx.commit()?;
        Ok((
            CommitReceipt {
                domain: candidate.domain.clone(),
                command_id: candidate.command_id.clone(),
                commit_seq: seq,
                raw_request_digest: candidate.raw_request_digest,
                delta_digest: candidate.delta_digest,
                attestation_digest: attestation,
                replayed: false,
            },
            Timing {
                lock_wait,
                lock_held: lock_acquired.elapsed(),
                transaction: tx_start.elapsed(),
            },
        ))
    }

    pub fn read_cut(&mut self, domain: &str) -> Result<Cut> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let row = tx.query_one(
            "SELECT head_seq FROM cmd1_coordinator WHERE domain=$1",
            &[&domain],
        )?;
        let head = from_i64(row.get::<_, i64>(0))?;
        if head > MAX_LAB_CUT_EVENTS {
            return Err(Error::Refused("laboratory publication cut budget exceeded"));
        }
        let rows = tx.query(
            "SELECT commit_seq,event_kind,command_id,delta_digest,members FROM cmd1_commit_log
             WHERE domain=$1 AND commit_seq <= $2 ORDER BY commit_seq",
            &[&domain, &to_i64(head)?],
        )?;
        verify_history_members(&mut tx, domain, head, &rows)?;
        let mut hasher = Digest256Hasher::new();
        let mut ids = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            let seq = from_i64(row.get::<_, i64>(0))?;
            if seq != index as u64 + 1 {
                return Err(Error::Corrupt("non-contiguous committed cut"));
            }
            let event_kind: String = row.get(1);
            let command_id: String = row.get(2);
            let digest: String = row.get(3);
            let members: Vec<String> = row.get(4);
            update_part(&mut hasher, &seq.to_be_bytes());
            update_part(&mut hasher, event_kind.as_bytes());
            update_part(&mut hasher, command_id.as_bytes());
            update_part(&mut hasher, digest.as_bytes());
            for member in members {
                update_part(&mut hasher, member.as_bytes());
            }
            ids.push(command_id);
        }
        if rows.len() as u64 != head {
            return Err(Error::Corrupt("committed cut has missing tail"));
        }
        tx.commit()?;
        Ok(Cut {
            domain: domain.to_owned(),
            through_commit_seq: head,
            log_digest: hasher.finalize(),
            command_ids: ids,
        })
    }

    pub fn seal_cut(&mut self, cut: &Cut) -> Result<()> {
        if u64::try_from(cut.command_ids.len()) != Ok(cut.through_commit_seq) {
            return Err(Error::InvalidInput("publication cut event count mismatch"));
        }
        if cut.through_commit_seq > MAX_LAB_CUT_EVENTS {
            return Err(Error::Refused("laboratory publication cut budget exceeded"));
        }
        // Re-read immutable log through K to reject a fabricated/stale cut.
        // This is lab-only log proof, not STO membership/index root proof.
        let current = self.read_cut(&cut.domain)?;
        if current.through_commit_seq < cut.through_commit_seq
            || current.command_ids.get(..cut.command_ids.len()) != Some(cut.command_ids.as_slice())
        {
            return Err(Error::Conflict("cut not present in current log"));
        }
        // A later head can coexist with a correct older cut. Its digest is
        // checked from the immutable prefix in the seal transaction below.
        let mut tx = self.client.transaction()?;
        let row = tx.query_one(
            "SELECT through_seq,log_digest FROM cmd1_publication WHERE domain=$1 FOR UPDATE",
            &[&cut.domain],
        )?;
        let published = from_i64(row.get::<_, i64>(0))?;
        let published_digest: Option<String> = row.get(1);
        let prefix = tx.query(
            "SELECT commit_seq,event_kind,command_id,delta_digest,members FROM cmd1_commit_log
             WHERE domain=$1 AND commit_seq <= $2 ORDER BY commit_seq",
            &[&cut.domain, &to_i64(cut.through_commit_seq)?],
        )?;
        if digest_log_rows(&prefix, cut.through_commit_seq)? != cut.log_digest {
            return Err(Error::Conflict("publication cut digest changed"));
        }
        verify_history_members(&mut tx, &cut.domain, cut.through_commit_seq, &prefix)?;
        let cut_digest_hex = cut.log_digest.to_hex();
        if cut.through_commit_seq <= published {
            if cut.through_commit_seq == published
                && published_digest.as_deref() != Some(cut_digest_hex.as_str())
            {
                return Err(Error::Corrupt("published cut digest differs from log"));
            }
            tx.commit()?;
            return Ok(());
        }
        tx.execute(
            "UPDATE cmd1_publication SET through_seq=$2,log_digest=$3 WHERE domain=$1",
            &[
                &cut.domain,
                &to_i64(cut.through_commit_seq)?,
                &cut_digest_hex,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn published_seq(&mut self, domain: &str) -> Result<u64> {
        let row = self.client.query_one(
            "SELECT through_seq FROM cmd1_publication WHERE domain=$1",
            &[&domain],
        )?;
        from_i64(row.get(0))
    }

    pub fn record_count(&mut self, domain: &str) -> Result<i64> {
        let row = self.client.query_one(
            "SELECT count(*) FROM cmd1_record WHERE domain=$1",
            &[&domain],
        )?;
        Ok(row.get(0))
    }

    pub fn read_record(
        &mut self,
        domain: &str,
        namespace: &str,
        key: &str,
    ) -> Result<Option<(u64, Digest256)>> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        // Current-rights read is held through the record read; local revoke
        // requires an incompatible FOR UPDATE lock on this coordinator row.
        let rights = tx.query_one(
            "SELECT rights_allowed FROM cmd1_coordinator WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if !rights.get::<_, bool>(0) {
            return Err(Error::Refused("current local rights revoked"));
        }
        let row = tx.query_opt(
            "SELECT version,digest FROM cmd1_record
             WHERE domain=$1 AND namespace=$2 AND key=$3",
            &[&domain, &namespace, &key],
        )?;
        let result = row
            .map(|r| -> Result<(u64, Digest256)> {
                Ok((from_i64(r.get::<_, i64>(0))?, parse_digest(r.get(1))?))
            })
            .transpose()?;
        tx.commit()?;
        Ok(result)
    }

    pub fn read_record_version(
        &mut self,
        domain: &str,
        namespace: &str,
        key: &str,
        version: u64,
    ) -> Result<Option<Digest256>> {
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        let rights = tx.query_one(
            "SELECT rights_allowed FROM cmd1_coordinator WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if !rights.get::<_, bool>(0) {
            return Err(Error::Refused("current local rights revoked"));
        }
        let row = tx.query_opt(
            "SELECT digest FROM cmd1_record_history WHERE domain=$1 AND namespace=$2
             AND key=$3 AND version=$4",
            &[&domain, &namespace, &key, &to_i64(version)?],
        )?;
        let result = row.map(|r| parse_digest(r.get(0))).transpose()?;
        tx.commit()?;
        Ok(result)
    }

    pub fn history_count(&mut self, domain: &str) -> Result<i64> {
        let row = self.client.query_one(
            "SELECT count(*) FROM cmd1_record_history WHERE domain=$1",
            &[&domain],
        )?;
        Ok(row.get(0))
    }

    pub fn receipt_count(&mut self, domain: &str) -> Result<i64> {
        let row = self.client.query_one(
            "SELECT count(*) FROM cmd1_receipt WHERE domain=$1",
            &[&domain],
        )?;
        Ok(row.get(0))
    }

    pub fn outbox_count(&mut self, domain: &str) -> Result<i64> {
        let row = self.client.query_one(
            "SELECT count(*) FROM cmd1_outbox WHERE domain=$1",
            &[&domain],
        )?;
        Ok(row.get(0))
    }
}

fn check_read(tx: &mut Transaction<'_>, domain: &str, read: &PredicateRead) -> Result<()> {
    match read {
        PredicateRead::Exact {
            namespace,
            key,
            expected_version,
            expected_digest,
        } => {
            let row = tx.query_opt(
                "SELECT version,digest FROM cmd1_record
                 WHERE domain=$1 AND namespace=$2 AND key=$3",
                &[&domain, namespace, key],
            )?;
            let observed = row.map(|row| (row.get::<_, i64>(0), row.get::<_, String>(1)));
            let expected = match (expected_version, expected_digest) {
                (Some(version), Some(digest)) => Some((to_i64(*version)?, digest.to_hex())),
                (None, None) => None,
                _ => return Err(Error::InvalidInput("incomplete exact read")),
            };
            if observed != expected {
                return Err(Error::Conflict("exact record changed"));
            }
        }
        PredicateRead::Absent { namespace, key } => {
            let row = tx.query_opt(
                "SELECT 1 FROM cmd1_record WHERE domain=$1 AND namespace=$2 AND key=$3",
                &[&domain, namespace, key],
            )?;
            if row.is_some() {
                return Err(Error::Conflict("absent key appeared"));
            }
        }
        PredicateRead::Generation {
            predicate,
            observed_generation,
        } => {
            let row = tx.query_opt(
                "SELECT generation,definition_version,complete FROM cmd1_predicate
                 WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5",
                &[
                    &domain,
                    &predicate.kind.as_str(),
                    &predicate.owner,
                    &predicate.scope,
                    &predicate.token,
                ],
            )?;
            let Some(row) = row else {
                return Err(Error::Refused("predicate index missing"));
            };
            let complete: bool = row.get(2);
            let version: String = row.get(1);
            if !complete || version != predicate.definition_version {
                return Err(Error::Refused("predicate coverage incomplete"));
            }
            if from_i64(row.get::<_, i64>(0))? != *observed_generation {
                return Err(Error::Conflict("predicate generation changed"));
            }
        }
    }
    Ok(())
}

fn attestation_digest(candidate: &Candidate, seq: u64, prior_head: u64) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    update_part(&mut hasher, &seq.to_be_bytes());
    update_part(&mut hasher, &prior_head.to_be_bytes());
    for digest in [
        candidate.attestation.prepare_base_revision,
        candidate.attestation.prepare_delta_digest,
        candidate.attestation.trace_digest,
        candidate.attestation.checked_predicates_digest,
        candidate.attestation.checked_rule_versions_digest,
        candidate.attestation.owner_fences_digest,
        candidate.attestation.schema_backend_digest,
    ] {
        update_part(&mut hasher, digest.as_bytes());
    }
    update_part(
        &mut hasher,
        candidate.attestation.prepare_overlay_id.as_bytes(),
    );
    update_part(
        &mut hasher,
        candidate.attestation.schema_profile_id.as_bytes(),
    );
    hasher.finalize()
}

fn digest_log_rows(rows: &[postgres::Row], expected_head: u64) -> Result<Digest256> {
    if rows.len() as u64 != expected_head {
        return Err(Error::Corrupt("log prefix incomplete"));
    }
    let mut hasher = Digest256Hasher::new();
    for (index, row) in rows.iter().enumerate() {
        let seq = from_i64(row.get::<_, i64>(0))?;
        if seq != index as u64 + 1 {
            return Err(Error::Corrupt("log prefix gap"));
        }
        let event_kind: String = row.get(1);
        let command_id: String = row.get(2);
        let digest: String = row.get(3);
        let members: Vec<String> = row.get(4);
        update_part(&mut hasher, &seq.to_be_bytes());
        update_part(&mut hasher, event_kind.as_bytes());
        update_part(&mut hasher, command_id.as_bytes());
        update_part(&mut hasher, digest.as_bytes());
        for member in members {
            update_part(&mut hasher, member.as_bytes());
        }
    }
    Ok(hasher.finalize())
}

fn member_commitment(
    namespace: &str,
    key: &str,
    version: u64,
    digest: Digest256,
    byte_length: u64,
) -> String {
    let mut hasher = Digest256Hasher::new();
    for part in [
        b"cmd1-member-v1".as_slice(),
        namespace.as_bytes(),
        key.as_bytes(),
        &version.to_be_bytes(),
        digest.as_bytes(),
        &byte_length.to_be_bytes(),
    ] {
        update_part(&mut hasher, part);
    }
    hasher.finalize().to_hex()
}

fn verify_history_members(
    tx: &mut Transaction<'_>,
    domain: &str,
    through_seq: u64,
    log_rows: &[postgres::Row],
) -> Result<()> {
    let history = tx.query(
        "SELECT commit_seq,namespace,key,version,digest,byte_length
         FROM cmd1_record_history WHERE domain=$1 AND commit_seq <= $2",
        &[&domain, &to_i64(through_seq)?],
    )?;
    let mut actual: HashMap<i64, Vec<String>> = HashMap::new();
    for row in history {
        let seq: i64 = row.get(0);
        let namespace: String = row.get(1);
        let key: String = row.get(2);
        let version = from_i64(row.get(3))?;
        let digest: String = row.get(4);
        let digest = Digest256::from_hex(&digest)
            .map_err(|_| Error::Corrupt("invalid history member digest"))?;
        let byte_length = from_i64(row.get(5))?;
        actual.entry(seq).or_default().push(member_commitment(
            &namespace,
            &key,
            version,
            digest,
            byte_length,
        ));
    }
    for row in log_rows {
        let seq: i64 = row.get(0);
        let kind: String = row.get(1);
        let mut expected: Vec<String> = row.get(4);
        if kind != "command" && !expected.is_empty() {
            return Err(Error::Corrupt("authority event has source members"));
        }
        let mut observed = actual.remove(&seq).unwrap_or_default();
        expected.sort_unstable();
        observed.sort_unstable();
        if observed != expected {
            return Err(Error::Corrupt("committed member history differs from log"));
        }
    }
    if !actual.is_empty() {
        return Err(Error::Corrupt("history has no committed event"));
    }
    Ok(())
}

fn update_part(hasher: &mut Digest256Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn parse_digest(value: String) -> Result<Digest256> {
    Digest256::from_hex(&value).map_err(|_| Error::Corrupt("invalid digest in receipt"))
}

fn zero_digest() -> String {
    Digest256::of_bytes(b"").to_hex()
}

fn to_i64(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| Error::InvalidInput("integer exceeds database range"))
}

fn from_i64(value: i64) -> Result<u64> {
    u64::try_from(value).map_err(|_| Error::Corrupt("negative database sequence/version"))
}
