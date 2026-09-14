# Package Bool Calcit Components

## 中文

Component packaging 的第二个 vertical slice 将 Calcit core 已推导并导出的 `Bool` 直接映射为 WIT `bool`。实现只扩展既有 Component type renderer，不增加另一套类型规则，也不对 unsupported composite types 提供 fallback。

测试 fixture 由 Calcit #1103 的真实 `component-wasm.cirru` 生成，覆盖 Bool import/export、true/false 往返以及 Bool/Number 混合签名。CI 固定对应的 Calcit adapter revision，现场生成 contract 与 core module，再由 Wasmtime 和 jco/Node 验证同一 Component；manifest、digest 与 stale check 继续复用 Number/String 路径。

## English

The second Component-packaging vertical slice maps inferred `Bool` exported by Calcit core directly to WIT `bool`. The implementation only extends the existing Component type renderer. It adds no parallel type rules and no fallback for unsupported composite types.

The contract fixture comes from the real `component-wasm.cirru` program in Calcit #1103 and covers Bool imports and exports, true/false round trips, and a mixed Bool/Number signature. CI pins the matching Calcit adapter revision, generates the contract and core module in place, and exercises the same Component through Wasmtime and jco/Node. Manifests, digests, and stale checks continue to reuse the Number/String path.
