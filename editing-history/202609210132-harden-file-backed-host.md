# Harden file-backed host access / 加固文件化宿主访问

## 中文

- 文件输入改为最多读取 4 MiB 加一个字节，避免并发增长的文件在拒绝前造成无界临时内存占用。
- 通过 preopen 目录句柄相对打开输入与结果文件，最终路径禁止跟随符号链接，并将已验证的结果文件句柄保留到 Component 执行结束，消除检查与写入之间的路径替换竞态。
- 文件系统权限错误稳定映射到 capability-denied；缺失或格式错误的输入仍映射到 invalid-input。
- transport 回归测试保留监听端口直至连接，并增加结果句柄不可重定向及权限错误分类测试。

## English

- Bound argument-file reads to 4 MiB plus one byte so a concurrently growing file cannot cause unbounded transient allocation before rejection.
- Open input and result files relative to a preopen directory handle, disallow following the final symlink, and retain the validated result handle until Component execution completes to remove the check/write replacement race.
- Map filesystem permission failures consistently to capability-denied while missing or malformed input remains invalid-input.
- Keep the transport test listener owned until connection and add regressions for result-handle redirection and permission classification.
