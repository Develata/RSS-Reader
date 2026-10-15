# 原生资料库的磁盘满验收

v0.2 阶段 A 的完成门禁之一（[v0.2 架构](../design/v0.2-architecture.md#阶段与门禁)）：磁盘写满时，资料库写入与旧库导入都不留下半成品，空间释放后照常工作。用例写满的是真实文件系统（写入返回 `ENOSPC`），不是用 SQLite 的页数上限模拟的。

## 用例

用例在 `crates/rssr-infra/tests/` 里，靠环境变量 `RSSR_DISK_FULL_DIR` 启用：它指向一个小文件系统上的目录。用例会把整个文件系统写满，所以必须是专门挂载的小文件系统，不能是日常使用的磁盘。没有设置这个变量时，用例打印 `skipped: …` 后直接通过；这种“通过”不算验收。

- `test_library_store` / `writes_on_a_full_disk_roll_back_and_resume`：
  - 资料库已有 4 篇文章；一次刷新开始后写满磁盘，再提交它带回的 32 篇文章：以 `storage` 类失败，整次刷新回滚（仍是 4 篇），`PRAGMA integrity_check` 为 `ok`。
  - 磁盘满时也不能开始新的刷新：领刷新序号本身是一次写入，同样以 `storage` 类失败。
  - 释放空间后同样的刷新成功；关闭后重新打开，数据完整。
- `test_library_import` / `an_import_onto_a_full_disk_publishes_nothing`：
  - 目标文件系统只剩约 16 KiB 时导入 v0.1 旧库：导入失败；没有发布资料库；目录里只剩填充文件和按设计保留的创建锁文件，没有暂存文件；旧库逐字节不变。
  - 释放空间后导入成功，资料库完整。

## 在 Linux（含 WSL）上运行

无需 root：在非特权的用户与挂载命名空间里挂一个 8 MiB 的 tmpfs，只对这条命令可见。

```bash
unshare --user --map-root-user --mount sh -c 'mkdir -p /tmp/rssr-df && mount -t tmpfs -o size=8m tmpfs /tmp/rssr-df && RSSR_DISK_FULL_DIR=/tmp/rssr-df cargo test -p rssr-infra --locked --test test_library_store --test test_library_import -- full_disk --nocapture'
```

在 WSL 中从 Windows 盘运行时，建议设置 `CARGO_TARGET_DIR` 指向 Linux 侧目录，避免与 Windows 的构建目录混用。

## 未覆盖

- Windows 与 macOS：没有不需要管理员权限就能挂载的小文件系统，用例在这些平台上跳过。SQLite 在三个平台上对 `SQLITE_FULL` 的处理相同，平台差异只在文件系统本身。
- Android 设备上的磁盘满未实测。
- 用户看到的是通用的“本地数据读写失败”，没有单独说明磁盘已满（错误分类见 [B2 契约](../design/v0.2-phase-b2-contracts.md) 的 D5）。
