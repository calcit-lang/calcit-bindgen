# 校验 v3 生命周期 IR / Validate v3 lifecycle IR

## 中文

- 在不改变同步生成能力的前提下，让 bindgen 同时读取 Interface IR v2 与 v3。
- v3 将 async stream 的 callback event、Unit callback result、cooperative cancel 与
  owned task result 表示为结构化 contract；resource 将 opaque-resource-v1、constructor
  的 own result 和 method 的 borrow/clone 参数表示为结构化 contract。
- validation 拒绝 v2 中的 lifecycle 字段、stream/resource 混用、错误的 native
  invoke/transport/kind、非 cooperative cancel、非 owned task、无协议资源、重复参数
  和未定义的 consuming input ownership；在生成 adapter 与 conformance vectors 到位前，
  lifecycle definition 不得宣称 `supported`。
- 即使 adapter 尚未生成，compatibility diff 也把 lifecycle metadata 修改视为 breaking，
  防止 future backend 支持前 contract drift 被忽略。

## English

- Let bindgen read both Interface IR v2 and v3 without changing synchronous
  generation capability.
- V3 models an async stream's callback event, Unit callback result, cooperative
  cancellation, and owned task result as structured contract; resources model
  opaque-resource-v1, an owned constructor result, and borrowed/cloned method
  parameters.
- Validation rejects lifecycle fields in v2, mixed stream/resource metadata,
  invalid native invoke/transport/kind, non-cooperative cancellation, unowned
  tasks, missing resource protocol, duplicate parameters, and unspecified
  consuming input ownership. Lifecycle definitions cannot claim `supported`
  until generated adapters and conformance vectors exist.
- Compatibility diff treats lifecycle metadata changes as breaking even before
  adapters are generated, preventing contract drift from being ignored.
