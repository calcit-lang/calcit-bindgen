# 2026-08-31 14:53 — sync edn-buffer Rust adapter

## 中文

- `generate` 新增托管的 `rust/bindings.rs`，只接受严格 `native + sync + edn-buffer-v1` lowering。
- namespace-qualified declaration/definition identity 映射为确定性 Rust 名称；映射碰撞、非法 ABI export symbol 和非同步 transport 在写文件前失败。
- 生成 typed service trait、Unit/Bool/Number/String/Buffer/List/Struct/Enum/Option/Result codec 与 package export macro。
- C ABI、panic/error 隔离和 buffer ownership 统一复用 `calcit-native-ffi`，生成物不含 `todo!`、Dynamic fallback 或复制的 ABI 常量。
- 新增 calcit.std `md5` fixture：生成临时 cdylib、离线编译、动态加载真实 symbol，并执行 request/response/free runtime smoke。

## English

- `generate` now manages `rust/bindings.rs` and accepts only strict `native + sync + edn-buffer-v1` lowerings.
- Namespace-qualified identities map to deterministic Rust names; collisions, invalid ABI export symbols, and non-sync transports fail before writes.
- Generated code includes a typed service trait, codecs for the strict supported type subset, and a package export macro.
- C ABI, panic/error isolation, and buffer ownership stay centralized in `calcit-native-ffi`; output contains no `todo!`, Dynamic fallback, or copied ABI constants.
- A calcit.std `md5` fixture builds an offline temporary cdylib, dynamically loads real symbols, and exercises request/response/free at runtime.
