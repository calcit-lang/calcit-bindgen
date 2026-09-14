# 处理 Component packaging review

- 固定 jco smoke 的 Node.js 22，并在生成目录声明 ESM，避免 runner 默认 Node.js 升级改变模块加载行为。
- 增加 core module 内容变化后的 stale check 回归测试，确认 manifest digest 会使只读 `check` 返回非 current。
- 保持现有 `validate`、`generate`、`check` 入口，不增加新的 Component 命令。
- 验证：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、`cargo package`。
