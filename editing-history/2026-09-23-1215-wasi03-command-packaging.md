# WASI 0.3 command Component 封装

Calcit #1266 需要 `calcit wasi` 最终生成可执行的 command Component。现有通用 Component generator 输出应用自有 world，无法据此声称产物实现了 `wasi:cli/run@0.3.0`。

本次在 bindgen 增加 Rust API `package_wasi_command`，加载固定的 WASI v0.3.0 command WIT 与依赖包，用现有 Canonical ABI encoder 验证并封装调用方提供的 core module。packager 明确拒绝 `wasi_snapshot_preview1` import，不把旧模块包装成新的目标。公开 CLI 不新增命令；后续由 Calcit core 的 `wasi` 路径提供语义 lowering 并调用此 API。

本地用 Wasmtime 47.0.4 的 `run -S p3` 执行成功和失败结果，并验证缺少新 ABI 的旧 import 会被拒绝。CI 固定同一 Wasmtime 版本。当前还没有从 Calcit 程序生成对应的 core module；参数、环境与标准流的 Calcit lowering 留在 #1266 的后续提交中。

PR review 后，CI 下载固定校验 release asset 的 SHA-256，测试会核对五个 WIT 文件与上方文档中的摘要，并将 core module 校验错误改成与调用入口无关的措辞。
