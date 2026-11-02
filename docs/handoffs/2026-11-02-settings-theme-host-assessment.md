# Settings 主题导入与预览宿主入口评估

- 日期：2026-11-02
- 作者 / Agent：Claude（Opus 5.5）
- 分支：`claude/nice-diffie-80f614`
- 评估基线：`f6c4a0cd`（main `2571f56a` 之后仅有文档提交，产品源码相同）
- 相关 commit：本记录及索引更新的文档提交；无产品源码改动
- 相关 tag / release：N/A
- 状态：`validated`（只读评估；结论按源码核实，未运行宿主）

## 工作摘要

核对 [维护待办](../maintenance-backlog.md#原生界面前置的当前顺序) 的下一候选：Settings 主题导入与预览的宿主入口（`themes/theme_io.rs`、`theme_file_io.rs`、`theme_android_import.rs`，以及经 `facade.rs` / `session.rs` 的预览路径）是否仍有实际复用阻碍。

**结论：没有需要现在处理的复用阻碍，本批不改代码。** 平台 `cfg` 只包住选择器、任务和组件入口；跨宿主的导入准入与结果映射已经是单一来源；没有页面到 runtime 的反向依赖。仍依赖 Dioxus 的部分正是[原生前置边界](../design/native-ui-preparation.md#settings-草稿与回填边界2026-10-31)已接受留在适配层的职责，且其中的 CSS 预览在原生渲染器中尚无定义的使用方，现在提取属于推测性设计。

## 影响范围

- 模块：只读核对 `crates/rssr-app/src/pages/settings_page/{facade,session}.rs`、`themes/{lab,theme_io,theme_file_io,theme_android_import,theme_android_export,theme_apply,theme_preset}.rs`、`themes/theme_android_import/bridge.rs`、`sync/session.rs`、`preferences.rs`。
- 平台：评估覆盖 Web / 桌面 / Android 三条源码路径；没有运行任何宿主。
- 额外影响：更新维护待办、当前状态和交接索引。

## 逐项核对

### 1. 平台 cfg / web_sys / Dioxus 混入规则：未发现

- `theme_io.rs` 的 cfg（1、3、8、11、14、19、59、67、73 行）都只包住 `spawn`、原生选择器入口或 Web 下载入口。唯一跨宿主的规则函数 `complete_css_import`（34–51 行：先消费身份，再分 `Ok(Some)` 预览 / `Ok(None)` 取消提示 / `Err` 错误提示）不含 cfg。
- `theme_file_io.rs` 是纯宿主文件能力：`rfd` + `tokio::fs`（10、16–19、72–85 行）、`web_sys`（47–60、93–131 行）、Android 转发（3–6、65–68 行）。原生函数不依赖 Dioxus；唯一依赖 Dioxus 的是 Web 专用 `read_css_file_in_browser` 的 `dioxus::html::FileData` 参数（29 行），它只服务 Dioxus Web 宿主。唯一的规则 `decode_css_file`（23 行，严格 UTF-8）是无 cfg 的普通 Rust。
- `theme_android_import.rs` 的 `ImportRequests` / `ImportWait`（14–75 行）是普通 Rust（`Mutex` + `oneshot`），只在 `bridge.rs` 接 JNI。
- `lab.rs` 的 cfg（6–82 行）在组件内选择两种触发方式：Web 需要隐藏 `input[type=file]` 与用户手势，原生直接等待选择器 Future。这是宿主差异，留在页面组件/适配层，没有进入 application/domain。

### 2. 重复的请求身份或取消逻辑：有两处相似，均不构成阻碍

- **页面身份与 Android 传输编号不是重复。** `ThemeImportRequest`（`session.rs:52–81`，u64 代次）负责业务回填准入；`ImportRequests`（`theme_android_import.rs:25–56`，i64 槽位）负责宿主等待与物理选择器占用。[适配契约](../design/native-control-adapter-contracts.md)“Android 主题导入”一行和 `theme_android_import.rs:1` 都明确两者分工。
- **Web 与原生的准入顺序各写了一遍。** Web 在 `lab.rs:60–65` 点击时 `begin`，暂存在 `picker_request` Signal（19 行），`onchange` 中 take → `is_theme_import_current` → `complete_css_import`（33–45 行）；原生在 `theme_io.rs:20–32` 中 begin → spawn → 轮询前检查 → `complete_css_import`。这是控制流相似，但两边消费同一个 session 身份和同一个结果函数；Web 拆成两步，是因为文件输入事件不能被 await，而且身份必须在打开选择器时分配（[2026-10-28 导入归属交接](./archive/2026-10/2026-10-28-theme-import-ownership.md)）。未来原生桌面复用的是 `theme_io.rs` 这条路径，Web 版本不会被它调用。合并两者需要先抽象“可等待的文件选择”，这超出本候选，也没有使用方。

### 3. 页面到 runtime 的反向依赖：无

`rg` 核对 `crates/rssr-app/src tests` 后确认：`detect_preset_key`、`preset_css`、`preset_display_name`、`validate_custom_css` 及主题导入方法只在 `settings_page/` 内引用（`save/session.rs:151` 仅测试使用）。`ui/runtime` 没有引用 Settings 主题模块。`facade.rs:1` / `session.rs:14` 经含组件的 `themes/mod.rs` 取预设函数，但这两个文件本身就持有 Signal；上一批已修正的是普通规则 `save_policy.rs` 对该聚合模块的依赖，现在没有新的同类引用。`draft_projection.rs` 只输出 `invalidate_theme_import` 布尔值（17、42 行），不需要预设识别。

## 仍依赖 Dioxus 的部分及接受理由

- `complete_css_import`（`theme_io.rs:34`）和 `facade.preview_custom_css`（`facade.rs:105–118`）接收 `&SettingsPageFacade`，其中的 Signal 由 session 持有。第二种 UI 不能不带 facade 直接调用它们。它们只做准入消费、domain 校验、草稿/预设/预览发布和提示文案组合；[原生前置边界](../design/native-ui-preparation.md#settings-草稿与回填边界2026-10-31)已把“预设识别、文件选择、任务作用域”列为当前适配层职责。
- 导入身份的失效触发分布在 `session.rs:84、160、360`、`facade.rs:39、106、141` 和 `sync/session.rs:97`，对应[适配契约](../design/native-control-adapter-contracts.md)“主题导入”一行列出的事件。把计数器单独移成普通 Rust 不会减少这些调用点；要集中它们，就得把编辑/预览/保存编排整体移出 session，也就是待办明确不做的“提前替换整页或统一页面控制器”。
- `session.rs:175–191` 的预览外观投影（显式预览 CSS 优先于已存 CSS；字号越界时回退到已存值）依赖 `crate::theme`（`theme.rs:1` 引入 Dioxus）。自定义 CSS 预览只对 HTML/CSS 渲染器有意义；[主题选择评估](../design/native-theme-selector-assessment.md#下一步设计的完成标准)要求原生设置面板先说明“CSS 能力如何解释”。在这份设计出现之前，这一投影没有第二个使用方。

## 观察（不是阻碍，不在本批修改）

- **Android 导出没有请求编号或 Drop 释放。** `theme_android_export.rs:21` 只有单个 `Option<Sender>`；页面离开后槽位保留到宿主回调。导入有 `ImportWait` 的 Drop（`theme_android_import.rs:68`）。适配契约已写明导出没有“最新请求”规则，本批不改。
- **Web 空文件选择不提示。** `lab.rs:35–37` 在 `onchange` 没有文件时只消费身份、不设提示；原生 `Ok(None)` 会提示“已取消载入 CSS 文件。”。由于触发前已清空 input 值（`theme_file_io.rs:57`），浏览器取消通常不触发 `change`，这一分支实际很少出现。本批没有用浏览器复现。
- **Android JNI 符号绑定 Dioxus 模板的 Activity 包名。** `bridge.rs:79`、`theme_android_export.rs:95` 的 `Java_dev_dioxus_main_MainActivity_*` 依赖 `dev.dioxus.main` 包名。更换 Android 宿主时这里是明确的替换点；它属于宿主打包，不属于规则。
- `theme_preset.rs` 没有 `use` 依赖，以后某个独立规则探针需要预设识别时，可以直接用 `#[path]` 纳入 `tests/settings_rules.rs`。现在没有使用方，所以不加。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过。
- `git diff --check`：通过。
- 本批没有改 Rust 源码，所以没有运行 clippy、workspace 测试和 wasm32 check。这些不是本批的通过项；最近一次结果见[上一批交接](./2026-10-31-settings-rule-boundary.md#验证与验收)。

### 手工验收

- 未执行。本批是只读源码评估，不声称任何宿主行为结论。

## 结果

- 候选完成评估，结论是没有实际复用阻碍。不提取模块，不新增 crate、全局 Host trait 或 UI 框架。
- 对用户无影响；不发版。

## 风险与后续事项

- 本结论的前提是“没有第二种实现”。若开始实施[主题选择评估](../design/native-theme-selector-assessment.md#下一步设计的完成标准)中的 Windows 独立原生设置面板小样，应先在该设计里说明 CSS 预览如何解释，再回来判断：导入身份计数器、预览外观投影和 `complete_css_import` 的结果映射是否需要移成普通 Rust。届时复用现有身份和契约，不另立第二套。
- 以上是推断，不是已批准的下一批。

## 给下一位 Agent 的备注

- 入口：`themes/theme_io.rs` 的 `complete_css_import`、`session.rs` 的 `ThemeImportRequest` 区段、`facade.rs` 的 `preview_custom_css`，以及[适配契约](../design/native-control-adapter-contracts.md)的“主题导入 / 主题文件 / Android 主题导入”三行。
- 不要因为 `lab.rs` 与 `theme_io.rs` 的控制流相似就合并 Web 和原生路径；身份必须在打开选择器时分配，Web 的文件输入事件不能 await。
