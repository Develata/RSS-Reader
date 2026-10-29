# 阅读工作区设计审查：Apple / Google 参考与当前证据

- 日期：2026-10-29
- 作者 / Agent：Codex（主审 + 官方资料、源码、Penpot 并行核对）
- 分支：`codex/secondary-page-surfaces`
- 当前 HEAD：`eb29193f163b694e2d0c5e70ff36f0d11f0f91f5`
- 相关 commit：审查时尚未提交，产品代码未改；后续由 `d14116ba` 归档
- 相关 tag / release：N/A
- 状态：`validated`（审查结论及所列证据；不是产品完整验收）

## 工作摘要

本文是改进前的只读审查记录，不是当前缺陷清单。用户确认后的实现与验收见[设计审查改进](./2026-10-29-design-refinements.md)。

用户要求参考 Apple 与 Google，深入检查尺寸、角度、动效、配色、行为等设计细节。本轮只做审查和建议，不修改生产代码、Penpot、用户资料库，也不创建或合并新 PR。

结论：保留现有阅读优先、暖色基调、安静层级与右上目录入口。优先修复深色目录浮层透底、部分主题按钮对比度及保存语义，再优化手机订阅布局和主题设置的信息组织。没有证据支持全面换肤或增加复杂动效。

## 影响范围

- 源码审查：`assets/styles/`、`assets/themes/`、`crates/rssr-app/src/components/`、文章／订阅／设置页面及保存逻辑。
- 设计审查：Penpot 当前代码设计规范的 Foundations、Components、文章、订阅、设置、登录代表图层；历史页未动。
- 平台：Windows 上 Codex 内置浏览器；CSS 视口 1200×900、760×960、390×844。当前环境使用传统滚动条，可用内容宽度会小于视口宽度。
- 实验数据：独立的 `directory-contract` 测试资料库，6 个来源、72 篇文章。设置保存实验只作用于该测试库。
- 额外影响：仅新增此审查交接文档与仓库外证据。

## 方法与证据边界

证据目录：`C:/Users/QQ/Documents/Codex/2026-10-29/design-audit/`。

沿用前一轮已经构建的 Web 产物，启动隔离测试服务 `127.0.0.1:8119`；本轮未重新构建。通过真实浏览器导航、点击、输入、视口切换、DOM 几何和计算样式读取，重新捕获并查看截图。源码核对负责解释根因，Penpot 核对负责识别设计基准漂移；不把静态画板当作运行行为证据。

| 步骤 | 本轮检查 | 状态与证据 |
| --- | --- | --- |
| 1 | 桌面文章、目录、导航与点击区 | 基本结构良好；`01-desktop-entries.png`、`01-desktop-measurements.json` |
| 2 | 手机文章工具栏、目录、搜索展开和空结果 | 有空间与可读性优化项；`02-phone-entries.png` 至 `05-phone-search-empty.png` |
| 3 | 手机订阅首页与单行操作 | 确认操作区过高；`06-phone-feeds.png`、`07-phone-feed-actions.png` |
| 4 | 设置、主题入口、主题应用的保存范围 | 确认重复入口和连带保存；`08-phone-settings.png`、`09-desktop-theme-controls.png`、`settings-save-evidence.json` |
| 5 | Midnight Ledger 主按钮和删除确认态 | 确认对比度／危险样式问题；`10-midnight-feeds.png`、`11-delete-confirmation.png` |
| 6 | 阅读页字级、行距、行宽 | 短测试正文基本正常；`12-desktop-reader.png`、`reader-measurements.json` |
| 7 | 760px 平板文章工具栏 | 四个操作同排，44px 点击区；`13-tablet-entries.png` |
| 8 | 默认深色手机目录 | 确认正文透底叠印，应优先修复；`14-dark-directory.png` |
| 9 | 当前 Penpot 规范与代表画板 | 现有顶部几何正确；字号注释与登录流程存在漂移 |

未覆盖：真机触控、iOS Safari、Android WebView、本轮完整键盘／读屏遍历、200% 文字缩放、全主题全断点组合、完整长中文正文和图片／表格阅读、实际用户可用性实验。Reduced Motion 本轮做源码检查，未做完整运行矩阵。没有据此宣称 WCAG 整体合规或所有平台验收通过。

## 优先发现

### P1：默认深色目录浮层正文透底，影响读取

**事实。** 在 390px 视口打开目录后，底下的文章标题、状态图标和来源／时间按钮与目录文字叠印。固定标题行正确地保持不透明，问题在其下方列表区域。

计算样式为透明底加 `linear-gradient(rgba(255,255,255,.05), rgba(255,255,255,.02))`，`backdrop-filter: none`。`reading-workspace.css:111` 附近的目录背景只消费 `--surface-reader`；默认深色的该变量只有低透明度白色渐变，没有实体底色（`tokens.css:135` 附近）。浅色也有少量透底，但深色最明显。

**建议。** 为浮层内容增加不透明的主题底色，再叠加现有渐变。优先限定目录浮层的表面语义，不全面修改主面板材质或布局。保留已经正确的固定标题、滚动容器、焦点和虚拟列表语义。验收应包含浅／深／系统主题和内置主题，在底层存在高对比文字时截图，而不只检测几何。

证据：`14-dark-directory.png`。这属于可复现缺陷，不是风格偏好。

### P1：部分主题主按钮的文字／填充对比不足

**事实。** Midnight Ledger 的“添加订阅”实测文字为 `#f6ffff`、16px／600，填充为 `#2bb4ae → #1d8f8a` 的渐变，与源码一致。按 sRGB 相对亮度公式计算，两个端点的文字／填充对比为 **2.507:1、3.864:1**。默认深色 `#ffffff / #c05a36` 为 **4.415:1**。当前阴影是偏移 1px、模糊 1px、18% 黑色，并不能作为稳定的完整文字衬底。

普通文字的网页 AA 目标为 4.5:1；16px／600 不属于可使用 3:1 门槛的大字。依据：[WCAG 文字对比度](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)。

**建议。** 分别定义按钮填充及其文字色，检查整条渐变，而不只检查强调色。Midnight 可试验深墨色文字 `#091514`（对现有两端约 **7.301:1、4.737:1**，需去除不适合深色文字的阴影），或压暗渐变后保留浅字。默认深色可调暗按钮填充，例如 `#ad492a` 配白字约 **5.580:1**。这些是候选值，不是已经批准的主题新规范。

Atlas／Amethyst 渐变亮端也有风险，但本轮未对文字覆盖位置完整取样，不把它们整颗按钮一概判失败。禁用按钮的低透明度不能与普通可用按钮按同一规则混算。

源码：`assets/themes/midnight-ledger.css:76`、`assets/styles/tokens.css:165`、`assets/styles/shell.css:443`。证据：`10-midnight-feeds.png`。

### P2：应用主题会连带保存其他设置草稿

**事实。** 实验先把“每页数量”由 50 改为 51，仅点击“应用 Midnight Ledger”，没有点击“保存设置”。随后新建独立标签页读取设置，值已经是 51。主题按钮的成功提示仅说明主题已应用，没有说明其他字段也被保存。

根因是主题应用／清空 CSS 调用 `save_with_message`，最终提交整个 `submit_edits()` 草稿：`settings_page/facade.rs:85`、`settings_page/save/session.rs:78`。

**建议。** 先统一保存模型，再调整布局。倾向保留清楚的“保存全部设置”：主题选择先预览，显式保存时提交全部更改；或者让“应用主题”只保存主题字段。后一方案会改变保存边界，需要独立核对行为契约。不要在纯 CSS 补丁中悄悄改变保存语义。若暂不改行为，按钮旁至少说明“将同时保存其他修改”。

行为学推论：按钮名称所表达的作用范围与实际保存范围不同，会降低操作可预测性；这不是已测得的用户误操作率。证据：`settings-save-evidence.json`。

### P2：手机订阅行操作纵向占用过多

**事实。** 390px 视口下，每个来源的编辑、刷新、删除按钮各为 **315×44px**，三条竖排共占 **132px**。并非点击区太小，而是宽度和排列不匹配图标形式。根因是 `responsive.css:181` 的手机规则仍将 `feed-card-actions` 固定为单列。

统计卡片为两行，新增订阅标题在 y≈405，已保存订阅标题在 y≈659；首屏很大一部分用于统计和表单，而非已有订阅。

**建议。** 三枚行按钮保持各 44×44px，合并为一行，间距建议 4–8px；就操作区可从 132px 降到约 44px。实施时需同时处理手机单列规则及按钮 `width: 100%`，不能只改父容器的排列方式。错误提示和确认删除时允许扩展为文字操作区。统计卡片试验每张 64–72px 高、紧凑数字和标签；“刷新全部”归到已保存订阅标题附近，区分添加单个来源与刷新现有订阅的任务范围。当前刷新按钮已经是次级样式，不属于双主按钮缺陷。所有数值为项目试验起点，需用长名称／大字体回归。

证据：`06-phone-feeds.png`、`07-phone-feed-actions.png`。

### P2：主题选择入口重复，保存和反馈离开当前任务区域

**事实。** 同一套主题有三条选择路径：下拉框＋应用、四枚快捷按钮、主题卡片＋应用。CSS 编辑器位于普通主题选择之前。未启用主题也显示“移除这套主题”；此时点击只会提示当前并未启用。保存设置位于整个主题区域之后，状态提示位于页面顶部。已有 live region，不能说没有辅助技术反馈。

**建议。** 保留一组带预览／当前状态的主题卡片，CSS 编辑和导入导出收进“高级”；只有当前主题提供“恢复默认”。保存策略确定后，将“未保存／保存中／已保存／失败重试”与保存按钮放在关联区域，必要时采用不遮挡最后一个字段的底部保存区。

这是利用渐进披露减少重复选择的建议，依据：[Apple Disclosure Controls](https://developer.apple.com/design/human-interface-guidelines/disclosure-controls)、[Apple Feedback](https://developer.apple.com/design/human-interface-guidelines/feedback)。没有用户测试数据证明所需时间会下降多少。

源码：`settings_page/themes/presets.rs:24`、`:47`、`:88`，`themes/mod.rs:22`，`appearance.rs:15`。证据：`08-phone-settings.png`、`09-desktop-theme-controls.png`。

### P2：删除确认态没有接上危险按钮样式

**事实。** 进入确认态后，文字变成“确认删除”，有红色旁注和状态信息，已有两次点击保护。但按钮使用 `.workspace-icon-button[data-variant="danger"]`，危险样式只匹配 `.button[...]`，因此实际仍是普通图标按钮色；获得交互状态时甚至使用主题强调色。本轮只进入确认态，没有确认删除。

**建议。** 确认态使用明确的危险色文字按钮与就近“取消”；保留订阅名称和操作后果。平时垃圾桶可继续中性色，不必让整个列表长期充满警报红。是否增加撤销涉及恢复语义，另行设计，不随样式修复附带新增。

源码：`feeds_page/sections/saved.rs:82`、`assets/styles/shell.css:470`、`reading-workspace.css:41`。证据：`11-delete-confirmation.png`。

### P2 / P3：命中区、小字号与轮廓需要分开校准

**事实。** 来源／时间按钮实际各 **54×40px**，外围上下 2px 留白让外观达到 44px，但留白不是按钮命中区。其余抽查工具按钮及目录关闭按钮为 44×44px。手机目录正文 12.8px、日期 12px、计数 11.2px；列表元信息 12px。

未选文章复选框为 16×16px、1px 边框，外层命中区 44px。浏览器边框色约 `#cbc3ba`，与 `#fffaf2` token 底色的理论对比约 **1.676:1**；最终背景含渐变，完整非文字对比判定还应按实际组合测量。这是值得加深的可交互轮廓，不是要求所有装饰线都达到 3:1。依据：[WCAG 非文字对比度](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)。

**建议。** 先把分段按钮本身做到项目既定 44px。主要可读的辅助信息试 13–14px、计数 12px；保持 44px 行高与标题两行上限，验证长中文标题截断及目录虚拟测量。提高复选框轮廓的区分度，保留小方框和大命中区的结构。

Apple 当前 HIG 的 iOS/iPadOS 表把 44pt 列为默认控制尺寸、28pt 列为最小；Android 推荐 48dp。这些不是 CSS px 的同义单位。本项目 44 CSS px 是既有产品目标，不能宣称它等于两家的原生规范。网页 WCAG 2.2 AA 的目标尺寸最低要求另为 24 CSS px 且有例外，不应拿最低门槛代替舒适触控目标。依据：[Apple Accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility)、[Android 触控目标](https://support.google.com/accessibility/android/answer/7101858?content_language=English&hl=en)、[WCAG Target Size](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)。

### P3：搜索恢复入口与 R 双重用途可以更易理解

**事实。** 搜索空结果有明确说明，搜索框自身也有清除图标；但空结果正文说“可清除筛选后查看”，没有在该处直接展示清除操作。全局清除入口藏于列表选项。R 的可访问名称固定为“Read / 首页”，其 `title` 才说明在首页再次点击会刷新全部订阅。

**建议。** 空结果区域提供就近“清除筛选”操作并保留现有入口；R 在首页的可访问名称可明确包含刷新含义，保持已约定的导航／刷新语义。触屏不能依赖悬停提示理解按钮。此项优先级低于叠印、对比度和保存问题。

## 尺寸、角度、圆角和动效的具体判断

以下是后续设计试验建议，不是 Apple／Google 要求网页照抄的数值。

| 维度 | 当前值／证据 | 建议与验收重点 |
| --- | --- | --- |
| 主标题 | 桌面 28.8px，手机 24px | 保留，订阅／设置／文章现在统一；长计数在自己的区域省略 |
| 阅读正文 | 桌面 17.28px、行高32.832px（1.9）、宽约690.5px；手机 CSS 16px／1.75 | 基础值合理；先用真实长中文、混排、表格和200%文字检验，再决定是否调到17–18px；`ch`不是汉字计数 |
| 导航 | 外壳46px，可见圆面32px，命中44px | 保留视觉尺寸与命中尺寸分离；不为“紧凑”缩小点击区 |
| 工具栏 | 760px四按钮同排，坐标x=521/565/617/669，y=92，44×44px | 保留稳定位置；按内容窗口宽度与容器断点验收，不按设备名称套尺寸 |
| 间距 | 工具组常用0／8px；主面板内边距桌面16、手机12px | 以4／8／12／16／24为检查基线；保留必要光学校正，不机械四舍五入所有值 |
| 圆角 | 主面板30、卡片22／18、控件16、图标行12、复选框3.5px | 层级有可解释性，不需要全局换圆角；优先减少设置页卡片套卡片及不必要投影 |
| 图标笔画 | 通用20px显示、24 viewBox、1.75单位线；搜索/RSS为2单位 | 需按最终显示大小做光学比较，不能只把SVG数字改成一样；在DPR=1和高分屏检查重心与清晰度 |
| 目录箭头 | 组6px、分支5px，1.5px线；右向−45°、下向+45° | 方向语义正确，保留。可试统一可见尺寸，但不能把45°称为两家强制标准 |
| 快速反馈 | 默认120ms ease；搜索约6px渐入 | 可继续使用；状态色／按下反馈建议试100–140ms，短开合140–180ms。都是本项目调校值 |
| 目录开合 | 当前即时显示，避免虚拟列表尺寸过渡 | 不建议动画高度／宽度，避免文字重排和滚动目标漂移。若加动效，先限于箭头旋转或短淡入，连续反向操作必须可打断 |
| Amethyst | 180／220ms，hover上移2–3px | 阅读场景可试减到0–1px或只变色；装饰效果不能压过当前阅读位置 |
| Reduced Motion | 默认及Amethyst已有覆盖 | 保留；检查位移、弹簧、滚动定位的替代方式。减少动画后仍要有清楚的状态变化 |

Apple 的符号指导强调光学重量、尺寸与对齐一致；Google Material Symbols 也有随显示大小变化的 optical size，不意味着所有图标只能用同一条线宽。[Apple SF Symbols](https://developer.apple.com/design/human-interface-guidelines/sf-symbols)、[Google Material Symbols](https://developers.google.com/fonts/docs/material_symbols)。

当前 Material 3 动效资料已经转向弹簧系统，并区分 Expressive 与更适合工具型产品的 Standard；不能把旧版200／300ms或强回弹说成当前通用标准。阅读器建议借鉴 Standard 的安静反馈，不为套用物理系统而增加依赖。[Material Motion](https://m3.material.io/styles/motion/overview/how-it-works)、[Apple Motion](https://developer.apple.com/design/human-interface-guidelines/motion)。

Apple 当前材质说明将导航／控制层与内容层区分；这不构成让内容全部半透明的理由。Google 强调语义色角色与正确的前景／背景配对。浮层可读性及主题对比度应先于材质风格。[Apple Materials](https://developer.apple.com/design/human-interface-guidelines/materials)、[Apple Color](https://developer.apple.com/design/human-interface-guidelines/color)、[Google Material 3](https://developer.android.com/develop/ui/compose/designsystems/material3)。

## Penpot 审查

当前文件：`cadc1646-c70c-8009-8008-d6f40c2d8fdc`。本轮工具连接成功，所有操作只读。

1. [01 Foundations](https://penpot.develata.me/#/workspace?team-id=f154fe8d-b0f4-80be-8008-d6e9bdc70b86&file-id=cadc1646-c70c-8009-8008-d6f40c2d8fdc&page-id=63fc74ca-8324-800c-8008-d92fcefbf443) 还写订阅标题32px、手机仅文章24px；当前代码与已修复代表画板已经统一28.8／24px。需要同步规范注释，避免后续按旧数值实现。
2. [10 当前登录](https://penpot.develata.me/#/workspace?team-id=f154fe8d-b0f4-80be-8008-d6e9bdc70b86&file-id=cadc1646-c70c-8009-8008-d6f40c2d8fdc&page-id=63fc74ca-8324-800c-8008-d9684766ea06) 仍有4张浏览器本地初始化／解锁稿；该流程已退役。服务端示例还引用旧 `RSS_READER_WEB_SECURE_COOKIE=true`，当前源码提示使用 `RSS_READER_WEB_PUBLIC_ORIGIN`。建议移入历史分类并保留可追溯信息，当前页只呈现实际流程，不删除历史稿。
3. Foundations／Components 已有选中、半选、禁用与静态尺寸，未读到完整focus-visible、pressed、busy、减少动效和动画打断说明。应补一张“交互状态契约”，包含各主题对比度、错误恢复及保存范围，不只继续增加整页静态图。规格缺失不等于运行代码没有这些行为。
4. 已确认正确：订阅／设置桌面导航46px、主面板y=76，手机主面板y=72、间隙6px；760px文章的四个操作同排、44px。未发现用户此前右上角误移再次出现。

## 建议实施顺序与验收

1. **可读性与明确缺陷。** 深色目录实体底、按钮文字配色、危险确认样式、分段真实命中区。范围可以小，保持既有命令和数据语义。
2. **空间与信息组织。** 手机订阅操作横排、统计区压缩、主题入口合并、高级CSS渐进披露。先用390／760／1200及实际断点两侧、长中文和大字体验证，不引入新功能。
3. **保存与反馈。** 单独确定“即时／预览／保存”的一致模型，验证未提交草稿、失败重试、离页和主题切换。此项涉及行为，不能当纯美化处理。
4. **设计规范补齐。** 同步Penpot当前页、交互状态表与主题颜色对；保留历史稿。新增截图要覆盖暗色浮层覆盖高对比内容的情况。

适应性参考：[Google Window Size Classes](https://developer.android.com/develop/ui/compose/layouts/adaptive/use-window-size-classes)、[Apple Layout](https://developer.apple.com/design/human-interface-guidelines/layout)。窗口分类不等于照搬600／840dp为本项目CSS断点。

## 验证与验收记录

- `git status --short`：审查开始时干净；本轮仅新增此报告。
- `git rev-parse HEAD`：核对为上述HEAD。
- `rg` 与相关源码／设计契约读取：已执行；一条附带查询的 `assets/styles/content.css` 路径不存在，实际结论取自存在的 `responsive.css`、`workspaces.css`、`reading-workspace.css` 等文件，不使用该失败查询作为证据。
- 对比度：按标准sRGB线性化及相对亮度公式独立重算上述候选与当前色值；渐变／透明图层边界按上文注明。
- 浏览器：执行表中9项；主题连带保存用新标签页确认；未确认删除任何订阅。主题和每页数量实验结束后已恢复默认CSS及50篇设置；最后为了暗色目录复现将隔离库主题设为深色。
- Penpot：读取当前页图层、文字和代表几何，无写入／版本保存。
- Rust格式／编译／测试：本轮未执行，因未修改生产代码；前轮检查不能替代今后修复的验收。

## 结果、风险与后续事项

本次交付为有证据的审查与待实施建议。P1问题未修复；不得把报告状态理解为产品可发布证明。尺寸优化、认知负担与阅读舒适度的收益仍属设计推论，应结合真机与实际读者验证。浏览器资料库是隔离测试数据，原用户工作区和资料库未改。

下一位Agent应先看本报告的P1与截图，再读取现行设计契约；不要把本报告建议数字直接写成已批准的强制规范。若用户授权实施，应分开纯视觉修复与设置保存语义调整，并重新检查实际运行结果和Penpot。
