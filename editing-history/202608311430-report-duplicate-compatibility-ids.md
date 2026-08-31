# 报告 compatibility 输入重复 ID / Report duplicate compatibility IDs

## 中文

- `compare` 在公开 `Document` API 边界检测 old/new declaration 与 definition 重复 ID。
- 重复 ID 产生稳定路径的 breaking change，且确定性保留首项用于后续字段比较。
- 增加四种 old/new declaration/definition 重复输入回归。
- 为 semantic comparison helpers 补充职责文档。

## English

- Detect duplicate declaration and definition IDs on both sides of the public
  `compare` API.
- Report duplicates as breaking changes with stable paths and deterministically
  retain the first item for remaining field comparison.
- Cover all four old/new declaration/definition duplicate cases.
- Document the responsibilities of semantic comparison helpers.
