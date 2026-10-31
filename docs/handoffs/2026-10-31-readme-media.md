# README 截图更新与 GitHub Attachments

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/readme-media-20261031`
- 源码 HEAD：`f3d26a90e08ac9a0dc2f30677633d96c46195ce1`（截图时 main）
- 相关 commit：`40c3e0de`（README、媒体约定、旧图删除与初始交接）；本记录随后以单独的本地文档提交补充 GC 回执
- 相关 tag / release：无；本次截图不代表 v0.2.1 发布包
- 状态：`validated`

## 工作摘要

用当前源码运行的正式 Web 文章首页替换旧 README 截图，通过本地 GitHub CLI 的官方附件入口托管媒体。仅维护文档与媒体，不改变前端设计、运行代码、依赖或工作流。

## 影响范围

- `README.md`、`docs/README.en.md`：保留原有 alt text，改用同一个已验证附件；说明截图日期、main 基线及示例数据。
- `assets/readme/rss-reader-overview.png`：从当前树删除旧的 651701 字节 PNG，不新增其他受 Git 跟踪的截图。
- `CONTRIBUTING.md`：增加简短媒体约定；本记录与交接索引更新。
- 平台：Windows 本地构建并用 Chrome 采集正式 Web；不是 Windows 原生宿主或 Android 验收。

## 关键变更

### 真实前端与隔离数据

使用现有 Dioxus CLI 0.7.9、`rssr-web`、`browser_fixture_library` 和 `scripts/browser/server_harness.mjs`。从上述源码构建前端、服务器与资料库工具，并将输入复制到本任务的忽略目录，固定 `RSSR_BUILD_ID=git-f3d26a90e08ac9a0dc2f30677633d96c46195ce1`。

资料库是新建的 SQLite，含 6 个虚构来源、36 篇非敏感示例文章；仅访问本机回环测试源。真实页面使用内置浅色主题，没有注入 CSS、替换 DOM 或拼接界面。Chrome 155.0.8059.40 / Playwright CLI，视口 1440 × 900，等待字体、图片、网络与列表定位就绪后截图。主导航、文章列表、已读/收藏状态和右侧时间目录均在截图中。

### 附件与备份

- 专用媒体 Issue：[docs: README media assets #58](https://github.com/Develata/RSS-Reader/issues/58)。创建前查过所有状态的 Issue，未找到既有媒体索引；本次只创建一个 Issue、上传一次。
- 附件：[当前 Web 文章首页](https://github.com/user-attachments/assets/2f28803c-2b02-4e83-a9c2-3acdfbf18167)。用本地已登录的 `gh 2.103.3` 执行 `gh issue create --attach`，回读 Issue 正文取得真实 URL。
- PNG：1440 × 900，178255 字节；SHA-256：`c1aaf70a948468e70137e57590539173b72d406cc48a9d7642a3a4d9f3193d4d`。
- 未认证下载返回 HTTP 200、`image/png`；PNG chunk CRC、像素数据解压和尺寸检查通过，下载与原图逐字节相同。
- 工作证据：`E:/gitclone/RSS-Reader-worktrees/native-back-policy/target/readme-media-20261031/`。
- 独立备份：`E:/gitclone/RSS-Reader-media-backups/2026-10-31-f3d26a90/`。原图、示例数据、采集脚本、就绪/构建标识回执和 Issue 回读均已逐字节核对，索引为 `SHA256SUMS.json`。备份不含真实账号、会话 cookie 或用户资料库。

### 历史引用与 Git 历史

两个活动 README 图片引用均迁移到附件。2026-09 的三份归档交接只以行内代码提及旧路径，没有活动图片或文件链接；保留这些当时的记录，不把历史截图结论改写为当前结论。旧图可从[截图前的历史提交](https://github.com/Develata/RSS-Reader/blob/f3d26a90e08ac9a0dc2f30677633d96c46195ce1/assets/readme/rss-reader-overview.png)追溯。

删除当前树中的 PNG 不删除历史对象。普通 `git gc` 可以整理、压缩存储，不能清除仍被历史提交引用的旧图；本次不重写历史、不改 tag、不推送。

## 验证与验收

### 已执行

- `dx build --platform web --package rssr-app --release --locked --debug-symbols false --verbose --log-to-file ...`：通过；核对常规及详细日志，没有 wasm-opt 失败或 ERROR。
- `cargo build -p rssr-web --locked`、`cargo build -p rssr-infra --example browser_fixture_library --locked`：通过。
- Playwright CLI 登录真实服务并截图：通过；`document.fonts.status=loaded`，没有图片加载失败或 alert，控制台 0 error / 0 warning。
- 刷新时监听实际 API 请求：全部 `x-rssr-build` 与服务器标识相同，页面异常为 0。
- 直接查看截图：布局、中文文字、导航和目录正常；没有加载画面、报错、调试覆盖层或浏览器边框。
- 附件匿名下载、PNG 格式检查、原图与备份散列校验：通过，见 `attachment-verification.json`、`backup-verification.json`。

### 本地提交及 GC 收尾

文档检查覆盖根 README、CONTRIBUTING 和 docs 共 349 份 Markdown、1734 个本地链接：没有失效链接或锚点。最终 diff 仅含上述文档和旧图删除，`git diff --check` 通过；主 checkout 的 HEAD、分支、index 与 41 个改动/缺失路径均与任务开始时一致。

`40c3e0de` 提交后，工作树干净。检查没有 Git 锁文件或并发 Git 写入；机器上三个已有 Git 进程仅执行只读的 `config --null --get core.fsmonitor`，未停止这些其他会话的进程。随后运行一次无附加参数的 `git gc`，退出码 0；`git fsck --full --no-dangling` 退出码 0，输出为空。

| `git count-objects -vH` | GC 前 | GC 后 |
| --- | --- | --- |
| count | 801 | 0 |
| size | 1.98 MiB | 0 bytes |
| in-pack | 20839 | 20736 |
| packs | 2 | 2 |
| size-pack | 14.61 MiB | 14.77 MiB |
| prune-packable | 0 | 0 |
| garbage | 1 | 1 |
| size-garbage | 0 bytes | 0 bytes |
| `.git/objects` 文件长度合计 | 18170549 字节 | 15659416 字节 |

对象目录文件长度合计减少 **2511133 字节（约 2.39 MiB）**。这是同一次 GC 前后的文件字节测量，非磁盘分配簇大小；计数中的 size / size-pack 不包含该目录全部辅助文件。GC 后再提交本交接回执会产生少量新的松散对象，不改变上表这次 GC 的测量结果。

GC 前后均有 `garbage found: .../.git/worktrees/native-back-policy/refs` 警告。只读核对该路径是既有空目录（2026-10-29），计入 1 项、0 字节；未手工清理 Git 元数据。`fsck` 没有报告对象损坏或缺失。

所有分支与 tag 的引用值、当前 HEAD、index、工作树状态在 GC 前后完全一致；旧图历史 blob 仍存在且大小为 651701 字节。主 checkout 的 HEAD、分支、index 和 41 个改动/缺失路径在 GC 后仍与原基线一致。详见证据目录中的 `gc-before.json`、`gc-after.json`、`gc-comparison.json`、`fsck.log`、`fsck-exit.txt` 与 `primary-preservation.json`。

## 当前状态、风险与后续事项

- 当前授权为本地提交；不 push、不创建 PR、不发布。文档分支仍需后续发布到远端，Issue 与附件已按本任务授权创建。
- `E:/gitclone/RSS-Reader` 主 checkout 原有 41 个改动/缺失路径已按 HEAD、分支、index 和文件散列基线核对，保持不变；没有切换、清理或提交该 checkout。
- 本次只验证截图所需的正式 Web 页面和文档；没有修改产品代码，不将这次截图当作完整产品回归或原生宿主验收。
- 外部附件依赖 GitHub 可达性；保留媒体 Issue 和独立本地备份，今后界面有实质变化时复用该 Issue 更新。

## 给下一位 Agent 的备注

从 [贡献媒体约定](../../CONTRIBUTING.md#文档媒体)和 Issue #58 接续。采集脚本保存在忽略目录与独立备份中，复用仓库现有 harness；没有引入永久截图依赖或新的自动化子系统。
