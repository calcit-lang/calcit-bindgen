# WASI 0.2.12 HTTP proxy WIT 来源

这里固定 [WebAssembly/WASI v0.2.12](https://github.com/WebAssembly/WASI/releases/tag/v0.2.12) 的正式 `wasi:http/proxy` world（导出 `wasi:http/incoming-handler@0.2.12`）及其 `io`、`clocks`、`filesystem`、`sockets`、`random`、`cli` 依赖包（与 `wasmtime-wasi-http 48.0.7` 随附 WIT 对齐）。七个合并后的 WIT 文件只移除行尾空格，其余 package 声明、接口、world 定义与 `@since(version = 0.2.x)` 标记保持原样，并用 `wasmtime serve`（Wasmtime 49/50）实测生成的 HTTP Component。

上游 WIT 归 WASI Specification Contributors 版权所有；许可与贡献条款见 [WASI LICENSE](https://github.com/WebAssembly/WASI/blob/v0.2.12/LICENSE)。本仓库的 MIT 许可不替代该上游声明。

本目录只作为 `package_wasi_http_proxy` 的版本化输入。生成的 Component 只保留 core module 实际使用的宿主接口与 `wasi:http/incoming-handler@0.2.12` 导出；完整 `proxy` world 并不意味着自动授予网络访问权限。

文件 SHA-256：

| 文件 | SHA-256 |
| --- | --- |
| `cli.wit` | `fb6ac5d23fcaf3d231142a3c2f9bb1e9bc1fe5af6f1f329f6b1a5555ce0d3873` |
| `clocks.wit` | `6ed8aa65bb8cbe224a0b2cbac9fc1b3bd25bdb17eda5ae0d23c983ed31c447cc` |
| `filesystem.wit` | `e675f261017bf9b4fa7df5b9701023fe0249ede71021ac1bf978748e62edcda8` |
| `http.wit` | `4bbd58f509700a6637385611f183c5bd5984d81feef85e29dfdc05ebd283045a` |
| `io.wit` | `96e206d00076fa0480df32c5bcf255a3fa4862805ac2f6b8537a781cce54f433` |
| `random.wit` | `48578c40213d5cab6650980905fa146a0be8c39c4433ee5e7e00637b87dbe08f` |
| `sockets.wit` | `ad38dbf3b0bbdf34c0f2b608edcbfee04d71b443a63faff1f5e0053c3f84b377` |
