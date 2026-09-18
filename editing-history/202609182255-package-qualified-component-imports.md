# Preserve package-qualified Component imports / 保留带 package 的 Component import

## 中文

- 允许 `namespace:package/interface` 形式的 Component import，同时继续拒绝其他隐式名称改写。
- 在 packaging 内部确定性映射 WIT alias 与 core import，并把最终 Component import 恢复为原始 identity。
- 增加回归测试，确认 Wasmtime 看到的是 `calcit:test-host/api`，而不是内部 alias。

## English

- Accept `namespace:package/interface` Component imports while continuing to reject other implicit name rewrites.
- Deterministically map the WIT alias and core import during packaging, then restore the original identity on the final Component.
- Add a regression test proving that Wasmtime sees `calcit:test-host/api`, not the internal alias.

## Verification / 验证

- `cargo test --test component_package packages_qualified_component_import_without_losing_its_identity`
- Calcit buffered HTTP Component end-to-end test through Wasmtime
