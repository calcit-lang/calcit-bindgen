# 2026-09-20 可运行的 Wasmtime HTTP 宿主

- 在闭合 buffered HTTP contract 的现有 `generate` / `check` 产物中加入可运行的
  `rust/wasmtime-http-host/` crate，不增加新的顶层命令。
- 增加 Cirru EDN capability 配置；网络和文件系统默认拒绝，仅允许逐项 origin 与
  `:read` / `:read-write` preopen，并拒绝未知字段。
- 宿主只调用零参数、返回 Unit 的 export，避免动态参数 codec；WASI 与 HTTP linker
  由生成脚手架统一接线。
- 增加宿主级 response 上限，使其与 guest 请求上限取较小值，并补充真实 HTTP 回归测试、
  managed/user-owned 边界和运行文档。
