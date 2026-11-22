# PR #60 CI and review follow-up / PR #60 CI 与评审意见修复

2026-11-22 17:00 +0800

## 中文

- 将 `.github/workflows/check.yaml` 中的已退役 runner `macos-14` 更新为 `macos-15`，修复 `Rust / macos-14` 配置不支持导致的 CI 失败。
- 在 `src/component.rs` 新增 `reject_embedded_component_metadata`，在 `package_wasi_world` 中拒绝预嵌入 `component-type*` 自定义段的 core module（`E_WASI_EMBEDDED_COMPONENT_TYPE`），并在 `tests/wasi_http_proxy.rs` 与 `tests/wasi_command.rs` 中加强唯一 Component 导出断言及补充回归测试。
- 更新 `src/wasi_http_wit/README.md`：明确七个 WIT 文件来自 `bytecodealliance/wasmtime` `v48.0.7`（`crates/wasi-http/wit/deps/*.wit`，展开了 `proxy` world 的 `include imports` 并去除行尾空格），修正 `v0.2.12/LICENSE.md` 链接，说明 `E_WASI_EMBEDDED_COMPONENT_TYPE` 唯一导出约束，并记录保留原样 WIT 文本的三处上游注释勘误。

## English

- Update the retired `macos-14` runner in `.github/workflows/check.yaml` to `macos-15` to resolve the unsupported runner configuration CI failure.
- Add `reject_embedded_component_metadata` in `src/component.rs` inside `package_wasi_world` to reject core modules with pre-embedded `component-type*` custom sections (`E_WASI_EMBEDDED_COMPONENT_TYPE`), tighten exact export assertions, and add regression tests in `tests/wasi_http_proxy.rs` and `tests/wasi_command.rs`.
- Update `src/wasi_http_wit/README.md` to document exact `bytecodealliance/wasmtime` `v48.0.7` (`crates/wasi-http/wit/deps/*.wit`) provenance and whitespace normalization, fix the `v0.2.12/LICENSE.md` link, state the single-export `E_WASI_EMBEDDED_COMPONENT_TYPE` guarantee, and record the three upstream comment errata while leaving vendored `.wit` files untouched.
