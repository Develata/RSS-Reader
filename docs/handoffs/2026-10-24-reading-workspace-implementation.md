# 阅读工作区模板落地、显式多选与订阅编辑

- 日期：2026-10-24
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 开始时 HEAD：`d9ebe35460b5a0975a2c8fff0db7b1233160c0c0`
- 相关 commit：与本记录同一笔本地提交；用 `git log -1 -- docs/handoffs/2026-10-24-reading-workspace-implementation.md` 定位
- 相关 tag / release：N/A；未获 push / tag / 发布授权
- 状态：`validated`（本地自动化；实机验收边界见下文）

## 工作摘要

用户在确认 Penpot v7、16:9 预览与轻量勾选样式后，明确授权将模板落到实际代码。此前“只改草模”的限制被本次代码授权替代。本次实现文章主面板＋独立目录、来源筛选弹层、当前页多选、批量已读／归档、订阅统计与名称／URL 编辑；单篇正文阅读页的布局和全站既有外围宽度保持。

## 影响范围

- UI：`rssr-app` 的 entries / feeds 页面、共享 SVG 图标及 dialog、末尾加载的 `reading-workspace.css`。
- 共享用例与宿主：`rssr-domain`、`rssr-application`、`rssr-host`；CLI 继续使用原有按筛选批量已读，并采用一致的手动／自动归档范围。
- 存储：SQLite migration 0008，BrowserStore 的兼容默认字段；JSON / OPML 名称往返。
- 平台：共享 Windows / Android / Web 代码；本地浏览器验证静态 Web BrowserStore。没有新增 crate 或依赖。
- 文档、浏览器脚本及本地回归记录；没有修改 CI workflow 或发布流程。

## 关键变更

### 布局与交互

- 默认 shell 最大宽度仍为 1080px。文章／目录间距 12px，目录 200px；≤1000px 收为可打开的独立浮层，标题正常换行。目录自身过长时滚动，主体保留原有文档滚动。
- 筛选条常驻：来源 checklist 可搜索、取消、应用；部分来源显示省略号；✓ / ○ / ☆ / ★ 状态筛选保留。来源／时间为单选按钮组。设置统一 SVG 齿轮。
- 勾选为 16px 圆角方框、44px 点击区，默认轻淡、悬停加强，已选行淡底色且不加整行描边；键盘独立焦点轮廓。顶部只全选当前页。
- 批量动作冻结明确选中的 ID，未选时禁用；目录浏览／同页定位不清空，分页／筛选／搜索／视图变化清空。成功清除已处理项，失败保留选择。
- 分页保持底部中央，左右按钮靠近；单篇 Reader 布局、快捷键和位置记忆未改。
- 来源编辑仅在 Subscribe，支持自定义名称、使用原名、URL、失败保留草稿。订阅页增加未读／归档总数。刷新日期保留在刷新按钮提示中，错误仍显示。
- Rust 管理草稿、选择、分页与目录跟随状态；DOM adapter 只处理 dialog、焦点和测量／定位（包括弹层打开后的窗口缩放与卸载清理）。选择变化不重建分组投影。

### 持久化与边界

- migration 0008 增加 `entries.is_archived` 与 `feeds.custom_title`；旧 BrowserStore 字段缺失时默认 false / None。没有删除数据、改正文库或同步文章状态。
- 手动归档与时间归档取并集，不重复计数；关闭时间归档仍保留手动归档。显示已归档可再次阅读；当前无批量撤销入口。
- 原生批量为一条 SQL，BrowserStore 为一次 FLAGS 提交。只操作显式 ID，不把 UI 批量范围扩展为整个筛选结果。
- URL 编辑保持 feed ID 和文章／阅读／收藏／归档状态；同步替换保存的来源筛选 URL，清除旧缓存验证器并提升 generation，拒绝迟到的旧刷新。重复地址与错误均原子失败。
- 新增 host 路由 `entries/apply_batch` / `feeds/edit_details` / `feeds/update`。旧 CLI 和 host 筛选预览接口保留；UI 旧 preview/confirm 控件退役。
- 配置结构不增字段；JSON `title` / OPML 名称导入为自定义名，刷新不会覆盖显示别名；配置不携带文章标记。

## 验证与验收

### 自动化验证

- `cargo test --workspace --locked -j 2`：384 passed，2 ignored，0 failed。
- `cargo fmt --all --check`、`git diff --check`：通过。
- `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings`：通过。
- `cargo test -p rssr-host --lib --locked -j 2`：17 passed；包含新增端点入参矩阵与编辑后 JSON 导出／导入名称及 URL 往返。
- `cargo test -p rssr-app --locked -j 2`：最终复验 104 passed / 1 ignored（含样式契约）。
- `dx build --platform web --package rssr-app --locked`：通过。
- Android `cargo check -p rssr-app --target aarch64-linux-android --locked -j 2`：通过；使用本机 NDK 28.2.13676358 的 clang.cmd / llvm-ar，target API 24。
- `cargo check -p rssr-infra --target wasm32-unknown-unknown --locked -j 2 --test wasm_subscription_contract_harness --test wasm_config_exchange_contract_harness --test wasm_refresh_contract_harness`：通过；补齐 harness 的 custom_title 及 query 单测的 feed_generations 初始化。
- `rssr_workspace_acceptance.cjs`：selection / sources / editing / responsive / no page errors 全通过，覆盖 1920×1080、1536×864、760×1000、390×844、360×800，以及打开弹层后缩放窗口。
- `rssr_small_viewport_assertions.mjs --directory-only true`：125 项断言通过，零控制台错误；旧横向目录用例更新为可收起 rail，末尾定位按浏览器可滚动范围验证（短尾组不能越界滚到视口顶部）。
- CI 同入口 `rssr_small_viewport_assertions.mjs` 默认主题：262 项断言通过。Atlas Sidebar / Newsprint / Amethyst Glass / Midnight Ledger 各 138 项通过，全部零控制台错误。
- `rssr_reading_position_acceptance.cjs`：360 / 1280 两尺寸通过；连续滚动及文本编辑的正文布局扫描均为 0。
- `rssr_task1_acceptance.cjs`：通过；长来源／作者／URL、DST 与非整点时区、缺少元信息、存储失败及订阅计数。统计校验随本次 UI 移到 Subscribe。

浏览器使用已安装的 Edge / Playwright 与独立测试存储。测试数据由既有 SPA fixture 服务注入；最终服务使用同一 handler 的临时 ThreadingTCPServer 包装以支持并发检查，未改应用服务。端口 8097 / 8098 不用于用户数据。

### 视觉检查与验收边界

- 已查看真实 Web 渲染：1920×1080、1536×864、760×1000、390×844、浅／深色、多选、来源筛选、订阅编辑。截图文章为隔离测试样例，不是用户文章库。
- 外部截图目录：`C:/Users/QQ/.codex/visualizations/2026/10/22/01a16568-e398-7fb0-9847-e636e6fdecb2/`，`rss-reader-code-*.png`。
- 这些是本地自动化浏览器证据和 Agent 视觉检查，不冒充人工实机验收。Windows WebView2、Android 触屏／系统返回以及 macOS / Linux 原生运行：本轮未做实机验收。
- 未做性能／内存量化，不把视觉分块视为性能优化成果。

## 结果

实现、五主题浏览器矩阵、格式和差异检查已完成，以单独功能分支本地提交；没有 push、tag、发布或触发远端 workflow。本地通过不替代发布前的设备验收。

## 风险与后续事项

- Wasm 三个专属 harness 本轮完成编译检查，未执行 wasm-bindgen 浏览器 runner（本机 0.2.128 与锁文件 0.2.126 不匹配，未改全局工具）。真实 BrowserStore 行为由上述 Web UI 自动化执行。广义 `--tests --target wasm32` 会把 SQLite 原生 integration tests 一并交叉编译而失败，不能用作这个仓库的 Wasm 检查命令。

- 已保存的自定义 CSS 是用户快照；旧筛选折叠、横向目录和全筛选批量操作的 selector 已退役，迁移说明见主题与命令参考。
- SQLite 新增列是持久化变化；回退旧版本时它不理解手动归档／自定义名称，不能以旧版本行为作为本次功能的验收结果。
- 首次无限并发 app test 遇到依赖编译进程 0xc0000409 / ICE，限制 `-j 2` 后通过；未把原因确定为业务代码问题。dx 0.7.10 / Dioxus 0.7.9 存在工具版本提示，本轮构建通过，未改锁文件或本机工具链。
- Android 初次缺少 clang 环境失败，显式采用已安装 NDK 后通过；不等于 APK 或设备验收。

## 给下一位 Agent 的备注

- 从 `docs/design/reading-workspace-penpot-contract.md`、`pages/entries_page/{state,reducer,controls}.rs`、`components/dialog.rs` 和 `reading-workspace.css` 进入。
- 分组／目录边界仍遵守 `entries-directory-follow.md`；新测试位于 `scripts/browser/rssr_workspace_acceptance.cjs`。
- 开始时已有 `.agents/skills/speckit-*`、`.specify/*` 删除、两份 Oct03 交接修改以及附件／旧 Penpot 交接 untracked；这些均保留，不纳入本次提交。
- 本次单独纳入已确认的 `reading-workspace-penpot-contract.md`，与实际实现授权和新交接相互链接。
