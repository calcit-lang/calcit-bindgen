# 禁止 CI checkout 持久化凭据

- `pull_request` workflow 的仓库 checkout、Calcit core checkout 与文档 checkout 均显式设置 `persist-credentials: false`。
- 保留现有 action revision、Calcit 精确 revision、路径与平台条件，避免被测代码读取持久化的 `GITHUB_TOKEN`。
