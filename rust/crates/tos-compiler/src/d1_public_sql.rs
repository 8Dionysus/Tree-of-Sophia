//! Offline Cloudflare D1 v9 SQL emission. The caller owns one fresh build
//! district and publishes the complete set only after source/current recheck.

use crate::{
    Error, Result,
    d1_public_baseline::PublicRowIndex,
    d1_public_capture::{PublicCapture, PublicCaptureLimits},
};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use tos_foundation::{Digest256, Digest256Hasher};

pub(crate) const MAX_STATEMENT_BYTES: usize = 100_000;
pub(crate) const MAX_ROW_VALUE_BYTES: usize = 2_000_000;
pub(crate) const MAX_INSERT_ROWS: usize = 512;
pub(crate) const CHUNK_BYTES: usize = 32_000;

fn row_bytes(values: &[&str]) -> Result<usize> {
    values.iter().try_fold(1024usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(Error::Budget("D1 SQL row value bytes"))
    })
}

pub(crate) fn quote(value: &str) -> Result<String> {
    let len = quote_len(value)?;
    let mut result = String::with_capacity(len);
    result.push('\'');
    for c in value.chars() {
        result.push(c);
        if c == '\'' {
            result.push('\'');
        }
    }
    result.push('\'');
    Ok(result)
}

pub(crate) fn quote_len(value: &str) -> Result<usize> {
    if value.contains('\0') {
        return Err(Error::Invalid("NUL in D1 SQL text"));
    }
    value
        .len()
        .checked_add(value.bytes().filter(|byte| *byte == b'\'').count())
        .and_then(|len| len.checked_add(2))
        .ok_or(Error::Budget("D1 SQL quoted value bytes"))
}

pub(crate) fn chunks(value: &str) -> impl Iterator<Item = &str> {
    struct Parts<'a> {
        value: &'a str,
        offset: usize,
    }
    impl<'a> Iterator for Parts<'a> {
        type Item = &'a str;
        fn next(&mut self) -> Option<Self::Item> {
            if self.offset >= self.value.len() {
                return None;
            }
            let mut end = (self.offset + CHUNK_BYTES).min(self.value.len());
            while end > self.offset && !self.value.is_char_boundary(end) {
                end -= 1;
            }
            let result = &self.value[self.offset..end];
            self.offset = end;
            Some(result)
        }
    }
    Parts { value, offset: 0 }
}

pub(crate) struct SqlSink<'a> {
    pending: PathBuf,
    writer: BufWriter<File>,
    bytes: u64,
    statements: u64,
    max_bytes: u64,
    finished: bool,
    capture: &'a PublicCapture,
    index: Option<PublicRowIndex>,
}
impl<'a> SqlSink<'a> {
    pub(crate) fn create(
        pending: &Path,
        index_path: &Path,
        max_bytes: u64,
        capture: &'a PublicCapture,
        limits: PublicCaptureLimits,
    ) -> Result<Self> {
        if max_bytes == 0 || pending.exists() || pending.is_symlink() {
            return Err(Error::Budget("D1 SQL output path/budget"));
        }
        let index = PublicRowIndex::create(index_path, capture, limits)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(pending)?;
        Ok(Self {
            pending: pending.to_owned(),
            writer: BufWriter::new(file),
            bytes: 0,
            statements: 0,
            max_bytes,
            finished: false,
            capture,
            index: Some(index),
        })
    }
    pub(crate) fn line(&mut self, statement: &str) -> Result<()> {
        if statement.len() > MAX_STATEMENT_BYTES {
            return Err(Error::Budget("D1 SQL statement bytes"));
        }
        self.bytes = self
            .bytes
            .checked_add(statement.len() as u64 + 1)
            .filter(|n| *n <= self.max_bytes)
            .ok_or(Error::Budget("D1 SQL output bytes"))?;
        self.capture.charge_work(statement.len() as u64 + 1)?;
        self.writer.write_all(statement.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.statements += 1;
        Ok(())
    }
    pub(crate) fn insert(
        &mut self,
        table: &str,
        columns: &[&str],
        values: &[String],
    ) -> Result<()> {
        self.capture
            .charge_work((values.len() * std::mem::size_of::<&str>()) as u64)?;
        let borrowed: Vec<&str> = values.iter().map(String::as_str).collect();
        let start = self.bytes;
        let statement = self.insert_borrowed(table, columns, &borrowed)?;
        self.index.as_mut().expect("public row index open").record(
            table,
            columns,
            &borrowed,
            Digest256::of_bytes(statement.as_bytes()),
            &[(start, statement.len() as u64)],
            self.capture,
        )
    }
    fn insert_borrowed(
        &mut self,
        table: &str,
        columns: &[&str],
        values: &[&str],
    ) -> Result<String> {
        if columns.len() != values.len() || row_bytes(values)? > MAX_ROW_VALUE_BYTES {
            return Err(Error::Budget("D1 SQL row value bytes"));
        }
        let statement_bytes = insert_len(table, columns, values)?;
        if statement_bytes > MAX_STATEMENT_BYTES {
            return Err(Error::Budget("D1 SQL statement bytes"));
        }
        self.capture.charge_work(statement_bytes as u64)?;
        let mut statement = String::with_capacity(statement_bytes);
        statement.push_str("INSERT INTO ");
        statement.push_str(table);
        statement.push_str(" (");
        for (index, column) in columns.iter().enumerate() {
            if index != 0 {
                statement.push(',');
            }
            statement.push_str(column);
        }
        statement.push_str(") VALUES (");
        for (index, value) in values.iter().enumerate() {
            if index != 0 {
                statement.push(',');
            }
            statement.push_str(value);
        }
        statement.push_str(");");
        self.line(&statement)?;
        Ok(statement)
    }
    /// Preserve a large source value through bounded UPDATEs. Each UPDATE is
    /// still charged as output; the final SQLite cell is never truncated.
    pub(crate) fn insert_chunked(
        &mut self,
        table: &str,
        columns: &[&str],
        values: &[String],
        selector: &str,
        chunked: &[(&str, &str)],
    ) -> Result<()> {
        if columns.len() != values.len() {
            return Err(Error::Budget("D1 SQL row value bytes"));
        }
        self.capture
            .charge_work((values.len() * std::mem::size_of::<&str>()) as u64)?;
        let borrowed: Vec<&str> = values.iter().map(String::as_str).collect();
        // Statement chunking never licenses an oversized eventual SQLite row.
        if row_bytes(&borrowed)? > MAX_ROW_VALUE_BYTES {
            return Err(Error::Budget("D1 SQL row value bytes"));
        }
        if insert_len(table, columns, &borrowed)? <= MAX_STATEMENT_BYTES {
            let start = self.bytes;
            let statement = self.insert_borrowed(table, columns, &borrowed)?;
            return self.index.as_mut().expect("public row index open").record(
                table,
                columns,
                &borrowed,
                Digest256::of_bytes(statement.as_bytes()),
                &[(start, statement.len() as u64)],
                self.capture,
            );
        }
        let mut base = borrowed.clone();
        for (column, _) in chunked {
            let position = columns
                .iter()
                .position(|field| field == column)
                .ok_or(Error::Invalid("D1 chunk column"))?;
            base[position] = "''";
        }
        if chunked.is_empty() || row_bytes(&base)? > MAX_ROW_VALUE_BYTES {
            return Err(Error::Budget("D1 SQL selection row value bytes"));
        }
        let start = self.bytes;
        let initial = self.insert_borrowed(table, columns, &base)?;
        let mut segments = vec![(start, initial.len() as u64)];
        let mut digest = Digest256Hasher::new();
        digest.update(initial.as_bytes());
        for (column, value) in chunked {
            if value.is_empty() {
                continue;
            }
            let mut prefix_bytes = 0u64;
            for chunk in chunks(value) {
                prefix_bytes = prefix_bytes
                    .checked_add(chunk.len() as u64)
                    .ok_or(Error::Budget("D1 SQL concatenated cell bytes"))?;
                // SQLite materializes the growing cell at every UPDATE, not
                // just the newly appended bytes carried by the SQL text.
                self.capture.charge_work(prefix_bytes)?;
                let quoted_bytes = quote_len(chunk)?;
                self.capture.charge_work(quoted_bytes as u64)?;
                let quoted = quote(chunk)?;
                let statement_bytes = "UPDATE  SET =|| WHERE ;"
                    .len()
                    .checked_add(table.len())
                    .and_then(|n| n.checked_add(column.len() * 2))
                    .and_then(|n| n.checked_add(selector.len()))
                    .and_then(|n| n.checked_add(quoted.len()))
                    .ok_or(Error::Budget("D1 SQL chunk statement bytes"))?;
                if statement_bytes > MAX_STATEMENT_BYTES {
                    return Err(Error::Budget("D1 SQL statement bytes"));
                }
                self.capture.charge_work(statement_bytes as u64)?;
                let statement = format!(
                    "UPDATE {table} SET {column}={column}||{} WHERE {selector};",
                    quoted
                );
                let start = self.bytes;
                self.line(&statement)?;
                segments.push((start, statement.len() as u64));
                digest.update(b"\n");
                digest.update(statement.as_bytes());
            }
        }
        self.index.as_mut().expect("public row index open").record(
            table,
            columns,
            &borrowed,
            digest.finalize(),
            &segments,
            self.capture,
        )
    }
    pub(crate) fn finish(
        mut self,
        baseline: &Path,
        revision: &str,
        reader_top: &Value,
        max_baseline_bytes: u64,
    ) -> Result<(PathBuf, PathBuf, u64, u64, u64, PublicRowIndex)> {
        let baseline_bytes = self
            .index
            .as_mut()
            .expect("public row index open")
            .emit_json(
                baseline,
                self.capture,
                revision,
                reader_top,
                max_baseline_bytes,
            )?;
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        self.finished = true;
        let index = self.index.take().expect("public row index open");
        Ok((
            self.pending.clone(),
            baseline.to_owned(),
            self.bytes,
            self.statements,
            baseline_bytes,
            index,
        ))
    }
}
fn insert_len(table: &str, columns: &[&str], values: &[&str]) -> Result<usize> {
    let mut total = "INSERT INTO  () VALUES ();"
        .len()
        .checked_add(table.len())
        .ok_or(Error::Budget("D1 SQL statement bytes"))?;
    for (index, column) in columns.iter().enumerate() {
        total = total
            .checked_add(column.len() + usize::from(index != 0))
            .ok_or(Error::Budget("D1 SQL statement bytes"))?;
    }
    for (index, value) in values.iter().enumerate() {
        total = total
            .checked_add(value.len() + usize::from(index != 0))
            .ok_or(Error::Budget("D1 SQL statement bytes"))?;
    }
    Ok(total)
}
impl Drop for SqlSink<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = fs::remove_file(&self.pending);
        }
    }
}
