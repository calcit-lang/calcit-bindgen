# 2026-09-20 修正生成宿主的 Component 路径

- 将 capability 示例中的相对路径修正为从 `rust/wasmtime-http-host/` 返回生成目录根部，再进入
  `component/component.wasm`。
- 用单元测试固定该目录关系，避免可运行示例再次指向 generated output 之外。
