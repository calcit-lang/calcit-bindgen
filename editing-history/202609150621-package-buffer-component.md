# Package Buffer Calcit Components

## 中文

Component packaging 的第三个 vertical slice 将 Calcit core 已推导并导出的 `Buffer` 直接映射为 WIT `list<u8>`。实现只扩展既有 Component type renderer，不增加另一套类型规则，也不把 Buffer 经过 UTF-8 String 转码。

测试 fixture 来自 Calcit #1108 的真实 `component-wasm.cirru`，覆盖空 Buffer、内嵌零、非 UTF-8 字节、Bool/Buffer 混合签名、类型身份及 host import/export。CI 固定对应的 Calcit adapter revision，现场生成 contract 与 core module，再由 Wasmtime 和 jco/Node 验证同一 Component；manifest、digest 与 stale check 继续复用既有路径。

## English

The third Component-packaging vertical slice maps inferred `Buffer` values exported by Calcit core directly to WIT `list<u8>`. The implementation only extends the existing Component type renderer. It adds no parallel type rules and never transcodes Buffer values through UTF-8 Strings.

The contract fixture comes from the real `component-wasm.cirru` program in Calcit #1108 and covers empty buffers, embedded zeroes, non-UTF-8 bytes, mixed Bool/Buffer signatures, type identity, and host imports and exports. CI pins the matching Calcit adapter revision, generates the contract and core module in place, and exercises the same Component through Wasmtime and jco/Node. Manifests, digests, and stale checks continue to reuse the existing path.
