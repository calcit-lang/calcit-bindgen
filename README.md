# calcit-bindgen

Calcit FFI Interface IR 的确定性 production generator 与兼容性守门工具。

Deterministic production generation and compatibility gates for Calcit FFI
Interface IR.

## Status / 状态

本仓库处于 **active development / experimental tooling** 阶段。native Interface IR v2/v3 的
validation、compatibility diff、canonical generate/check 与严格同步 Rust、Calcit、TypeScript、WIT
backends 已可用；Component Interface IR v2/v3 的 production 路径也可将 Calcit 生成的
Bool/Buffer/Number/String、递归同质 List、闭合单态 Option/Result、单态 Struct record 及 Unit 结果的 core module 打包为可运行 WebAssembly Component。公开、版本化的 Interface IR 和
Canonical ABI adapter 由 Calcit core 定义；WIT、组件封装、manifest 和多宿主验证由本工具负责。
更完整的复合类型支持与真实生态迁移仍由
[calcit-bindgen#5](https://github.com/calcit-lang/calcit-bindgen/issues/5) 追踪。

This repository is active experimental tooling. Native Interface IR v2/v3
validation, compatibility diff, canonical generate/check, and strict
synchronous Rust, Calcit, TypeScript, and WIT backends are usable. The first
Component Interface IR v2/v3 production path also packages Calcit-generated
Bool/Buffer/Number/String, recursively homogeneous List, closed monomorphic Option/Result, monomorphic Struct records, closed monomorphic Enum variants, and Unit-result core modules as runnable WebAssembly Components. Calcit core owns
the public versioned contract and Canonical ABI adapters; this tool owns WIT,
component packaging, manifests, and cross-host verification. Composite types
and broader real-module migration remain tracked by calcit-bindgen#5.

## 中文

该 crate 独立于 Calcit core，严格消费 `calcit ffi export` 产生的版本化 Interface IR。
Component contract 默认使用 Cirru EDN；需要接入只接受 JSON 的工具时，可显式指定
`--format json`。native v2/v3 envelope/document 支持 JSON 校验和兼容性 diff，确保
未知版本、缺失 declaration、错误 nominal kind/arity、非 monomorphic callable 在
进入生成器前失败。

输入为 `calcit ffi export --json` envelope 时，还会校验 envelope schema ID、依赖
过滤条件、summary 计数、完整结构化 diagnostics 与 core 生成的 revision digest。
裸 Interface IR v2/v3 document 也是受支持输入；两种入口都会按公开 JSON schema 拒绝
未知字段。`filters.namespace` 是 v1 必填字段，未筛选时必须为 `null`。loader 会先
验证 envelope metadata，再抽取并返回内层 `Document`；返回值本身不保留 envelope
metadata。

```bash
calcit project/calcit.cirru ffi export --json > interface.json
cargo run -- validate interface.json
cargo run -- diff previous.json interface.json --json
cargo run -- generate interface.json --out generated
cargo run -- check interface.json --out generated
# 只生成和守门选定目标；省略 --backend 时启用全部目标
cargo run -- generate interface.json --out generated-rust --backend rust
cargo run -- check interface.json --out generated-rust --backend rust
```

Component 路径复用同一组命令，不增加新的顶层工具入口：

```bash
# Cirru EDN 是 Component contract 的默认格式
calcit project/calcit.cirru ffi export --boundary component > component-interface.cirru
calcit project/calcit.cirru wasm --boundary component --emit-path target/component-core

cargo run -- validate component-interface.cirru
cargo run -- generate component-interface.cirru \
  --core-module target/component-core/program.wasm \
  --out generated-component
cargo run -- check component-interface.cirru \
  --core-module target/component-core/program.wasm \
  --out generated-component

# JSON 仅作为显式兼容投影
calcit project/calcit.cirru ffi export --boundary component --format json \
  > component-interface.json
```

Component generation 当前严格接受 monomorphic Bool/Buffer/Number/String、递归同质 `List<T>`、闭合单态 `Option<T>` / `Result<T,E>`、单态 Struct record、闭合单态 Enum variant 与 Unit 结果，并产生规范化
`interface.json`、`wit/interface.wit`、`component/component.wasm` 与 ownership manifest。
当 contract 包含精确的 `calcit:wasi-http/client.request` 闭合接口时，还会生成
`rust/wasmtime_http_adapter.rs` 以及可直接运行的 `rust/wasmtime-http-host/`；它们复用 crate 的
`wasmtime-http` feature 和现有 `generate` / `check` / manifest，不增加命令入口。
adapter 默认拒绝全部网络 origin，调用方必须逐个授予 `scheme://authority`，并由每次请求的
`max-response-bytes` 与宿主上限共同约束完整缓冲。宿主 capability 使用 Cirru EDN，网络与文件系统
均默认拒绝；同一配置中的 `arguments` 会按 Component function type 严格解码，结果也以 Cirru EDN
写到 stdout。需要文件化调用时，`:arguments-file` 与 `:result-file` 只通过显式 preopen 读取或写入同一
Cirru EDN 数据，不接受 host path、越界或 JSON fallback；生成 host 还为闭合 HTTP failure 提供稳定退出码。
零参数 Unit 入口继续兼容，不增加另一套 host 或 JSON 默认。完整接入方式见
[WASI HTTP 文档](docs/wasi-http.md)。
Component v3 的 `invocation` 会被显式校验；`sync` 生成普通 `func`，`async` 生成 WASI 0.3 原生
`async func`。打包仍要求 core module 实现对应的 Canonical ABI，缺少 async builtin wiring 时会在写入产物前失败。
仓库内 Wasmtime 47 验收会实际调用 async export、typed Result success/error，并让 concurrent async host import
至少挂起一次后通过 waitable 生命周期恢复。CI 还固定使用 Calcit 0.18.0 生成的真实 core，分别验证主动取消与
callback 竞态、exactly-once completion、同步 post-return 内存复用，以及 stream backpressure、cancel-read 和 drop。
Calcit core 负责生成并测试这些 wiring；bindgen 只消费公开 contract、验证打包结果，不复制 lowering。
manifest 同时记录 contract digest、core module digest、core lifecycle import/export 及其函数签名，以及全部 managed artifacts。
输入 core module 的 import/export、memory、`cabi_realloc` 或 Canonical ABI 签名不匹配时，
命令会在创建或替换输出目录前失败。`check` 会重新编码并保持只读，因此也能发现 core module
变化造成的 stale artifact。

`diff` 将新增 definition/declaration 标记为 additive；删除或修改现有 contract
标记为 breaking，并以非零状态退出，适合 CI 守门。

兼容性比较只覆盖生成代码和调用边界依赖的公开契约：package identity、nominal
declaration shape、supported definition 的 signature/status，以及
backend/target/kind/symbol/invoke/transport lowering。package version、文档、
`logical_schema` 展示文本、`lowering.raw` 与 diagnostic metadata 不属于兼容性
判定；这些内容未来由 stale-artifact `check` 处理，而不是误报 ABI breaking。
unsupported definition 变为 supported 是 additive，反向变化是 breaking。报告路径
精确到发生变化的字段，并使用稳定顺序输出。

`generate` 默认产生规范化、稳定排序的 `interface.json`、`rust/bindings.rs`、
`calcit/bindings.cirru`、`typescript/bindings.d.ts`、`wit/interface.wit` 和版本化
`calcit-bindgen.manifest.json`。manifest 记录启用的 backend、generator、IR/package identity、
确定性 contract digest 和 managed artifact digest。重复的 `--backend` 可选择目标；`check`
必须使用同一集合，并同时守门该集合的所有文件。

输出目录由 manifest 明确标记为 `calcit-bindgen` 专用目录。首次生成不会覆盖已有的
未托管目录；后续生成如果发现 manifest 之外的文件也会拒绝删除。完整输出先写入同文件系统
临时目录，再以目录切换提交。`check` 完全只读，并分别报告 missing、modified、
stale-manifest 与 unexpected artifacts，适合直接用于 CI。

Rust backend 只接受 `native + sync + edn-buffer-v1`，生成 namespace-qualified Rust
名称、typed service trait、Unit/Bool/Number/String/Buffer/List/Struct/Enum/Option/Result
codec 和 C export。生成物通过 `calcit-native-ffi` 处理 decode/encode failure、panic
隔离和 buffer ownership，不复制 ABI 常量。async、callback、resource
ownership/cancel/lifecycle 会明确失败，不通过 Dynamic fallback 假装支持。
package 生成的 `<Package>Ffi` service trait 名属于保留 Rust type 名；declaration 映射到同名时 generation
会在写文件前明确失败。

Calcit backend 生成 nominal `FfiClient`、带静态签名的 trait 和 impl；调用方通过
`client .method` 使用绑定，不直接保存 native symbol 字符串。TypeScript declaration 名使用完整
namespace-qualified declaration ID 派生，避免不同 namespace 的同名 nominal declaration 被折叠。
WIT 将 Calcit Buffer 严格映射为 `list<u8>`，将 Calcit Number 映射为当前 Component Model
的 `f64`，并将明确的有/无符号 8/16/32/64 位整数与 Float32/Float64 一一映射为 WIT 数值类型，不从名称或样例值猜测宽度；再从同一闭合类型树递归生成 `list<T>`、`option<T>` 与 `result<T,E>`。单态 Struct declaration 生成 namespace-qualified named record；闭合单态 Enum 生成 namespace-qualified WIT variant，无、单个和多个 payload 分别映射为无 payload case、直接 payload 与 tuple。字段与 case 的顺序、名称和嵌套类型严格来自 contract；Unit result 省略返回类型，Result 的 Unit 分支使用 WIT 省略 payload 语法。不猜测 Dynamic、异构成员或 anonymous/open variant。只生成严格可表示的 monomorphic subset；Unit field/parameter/Enum payload、generic declaration/application
等失败会包含精确的 definition/declaration type path。CI 使用 Bytecode Alliance `wit-parser`，发布前
同时用 Wasmtime 运行 Bool/Buffer/Number/String、明确宽度数值、递归 List、Option/Result、Struct、Enum、Unit、混合签名与 host import smoke，并用 jco
转译后在 Node.js 再运行同一语义。Option/Result smoke 覆盖全部分支、Unit payload、嵌套 List 与 post-return；List smoke 覆盖空列表、嵌套空列表及 Bool/Number/String/Buffer item；Buffer 覆盖内嵌零与非 UTF-8 字节，不经过 String 转码。

### Backend capability matrix

| 能力 / Capability | Rust | Calcit | TypeScript | WIT |
| --- | --- | --- | --- | --- |
| Unit/Bool/Number/String/Buffer/List | yes | typed method schema | yes | yes, except Unit value positions |
| Struct/Option/Result | codecs | qualified schema references | qualified generated names | monomorphic yes |
| Enum | codecs | qualified schema references | qualified generated names | closed monomorphic variants |
| Generic declarations | yes | applied callable references | yes | unsupported |
| `native + sync + edn-buffer-v1` | yes | yes | declaration view | interface view |
| async WIT / async Canonical ABI lifecycle | unsupported | unsupported | unsupported | declaration、真实 core 打包/执行、lifecycle surface 校验与 manifest 记录；不生成 wiring |

非目标包括猜测 Dynamic、把 resource 伪装成 Struct、生成双向 Component bindings，以及在本仓库
重新定义 Calcit Interface IR 或 native ABI。当前 async 支持包括 WIT declaration 渲染，以及打包并执行
调用方提供的、已实现 async Canonical ABI wiring 的 core module。本工具不生成该 wiring，也不保证缺少
该 wiring 的 core module 可运行；但会在写入前验证完整 Canonical ABI，并把 callback、取消、post-return、
stream/resource drop 等 lifecycle surface 及函数签名写入 ownership manifest，供 `check` 检测漂移。

消费 crate 需要依赖 `calcit_native_ffi = "0.1.3"` 和 `cirru_edn = "0.8.2"`，在 crate
根部 `include!` 生成文件，实现其中的 package service trait，然后调用生成的
`export_<package>_ffi!(SERVICE)` macro。生成目录是整体托管产物，不要手改
`rust/bindings.rs`。

路线图：[calcit#544](https://github.com/calcit-lang/calcit/issues/544)

WASI 0.3 command 的固定 WIT 与封装入口由 `package_wasi_command(core_module)` Rust API 提供。
它校验并封装已实现 `wasi:cli/run@0.3.0` Canonical ABI 的 core module，明确拒绝
`wasi_snapshot_preview1` import；不会自动为 Calcit 代码生成参数、标准流等 lowering。
Calcit 的 `calcit wasi --boundary component` 负责生成对应 core module 并调用本 API；
当前 Calcit 端仅支持零参数纯计算入口。WIT 来源与校验摘要见
[`src/wasi_wit/README.md`](src/wasi_wit/README.md)。

## English

This crate is independent of Calcit core and strictly consumes versioned
Interface IR emitted by `calcit ffi export`. Component contracts default to
Cirru EDN; use `--format json` only when interoperating with JSON-only tooling.
Native v2/v3 envelopes/documents are JSON-compatible and support compatibility
diffs before generation. Unknown versions, missing declarations, nominal
kind/arity mismatches, and non-monomorphic callables fail explicitly.

```bash
# Cirru EDN is the default Component contract format.
calcit project/calcit.cirru ffi export --boundary component > component-interface.cirru
calcit project/calcit.cirru wasm --boundary component --emit-path target/component-core
cargo run -- generate component-interface.cirru \
  --core-module target/component-core/program.wasm \
  --out generated-component
cargo run -- check component-interface.cirru \
  --core-module target/component-core/program.wasm \
  --out generated-component
```

For a `calcit ffi export --json` envelope, validation also checks the envelope
schema ID, dependency filter, summary counts, complete structured diagnostics,
and the core-produced revision digest. Raw Interface IR v2/v3 documents remain a
supported input. Both entry forms reject unknown fields in line with the public
JSON schema. `filters.namespace` is required by v1 and must be `null` when no
filter is active. The loader validates envelope metadata before extracting and
returning the embedded `Document`; that return value does not retain the
envelope metadata.

`diff` classifies added definitions/declarations as additive. Removing or
changing an existing contract is breaking and exits non-zero, making the
command suitable for CI gates.

Compatibility covers only the public contract consumed by generated code and
call boundaries: package identity, nominal declaration shape, supported
definition signatures/status, and backend/target/kind/symbol/invoke/transport
lowering. Package versions, documentation, display-only `logical_schema`,
`lowering.raw`, and diagnostic metadata do not cause ABI-breaking reports;
future stale-artifact checks own those regeneration concerns. Enabling a
previously unsupported definition is additive, while disabling a supported
definition is breaking. Reports use deterministic field-level paths.

`generate` writes canonical `interface.json`, compilable `rust/bindings.rs`,
method-oriented `calcit/bindings.cirru`, namespace-qualified
`typescript/bindings.d.ts`, strict-subset `wit/interface.wit`, and a versioned
ownership manifest. The manifest records the enabled backend set, generator,
IR/package identity, deterministic contract digest, and managed artifact
digests. Repeat `--backend` to select targets; `check` uses the same set and
guards every selected artifact together.

The manifest marks the output as a dedicated calcit-bindgen directory. Initial
generation never replaces an existing unowned directory, and later runs refuse
to remove files outside the previous manifest. A complete staged directory is
committed by an atomic same-filesystem rename. `check` is read-only and reports
missing, modified, stale-manifest, and unexpected artifacts separately for CI.

Component contracts reuse the same `validate`, `generate`, and `check` entry
points. Generation additionally requires `--core-module <program.wasm>` and
currently accepts monomorphic Bool/Buffer/Number/String, recursively homogeneous `List<T>`, closed Option/Result, monomorphic Struct records, closed monomorphic Enum variants, and Unit-result definitions. It
emits canonical
`interface.json`, `wit/interface.wit`, a runnable `component/component.wasm`,
and a manifest containing both contract and core-module digests. Core imports,
exports, memory, `cabi_realloc`, and Canonical ABI signatures are checked before
the managed output directory is created or replaced. `check` deterministically
re-encodes the component without modifying the output directory.
Component v3 `invocation` is validated explicitly. `sync` emits ordinary
`func` declarations while `async` emits native WASI 0.3 `async func`
declarations. Packaging still requires the core module to implement the matching
Canonical ABI and fails before writing artifacts when async builtin wiring is missing.
The in-repository Wasmtime 47 acceptance invokes an async export, typed Result
success/error, and a concurrent async host import that suspends at least once
before resuming through the waitable lifecycle. CI also packages real core
modules produced by Calcit 0.18.0 and verifies active cancellation and callback
races, exactly-once completion, synchronous post-return reuse, and stream
backpressure, cancel-read, and drop. Calcit core owns that lowering; bindgen
validates and packages its public contract without duplicating it. The manifest
records lifecycle imports/exports and their function signatures.

When the contract contains the exact closed `calcit:wasi-http/client.request`
interface, generation also owns `rust/wasmtime_http_adapter.rs` and a runnable
`rust/wasmtime-http-host/` crate. Both stay under the existing `generate`,
`check`, and manifest lifecycle, without another command. The adapter is
backed by the stable WASI 0.2 HTTP transport in Wasmtime 47, denies all origins
and preopened directories by default, and combines the host response limit with
the explicit per-request limit. Its existing Cirru EDN configuration also
decodes `arguments` strictly from the Component function type and writes typed
results as Cirru EDN to stdout. Optional `arguments-file` and `result-file`
paths reuse explicit preopens for the same Cirru EDN values, reject host paths
and traversal, and preserve stable exits for the closed HTTP failures.
Zero-parameter Unit entries remain compatible.
See the Chinese
[WASI HTTP guide](docs/wasi-http.md) for integration details.

The Rust backend accepts only `native + sync + edn-buffer-v1`. It emits
namespace-qualified Rust names, a typed service trait, codecs for the strict
Unit/Bool/Number/String/Buffer/List/Struct/Enum/Option/Result subset, and C
exports. `calcit-native-ffi` remains responsible for failure/panic isolation
and buffer ownership; generated code does not copy ABI constants. Async,
callback, and resource ownership/cancel/lifecycle fail explicitly, with no
Dynamic fallback.
The generated `<Package>Ffi` service trait is a reserved Rust type name. Generation fails before writing when a
declaration maps to that same name.

The Calcit backend exposes a nominal client with typed trait methods, so callers
do not hand-write native symbol strings. TypeScript names derive from complete
namespace-qualified declaration IDs. WIT accepts only its monomorphic,
representable subset, derives recursive `list<T>`, `option<T>`, and `result<T,E>` only from closed schemas, emits namespace-qualified named records for monomorphic Struct declarations, and emits namespace-qualified variants for closed monomorphic Enums. Zero, single, and multiple payloads map to payload-free cases, direct payloads, and tuples while preserving contract order and names. Unsupported Unit value positions or Enum payloads, generics, and open shapes report precise definition/declaration type paths. CI parses WIT
with Bytecode Alliance `wit-parser`, executes Bool/Buffer/Number/String,
explicit numeric-width, recursive-List, Option/Result, Struct, Enum, Unit,
mixed-signature, and host-import smokes in Wasmtime, and transpiles and executes
the same Component through jco
and Node.js. Calcit Buffer maps strictly to `list<u8>` and preserves empty,
embedded-zero, and non-UTF-8 byte sequences without String transcoding. Calcit
Number maps to current WIT `f64`; explicit integer and float refinements map
one-to-one without name- or sample-based inference.
Recursive List smokes cover empty and nested-empty lists plus Bool, Number,
String, and Buffer items without a Dynamic fallback.

The capability matrix above is normative for the current MVP. Async WIT
declaration rendering plus packaging and execution of caller-supplied core
modules with complete async Canonical ABI wiring are supported. This tool does
not generate that wiring or make core modules runnable when it is absent. It
does validate and record callback, cancellation, post-return, and stream/resource
drop surfaces supplied by the core module. Generating that lifecycle wiring,
Dynamic guessing, bidirectional Component bindings, and ownership of the
Interface IR or native ABI remain explicit non-goals.

Consumer crates depend on `calcit_native_ffi = "0.1.3"` and
`cirru_edn = "0.8.2"`, `include!` the generated file at crate root, implement
its package service trait, and invoke the generated
`export_<package>_ffi!(SERVICE)` macro. Treat the generated directory as a
managed artifact and do not edit `rust/bindings.rs` by hand.

Roadmap: [calcit#544](https://github.com/calcit-lang/calcit/issues/544)
