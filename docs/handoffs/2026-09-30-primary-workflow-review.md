# 日常阅读与筛选流程审查

- 日期：2026-09-30
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：`06bf9df`（审查基线）
- 相关 commit：本记录所在的独立审查提交（应用代码审查基线为 `06bf9df`）
- 相关 tag / release：N/A
- 状态：`validated`（发现已复现，尚未修复）

## 工作摘要

按用户“看看还有没有重要的值得优化的地方，暂不考虑边缘情形”的要求，检查文章筛选、连续阅读和阅读页操作。确认四项常见使用路径上的问题，并给出局部修正方向；不扩展异常恢复、并发、极端数据或架构迁移议题。

## 影响范围

- 模块：`rssr-app` 的 Entries / Reader 页面、Reader runtime、Amethyst Glass 主题。
- 平台：在 Web Chromium 实测；相关 Rust UI 和 CSS 为跨端共用代码。此次未做 desktop / Android 原生验收。
- 文档：仅新增本记录。复现脚本、测量 JSON 和截图保存在被 Git 忽略的 `target/important-ux-review/`。
- workflow / 数据：没有 workflow、存储格式或生产数据变更。浏览器测试使用独立 context 和项目内置 `reader-demo` fixture。

## 关键变更

以下为按影响排序的审查发现和建议，不代表修复已实施。

### 1. P2：未读导航会回退到已读文章

- 事实：`reader_page/session.rs:91–97` 在没有对应方向的未读目标时，回退到同订阅的邻近文章；底部按钮和快捷键文案仍表示“上一未读 / 下一未读”。
- 复现：fixture 的第 1 篇已读、第 2 篇未读。在第 2 篇点击标已读，确认存储中未读数为 0；“下一未读”依然可用，点击后进入已读的第 1 篇。
- 影响：用户读完未读文章后仍被带入旧文，无法通过此操作判断当前方向已无未读内容。
- 建议：未读按钮和方向快捷键只使用未读目标，没有目标时禁用并给出清楚状态。同订阅浏览沿用正文末尾已有的独立按钮。无需重做 application / domain 导航模型。
- 入口：[导航目标](../../crates/rssr-app/src/pages/reader_page/session.rs)、[按钮与同订阅导航](../../crates/rssr-app/src/pages/reader_page/mod.rs)、[用户契约](../user-guide.md)。

### 2. P2：Amethyst Glass 窄屏底部按钮覆盖彼此

- 事实：主题给 `mark-read` / `toggle-starred` 设置 `min-width: 6rem`，而底栏使用四个等宽网格列。360×800 下列宽为 63.5px，两个动作按钮实际宽度为 96px。
- 复现：标已读与收藏、收藏与下一未读分别重叠 24.5px。收藏按钮矩形为 x=184–280，下一未读为 x=255.5–319；点击重叠区域 x=267.75，实际进入下一篇文章。鼠标模式和触屏媒体条件下均复现。
- 控制变量验证：仅在测试页面把按钮的 `min-width` 清零，所有按钮变为 63.5px 宽，重叠消失。
- 同一主题的另一处排版问题：`reader-meta` 每行重复增加 32px 外边距、18px 内边距和 1px 边框。来源、作者、时间三行共多占 153px。保持主题其余样式不变，仅去掉这三项后，360px 下正文起点从 y=485.70 变为 y=332.70；1280px 下从 y=544.31 变为 y=391.31。
- 建议：底栏按钮服从网格宽度；元信息的分隔和留白由已有 `reader-meta-block` 容器统一承担，保持已确认的元信息顺序。主题改版应沿用旧 CSS 身份识别机制，并说明已保存主题需要重新应用；不直接覆盖用户 CSS。
- 入口：[主题 CSS](../../assets/themes/amethyst-glass.css)、[底栏与元信息布局](../../assets/styles/reader.css)、[主题历史识别](../../crates/rssr-app/src/pages/settings_page/themes/theme_preset.rs)。

### 3. P2：筛选收起后缺少状态摘要，空结果提示原因错误

- 事实：`entries_page/controls.rs:49–63` 收起后仅显示“筛选与组织”；`facade.rs:230–238` 除批量已读后的特殊分支外，空列表都建议添加或刷新订阅，没有根据当前筛选解释原因。
- 复现：已有一个订阅、两篇文章，选择“仅已读”与“仅收藏”后结果为 0。收起面板，页面提示“没有可显示的文章，先去订阅页添加并刷新 feed。”，没有显示仍在生效的两个条件。重新展开可确认条件仍被选中。
- 影响：日常搜索或组合筛选无结果时，用户容易误判为订阅没有内容；返回列表时也难以看出筛选为何减少了文章。
- 建议：折叠状态保留简短条件摘要和清除筛选入口；区分首次没有文章与“当前筛选没有匹配结果”，并在后者提供直接恢复筛选的操作。
- 入口：[筛选折叠呈现](../../crates/rssr-app/src/pages/entries_page/controls.rs)、[空态文案](../../crates/rssr-app/src/pages/entries_page/facade.rs)。

### 4. P2：收藏成功提示改变当前正文位置

- 事实：Reader 将成功 `StatusBanner` 插在元信息与正文之间；收藏与标已读都会写入此状态。空提示不占布局，操作成功后变为可见提示条。
- 复现：默认主题、360×800、24 段普通正文。在顶部取消收藏，正文起点由 y=328.70 变为 y=392.70，移动 64px。重新加载后用真实鼠标滚轮滚动到 scrollY=1000，再点击收藏，第 9 段屏幕位置由 y=346.08 变为 y=410.08；等待 2.2 秒后仍然移动 64px，scrollY 未变。
- 影响：阅读中执行常用收藏操作后，正在看的文字离开原位置。这里实测的是收藏；标已读使用相同状态和呈现路径，未单独测量其正文位移。
- 建议：成功操作以按钮状态和不占正文布局的辅助技术播报反馈，或使用不引起正文重排的轻量提示；失败继续提供可见错误信息。保持状态播报能力，不需要修改存储或业务语义。
- 入口：[Reader 状态条位置](../../crates/rssr-app/src/pages/reader_page/mod.rs)、[操作结果](../../crates/rssr-app/src/ui/runtime/reader.rs)、[状态组件](../../crates/rssr-app/src/components/status_banner.rs)。

## 验证与验收

### 自动化验证

- `bash scripts/run_web_spa_regression_server.sh --debug --skip-build --port 8123`：使用当前 `06bf9df` 对应的本地 Web 构建启动测试服务。
- `node target/important-ux-review/review.cjs`：首次运行已复现导航与筛选问题；后续主题对照的测试页样式注入没有覆盖 body 内的主题 style，导致测量断言失败。改为元素内联样式做控制变量，未修改产品 CSS。
- `REVIEW_PHASE=reader node target/important-ux-review/review.cjs`：四组主题 / 视口测量完成，验证重复留白为 153px；后续段落定位因 sanitizer 移除了测试用 id 而超时。改用正文段落顺序定位。
- `REVIEW_PHASE=feedback node target/important-ux-review/review.cjs`：退出 0，完成顶部与正文中段的收藏位移测量；最后一轮使用真实滚轮输入并等待 2.2 秒确认稳定结果。此前已经完成的导航、筛选和主题数据保留在 `report.json`。
- `node target/important-ux-review/theme-controls.cjs`：退出 0；鼠标 / 触屏媒体条件下均确认按钮重叠、实际点击目标和 CSS 控制变量结果。
- `node --check`（上述两个临时复现脚本）：通过。
- `git diff --check`：通过。
- Python 文档校验：11 个本地链接均存在，固定章节、pending 状态和行尾空白检查通过；新增文件的 `git diff --no-index --check` 没有空白错误输出。
- 未运行：Rust fmt / clippy / workspace test、重新构建和全主题全页面回归。原因：本次只做审查记录，应用源码与构建输入没有变化；本轮验证聚焦被审查的实际行为。

### 实际浏览器验收

- 查看默认主题、Amethyst Glass、筛选空结果及触屏底栏截图，核对可见文字和按钮覆盖现象。
- 主题元信息测量覆盖 360×800 和 1280×800；导航、筛选、正文位移和按钮重叠主要在 360×800 复现。
- 浏览器为现有 Playwright Chromium；复现用例记录的 `pageerror` 为空。测试服务不提供 debug bundle 的 live-reload WebSocket，不以该静态服务验证热重载。
- 未执行 desktop / Android 原生验收，也未将 Web 结果等同于原生端实测。

## 结果

- 本次交付为有源码和浏览器证据的审查建议；四项问题仍存在，未实施修复、安装、发布或 push。
- 复现结束后已关闭浏览器及 8123 端口的临时测试服务。
- 没有证据支持以此次发现为由调整当前分层、替换存储或开展广泛性能优化。建议沿现有页面、状态反馈与主题边界局部处理。
- 审查阶段仅新增本记录，随后独立本地提交；本文件记录修复实施前的发现。

## 风险与后续事项

- 后续修复应分别验证：无未读时导航不可误导、窄屏按钮区域互不覆盖、筛选摘要与空态一致、成功反馈不移动正在阅读的正文。
- 主题的文字高度受字体和内容影响，153px 是三个元信息段落额外边距 / 内边距 / 边框的控制变量差值，不是所有文章固定的首屏高度。
- 这些发现来自普通两篇文章、常见筛选和一般长度正文；本轮没有覆盖异常存储、并发、极端长文本或大数据性能，也未对这些方向作结论。

## 给下一位 Agent 的备注

- 审查基线 `06bf9df`，生产代码未改。先读上述源码入口和 `docs/design/frontend-command-reference.md`，再决定具体修正。
- 原始证据位于 `target/important-ux-review/report.json`、`theme-controls.json` 及同目录截图；临时文件不属于提交内容。需要重现时启动上述服务，再运行同目录脚本。
- 测试资料全部来自独立浏览器 fixture；未操作用户实际订阅和收藏。
