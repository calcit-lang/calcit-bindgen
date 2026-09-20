# 在 Wasmtime 中接入有界 WASI HTTP

Calcit 的 HTTP 边界是一个闭合、完整缓冲的 async Component import：
`calcit:wasi-http/client.request`。它只向 Calcit 暴露 method、URL、headers、body、status
和 typed error，不暴露 `wasi:http` 的 resource、stream、pollable、socket 或 TLS 对象。

`calcit-bindgen generate` 发现精确匹配的 contract 后，会在现有 managed output 中额外生成
`rust/wasmtime_http_adapter.rs` 与可运行的 `rust/wasmtime-http-host/` crate。`check` 会用相同
manifest 规则守门这些文件；接口缺字段、类型变化、改成 sync、出现重复 import，或者生成文件
过期都会直接失败，避免宿主与 Calcit 静默漂移。这里没有增加新的命令。
Component packaging 会保留 `calcit:wasi-http/client` 这一 package-qualified import identity；
生成阶段使用内部合法 WIT alias 包装 core module，最终 Component 对宿主暴露的仍是原始名称，
不会要求业务代码改成私有缩写。

## 直接运行生成的宿主

复制生成的 capability 示例到 managed output 之外，再显式填写权限：

```cirru
{}
  :component |/absolute/path/to/component.wasm
  :entry |call-host-http-request
  :arguments-file |/input/request.cirru
  :result-file |/output/result.cirru
  :max-response-bytes 1048576
  :allowed-origins $ []
    |https://api.example.com
  :preopens $ []
    {}
      :host |/absolute/path/to/input
      :guest |/input
      :access :read
    {}
      :host |/absolute/path/to/output
      :guest |/output
      :access :read-write
```

`/input/request.cirru` 保存完整参数列表，而不是单个参数：

```cirru
[]
  {}
    :body $ :: :empty
    :headers $ []
    :max-response-bytes 65536
    :method $ :: :get
    :url |https://api.example.com/items
```

然后运行同一个 `generate` 产物中的 host crate：

```bash
cargo run --manifest-path generated-component/rust/wasmtime-http-host/Cargo.toml \
  -- /path/to/capabilities.cirru
```

生成目录完全由 `generate` 管理；capability 文件和业务封装归用户维护。`allowed-origins` 与
`preopens` 缺省时都为空，未知字段会被拒绝；文件只接受明确的 `:read` / `:read-write` preopen。
`arguments` 缺省为空列表。`:arguments-file` 与非空 inline `:arguments` 互斥；文件路径是 guest path，
必须落在 read 或 read-write preopen。`:result-file` 必须落在 read-write preopen，并写入与 stdout 相同的
规范 Cirru EDN。两类路径都拒绝 `..`、未授权路径与 symlink 逃逸，不接受 host path 或 JSON fallback。
宿主从 Component function type 读取参数形状，在实例调用前严格解码；参数数量、字段、case、整数范围或
payload 不匹配都会带路径失败。参数文件上限为 4 MiB。
零参数 Unit 入口保持兼容且不输出结果。

生成 host 的退出码是稳定协议：成功为 `0`，配置/参数/`invalid-request` 为 `2`，文件或网络
`capability-denied` 为 `3`，`transport` 为 `4`，`response-too-large` 为 `5`，`unsupported` 为 `6`，
其他宿主或 Component 内部失败为 `1`。typed error 仍完整写入 stdout 和可选结果文件，退出码只用于编排。

数据映射保持一套确定规则：Bool、String 与有限 Number 使用对应 Cirru EDN 标量；WIT 整数只接受
范围内的无小数 Number；超出 Cirru EDN `f64` 无损范围的 64 位整数使用明确的
`:: :s64 |decimal` / `:: :u64 |decimal`，保证完整范围不丢精度。`list<u8>` 使用 `buf`，其他 list
使用 `[]`；record 使用 tag-key map；variant 与 enum 使用未限定的 `::`；Option
使用 `:: :none` / `:: :some value`，Result 使用 `:: :ok value` / `:: :err value`。不接受隐式 coercion、
额外 record 字段、`%::` nominal 名称或 JSON fallback。resource、future、stream 等宿主所有权值也会
明确拒绝，而不是猜测 Dynamic 表示。

## 自定义宿主依赖

宿主 crate 使用与生成器一致的 Wasmtime 47，并显式启用 HTTP adapter：

```toml
[dependencies]
calcit-bindgen = { version = "0.1.7", features = ["wasmtime-http"] }
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
    .allow_origin("https://api.example.com")?
    .max_response_bytes(1_048_576);
calcit_wasi_http::add_to_linker(&mut linker, http)?;
```

`WasiHttpConfig::default()` 不授予任何网络访问。origin 使用精确的
`scheme://authority` 匹配，端口是 authority 的一部分；当前不自动跟随重定向，因此重定向也不会
绕过 capability。生产环境应只授予业务需要的 origin，而不是把用户输入直接传给
`allow_origin`。

## 有界行为

- request body 的 empty、UTF-8 text 与任意 bytes 都会保持原始语义；
- response 在读取过程中累计字节数，首次超过 `max-response-bytes` 或宿主上限就返回
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
