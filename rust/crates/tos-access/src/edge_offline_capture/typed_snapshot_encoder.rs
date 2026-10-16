//! Borrowed-view TOSLSNP1 producer. This is a child of `typed_snapshot`, so the
//! importer owns the role registry and wire profile. Source SQL is evidence;
//! it is never executed. No source path is opened and no transaction is changed.
//!
//! The caller holds all selected transactions and an exclusive, empty output
//! file throughout encode and consumption. It supplies its original deadline
//! and cancellation check, and owns VM cancellation during a running SQLite
//! step via its interrupt/VM guard (this module must not replace a borrowed
//! connection's progress hook). The caller must exclude concurrent/reentrant
//! use of each borrowed connection, including inside its cancellation check.
//! Failure leaves partial output and consumed budget with the caller.

use super::{DatabaseEncoding, Role, build_inventory, hex, tables};
use rusqlite::{Connection, ffi};
use serde_json::{Value, json};
use std::{
    cell::Cell,
    collections::BTreeMap,
    ffi::CString,
    fs::File,
    io::{Seek, Write},
    ptr,
    rc::Rc,
    slice,
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher};

/// One ledger for all selected frames. Charges are conservative logical state,
/// not a process RSS guarantee, and are never refunded on a failed capture.
pub(crate) struct EncodeBudget {
    frame_remaining: u64,
    schema_remaining: Rc<Cell<u64>>,
}
impl EncodeBudget {
    pub(crate) fn new(frame_bytes: u64, schema_bytes: u64) -> Result<Self, String> {
        if frame_bytes == 0 || schema_bytes == 0 {
            return Err("typed encoder requires positive finite budgets".into());
        }
        Ok(Self {
            frame_remaining: frame_bytes,
            schema_remaining: Rc::new(Cell::new(schema_bytes)),
        })
    }
    fn schema(&mut self, bytes: u64) -> Result<(), String> {
        reserve_schema(&self.schema_remaining, bytes)
    }
    pub(crate) fn schema_meter(&self) -> Rc<Cell<u64>> {
        self.schema_remaining.clone()
    }
    fn frame(&mut self, bytes: u64) -> Result<(), String> {
        self.frame_remaining = self
            .frame_remaining
            .checked_sub(bytes)
            .ok_or("typed encoder aggregate frame budget")?;
        Ok(())
    }
}

/// Engine operations only: the encoder owns every SELECT and the wire registry.
/// An adapter must exclude concurrent/reentrant use and preserve the same held
/// transaction. It may not execute schema SQL or reopen a selected source path.
pub(crate) trait ReadView {
    fn paired_row_lengths(&self) -> bool {
        false
    }
    fn transaction_held(&self) -> Result<bool, String>;
    fn query<'a>(
        &'a self,
        sql: &str,
        deadline: Instant,
        cancel: &'a dyn Fn() -> Result<(), String>,
    ) -> Result<Box<dyn ReadStatement + 'a>, String>;
}
pub(crate) trait ReadStatement {
    fn raw_row_cap(&mut self, _bytes: u64, _prepaid: bool) -> Result<(), String> {
        Ok(())
    }
    fn step(&mut self) -> Result<bool, String>;
    fn kind(&self, column: i32) -> i32;
    fn integer(&self, column: i32) -> Result<i64, String>;
    fn blob(&self, column: i32) -> Result<Option<&[u8]>, String>;
    fn real_bits(&self, column: i32) -> Result<[u8; 8], String>;
}
pub(crate) fn reserve_schema(meter: &Cell<u64>, bytes: u64) -> Result<(), String> {
    meter.set(
        meter
            .get()
            .checked_sub(bytes)
            .ok_or("typed encoder cumulative schema allocation budget")?,
    );
    Ok(())
}
struct Active<'a> {
    db: &'a dyn ReadView,
    deadline: Instant,
    cancel: &'a dyn Fn() -> Result<(), String>,
}
impl Active<'_> {
    fn check(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("typed encoder deadline exceeded".into());
        }
        (self.cancel)()?;
        if !self.db.transaction_held()? {
            return Err("typed encoder caller released selected transaction".into());
        }
        Ok(())
    }
    fn query(&self, sql: &str) -> Result<Box<dyn ReadStatement + '_>, String> {
        self.check()?;
        self.db.query(sql, self.deadline, self.cancel)
    }
}
struct SqliteView<'a>(&'a Connection);
impl ReadView for SqliteView<'_> {
    fn transaction_held(&self) -> Result<bool, String> {
        Ok(!self.0.is_autocommit())
    }
    fn query<'a>(
        &'a self,
        sql: &str,
        deadline: Instant,
        cancel: &'a dyn Fn() -> Result<(), String>,
    ) -> Result<Box<dyn ReadStatement + 'a>, String> {
        let sql = CString::new(sql).map_err(|_| "typed encoder query NUL")?;
        let mut statement = ptr::null_mut();
        let mut tail = ptr::null();
        // Only this bundled engine's own handle is used; no foreign handle.
        let status = unsafe {
            ffi::sqlite3_prepare_v2(self.0.handle(), sql.as_ptr(), -1, &mut statement, &mut tail)
        };
        let result = RawStatement {
            statement,
            db: self.0,
            deadline,
            cancel,
        };
        if status != ffi::SQLITE_OK || statement.is_null() {
            return Err(format!("typed encoder prepare status {status}"));
        }
        if unsafe { ffi::sqlite3_stmt_readonly(statement) } != 1 {
            return Err("typed encoder query is not read only".into());
        }
        result.check()?;
        Ok(Box::new(result))
    }
}
struct RawStatement<'a> {
    statement: *mut ffi::sqlite3_stmt,
    db: &'a Connection,
    deadline: Instant,
    cancel: &'a dyn Fn() -> Result<(), String>,
}
impl RawStatement<'_> {
    fn check(&self) -> Result<(), String> {
        if Instant::now() >= self.deadline {
            return Err("typed encoder deadline exceeded".into());
        }
        (self.cancel)()?;
        if self.db.is_autocommit() {
            return Err("typed encoder caller released selected transaction".into());
        }
        Ok(())
    }
}
impl Drop for RawStatement<'_> {
    fn drop(&mut self) {
        if !self.statement.is_null() {
            unsafe {
                ffi::sqlite3_finalize(self.statement);
            }
        }
    }
}
impl ReadStatement for RawStatement<'_> {
    fn step(&mut self) -> Result<bool, String> {
        self.check()?;
        let status = unsafe { ffi::sqlite3_step(self.statement) };
        self.check()?;
        match status {
            ffi::SQLITE_ROW => Ok(true),
            ffi::SQLITE_DONE => Ok(false),
            _ => Err(format!("typed encoder SQLite step status {status}")),
        }
    }
    fn kind(&self, column: i32) -> i32 {
        unsafe { ffi::sqlite3_column_type(self.statement, column) }
    }
    fn integer(&self, column: i32) -> Result<i64, String> {
        if self.kind(column) != ffi::SQLITE_INTEGER {
            return Err("typed encoder expected integer evidence".into());
        }
        Ok(unsafe { ffi::sqlite3_column_int64(self.statement, column) })
    }
    fn real_bits(&self, column: i32) -> Result<[u8; 8], String> {
        if self.kind(column) != ffi::SQLITE_FLOAT {
            return Err("typed encoder expected real evidence".into());
        }
        Ok(unsafe { ffi::sqlite3_column_double(self.statement, column) }.to_le_bytes())
    }
    fn blob(&self, column: i32) -> Result<Option<&[u8]>, String> {
        match self.kind(column) {
            ffi::SQLITE_NULL => return Ok(None),
            ffi::SQLITE_BLOB => {}
            _ => return Err("typed encoder expected raw BLOB evidence".into()),
        }
        let pointer = unsafe { ffi::sqlite3_column_blob(self.statement, column) };
        let length = unsafe { ffi::sqlite3_column_bytes(self.statement, column) };
        if length < 0 || (length != 0 && pointer.is_null()) {
            return Err("typed encoder invalid raw cell".into());
        }
        if length == 0 {
            return Ok(Some(&[]));
        }
        // Borrow ends before next step/finalize; SQLite CAST AS BLOB avoids TEXT conversion.
        Ok(Some(unsafe {
            slice::from_raw_parts(pointer.cast(), length as usize)
        }))
    }
}
fn add(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or_else(|| "typed encoder size overflow".into())
}
fn mul(a: u64, b: u64) -> Result<u64, String> {
    a.checked_mul(b)
        .ok_or_else(|| "typed encoder size overflow".into())
}
fn nonnegative(n: i64) -> Result<u64, String> {
    u64::try_from(n).map_err(|_| "typed encoder negative size evidence".into())
}
fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
fn literal(name: &str) -> String {
    format!("'{}'", name.replace('\'', "''"))
}
fn decode(raw: &[u8], encoding: DatabaseEncoding) -> Result<String, String> {
    match encoding {
        DatabaseEncoding::Utf8 => {
            String::from_utf8(raw.to_vec()).map_err(|_| "typed encoder schema UTF-8".into())
        }
        DatabaseEncoding::Utf16Le | DatabaseEncoding::Utf16Be => {
            if raw.len() % 2 != 0 {
                return Err("typed encoder schema odd UTF-16".into());
            }
            let units = raw.chunks_exact(2).map(|b| match encoding {
                DatabaseEncoding::Utf16Le => u16::from_le_bytes([b[0], b[1]]),
                _ => u16::from_be_bytes([b[0], b[1]]),
            });
            char::decode_utf16(units)
                .collect::<Result<String, _>>()
                .map_err(|_| "typed encoder schema invalid UTF-16".into())
        }
    }
}
fn wire(
    output: &mut Vec<u8>,
    value: Option<&str>,
    wide: bool,
    nullable: bool,
) -> Result<(), String> {
    let maximum = if wide {
        u32::MAX as u64
    } else {
        u16::MAX as u64
    };
    let length = match value {
        Some(v) if (v.len() as u64) < maximum + u64::from(!nullable) => v.len() as u64,
        Some(_) => return Err("typed encoder schema string width".into()),
        None if nullable => maximum,
        None => return Err("typed encoder required schema string absent".into()),
    };
    if wide {
        output.extend_from_slice(&(length as u32).to_le_bytes());
    } else {
        output.extend_from_slice(&(length as u16).to_le_bytes());
    }
    if let Some(v) = value {
        output.extend_from_slice(v.as_bytes());
    }
    Ok(())
}
#[derive(Clone)]
enum Evidence {
    Integer(i64),
    Text(String),
    Null,
}
fn number(row: &[Evidence], at: usize) -> Result<i64, String> {
    match &row[at] {
        Evidence::Integer(n) => Ok(*n),
        _ => Err("typed encoder schema integer".into()),
    }
}
fn text(row: &[Evidence], at: usize) -> Result<&str, String> {
    match &row[at] {
        Evidence::Text(s) => Ok(s),
        _ => Err("typed encoder schema text".into()),
    }
}
fn optional_text(row: &[Evidence], at: usize) -> Result<Option<&str>, String> {
    match &row[at] {
        Evidence::Text(s) => Ok(Some(s)),
        Evidence::Null => Ok(None),
        _ => Err("typed encoder nullable schema text".into()),
    }
}
fn boolean(n: i64) -> Result<u8, String> {
    match n {
        0 | 1 => Ok(n as u8),
        _ => Err("typed encoder schema boolean".into()),
    }
}
fn width16(n: i64) -> Result<u16, String> {
    u16::try_from(n).map_err(|_| "typed encoder schema u16 width".into())
}
fn scalar(active: &Active<'_>, sql: &str) -> Result<u64, String> {
    let mut query = active.query(sql)?;
    if !query.step()? {
        return Err("typed encoder missing scalar".into());
    }
    let value = nonnegative(query.integer(0)?)?;
    if query.step()? {
        return Err("typed encoder extra scalar".into());
    }
    Ok(value)
}
fn metadata(
    active: &Active<'_>,
    encoding: DatabaseEncoding,
    budget: &mut EncodeBudget,
    remaining: u64,
    query: &str,
    preflight: &str,
    columns: usize,
) -> Result<Vec<Vec<Evidence>>, String> {
    let mut size = active.query(preflight)?;
    if !size.step()? {
        return Err("typed encoder metadata preflight absent".into());
    }
    let count = nonnegative(size.integer(0)?)?;
    let bytes = nonnegative(size.integer(1)?)?;
    if add(mul(bytes, 3)?, mul(count, 16)?)? > remaining {
        return Err("typed encoder metadata frame envelope".into());
    }
    budget.schema(add(mul(count, 512)?, mul(bytes, 24)?)?)?;
    drop(size);
    let mut result = Vec::new();
    result
        .try_reserve_exact(usize::try_from(count).map_err(|_| "typed encoder metadata count")?)
        .map_err(|_| "typed encoder metadata allocation")?;
    let mut rows = active.query(query)?;
    rows.raw_row_cap(add(bytes, mul(columns as u64, 8)?)?, true)?;
    let mut used = 0;
    while rows.step()? {
        if result.len() as u64 >= count {
            return Err("typed encoder metadata count changed".into());
        }
        let mut row = Vec::with_capacity(columns);
        used = add(used, 64)?;
        for column in 0..columns {
            let column = column as i32;
            match rows.kind(column) {
                ffi::SQLITE_INTEGER => {
                    used = add(used, 16)?;
                    row.push(Evidence::Integer(rows.integer(column)?));
                }
                ffi::SQLITE_NULL => {
                    used = add(used, 16)?;
                    row.push(Evidence::Null);
                }
                ffi::SQLITE_BLOB => {
                    let raw = rows
                        .blob(column)?
                        .ok_or("typed encoder metadata blob absent")?;
                    // Three UTF-8 bytes per original byte safely bounds UTF-16
                    // decoding before constructing any owned schema string.
                    if add(used, mul(raw.len() as u64, 3)?)? > remaining {
                        return Err("typed encoder metadata state budget".into());
                    }
                    let value = decode(raw, encoding)?;
                    used = add(used, value.len() as u64)?;
                    row.push(Evidence::Text(value));
                }
                _ => return Err("typed encoder unexpected schema storage class".into()),
            }
            if used > remaining {
                return Err("typed encoder metadata state budget".into());
            }
        }
        result.push(row);
    }
    if result.len() as u64 != count {
        return Err("typed encoder metadata count changed".into());
    }
    Ok(result)
}
struct Table {
    name: String,
    columns: Vec<Vec<Evidence>>,
    rowid: Option<&'static str>,
    descriptor: Vec<u8>,
    flags: u8,
    indexes: u16,
}
struct Planned {
    header: Vec<u8>,
    table: Option<Table>,
    rows: u64,
    opaque: bool,
}
struct Plan {
    encoding: DatabaseEncoding,
    tables: Vec<Planned>,
    objects: Vec<Vec<u8>>,
    total: u64,
}
fn cell_size(columns: &[Vec<Evidence>]) -> Result<String, String> {
    let mut terms = Vec::new();
    for column in columns {
        let name = quoted(text(column, 1)?);
        terms.push(format!("CASE typeof({name}) WHEN 'null' THEN 1 WHEN 'integer' THEN 9 WHEN 'real' THEN 9 ELSE 9+length(CAST({name} AS BLOB)) END"));
    }
    Ok(if terms.is_empty() {
        "0".into()
    } else {
        terms.join("+")
    })
}
fn plan(active: &Active<'_>, role: Role, budget: &mut EncodeBudget) -> Result<Plan, String> {
    let remaining = budget.frame_remaining;
    let mut encoding_query = active.query("SELECT CAST(encoding AS BLOB) FROM pragma_encoding")?;
    if !encoding_query.step()? {
        return Err("typed encoder encoding absent".into());
    }
    let raw = encoding_query
        .blob(0)?
        .ok_or("typed encoder encoding absent")?;
    let encoding = if raw == b"UTF-8" {
        DatabaseEncoding::Utf8
    } else if raw == b"U\0T\0F\0-\01\06\0l\0e\0" {
        DatabaseEncoding::Utf16Le
    } else if raw == b"\0U\0T\0F\0-\01\06\0b\0e" {
        DatabaseEncoding::Utf16Be
    } else {
        return Err("typed encoder unsupported database encoding".into());
    };
    drop(encoding_query);
    let mut objects = metadata(
        active,
        encoding,
        budget,
        remaining,
        "SELECT CAST(type AS BLOB),CAST(name AS BLOB),CAST(tbl_name AS BLOB),CAST(sql AS BLOB) FROM main.sqlite_schema ORDER BY CAST(type AS BLOB),CAST(name AS BLOB)",
        "SELECT count(*),coalesce(sum(length(CAST(type AS BLOB))+length(CAST(name AS BLOB))+length(CAST(tbl_name AS BLOB))+coalesce(length(CAST(sql AS BLOB)),0)),0) FROM main.sqlite_schema",
        4,
    )?;
    if objects.len() > super::MAX_SCHEMA_OBJECTS {
        return Err("typed encoder schema object count".into());
    }
    budget.schema(mul(objects.len() as u64, 32)?)?;
    for object in &objects {
        text(object, 0)?;
        text(object, 1)?;
    }
    objects.sort_by(|a, b| match (&a[0], &a[1], &b[0], &b[1]) {
        (Evidence::Text(ak), Evidence::Text(an), Evidence::Text(bk), Evidence::Text(bn)) => {
            (ak, an).cmp(&(bk, bn))
        }
        _ => unreachable!("validated schema object key"),
    });
    let listings = metadata(
        active,
        encoding,
        budget,
        remaining,
        "SELECT CAST(name AS BLOB),CAST(type AS BLOB),CAST(ncol AS INTEGER),CAST(wr AS INTEGER),CAST(strict AS INTEGER) FROM pragma_table_list WHERE schema='main' AND name!='sqlite_schema'",
        "SELECT count(*),coalesce(sum(length(CAST(name AS BLOB))+length(CAST(type AS BLOB))),0) FROM pragma_table_list WHERE schema='main' AND name!='sqlite_schema'",
        5,
    )?;
    let mut actual = BTreeMap::new();
    for entry in listings {
        active.check()?;
        let kind = text(&entry, 1)?;
        if kind == "view" {
            continue;
        }
        if kind != "table" {
            return Err("typed encoder requires ordinary source tables".into());
        }
        let name = text(&entry, 0)?.to_owned();
        let ncol = width16(number(&entry, 2)?)?;
        if usize::from(ncol) > super::MAX_COLUMNS || mul(u64::from(ncol), 64)? > remaining {
            return Err("typed encoder column envelope".into());
        }
        let wr = boolean(number(&entry, 3)?)?;
        let strict = boolean(number(&entry, 4)?)?;
        let arg = literal(&name);
        let columns = metadata(
            active,
            encoding,
            budget,
            remaining,
            &format!(
                "SELECT CAST(cid AS INTEGER),CAST(name AS BLOB),CAST(type AS BLOB),CAST([notnull] AS INTEGER),CAST(pk AS INTEGER),CAST(hidden AS INTEGER),CAST(dflt_value AS BLOB) FROM pragma_table_xinfo({arg}) ORDER BY cid"
            ),
            &format!(
                "SELECT count(*),coalesce(sum(length(CAST(name AS BLOB))+length(CAST(type AS BLOB))+coalesce(length(CAST(dflt_value AS BLOB)),0)),0) FROM pragma_table_xinfo({arg})"
            ),
            7,
        )?;
        if columns.len() != usize::from(ncol) {
            return Err("typed encoder column count changed".into());
        }
        for (i, col) in columns.iter().enumerate() {
            if number(col, 0)? != i as i64 {
                return Err("typed encoder column ordinal changed".into());
            }
        }
        for column in &columns {
            text(column, 1)?;
        }
        let rowid = if wr == 1 {
            None
        } else {
            let alias = ["_rowid_", "rowid", "oid"].into_iter().find(|alias| {
                columns
                    .iter()
                    .all(|col| !matches!(&col[1], Evidence::Text(name) if name.eq_ignore_ascii_case(alias)))
            });
            Some(alias.ok_or("typed encoder source rowid aliases shadowed")?)
        };
        let indices = metadata(
            active,
            encoding,
            budget,
            remaining,
            &format!(
                "SELECT CAST(name AS BLOB),CAST([unique] AS INTEGER),CAST(origin AS BLOB),CAST(partial AS INTEGER) FROM pragma_index_list({arg}) ORDER BY seq"
            ),
            &format!(
                "SELECT count(*),coalesce(sum(length(CAST(name AS BLOB))+length(CAST(origin AS BLOB))),0) FROM pragma_index_list({arg})"
            ),
            4,
        )?;
        if indices.len() > super::MAX_INDEXES {
            return Err("typed encoder index count".into());
        }
        let index_count = width16(indices.len() as i64)?;
        let mut encoded_indices = Vec::new();
        for index in indices {
            let index_name = text(&index, 0)?;
            let arg = literal(index_name);
            let keys = metadata(
                active,
                encoding,
                budget,
                remaining,
                &format!(
                    "SELECT CAST(cid AS INTEGER),CAST(name AS BLOB),CAST([desc] AS INTEGER),CAST(coll AS BLOB),CAST([key] AS INTEGER) FROM pragma_index_xinfo({arg}) ORDER BY seqno"
                ),
                &format!(
                    "SELECT count(*),coalesce(sum(coalesce(length(CAST(name AS BLOB)),0)+length(CAST(coll AS BLOB))),0) FROM pragma_index_xinfo({arg})"
                ),
                5,
            )?;
            if keys.len() > super::MAX_COLUMNS {
                return Err("typed encoder index key count".into());
            }
            let mut size = add(7, index_name.len() as u64)?;
            for key in &keys {
                size = add(
                    size,
                    add(
                        10,
                        add(
                            optional_text(key, 1)?.map_or(0, |s| s.len() as u64),
                            text(key, 3)?.len() as u64,
                        )?,
                    )?,
                )?;
            }
            budget.schema(add(mul(size, 8)?, 128)?)?;
            let mut encoded = Vec::new();
            wire(&mut encoded, Some(index_name), false, false)?;
            encoded.push(boolean(number(&index, 1)?)?);
            encoded.push(match text(&index, 2)? {
                "c" => 0,
                "u" => 1,
                "pk" => 2,
                _ => return Err("typed encoder index origin".into()),
            });
            encoded.push(boolean(number(&index, 3)?)?);
            encoded.extend_from_slice(&width16(keys.len() as i64)?.to_le_bytes());
            for key in &keys {
                active.check()?;
                let cid =
                    i32::try_from(number(key, 0)?).map_err(|_| "typed encoder index cid width")?;
                encoded.extend_from_slice(&cid.to_le_bytes());
                wire(&mut encoded, optional_text(key, 1)?, false, true)?;
                encoded.push(boolean(number(key, 2)?)?);
                wire(&mut encoded, Some(text(key, 3)?), false, false)?;
                encoded.push(boolean(number(key, 4)?)?);
            }
            encoded_indices.push(encoded);
        }
        let mut descriptor_size = 0;
        for col in &columns {
            descriptor_size = add(
                descriptor_size,
                add(
                    12,
                    add(
                        text(col, 1)?.len() as u64,
                        add(
                            text(col, 2)?.len() as u64,
                            optional_text(col, 6)?.map_or(0, |s| s.len() as u64),
                        )?,
                    )?,
                )?,
            )?;
        }
        for index in &encoded_indices {
            descriptor_size = add(descriptor_size, index.len() as u64)?;
        }
        budget.schema(add(
            mul(descriptor_size, 8)?,
            add(mul(columns.len() as u64, 128)?, 256)?,
        )?)?;
        let mut descriptor = Vec::new();
        for col in &columns {
            wire(&mut descriptor, Some(text(col, 1)?), false, false)?;
            wire(&mut descriptor, Some(text(col, 2)?), false, false)?;
            descriptor.push(boolean(number(col, 3)?)?);
            descriptor.extend_from_slice(&width16(number(col, 4)?)?.to_le_bytes());
            descriptor
                .push(u8::try_from(number(col, 5)?).map_err(|_| "typed encoder hidden width")?);
            wire(&mut descriptor, optional_text(col, 6)?, true, true)?;
        }
        for index in encoded_indices {
            descriptor.extend_from_slice(&index);
        }
        let table = Table {
            name: name.clone(),
            columns,
            rowid,
            descriptor,
            flags: wr | (strict << 1) | if rowid.is_some() { 4 } else { 0 },
            indexes: index_count,
        };
        if actual.insert(name, table).is_some() {
            return Err("typed encoder duplicate source table".into());
        }
    }
    let registry = tables(role);
    let mut ordered: Vec<_> = registry
        .iter()
        .map(|spec| (spec.id, spec.name.to_owned()))
        .collect();
    for name in actual.keys() {
        if !registry.iter().any(|spec| spec.name == name) {
            ordered.push((0xffff, name.clone()));
        }
    }
    if ordered.len() - registry.len() > u16::MAX as usize {
        return Err("typed encoder opaque table count".into());
    }
    let mut planned = Vec::new();
    let mut body = 0;
    for (id, name) in ordered {
        active.check()?;
        let table = actual.remove(&name);
        let rows = if let Some(table) = &table {
            let query_table = format!("main.{}", quoted(&name));
            let rows = scalar(active, &format!("SELECT count(*) FROM {query_table}"))?;
            let minimum = add(
                table.columns.len() as u64,
                if table.rowid.is_some() { 8 } else { 0 },
            )?;
            if mul(rows, minimum)? > remaining {
                return Err("typed encoder row count frame envelope".into());
            }
            let cells = scalar(
                active,
                &format!(
                    "SELECT coalesce(sum({}),0) FROM {query_table}",
                    cell_size(&table.columns)?
                ),
            )?;
            body = add(
                body,
                add(cells, mul(rows, if table.rowid.is_some() { 8 } else { 0 })?)?,
            )?;
            rows
        } else {
            0
        };
        let descriptor = table.as_ref().map_or(0, |t| t.descriptor.len() as u64);
        budget.schema(mul(add(add(name.len() as u64, descriptor)?, 20)?, 8)?)?;
        let mut header = Vec::new();
        header.extend_from_slice(&id.to_le_bytes());
        header.push(u8::from(table.is_some()));
        header.push(table.as_ref().map_or(0, |t| t.flags));
        wire(&mut header, Some(&name), false, false)?;
        header.extend_from_slice(
            &table
                .as_ref()
                .map_or(0, |t| t.columns.len() as u16)
                .to_le_bytes(),
        );
        header.extend_from_slice(&table.as_ref().map_or(0, |t| t.indexes).to_le_bytes());
        header.extend_from_slice(&rows.to_le_bytes());
        if let Some(table) = &table {
            header.extend_from_slice(&table.descriptor);
        }
        body = add(body, header.len() as u64)?;
        if add(body, 58)? > remaining {
            return Err("typed encoder planned frame budget".into());
        }
        planned.push(Planned {
            header,
            table,
            rows,
            opaque: id == 0xffff,
        });
    }
    let mut encoded_objects = Vec::new();
    for object in objects {
        let name = text(&object, 1)?;
        let table = text(&object, 2)?;
        let sql = optional_text(&object, 3)?;
        let size = add(
            9,
            add(
                name.len() as u64,
                add(table.len() as u64, sql.map_or(0, |s| s.len() as u64))?,
            )?,
        )?;
        budget.schema(add(mul(size, 8)?, 128)?)?;
        let mut encoded = Vec::new();
        encoded.push(match text(&object, 0)? {
            "table" => 1,
            "index" => 2,
            "trigger" => 3,
            "view" => 4,
            _ => return Err("typed encoder schema object kind".into()),
        });
        wire(&mut encoded, Some(name), false, false)?;
        wire(&mut encoded, Some(table), false, false)?;
        wire(&mut encoded, sql, true, true)?;
        body = add(body, encoded.len() as u64)?;
        encoded_objects.push(encoded);
    }
    let total = add(body, 58)?;
    budget.frame(total)?;
    Ok(Plan {
        encoding,
        tables: planned,
        objects: encoded_objects,
        total,
    })
}

struct Emitter<'a, 'b> {
    output: &'a mut File,
    active: &'a Active<'b>,
    hash: Digest256Hasher,
    complete_hash: Digest256Hasher,
    encoding: DatabaseEncoding,
    opaque_hash: Digest256Hasher,
    current: Option<Digest256Hasher>,
    written: u64,
    total: u64,
}
impl Emitter<'_, '_> {
    fn emit(&mut self, bytes: &[u8], hash: bool) -> Result<(), String> {
        if add(self.written, bytes.len() as u64)? > self.total {
            return Err("typed encoder view grew during capture".into());
        }
        for chunk in bytes.chunks(65536) {
            self.active.check()?;
            self.output
                .write_all(chunk)
                .map_err(|_| "typed encoder output write")?;
        }
        self.written = add(self.written, bytes.len() as u64)?;
        self.complete_hash.update(bytes);
        if hash {
            self.hash.update(bytes);
        }
        if let Some(current) = &mut self.current {
            current.update(bytes);
            self.opaque_hash.update(bytes);
        }
        Ok(())
    }
}
fn rows(emitter: &mut Emitter<'_, '_>, table: &Table, expected: u64) -> Result<(), String> {
    let mut fields = Vec::new();
    if let Some(rowid) = table.rowid {
        fields.push(format!("CAST({} AS INTEGER)", quoted(rowid)));
    }
    for col in &table.columns {
        let name = quoted(text(col, 1)?);
        fields.push(format!("CAST(typeof({name}) AS BLOB)"));
        fields.push(format!(
            "CASE WHEN typeof({name}) IN ('integer','real') THEN {name} ELSE NULL END"
        ));
        fields.push(format!(
            "CASE WHEN typeof({name}) IN ('text','blob') THEN CAST({name} AS BLOB) ELSE NULL END"
        ));
    }
    let order = if let Some(rowid) = table.rowid {
        quoted(rowid)
    } else {
        let mut keys = Vec::new();
        for col in &table.columns {
            let pk = number(col, 4)?;
            if pk > 0 {
                keys.push((pk, quoted(text(col, 1)?)));
            }
        }
        keys.sort_by_key(|(pk, _)| *pk);
        keys.into_iter()
            .map(|(_, name)| name)
            .collect::<Vec<_>>()
            .join(",")
    };
    let sql = format!(
        "SELECT {} FROM main.{} WHERE ({}) <= {}{}",
        fields.join(","),
        quoted(&table.name),
        cell_size(&table.columns)?,
        emitter.total,
        if order.is_empty() {
            String::new()
        } else {
            format!(" ORDER BY {order}")
        }
    );
    let active = emitter.active;
    let mut lengths = if active.db.paired_row_lengths() {
        Some(active.query(&format!(
            "SELECT ({}) FROM main.{} WHERE ({}) <= {}{}",
            cell_size(&table.columns)?,
            quoted(&table.name),
            cell_size(&table.columns)?,
            emitter.total,
            if order.is_empty() {
                String::new()
            } else {
                format!(" ORDER BY {order}")
            }
        ))?)
    } else {
        None
    };
    let mut query = active.query(&sql)?;
    let mut count = 0;
    loop {
        let encoded = if let Some(lengths) = &mut lengths {
            if !lengths.step()? {
                if query.step()? {
                    return Err("typed encoder paired row count differs".into());
                }
                break;
            }
            let length = nonnegative(lengths.integer(0)?)?;
            query.raw_row_cap(add(length, mul(fields.len() as u64, 16)?)?, false)?;
            Some(length)
        } else {
            None
        };
        if !query.step()? {
            if encoded.is_some() {
                return Err("typed encoder paired row absent".into());
            }
            break;
        }
        let row_start = emitter.written;
        if count >= expected {
            return Err("typed encoder row count changed".into());
        }
        let offset = if table.rowid.is_some() {
            emitter.emit(&query.integer(0)?.to_le_bytes(), true)?;
            1
        } else {
            0
        };
        for i in 0..table.columns.len() {
            let base = offset + (i as i32) * 3;
            let raw = query
                .blob(base)?
                .ok_or("typed encoder storage class absent")?;
            let kind = decode(raw, emitter.encoding)?;
            match kind.as_str() {
                "null" => {
                    if query.kind(base + 1) != ffi::SQLITE_NULL
                        || query.kind(base + 2) != ffi::SQLITE_NULL
                    {
                        return Err("typed encoder cell type changed".into());
                    }
                    emitter.emit(&[0], true)?;
                }
                "integer" => {
                    emitter.emit(&[1], true)?;
                    emitter.emit(&query.integer(base + 1)?.to_le_bytes(), true)?;
                }
                "real" => {
                    if query.kind(base + 1) != ffi::SQLITE_FLOAT {
                        return Err("typed encoder real type changed".into());
                    }
                    let value = query.real_bits(base + 1)?;
                    emitter.emit(&[2], true)?;
                    emitter.emit(&value, true)?;
                }
                "text" | "blob" => {
                    let bytes = query
                        .blob(base + 2)?
                        .ok_or("typed encoder raw cell absent")?;
                    emitter.emit(&[if kind == "text" { 3 } else { 4 }], true)?;
                    emitter.emit(&(bytes.len() as u64).to_le_bytes(), true)?;
                    emitter.emit(bytes, true)?;
                }
                _ => return Err("typed encoder cell storage class".into()),
            }
        }
        if let Some(length) = encoded {
            let expected_bytes = add(length, if table.rowid.is_some() { 8 } else { 0 })?;
            if emitter.written - row_start != expected_bytes {
                return Err("typed encoder paired row type/length differs".into());
            }
        }
        count += 1;
    }
    if count != expected {
        return Err("typed encoder row count changed".into());
    }
    Ok(())
}

/// Encode a currently held view. Output must be caller-created exclusive and
/// empty; this function neither creates nor reopens any path. The returned
/// inventory is transport evidence only, matching the existing importer shape.
pub(crate) fn encode_view(
    connection: &dyn ReadView,
    role: Role,
    input_field: &str,
    output: &mut File,
    budget: &mut EncodeBudget,
    deadline: Instant,
    cancel: &dyn Fn() -> Result<(), String>,
) -> Result<Value, String> {
    if !matches!(
        (role, input_field),
        (Role::D1, "d1_database")
            | (
                Role::Prepared,
                "before_prepared_database" | "after_prepared_database"
            )
    ) {
        return Err("typed encoder role/input field mismatch".into());
    }
    let active = Active {
        db: connection,
        deadline,
        cancel,
    };
    active.check()?;
    let output_metadata = output
        .metadata()
        .map_err(|_| "typed encoder output metadata")?;
    if !output_metadata.is_file()
        || output_metadata.len() != 0
        || output
            .stream_position()
            .map_err(|_| "typed encoder output position")?
            != 0
    {
        return Err("typed encoder requires empty regular output file".into());
    }
    let plan = plan(&active, role, budget)?;
    // Result inventory containers are part of the shared allocation ledger.
    budget.schema(add(
        1024,
        mul((plan.tables.len() - tables(role).len()) as u64, 1024)?,
    )?)?;
    let registry_count = width16(tables(role).len() as i64)?;
    let opaque_count = width16((plan.tables.len() - tables(role).len()) as i64)?;
    let mut emitter = Emitter {
        output,
        active: &active,
        hash: Digest256Hasher::new(),
        complete_hash: Digest256Hasher::new(),
        encoding: plan.encoding,
        opaque_hash: Digest256Hasher::new(),
        current: None,
        written: 0,
        total: plan.total,
    };
    let mut header = Vec::from(&b"TOSLSNP1"[..]);
    header.push(role as u8);
    header.push(plan.encoding as u8);
    header.extend_from_slice(&registry_count.to_le_bytes());
    header.extend_from_slice(&opaque_count.to_le_bytes());
    header.extend_from_slice(&(plan.objects.len() as u32).to_le_bytes());
    header.extend_from_slice(&plan.total.to_le_bytes());
    emitter.emit(&header, true)?;
    let mut opaque = Vec::new();
    for table in plan.tables {
        if table.opaque {
            emitter.current = Some(Digest256Hasher::new());
        }
        emitter.emit(&table.header, true)?;
        if let Some(selected) = &table.table {
            rows(&mut emitter, selected, table.rows)?;
        }
        if table.opaque {
            let selected = table
                .table
                .as_ref()
                .ok_or("typed encoder opaque table absent")?;
            let digest = emitter
                .current
                .take()
                .ok_or("typed encoder opaque hash absent")?
                .finalize();
            opaque.push(json!({"name_sha256":hex(Digest256::of_bytes(selected.name.as_bytes())),"row_count":table.rows,"logical_sha256":hex(digest)}));
        }
    }
    let mut object_hash = Digest256Hasher::new();
    for object in plan.objects {
        object_hash.update(&object);
        emitter.emit(&object, true)?;
    }
    if emitter.written != plan.total - 32 {
        return Err("typed encoder planned size changed".into());
    }
    let hash = std::mem::replace(&mut emitter.hash, Digest256Hasher::new()).finalize();
    emitter.emit(hash.as_bytes(), false)?;
    emitter
        .output
        .sync_all()
        .map_err(|_| "typed encoder output sync")?;
    active.check()?;
    if emitter
        .output
        .metadata()
        .map_err(|_| "typed encoder final metadata")?
        .len()
        != plan.total
        || emitter
            .output
            .stream_position()
            .map_err(|_| "typed encoder final position")?
            != plan.total
    {
        return Err("typed encoder output size changed".into());
    }
    Ok(build_inventory(
        input_field,
        plan.encoding,
        plan.total,
        hex(emitter.complete_hash.finalize()),
        hex(object_hash.finalize()),
        hex(emitter.opaque_hash.finalize()),
        opaque,
    ))
}

pub(crate) fn encode(
    connection: &Connection,
    role: Role,
    input_field: &str,
    output: &mut File,
    budget: &mut EncodeBudget,
    deadline: Instant,
    cancel: &dyn Fn() -> Result<(), String>,
) -> Result<Value, String> {
    encode_view(
        &SqliteView(connection),
        role,
        input_field,
        output,
        budget,
        deadline,
        cancel,
    )
}
