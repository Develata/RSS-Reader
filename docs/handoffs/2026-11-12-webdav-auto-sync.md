# WebDAV 自动同步

- 日期：2026-11-12
- 作者 / Agent：Claude（Opus）
- 分支：`claude/webdav-auto-sync`
- 当前 HEAD：见分支
- 相关 commit：`fcd356fa`（实现）
- 相关 tag / release：N/A
- 状态：`draft`（独立评审进行中）

## 工作摘要

设置页可以“记住并自动同步”一个 WebDAV 端点：程序启动后（首轮自动刷新完成时）与修改配置后，按基线规则自动上传或下载配置包；两边都改过时不合并，交给用户手动选择。范围仍是“基础配置交换”，不同步文章、已读、收藏。设计见 [WebDAV 自动同步](../design/webdav-auto-sync.md)，方向由 Develata 于当日确认（凭据先明文保存；整库复制另立“备份 / 恢复”入口，未在本次实现）。

## 影响范围

- 模块：
  - `rssr-application`：`import_export_service/remote_sync.rs`（规则表、内容哈希、档案类型）、`RemoteConfigStore` 的条件上传 / ETag 默认方法、替换返回事务内指纹
  - `rssr-infra`：`config_sync/profile_file.rs`（`remote-sync.json`）、WebDAV `If-Match` / `If-None-Match: *` / 强 ETag、`LocalBackend` 启动同步与修改后防抖同步、`native_local_backend_in`、`HttpBackend`
  - `rssr-api`：`Backend` 新增 `load/save/clear_remote_sync`、`startup_remote_sync`；推送 / 拉取改为 `RemoteTarget`；协议新增 `/api/v1/remote-sync`
  - `rssr-web`：新路由；服务器后端在数据目录保存档案
  - `rssr-app`：设置页 WebDAV 卡片（记住 / 停用 / 最近结果）、外壳启动同步提示、原生启动使用数据目录档案
- 平台：Windows / macOS / Linux / Android / 正式 Web（服务器）；Pages 演示不可用
- 额外影响：README、用户指南、部署文档（备份对象新增 `remote-sync.json`）、功能设计哲学条目

## 关键变更

### 规则（application）

- 本机指纹只在本机比较；远端内容哈希按订阅地址排序、忽略导出时间，跨设备可比。
- 远端不存在 → 上传（`If-None-Match: *`）；内容相同 → 只记基线；无基线且不同 → 提示首次选择；只远端变 → 下载；只本机变 → 上传（有强 ETag 时 `If-Match`，412 视为冲突）；两边都变 → 冲突。
- 下载后的基线取替换事务内的新指纹；修改后的同步在本机指纹等于基线时不联网。

### 宿主与界面

- 同一后端内的同步、保存 / 清除档案、手动上传下载经同一把锁串行。
- 原生 GUI 启动同步结束后：下载了配置则重读页面与设置并提示；冲突、需选择、失败给一条提示；上传与无变化不提示。正式 Web 的启动属于服务器，页面加载不提示。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test --workspace --locked`：通过（67 个测试目标，0 失败）；新增 `test_library_remote_sync`（6）、`local_backend::tests` 两例、`webdav_conditional_*` / `webdav_weak_entity_tags_are_not_used`、`profile_file` 两例、`remote_sync_text` 两例、同步状态一例；UI 特征转录只新增条目
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown --features pages-demo --locked`：通过
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：未执行（本机无 Android NDK，`ring` 找不到 `aarch64-linux-android-clang`）；CI 的 Android job 覆盖

### 手工验收

- 真实 WebDAV 服务（坚果云 / Nextcloud）的 ETag 与条件请求：未执行
- 桌面 / Android / 正式 Web 界面上的记住、停用、启动提示：未执行

## 结果

- 待独立评审结论后再判断是否可合并。

## 风险与后续事项

- 服务器不返回强 ETag 时，上传前后的竞争窗口内另一设备的上传可能被覆盖；返回 ETag 的服务器会识别为冲突。
- 凭据明文保存（Develata 决定）；以后可换系统钥匙串，档案接口不变。
- 同一数据目录只应由一个带档案的进程使用（桌面与服务器不要共用数据目录）。
- 下一步：“备份 / 恢复”入口单独写设计文档，第一步做本地整库文件导出与恢复（GUI + `rssr-cli restore`）。

## 给下一位 Agent 的备注

- 入口：`docs/design/webdav-auto-sync.md`，`ImportExportService::sync_remote_config`，`LocalBackend::run_remote_sync`。
- 改规则时跑 `cargo test -p rssr-infra --test test_library_remote_sync --locked` 与 `cargo test -p rssr-infra --lib local_backend --locked`。
