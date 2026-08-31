# 保留生成的 Rust service 名 / Reserve the generated Rust service name

## 中文

- 在校验 Interface IR declaration 前，将 package 派生的 `<Package>Ffi` service trait 名加入 Rust type
  保留集合。
- declaration 映射到同一 Rust 名时沿用确定性的 type collision 诊断，并在任何生成物写入前失败。
- 增加 `demo/Ffi` 与 `DemoFfi` service trait 冲突的回归测试，并在 README 记录该名称边界。

## English

- Reserve the package-derived `<Package>Ffi` service trait before validating Interface IR declarations.
- Reuse the deterministic Rust type-collision diagnostic and fail before writing any generated artifact when a
  declaration maps to that reserved name.
- Cover the `demo/Ffi` versus `DemoFfi` service-trait collision and document the naming boundary in README.
