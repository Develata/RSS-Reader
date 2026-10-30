# Reader 快捷键模块归属与依赖方向整理

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/reader-shortcut-definition`，复用 E 盘 `native-back-policy` 工作树
- 审查基线：`9368d7aa095b7bd7649ff0bb64015849c1e2ef4a`；其产品代码为 `ddb7045e`
- 相关 commit：本记录所在提交，同时包含实现、文档和精简回执；准确产物以输入清单及哈希核对
- 相关 tag / release：无；不合并、不发布
- 状态：`validated`（本地编译、规则及实际宿主；最终 head CI 单独核对）

## 工作摘要

按高内聚、低耦合的要求复核 Reader 后，发现两处明确的模块归属问题：共享阅读位置适配器反向调用 Reader 页面里的脚本生成函数；Reader 又通过全局快捷键 hook 回到自身的 session 和 WebView 模块。本批整理这两条依赖，延续 PR #52 的快捷键边界，不新增分支或扩大成全局输入框架。

这不是已复现的运行缺陷。按键规则、动作执行、DOM 脚本和监听生命周期保持原行为。

## 影响范围

- 普通规则和当前宿主配置：`crates/rssr-app/src/ui/reader_shortcuts/`。
- 页面装配：`pages/reader_page/shortcuts.rs`、Reader 页面及其私有 `webview` 模块。
- 共享阅读位置接线、`hooks.rs`、同一生产模块的独立测试入口。
- Windows WebView2、Android 真机、正式 Web；设计契约、验收入口和维护状态同步更新。

七 crate、Backend、业务状态、数据库、页面树与 CSS 保持现状。

## 关键变更与 review

1. 普通 Rust `policy.rs`、当前 WebView 脚本生成与同步过滤 JS 聚合到 `ui/reader_shortcuts/`。规则仅向外提供动作和解析，绑定表及捕获属性收窄为模块内部可见。
2. Dioxus/session/router 快捷键 hook 移到 Reader 私有 `shortcuts.rs`。页面 `webview` 改为私有，`use_shortcut_keys` 只向父模块开放，减少可被其他页面误用的入口。
3. App 阅读位置和 Reader WebView 分别调用同一个共享脚本生成入口。该模块不导入 Dioxus、页面或路由，也不拥有监听或额外状态；App/页面原有作用域和取消规则保持不变。
4. 对照基线核验：规则只改两处可见性，动作派发函数体不变，配置生成函数体仅更换路径/名称；5 个相关 JS 文件逐字节一致。移动后的生产规则仍由原独立测试程序直接编译。

未发现新的产品阻塞问题。本次是源码自审；核对 PR #52 的 reviews、inline threads 和 conversation 均为空，不称作外部批准。整个 `ui`、session/runtime 仍有现有框架装配职责，此次不声称全部脱离页面或 Dioxus。

## 验证与验收

证据根：`target/reader-shortcut-cohesion-20261030/`；提交内的精简回执见 `docs/testing/evidence/2026-10-30-reader-shortcut-cohesion/receipt.json`。历史结果没有作为本批通过依据。

### 本地编译与规则

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all --check`、`git diff --check` | 通过 |
| `cargo test -p rssr-app --locked` | 420 通过、1 忽略：391 单元、24 独立规则、2 主题、3 样式 |
| `python scripts/check_reader_rule_boundary.py --target-dir target/reader-shortcut-cohesion-20261030/rules-build` | 同一 24 项生产规则通过，无 Dioxus/WebView 依赖 |
| `cargo clippy -p rssr-app --all-targets --locked -- -D warnings` | 通过 |
| `cargo build -p rssr-app -p rssr-web --locked` | 通过 |
| `cargo check -p rssr-app --target wasm32-unknown-unknown --locked` | 通过 |
| `dx 0.7.9 build --platform web --package rssr-app --release --locked --debug-symbols false` | 通过，正式同源服务包 |
| `dx 0.7.9 build --platform android --package rssr-app --target aarch64-linux-android --locked` | 隔离源码快照与独立 target，JDK 21、NDK 28.2.13676358；通过，保留既有 `LOCAL_DATA_DIR_NAME` 未使用警告 |

`verify-cohesion.py` 记录移动与依赖核对；`verify-android.py` 核对全部 464 项产品输入、生成 Activity、依赖源码路径，以及 Cargo/JNI/APK 内动态库一致。Android 快照只对独立测试包标识和 BuildConfig 别名作既有覆盖。

### 实际宿主

| 平台 | 环境 | 基线 / 候选 |
| --- | --- | --- |
| Windows | 实际 EXE，WebView2 `156.0.4314.50`，1280/390 两视口 | 各 8/8；`run-windows.ps1 -Variant baseline/candidate -RunName windows-*-01` |
| Android | 真机 `caa0c283`，Android 15，独立 `backpolicytest` 包，392 视口 | 各 4/4；`run-android.ps1 -Variant baseline/candidate -RunName android-cohesion-*-01` |
| 正式 Web | 实际 rssr-web 和隔离 SQLite，Chrome `155.0.8059.40`，1280/390 | 各 4/4；`node scripts/browser/rssr_reader_host_acceptance.cjs`，显式指定各自产物与新证据目录 |

三端复用同一未修改的宿主脚本；每视口 70 个过滤/阶段边界、10 个有效键值、6 次实际导航/返回，并检查标记写入、图片层、回焦、滚动及卸载。Android 图片层关闭使用实际系统返回。所有正式回执无页面或控制台错误。

视觉复核：首轮 22 对图中 21 对像素一致；Windows `1280-reader-after-shortcuts.png` 仅返回箭头区域有 47 像素差异，范围 `[160,26,180,36]`。第二轮基线/候选的两个精确图像哈希交叉出现，说明该差别不稳定随版本变化。第二轮另有一张窄屏图在返回按钮范围内差 1413 像素。

随后用 `observe-screenshots.cjs` 仅包装测试截图：先保留原图和按钮事实，再移开指针到 `(0,0)`、等 350 ms、补拍对照，不改 CSS、焦点、应用回调或断言。两视口对照均逐字节相同；窄屏对照恰与第二轮候选的那张图哈希相同。记录确认移开前 hover 的 `translateY(-1px)`、移开后 transform 为 none，两端保持焦点和 focus-visible。所有 12 种 Windows 原截图都在两版本出现过相同的精确图像；Windows 三轮基线/候选各 8/8。

结论是未发现稳定的结构或视觉回归。首两轮缺少当时 hover 事实，不反推每张差异图的精确原因，也不将挑选出的匹配图宣称为首轮全部相同。原图、差异、受控对照及哈希全部保留在 `host-results.json`、`host-results-final.json`、`visual-review.json`。

### 产物身份与保全

- 产品输入清单 SHA-256：`cfa3a2d7d7554c0efef020f7e3ac458cea22b9e1fed3d2e83a31e7833316769c`。
- Windows 候选 EXE：`16e52e7921313fb0d7220cf64d9afec4b0e006f4422ddd948b36bd59866da9ea`。
- Android 候选 APK：`535f37a69aa208d64acb840cc428e33de3bd38ecca235cf3aa40d88f63167cfb`；安装后设备哈希相同，没有清除资料。
- 正式 Web 候选 server/build：`dev-9368d7aa095b-dirty-0bd8abcd08b5`；各 public 文件哈希与 server EXE 哈希保存在证据根。
- 主 checkout 的 HEAD、分支、status、index 和 41 个文件/缺失路径均与开始时相同。测试包原 6 个资料文件、日常 APK、既有 adb 映射全部复核保留；测试过程只使用隔离夹具。

汇总图片使用工具自带 Python 3.12/Pillow。系统 Python 的 Pillow 二进制导入失败，未改动系统配置或安装包；该环境问题不是产品失败。

## 结果、风险与后续

- 本地行为检查通过，保持 PR #52 依赖 #51（前置 #50）的结构。最终提交、远端 CI 以 PR Checks 和证据根 `ci-receipt-final.json` 核对；推送前不预报 CI 通过。
- 合成 KeyboardEvent、扫描探针和受控路由切换不证明物理键盘、系统 IME 或辅助技术。未执行 macOS/Linux 原生交互、候选渲染器、Docker 或发布验收；Android 编译与实际宿主结果分开记录。
- 既有 CI 偏好响应结束等待的偶发失败仍待确认根因，见[前批记录](./2026-10-30-reader-shortcut-definition.md#ci-失败与独立诊断)，不能被本次模块移动宣称解决。
- 下一步按依赖顺序收口 #50–#52，再明确正文内容、图片资源和阅读锚点与当前 HTML/CSS、复制、IME、无障碍的兼容契约。不继续为拆文件而拆文件。

## 给下一位 Agent 的备注

先读[依赖方向契约](../design/native-ui-preparation.md#模块归属与依赖方向)，再看共享 `policy.rs` / `webview.rs` 及 Reader `shortcuts.rs`。App 和页面只共享规则与脚本，不共享生命周期。重跑宿主脚本必须使用全新证据目录；主 checkout 的其他任务改动、旧 target 证据及日常手机包继续保留。
