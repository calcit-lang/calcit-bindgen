# WASI 0.2.12 HTTP proxy packaging / WASI 0.2.12 HTTP 代理封装

2026-11-22 15:15 +0800

## 中文

- 在 `src/wasi_http_wit/` 固定官方 WASI v0.2.12 `wasi:http/proxy` world 及其 `io`、`clocks`、`filesystem`、`sockets`、`random`、`cli` 依赖 WIT（与 `wasmtime-wasi-http 48.0.7` 对齐），记录来源与 SHA-256 摘要，并在测试中校验文档摘要一致。
- 在 `src/component.rs` 提取共享辅助函数并公开 `package_wasi_http_proxy(core_module)`，针对 `wasi:http/incoming-handler@0.2.12` 校验与封装 Component，明确拒绝 `wasi_snapshot_preview1`（`E_WASI_HTTP_PREVIEW1_IMPORT`）与旧版 0.2.0 导出；版本号升级为 `0.1.11` 供 `calcit-lang/calcit#2877` 接入 `wasmtime serve`。
- 新增 `tests/wasi_http_proxy.rs` 覆盖组件封装、`wasmtime serve --max-instance-reuse-count 8` 多请求复用、Preview 1 拒绝、0.2.0 版本拒绝及 WIT SHA-256 校验，并接入 `.github/workflows/check.yaml`。

## English

- Pin the official WASI v0.2.12 `wasi:http/proxy` world and its `io`, `clocks`, `filesystem`, `sockets`, `random`, and `cli` WIT dependencies in `src/wasi_http_wit/` (aligned with `wasmtime-wasi-http 48.0.7`), recording provenance and SHA-256 digests verified by tests.
- Extract shared packaging helpers in `src/component.rs` and export `package_wasi_http_proxy(core_module)` targeting `wasi:http/incoming-handler@0.2.12`, rejecting `wasi_snapshot_preview1` (`E_WASI_HTTP_PREVIEW1_IMPORT`) and older `0.2.0` handler exports; bump crate version to `0.1.11` for `calcit-lang/calcit#2877`.
- Add `tests/wasi_http_proxy.rs` covering deterministic Component packaging, multi-request `wasmtime serve --max-instance-reuse-count 8` execution, Preview 1 / `0.2.0` rejection, and WIT hash checks, wired into `.github/workflows/check.yaml`.
