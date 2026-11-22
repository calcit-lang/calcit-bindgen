# `calcit:db@0.1.0` 数据库 WIT 宿主能力合约

本目录固定 Calcit HTTP + DB WASI 服务的版本化 WIT 接口 [`db.wit`](db.wit)，供 `calcit-bindgen::package_wasi_http_service`、代码生成器及 `wasmtime_db` 宿主适配层共同消费。

## Canonical ABI 与名义类型对齐

Calcit 在生成/校验 nominal `defenum`（`DbParam`、`DbError`）与 `defstruct`（`DbExecResult`、`DbQueryResult`）的 Canonical ABI 布局时按成员名称字典序固定判别式与字段顺序，因此 `db.wit` 中所有 `variant` case 与 `record` 字段均严格保持字典序：

- `db-param`：`bool-value`、`bytes-value`、`float-value`、`int-value`、`null-value`、`text-value`
- `db-error`：`backend`、`capability-denied`、`constraint-violation`、`decode-error`、`invalid-query`、`timeout`、`tx-error`、`unsupported`
- `db-exec-result`：`last-insert-id`、`rows-affected`
- `db-query-result`：`columns`、`rows-json`

## 兼容性约定

- `http-service` world 通过 `include wasi:http/proxy@0.2.12;` 组合 WASI HTTP 服务导出，同时导入 `calcit:db/sql@0.1.0` 与 `calcit:db/kv@0.1.0`。
- `package_wasi_http_service` 与 `package_wasi_http_proxy` 一样拒绝携带 `component-type*` 自定义段的 core module（`E_WASI_EMBEDDED_COMPONENT_TYPE`）及 Preview 1 导入（`E_WASI_HTTP_PREVIEW1_IMPORT`），在最终 Component 中省略核心模块未使用的功能接口（例如只导入 `sql` 时不导入 `calcit:db/kv@0.1.0`），同时保留被在用签名引用的 WIT 类型依赖接口（例如因 `db-error` / `db-param` 等类型被 `sql` 签名引用而保留 `calcit:db/types@0.1.0`）。
- 对 `calcit:db@0.1.0` 的破坏性变更必须提升 WIT 包版本号，保持旧版本接口行为不静默漂移。
