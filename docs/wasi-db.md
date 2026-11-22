# `calcit:db@0.1.0` 有界 WASI 数据库能力边界

为避免在 WASI WebAssembly Component 中打包重型数据库驱动与 TLS 协议栈，并维持默认拒绝（default-deny）与闭合静态类型契约，`calcit-bindgen` 提供了版本化的 `calcit:db@0.1.0` WIT 数据库宿主能力合约、`package_wasi_http_service` 封装入口、代码生成产物，以及可选的 `wasmtime-db` 宿主适配层。

## WIT 合约与封装世界

[`src/wasi_db_wit/db.wit`](../src/wasi_db_wit/db.wit) 定义：

- `interface types`：`db-param`、`db-error`、`db-exec-result`、`db-query-result`，成员名称严格按字典序排列以对齐 Calcit nominal `defenum` / `defstruct` 的 Canonical ABI 判别式与字段布局。
- `interface sql`：`query`、`exec`、`tx-begin`、`tx-query`、`tx-exec`、`tx-commit`、`tx-rollback`（全部同步调用，参数化绑定、返回结构化 `result<_, db-error>`）。
- `interface kv`：`get`、`set`、`delete`（支持字节值、前缀权限校验与可选毫秒 TTL）。
- `world http-service`：通过 `include wasi:http/proxy@0.2.12;` 组合 WASI HTTP 服务导出与 `sql` / `kv` 导入。

`calcit_bindgen::package_wasi_http_service(core_module)` 会拒绝 WASI Preview 1 导入（`E_WASI_HTTP_PREVIEW1_IMPORT`）与预先嵌入的 `component-type*` 自定义段（`E_WASI_EMBEDDED_COMPONENT_TYPE`），并在产物 Component 中只保留实际使用到的接口（例如只导入 `sql` 时不要求宿主提供 `kv`）。

## 生成产物与零 `Dynamic` 查询解码

当 Component Interface IR 包含 `calcit:db/sql@0.1.0` 或 `calcit:db/kv@0.1.0` 同步导入并严格匹配上述 nominal shape 时，`generate` 与 `check` 会额外生成并由 manifest 校验三个受管文件：

- `wit/db.wit`：固定版本的 `calcit:db@0.1.0` WIT 合约。
- `calcit/db_bindings.cirru`：包含 `DbParam`、`DbError`、`DbExecResult`、`DbQueryResult`、`defwasm-import` 包装，以及零 `Dynamic` 泄漏的 `query-as` / `tx-query-as` 宏：内部顺序执行 `sql-query` / `tx-query` -> `try-parse-json` -> `try-decode-map-as parsed (List ~row-type)`，直接返回 `Result (List RowStruct) DbError`（JSON 或结构解码错误统一收敛为 `DbError :decode-error`）。
- `rust/wasmtime_db_adapter.rs`：启用 `wasmtime-db` feature 时重新导出宿主端类型与运行器。

## 默认拒绝的 `wasmtime-db` 宿主实现

在宿主 crate 启用 `features = ["wasmtime-db"]` 可使用 `calcit_bindgen::wasmtime_db`：

```rust
use calcit_bindgen::wasmtime_db::{
    DbConfig, HttpDbServiceRunner, InMemoryKvBackend, SqliteSqlBackend, WasmtimeDbState,
};

let mut sql = SqliteSqlBackend::in_memory()?;
sql.exec(&DbConfig::default().allow_sql_ddl(), "CREATE TABLE ...", &[])?;

let config = DbConfig::default()
    .allow_sql_read()
    .allow_sql_write()
    .allow_kv_prefix("app:")
    .max_rows(256)
    .max_result_bytes(60 * 1024);

let db = WasmtimeDbState::new(config, Box::new(sql), Box::new(InMemoryKvBackend::new()));
let mut runner = HttpDbServiceRunner::new(&component_bytes, db, Some(8 * 1024 * 1024)).await?;
```

宿主侧安全与有界约束：

- `DbConfig::default()` 默认拒绝所有 SQL read / write / DDL 与所有 KV 键，需由宿主显式授权。
- SQLite 后端 `SqliteSqlBackend` 拒绝多语句拼接、未闭合字面量以及显式 `BEGIN` / `COMMIT` / `ROLLBACK` / `ATTACH` / `PRAGMA` 等控制语句；事务通过宿主管理的 `SAVEPOINT` 追踪，并在每次 HTTP turn 结束后自动回滚泄漏的未提交事务。
- 查询行数上限默认 `1024`、`rows-json` 字节上限默认 `60 KiB`（匹配 Calcit `try-parse-json` 64 KiB 输入上限），超出上限直接返回 `DbError::InvalidQuery`。
- `HttpDbServiceRunner` 为单实例提供 `wasmtime::ResourceLimiter` 线性内存硬上限（默认 `8 MiB`）并在每次响应中报告峰值内存与回滚的泄漏事务数。
