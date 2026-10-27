# 目录模块内聚性与 agent 可读性整理

- 日期：2026-10-27
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 修改前 HEAD：`b473fd55`
- 相关 commit：本记录与代码同一提交，准确提交号见 `git log -1 -- docs/handoffs/2026-10-27-directory-cohesion-readability.md`。
- 相关 tag / release：N/A；未 push、未发布。
- 状态：`validated`（下列范围）；release 二次优化工具异常单列。

## 工作摘要与检查发现

用户要求提高高内聚、低耦合与 agent 可读性。本轮范围是刚调整的目录 UI 及直接相邻的模型／虚拟列表绑定，不是整个仓库的架构重写。

按维护风险排序发现并处理：

1. 普通分支与虚拟分支重复实现 `(index, id)` 校验及全局页码计算，未来修改可能产生两套导航规则；`model.1` 也掩盖了文章快照的含义。
2. 时间日期与来源月份重复拼装分支界面，`date/time: bool` 参数以及 `directory-month-*` 内部类名不能准确表达两种用途。
3. 父级计数预留 48px、箭头右偏移 19px 与 44px 点击区域存在隐式关系，分散修改可能破坏对齐；已隐藏的 SVG 仍被创建。
4. 控制器、桥接、DOM adapter、窗口模型的入口分散，缺少就近说明，容易把修改放进错误职责层。

这些是代码维护风险；没有将其表述为已证实的运行时功能故障。

## 影响范围与关键变更

- `directory.rs`：将 `DirectoryModel` 改为具名私有字段，通过 `new` 构建；新增 `resolve_article`，统一校验当前快照身份并计算全局目标页，返回借用的文章 Arc 与页码。
- `facade.rs`：仅适配具名构造入口；保留原有 Arc 所有权、快照来源和生命周期。
- `directory/view.rs`：以借用的 `ArticleBranch` 和明确的分支类型集中渲染日期／月份；标签→右侧展开按钮→文章的结构只有一处。保留键控根节点，集合直接迭代这些节点；虚拟化阈值命名为 `VIRTUAL_ARTICLE_THRESHOLD`。
- `directory/virtual_articles.rs`：复用相同文章解析入口，保留窗口下标、焦点、全局引用、分页及根节点 key。
- `reading-workspace.css`：内部类名改为 `directory-branch-*`；尺寸变量只定义在目录 rail 内，父级箭头偏移与计数预留由点击区域尺寸推导。删除创建后又隐藏的旧 SVG，保留共用按钮基础样式及 5px CSS 箭头。
- `directory/README.md`：新增职责入口表、共享规则、稳定 DOM 接口、焦点/key 约束与验证命令；模块源码首部链接该入口。
- 控制器、消息协议、host/application/domain、查询、选择、归档、自动跟随和布局效果保持原语义。无新增依赖或生产数据副本。

## 验证与验收

### Rust 与构建

- `cargo fmt --all --check`：通过。
- `git diff --check`：通过。
- `cargo test -p rssr-app --locked directory:: -- --nocapture`：25 项通过，包括新增的失效 ID／越界引用拒绝、全局页边界及零页尺寸回退测试。
- `cargo test -p rssr-app --locked --test test_default_style_token_contract --test test_builtin_theme_contracts`：4 项通过。
- `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`：通过。
- `dx build --platform web --package rssr-app --locked`：38.14 秒成功，含既有 dx 0.7.10 / Dioxus 0.7.9 版本提示。
- `dx build --platform web --package rssr-app --release --locked`：编译与打包完成、命令退出 0，但 `wasm-opt` 报 `0xc0000409`。不能据此声明完整 release 优化通过；下列回归验证的是实际产出的可运行 Web 包。

### 真实浏览器与既有回归

- 首轮使用 debug 产物并行运行离线脚本：窗口测试在初始页面加载前报告 page/context/browser closed，断言数组为空；另一路未完成并已停止本任务进程及子进程。这轮不计为通过，未确定浏览器退出的具体原因。
- 最终改用 release 产物顺序运行，环境为 `NODE_PATH=C:/Users/QQ/node_modules`、`PUBLIC_DIR=target/dx/rssr-app/release/web/public`：
  - `node scripts/browser/rssr_directory_offline_regression.cjs`：150 项通过，console errors 为空。
  - `node scripts/browser/rssr_directory_window_acceptance.cjs`：22 项通过，包含 600 篇、窗口边界 Tab、全局第 12 页、变高行、手机浮层、隐藏后滚动恢复及坏消息后的继续使用；page errors 为空。
- 另以本任务隔离服务 18105 打开刚构建的 debug 实际页面，1200×936 默认桌面几何与重构前记录完整 JSON 相等：目录、父级标题／计数、日期行、按钮、每篇文章位置／尺寸、箭头尺寸与无横向溢出全部一致。实际截图复核通过。
- 分支实际鼠标收起文章 6→0、Enter 展开 0→6；分支内 SVG 数为 0，CSS 箭头保留。
- 390×844 手机：目录宽 220px，文章文字 x=173、按钮 44px、箭头 5px，无横向溢出，和上一版一致。
- 本轮无头脚本均使用隔离离线 fixture；交互页面使用独立测试端口，没有读取用户真实订阅。

## 结果与风险

- 目录的共享身份／分页规则、分支渲染和几何依赖已集中，命名与维护入口更明确。现有右侧箭头、8px 缩进设计保留，Penpot rev 369 的视觉基准无需修改。
- 本轮不是全仓库审计。没有重验 Android 或 Windows WebView2 实机；浏览器响应式与自动化验收不能替代它们。
- `wasm-opt` 的本机退出错误未在本轮修复，未做发布、性能或优化体积验收。普通 Web 构建、静态检查和上述功能回归通过。
- 未触碰其他任务的 dirty / deleted / untracked 文件；仅提交本轮理解并验证的路径。

## 给下一位 Agent 的备注

- 先读 `crates/rssr-app/src/pages/entries_page/directory/README.md`，再按职责进入具体文件。
- 自动化证据：`target/directory-cohesion-20261027/release-regression/acceptance.json`、`target/directory-cohesion-20261027/release-window/acceptance.json`；初次失败窗口回执在同目录的 `window/acceptance.json`。
- 交互证据：`C:/Users/QQ/.codex/visualizations/2026/10/26/01a1779a-f271-7ec2-ab70-c1ef756f9ffb/directory-cohesion-desktop-geometry.json`、`directory-cohesion-desktop.png`、`directory-cohesion-mobile.png`。
- 视觉比较基准为同证据目录的 `directory-right-desktop-geometry.json`；不要用旧 `directory-month-*` 内部类名作为新的主题契约，稳定接口继续用 `data-layout` / `data-slot`。
