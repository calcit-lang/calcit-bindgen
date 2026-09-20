# Strict Cirru EDN generated-host values / 严格 Cirru EDN 生成宿主值

## 中文

- 为 generated Wasmtime host 的同一份 capability 配置增加可选 `:arguments`，按运行时 Component function type 严格解码。
- 支持 Bool、String、明确宽度数值、Buffer/list、record、variant/enum、Option 与 Result；拒绝隐式转换、额外字段、错误 case、越界数值及不受支持的 resource/stream/future。
- 以 Cirru EDN 向 stdout 输出类型化结果，并保留零参数 Unit 入口兼容；64 位大整数使用显式十进制 case 保持无损。
- 用 Calcit 0.18.0 真实 HTTP Component 固定成功、默认拒绝与 response-too-large 三条 generated-host 路径。

## English

- Add optional `:arguments` to the generated Wasmtime host's existing capability configuration and decode them strictly from the runtime Component function type.
- Support Bool, String, width-specific numbers, Buffer/list, record, variant/enum, Option, and Result while rejecting coercions, extra fields, invalid cases, out-of-range values, and unsupported resource/stream/future values.
- Emit typed results as Cirru EDN on stdout while preserving zero-parameter Unit compatibility; explicit decimal cases keep large 64-bit integers lossless.
- Pin generated-host success, default-deny, and response-too-large paths with a real Calcit 0.18.0 HTTP Component.
