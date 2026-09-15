# Package Component Option/Result / 打包 Component Option/Result

## 中文

- 递归渲染 Component contract 中的 `Option<T>` 与 `Result<T,E>`，Unit 分支使用 WIT result 的省略 payload 语法，普通 Unit 结果不制造占位类型。
- 继续复用 `validate`、`generate`、`check`、manifest、digest 与 stale check，不增加新的命令入口或 variant 专用规则集。
- 将真实 Calcit smoke 固定到 calcit#1113 的当前提交，计划同时在 Wasmtime 与 jco/Node 验证 Option/Result 的全部分支、Unit、递归 List 和 host import/export。

## English

- Recursively render `Option<T>` and `Result<T,E>` from Component contracts, using omitted WIT result payloads for Unit branches and no synthetic placeholder for a plain Unit result.
- Reuse `validate`, `generate`, `check`, manifests, digests, and stale checks without adding a command entry point or variant-specific rule set.
- Pin the real Calcit smoke to the current calcit#1113 commit and verify all Option/Result branches, Unit, recursive Lists, and host imports/exports in both Wasmtime and jco/Node.
