# Address strict host review / 处理严格宿主 review

## 中文

- generated host 的 Option/Result 解码现在与普通 Variant/Enum 一致，只接受无类型名的 `::` case，拒绝 `%::` nominal enum 混入 Component value。
- 用真实 Component type introspection 固定 nominal Option/Result 的字段路径错误，同时保留原有无类型 case 行为。
- CI 在 ignored 的真实 Calcit HTTP 验收前先运行完整 `wasmtime-http` feature 测试，避免 feature-gated 单元和集成测试被过滤掉。

## English

- Align generated-host Option/Result decoding with ordinary Variant/Enum handling by accepting only unqualified `::` cases and rejecting nominal `%::` enums as Component values.
- Pin path-aware nominal Option/Result failures through real Component type introspection while preserving unqualified case behavior.
- Run the complete `wasmtime-http` feature suite in CI before the ignored real Calcit HTTP acceptance test so feature-gated unit and integration tests cannot be filtered out.
