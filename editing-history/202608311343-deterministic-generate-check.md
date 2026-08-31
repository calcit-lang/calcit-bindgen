# Deterministic generate/check transaction

## 中文

- 增加 `generate`/`check` CLI 与公开 library API。
- 以稳定排序的 Interface IR v2 snapshot 作为第一个真实 managed artifact。
- 增加 versioned manifest、FNV-1a 128 contract/file digests 和稳定 JSON。
- generation 使用同文件系统 staging 与目录级切换；拒绝覆盖未托管目录或删除 unexpected files。
- check 保持只读并区分 missing、modified、stale manifest 和 unexpected artifacts。

## English

- Added generate/check CLI commands and library APIs.
- Made a canonical Interface IR v2 snapshot the first concrete managed artifact.
- Added a versioned manifest with deterministic FNV-1a 128 contract/file digests.
- Staged complete output on the same filesystem and atomically swapped owned directories while preserving unmanaged files.
- Kept checks read-only with distinct missing, modified, stale-manifest, and unexpected diagnostics.
