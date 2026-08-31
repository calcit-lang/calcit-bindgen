# Semantic Interface IR compatibility diff

## 中文

- 用显式字段策略替代 `Declaration`/`Definition` 整体 `PartialEq`。
- 只比较 nominal declaration、supported signature/status 与结构化 lowering contract。
- 忽略 package version、doc、display schema、raw lowering 和 diagnostics，避免假 breaking。
- 为 definition 状态迁移、字段级路径、稳定顺序以及 CLI JSON/text 退出行为增加回归测试。

## English

- Replaced whole-value declaration/definition equality with an explicit field policy.
- Limited breaking checks to nominal declarations, supported signatures/status, and structured lowering contracts.
- Ignored package-version and display/debug metadata to avoid false breaking changes.
- Added status-transition, deterministic field-path, and CLI JSON/text regression coverage.
