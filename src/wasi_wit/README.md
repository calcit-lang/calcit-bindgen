# WASI 0.3.1 command WIT 来源

这里固定 [WebAssembly/WASI v0.3.1](https://github.com/WebAssembly/WASI/releases/tag/v0.3.1) 的正式 `wasi:cli/command` world 及其 clocks、filesystem、sockets、random 依赖。五个合并后的 WIT 文件由该 tag 的 `proposals/{cli,clocks,filesystem,sockets,random}/wit/*.wit` 按 package 合并而成：每个 package 只保留一次 `package` 声明，接口与 world 定义保持原样，仅移除行尾空格。各文件的 `package` 及跨包引用均为 `0.3.1`；原有 `@since(version = 0.3.0)` 表示接口引入版本，不应改写。更新时应同时核对正式 WASI release 的 WIT、更新所有依赖包，并用固定版本 Wasmtime 执行生成的 command Component。当前验证版本为 Wasmtime 49.0.0。

上游 WIT 归 WASI Specification Contributors 版权所有；许可与贡献条款见 [WASI LICENSE](https://github.com/WebAssembly/WASI/blob/v0.3.1/LICENSE)。本仓库的 MIT 许可不替代该上游声明。

本目录只作为 `package_wasi_command` 的版本化输入。生成的 Component 只保留 core module 实际使用的宿主接口；完整 command world 并不意味着自动授予文件系统、网络等能力。

文件 SHA-256：

| 文件 | SHA-256 |
| --- | --- |
| `cli.wit` | `0dbb1762f4c0d96447b70f0ffc2017e1ae7597a6a079f5b98921c8422f4396cd` |
| `clocks.wit` | `865c9af2bfe2985369a01b48e5a855574d5aa0175fda369561f4792359acac60` |
| `filesystem.wit` | `b157b5d41d552629abcdb2a2b65bbe30e5f382e3cf2c2ad2d9402e5c6040075f` |
| `random.wit` | `c4c136385fef43709cf7b7dbef8b40bc4600ec1858cb5a6dfa8ef0d0b7c987a5` |
| `sockets.wit` | `7cb4327c2da039d974e5a88725a936900f8fe06cbbec6cf9498f84f0de838adc` |
