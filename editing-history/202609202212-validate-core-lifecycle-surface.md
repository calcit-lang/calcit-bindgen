# 2026-09-20 验证 core lifecycle surface

- ownership manifest 升级到 schema 4，记录 core module 的 lifecycle imports、exports 与精确函数签名。
- 保持 Canonical ABI callback lowering 由 Calcit core 生成；bindgen 只验证、打包并检测 contract drift。
- CI 固定 Calcit 0.18.0，验证主动取消、竞态、exactly-once completion、post-return 与 stream drop，
  再用同一批真实 core 验证当前 bindgen 打包结果。
- 增加 lifecycle 签名漂移的原子失败回归，确保 `generate` 和只读 `check` 不替换已有产物。
