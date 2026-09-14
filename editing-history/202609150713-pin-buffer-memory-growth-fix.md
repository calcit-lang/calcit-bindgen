# 跟进 Buffer memory growth 修复

- 将跨仓库 Component Buffer smoke 固定到 Calcit PR #1108 合并进 `main` 的精确 revision `29fc1fb0`。
- 新 revision 让 Component String/Buffer tagged bytes 通过 `cabi_realloc` 分配，并由上游大 Buffer 重复 round-trip 测试覆盖 `memory.grow`。
- bindgen 继续通过真实 Calcit core、Wasmtime 与 jco 验证同一份 `list<u8>` 契约；未改变 WIT schema 或生成输出。
