# WASI 0.3 command WIT 来源

这里固定 [WebAssembly/WASI v0.3.0](https://github.com/WebAssembly/WASI/releases/tag/v0.3.0) 的 `wasi:cli/command` world 及其依赖包。五个合并后的 WIT 文件取自 [Wasmtime v47.0.4](https://github.com/bytecodealliance/wasmtime/tree/v47.0.4/crates/wasi/src/p3/wit/deps) 随附的 WASI 0.3 定义；各文件的 `package` 版本均为 `0.3.0`。更新时应同时核对正式 WASI release 的 WIT、更新所有依赖包，并用固定版本 Wasmtime 执行生成的 command Component。

上游 WIT 归 WASI Specification Contributors 版权所有；许可与贡献条款见 [WASI LICENSE](https://github.com/WebAssembly/WASI/blob/v0.3.0/LICENSE)。本仓库的 MIT 许可不替代该上游声明。

本目录只作为 `package_wasi_command` 的版本化输入。生成的 Component 只保留 core module 实际使用的宿主接口；完整 command world 并不意味着自动授予文件系统、网络等能力。

文件 SHA-256：

| 文件 | SHA-256 |
| --- | --- |
| `cli.wit` | `04b2de3bf344052c78080f3c5320442132b4e7c20b42633a92b8400b6d29ab0d` |
| `clocks.wit` | `5b59398732d19f6e327b1693c851a690bdf30e86cbf755d5a53b2fe7d6f2775e` |
| `filesystem.wit` | `612729afb560f715c18de8787014f6b35ee7d3d12a8ef5566d93720ca463c9c9` |
| `random.wit` | `671234beada8dbdff53d10a6554bfbdc6ed9259d3093dfb8cc78c00b67fdf6e5` |
| `sockets.wit` | `9ea0d8bbd63773ee67ee58f4dbb1a4ad5ec250ed79487ed847715c0b87ee95cf` |
