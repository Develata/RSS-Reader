# README 截图更新与 GitHub Attachments

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/readme-media-20261031`
- 源码 HEAD：`f3d26a90e08ac9a0dc2f30677633d96c46195ce1`（截图时 main）
- 相关 commit：pending；本地文档提交后执行标准 GC，再补回执
- 相关 tag / release：无；本次截图不代表 v0.2.1 发布包
- 状态：`draft`

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

本地提交后先记录 `git count-objects -vH` / `git status --short`，确认没有并发 Git 写入或锁文件，再执行不带激进参数的 `git gc`，比较实际对象目录字节并执行 `git fsck --full --no-dangling`。GC 尚待执行，最终结果补入本节，不用历史结果替代。

## 当前状态、风险与后续事项

- 当前授权为本地提交；不 push、不创建 PR、不发布。文档分支仍需后续发布到远端，Issue 与附件已按本任务授权创建。
- `E:/gitclone/RSS-Reader` 主 checkout 原有 41 个改动/缺失路径已建立 HEAD、分支、index 和文件散列基线，收尾时再次核对；该 checkout 不切换、不清理、不提交。
- 本次只验证截图所需的正式 Web 页面和文档；没有修改产品代码，不将这次截图当作完整产品回归或原生宿主验收。
- 外部附件依赖 GitHub 可达性；保留媒体 Issue 和独立本地备份，今后界面有实质变化时复用该 Issue 更新。

## 给下一位 Agent 的备注

从 [贡献媒体约定](../../CONTRIBUTING.md#文档媒体)和 Issue #58 接续。采集脚本保存在忽略目录与独立备份中，复用仓库现有 harness；没有引入永久截图依赖或新的自动化子系统。
