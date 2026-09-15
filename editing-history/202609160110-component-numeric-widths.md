# 生成明确数值宽度的 Component / Generate explicit numeric-width Components

## 中文

- 消费 Component Interface IR v2 的 Int8/UInt8/Int16/UInt16/Int32/UInt32/Int64/UInt64/Float32/Float64，并一一映射到 WIT Canonical ABI 类型。
- 复用同一递归类型 renderer 覆盖参数、结果、Struct、Enum、List、Option 与 Result；WIT 保留字使用 `%` 转义，不改变 Calcit contract 身份。
- 扩展既有 `diff` 支持 Component contract，使数值宽度、方向、模块、符号与签名变化成为明确 breaking change；manifest 与 stale check 继续基于完整规范化 contract。
- 真实 Calcit CI 固定到包含数值 adapter 与 Float32 NaN 修复的提交，通过 Wasmtime 和 jco/Node 验证 direct export、host import、边界值及不安全 64 位整数拒绝路径。
- 不新增命令，也不为旧 Component IR v1 增加兼容层；Native Interface IR v2 继续拒绝 Component 专用数值 refinement。

## English

- Consume Int8/UInt8/Int16/UInt16/Int32/UInt32/Int64/UInt64/Float32/Float64 from Component Interface IR v2 and map them one-to-one to WIT Canonical ABI types.
- Reuse one recursive type renderer across parameters, results, Struct, Enum, List, Option, and Result positions. Escape WIT keywords with `%` without changing Calcit contract identity.
- Extend the existing `diff` entry point to Component contracts so numeric widths, directions, modules, symbols, and signatures produce explicit breaking changes. Manifests and stale checks continue hashing the complete canonical contract.
- Pin real-Calcit CI to the numeric adapter plus Float32 NaN fix, then exercise direct exports, host imports, boundary values, and unsafe 64-bit integer rejection with Wasmtime and jco/Node.
- Add no command and no compatibility adapter for Component IR v1. Native Interface IR v2 continues to reject Component-only numeric refinements.
