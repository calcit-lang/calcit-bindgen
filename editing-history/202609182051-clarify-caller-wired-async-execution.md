# Clarify caller-wired async execution / 澄清调用方 wiring 的异步执行边界

## 中文

- 修正 capability matrix 与新增 Wasmtime 执行验收之间的矛盾。
- 明确本工具可以打包并执行调用方提供完整 async Canonical ABI wiring 的 core module。
- 明确 calcit-bindgen 不生成该 wiring，也不把主动取消、callback 或 resource lifecycle 视为已完成。

## English

- Resolve the contradiction between the capability matrix and the new Wasmtime execution acceptance.
- State that the tool packages and executes caller-supplied core modules with complete async Canonical ABI wiring.
- Keep wiring generation, active cancellation, callbacks, and resource lifecycle explicitly unsupported.

## Verification / 验证

- `cargo fmt --check`
- `cargo test --test async_component`
