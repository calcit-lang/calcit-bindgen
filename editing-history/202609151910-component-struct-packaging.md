# Component Struct packaging

## 中文

- 从 Component Interface IR v1 的单态 Struct declaration 确定性生成 namespace-qualified WIT named record。
- 在函数签名和 record 字段中递归支持闭合 Bool、Buffer、Number、String、List、Option、Result 与 Struct 类型；泛型、Unit 字段、Enum 和缺失 declaration 明确失败。
- 通过真实 Calcit fixture 在 Wasmtime 与 jco/Node.js 中覆盖 Struct export、host import、嵌套 Struct、List，以及 Option/Result 的全部分支。

## English

- Deterministically generate namespace-qualified WIT named records from monomorphic Component Interface IR v1 Struct declarations.
- Recursively support closed Bool, Buffer, Number, String, List, Option, Result, and Struct types in signatures and record fields; fail explicitly for generics, Unit fields, Enum, and missing declarations.
- Cover Struct exports, host imports, nested Struct, List, and every Option/Result branch through the real Calcit fixture in Wasmtime and jco/Node.js.
