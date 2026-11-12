# WebDAV 自动同步

- 日期：2026-11-12
- 作者 / Agent：Claude（Opus）
- 分支：`claude/webdav-auto-sync`
- 当前 HEAD：见分支
- 相关 commit：`fcd356fa`（实现）、`b39e3729`（交接）、`dfebd297`（首轮评审修复）、复审修复见下一提交
- 相关 tag / release：N/A
- 状态：`validated`（自动化验证；真实网盘与界面手工验收未做）

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

## 用户验收后的补充（2026-11-13）

- Mac 编辑快捷键：本分支早于 PR #72，已合并 `main`（`ece1d067`）。
- 用户名、密码单独输入：`rssr_application::endpoint_with_credentials` 把两栏按“字母数字以外全部百分号编码（含 `%`）”写进端点，WebDAV 客户端解码后原样还原；只填密码时拒绝。按 Develata 的决定只保留这一种格式：Endpoint 自带凭据即拒绝（功能未发布，旧写法只存在于会话里，无需迁移；CLI 的 `--endpoint` 未改）。密码框带显示／隐藏切换（`data-action="toggle-password-visibility"`），选择器文档已补。测试：`credentials_typed_apart_come_back_unchanged`、`credentials_typed_apart_reach_the_endpoint`。

- Penpot：4 张含 WebDAV 卡片的画板已按 `2aa905c5` 实测更新，rev 444，命名版本 `WebDAV auto sync & credential fields · 2aa905c5 · 2026-11-13`，完整性 0 错误；细节见 `docs/design/penpot-code-baseline.md` 开头。主题区（五张卡片、列表密度）与第 12 页主题实效仍是旧代码状态，未在本 PR 处理。

- 去掉 Remote Path：配置固定为 Endpoint 文件夹里的 `rss-reader.json`（`rssr_application::REMOTE_CONFIG_FILE`，`012ee973`）；无需先建子目录。CLI 的 `remote_path` 参数可省略、默认同名。v0.2.3 及更早默认写到 `config/rss-reader.json`，读旧文件时把 Endpoint 指向该 `config/` 文件夹。Endpoint 示例改为 `https://example.com/dav/path/`（`5b89d171`）。Penpot 4 张设置画板同步，rev 490，命名版本 `WebDAV fixed file name & endpoint example · 012ee973 · 2026-11-13` 已回读。

## 独立评审（Codex，第一轮，`fcd356fa`）

结论“不通过”。逐条核对源码后处理如下：

| # | 结论 | 处理 |
| --- | --- | --- |
| 1 指纹与上传内容不是同一快照（40→50→40 后两边各留一值却显示已同步） | 成立（blocker） | `ConfigSnapshot` 带同一读事务的指纹；下载后再取（包, 指纹）。回归 `an_edit_during_a_transfer_and_its_undo_both_reach_the_remote`；去掉修复后该测试失败（变异检查） |
| 2 未保存端点的手动操作绕过锁 | 成立 | 两种目标都进同一把锁 |
| 3 下载确认未绑定档案身份 | 成立 | `RemoteTarget::Saved` 携带页面看到的端点与路径，锁内比对不符即冲突；页面在端点变化时取消确认 |
| 4 `Debug` 输出密码 | 成立 | `RemoteEndpoint` / `RemoteSyncProfile` 自定义 `Debug`；`SaveRemoteSync`、`SettingsCommand` 改用 `RemoteEndpoint` |
| 5 无强 ETag 时有覆盖窗口 | 成立，属规格取舍 | 不声明 gzip（避免代理把强 ETag 变弱）；规格与指南写明限制；是否停用无强 ETag 时的自动上传留给 Develata 决定 |
| 6 旧读取覆盖新写入后的卡片 | 成立 | 页面档案读写分代：写一次一个，写开始后旧读取作废 |
| 7 修改后同步结果不回到打开的页面 | 成立 | 视图带 `syncing`；页面保存设置后读一次，进行中每 2 秒再读 |
| 8 档案损坏时无法停用 | 成立 | 视图带 `problem`，卡片显示原因并提供停用 |
| 9 基线写入失败仍报成功 | 成立 | 自动同步记为失败；手动操作返回 `storage` 错误 |
| 10 缺交接 | 已由 `b39e3729` 补 | — |
| 11 多实例共用数据目录 | 不在支持范围 | 规格与指南写明 |
| 12 测试覆盖 | 部分采纳 | 新增上列回归、失败路径、`Debug` 脱敏、`/api/v1/remote-sync` 经 `HttpBackend` 的往返（含保存端点上传、身份不符拒绝、停用后文件删除） |

## 独立评审（Codex，复审，`dfebd297`）

结论“通过 / 无 blocker”：首轮 1、2、3、8、10、11 已解决。其余与新增问题核对后均成立，处理如下：

| 项 | 处理 |
| --- | --- |
| N1 关掉 gzip 却没拒绝压缩响应 | 每个请求带 `Accept-Encoding: identity`；测试 `webdav_asks_for_the_identity_encoding`（去掉该头时失败） |
| N2 读与读乱序 | 每次读写都领新序号，只采纳最新请求；过期读取的失败也不显示 |
| N3 `syncing` 不可靠、轮询可叠加 | 启动同步由守卫结束（含 panic，测试 `the_start_never_stays_pending`）；修改后同步取得锁之后才清排队标记；保存档案只看排队状态；页面同时只有一个后续读取 |
| N4 写不进档案时把 `Pulled` 改成失败 | 结果保持原样，另标 `unrecorded`；外壳照常重读并以错误提示告知 |
| N5、R3 文档措辞 | 窗口改为“从读取远端到上传完成”；备份说明区分 `rssr-cli backup` 与整目录复制 |
| R1 其余 `Debug` | `WebDavCredentials`、设置页同步状态改为脱敏 |
| R2 写不进档案的结果看不到 | 后端在内存保留最近未写入的报告，`load_remote_sync` 优先显示，写入成功后清除 |
| V1 手动上传竞态测试无区分力 | 删去该段：手动上传一次调用同时取包与指纹，中间无可插入点 |

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test --workspace --locked`：通过（67 个测试目标，0 失败）；新增 `test_library_remote_sync`（8）、`local_backend::tests` 五例、`webdav_conditional_*` / `webdav_asks_for_the_identity_encoding` / `webdav_weak_entity_tags_are_not_used`、`profile_file` 两例、`remote_sync_text` 三例、同步页状态三例、`client_tests::the_saved_endpoint_through_the_client`；UI 特征转录只新增条目
- PR #74 首次 CI：`web-ui (amethyst-glass)` 的 `restored-draft-then-sign-in` 失败，原因是会话结束后设置页重读时 `/api/v1/remote-sync` 也返回 401，场景只声明了 `/api/v1/settings`；已在场景中补上这一预期（`scripts/browser/rssr_server_acceptance.mjs`），场景自身的三项检查当时都已通过
- 变异检查：恢复旧的“先读指纹后导出”顺序、去掉 `identity` 请求头，各自对应的测试失败
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过
- `cargo check -p rssr-app --target wasm32-unknown-unknown --features pages-demo --locked`：通过
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：未执行（本机无 Android NDK，`ring` 找不到 `aarch64-linux-android-clang`）；CI 的 Android job 覆盖

### 手工验收

- 真实 WebDAV 服务（坚果云 / Nextcloud）的 ETag 与条件请求：未执行
- 桌面 / Android / 正式 Web 界面上的记住、停用、启动提示：未执行

## 结果

- 两轮独立评审的问题均已修复并经自动化验证，可开 PR；合并前建议用真实网盘（坚果云 / Nextcloud）验收 ETag 与条件请求，并在桌面与正式 Web 上走一遍记住、停用、启动提示。

## 风险与后续事项

- 服务器不返回强 ETag 时，从读取远端到上传完成之间另一设备的上传可能被覆盖；返回强 ETag 的服务器会识别为冲突。是否在此时停用自动上传，待 Develata 在真实网盘验收后决定。
- 凭据明文保存（Develata 决定）；以后可换系统钥匙串，档案接口不变。
- 同一数据目录只应由一个带档案的进程使用（桌面与服务器不要共用数据目录）。
- 下一步：“备份 / 恢复”入口单独写设计文档，第一步做本地整库文件导出与恢复（GUI + `rssr-cli restore`）。

## 给下一位 Agent 的备注

- 入口：`docs/design/webdav-auto-sync.md`，`ImportExportService::sync_remote_config`，`LocalBackend::run_remote_sync`。
- 改规则时跑 `cargo test -p rssr-infra --test test_library_remote_sync --locked` 与 `cargo test -p rssr-infra --lib local_backend --locked`。
