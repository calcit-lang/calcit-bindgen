# Execute a native-async Component in Wasmtime / 在 Wasmtime 执行原生异步 Component

## 中文

- 增加自包含 Component IR v3 与 Canonical ABI core fixture，并通过已有 `generate` 入口打包。
- 使用 Wasmtime 47 的 `call_async` 实际调用 async export，覆盖 Number 与 typed Result 的 success/error。
- 使用 `func_wrap_concurrent` 提供 async import；host future 至少挂起一次，core 通过 waitable set、join、wait 与 drop 生命周期恢复并完成。
- 当前切片只证明可运行的 request/future 基础，不宣称主动取消或完整 post-return ownership 已完成。

## English

- Add self-contained Component IR v3 and Canonical ABI core fixtures packaged through the existing `generate` entry point.
- Invoke async exports through Wasmtime 47 `call_async`, covering Number plus typed Result success/error.
- Provide an async import with `func_wrap_concurrent`; the host future suspends at least once and the core resumes through waitable-set, join, wait, and drop lifecycle operations.
- This slice proves a runnable request/future foundation without claiming active cancellation or complete post-return ownership.

## Verification / 验证

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo package`
