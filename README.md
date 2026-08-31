# calcit-bindgen

Calcit FFI Interface IR 的确定性 production generator 与兼容性守门工具。

Deterministic production generation and compatibility gates for Calcit FFI
Interface IR.

## 中文

该 crate 独立于 Calcit core，严格消费 `calcit ffi export --json` 产生的版本化
Interface IR。当前第一段实现提供 v2 envelope/document 校验和兼容性 diff，确保
未知版本、缺失 declaration、错误 nominal kind/arity、非 monomorphic callable 在
进入生成器前失败。

```bash
calcit project/calcit.cirru ffi export --json > interface.json
cargo run -- validate interface.json
cargo run -- diff previous.json interface.json --json
```

`diff` 将新增 definition/declaration 标记为 additive；删除或修改现有 contract
标记为 breaking，并以非零状态退出，适合 CI 守门。

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

The next slice migrates deterministic Rust/Calcit/TypeScript/WIT generation,
stale-artifact checks, and compilable typed `sync + edn-buffer-v1` adapters into
this crate. Async, callback, and resource ownership/cancel/lifecycle remain
explicitly unsupported until structured IR is available; no Dynamic fallback
is generated.

Roadmap: [calcit#544](https://github.com/calcit-lang/calcit/issues/544)
