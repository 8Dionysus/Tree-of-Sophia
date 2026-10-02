//! Private operation-local Navigation rows. No source or creation authority.
use crate::{Error, Result};
use rusqlite::{OptionalExtension, params};
use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::Digest256;
use tos_source_store::{
    PinnedSqliteAuxRequest, PinnedSqliteAuxScope, PinnedSqliteConnection, PinnedSqliteIoBudget,
    PinnedSqliteSpaceBudget,
};

#[derive(Clone, Copy, Debug)]
pub struct NavigationStorageLimits {
    pub max_rows: u64,
    pub max_logical_bytes: u64,
    pub max_database_bytes: u64,
    pub max_row_bytes: usize,
    pub cache_kib: u32,
    pub max_vm_steps: u64,
    pub max_workspace_bytes: usize,
}

pub(crate) struct NavigationStorage<'a> {
    db: PinnedSqliteConnection,
    scope: PinnedSqliteAuxScope,
    io_budget: PinnedSqliteIoBudget,
    space_budget: PinnedSqliteSpaceBudget,
    limits: NavigationStorageLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    rows: u64,
    bytes: u64,
    failed: bool,
}

impl<'a> NavigationStorage<'a> {
    pub(crate) fn create(
        workspace_dir: File,
        request: PinnedSqliteAuxRequest,
        limits: NavigationStorageLimits,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        if limits.max_rows == 0
            || limits.max_rows == u64::MAX
            || limits.max_logical_bytes == 0
            || limits.max_logical_bytes == u64::MAX
            || limits.max_database_bytes < 4096
            || limits.max_database_bytes == u64::MAX
            || limits.max_row_bytes == 0
            || limits.max_row_bytes > 8 * 1024 * 1024
            || limits.cache_kib == 0
            || limits.cache_kib > 64 * 1024
            || limits.max_vm_steps == 0
            || limits.max_workspace_bytes == 0
        {
            return Err(Error::Budget("navigation storage limits"));
        }
        Self::clock(deadline, cancelled)?;
        if request.deadline != deadline
            || !std::ptr::eq(request.cancelled.as_ref(), cancelled)
            || limits.max_database_bytes > request.limits.main_logical_bytes
        {
            return Err(Error::Invalid("navigation auxiliary original operation"));
        }
        let io_budget = request.io_budget.clone();
        let space_budget = request.space_budget.clone();
        let mut scope = PinnedSqliteAuxScope::new(workspace_dir, request)
            .map_err(|e| Error::Source(e.to_string()))?;
        let db = scope
            .open_connection()
            .map_err(|e| Error::Source(e.to_string()))?;
        crate::sqlite_budget::install_progress_until(
            &db,
            crate::Limits {
                max_rows: limits.max_rows,
                max_row_bytes: limits.max_row_bytes,
                max_output_bytes: limits.max_database_bytes,
                max_work_bytes: limits.max_logical_bytes,
                sqlite_cache_kib: limits.cache_kib,
                max_sql_vm_steps: limits.max_vm_steps,
            },
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            deadline,
        );
        db.pragma_update(None, "cache_size", -(limits.cache_kib as i64))?;
        let page: u64 = db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        if page == 0 || page > limits.max_database_bytes {
            return Err(Error::Budget("navigation storage page"));
        }
        let maximum = limits.max_database_bytes / page;
        db.pragma_update(None, "max_page_count", maximum)?;
        let effective: u64 = db.query_row("PRAGMA max_page_count", [], |r| r.get(0))?;
        if effective > maximum {
            return Err(Error::Invalid("navigation storage page cap"));
        }
        db.execute_batch("CREATE TABLE navigation_rows(category TEXT NOT NULL,key TEXT NOT NULL,body BLOB NOT NULL,sha BLOB NOT NULL,sort_key TEXT,sort_second TEXT,sort_third TEXT,PRIMARY KEY(category,key)) WITHOUT ROWID;CREATE INDEX navigation_order ON navigation_rows(category,sort_key,sort_second,sort_third,key);")?;
        let s = Self {
            db,
            scope,
            io_budget,
            space_budget,
            limits,
            deadline,
            cancelled,
            rows: 0,
            bytes: 0,
            failed: false,
        };
        s.guard()?;
        Ok(s)
    }

    pub(crate) fn close(self) -> Result<()> {
        self.guard()?;
        let Self {
            db,
            scope,
            io_budget,
            space_budget,
            deadline,
            cancelled,
            ..
        } = self;
        db.close().map_err(|(_, error)| Error::Sql(error))?;
        // The quota scope remains held through SQLite close and the final clock check.
        Self::clock(deadline, cancelled)?;
        drop(scope);
        let space = space_budget.snapshot();
        if io_budget.snapshot().failure.is_some()
            || !space.ledger_consistent
            || space.allocation_anomalies != 0
        {
            return Err(Error::Budget("navigation auxiliary cleanup"));
        }
        Self::clock(deadline, cancelled)
    }

    fn clock(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
        if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
            Err(Error::Budget("navigation storage cancelled/deadline"))
        } else {
            Ok(())
        }
    }
    pub(crate) fn guard(&self) -> Result<()> {
        if self.failed {
            return Err(Error::Invalid("navigation storage poisoned"));
        }
        Self::clock(self.deadline, self.cancelled)?;
        let space = self.space_budget.snapshot();
        if self.io_budget.snapshot().failure.is_some()
            || !space.ledger_consistent
            || space.allocation_anomalies != 0
        {
            return Err(Error::Budget("navigation shared auxiliary budget"));
        }
        Self::clock(self.deadline, self.cancelled)
    }
    pub(crate) fn workspace_available(&self, raw_len: usize) -> Result<usize> {
        let baseline = (self.limits.cache_kib as usize)
            .checked_mul(1024)
            .and_then(|n| n.checked_add(64 * 1024))
            .and_then(|n| n.checked_add(raw_len))
            .ok_or(Error::Budget("navigation row workspace"))?;
        self.limits
            .max_workspace_bytes
            .checked_sub(baseline)
            .filter(|n| *n > 0)
            .ok_or(Error::Budget("navigation row workspace"))
    }
    pub(crate) fn get(&self, category: &str, key: &str) -> Result<Option<Vec<u8>>> {
        self.guard()?;
        let len: Option<u64> = self
            .db
            .query_row(
                "SELECT length(body) FROM navigation_rows WHERE category=?1 AND key=?2",
                params![category, key],
                |r| r.get(0),
            )
            .optional()?;
        let Some(len) = len else {
            self.guard()?;
            return Ok(None);
        };
        let len =
            usize::try_from(len).map_err(|_| Error::Budget("navigation retained row length"))?;
        if len > self.limits.max_row_bytes {
            return Err(Error::Budget("navigation retained row"));
        }
        self.workspace_available(
            len.checked_add(key.len())
                .ok_or(Error::Budget("navigation row workspace"))?,
        )?;
        let row = self.db.query_row("SELECT CASE WHEN length(body)<=?3 THEN body ELSE NULL END,sha FROM navigation_rows WHERE category=?1 AND key=?2", params![category,key,len as i64], |r| Ok((r.get::<_,Option<Vec<u8>>>(0)?,r.get::<_,Vec<u8>>(1)?))).optional()?;
        self.guard()?;
        row.map(|(body, sha)| {
            let body = body.ok_or(Error::Budget("navigation retained row"))?;
            if sha.as_slice() != Digest256::of_bytes(&body).as_bytes() {
                return Err(Error::Invalid("navigation retained row digest"));
            }
            Ok(body)
        })
        .transpose()
    }
    /// Replacements remain charged: no transient or retained credit is minted.
    pub(crate) fn put(&mut self, category: &str, key: &str, body: &[u8]) -> Result<()> {
        self.put_ordered(category, key, Some(key), body)
    }
    pub(crate) fn put_ordered(
        &mut self,
        category: &str,
        key: &str,
        order: Option<&str>,
        body: &[u8],
    ) -> Result<()> {
        self.put_sorted(category, key, [order, None, None], body)
    }
    fn put_sorted(
        &mut self,
        category: &str,
        key: &str,
        orders: [Option<&str>; 3],
        body: &[u8],
    ) -> Result<()> {
        self.guard()?;
        if key.len() > self.limits.max_row_bytes
            || orders
                .iter()
                .flatten()
                .any(|v| v.len() > self.limits.max_row_bytes)
            || body.len() > self.limits.max_row_bytes
        {
            return Err(Error::Budget("navigation stored row"));
        }
        let exists: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM navigation_rows WHERE category=?1 AND key=?2)",
            params![category, key],
            |r| r.get(0),
        )?;
        let bytes = self
            .bytes
            .checked_add(body.len() as u64)
            .and_then(|n| {
                n.checked_add(
                    key.len() as u64
                        + orders.iter().flatten().map(|v| v.len()).sum::<usize>() as u64
                        + 64,
                )
            })
            .filter(|n| *n <= self.limits.max_logical_bytes)
            .ok_or(Error::Budget("navigation storage logical bytes"))?;
        let rows = self
            .rows
            .checked_add(u64::from(!exists))
            .filter(|n| *n <= self.limits.max_rows)
            .ok_or(Error::Budget("navigation storage rows"))?;
        self.bytes = bytes;
        self.rows = rows;
        let result = (|| {
            self.db.execute("INSERT INTO navigation_rows(category,key,body,sha,sort_key,sort_second,sort_third) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(category,key) DO UPDATE SET body=excluded.body,sha=excluded.sha,sort_key=excluded.sort_key,sort_second=excluded.sort_second,sort_third=excluded.sort_third", params![category,key,body,Digest256::of_bytes(body).as_bytes().as_slice(),orders[0],orders[1],orders[2]])?;
            self.guard()
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub(crate) fn visit_rows(
        &self,
        category: &str,
        mut visit: impl FnMut(&str, &[u8]) -> Result<()>,
    ) -> Result<u64> {
        self.guard()?;
        self.workspace_available(self.limits.max_row_bytes)?;
        let mut query = self.db.prepare("SELECT CASE WHEN length(CAST(key AS BLOB))<=?3 THEN key ELSE NULL END,CASE WHEN length(body)<=?2 THEN body ELSE NULL END,sha FROM navigation_rows WHERE category=?1 ORDER BY sort_key,sort_second,sort_third,key")?;
        let mut rows = query.query(params![
            category,
            self.limits.max_row_bytes as i64,
            self.limits.max_row_bytes as i64
        ])?;
        let mut count = 0u64;
        while let Some(row) = rows.next()? {
            self.guard()?;
            let key: Option<String> = row.get(0)?;
            let key = key.ok_or(Error::Budget("navigation stored key"))?;
            let body = row
                .get_ref(1)?
                .as_blob()
                .map_err(|_| Error::Budget("navigation retained row"))?;
            let sha = row
                .get_ref(2)?
                .as_blob()
                .map_err(|_| Error::Invalid("navigation retained row digest"))?;
            if sha != Digest256::of_bytes(body).as_bytes() {
                return Err(Error::Invalid("navigation retained row digest"));
            }
            visit(&key, body)?;
            self.guard()?;
            count = count
                .checked_add(1)
                .filter(|n| *n <= self.limits.max_rows)
                .ok_or(Error::Budget("navigation sink rows"))?;
        }
        self.guard()?;
        Ok(count)
    }

    pub(crate) fn next(&self, category: &str, after: Option<&str>) -> Result<Option<String>> {
        self.guard()?;
        let row = match after {
            Some(after) => self.db.query_row("SELECT CASE WHEN length(CAST(key AS BLOB))<=?3 THEN key ELSE NULL END FROM navigation_rows WHERE category=?1 AND key>?2 ORDER BY key LIMIT 1", params![category,after,self.limits.max_row_bytes as i64], |r|r.get::<_,Option<String>>(0)).optional()?,
            None => self.db.query_row("SELECT CASE WHEN length(CAST(key AS BLOB))<=?3 THEN key ELSE NULL END FROM navigation_rows WHERE category=?1 ORDER BY key LIMIT 1", params![category,rusqlite::types::Null,self.limits.max_row_bytes as i64], |r|r.get::<_,Option<String>>(0)).optional()?,
        };
        self.guard()?;
        row.map(|r| r.ok_or(Error::Budget("navigation stored key")))
            .transpose()
    }
}

pub(crate) type SharedNavigationStorage<'a> =
    std::rc::Rc<std::cell::RefCell<NavigationStorage<'a>>>;

/// Only the Navigation renderer's private Value maps use this storage split.
pub(crate) enum NavigationMap<'a> {
    Resident(std::collections::BTreeMap<String, serde_json::Value>),
    Disk {
        storage: SharedNavigationStorage<'a>,
        category: &'static str,
        count: usize,
    },
}
impl<'a> NavigationMap<'a> {
    pub(crate) fn new(
        storage: Option<&SharedNavigationStorage<'a>>,
        category: &'static str,
    ) -> Self {
        match storage {
            None => Self::Resident(std::collections::BTreeMap::new()),
            Some(s) => Self::Disk {
                storage: s.clone(),
                category,
                count: 0,
            },
        }
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Resident(m) => m.len(),
            Self::Disk { count, .. } => *count,
        }
    }
    pub(crate) fn get(&self, key: &str) -> Result<Option<std::borrow::Cow<'_, serde_json::Value>>> {
        match self {
            Self::Resident(m) => Ok(m.get(key).map(std::borrow::Cow::Borrowed)),
            Self::Disk {
                storage, category, ..
            } => {
                let s = storage.borrow();
                s.get(category, key)?
                    .map(|raw| {
                        let available=s.workspace_available(raw.len())?/2;
                        let (row, _)=crate::knowledge_normalization::SourceRow::parse_raw_input_with_state_budget(
                            &raw,s.limits.max_row_bytes,available)?;
                        Ok(std::borrow::Cow::Owned(row.value().clone()))
                    })
                    .transpose()
            }
        }
    }
    pub(crate) fn contains(&self, key: &str) -> Result<bool> {
        match self {
            Self::Resident(m) => Ok(m.contains_key(key)),
            Self::Disk {
                storage, category, ..
            } => {
                let s = storage.borrow();
                s.guard()?;
                let found = s.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM navigation_rows WHERE category=?1 AND key=?2)",
                    params![category, key],
                    |r| r.get(0),
                )?;
                s.guard()?;
                Ok(found)
            }
        }
    }
    pub(crate) fn insert(&mut self, key: String, value: serde_json::Value) -> Result<()> {
        match self {
            Self::Resident(m) => {
                m.insert(key, value);
                Ok(())
            }
            Self::Disk {
                storage,
                category,
                count,
            } => {
                let mut s = storage.borrow_mut();
                s.guard()?;
                let raw =
                    crate::source_bibliographic_render::encode(&value, s.limits.max_row_bytes)?;
                let exists = s.db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM navigation_rows WHERE category=?1 AND key=?2)",
                    params![category, key],
                    |r| r.get::<_, bool>(0),
                );
                s.guard()?;
                let exists = exists?;
                s.put(category, &key, &raw)?;
                if !exists {
                    *count = count
                        .checked_add(1)
                        .ok_or(Error::Budget("navigation map count"))?;
                }
                Ok(())
            }
        }
    }
    pub(crate) fn next(&self, after: Option<&str>) -> Result<Option<String>> {
        match self {
            Self::Resident(m) => Ok(match after {
                None => m.keys().next().cloned(),
                Some(after) => m
                    .range::<str, _>((std::ops::Bound::Excluded(after), std::ops::Bound::Unbounded))
                    .next()
                    .map(|(k, _)| k.clone()),
            }),
            Self::Disk {
                storage, category, ..
            } => storage.borrow().next(category, after),
        }
    }
    pub(crate) fn into_values(self) -> Result<Vec<serde_json::Value>> {
        match self {
            Self::Resident(m) => Ok(m.into_values().collect()),
            Self::Disk { .. } => Err(Error::Invalid("navigation disk rows require bounded sink")),
        }
    }
}

pub(crate) enum NavigationList<'a> {
    Resident(Vec<serde_json::Value>),
    Disk {
        storage: SharedNavigationStorage<'a>,
        category: &'static str,
        count: usize,
    },
}
impl<'a> NavigationList<'a> {
    pub(crate) fn new(
        storage: Option<&SharedNavigationStorage<'a>>,
        category: &'static str,
    ) -> Self {
        match storage {
            None => Self::Resident(Vec::new()),
            Some(s) => Self::Disk {
                storage: s.clone(),
                category,
                count: 0,
            },
        }
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Resident(v) => v.len(),
            Self::Disk { count, .. } => *count,
        }
    }
    pub(crate) fn push(&mut self, value: serde_json::Value) -> Result<()> {
        match self {
            Self::Resident(v) => {
                v.push(value);
                Ok(())
            }
            Self::Disk {
                storage,
                category,
                count,
            } => {
                let mut s = storage.borrow_mut();
                s.guard()?;
                let raw =
                    crate::source_bibliographic_render::encode(&value, s.limits.max_row_bytes)?;
                let key = format!("{count:020}");
                let order = if *category == "rights" {
                    value["rights_id"].as_str()
                } else {
                    Some(key.as_str())
                };
                if *category == "file_diagnostics" {
                    s.put_sorted(
                        category,
                        &key,
                        [
                            value["path"].as_str(),
                            value["message"].as_str(),
                            value["level"].as_str(),
                        ],
                        &raw,
                    )?;
                } else {
                    s.put_ordered(category, &key, order, &raw)?;
                }
                *count = count
                    .checked_add(1)
                    .ok_or(Error::Budget("navigation list count"))?;
                Ok(())
            }
        }
    }
    pub(crate) fn drain_file_diagnostics(
        self,
        mut visit: impl FnMut(serde_json::Value) -> Result<()>,
    ) -> Result<()> {
        match self {
            Self::Resident(mut values) => {
                values.sort_by(|a, b| {
                    ["path", "message", "level"]
                        .iter()
                        .map(|k| a[*k].as_str())
                        .cmp(["path", "message", "level"].iter().map(|k| b[*k].as_str()))
                });
                for value in values {
                    visit(value)?;
                }
            }
            Self::Disk {
                storage,
                category,
                count,
            } => {
                let mut after: Option<(String, String, String, String)> = None;
                let mut observed = 0u64;
                loop {
                    let next = {
                        let s = storage.borrow();
                        s.guard()?;
                        let read = |r: &rusqlite::Row<'_>| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, String>(2)?,
                                r.get::<_, String>(3)?,
                            ))
                        };
                        s.workspace_available(
                            s.limits
                                .max_row_bytes
                                .checked_mul(8)
                                .ok_or(Error::Budget("navigation diagnostic cursor workspace"))?,
                        )?;
                        let columns = "SELECT sort_key,sort_second,sort_third,key FROM navigation_rows WHERE category=?1";
                        let row = match after.as_ref() {
                            Some((a,b,c,d)) => s.db.query_row(&format!("{columns} AND (sort_key,sort_second,sort_third,key)>(?2,?3,?4,?5) ORDER BY sort_key,sort_second,sort_third,key LIMIT 1"), params![category,a,b,c,d], read).optional()?,
                            None => s.db.query_row(&format!("{columns} ORDER BY sort_key,sort_second,sort_third,key LIMIT 1"), params![category], read).optional()?,
                        };
                        s.guard()?;
                        row
                    };
                    let Some(next) = next else {
                        break;
                    };
                    let value = {
                        let s = storage.borrow();
                        let raw = s
                            .get(category, &next.3)?
                            .ok_or(Error::Invalid("navigation diagnostic disappeared"))?;
                        let cursor = s
                            .limits
                            .max_row_bytes
                            .checked_mul(8)
                            .ok_or(Error::Budget("navigation diagnostic cursor workspace"))?;
                        let available = s.workspace_available(
                            raw.len()
                                .checked_add(cursor)
                                .ok_or(Error::Budget("navigation diagnostic workspace"))?,
                        )?;
                        let (parsed, _) = crate::knowledge_normalization::SourceRow::parse_raw_input_with_state_budget(&raw, s.limits.max_row_bytes, available / 2)?;
                        parsed.value().clone()
                    };
                    // Release the immutable storage loan before the sink writes
                    // its separate diagnostic category in this same scope.
                    after = Some(next);
                    visit(value)?;
                    observed = observed
                        .checked_add(1)
                        .ok_or(Error::Budget("navigation diagnostic count"))?;
                }
                if observed != count as u64 {
                    return Err(Error::Invalid("navigation diagnostic count"));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn sort_rights(&mut self) {
        if let Self::Resident(v) = self {
            v.sort_by(|a, b| a["rights_id"].as_str().cmp(&b["rights_id"].as_str()));
        }
    }
    pub(crate) fn into_values(self) -> Result<Vec<serde_json::Value>> {
        match self {
            Self::Resident(v) => Ok(v),
            Self::Disk { .. } => Err(Error::Invalid("navigation disk rows require bounded sink")),
        }
    }
}

pub(crate) struct NavigationPaths<'a>(NavigationMap<'a>);
impl<'a> NavigationPaths<'a> {
    pub(crate) fn new(storage: Option<&SharedNavigationStorage<'a>>) -> Self {
        Self(NavigationMap::new(storage, "paths"))
    }
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn push(&mut self, path: String) -> Result<()> {
        self.0
            .insert(path.clone(), serde_json::json!({"path":path}))
    }
    pub(crate) fn selected<'s>(
        &'s self,
        predicate: impl Fn(&str) -> bool + 's,
    ) -> impl Iterator<Item = Result<String>> + 's {
        NavigationPathsIter {
            paths: self,
            after: None,
            done: false,
        }
        .filter_map(move |row| match row {
            Ok(path) if predicate(&path) => Some(Ok(path)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
    }
}
struct NavigationPathsIter<'s, 'a> {
    paths: &'s NavigationPaths<'a>,
    after: Option<String>,
    done: bool,
}
impl Iterator for NavigationPathsIter<'_, '_> {
    type Item = Result<String>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let result = (|| {
            let Some(key) = self.paths.0.next(self.after.as_deref())? else {
                return Ok(None);
            };
            let row = self
                .paths
                .0
                .get(&key)?
                .ok_or(Error::Invalid("navigation path disappeared"))?;
            if row["path"].as_str() != Some(key.as_str()) {
                return Err(Error::Invalid("navigation path row binding"));
            }
            self.after = Some(key.clone());
            Ok(Some(key))
        })();
        match result {
            Ok(Some(path)) => Some(Ok(path)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}
