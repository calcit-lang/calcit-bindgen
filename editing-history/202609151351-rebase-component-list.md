# Rebase Component List packaging / 重放 Component List 打包改动

## 中文

- 在 Buffer Component 支持合并后，把递归 List Component 改动重放到最新 `main`，移除 PR 中重复的父分支历史并解决工作流冲突。
- 保留所有 checkout 步骤的 `persist-credentials: false`，延续主分支的最小权限设置。
- 将真实 Calcit 集成测试固定到 Calcit PR #1110 的最新提交 `f386e3147cf1fcafed9d43799bcbfd593daa7238`，覆盖修正后的 List ABI 内存边界校验。

## English

- Replayed the recursive List Component change onto the latest `main` after Buffer Component support merged, removing duplicated parent-branch history and resolving the workflow conflict.
- Preserved `persist-credentials: false` for every checkout step to retain the least-privilege setting from `main`.
- Pinned the real Calcit integration test to the latest Calcit PR #1110 commit, `f386e3147cf1fcafed9d43799bcbfd593daa7238`, including the corrected List ABI memory-boundary validation.
