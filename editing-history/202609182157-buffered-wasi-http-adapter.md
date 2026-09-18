# 2026-09-18 buffered WASI HTTP adapter

- 为闭合的 `calcit:wasi-http/client.request` contract 增加严格 shape 守门，并在现有
  Component `generate/check` 产物中纳入 Wasmtime adapter 接线文件。
- 增加 opt-in `wasmtime-http` runtime adapter，默认拒绝全部 origin，以显式 capability、
  transport timeout 和 `max-response-bytes` 提供有界 client。
- 使用 Wasmtime 47 中稳定的 WASI 0.2 HTTP 生产传输，保留 Calcit-facing WASI 0.3 async
  contract；不暴露 resource、stream、pollable 或 socket。
- 增加真实回环 HTTP、capability denied、unsupported、response-too-large 和 async Component
  linker 类型验收，并补充中文接入文档。
