# 任务 2：本次运行内的位置恢复

- 日期：2026-09-25
- 作者 / Agent：Codex
- 分支：main（原隔离 worktree 如有则保留）
- 当前 HEAD：fc717c0（2026-09-26 集成完成时）
- 相关 commit：4d0532e
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

实现本次运行内的列表返回锚定、分页恢复和正文位置记忆；不自动标已读。

2026-09-26 已集成复验并本地提交。初轮受环境阻塞的记录保留供追溯；当前结论以末尾补验为准。

## 影响范围

- 模块：app UI runtime、entries / reader 页面、entries CSS、README 与用户/前端/主题文档。
- 平台：共享 Web / 桌面 / Android UI；Android 本轮只检查编译。
- 额外影响：无数据库 schema、domain/application trait、CLI、持久化或配置交换改动。

## 关键变更

- `ui/reading_position.rs/js`：Rust 保存位置并决定锚定、像素回退、有效范围及用户打断；DOM bridge 提供页面代次、坐标、块锚点和时钟事实。运行结束清除。
- 列表按筛选上下文保存分页；首次查询完成后恢复页码，等待 presenter 与当前页一致后才允许恢复 DOM 位置。
- 阅读页只在当前文章正文加载完成后恢复，最多校正 2 秒；顶部和末尾均记录，不显示提示、不修改已读状态。
- 新增主题接口：`[data-layout="entry-card"][data-return-highlight="true"]`。`data-position-*` 为内部测量字段，不承诺为主题接口。
- 一次性验收脚本与日志均放在忽略目录 `target/task2-validation/`，用 bun 执行；未提交本机路径脚本。

## 验证与验收

以下为 2026-09-25 初轮记录；2026-09-26 最终结果见末尾补验。

### 自动化验证

当前命令日志在 `target/task2-validation/`；最后一轮结果是 `checks.tsv` 的最后 7 行。

- `cargo fmt --all --check`：0。
- `cargo clippy --workspace --all-targets -- -D warnings`：101，缺原生 GTK/WebKit 等开发库。
- `cargo test --workspace`：101，同一环境阻塞，不能算测试通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：0。
- `cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings`：初次 101（collapsible_if）；修复后 0。
- `cargo check -p rssr-app --target aarch64-linux-android`：101，target 已装，但缺 `aarch64-linux-android-clang`。
- `cargo check -p rssr-app --tests --target wasm32-unknown-unknown`：0，只代表测试代码可编译。
- `cargo test --offline --manifest-path target/task2-validation/policy-check/Cargo.toml`：0，直接引用产品 Rust 模块的隔离测试，3 passed；不替代 workspace 测试。
- `git diff --check`：0。
- `dx build --platform web --package rssr-app --locked`：基线和实现 bundle 均成功；现有 dx 0.7.10 报与 Dioxus 0.7.9 不匹配，未自行安装工具或升级依赖。

### 手工 / 真实浏览器验收

使用已有 Chrome 151 / Playwright，通过 bun 运行临时脚本，现有 SPA fixture server 端口 8099。

- 基线 Web 360×800 / 1280×800：第一页通过返回按钮回列表时原像素位置已由既有行为恢复（2062 / 2012）；文章内滚到 3200 后离开，再打开均回到 0。`baseline.json`。
- 第一版相同路径：列表位置不变；文章重新打开恢复至 3200。`after.json`。
- 第二页返回曾失败：缓存已正确保存 2，但首次挂载的搜索副作用随后把页码重置成 1。修复为仅在真实搜索上下文变化时重置，补充分页恢复的 session 单元测试。
- 最新 `bun target/task2-validation/positions.cjs target/task2-validation/verified.json`：退出 0。两种视口均通过返回按钮恢复、3200px 正文恢复、第二页浏览器后退恢复、淡背景出现与消失、主动翻页到顶。已查看 `list-360.png` 与 `reader-1280.png`。
- 360×800 另通过：未读筛选下阅读页标已读导致目标消失，返回按原像素回退；相邻文章跳转从顶部开始，后退恢复原文章；用户滚动停止校正；Web reload 清除位置。
- 原生当前基线无法构建，未完成同路径前后对照；不能用旧可执行文件冒充当前基线。
- 未运行：Android 运行验收，按已确认范围只做编译检查；缺 NDK 编译器导致检查阻塞。

## 结果

本轮确认范围通过，已本地提交 4d0532e，未 push / tag / release。Android 仅编译；其余平台及输入方式的验证边界见末尾补验。

## 风险与后续事项

- 已验证真实延迟图片：最终 `bun target/task2-validation/delayed-image.cjs` 退出0；360×800与1280×800、650ms图片延迟，禁用浏览器原生scroll anchoring并清除图片缓存后，正文锚点top与3200px位置均精确恢复。初次图片缓存命中未产生第二次请求，清除cache后验证实际请求两次；日志delayed-image.log/json。
- 未验证：原生当前版本 GUI 前后对照、Android 运行（明确不在本轮范围）。
- 缺 GTK/WebKit 开发依赖：Debian 安装命令 `sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libxdo-dev`。未执行；这些包用于原生 app 编译和 workspace 检查。
- 缺 Linux Android NDK 编译器：现有 CI 固定 27.3.13750724，对应安装命令 `sdkmanager --install "ndk;27.3.13750724"`，之后按 `.github/workflows/ci.yml` 配置 Linux toolchain 的 CC/AR/linker。未自行安装。
- 按已确认规则，环境阻塞时保留本工作树，不提交任务；独立任务可在隔离 worktree 从最近已通过的提交推进。

## 给下一位 Agent 的备注

- 读取 `.handoff/2026-09-25-tasks2-5-combined.md` 及本会话最终确认：4.3 选 B，其余选 A；不能把原文的待澄清选项当作新问题重复询问。
- 当前专项脚本为 `target/task2-validation/positions.cjs`；构建后再运行，避免加载构建中途产物。

## 改动文件清单

- `README.md`：同步本任务的用户流程、行为口径和限制。
- `assets/styles/entries.css`：增加返回条目的短暂弱化高亮样式。
- `crates/rssr-app/src/app.rs`：在应用生命周期挂载位置恢复能力。
- `crates/rssr-app/src/pages/entries_page/cards.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/facade.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/mod.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/reducer.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/session.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/entries_page/state.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/reader_page/facade.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/reader_page/mod.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/reader_page/reducer.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/pages/reader_page/state.rs`：连接页面状态、交互事件、异步结果与渲染。
- `crates/rssr-app/src/ui/mod.rs`：连接命令执行、结果映射或反馈生命周期。
- `crates/rssr-app/src/ui/reading_position.js`：采集DOM位置、用户输入及布局事实并执行Rust恢复命令。
- `crates/rssr-app/src/ui/reading_position.rs`：实现会话内位置缓存、恢复策略与有界校正。
- `docs/design/frontend-command-reference.md`：同步命令、状态流转与反馈规则。
- `docs/design/theme-author-selector-reference.md`：登记新增稳定选择器与样式接口。
- `docs/handoffs/2026-09-25-task2-reading-position.md`：记录本任务接口、验收证据和剩余阻塞。
- `docs/user-guide.md`：同步本任务的用户流程、行为口径和限制。

## 最终环境备注

本轮创建的临时HTTP fixture、SPA验收服务和ChromeDriver已停止，截图与日志保留在target中。最终PATH未找到sdkmanager、adb或xvfb-run，ANDROID_HOME/ANDROID_SDK_ROOT未配置；NDK安装命令以已有Linux Android command-line tools为前提，本轮没有安装。各worktree仍以2ba826a为HEAD，全部改动未暂存，未commit/push。

## 2026-09-26 补验（覆盖此前环境阻塞状态）

GTK/WebKit 与 Android SDK/NDK 已补齐，具体版本和安装记录见 `2026-09-26-integration-revalidation.md`。

- 全量 `cargo test --workspace` 首次暴露高亮 CSS 硬编码颜色回退，被既有主题契约测试拦截；改为 `background: var(--accent-soft)` 后重跑退出 0：297 passed、2 ignored。
- 最终 `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、wasm check / clippy、Android ARM64 check、`git diff --check` 均退出 0。Android NDK 27.3.13750724 / API 34；没有 Android 运行验收。
- 原生 `cargo build -p rssr-app` 退出 0；从 HEAD 2ba826a 的源码归档构建基线成功。基线构建首个外层进程退出 143，日志已完成构建；重跑获取外层退出 0 后才复制使用产物，不能将中断当通过。
- Linux / WSLg、WebKitGTK 2.52.6、1280×900：通过隔离实例的 WebKit Inspector 执行 DOM click / scrollTo 并读取坐标，X11 抓取真实原生窗口截图。基线列表 2500→157、正文重开 3200→0；任务 2 列表 2500→2500，标题 top 171→171，正文 3200→3200。两个原生专项脚本均退出 0，截图已查看；不是用 Web bundle 替代原生界面。
- 现有 X11 辅助脚本的来源点击有效，合成滚轮与 PageDown 未观察到滚动；因此上述滚动专项明确使用 Inspector，不宣称物理滚轮已验证。
- Web 最终重新构建退出 0；360×800、1280×800 的 positions.cjs 重跑退出 0，覆盖按钮/浏览器返回、分页、文章恢复、目标消失回退、主动输入取消与高亮生命周期。
- 日志、原生二进制 SHA-256、截图和隔离数据库位于 `target/integration-validation/`，不提交本机工具或数据。
- 最终延迟图片专项重跑退出 0；两视口均覆盖真实延迟加载。任务 2 已达到本轮已确认范围内的本地提交条件；本文件首次提交即任务 2 实现提交。

## 集成交付补充文件

- `docs/handoffs/2026-09-26-integration-revalidation.md`：汇总开发依赖、各任务最终检查、提交及保留工作树状态。
