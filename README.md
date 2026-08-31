# calcit-bindgen

Calcit FFI Interface IR 的确定性 production generator 与兼容性守门工具。

Deterministic production generation and compatibility gates for Calcit FFI
Interface IR.

## Status / 状态

本仓库处于 **active development / experimental tooling** 阶段。v2 validation、
compatibility diff 以及 canonical generate/check baseline 可用，但在 Rust/Calcit/TS/WIT
backends 和真实模块 smoke 完成前，尚不替代 Calcit core 的 preview generator。公开、
版本化的 Interface IR 由 Calcit core 定义；本工具独立发布，production MVP 由
[calcit-bindgen#5](https://github.com/calcit-lang/calcit-bindgen/issues/5) 追踪。

This repository is active experimental tooling. Validation, compatibility diff,
and the canonical generate/check baseline are usable, but it does not replace
the core preview generator until concrete backends and real-module smokes pass.
Calcit core owns the versioned Interface IR contract; this tool has an
independent release cadence tracked by calcit-bindgen#5.

## 中文

该 crate 独立于 Calcit core，严格消费 `calcit ffi export --json` 产生的版本化
Interface IR。当前第一段实现提供 v2 envelope/document 校验和兼容性 diff，确保
未知版本、缺失 declaration、错误 nominal kind/arity、非 monomorphic callable 在
进入生成器前失败。

```bash
calcit project/calcit.cirru ffi export --json > interface.json
cargo run -- validate interface.json
cargo run -- diff previous.json interface.json --json
cargo run -- generate interface.json --out generated
cargo run -- check interface.json --out generated
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

`generate` 当前产生规范化、稳定排序的 `interface.json` 和版本化
`calcit-bindgen.manifest.json`。它们是后续 Rust/Calcit/TypeScript/WIT backend
共享的真实 compatibility baseline，不是 placeholder binding。manifest 记录 generator、
IR/package identity、确定性 contract digest 和 managed artifact digest。

输出目录由 manifest 明确标记为 `calcit-bindgen` 专用目录。首次生成不会覆盖已有的
未托管目录；后续生成如果发现 manifest 之外的文件也会拒绝删除。完整输出先写入同文件系统
临时目录，再以目录切换提交。`check` 完全只读，并分别报告 missing、modified、
stale-manifest 与 unexpected artifacts，适合直接用于 CI。

下一段在同一 crate 中迁移 deterministic Rust/Calcit/TypeScript/WIT generation、
stale artifact check 与可编译的 `sync + edn-buffer-v1` typed adapter。async、
callback、resource ownership/cancel/lifecycle 在结构化 IR 就绪前保持明确
unsupported，不通过 Dynamic fallback 假装支持。

路线图：[calcit#544](https://github.com/calcit-lang/calcit/issues/544)

## English

This crate is independent of Calcit core and strictly consumes versioned
Interface IR emitted by `calcit ffi export --json`. The initial slice validates
v2 envelopes/documents and reports compatibility changes before generation.
Unknown versions, missing declarations, nominal kind/arity mismatches, and
non-monomorphic callables fail explicitly.

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

`generate` currently writes a canonical, deterministically ordered
`interface.json` plus a versioned `calcit-bindgen.manifest.json`. This is the
real compatibility baseline shared by future Rust/Calcit/TypeScript/WIT
backends, not a placeholder binding. The manifest records generator,
IR/package identity, a deterministic contract digest, and managed artifact
digests.

The manifest marks the output as a dedicated calcit-bindgen directory. Initial
generation never replaces an existing unowned directory, and later runs refuse
to remove files outside the previous manifest. A complete staged directory is
committed by an atomic same-filesystem rename. `check` is read-only and reports
missing, modified, stale-manifest, and unexpected artifacts separately for CI.

The next slice migrates deterministic Rust/Calcit/TypeScript/WIT generation,
stale-artifact checks, and compilable typed `sync + edn-buffer-v1` adapters into
this crate. Async, callback, and resource ownership/cancel/lifecycle remain
explicitly unsupported until structured IR is available; no Dynamic fallback
is generated.

Roadmap: [calcit#544](https://github.com/calcit-lang/calcit/issues/544)
