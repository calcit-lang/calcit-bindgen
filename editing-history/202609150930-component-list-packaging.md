# Package recursive Component Lists / 打包递归 Component List

## 中文

- 让 Component WIT backend 从闭合 Interface IR item schema 递归生成 `list<T>`，不引入 Dynamic fallback 或第二套类型判断。
- 用 Calcit #1110 的精确提交刷新 Component contract fixture，并将 CI 的真实 core checkout 固定到同一提交。
- 扩展合成 core、Wasmtime 与 jco smoke，覆盖 Bool/Number/String/Buffer List、嵌套 Number List、空列表及 host Number List import。
- 更新中英文 README，明确 List 仍复用 `validate`、`generate`、`check`，没有新增工具入口。

## English

- Render recursive `list<T>` WIT directly from closed Component Interface IR item schemas, without a Dynamic fallback or a second type rule set.
- Refresh the Component contract fixture from the exact Calcit #1110 commit and pin the real-core CI checkout to that same commit.
- Extend the synthetic core, Wasmtime, and jco smokes across Bool/Number/String/Buffer lists, nested Number lists, empty lists, and a host Number-list import.
- Update the bilingual README to keep `validate`, `generate`, and `check` as the only entry points.

## Verification / 验证

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- ignored real Calcit Component test against Calcit `937f1621232534b32e5b26dbe7ccea9f236ccf14`
- jco 1.16.1 transpile and Node.js smoke
- `cargo package`
