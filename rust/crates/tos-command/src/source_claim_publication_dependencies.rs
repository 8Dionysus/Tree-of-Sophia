//! Existing source_dependency_* insertion/staging/finalization for Claim addition.
//! Complete addressed checksums bind stored declarations, not source completeness.
use super::source_claim_publication_bytes as bytes;
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use tos_compiler::{Error, Result};
use tos_foundation::Digest256;

const MAX_ORDER: u64 = 9_007_199_254_740_991;
const CHECKSUM: &str = "tos_source_dependency_count_xor_sum_sha256_v1";
const DDL: [&str; 6] = [
    "CREATE TABLE source_dependency_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),json TEXT NOT NULL,sha256 TEXT NOT NULL)",
    "CREATE TABLE source_dependency_claims(claim_id TEXT PRIMARY KEY,declaration TEXT NOT NULL,digest TEXT NOT NULL) WITHOUT ROWID",
    "CREATE TABLE source_dependency_refs(kind TEXT NOT NULL,ref_key TEXT NOT NULL,claim_id TEXT NOT NULL,declaration_digest TEXT NOT NULL,PRIMARY KEY(kind,ref_key,claim_id)) WITHOUT ROWID",
    "CREATE INDEX source_dependency_claim_refs ON source_dependency_refs(claim_id,kind,ref_key)",
    "CREATE TABLE source_dependency_heads(kind TEXT NOT NULL,ref_key TEXT NOT NULL,n INTEGER NOT NULL CHECK(n>0),xor_sha256 TEXT NOT NULL,sum_sha256 TEXT NOT NULL,seal TEXT NOT NULL,PRIMARY KEY(kind,ref_key)) WITHOUT ROWID",
    "CREATE TABLE source_dependency_pending(claim_id TEXT PRIMARY KEY,digest TEXT) WITHOUT ROWID",
];
pub(super) struct Declaration {
    pub value: Value,
    pub raw: Vec<u8>,
    pub digest: String,
    pub id: String,
}
impl Declaration {
    pub fn parse(raw: &[u8], max_row: usize, max_dependencies: usize) -> Result<Self> {
        let value = bytes::parse(raw, max_row)?;
        if value.as_object().map(|o| o.len()) != Some(6)
            || value["schema"] != "tos_source_claim_dependencies_v1"
            || bytes::canonical(&value, max_row)? != raw
        {
            return Err(Error::Invalid("Claim dependency declaration framing"));
        }
        let id = bytes::text(&value, "claim_id")?.to_owned();
        let input = bytes::text(&value, "input_sha256")?;
        bytes::sha(input)?;
        let entry = &value["source_entry"];
        let source = bytes::text(entry, "source_claim_file_ref")?;
        let line = bytes::number(entry, "source_claim_line")?;
        if entry["claim_id"] != id
            || entry["claim_sha256"] != input
            || line == 0
            || !source.starts_with("ToS/")
            || source
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
            || bytes::row_digest(entry, max_row)? != bytes::text(&value, "source_entry_sha256")?
        {
            return Err(Error::Invalid(
                "Claim declaration source identity/locator/digest",
            ));
        }
        let dependencies = value["dependencies"]
            .as_array()
            .filter(|a| a.len() <= max_dependencies)
            .ok_or(Error::Budget("Claim declaration dependencies"))?;
        let mut previous: Option<(String, String)> = None;
        let mut own_claim = false;
        let mut own_source = false;
        for dependency in dependencies {
            if dependency.as_object().map(|o| o.len()) != Some(4) {
                return Err(Error::Invalid("Claim dependency shape"));
            }
            let kind = bytes::text(dependency, "kind")?;
            address(kind, &dependency["ref"])?;
            let reference = dependency["ref"].as_str().unwrap_or("");
            let key = (kind.to_owned(), reference.to_owned());
            if previous.as_ref().is_some_and(|p| p >= &key) {
                return Err(Error::Invalid("Claim dependency order"));
            }
            previous = Some(key);
            for field in ["field_paths", "reasons"] {
                let rows = dependency[field]
                    .as_array()
                    .filter(|a| !a.is_empty())
                    .ok_or(Error::Invalid("Claim dependency reasons/paths"))?;
                let mut prev: Option<&str> = None;
                for row in rows {
                    let value = row
                        .as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 4096)
                        .ok_or(Error::Invalid("Claim dependency reason identifier"))?;
                    if prev.is_some_and(|p| p >= value)
                        || field == "field_paths" && !value.starts_with('/')
                    {
                        return Err(Error::Invalid("Claim dependency sorted JSON pointers"));
                    }
                    prev = Some(value);
                }
            }
            own_claim |= kind == "claim" && reference == id;
            own_source |= kind == "path" && reference == source;
        }
        if !own_claim || !own_source {
            return Err(Error::Invalid("Claim declaration missing own source slot"));
        }
        Ok(Self {
            value,
            raw: raw.to_vec(),
            digest: bytes::digest(raw),
            id,
        })
    }
}
fn address(kind: &str, reference: &Value) -> Result<String> {
    if ![
        "identity",
        "provenance_event",
        "claim",
        "path",
        "unresolved",
    ]
    .contains(&kind)
        || reference.is_null() && kind != "unresolved"
        || !reference.is_null()
            && reference
                .as_str()
                .is_none_or(|s| s.is_empty() || s.len() > 4096)
    {
        return Err(Error::Invalid("Claim source dependency address"));
    }
    String::from_utf8(bytes::canonical(reference, 24584)?)
        .map_err(|_| Error::Invalid("Claim address UTF8"))
}
#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub max_row: usize,
    pub max_state: usize,
    pub max_dependencies: usize,
    pub max_claims: usize,
    pub max_change_bytes: usize,
    pub max_queries: u64,
    pub max_rows: u64,
    pub max_read_bytes: usize,
    pub max_writes: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_row: 1048576,
            max_state: 8388608,
            max_dependencies: 4096,
            max_claims: 512,
            max_change_bytes: 16777216,
            max_queries: 100000,
            max_rows: 100000,
            max_read_bytes: 33554432,
            max_writes: 100000,
        }
    }
}
pub(super) struct Dependencies<'a, 'tx> {
    tx: &'a Transaction<'tx>,
    limits: Limits,
    queries: u64,
    rows: u64,
    read: usize,
    start: u64,
}
impl<'a, 'tx> Dependencies<'a, 'tx> {
    pub fn new(tx: &'a Transaction<'tx>, limits: Limits) -> Result<Self> {
        if tx.is_autocommit() {
            return Err(Error::Invalid("Claim dependency caller transaction"));
        }
        Ok(Self {
            tx,
            limits,
            queries: 0,
            rows: 0,
            read: 0,
            start: tx.total_changes(),
        })
    }
    fn query(&mut self) -> Result<()> {
        self.queries = self
            .queries
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_queries)
            .ok_or(Error::Budget("Claim dependency queries"))?;
        Ok(())
    }
    fn read(&mut self, raw: &str) -> Result<()> {
        self.rows = self
            .rows
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_rows)
            .ok_or(Error::Budget("Claim dependency rows"))?;
        self.read = self
            .read
            .checked_add(raw.len())
            .filter(|n| *n <= self.limits.max_read_bytes)
            .ok_or(Error::Budget("Claim dependency read bytes"))?;
        Ok(())
    }
    fn mutations(&self) -> Result<u64> {
        self.tx
            .total_changes()
            .checked_sub(self.start)
            .filter(|n| *n <= self.limits.max_writes)
            .ok_or(Error::Budget("Claim dependency mutations"))
    }
    fn schema(&mut self) -> Result<()> {
        for sql in DDL {
            self.query()?;
            let name = sql
                .split_whitespace()
                .nth(2)
                .unwrap()
                .split('(')
                .next()
                .unwrap();
            let actual:Option<String>=self.tx.query_row("SELECT CASE WHEN length(CAST(sql AS BLOB))<=4096 THEN sql END FROM sqlite_master WHERE name=?",
                [name],|r|r.get(0)).optional()?;
            if actual.as_deref() != Some(sql) {
                return Err(Error::Invalid("Claim dependency physical schema"));
            }
            self.read(sql)?;
        }
        self.query()?;
        let trigger:Option<i64>=self.tx.query_row("SELECT 1 FROM sqlite_master WHERE type='trigger' AND tbl_name IN ('source_dependency_state','source_dependency_claims','source_dependency_refs','source_dependency_heads','source_dependency_pending') LIMIT 1",[],|r|r.get(0)).optional()?;
        if trigger.is_some() {
            return Err(Error::Invalid("Claim dependency triggers"));
        }
        Ok(())
    }
    pub fn state(
        &mut self,
        binding: &Value,
        source_digest: &str,
        profile: &str,
        implementation: &str,
    ) -> Result<Value> {
        self.schema()?;
        self.query()?;
        let (raw,digest):(Option<String>,Option<String>)=self.tx.query_row(
            "SELECT CASE WHEN length(CAST(json AS BLOB))<=? THEN json END,CASE WHEN length(CAST(sha256 AS BLOB))=64 THEN sha256 END FROM source_dependency_state WHERE singleton=1",
            [self.limits.max_state],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let raw = raw.ok_or(Error::Budget("Claim dependency state bytes"))?;
        self.read(&raw)?;
        if digest.as_deref() != Some(bytes::digest(raw.as_bytes()).as_str()) {
            return Err(Error::Invalid("Claim dependency state checksum"));
        }
        let value = bytes::parse(raw.as_bytes(), self.limits.max_state)?;
        if bytes::canonical(&value, self.limits.max_state)? != raw.as_bytes()
            || value.as_object().map(|o| o.len()) != Some(10)
            || value["schema"] != "tos_prepared_source_dependencies_v1"
            || value["checksum"] != CHECKSUM
            || value["declaration_profile_sha256"] != profile
            || value["implementation_sha256"] != implementation
            || value["binding"] != *binding
            || value["source_inputs_sha256"] != source_digest
            || value["pending"] != false
            || bytes::number(&value, "pending_count")? != 0
        {
            return Err(Error::Invalid("Claim dependency current state/profile"));
        }
        bytes::number(&value, "claim_count")?;
        bytes::number(&value, "dependency_count")?;
        self.query()?;
        if self
            .tx
            .query_row("SELECT 1 FROM source_dependency_pending LIMIT 1", [], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?
            .is_some()
        {
            return Err(Error::Invalid("Claim dependency orphan pending"));
        }
        Ok(value)
    }
    fn claim(&mut self, id: &str) -> Result<Option<Declaration>> {
        self.query()?;
        let row:Option<(Option<String>,Option<String>)>=self.tx.query_row(
            "SELECT CASE WHEN length(CAST(declaration AS BLOB))<=? THEN declaration END,CASE WHEN length(CAST(digest AS BLOB))=64 THEN digest END FROM source_dependency_claims WHERE claim_id=?",
            params![self.limits.max_row,id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let Some((raw, digest)) = row else {
            self.query()?;
            if self.tx.query_row("SELECT 1 FROM source_dependency_refs INDEXED BY source_dependency_claim_refs WHERE claim_id=? LIMIT 1",[id],|r|r.get::<_,i64>(0)).optional()?.is_some(){
                return Err(Error::Invalid("Claim dependency orphan reverse declaration"));
            }
            return Ok(None);
        };
        let raw = raw.ok_or(Error::Budget("Claim declaration stored bytes"))?;
        self.read(&raw)?;
        let declaration = Declaration::parse(
            raw.as_bytes(),
            self.limits.max_row,
            self.limits.max_dependencies,
        )?;
        if declaration.id != id || digest.as_deref() != Some(&declaration.digest) {
            return Err(Error::Invalid("Claim declaration digest/id"));
        }
        let expected: BTreeSet<_> = declaration.value["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                Ok((
                    bytes::text(d, "kind")?.to_owned(),
                    address(bytes::text(d, "kind")?, &d["ref"])?,
                    declaration.digest.clone(),
                ))
            })
            .collect::<Result<_>>()?;
        self.query()?;
        let mut stmt=self.tx.prepare("SELECT CASE WHEN length(CAST(kind AS BLOB))<=32 THEN kind END,CASE WHEN length(CAST(ref_key AS BLOB))<=24584 THEN ref_key END,CASE WHEN length(CAST(declaration_digest AS BLOB))=64 THEN declaration_digest END FROM source_dependency_refs INDEXED BY source_dependency_claim_refs WHERE claim_id=? ORDER BY kind,ref_key LIMIT ?")?;
        let mut cursor = stmt.query(params![id, self.limits.max_dependencies + 1])?;
        let mut found = BTreeSet::new();
        while let Some(row) = cursor.next()? {
            let kind: String = row.get(0)?;
            let reference: String = row.get(1)?;
            let digest: String = row.get(2)?;
            self.read(&kind)?;
            self.read(&reference)?;
            self.read(&digest)?;
            found.insert((kind, reference, digest));
        }
        if found != expected {
            return Err(Error::Invalid("Claim complete reverse declarations"));
        }
        Ok(Some(declaration))
    }
    fn head(&mut self, kind: &str, reference: &str) -> Result<(u64, [u8; 32], [u8; 32])> {
        self.query()?;
        let row:Option<(u64,String,String,String)>=self.tx.query_row(
            "SELECT n,CASE WHEN length(CAST(xor_sha256 AS BLOB))=64 THEN xor_sha256 END,CASE WHEN length(CAST(sum_sha256 AS BLOB))=64 THEN sum_sha256 END,CASE WHEN length(CAST(seal AS BLOB))=64 THEN seal END FROM source_dependency_heads WHERE kind=? AND ref_key=?",
            params![kind,reference],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let Some((count, xor, sum, seal)) = row else {
            return Ok((0, [0; 32], [0; 32]));
        };
        self.read(&xor)?;
        self.read(&sum)?;
        self.read(&seal)?;
        if count == 0
            || count > MAX_ORDER
            || seal
                != bytes::row_digest(&json!([CHECKSUM, kind, reference, count, xor, sum]), 33792)?
        {
            return Err(Error::Invalid("Claim dependency head count/seal"));
        }
        Ok((count, decode(&xor)?, decode(&sum)?))
    }
    pub fn lookup(&mut self, kind: &str, reference: &Value) -> Result<Vec<String>> {
        let key = address(kind, reference)?;
        let expected = self.head(kind, &key)?;
        if expected.0 > self.limits.max_claims as u64 {
            return Err(Error::Budget("Claim dependency fanout"));
        }
        self.query()?;
        let tx = self.tx;
        let mut statement=tx.prepare("SELECT CASE WHEN length(CAST(claim_id AS BLOB))<=4096 THEN claim_id END,CASE WHEN length(CAST(declaration_digest AS BLOB))=64 THEN declaration_digest END FROM source_dependency_refs WHERE kind=? AND ref_key=? ORDER BY claim_id LIMIT ?")?;
        let mut cursor = statement.query(params![kind, key, self.limits.max_claims + 1])?;
        let mut selected = Vec::new();
        let mut xor = [0; 32];
        let mut sum = [0; 32];
        while let Some(row) = cursor.next()? {
            if selected.len() >= self.limits.max_claims || selected.len() as u64 >= expected.0 {
                return Err(Error::Invalid(
                    "Claim dependency complete fanout exceeds aggregate",
                ));
            }
            let id: String = row.get(0)?;
            let digest: String = row.get(1)?;
            self.read(&id)?;
            self.read(&digest)?;
            let declaration = self
                .claim(&id)?
                .ok_or(Error::Invalid("Claim fanout missing declaration"))?;
            if declaration.digest != digest {
                return Err(Error::Invalid("Claim fanout declaration digest"));
            }
            aggregate(&mut xor, &mut sum, &contribution(kind, &key, &id, &digest)?);
            selected.push(id);
        }
        if (selected.len() as u64, xor, sum) != expected {
            return Err(Error::Invalid("Claim complete fanout checksum"));
        }
        Ok(selected)
    }
    pub fn require_absent(&mut self, id: &str) -> Result<()> {
        if self.claim(id)?.is_some() {
            return Err(Error::Invalid(
                "Claim declaration already belongs to predecessor",
            ));
        }
        Ok(())
    }
    fn put_state(&mut self, value: &Value, before: &str) -> Result<()> {
        let raw = bytes::canonical(value, self.limits.max_state)?;
        self.query()?;
        if self.tx.execute(
            "UPDATE source_dependency_state SET json=?,sha256=? WHERE singleton=1 AND sha256=?",
            params![
                std::str::from_utf8(&raw).map_err(|_| Error::Invalid("Claim state UTF8"))?,
                bytes::digest(&raw),
                before
            ],
        )? != 1
        {
            return Err(Error::Invalid("Claim dependency state CAS"));
        }
        self.mutations()?;
        Ok(())
    }
    pub fn rebind_reviewed(
        &mut self,
        mut state: Value,
        binding: &Value,
        source_digest: &str,
        profile: &str,
        implementation: &str,
    ) -> Result<()> {
        bytes::sha(profile)?;
        bytes::sha(implementation)?;
        bytes::sha(source_digest)?;
        let before = bytes::row_digest(&state, self.limits.max_state)?;
        if state["pending"] != false || bytes::number(&state, "pending_count")? != 0 {
            return Err(Error::Invalid(
                "Claim profile transition pending declarations",
            ));
        }
        state["binding"] = binding.clone();
        state["source_inputs_sha256"] = json!(source_digest);
        state["declaration_profile_sha256"] = json!(profile);
        state["implementation_sha256"] = json!(implementation);
        self.put_state(&state, &before)?;
        if self.state(binding, source_digest, profile, implementation)? != state {
            return Err(Error::Invalid("Claim profile transition readback"));
        }
        Ok(())
    }
    pub fn stage(
        &mut self,
        mut state: Value,
        declarations: &[Declaration],
        after_digest: &str,
        after_revision: &str,
    ) -> Result<usize> {
        bytes::sha(after_digest)?;
        bytes::sha(after_revision)?;
        let old_digest = bytes::row_digest(&state, self.limits.max_state)?;
        if declarations.len() > self.limits.max_claims {
            return Err(Error::Budget("Claim dependency changed cohort"));
        }
        let next_epoch = bytes::number(&state["binding"], "publication_epoch")?
            .checked_add(1)
            .filter(|n| *n <= MAX_ORDER)
            .ok_or(Error::Budget("Claim dependency epoch"))?;
        let mut seen = BTreeSet::new();
        let mut selected_bytes = 0usize;
        // Capture/check every declaration before the first insertion. All
        // caller-provided data here already belongs to the source observer.
        for declaration in declarations {
            if !seen.insert(&declaration.id) {
                return Err(Error::Invalid("Claim dependency repeated target"));
            }
            self.require_absent(&declaration.id)?;
            Declaration::parse(
                &declaration.raw,
                self.limits.max_row,
                self.limits.max_dependencies,
            )?;
            selected_bytes = selected_bytes
                .checked_add(declaration.raw.len())
                .filter(|n| *n <= self.limits.max_change_bytes)
                .ok_or(Error::Budget("Claim dependency staged bytes"))?;
        }
        for declaration in declarations {
            self.query()?;
            self.tx.execute(
                "INSERT INTO source_dependency_claims VALUES (?,?,?)",
                params![
                    declaration.id,
                    std::str::from_utf8(&declaration.raw)
                        .map_err(|_| Error::Invalid("Claim declaration UTF8"))?,
                    declaration.digest
                ],
            )?;
            self.mutations()?;
            for dependency in declaration.value["dependencies"].as_array().unwrap() {
                let kind = bytes::text(dependency, "kind")?;
                let reference = address(kind, &dependency["ref"])?;
                self.query()?;
                self.tx.execute(
                    "INSERT INTO source_dependency_refs VALUES (?,?,?,?)",
                    params![kind, reference, declaration.id, declaration.digest],
                )?;
                let (count, mut xor, mut sum) = self.head(kind, &reference)?;
                if count == 0 {
                    self.query()?;
                    let tx = self.tx;
                    let mut stmt=tx.prepare("SELECT claim_id FROM source_dependency_refs WHERE kind=? AND ref_key=? ORDER BY claim_id LIMIT 2")?;
                    let found = stmt
                        .query_map(params![kind, reference], |r| r.get::<_, String>(0))?
                        .collect::<rusqlite::Result<Vec<_>>>()?;
                    if found != [declaration.id.clone()] {
                        return Err(Error::Invalid("Claim missing head for preexisting address"));
                    }
                }
                let count = count
                    .checked_add(1)
                    .filter(|n| *n <= MAX_ORDER)
                    .ok_or(Error::Budget("Claim dependency head count"))?;
                aggregate(
                    &mut xor,
                    &mut sum,
                    &contribution(kind, &reference, &declaration.id, &declaration.digest)?,
                );
                let xor = hex(&xor);
                let sum = hex(&sum);
                let seal =
                    bytes::row_digest(&json!([CHECKSUM, kind, reference, count, xor, sum]), 33792)?;
                self.query()?;
                self.tx.execute("INSERT INTO source_dependency_heads VALUES (?,?,?,?,?,?) ON CONFLICT(kind,ref_key) DO UPDATE SET n=excluded.n,xor_sha256=excluded.xor_sha256,sum_sha256=excluded.sum_sha256,seal=excluded.seal",
                    params![kind,reference,count,xor,sum,seal])?;
                self.mutations()?;
            }
            self.query()?;
            self.tx.execute(
                "INSERT INTO source_dependency_pending VALUES (?,?)",
                params![declaration.id, declaration.digest],
            )?;
            self.mutations()?;
            let claims = bytes::number(&state, "claim_count")?
                .checked_add(1)
                .filter(|n| *n <= MAX_ORDER)
                .ok_or(Error::Budget("Claim dependency state claim count"))?;
            let dependencies = bytes::number(&state, "dependency_count")?
                .checked_add(declaration.value["dependencies"].as_array().unwrap().len() as u64)
                .filter(|n| *n <= MAX_ORDER)
                .ok_or(Error::Budget("Claim dependency state entry count"))?;
            state["claim_count"] = json!(claims);
            state["dependency_count"] = json!(dependencies);
        }
        state["pending"] = json!(true);
        state["pending_count"] = json!(declarations.len());
        state["next_source_inputs_sha256"] = json!(after_digest);
        state["next_source_revision"] = json!(after_revision);
        state["next_epoch"] = json!(next_epoch);
        self.put_state(&state, &old_digest)?;
        Ok(declarations.len() + 1)
    }
    pub fn finalize(
        &mut self,
        new_binding: &Value,
        source_digest: &str,
        profile: &str,
        implementation: &str,
    ) -> Result<()> {
        self.schema()?;
        self.query()?;
        let (raw,checksum):(Option<String>,String)=self.tx.query_row("SELECT CASE WHEN length(CAST(json AS BLOB))<=? THEN json END,sha256 FROM source_dependency_state WHERE singleton=1",
            [self.limits.max_state],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let raw = raw.ok_or(Error::Budget("Claim pending state size"))?;
        self.read(&raw)?;
        if bytes::digest(raw.as_bytes()) != checksum {
            return Err(Error::Invalid("Claim pending state checksum"));
        }
        let mut state = bytes::parse(raw.as_bytes(), self.limits.max_state)?;
        if state.as_object().map(|o| o.len()) != Some(13)
            || state["schema"] != "tos_prepared_source_dependencies_v1"
            || state["checksum"] != CHECKSUM
            || state["declaration_profile_sha256"] != profile
            || state["implementation_sha256"] != implementation
            || state["pending"] != true
            || state["next_source_inputs_sha256"] != source_digest
            || state["next_source_revision"] != new_binding["source_revision"]
            || state["next_epoch"] != new_binding["publication_epoch"]
            || state["binding"]["normalization_binding"] != new_binding["normalization_binding"]
        {
            return Err(Error::Invalid("Claim pending final binding/profile"));
        }
        let count = bytes::number(&state, "pending_count")? as usize;
        if count > self.limits.max_claims {
            return Err(Error::Budget("Claim dependency pending cohort"));
        }
        self.query()?;
        let tx = self.tx;
        let mut stmt=tx.prepare("SELECT CASE WHEN length(CAST(claim_id AS BLOB))<=4096 THEN claim_id END,CASE WHEN length(CAST(digest AS BLOB))=64 THEN digest END FROM source_dependency_pending ORDER BY claim_id LIMIT ?")?;
        let pending = stmt
            .query_map([self.limits.max_claims + 1], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if pending.len() != count {
            return Err(Error::Invalid("Claim complete pending declarations"));
        }
        drop(stmt);
        for (id, digest) in pending {
            self.read(&id)?;
            self.read(&digest)?;
            if self.claim(&id)?.is_none_or(|d| d.digest != digest) {
                return Err(Error::Invalid("Claim final declaration changed"));
            }
            self.query()?;
            if self.tx.execute(
                "DELETE FROM source_dependency_pending WHERE claim_id=? AND digest=?",
                params![id, digest],
            )? != 1
            {
                return Err(Error::Invalid("Claim pending declaration CAS"));
            }
            self.mutations()?;
        }
        state["binding"] = new_binding.clone();
        state["source_inputs_sha256"] = json!(source_digest);
        state["pending"] = json!(false);
        state["pending_count"] = json!(0);
        let object = state.as_object_mut().unwrap();
        for field in [
            "next_source_inputs_sha256",
            "next_source_revision",
            "next_epoch",
        ] {
            object.remove(field);
        }
        self.put_state(&state, &checksum)
    }
}

// Byte arrays retain the existing modulo-2^256 checksum without adding a
// numeric representation/dependency or accepting a caller aggregate.
fn contribution(kind: &str, reference: &str, id: &str, digest: &str) -> Result<[u8; 32]> {
    let hash = Digest256::of_bytes(&bytes::canonical(
        &json!([CHECKSUM, kind, reference, id, digest]),
        99328,
    )?);
    decode(&hash.to_hex())
}
pub(super) fn decode(value: &str) -> Result<[u8; 32]> {
    bytes::sha(value)?;
    let mut out = [0; 32];
    for (i, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        out[i] = (digit(pair[0]) << 4) | digit(pair[1]);
    }
    Ok(out)
}
fn hex(value: &[u8; 32]) -> String {
    value.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn aggregate(xor: &mut [u8; 32], sum: &mut [u8; 32], value: &[u8; 32]) {
    let mut carry = 0u16;
    for i in (0..32).rev() {
        xor[i] ^= value[i];
        let n = sum[i] as u16 + value[i] as u16 + carry;
        sum[i] = n as u8;
        carry = n >> 8;
    }
}
