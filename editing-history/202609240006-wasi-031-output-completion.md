# WASI 输出 completion 回归 / WASI output completion regression

2026-09-24 00:06 +0800

## 中文

扩展 #57 的正式 Wasmtime 49 回归：1 MiB 标准输出在关闭 writable stream 后读取 `future<result<_,error-code>>`，确认成功结果再释放 future；另以关闭的 stdout pipe 验证输出失败不能静默返回成功。这是 Calcit #1266 的 ABI 证据，不扩展 bindgen API。

## English

Extend the released-Wasmtime-49 regression from #57: after a 1 MiB stdout write and writable-stream close, read `future<result<_,error-code>>`, verify success, then drop the future. A closed stdout pipe also must not silently report success. This is ABI evidence for Calcit #1266, with no bindgen API change.
