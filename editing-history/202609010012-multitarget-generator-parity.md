# 多目标 generator parity / Multi-target generator parity

## 中文

- 将 Calcit、TypeScript 与 WIT generator 从 Calcit core preview 迁入统一的 production generate/check transaction。
- 默认生成 Rust、Calcit、TypeScript、WIT 四个 backend；支持重复 `--backend` 选择目标，并把规范化 backend 集合写入 manifest schema v2。
- Calcit 输出使用 nominal client、typed trait methods 与 result assertion，使业务调用不再直接维护 native symbol 字符串。
- TypeScript declaration 名由完整 namespace-qualified ID 派生；WIT 严格拒绝 Unit value position、generic declaration/application，并报告 Interface IR type path。
- 新增 composite fixture、逐字节确定性、backend-scoped manifest/check、WIT parser、真实 dylib Rust smoke 的组合验证。
- 真实 core composite fixture 的 TypeScript/WIT 输出与旧 preview 除 generator header 外一致；另以 `wasm-tools component wit`、Calcit Cirru parser 与 TypeScript compiler 完成 smoke。

## English

- Migrated Calcit, TypeScript, and WIT generation from the Calcit core preview into the shared production generate/check transaction.
- Generate Rust, Calcit, TypeScript, and WIT by default; repeated `--backend` flags select targets, and manifest schema v2 records the normalized backend set.
- Calcit output uses a nominal client, typed trait methods, and result assertions so application code does not maintain native symbol strings.
- TypeScript declaration names derive from full namespace-qualified IDs. WIT rejects Unit value positions and generic declarations/applications with precise Interface IR type paths.
- Added composite-fixture determinism, backend-scoped manifest/check, WIT parser validation, and retained the real generated-dylib Rust smoke.
- Against the real core composite fixture, TypeScript and WIT match the old preview except for the generator header; `wasm-tools component wit`, the Calcit Cirru parser, and the TypeScript compiler also pass.
