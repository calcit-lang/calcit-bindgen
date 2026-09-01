# Revert unused v3 lifecycle contract / 撤回未使用的 v3 生命周期契约

## 中文

- 撤回 Interface IR v3 stream/resource lifecycle 数据模型、校验和 compatibility diff；Calcit core 的对应提案 PR #574 未合并并已关闭。
- bindgen 恢复为只接受当前 core 实际发布且被 calcit.std 使用的 Interface IR v2。
- async cancellation 与 resource handle 状态留在 calcit-wss、calcit-regex 或具体 adapter 内部，不要求 Calcit 用户声明 callback 下标或 owner/borrow/clone。
- 保留既有同步生成能力、历史记录与生产 MD5 adapter 回归；未来 IR 扩展必须与可运行 consumer 一起提交。

## English

- Revert the Interface IR v3 stream/resource lifecycle model, validation, and compatibility diff because the corresponding Calcit core proposal PR #574 was closed without merging.
- Restore bindgen to the released Interface IR v2 that core publishes and calcit.std consumes today.
- Keep async-cancellation and resource-handle state inside calcit-wss, calcit-regex, or concrete adapters instead of requiring callback indexes or owner/borrow/clone metadata from Calcit users.
- Preserve existing synchronous generation, historical notes, and the production MD5 adapter regression; future IR extensions must ship together with a working consumer.
