//! Bounded default-deny Wasmtime SQLite/KV host capability and HTTP+DB service runner
//! for `calcit:db@0.1.0` (`wasi:http/proxy@0.2.12` + synchronous `sql` and `kv` imports).

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use rusqlite::types::{Value as SqlValue, ValueRef as SqlValueRef};
use rusqlite::{Connection, Error as RusqliteError, ErrorCode, params_from_iter};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
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
const DEFAULT_MEMORY_LIMIT_BYTES: usize = 8 * 1024 * 1024;

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

    pub fn query_timeout(mut self, timeout: Duration) -> Self {
        self.query_timeout = timeout;
        self
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

    fn check_kv_key(&self, key: &str) -> Result<(), DbError> {
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
        LineComment,
        BlockComment,
    }

    let bytes = sql.as_bytes();
    let mut state = LexState::Normal;
    let mut idx = 0usize;
    let mut paren_depth = 0usize;
    let mut saw_semicolon = false;
    let mut first_keyword: Option<String> = None;
    let mut top_level_dml: Option<String> = None;
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
                    b';' => {
                        saw_semicolon = true;
                        idx += 1;
                    }
                    b'(' => {
                        paren_depth += 1;
                        idx += 1;
                    }
                    b')' => {
                        paren_depth = paren_depth.saturating_sub(1);
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
                            if matches!(word.as_str(), "INSERT" | "UPDATE" | "DELETE" | "REPLACE") {
                                top_level_dml = Some(word.clone());
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
        LexState::SingleQuote | LexState::DoubleQuote => {
            return Err(DbError::InvalidQuery(
                "unterminated SQL string literal".to_owned(),
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
            if let Some(dml) = top_level_dml.as_deref() {
                Ok(SqlStatementKind::Write {
                    is_insert: matches!(dml, "INSERT" | "REPLACE"),
                    has_returning,
                })
            } else {
                Ok(SqlStatementKind::Read)
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

    fn value_ref_to_json(value: SqlValueRef<'_>) -> Result<JsonValue, DbError> {
        match value {
            SqlValueRef::Null => Ok(JsonValue::Null),
            SqlValueRef::Integer(number) => Ok(JsonValue::Number(number.into())),
            SqlValueRef::Real(number) => JsonNumber::from_f64(number)
                .map(JsonValue::Number)
                .ok_or_else(|| {
                    DbError::DecodeError("non-finite float in SQLite result row".to_owned())
                }),
            SqlValueRef::Text(bytes) => std::str::from_utf8(bytes)
                .map(|text| JsonValue::String(text.to_owned()))
                .map_err(|err| DbError::DecodeError(format!("non-UTF-8 SQLite text: {err}"))),
            SqlValueRef::Blob(bytes) => Ok(JsonValue::Array(
                bytes
                    .iter()
                    .map(|byte| JsonValue::Number((*byte).into()))
                    .collect(),
            )),
        }
    }

    fn run_query(
        &mut self,
        config: &DbConfig,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbQueryResult, DbError> {
        let kind = inspect_sql_statement(sql)?;
        config.check_sql_kind(kind)?;
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
        let _ = self.conn.busy_timeout(config.query_timeout);
        let bound = Self::bind_params(params)?;
        let mut stmt = self.conn.prepare(sql).map_err(map_rusqlite_error)?;
        if stmt.readonly() {
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
        let mut rows = stmt
            .query(params_from_iter(bound))
            .map_err(map_rusqlite_error)?;

        let mut json_bytes = String::from("[");
        let mut row_count = 0usize;
        while let Some(row) = rows.next().map_err(map_rusqlite_error)? {
            row_count += 1;
            if row_count > config.max_rows {
                return Err(DbError::InvalidQuery(format!(
                    "SQL query result exceeded max_rows limit ({})",
                    config.max_rows
                )));
            }
            let mut object = JsonMap::with_capacity(columns.len());
            for (index, name) in columns.iter().enumerate() {
                let raw = row.get_ref(index).map_err(map_rusqlite_error)?;
                object.insert(name.clone(), Self::value_ref_to_json(raw)?);
            }
            let row_json = serde_json::to_string(&JsonValue::Object(object))
                .map_err(|err| DbError::DecodeError(format!("serialize row JSON: {err}")))?;
            let extra = row_json.len() + usize::from(row_count > 1) + 1;
            if json_bytes.len() + extra > config.max_result_bytes {
                return Err(DbError::InvalidQuery(format!(
                    "SQL query result exceeded max_result_bytes limit ({})",
                    config.max_result_bytes
                )));
            }
            if row_count > 1 {
                json_bytes.push(',');
            }
            json_bytes.push_str(&row_json);
        }
        json_bytes.push(']');

        Ok(DbQueryResult {
            columns,
            rows_json: json_bytes,
        })
    }

    fn run_exec(
        &mut self,
        config: &DbConfig,
        sql: &str,
        params: &[DbParam],
    ) -> Result<DbExecResult, DbError> {
        let kind = inspect_sql_statement(sql)?;
        if matches!(kind, SqlStatementKind::Read) {
            return Err(DbError::InvalidQuery(
                "exec expects a write or DDL SQL statement".to_owned(),
            ));
        }
        config.check_sql_kind(kind)?;
        let _ = self.conn.busy_timeout(config.query_timeout);
        let bound = Self::bind_params(params)?;
        let rows_affected = {
            let mut stmt = self.conn.prepare(sql).map_err(map_rusqlite_error)?;
            if stmt.readonly() {
                return Err(DbError::InvalidQuery(
                    "exec expects a write or DDL SQL statement".to_owned(),
                ));
            }
            stmt.execute(params_from_iter(bound))
                .map_err(map_rusqlite_error)? as u64
        };
        let last_insert_id = (matches!(
            kind,
            SqlStatementKind::Write {
                is_insert: true,
                ..
            }
        ) && rows_affected > 0)
            .then(|| self.conn.last_insert_rowid())
            .filter(|id| *id > 0);
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
        if let Some(reason) = &config.unsupported_reason {
            return Err(DbError::Unsupported(reason.to_string()));
        }
        if !config.allow_sql_read && !config.allow_sql_write && !config.allow_sql_ddl {
            return Err(DbError::CapabilityDenied(
                "SQL capability is denied".to_owned(),
            ));
        }
        if self.open_txs.len() >= config.max_open_transactions {
            return Err(DbError::TxError(format!(
                "exceeded max_open_transactions ({})",
                config.max_open_transactions
            )));
        }
        let tx_id = self.next_tx_id;
        self.next_tx_id += 1;
        let _ = self.conn.busy_timeout(config.query_timeout);
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
        if let Some(reason) = &config.unsupported_reason {
            return Err(DbError::Unsupported(reason.to_string()));
        }
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
        if let Some(reason) = &config.unsupported_reason {
            return Err(DbError::Unsupported(reason.to_string()));
        }
        self.require_active_tx(tx_id)?;
        self.run_exec(config, sql, params)
    }

    fn tx_commit(&mut self, tx_id: u64) -> Result<(), DbError> {
        self.require_active_tx(tx_id)?;
        self.conn
            .execute_batch(&format!("RELEASE SAVEPOINT calcit_tx_{tx_id}"))
            .map_err(|err| DbError::TxError(format!("commit savepoint {tx_id}: {err}")))?;
        self.open_txs.pop();
        Ok(())
    }

    fn tx_rollback(&mut self, tx_id: u64) -> Result<(), DbError> {
        self.require_active_tx(tx_id)?;
        self.conn
            .execute_batch(&format!(
                "ROLLBACK TO SAVEPOINT calcit_tx_{tx_id}; RELEASE SAVEPOINT calcit_tx_{tx_id};"
            ))
            .map_err(|err| DbError::TxError(format!("rollback savepoint {tx_id}: {err}")))?;
        self.open_txs.pop();
        Ok(())
    }

    fn rollback_all_open_transactions(&mut self) -> Result<usize, DbError> {
        let mut rolled_back = 0usize;
        while let Some(tx_id) = self.open_txs.pop() {
            self.conn
                .execute_batch(&format!(
                    "ROLLBACK TO SAVEPOINT calcit_tx_{tx_id}; RELEASE SAVEPOINT calcit_tx_{tx_id};"
                ))
                .map_err(|err| DbError::TxError(format!("cleanup savepoint {tx_id}: {err}")))?;
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
#[derive(Default)]
pub struct InMemoryKvBackend {
    entries: BTreeMap<String, KvEntry>,
}

impl InMemoryKvBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn purge_if_expired(&mut self, key: &str, now: Instant) {
        if let Some(entry) = self.entries.get(key)
            && entry.expires_at.is_some_and(|deadline| now >= deadline)
        {
            self.entries.remove(key);
        }
    }
}

impl KvBackend for InMemoryKvBackend {
    fn get(&mut self, config: &DbConfig, key: &str) -> Result<Option<Vec<u8>>, DbError> {
        config.check_kv_key(key)?;
        self.purge_if_expired(key, Instant::now());
        Ok(self.entries.get(key).map(|entry| entry.value.clone()))
    }

    fn set(
        &mut self,
        config: &DbConfig,
        key: &str,
        value: &[u8],
        ttl_ms: Option<u64>,
    ) -> Result<(), DbError> {
        config.check_kv_key(key)?;
        if value.len() > config.max_kv_value_bytes {
            return Err(DbError::InvalidQuery(format!(
                "KV value ({} bytes) exceeds max_kv_value_bytes ({})",
                value.len(),
                config.max_kv_value_bytes
            )));
        }
        let expires_at =
            ttl_ms.and_then(|ms| Instant::now().checked_add(Duration::from_millis(ms)));
        self.entries.insert(
            key.to_owned(),
            KvEntry {
                value: value.to_vec(),
                expires_at,
            },
        );
        Ok(())
    }

    fn delete(&mut self, config: &DbConfig, key: &str) -> Result<bool, DbError> {
        config.check_kv_key(key)?;
        self.purge_if_expired(key, Instant::now());
        Ok(self.entries.remove(key).is_some())
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
                let res = state.sql.query(&state.config, &query_sql, &params);
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "exec",
            move |mut caller: wasmtime::StoreContextMut<'_, T>,
                  (exec_sql, params): (String, Vec<DbParam>)| {
                let state = project(caller.data_mut());
                let res = state.sql.exec(&state.config, &exec_sql, &params);
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-begin",
            move |mut caller: wasmtime::StoreContextMut<'_, T>, (): ()| {
                let state = project(caller.data_mut());
                let res = state.sql.tx_begin(&state.config);
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-query",
            move |mut caller: wasmtime::StoreContextMut<'_, T>,
                  (tx_id, query_sql, params): (u64, String, Vec<DbParam>)| {
                let state = project(caller.data_mut());
                let res = state
                    .sql
                    .tx_query(&state.config, tx_id, &query_sql, &params);
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-exec",
            move |mut caller: wasmtime::StoreContextMut<'_, T>,
                  (tx_id, exec_sql, params): (u64, String, Vec<DbParam>)| {
                let state = project(caller.data_mut());
                let res = state.sql.tx_exec(&state.config, tx_id, &exec_sql, &params);
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-commit",
            move |mut caller: wasmtime::StoreContextMut<'_, T>, (tx_id,): (u64,)| {
                let state = project(caller.data_mut());
                let res = state.sql.tx_commit(tx_id);
                Ok((res,))
            },
        )?;
        sql.func_wrap(
            "tx-rollback",
            move |mut caller: wasmtime::StoreContextMut<'_, T>, (tx_id,): (u64,)| {
                let state = project(caller.data_mut());
                let res = state.sql.tx_rollback(tx_id);
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
                let res = state.kv.get(&state.config, &key);
                Ok((res,))
            },
        )?;
        kv.func_wrap(
            "set",
            move |mut caller: wasmtime::StoreContextMut<'_, T>,
                  (key, value, ttl_ms): (String, Vec<u8>, Option<u64>)| {
                let state = project(caller.data_mut());
                let res = state.kv.set(&state.config, &key, &value, ttl_ms);
                Ok((res,))
            },
        )?;
        kv.func_wrap(
            "delete",
            move |mut caller: wasmtime::StoreContextMut<'_, T>, (key,): (String,)| {
                let state = project(caller.data_mut());
                let res = state.kv.delete(&state.config, &key);
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
/// under a configurable linear memory ceiling.
pub struct HttpDbServiceRunner {
    store: Store<ServiceHostData>,
    proxy: wasmtime_wasi_http::p2::bindings::Proxy,
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

        Ok(Self { store, proxy })
    }

    pub fn peak_memory_bytes(&self) -> usize {
        self.store.data().limiter.peak_memory_bytes
    }

    pub async fn handle_request(
        &mut self,
        method: http::Method,
        path_and_query: &str,
        headers: Vec<(String, Vec<u8>)>,
        body: Vec<u8>,
    ) -> Result<ServiceHttpResponse, String> {
        let uri_text = if path_and_query.starts_with('/') {
            format!("http://localhost{path_and_query}")
        } else {
            path_and_query.to_owned()
        };
        let mut builder = http::Request::builder().method(method).uri(uri_text);
        let mut has_host = false;
        for (name, value) in headers {
            if name.eq_ignore_ascii_case("host") {
                has_host = true;
            }
            let header_value = http::HeaderValue::from_bytes(&value)
                .map_err(|err| format!("invalid request header {name:?}: {err}"))?;
            builder = builder.header(name, header_value);
        }
        if !has_host {
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

        let receive_response = async {
            let received = rx
                .await
                .map_err(|_| {
                    "guest dropped response-outparam without sending a response".to_owned()
                })?
                .map_err(|err| format!("guest returned wasi:http error-code: {err:?}"))?;
            let status = received.status().as_u16();
            let headers = received
                .headers()
                .iter()
                .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
                .collect::<Vec<_>>();
            let collected = received
                .into_body()
                .collect()
                .await
                .map_err(|err| format!("collect HTTP response body: {err:?}"))?;
            Ok::<_, String>((status, headers, collected.to_bytes().to_vec()))
        };

        let (call_res, resp_res) = tokio::join!(
            self.proxy.wasi_http_incoming_handler().call_handle(
                &mut self.store,
                req_resource,
                out_resource
            ),
            receive_response
        );
        let leaked_tx_rolled_back = self
            .store
            .data_mut()
            .db
            .rollback_all_open_transactions()
            .map_err(|err| format!("rollback open transactions after HTTP turn: {err:?}"))?;

        call_res.map_err(|err| format!("incoming-handler trap/error: {err}"))?;
        let (status, headers, body) = resp_res?;
        Ok(ServiceHttpResponse {
            status,
            headers,
            body,
            peak_memory_bytes: self.peak_memory_bytes(),
            leaked_tx_rolled_back,
        })
    }
}
