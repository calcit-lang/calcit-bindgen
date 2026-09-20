# Pin the HTTP-only CI example / 固定仅 HTTP 的 CI 示例

## 中文

- 真实 generated-host 验收改为从 Calcit 0.18.0 的 `examples/wasi-http-client` 单独生成 Component。
- 不再复用包含额外 `host.combine` import 的综合 async-import fixture，确保测试只验证生产 HTTP contract 与 generated host。

## English

- Generate the real generated-host acceptance Component directly from Calcit 0.18.0 `examples/wasi-http-client`.
- Stop reusing the aggregate async-import fixture with its extra `host.combine` import so the test isolates the production HTTP contract and generated host.
