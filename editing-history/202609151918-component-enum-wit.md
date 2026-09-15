# Component Enum WIT

中文：

- 从 Component contract 直接生成 namespace-qualified WIT variant，保留 case 名称与顺序。
- 无 payload、单 payload 与多 payload 分别生成无 payload case、直接 payload 与 tuple。
- 复用现有递归类型渲染支持嵌套 Struct，并以精确 path 拒绝 Unit payload、generic 与 open shape。
- Wasmtime 与 jco smoke 覆盖直接及宿主 Enum 往返，继续沿用 `generate` / `check` 入口。
- CI 固定到已合并的 Calcit Enum adapter revision，避免 contract 与 core module 漂移。

English:

- Generate namespace-qualified WIT variants directly from the Component contract while preserving case names and order.
- Render zero, single, and multiple payloads as payload-free cases, direct payloads, and tuples.
- Reuse recursive type rendering for nested Structs and reject Unit payloads, generics, and open shapes with precise paths.
- Cover direct and host Enum round trips in Wasmtime and jco while retaining the existing `generate` / `check` entry points.
- Pin CI to the merged Calcit Enum-adapter revision so the contract and core module cannot drift.
