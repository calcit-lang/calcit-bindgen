# Clarify async capability boundaries / 澄清 async 能力边界

## 中文

- 根据 PR review 修正 README 能力矩阵，区分已支持的 async WIT declaration 渲染与尚未支持的 async Canonical ABI lifecycle。
- 在规范性非目标中明确可运行 async Component、callback 与 resource lifecycle 仍未实现，避免与本次 WIT 支持互相矛盾。

## English

- Fix the README capability matrix after PR review by separating supported async WIT declaration rendering from the unsupported async Canonical ABI lifecycle.
- Clarify in the normative non-goals that runnable async Components, callbacks, and resource lifecycle remain unimplemented, avoiding a contradiction with this WIT slice.

## Verification / 验证

- `cargo fmt --check`
- `cargo test component_v3_async_invocation_renders_native_async_wit`
