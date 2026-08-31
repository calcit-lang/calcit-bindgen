# Development guide / 开发指南

## 中文

- 本仓库只消费公开、版本化的 Calcit FFI Interface IR，不依赖 Calcit core 内部 Rust 类型。
- generator 输出必须确定性；未知版本、unsupported definition 与无法表示的类型必须明确失败。
- production output 不允许保留 `todo!` 或 Dynamic fallback。
- generated output 只能替换带有效 ownership manifest 的专用目录；发现未托管文件必须失败并保留用户数据。
- `check` 必须保持只读，missing、modified、stale manifest 与 unexpected artifact 使用不同诊断。
- Issue、PR 标题和正文保持中英双语。
- 每次提交前在 `editing-history/` 增加时间戳记录。

## English

- Consume only the public, versioned Calcit FFI Interface IR; never depend on Calcit core's internal Rust types.
- Generator output must be deterministic. Unknown versions, unsupported definitions, and unrepresentable types fail explicitly.
- Production output must not contain `todo!` or Dynamic fallbacks.
- Replace generated output only when a valid ownership manifest marks a dedicated directory; preserve user data and fail on unmanaged files.
- Keep `check` read-only and distinguish missing, modified, stale-manifest, and unexpected-artifact diagnostics.
- Keep Issue and PR titles and bodies bilingual in Chinese and English.
- Add a timestamped note under `editing-history/` before every commit.

## Verification / 验证

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo package
```
