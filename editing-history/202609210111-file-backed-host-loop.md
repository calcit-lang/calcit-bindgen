# File-backed generated host loop / 文件化 generated host 闭环

## 中文

- 在现有 Cirru EDN capability 配置中增加可选 `:arguments-file` 与 `:result-file`，不增加命令或另一套 host。
- 只通过显式 preopen 解析 guest path；区分 read 与 read-write，拒绝 `..`、未授权路径和 symlink 逃逸，并限制参数文件为 4 MiB。
- 保持 stdout 的规范 Cirru EDN 结果，同时可写入结果文件；为闭合 HTTP Result 固定成功、输入、权限、transport、响应超限、unsupported 与内部错误退出码。
- 用真实 Calcit 0.18.0 HTTP Component 验证文件输入输出、成功、网络拒绝、transport、响应超限和错误输入。

## English

- Add optional `:arguments-file` and `:result-file` fields to the existing Cirru EDN capability configuration without adding a command or parallel host.
- Resolve guest paths only through explicit preopens, distinguish read from read-write access, reject traversal, ungranted paths, and symlink escape, and cap argument files at 4 MiB.
- Preserve canonical Cirru EDN stdout while optionally writing the same result to a file, with stable exits for success, input, capability, transport, response-limit, unsupported, and internal outcomes.
- Validate file input/output, success, network denial, transport, response limits, and invalid input against a real Calcit 0.18.0 HTTP Component.
