# 2026-11-23 01:40 — Address PR #61 CodeRabbit review findings on `calcit:db@0.1.0` and `wasmtime-db`

- Moved `WIT_DB_FILE` in `src/generate.rs` from `wit/db.wit` to `wit-db/db.wit` so `wit-parser` loads `wit/` as a single root WIT package without package-identifier conflicts, and updated `docs/wasi-db.md` / `README.md`.
- Updated `docs/wasi-db.md` (adding `SqlBackend` import and a valid `CREATE TABLE items (id INTEGER PRIMARY KEY)` statement) and `src/wasi_db_wit/README.md` (distinguishing unused omitted function interfaces from live signature WIT type dependencies such as `calcit:db/types@0.1.0`).
- Updated `render_calcit_db_bindings` in `src/db_adapter.rs` to emit a standalone Calcit Snapshot carrying explicit structured `:schema` entries on every nominal type, `defwasm-import`, and `defmacro` (`query-as` / `tx-query-as`) with zero `; Schema:` comments, verified by `cirru_edn::parse` and Calcit CLI checks.
- Hardened `src/wasmtime_db.rs`:
  - Enabled rusqlite `hooks` in `Cargo.toml` for scoped query deadlines (`SqliteProgressGuard`) and statement-level `update_hook` insert rowid capture (`SqliteInsertHookGuard`), preserving `0` / negative rowids while returning `None` for `WITHOUT ROWID` tables and reporting `rows_affected == 0` for DDL.
  - Added out-of-range `query_timeout` validation (`apply_busy_timeout`) without panicking on `Duration::MAX`.
  - Extended `inspect_sql_statement` to skip bracket `[ident]` and backtick `` `ident` `` quoted SQLite identifiers and resolve CTE column lists vs main statement verbs.
  - Added duplicate column projection rejection, `max_result_bytes < 2` rejection for empty results, streaming `BoundedJsonWriter` (rejecting oversized BLOB/TEXT values before allocation), and scoped internal savepoints for writable `RETURNING` queries so limit/decode failures roll back without disturbing caller savepoints.
  - Added `reconcile_tx_state()` on SQLite autocommit/error paths (e.g., `ON CONFLICT ROLLBACK`) and pop-on-success in `rollback_all_open_transactions`.
  - Exposed public `DbConfig` policy checkers and store-wide `max_kv_total_bytes`, enforced host policy in `add_to_linker` prior to pluggable backend calls, and added controllable clock (`InMemoryKvBackend::with_clock`) with eager expiry reclamation on `set`.
  - Updated `HttpDbServiceRunner` to keep relative URIs so `Host` header authority is preserved, bounded streaming response body reads (`with_max_response_body_bytes`), handled trapped/unset-outparam turns cleanly without hanging, poisoned retired stores on trap/cancel/body-overflow, and rolled back open transactions across cancelled futures and `Drop`.
- Expanded `tests/wasi_http_db.rs` with 9 tests covering all edge cases deterministically without `thread::sleep`.
