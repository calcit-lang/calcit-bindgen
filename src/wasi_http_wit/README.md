# WASI 0.2.12 HTTP proxy WIT 来源

这里固定的七个 WIT 文件直接复制自 `bytecodealliance/wasmtime` `v48.0.7`（对应 crate `wasmtime-wasi-http 48.0.7` 中的 `crates/wasi-http/wit/deps/{cli,clocks,filesystem,http,io,random,sockets}.wit`），打包了 [WebAssembly/WASI v0.2.12](https://github.com/WebAssembly/WASI/releases/tag/v0.2.12) 的正式 `wasi:http/proxy` world（导出 `wasi:http/incoming-handler@0.2.12`）及其 `io`、`clocks`、`filesystem`、`sockets`、`random`、`cli` 依赖包。与上游 `WebAssembly/WASI v0.2.12` 仓库中 `proxy.wit` 使用 `include imports` 的写法相比，Wasmtime 随附副本在合并为 `http.wit` 时已将 `proxy` world 的依赖接口展开为显式 `import` 列表；本仓库仅对其执行 `sed 's/[[:space:]]*$//'` 移除行尾空格，其余 package 声明、接口、world 定义与 `@since(version = 0.2.x)` 标记保持原样，并用 `wasmtime serve`（Wasmtime 49/50）实测生成的 HTTP Component。

上游 WIT 归 WASI Specification Contributors 版权所有；许可与贡献条款见 [WASI LICENSE.md](https://github.com/WebAssembly/WASI/blob/v0.2.12/LICENSE.md)。本仓库的 MIT 许可不替代该上游声明。

本目录只作为 `package_wasi_http_proxy` 的版本化输入。`package_wasi_http_proxy` 会拒绝已包含 `component-type*` 自定义段的 core module（`E_WASI_EMBEDDED_COMPONENT_TYPE`），因此生成的 Component 以 `wasi:http/incoming-handler@0.2.12` 作为唯一导出，且仅保留 core module 实际使用的宿主导入接口；完整 `proxy` world 并不意味着自动授予网络访问权限。

## 上游注释勘误

为保持与上游 SHA-256 及文本逐字节对齐（仅移除行尾空格），未直接改动 `http.wit` 中的上游注释，在此记录三处已知上游注释勘误：

- `http.wit` `outgoing-response.headers`（第 527–534 行）：注释误写了 `outgoing-request` 与 `outgoing-handler.handle`，实际父资源为 `outgoing-response`，所有权经由 `response-outparam.set` 转移。
- `http.wit` `outgoing-body.finish`（第 575–578 行）：注释误写了未声明的 `outgoing-body.finalize`，实际应在释放 `output-stream` 后调用静态方法 `outgoing-body.finish`；未调用 `finish` 直接丢弃 `outgoing-body` 会被视为响应体不完整。
- `http.wit` `future-incoming-response.get`（第 611–613 行）：响应体读取阶段的错误由 `incoming-body` 及其子资源 `input-stream`（由 `incoming-body.stream` 返回）上报，而非上游注释中误写的 `output-stream`。

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
