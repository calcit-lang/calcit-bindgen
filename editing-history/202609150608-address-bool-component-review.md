# 处理 Bool Component review

- 补齐 Wasmtime 与 jco 对 `call-host-bool-not(false) -> true` 的验证，并在 Wasmtime 每次调用后执行 `post_return`。
- 将 Check workflow 中三处 `actions/checkout` 固定到同一个 v4 不可变提交，避免 CI 随可变 tag 漂移。
- 保持 PR 为 Draft；Calcit 0.14.20 发布、#1103 合并并验证后，再把真实产物 CI 更新到最终 main SHA。
