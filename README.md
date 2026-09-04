# calcit-bindgen

Calcit FFI Interface IR 的确定性 production generator 与兼容性守门工具。

Deterministic production generation and compatibility gates for Calcit FFI
Interface IR.

## Status / 状态

本仓库处于 **active development / experimental tooling** 阶段。v2 validation、
compatibility diff、canonical generate/check，以及严格同步 Rust、Calcit、TypeScript、WIT
backends 已可用；在更多真实模块迁移和 core cutover 完成前，尚不替代 Calcit core 的 preview generator。公开、
版本化的 Interface IR 由 Calcit core 定义；本工具独立发布，production MVP 由
[calcit-bindgen#5](https://github.com/calcit-lang/calcit-bindgen/issues/5) 追踪。

This repository is active experimental tooling. Validation, compatibility diff,
canonical generate/check, and strict synchronous Rust, Calcit, TypeScript, and
WIT backends are usable. It does not replace the core preview generator until
real-module migration and the core cutover pass.
Calcit core owns the versioned Interface IR contract; this tool has an
independent release cadence tracked by calcit-bindgen#5.

## 中文

该 crate 独立于 Calcit core，严格消费 `calcit ffi export --json` 产生的版本化
Interface IR。当前第一段实现提供 v2 envelope/document 校验和兼容性 diff，确保
未知版本、缺失 declaration、错误 nominal kind/arity、非 monomorphic callable 在
进入生成器前失败。

输入为 `calcit ffi export --json` envelope 时，还会校验 envelope schema ID、依赖
过滤条件、summary 计数、完整结构化 diagnostics 与 core 生成的 revision digest。
裸 Interface IR v2 document 仍是受支持输入，但 envelope metadata 不再被静默丢弃。

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
WIT 只生成严格可表示的 monomorphic subset；Unit field/parameter、generic declaration/application
等失败会包含精确的 definition/declaration type path。CI 使用 Bytecode Alliance `wit-parser`，发布前
同时以 `wasm-tools component wit generated/wit/interface.wit` 抽样验证。

### Backend capability matrix

| 能力 / Capability | Rust | Calcit | TypeScript | WIT |
| --- | --- | --- | --- | --- |
| Unit/Bool/Number/String/Buffer/List | yes | typed method schema | yes | yes, except Unit value positions |
| Struct/Enum/Option/Result | codecs | qualified schema references | qualified generated names | monomorphic only |
| Generic declarations | yes | applied callable references | yes | unsupported |
| `native + sync + edn-buffer-v1` | yes | yes | declaration view | interface view |
| async/callback/resource lifecycle | unsupported | unsupported | unsupported | unsupported |

非目标包括猜测 Dynamic、把 resource 伪装成 Struct、生成双向 Component bindings，以及在本仓库
重新定义 Calcit Interface IR 或 native ABI。

消费 crate 需要依赖 `calcit_native_ffi = "0.1.3"` 和 `cirru_edn = "0.8.0"`，在 crate
根部 `include!` 生成文件，实现其中的 package service trait，然后调用生成的
`export_<package>_ffi!(SERVICE)` macro。生成目录是整体托管产物，不要手改
`rust/bindings.rs`。

路线图：[calcit#544](https://github.com/calcit-lang/calcit/issues/544)

## English

This crate is independent of Calcit core and strictly consumes versioned
Interface IR emitted by `calcit ffi export --json`. The initial slice validates
v2 envelopes/documents and reports compatibility changes before generation.
Unknown versions, missing declarations, nominal kind/arity mismatches, and
non-monomorphic callables fail explicitly.

For a `calcit ffi export --json` envelope, validation also checks the envelope
schema ID, dependency filter, summary counts, complete structured diagnostics,
and the core-produced revision digest. Raw Interface IR v2 documents remain a
supported input, but envelope metadata is never silently discarded.

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
representable subset and reports precise definition/declaration type paths for
Unit value positions, generics, and other unsupported shapes. CI parses WIT
with Bytecode Alliance `wit-parser`; release evidence also runs `wasm-tools
component wit`.

The capability matrix above is normative for the current MVP. Async, callback,
resource lifecycle, Dynamic guessing, bidirectional Component bindings, and
ownership of the Interface IR or native ABI are explicit non-goals.

Consumer crates depend on `calcit_native_ffi = "0.1.3"` and
`cirru_edn = "0.8.0"`, `include!` the generated file at crate root, implement
its package service trait, and invoke the generated
`export_<package>_ffi!(SERVICE)` macro. Treat the generated directory as a
managed artifact and do not edit `rust/bindings.rs` by hand.

Roadmap: [calcit#544](https://github.com/calcit-lang/calcit/issues/544)
