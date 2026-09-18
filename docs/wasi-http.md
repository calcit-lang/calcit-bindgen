# 在 Wasmtime 中接入有界 WASI HTTP

Calcit 的 HTTP 边界是一个闭合、完整缓冲的 async Component import：
`calcit:wasi-http/client.request`。它只向 Calcit 暴露 method、URL、headers、body、status
和 typed error，不暴露 `wasi:http` 的 resource、stream、pollable、socket 或 TLS 对象。

`calcit-bindgen generate` 发现精确匹配的 contract 后，会在现有 managed output 中额外生成
`rust/wasmtime_http_adapter.rs`。`check` 会用相同规则守门该文件；接口缺字段、类型变化、改成
sync 或出现重复 import 都会直接失败，避免宿主与 Calcit 静默漂移。这里没有增加新的命令。

## 宿主依赖

宿主 crate 使用与生成器一致的 Wasmtime 47，并显式启用 HTTP adapter：

```toml
[dependencies]
calcit-bindgen = { version = "0.1.1", features = ["wasmtime-http"] }
tokio = { version = "1", features = ["rt-multi-thread"] }
wasmtime = { version = "=47.0.4", default-features = false, features = [
  "component-model",
  "component-model-async",
  "cranelift",
  "runtime",
  "std",
] }
```

在宿主的 linker 建立阶段引入生成文件，并明确授予 origin：

```rust
mod calcit_wasi_http {
    include!("generated-component/rust/wasmtime_http_adapter.rs");
}

let http = calcit_wasi_http::WasiHttpConfig::default()
    .allow_origin("https://api.example.com")?;
calcit_wasi_http::add_to_linker(&mut linker, http)?;
```

`WasiHttpConfig::default()` 不授予任何网络访问。origin 使用精确的
`scheme://authority` 匹配，端口是 authority 的一部分；当前不自动跟随重定向，因此重定向也不会
绕过 capability。生产环境应只授予业务需要的 origin，而不是把用户输入直接传给
`allow_origin`。

## 有界行为

- request body 的 empty、UTF-8 text 与任意 bytes 都会保持原始语义；
- response 在读取过程中累计字节数，首次超过 `max-response-bytes` 就返回
  `response-too-large(observed-bytes)` 并释放连接；
- `text/*`、`application/json` 与 `*+json` 响应在 UTF-8 有效时返回 text，否则返回 bytes；
- header 值保持 bytes；非法或标准禁止的 request header 返回 `invalid-request`；
- 未授权 origin 返回 `capability-denied`，DNS、连接、TLS、超时与协议失败返回 `transport`；
- 宿主需要保留接口但不提供 HTTP 时，可用 `WasiHttpConfig::unsupported(reason)` 返回闭合的
  `unsupported`，而不是让调用悬空或伪造成功；
- 空响应返回 empty，不用伪造的成功响应掩盖错误。

实现复用 Wasmtime 47 稳定的 WASI 0.2 `wasi:http/outgoing-handler` 生产传输，而 Calcit-facing
Component contract 保持 WASI 0.3 原生 async。这样可以先支持简单业务的真实 HTTP 调用，同时
避免依赖 Wasmtime 当前仍标记为 experimental 的 WASI 0.3 HTTP host 模块。等该模块稳定后，
可以替换 adapter 内部实现，不改变 Calcit contract 或命令入口。

仓库测试会在本机回环地址启动真实 HTTP server，验证成功请求、默认拒绝、响应上限，以及
生成的 async Component import 与 linker 类型完全匹配。仅生成 WIT 或 mock 返回值不算通过。
