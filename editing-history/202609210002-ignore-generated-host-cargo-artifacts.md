# Ignore generated-host Cargo artifacts / 忽略生成宿主的 Cargo 产物

## 中文

- generated Wasmtime host 运行后，只把其精确目录下的 `Cargo.lock` 与 `target/` 识别为可丢弃的 Cargo 运行产物。
- `check` 不再因正常 `cargo run` 误报，`generate` 可以安全重建同一受管输出；其他未知文件仍会阻止覆盖。
- 回归测试同时固定可重复 check/generate 与未知用户文件保护，生成 README 明确 capability 配置必须留在受管目录之外。

## English

- Recognize only the generated Wasmtime host's exact `Cargo.lock` and `target/` paths as disposable Cargo runtime artifacts after execution.
- Keep `check` current after a normal `cargo run` and allow `generate` to rebuild the managed output while every other unknown file still blocks replacement.
- Cover repeatable check/generate and unknown user-file protection together, and document that capability configuration stays outside the managed directory.
