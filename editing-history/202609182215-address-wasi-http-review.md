# 2026-09-18 address WASI HTTP review

- 让闭合 HTTP contract 的 record field 与 variant case 按声明顺序比较，拒绝会改变
  Component ABI layout 或 discriminant 的重排。
- 按 RFC 语义以 ASCII case-insensitive media type 判定 text/JSON response。
- 回环 HTTP 测试读取完整 request line，避免 TCP 分段造成偶发失败。
