//! Bounded default-deny Wasmtime SQLite/KV host capability and HTTP+DB service runner
//! for `calcit:db@0.1.0` (`wasi:http/proxy@0.2.12` + synchronous `sql` and `kv` imports).

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use rusqlite::hooks::Action;
use rusqlite::types::{Value as SqlValue, ValueRef as SqlValueRef};
use rusqlite::{Connection, Error as RusqliteError, ErrorCode, params_from_iter};
use serde_json::Number as JsonNumber;
use wasmtime::component::{Component, ComponentType, Lift, Linker, Lower, ResourceTable};
use wasmtime::{Config, Engine, ResourceLimiter, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::p2::bindings::ProxyPre;
use wasmtime_wasi_http::p2::bindings::http::types::Scheme;
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpCtxView, WasiHttpView};

pub const DB_SQL_INSTANCE: &str = "calcit:db/sql@0.1.0";
pub const DB_KV_INSTANCE: &str = "calcit:db/kv@0.1.0";

const DEFAULT_MAX_ROWS: usize = 1024;
const DEFAULT_MAX_RESULT_BYTES: usize = 60 * 1024;
const DEFAULT_MAX_OPEN_TRANSACTIONS: usize = 4;
const DEFAULT_MAX_KV_VALUE_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_KV_TOTAL_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_MEMORY_LIMIT_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_MAX_RESPONSE_BODY_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, ComponentType, Lift, Lower)]
#[component(variant)]
pub enum DbParam {
    #[component(name = "bool-value")]
    BoolValue(bool),
    #[component(name = "bytes-value")]
    BytesValue(Vec<u8>),
    #[component(name = "float-value")]
    FloatValue(f64),
    #[component(name = "int-value")]
    IntValue(i64),
    #[component(name = "null-value")]
    NullValue,
    #[component(name = "text-value")]
    TextValue(String),
}

#[derive(Clone, Debug, PartialEq, Eq, ComponentType, Lift, Lower)]
#[component(variant)]
pub enum DbError {
    #[component(name = "backend")]
    Backend(String),
    #[component(name = "capability-denied")]
    CapabilityDenied(String),
    #[component(name = "constraint-violation")]
    ConstraintViolation(String),
    #[component(name = "decode-error")]
    DecodeError(String),
    #[component(name = "invalid-query")]
    InvalidQuery(String),
    #[component(name = "timeout")]
    Timeout(String),
    #[component(name = "tx-error")]
    TxError(String),
    #[component(name = "unsupported")]
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq, ComponentType, Lift, Lower)]
#[component(record)]
pub struct DbExecResult {
    #[component(name = "last-insert-id")]
    pub last_insert_id: Option<i64>,
    #[component(name = "rows-affected")]
    pub rows_affected: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, ComponentType, Lift, Lower)]
#[component(record)]
pub struct DbQueryResult {
    pub columns: Vec<String>,
    #[component(name = "rows-json")]
    pub rows_json: String,
}

/// Default-deny capability policy and bounds for database host calls.
#[derive(Clone, Debug)]
pub struct DbConfig {
    allow_sql_read: bool,
    allow_sql_write: bool,
    allow_sql_ddl: bool,
    allowed_kv_prefixes: Arc<BTreeSet<String>>,
    max_rows: usize,
    max_result_bytes: usize,
    max_open_transactions: usize,
    max_kv_value_bytes: usize,
    max_kv_total_bytes: usize,
    query_timeout: Duration,
    unsupported_reason: Option<Arc<str>>,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            allow_sql_read: false,
            allow_sql_write: false,
            allow_sql_ddl: false,
            allowed_kv_prefixes: Arc::default(),
            max_rows: DEFAULT_MAX_ROWS,
            max_result_bytes: DEFAULT_MAX_RESULT_BYTES,
            max_open_transactions: DEFAULT_MAX_OPEN_TRANSACTIONS,
            max_kv_value_bytes: DEFAULT_MAX_KV_VALUE_BYTES,
            max_kv_total_bytes: DEFAULT_MAX_KV_TOTAL_BYTES,
            query_timeout: Duration::from_secs(5),
            unsupported_reason: None,
        }
    }
}

impl DbConfig {
    /// Report database capabilities as unsupported instead of denying with permissions.
    pub fn unsupported(reason: impl Into<Arc<str>>) -> Self {
        Self {
            unsupported_reason: Some(reason.into()),
            ..Self::default()
        }
    }

    pub fn allow_sql_read(mut self) -> Self {
        self.allow_sql_read = true;
        self
    }

    pub fn allow_sql_write(mut self) -> Self {
        self.allow_sql_write = true;
        self
    }

    pub fn allow_sql_ddl(mut self) -> Self {
        self.allow_sql_ddl = true;
        self
    }

    pub fn allow_kv_prefix(mut self, prefix: impl Into<String>) -> Self {
        Arc::make_mut(&mut self.allowed_kv_prefixes).insert(prefix.into());
        self
    }

    pub fn max_rows(mut self, limit: usize) -> Self {
        self.max_rows = limit;
        self
    }

    pub fn max_result_bytes(mut self, limit: usize) -> Self {
        self.max_result_bytes = limit;
        self
    }

    pub fn max_open_transactions(mut self, limit: usize) -> Self {
        self.max_open_transactions = limit;
        self
    }

    pub fn max_kv_value_bytes(mut self, limit: usize) -> Self {
        self.max_kv_value_bytes = limit;
        self
    }

    pub fn max_kv_total_bytes(mut self, limit: usize) -> Self {
        self.max_kv_total_bytes = limit;
        self
    }

    pub fn query_timeout(mut self, timeout: Duration) -> Self {
        self.query_timeout = timeout;
        self
    }

    pub fn allows_sql_read(&self) -> bool {
        self.allow_sql_read
    }

    pub fn allows_sql_write(&self) -> bool {
        self.allow_sql_write
    }

    pub fn allows_sql_ddl(&self) -> bool {
        self.allow_sql_ddl
    }

    pub fn max_rows_limit(&self) -> usize {
        self.max_rows
    }

    pub fn max_result_bytes_limit(&self) -> usize {
        self.max_result_bytes
    }

    pub fn max_open_transactions_limit(&self) -> usize {
        self.max_open_transactions
    }

    pub fn max_kv_value_bytes_limit(&self) -> usize {
        self.max_kv_value_bytes
    }

    pub fn max_kv_total_bytes_limit(&self) -> usize {
        self.max_kv_total_bytes
    }

    pub fn query_timeout_limit(&self) -> Duration {
        self.query_timeout
    }

    /// Enforce policy for read queries (and writable queries with `RETURNING`) before
    /// dispatching to any `SqlBackend`.
    pub fn check_sql_query(&self, sql: &str) -> Result<(), DbError> {
        let kind = inspect_sql_statement(sql)?;
        self.check_sql_kind(kind)?;
        if matches!(
            kind,
            SqlStatementKind::Ddl
                | SqlStatementKind::Write {
                    has_returning: false,
                    ..
                }
        ) {
            return Err(DbError::InvalidQuery(
                "query expects a read statement or a write statement with RETURNING".to_owned(),
            ));
        }
        Ok(())
    }

    /// Enforce policy for mutating SQL (`exec` / `tx-exec`) before dispatching to any
    /// `SqlBackend`.
    pub fn check_sql_exec(&self, sql: &str) -> Result<(), DbError> {
        let kind = inspect_sql_statement(sql)?;
        if matches!(kind, SqlStatementKind::Read) {
            return Err(DbError::InvalidQuery(
                "exec expects a write or DDL SQL statement".to_owned(),
            ));
        }
        self.check_sql_kind(kind)
    }

    /// Enforce policy before starting a managed transaction (`tx-begin`).
    pub fn check_tx_begin(&self) -> Result<(), DbError> {
        self.check_tx_lifecycle()?;
        if !self.allow_sql_read && !self.allow_sql_write && !self.allow_sql_ddl {
            return Err(DbError::CapabilityDenied(
                "SQL capability is denied".to_owned(),
            ));
        }
        Ok(())
    }

    /// Enforce unsupported-host gating for transaction commit/rollback entry points.
    pub fn check_tx_lifecycle(&self) -> Result<(), DbError> {
        if let Some(reason) = &self.unsupported_reason {
            return Err(DbError::Unsupported(reason.to_string()));
        }
        Ok(())
    }

    fn check_sql_kind(&self, kind: SqlStatementKind) -> Result<(), DbError> {
        if let Some(reason) = &self.unsupported_reason {
            return Err(DbError::Unsupported(reason.to_string()));
        }
        match kind {
            SqlStatementKind::Read => {
                if !self.allow_sql_read {
                    return Err(DbError::CapabilityDenied(
                        "SQL read capability is denied".to_owned(),
                    ));
                }
            }
            SqlStatementKind::Write { has_returning, .. } => {
                if !self.allow_sql_write {
                    return Err(DbError::CapabilityDenied(
                        "SQL write capability is denied".to_owned(),
                    ));
                }
                if has_returning && !self.allow_sql_read {
                    return Err(DbError::CapabilityDenied(
                        "SQL read capability is denied for RETURNING".to_owned(),
                    ));
                }
            }
            SqlStatementKind::Ddl => {
                if !self.allow_sql_ddl {
                    return Err(DbError::CapabilityDenied(
                        "SQL DDL capability is denied".to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Enforce KV prefix capability policy before calling any [`KvBackend`] method.
    pub fn check_kv_key(&self, key: &str) -> Result<(), DbError> {
        if let Some(reason) = &self.unsupported_reason {
            return Err(DbError::Unsupported(reason.to_string()));
        }
        if self
            .allowed_kv_prefixes
            .iter()
            .any(|prefix| key.starts_with(prefix))
        {
            Ok(())
        } else {
            Err(DbError::CapabilityDenied(format!(
                "KV key {key:?} is outside granted prefixes"
            )))
        }
    }

    /// Enforce KV prefix policy and per-value size budget before calling [`KvBackend::set`].
    pub fn check_kv_set(&self, key: &str, value: &[u8]) -> Result<(), DbError> {
        self.check_kv_key(key)?;
        if value.len() > self.max_kv_value_bytes {
            return Err(DbError::InvalidQuery(format!(
                "KV value ({} bytes) exceeds max_kv_value_bytes ({})",
                value.len(),
                self.max_kv_value_bytes
            )));
        }
        Ok(())
    }
}

/// Pluggable SQL host implementation backing `calcit:db/sql@0.1.0`.
pub trait SqlBackend: Send {
    fn query(
        &mut self,
        config: &DbConfig,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbQueryResult, DbError>;

    fn exec(
        &mut self,
        config: &DbConfig,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbExecResult, DbError>;

    fn tx_begin(&mut self, config: &DbConfig) -> Result<u64, DbError>;

    fn tx_query(
        &mut self,
        config: &DbConfig,
        tx_id: u64,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbQueryResult, DbError>;

    fn tx_exec(
        &mut self,
        config: &DbConfig,
        tx_id: u64,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbExecResult, DbError>;

    fn tx_commit(&mut self, tx_id: u64) -> Result<(), DbError>;

    fn tx_rollback(&mut self, tx_id: u64) -> Result<(), DbError>;

    fn rollback_all_open_transactions(&mut self) -> Result<usize, DbError>;
}

/// Pluggable KV host implementation backing `calcit:db/kv@0.1.0`.
pub trait KvBackend: Send {
    fn get(&mut self, config: &DbConfig, key: &str) -> Result<Option<Vec<u8>>, DbError>;

    fn set(
        &mut self,
        config: &DbConfig,
        key: &str,
        value: &[u8],
        ttl_ms: Option<u64>,
    ) -> Result<(), DbError>;

    fn delete(&mut self, config: &DbConfig, key: &str) -> Result<bool, DbError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SqlStatementKind {
    Read,
    Write {
        is_insert: bool,
        has_returning: bool,
    },
    Ddl,
}

fn inspect_sql_statement(sql: &str) -> Result<SqlStatementKind, DbError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum LexState {
        Normal,
        SingleQuote,
        DoubleQuote,
        BracketIdent,
        BacktickIdent,
        LineComment,
        BlockComment,
    }

    let bytes = sql.as_bytes();
    let mut state = LexState::Normal;
    let mut idx = 0usize;
    let mut paren_depth = 0usize;
    let mut saw_semicolon = false;
    let mut first_keyword: Option<String> = None;
    let mut after_cte_body = false;
    let mut cte_main_verb: Option<String> = None;
    let mut has_returning = false;

    while idx < bytes.len() {
        let ch = bytes[idx];
        match state {
            LexState::SingleQuote => {
                if ch == b'\'' {
                    if bytes.get(idx + 1) == Some(&b'\'') {
                        idx += 2;
                        continue;
                    }
                    state = LexState::Normal;
                }
                idx += 1;
            }
            LexState::DoubleQuote => {
                if ch == b'"' {
                    if bytes.get(idx + 1) == Some(&b'"') {
                        idx += 2;
                        continue;
                    }
                    state = LexState::Normal;
                }
                idx += 1;
            }
            LexState::BracketIdent => {
                if ch == b']' {
                    state = LexState::Normal;
                }
                idx += 1;
            }
            LexState::BacktickIdent => {
                if ch == b'`' {
                    if bytes.get(idx + 1) == Some(&b'`') {
                        idx += 2;
                        continue;
                    }
                    state = LexState::Normal;
                }
                idx += 1;
            }
            LexState::LineComment => {
                if ch == b'\n' {
                    state = LexState::Normal;
                }
                idx += 1;
            }
            LexState::BlockComment => {
                if ch == b'*' && bytes.get(idx + 1) == Some(&b'/') {
                    state = LexState::Normal;
                    idx += 2;
                } else {
                    idx += 1;
                }
            }
            LexState::Normal => {
                if ch.is_ascii_whitespace() {
                    idx += 1;
                    continue;
                }
                if ch == b'-' && bytes.get(idx + 1) == Some(&b'-') {
                    state = LexState::LineComment;
                    idx += 2;
                    continue;
                }
                if ch == b'/' && bytes.get(idx + 1) == Some(&b'*') {
                    state = LexState::BlockComment;
                    idx += 2;
                    continue;
                }
                if saw_semicolon {
                    return Err(DbError::InvalidQuery(
                        "multi-statement SQL batches are forbidden".to_owned(),
                    ));
                }
                match ch {
                    b'\'' => {
                        if first_keyword.is_none() {
                            return Err(DbError::InvalidQuery(
                                "SQL statement must start with a keyword".to_owned(),
                            ));
                        }
                        state = LexState::SingleQuote;
                        idx += 1;
                    }
                    b'"' => {
                        if first_keyword.is_none() {
                            return Err(DbError::InvalidQuery(
                                "SQL statement must start with a keyword".to_owned(),
                            ));
                        }
                        state = LexState::DoubleQuote;
                        idx += 1;
                    }
                    b'[' => {
                        if first_keyword.is_none() {
                            return Err(DbError::InvalidQuery(
                                "SQL statement must start with a keyword".to_owned(),
                            ));
                        }
                        state = LexState::BracketIdent;
                        idx += 1;
                    }
                    b'`' => {
                        if first_keyword.is_none() {
                            return Err(DbError::InvalidQuery(
                                "SQL statement must start with a keyword".to_owned(),
                            ));
                        }
                        state = LexState::BacktickIdent;
                        idx += 1;
                    }
                    b';' => {
                        saw_semicolon = true;
                        idx += 1;
                    }
                    b',' => {
                        if paren_depth == 0
                            && first_keyword.as_deref() == Some("WITH")
                            && cte_main_verb.is_none()
                        {
                            after_cte_body = false;
                        }
                        idx += 1;
                    }
                    b'(' => {
                        if paren_depth == 0
                            && first_keyword.as_deref() == Some("WITH")
                            && cte_main_verb.is_none()
                        {
                            after_cte_body = false;
                        }
                        paren_depth += 1;
                        idx += 1;
                    }
                    b')' => {
                        let prev = paren_depth;
                        paren_depth = paren_depth.saturating_sub(1);
                        if prev == 1
                            && first_keyword.as_deref() == Some("WITH")
                            && cte_main_verb.is_none()
                        {
                            after_cte_body = true;
                        }
                        idx += 1;
                    }
                    _ if ch.is_ascii_alphabetic() || ch == b'_' => {
                        let start = idx;
                        idx += 1;
                        while idx < bytes.len()
                            && (bytes[idx].is_ascii_alphanumeric() || bytes[idx] == b'_')
                        {
                            idx += 1;
                        }
                        let word = std::str::from_utf8(&bytes[start..idx])
                            .expect("ASCII keyword")
                            .to_ascii_uppercase();
                        if first_keyword.is_none() {
                            first_keyword = Some(word);
                        } else if paren_depth == 0 {
                            if first_keyword.as_deref() == Some("WITH")
                                && after_cte_body
                                && cte_main_verb.is_none()
                            {
                                // `(cols) AS (...)` closes a CTE column list rather than a CTE body.
                                if word == "AS" {
                                    after_cte_body = false;
                                } else {
                                    cte_main_verb = Some(word);
                                }
                            } else if word == "RETURNING" {
                                has_returning = true;
                            }
                        }
                    }
                    _ => {
                        if first_keyword.is_none() {
                            return Err(DbError::InvalidQuery(
                                "SQL statement must start with a keyword".to_owned(),
                            ));
                        }
                        idx += 1;
                    }
                }
            }
        }
    }

    match state {
        LexState::SingleQuote => {
            return Err(DbError::InvalidQuery(
                "unterminated SQL string literal".to_owned(),
            ));
        }
        LexState::DoubleQuote | LexState::BracketIdent | LexState::BacktickIdent => {
            return Err(DbError::InvalidQuery(
                "unterminated SQL quoted identifier".to_owned(),
            ));
        }
        LexState::BlockComment => {
            return Err(DbError::InvalidQuery(
                "unterminated SQL block comment".to_owned(),
            ));
        }
        LexState::Normal | LexState::LineComment => {}
    }

    let leading = first_keyword
        .ok_or_else(|| DbError::InvalidQuery("SQL statement must not be empty".to_owned()))?;
    match leading.as_str() {
        "BEGIN" | "COMMIT" | "END" | "ROLLBACK" | "SAVEPOINT" | "RELEASE" | "ATTACH" | "DETACH"
        | "VACUUM" | "PRAGMA" | "REINDEX" => Err(DbError::InvalidQuery(format!(
            "statement `{leading}` must use host transaction/capability APIs"
        ))),
        "SELECT" | "VALUES" | "EXPLAIN" => Ok(SqlStatementKind::Read),
        "WITH" => {
            let verb = cte_main_verb.as_deref().ok_or_else(|| {
                DbError::InvalidQuery("WITH statement is missing main SQL clause".to_owned())
            })?;
            match verb {
                "SELECT" | "VALUES" => Ok(SqlStatementKind::Read),
                "INSERT" | "REPLACE" => Ok(SqlStatementKind::Write {
                    is_insert: true,
                    has_returning,
                }),
                "UPDATE" | "DELETE" => Ok(SqlStatementKind::Write {
                    is_insert: false,
                    has_returning,
                }),
                other => Err(DbError::CapabilityDenied(format!(
                    "SQL statement `{other}` is not permitted"
                ))),
            }
        }
        "INSERT" | "REPLACE" => Ok(SqlStatementKind::Write {
            is_insert: true,
            has_returning,
        }),
        "UPDATE" | "DELETE" => Ok(SqlStatementKind::Write {
            is_insert: false,
            has_returning,
        }),
        "CREATE" | "ALTER" | "DROP" => Ok(SqlStatementKind::Ddl),
        other => Err(DbError::CapabilityDenied(format!(
            "SQL statement `{other}` is not permitted"
        ))),
    }
}

fn apply_busy_timeout(conn: &Connection, timeout: Duration) -> Result<(), DbError> {
    let total_ms = timeout
        .as_secs()
        .checked_mul(1000)
        .and_then(|ms| ms.checked_add(u64::from(timeout.subsec_millis())))
        .ok_or_else(|| {
            DbError::InvalidQuery(
                "query_timeout duration exceeds SQLite i32 millisecond limit".to_owned(),
            )
        })?;
    if i32::try_from(total_ms).is_err() {
        return Err(DbError::InvalidQuery(
            "query_timeout duration exceeds SQLite i32 millisecond limit".to_owned(),
        ));
    }
    conn.busy_timeout(timeout).map_err(map_rusqlite_error)
}

struct SqliteProgressGuard<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteProgressGuard<'a> {
    fn new(conn: &'a Connection, timeout: Duration) -> Result<Self, DbError> {
        let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
            DbError::InvalidQuery("query_timeout duration exceeds host clock range".to_owned())
        })?;
        let ops = if timeout.is_zero() { 1 } else { 256 };
        conn.progress_handler(ops, Some(move || Instant::now() >= deadline));
        Ok(Self { conn })
    }
}

impl Drop for SqliteProgressGuard<'_> {
    fn drop(&mut self) {
        self.conn.progress_handler(0, None::<fn() -> bool>);
    }
}

struct SqliteInsertHookGuard<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteInsertHookGuard<'a> {
    fn new(conn: &'a Connection, slot: Arc<Mutex<Option<i64>>>) -> Self {
        conn.update_hook(Some(move |action, _db: &str, _table: &str, rowid: i64| {
            if action == Action::SQLITE_INSERT
                && let Ok(mut guard) = slot.lock()
            {
                *guard = Some(rowid);
            }
        }));
        Self { conn }
    }
}

impl Drop for SqliteInsertHookGuard<'_> {
    fn drop(&mut self) {
        self.conn.update_hook(None::<fn(Action, &str, &str, i64)>);
    }
}

struct BoundedJsonWriter {
    buf: String,
    limit: usize,
    max_content_len: usize,
}

impl BoundedJsonWriter {
    fn new(limit: usize) -> Result<Self, DbError> {
        if limit < 2 {
            return Err(DbError::InvalidQuery(format!(
                "SQL query result exceeded max_result_bytes limit ({limit})"
            )));
        }
        let mut buf = String::with_capacity(limit.min(1024));
        buf.push('[');
        Ok(Self {
            buf,
            limit,
            max_content_len: limit - 1,
        })
    }

    fn remaining(&self) -> usize {
        self.max_content_len.saturating_sub(self.buf.len())
    }

    fn budget_err(&self) -> DbError {
        DbError::InvalidQuery(format!(
            "SQL query result exceeded max_result_bytes limit ({})",
            self.limit
        ))
    }

    fn push_byte(&mut self, byte: u8) -> Result<(), DbError> {
        if self.buf.len() >= self.max_content_len {
            return Err(self.budget_err());
        }
        self.buf.push(byte as char);
        Ok(())
    }

    fn push_str(&mut self, text: &str) -> Result<(), DbError> {
        if text.len() > self.remaining() {
            return Err(self.budget_err());
        }
        self.buf.push_str(text);
        Ok(())
    }

    fn push_json_string(&mut self, bytes: &[u8]) -> Result<(), DbError> {
        if self.remaining() < 2 || bytes.len() > self.remaining() - 2 {
            return Err(self.budget_err());
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|err| DbError::DecodeError(format!("non-UTF-8 SQLite text: {err}")))?;
        self.push_byte(b'"')?;
        let mut chunk_start = 0usize;
        for (idx, ch) in text.char_indices() {
            let escaped: Option<&str> = match ch {
                '"' => Some("\\\""),
                '\\' => Some("\\\\"),
                '\x08' => Some("\\b"),
                '\x0c' => Some("\\f"),
                '\n' => Some("\\n"),
                '\r' => Some("\\r"),
                '\t' => Some("\\t"),
                c if (c as u32) < 0x20 => None,
                _ => continue,
            };
            if idx > chunk_start {
                self.push_str(&text[chunk_start..idx])?;
            }
            if let Some(esc) = escaped {
                self.push_str(esc)?;
            } else {
                let code = ch as u32;
                let hex = [
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    hex_digit((code >> 4) & 0xf),
                    hex_digit(code & 0xf),
                ];
                let esc = std::str::from_utf8(&hex).expect("ASCII unicode escape");
                self.push_str(esc)?;
            }
            chunk_start = idx + ch.len_utf8();
        }
        if chunk_start < text.len() {
            self.push_str(&text[chunk_start..])?;
        }
        self.push_byte(b'"')
    }

    fn push_sql_value(&mut self, value: SqlValueRef<'_>) -> Result<(), DbError> {
        use std::fmt::Write;
        match value {
            SqlValueRef::Null => self.push_str("null"),
            SqlValueRef::Integer(number) => {
                let mut tmp = String::new();
                write!(&mut tmp, "{number}").expect("write integer to String");
                self.push_str(&tmp)
            }
            SqlValueRef::Real(number) => {
                let json_num = JsonNumber::from_f64(number).ok_or_else(|| {
                    DbError::DecodeError("non-finite float in SQLite result row".to_owned())
                })?;
                let mut tmp = String::new();
                write!(&mut tmp, "{json_num}").expect("write float to String");
                self.push_str(&tmp)
            }
            SqlValueRef::Text(bytes) => self.push_json_string(bytes),
            SqlValueRef::Blob(bytes) => {
                let min_len = 2usize.saturating_add(
                    bytes
                        .len()
                        .saturating_mul(2)
                        .saturating_sub(usize::from(!bytes.is_empty())),
                );
                if min_len > self.remaining() {
                    return Err(self.budget_err());
                }
                self.push_byte(b'[')?;
                let mut num_buf = [0u8; 3];
                for (index, &byte) in bytes.iter().enumerate() {
                    if index > 0 {
                        self.push_byte(b',')?;
                    }
                    let formatted = format_u8(byte, &mut num_buf);
                    self.push_str(formatted)?;
                }
                self.push_byte(b']')
            }
        }
    }

    fn finish(mut self) -> String {
        self.buf.push(']');
        self.buf
    }
}

fn hex_digit(nibble: u32) -> u8 {
    match nibble {
        0..=9 => b'0' + (nibble as u8),
        _ => b'a' + ((nibble - 10) as u8),
    }
}

fn format_u8(mut value: u8, buf: &mut [u8; 3]) -> &str {
    let mut pos = 3;
    loop {
        pos -= 1;
        buf[pos] = b'0' + (value % 10);
        value /= 10;
        if value == 0 {
            break;
        }
    }
    std::str::from_utf8(&buf[pos..]).expect("ASCII decimal digits")
}

/// SQLite-backed implementation of [`SqlBackend`].
pub struct SqliteSqlBackend {
    conn: Connection,
    next_tx_id: u64,
    open_txs: Vec<u64>,
}

impl SqliteSqlBackend {
    pub fn in_memory() -> Result<Self, DbError> {
        let conn = Connection::open_in_memory().map_err(map_rusqlite_error)?;
        Ok(Self {
            conn,
            next_tx_id: 1,
            open_txs: Vec::new(),
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let conn = Connection::open(path).map_err(map_rusqlite_error)?;
        Ok(Self {
            conn,
            next_tx_id: 1,
            open_txs: Vec::new(),
        })
    }

    fn bind_params(params: &[DbParam]) -> Result<Vec<SqlValue>, DbError> {
        params
            .iter()
            .map(|param| match param {
                DbParam::NullValue => Ok(SqlValue::Null),
                DbParam::BoolValue(value) => Ok(SqlValue::Integer(i64::from(*value))),
                DbParam::IntValue(value) => Ok(SqlValue::Integer(*value)),
                DbParam::FloatValue(value) => {
                    if value.is_finite() {
                        Ok(SqlValue::Real(*value))
                    } else {
                        Err(DbError::InvalidQuery(
                            "SQL float parameters must be finite numbers".to_owned(),
                        ))
                    }
                }
                DbParam::TextValue(value) => Ok(SqlValue::Text(value.clone())),
                DbParam::BytesValue(value) => Ok(SqlValue::Blob(value.clone())),
            })
            .collect()
    }

    fn reconcile_tx_state(&mut self) {
        if self.conn.is_autocommit() {
            self.open_txs.clear();
        }
    }

    fn run_query(
        &mut self,
        config: &DbConfig,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbQueryResult, DbError> {
        config.check_sql_query(sql)?;
        apply_busy_timeout(&self.conn, config.query_timeout)?;
        let bound = Self::bind_params(params)?;
        if config.max_result_bytes < 2 {
            return Err(DbError::InvalidQuery(format!(
                "SQL query result exceeded max_result_bytes limit ({})",
                config.max_result_bytes
            )));
        }

        let (is_readonly, columns) = {
            let stmt = self.conn.prepare(sql).map_err(map_rusqlite_error)?;
            let is_readonly = stmt.readonly();
            if is_readonly {
                if !config.allow_sql_read {
                    return Err(DbError::CapabilityDenied(
                        "SQL read capability is denied".to_owned(),
                    ));
                }
            } else if !config.allow_sql_write || !config.allow_sql_read {
                return Err(DbError::CapabilityDenied(
                    "SQL write and read capabilities are required".to_owned(),
                ));
            }
            let columns = stmt
                .column_names()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            let mut seen = BTreeSet::new();
            for name in &columns {
                if !seen.insert(name.as_str()) {
                    return Err(DbError::InvalidQuery(format!(
                        "SQL query returned duplicate column name {name:?}; use distinct AS aliases"
                    )));
                }
            }
            (is_readonly, columns)
        };

        if !is_readonly {
            self.conn
                .execute_batch("SAVEPOINT calcit_internal_returning")
                .map_err(|err| {
                    DbError::TxError(format!("begin internal RETURNING savepoint: {err}"))
                })?;
        }

        let query_result: Result<DbQueryResult, DbError> = (|| {
            let _progress = SqliteProgressGuard::new(&self.conn, config.query_timeout)?;
            let mut stmt = self.conn.prepare(sql).map_err(map_rusqlite_error)?;
            let mut rows = stmt
                .query(params_from_iter(bound))
                .map_err(map_rusqlite_error)?;

            let mut writer = BoundedJsonWriter::new(config.max_result_bytes)?;
            let mut row_count = 0usize;
            while let Some(row) = rows.next().map_err(map_rusqlite_error)? {
                row_count += 1;
                if row_count > config.max_rows {
                    return Err(DbError::InvalidQuery(format!(
                        "SQL query result exceeded max_rows limit ({})",
                        config.max_rows
                    )));
                }
                if row_count > 1 {
                    writer.push_byte(b',')?;
                }
                writer.push_byte(b'{')?;
                for (index, name) in columns.iter().enumerate() {
                    if index > 0 {
                        writer.push_byte(b',')?;
                    }
                    writer.push_json_string(name.as_bytes())?;
                    writer.push_byte(b':')?;
                    let raw = row.get_ref(index).map_err(map_rusqlite_error)?;
                    writer.push_sql_value(raw)?;
                }
                writer.push_byte(b'}')?;
            }

            Ok(DbQueryResult {
                columns,
                rows_json: writer.finish(),
            })
        })();

        match query_result {
            Ok(result) => {
                if !is_readonly {
                    self.conn
                        .execute_batch("RELEASE SAVEPOINT calcit_internal_returning")
                        .map_err(|err| {
                            self.reconcile_tx_state();
                            DbError::TxError(format!("commit internal RETURNING savepoint: {err}"))
                        })?;
                }
                Ok(result)
            }
            Err(err) => {
                self.reconcile_tx_state();
                if !is_readonly && !self.conn.is_autocommit() {
                    let _ = self.conn.execute_batch(
                        "ROLLBACK TO SAVEPOINT calcit_internal_returning; RELEASE SAVEPOINT calcit_internal_returning;",
                    );
                    self.reconcile_tx_state();
                }
                Err(err)
            }
        }
    }

    fn run_exec(
        &mut self,
        config: &DbConfig,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbExecResult, DbError> {
        let kind = inspect_sql_statement(sql)?;
        config.check_sql_exec(sql)?;
        apply_busy_timeout(&self.conn, config.query_timeout)?;
        let bound = Self::bind_params(params)?;
        let is_insert = matches!(
            kind,
            SqlStatementKind::Write {
                is_insert: true,
                ..
            }
        );
        let inserted_rowid = Arc::new(Mutex::new(None::<i64>));
        let exec_res = {
            let _progress = SqliteProgressGuard::new(&self.conn, config.query_timeout)?;
            let _insert_hook = if is_insert {
                Some(SqliteInsertHookGuard::new(
                    &self.conn,
                    Arc::clone(&inserted_rowid),
                ))
            } else {
                None
            };
            let mut stmt = self.conn.prepare(sql).map_err(map_rusqlite_error)?;
            if stmt.readonly() {
                return Err(DbError::InvalidQuery(
                    "exec expects a write or DDL SQL statement".to_owned(),
                ));
            }
            stmt.execute(params_from_iter(bound))
        };
        let raw_changes = match exec_res {
            Ok(count) => count as u64,
            Err(err) => {
                self.reconcile_tx_state();
                return Err(map_rusqlite_error(err));
            }
        };
        let rows_affected = if matches!(kind, SqlStatementKind::Ddl) {
            0
        } else {
            raw_changes
        };
        let last_insert_id = if is_insert && rows_affected > 0 {
            *inserted_rowid.lock().unwrap_or_else(|e| e.into_inner())
        } else {
            None
        };
        Ok(DbExecResult {
            last_insert_id,
            rows_affected,
        })
    }

    fn require_active_tx(&self, tx_id: u64) -> Result<(), DbError> {
        match self.open_txs.last().copied() {
            Some(current) if current == tx_id => Ok(()),
            Some(current) => Err(DbError::TxError(format!(
                "transaction {tx_id} is not the active innermost transaction ({current})"
            ))),
            None => Err(DbError::TxError(format!("transaction {tx_id} is not open"))),
        }
    }
}

impl SqlBackend for SqliteSqlBackend {
    fn query(
        &mut self,
        config: &DbConfig,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbQueryResult, DbError> {
        self.run_query(config, sql, params)
    }

    fn exec(
        &mut self,
        config: &DbConfig,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbExecResult, DbError> {
        self.run_exec(config, sql, params)
    }

    fn tx_begin(&mut self, config: &DbConfig) -> Result<u64, DbError> {
        config.check_tx_begin()?;
        if self.open_txs.len() >= config.max_open_transactions {
            return Err(DbError::TxError(format!(
                "exceeded max_open_transactions ({})",
                config.max_open_transactions
            )));
        }
        apply_busy_timeout(&self.conn, config.query_timeout)?;
        let tx_id = self.next_tx_id;
        self.next_tx_id += 1;
        self.conn
            .execute_batch(&format!("SAVEPOINT calcit_tx_{tx_id}"))
            .map_err(|err| DbError::TxError(format!("begin savepoint {tx_id}: {err}")))?;
        self.open_txs.push(tx_id);
        Ok(tx_id)
    }

    fn tx_query(
        &mut self,
        config: &DbConfig,
        tx_id: u64,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbQueryResult, DbError> {
        config.check_tx_lifecycle()?;
        self.reconcile_tx_state();
        self.require_active_tx(tx_id)?;
        self.run_query(config, sql, params)
    }

    fn tx_exec(
        &mut self,
        config: &DbConfig,
        tx_id: u64,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbExecResult, DbError> {
        config.check_tx_lifecycle()?;
        self.reconcile_tx_state();
        self.require_active_tx(tx_id)?;
        self.run_exec(config, sql, params)
    }

    fn tx_commit(&mut self, tx_id: u64) -> Result<(), DbError> {
        self.reconcile_tx_state();
        self.require_active_tx(tx_id)?;
        if let Err(err) = self
            .conn
            .execute_batch(&format!("RELEASE SAVEPOINT calcit_tx_{tx_id}"))
        {
            self.reconcile_tx_state();
            return Err(DbError::TxError(format!("commit savepoint {tx_id}: {err}")));
        }
        self.open_txs.pop();
        Ok(())
    }

    fn tx_rollback(&mut self, tx_id: u64) -> Result<(), DbError> {
        self.reconcile_tx_state();
        self.require_active_tx(tx_id)?;
        if let Err(err) = self.conn.execute_batch(&format!(
            "ROLLBACK TO SAVEPOINT calcit_tx_{tx_id}; RELEASE SAVEPOINT calcit_tx_{tx_id};"
        )) {
            self.reconcile_tx_state();
            return Err(DbError::TxError(format!(
                "rollback savepoint {tx_id}: {err}"
            )));
        }
        self.open_txs.pop();
        Ok(())
    }

    fn rollback_all_open_transactions(&mut self) -> Result<usize, DbError> {
        self.reconcile_tx_state();
        let mut rolled_back = 0usize;
        while let Some(&tx_id) = self.open_txs.last() {
            if let Err(err) = self.conn.execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT calcit_tx_{tx_id}; RELEASE SAVEPOINT calcit_tx_{tx_id};"
            )) {
                self.reconcile_tx_state();
                return Err(DbError::TxError(format!(
                    "cleanup savepoint {tx_id}: {err}"
                )));
            }
            self.open_txs.pop();
            rolled_back += 1;
        }
        Ok(rolled_back)
    }
}

fn map_rusqlite_error(error: RusqliteError) -> DbError {
    match &error {
        RusqliteError::SqliteFailure(ffi_err, _) => match ffi_err.code {
            ErrorCode::ConstraintViolation => DbError::ConstraintViolation(error.to_string()),
            ErrorCode::DatabaseBusy
            | ErrorCode::DatabaseLocked
            | ErrorCode::OperationInterrupted => DbError::Timeout(error.to_string()),
            ErrorCode::Unknown | ErrorCode::ApiMisuse | ErrorCode::TypeMismatch => {
                DbError::InvalidQuery(error.to_string())
            }
            _ => DbError::Backend(error.to_string()),
        },
        RusqliteError::SqlInputError { .. } | RusqliteError::InvalidParameterName(_) => {
            DbError::InvalidQuery(error.to_string())
        }
        _ => DbError::Backend(error.to_string()),
    }
}

#[derive(Clone, Debug)]
struct KvEntry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
}

/// Default bounded in-memory implementation of [`KvBackend`].
pub struct InMemoryKvBackend {
    entries: BTreeMap<String, KvEntry>,
    total_bytes: usize,
    clock: Arc<dyn Fn() -> Instant + Send + Sync>,
}

impl Default for InMemoryKvBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryKvBackend {
    pub fn new() -> Self {
        Self::with_clock(Arc::new(Instant::now))
    }

    pub fn with_clock(clock: Arc<dyn Fn() -> Instant + Send + Sync>) -> Self {
        Self {
            entries: BTreeMap::new(),
            total_bytes: 0,
            clock,
        }
    }

    fn entry_size(key: &str, value: &[u8]) -> usize {
        key.len().saturating_add(value.len())
    }

    fn purge_expired(&mut self, now: Instant) {
        let expired_keys = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.expires_at.is_some_and(|deadline| now >= deadline))
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in expired_keys {
            if let Some(removed) = self.entries.remove(&key) {
                self.total_bytes = self
                    .total_bytes
                    .saturating_sub(Self::entry_size(&key, &removed.value));
            }
        }
    }

    fn purge_if_expired(&mut self, key: &str, now: Instant) {
        if let Some(entry) = self.entries.get(key)
            && entry.expires_at.is_some_and(|deadline| now >= deadline)
            && let Some(removed) = self.entries.remove(key)
        {
            self.total_bytes = self
                .total_bytes
                .saturating_sub(Self::entry_size(key, &removed.value));
        }
    }
}

impl KvBackend for InMemoryKvBackend {
    fn get(&mut self, config: &DbConfig, key: &str) -> Result<Option<Vec<u8>>, DbError> {
        config.check_kv_key(key)?;
        let now = (self.clock)();
        self.purge_if_expired(key, now);
        Ok(self.entries.get(key).map(|entry| entry.value.clone()))
    }

    fn set(
        &mut self,
        config: &DbConfig,
        key: &str,
        value: &[u8],
        ttl_ms: Option<u64>,
    ) -> Result<(), DbError> {
        config.check_kv_set(key, value)?;
        let now = (self.clock)();
        let expires_at = match ttl_ms {
            Some(ms) => Some(now.checked_add(Duration::from_millis(ms)).ok_or_else(|| {
                DbError::InvalidQuery("KV ttl_ms exceeds host clock range".to_owned())
            })?),
            None => None,
        };
        self.purge_expired(now);
        let existing_bytes = self
            .entries
            .get(key)
            .map_or(0, |entry| Self::entry_size(key, &entry.value));
        let new_bytes = Self::entry_size(key, value);
        let next_total = self
            .total_bytes
            .saturating_sub(existing_bytes)
            .checked_add(new_bytes);
        if next_total.is_none_or(|total| total > config.max_kv_total_bytes) {
            return Err(DbError::InvalidQuery(format!(
                "KV store total size would exceed max_kv_total_bytes ({})",
                config.max_kv_total_bytes
            )));
        }
        self.entries.insert(
            key.to_owned(),
            KvEntry {
                value: value.to_vec(),
                expires_at,
            },
        );
        self.total_bytes = next_total.expect("checked next_total");
        Ok(())
    }

    fn delete(&mut self, config: &DbConfig, key: &str) -> Result<bool, DbError> {
        config.check_kv_key(key)?;
        let now = (self.clock)();
        self.purge_if_expired(key, now);
        Ok(match self.entries.remove(key) {
            Some(removed) => {
                self.total_bytes = self
                    .total_bytes
                    .saturating_sub(Self::entry_size(key, &removed.value));
                true
            }
            None => false,
        })
    }
}

/// Host store state owning the database capability policy and pluggable backends.
pub struct WasmtimeDbState {
    config: DbConfig,
    sql: Box<dyn SqlBackend>,
    kv: Box<dyn KvBackend>,
}

impl WasmtimeDbState {
    pub fn new(config: DbConfig, sql: Box<dyn SqlBackend>, kv: Box<dyn KvBackend>) -> Self {
        Self { config, sql, kv }
    }

    pub fn with_sqlite_in_memory(config: DbConfig) -> Result<Self, DbError> {
        let sql = SqliteSqlBackend::in_memory()?;
        Ok(Self::new(
            config,
            Box::new(sql),
            Box::new(InMemoryKvBackend::new()),
        ))
    }

    pub fn rollback_all_open_transactions(&mut self) -> Result<usize, DbError> {
        self.sql.rollback_all_open_transactions()
    }
}

/// Register `calcit:db/sql@0.1.0` and `calcit:db/kv@0.1.0` into a Wasmtime component linker.
pub fn add_to_linker<T: 'static>(
    linker: &mut Linker<T>,
    project: impl Fn(&mut T) -> &mut WasmtimeDbState + Copy + Send + Sync + 'static,
) -> wasmtime::Result<()> {
    {
        let mut sql = linker.instance(DB_SQL_INSTANCE)?;
        sql.func_wrap(
            "query",
            move |mut caller: wasmtime::StoreContextMut<'_, T>,
                  (query_sql, params): (String, Vec<DbParam>)| {
                let state = project(caller.data_mut());
                let res = state
                    .config
                    .check_sql_query(&query_sql)
                    .and_then(|()| state.sql.query(&state.config, &query_sql, &params));
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "exec",
            move |mut caller: wasmtime::StoreContextMut<'_, T>,
                  (exec_sql, params): (String, Vec<DbParam>)| {
                let state = project(caller.data_mut());
                let res = state
                    .config
                    .check_sql_exec(&exec_sql)
                    .and_then(|()| state.sql.exec(&state.config, &exec_sql, &params));
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-begin",
            move |mut caller: wasmtime::StoreContextMut<'_, T>, (): ()| {
                let state = project(caller.data_mut());
                let res = state
                    .config
                    .check_tx_begin()
                    .and_then(|()| state.sql.tx_begin(&state.config));
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-query",
            move |mut caller: wasmtime::StoreContextMut<'_, T>,
                  (tx_id, query_sql, params): (u64, String, Vec<DbParam>)| {
                let state = project(caller.data_mut());
                let res = state.config.check_sql_query(&query_sql).and_then(|()| {
                    state
                        .sql
                        .tx_query(&state.config, tx_id, &query_sql, &params)
                });
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-exec",
            move |mut caller: wasmtime::StoreContextMut<'_, T>,
                  (tx_id, exec_sql, params): (u64, String, Vec<DbParam>)| {
                let state = project(caller.data_mut());
                let res = state
                    .config
                    .check_sql_exec(&exec_sql)
                    .and_then(|()| state.sql.tx_exec(&state.config, tx_id, &exec_sql, &params));
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-commit",
            move |mut caller: wasmtime::StoreContextMut<'_, T>, (tx_id,): (u64,)| {
                let state = project(caller.data_mut());
                let res = state
                    .config
                    .check_tx_lifecycle()
                    .and_then(|()| state.sql.tx_commit(tx_id));
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-rollback",
            move |mut caller: wasmtime::StoreContextMut<'_, T>, (tx_id,): (u64,)| {
                let state = project(caller.data_mut());
                let res = state
                    .config
                    .check_tx_lifecycle()
                    .and_then(|()| state.sql.tx_rollback(tx_id));
                Ok((res,))
            },
        )?;
    }

    {
        let mut kv = linker.instance(DB_KV_INSTANCE)?;
        kv.func_wrap(
            "get",
            move |mut caller: wasmtime::StoreContextMut<'_, T>, (key,): (String,)| {
                let state = project(caller.data_mut());
                let res = state
                    .config
                    .check_kv_key(&key)
                    .and_then(|()| state.kv.get(&state.config, &key));
                Ok((res,))
            },
        )?;
        kv.func_wrap(
            "set",
            move |mut caller: wasmtime::StoreContextMut<'_, T>,
                  (key, value, ttl_ms): (String, Vec<u8>, Option<u64>)| {
                let state = project(caller.data_mut());
                let res = state
                    .config
                    .check_kv_set(&key, &value)
                    .and_then(|()| state.kv.set(&state.config, &key, &value, ttl_ms));
                Ok((res,))
            },
        )?;
        kv.func_wrap(
            "delete",
            move |mut caller: wasmtime::StoreContextMut<'_, T>, (key,): (String,)| {
                let state = project(caller.data_mut());
                let res = state
                    .config
                    .check_kv_key(&key)
                    .and_then(|()| state.kv.delete(&state.config, &key));
                Ok((res,))
            },
        )?;
    }

    Ok(())
}

struct PeakMemoryLimiter {
    max_memory_bytes: usize,
    peak_memory_bytes: usize,
}

impl PeakMemoryLimiter {
    fn new(max_memory_bytes: usize) -> Self {
        Self {
            max_memory_bytes,
            peak_memory_bytes: 0,
        }
    }
}

impl ResourceLimiter for PeakMemoryLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let cap = maximum.map_or(self.max_memory_bytes, |m| m.min(self.max_memory_bytes));
        let next = current.max(desired);
        if desired > cap {
            return Ok(false);
        }
        self.peak_memory_bytes = self.peak_memory_bytes.max(next);
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(maximum.is_none_or(|m| desired <= m))
    }
}

struct ServiceHostData {
    wasi: WasiCtx,
    http: WasiHttpCtx,
    table: ResourceTable,
    db: WasmtimeDbState,
    limiter: PeakMemoryLimiter,
}

impl WasiView for ServiceHostData {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for ServiceHostData {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.http,
            table: &mut self.table,
            hooks: Default::default(),
        }
    }
}

/// Response and resource-footprint summary from one [`HttpDbServiceRunner`] turn.
#[derive(Clone, Debug)]
pub struct ServiceHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, Vec<u8>)>,
    pub body: Vec<u8>,
    pub peak_memory_bytes: usize,
    pub leaked_tx_rolled_back: usize,
}

/// Reusable async Wasmtime runner binding `wasi:http/proxy@0.2.12` and `calcit:db@0.1.0`
/// under configurable linear memory and HTTP response body ceilings.
pub struct HttpDbServiceRunner {
    store: Store<ServiceHostData>,
    proxy: wasmtime_wasi_http::p2::bindings::Proxy,
    max_response_body_bytes: usize,
    turn_in_progress: bool,
    store_poisoned: bool,
}

impl Drop for HttpDbServiceRunner {
    fn drop(&mut self) {
        let _ = self.store.data_mut().db.rollback_all_open_transactions();
    }
}

impl HttpDbServiceRunner {
    pub async fn new(
        component_bytes: &[u8],
        db: WasmtimeDbState,
        max_memory_bytes: Option<usize>,
    ) -> Result<Self, String> {
        let config = Config::new();
        let engine =
            Engine::new(&config).map_err(|err| format!("create Wasmtime engine: {err}"))?;
        let component = Component::new(&engine, component_bytes)
            .map_err(|err| format!("compile service Component: {err}"))?;

        let mut linker = Linker::<ServiceHostData>::new(&engine);
        wasmtime_wasi_http::p2::add_to_linker_async(&mut linker)
            .map_err(|err| format!("register wasi:http interfaces: {err}"))?;
        add_to_linker(&mut linker, |data| &mut data.db)
            .map_err(|err| format!("register calcit:db interfaces: {err}"))?;

        let instance_pre = linker
            .instantiate_pre(&component)
            .map_err(|err| format!("pre-instantiate HTTP+DB Component: {err}"))?;
        let proxy_pre = ProxyPre::new(instance_pre)
            .map_err(|err| format!("bind wasi:http/proxy world: {err}"))?;

        let host_data = ServiceHostData {
            wasi: WasiCtxBuilder::new().build(),
            http: WasiHttpCtx::new(),
            table: ResourceTable::new(),
            db,
            limiter: PeakMemoryLimiter::new(max_memory_bytes.unwrap_or(DEFAULT_MEMORY_LIMIT_BYTES)),
        };
        let mut store = Store::new(&engine, host_data);
        store.limiter(|data| &mut data.limiter);
        let proxy = proxy_pre
            .instantiate_async(&mut store)
            .await
            .map_err(|err| format!("instantiate HTTP+DB Component: {err}"))?;

        Ok(Self {
            store,
            proxy,
            max_response_body_bytes: DEFAULT_MAX_RESPONSE_BODY_BYTES,
            turn_in_progress: false,
            store_poisoned: false,
        })
    }

    pub fn with_max_response_body_bytes(mut self, limit: usize) -> Self {
        self.max_response_body_bytes = limit;
        self
    }

    pub fn peak_memory_bytes(&self) -> usize {
        self.store.data().limiter.peak_memory_bytes
    }

    pub fn rollback_open_transactions(&mut self) -> Result<usize, String> {
        self.store
            .data_mut()
            .db
            .rollback_all_open_transactions()
            .map_err(|err| format!("rollback open transactions: {err:?}"))
    }

    pub async fn handle_request(
        &mut self,
        method: http::Method,
        path_and_query: &str,
        headers: Vec<(String, Vec<u8>)>,
        body: Vec<u8>,
    ) -> Result<ServiceHttpResponse, String> {
        if self.turn_in_progress {
            let cleanup_err = self
                .store
                .data_mut()
                .db
                .rollback_all_open_transactions()
                .err();
            self.turn_in_progress = false;
            self.store_poisoned = true;
            return match cleanup_err {
                Some(err) => Err(format!(
                    "previous HTTP turn was cancelled before completion; rollback failed: {err:?}"
                )),
                None => Err("previous HTTP turn was cancelled before completion".to_owned()),
            };
        }
        if self.store_poisoned {
            return Err(
                "HTTP service runner cannot be reused after a trap or cancelled turn".to_owned(),
            );
        }

        let uri: http::Uri = path_and_query
            .parse()
            .map_err(|err| format!("invalid request URI {path_and_query:?}: {err}"))?;
        let uri_has_authority = uri.authority().is_some();
        let mut builder = http::Request::builder().method(method).uri(uri);
        let mut has_host = false;
        for (name, value) in headers {
            if name.eq_ignore_ascii_case("host") {
                has_host = true;
            }
            let header_value = http::HeaderValue::from_bytes(&value)
                .map_err(|err| format!("invalid request header {name:?}: {err}"))?;
            builder = builder.header(name, header_value);
        }
        if !uri_has_authority && !has_host {
            builder = builder.header("host", "localhost");
        }
        let body_stream = Full::new(Bytes::from(body)).map_err(|never: Infallible| match never {});
        let request = builder
            .body(body_stream)
            .map_err(|err| format!("build HTTP request: {err}"))?;

        let req_resource = self
            .store
            .data_mut()
            .http()
            .new_incoming_request(Scheme::Http, request)
            .map_err(|err| format!("allocate incoming-request resource: {err}"))?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let out_resource = self
            .store
            .data_mut()
            .http()
            .new_response_outparam(tx)
            .map_err(|err| format!("allocate response-outparam resource: {err}"))?;

        self.turn_in_progress = true;
        let turn_result = drive_http_turn(
            &self.proxy,
            &mut self.store,
            req_resource,
            out_resource,
            rx,
            self.max_response_body_bytes,
        )
        .await;
        let cleanup_res = self.store.data_mut().db.rollback_all_open_transactions();
        self.turn_in_progress = false;

        let leaked_tx_rolled_back = cleanup_res.map_err(|err| {
            self.store_poisoned = true;
            format!("rollback open transactions after HTTP turn: {err:?}")
        })?;

        match turn_result {
            Ok((status, headers, body)) => Ok(ServiceHttpResponse {
                status,
                headers,
                body,
                peak_memory_bytes: self.peak_memory_bytes(),
                leaked_tx_rolled_back,
            }),
            Err((poison, err)) => {
                if poison {
                    self.store_poisoned = true;
                }
                Err(err)
            }
        }
    }
}

async fn drive_http_turn(
    proxy: &wasmtime_wasi_http::p2::bindings::Proxy,
    store: &mut Store<ServiceHostData>,
    req_resource: wasmtime::component::Resource<wasmtime_wasi_http::p2::types::HostIncomingRequest>,
    out_resource: wasmtime::component::Resource<
        wasmtime_wasi_http::p2::types::HostResponseOutparam,
    >,
    rx: tokio::sync::oneshot::Receiver<
        Result<
            http::Response<wasmtime_wasi_http::p2::body::HyperOutgoingBody>,
            wasmtime_wasi_http::p2::bindings::http::types::ErrorCode,
        >,
    >,
    max_response_body_bytes: usize,
) -> Result<(u16, Vec<(String, Vec<u8>)>, Vec<u8>), (bool, String)> {
    let mut call_fut = std::pin::pin!(proxy.wasi_http_incoming_handler().call_handle(
        store,
        req_resource,
        out_resource
    ));
    let mut rx_fut = std::pin::pin!(rx);

    let received = tokio::select! {
        call_res = &mut call_fut => {
            call_res.map_err(|err| (true, format!("incoming-handler trap/error: {err}")))?;
            match rx_fut.try_recv() {
                Ok(Ok(response)) => {
                    return drain_hyper_response(response, max_response_body_bytes)
                        .await
                        .map_err(|err| (false, err));
                }
                Ok(Err(err_code)) => {
                    return Err((false, format!("guest returned wasi:http error-code: {err_code:?}")));
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    return Err((false, "guest dropped response-outparam without sending a response".to_owned()));
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    return Err((true, "guest returned without setting response-outparam".to_owned()));
                }
            }
        }
        head = &mut rx_fut => {
            match head {
                Err(_) => {
                    call_fut
                        .await
                        .map_err(|err| (true, format!("incoming-handler trap/error: {err}")))?;
                    return Err((false, "guest dropped response-outparam without sending a response".to_owned()));
                }
                Ok(Err(err_code)) => {
                    call_fut
                        .await
                        .map_err(|err| (true, format!("incoming-handler trap/error: {err}")))?;
                    return Err((false, format!("guest returned wasi:http error-code: {err_code:?}")));
                }
                Ok(Ok(response)) => response,
            }
        }
    };

    let status = received.status().as_u16();
    let headers = received
        .headers()
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
        .collect::<Vec<_>>();
    let mut body_fut = std::pin::pin!(drain_bounded_body(
        received.into_body(),
        max_response_body_bytes
    ));
    let body = tokio::select! {
        body_res = &mut body_fut => match body_res {
            Ok(bytes) => {
                call_fut
                    .await
                    .map_err(|err| (true, format!("incoming-handler trap/error: {err}")))?;
                bytes
            }
            Err(limit_err) => {
                return Err((true, limit_err));
            }
        },
        call_res = &mut call_fut => {
            call_res.map_err(|err| (true, format!("incoming-handler trap/error: {err}")))?;
            body_fut.await.map_err(|err| (true, err))?
        }
    };

    Ok((status, headers, body))
}

async fn drain_hyper_response(
    received: http::Response<wasmtime_wasi_http::p2::body::HyperOutgoingBody>,
    max_response_body_bytes: usize,
) -> Result<(u16, Vec<(String, Vec<u8>)>, Vec<u8>), String> {
    let status = received.status().as_u16();
    let headers = received
        .headers()
        .iter()
        .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
        .collect::<Vec<_>>();
    let body = drain_bounded_body(received.into_body(), max_response_body_bytes).await?;
    Ok((status, headers, body))
}

async fn drain_bounded_body(
    mut body: wasmtime_wasi_http::p2::body::HyperOutgoingBody,
    max_response_body_bytes: usize,
) -> Result<Vec<u8>, String> {
    let mut collected = Vec::new();
    while let Some(frame_res) = body.frame().await {
        let frame = frame_res.map_err(|err| format!("collect HTTP response body: {err:?}"))?;
        if let Some(chunk) = frame.data_ref() {
            let next_len = collected.len().checked_add(chunk.len()).ok_or_else(|| {
                format!("HTTP response body exceeded limit ({max_response_body_bytes} bytes)")
            })?;
            if next_len > max_response_body_bytes {
                return Err(format!(
                    "HTTP response body exceeded limit ({next_len} > {max_response_body_bytes} bytes)"
                ));
            }
            collected.extend_from_slice(chunk);
        }
    }
    Ok(collected)
}
