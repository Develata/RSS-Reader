# Reader 规则独立性样本与 PR #49 整合

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/reader-rule-boundary`
- 工作树：`E:/gitclone/RSS-Reader-worktrees/native-back-policy`
- 产品基线：`4cef074c54d7df54808e7892f950f56a7a750232`
- 产品 commit：`82beaf5219988117b2908a05afff6c2222e1c32a`
- PR：[Reader 规则独立性 #50](https://github.com/Develata/RSS-Reader/pull/50)，已推送，保持未合并
- 本交接和维护文档：随本文件的文档提交；以 `git log -1 -- docs/handoffs/2026-10-30-reader-rule-boundary.md` 定位
- 相关 tag / release：无；未发布
- 状态：`validated`（本地、正式 Web 与所列实际宿主）；本批分支尚未合并，远端检查以分支 PR 当前 head 为准

## 工作摘要

按用户确认的方向，让未来 Rust 原生界面复用同一交互规则，JS 行数只作辅助观察。先复核 PR #49 head `b03ebc4e`、CI 36/36、无未处理评审，再按 expected-head 合并至 `4cef074c`；合并树与 head 相同。本地 main 快进，使用新分支继续复用 E 盘工作树。合并 main 的 CI 36/36、Pages/Docker workflow 成功；不是新的容器手工验收。

本批以 Reader 的普通状态、读取归属、图片查看和既有快捷键规则为样本，验证规则可以独立编译和调用。没有发现需要混入本批修复的已确认产品缺陷；源码 review 未发现未处理的阻塞项。

## 影响范围与关键变更

- `reader_page/reducer.rs` 移出 `Signal` dispatch，由原 session 执行响应式发布。state/intent 使用原 domain `EntryNavigation`，不再经过平台 bootstrap。状态、类型、Backend 与业务语义不复制。
- 图片普通数据和开关规则移至 `image_viewer_state.rs`；hook、组件、eval、DOM 事件、模态焦点/滚动与卸载仍在当前适配器，脚本内容未改。
- 扩展原 `ReaderResultScope`：访问身份由文章 ID + 原状态的 load generation 组成；只读刷新额外捕获 reread generation。加载、刷新和 App 标记写入复用判断；刷新代次不能取消同访问的标记写入。
- `plan_reread` 只输出忽略、恢复加载、等待加载或仅读标记/邻居。session 保留先发布 `BeginLoading/RereadIssued` 再捕获身份、加载完成后处理待刷新，以及恢复读取不重复本地化图片的顺序。读取/自动处理仍归页面，已确认写入仍归 App，离页回执仍走 Shell。
- `tests/reader_rules.rs` 引用同一生产模块及既有测试；`scripts/check_reader_rule_boundary.py` 只构建既有 API 依赖图，再用 rustc 显式链接 API/domain/serde 编译该测试程序，拒绝 Dioxus/web-sys/Wry/Tao。没有测试替身、规则副本或新增 Cargo package。CI 在 `rssr-api` job 增加此检查。
- 既有原生返回运行器增加 `--reader-only true`，复用隔离包/进程身份、fixture 与 CDP，集中检查本批触及的路径。
- 当前状态、维护待办、适配契约、原生前置与验收入口同步到用户确认的顺序。独立设置面板保留候选；快捷键定义去重、宿主入口进一步收敛和正文边界是后续批次。

## 验证与验收

| 命令或场景 | 实际结果 |
| --- | --- |
| `cargo test -p rssr-app --locked` | 390 unit + 23 同生产模块的独立 harness + 5 既有主题契约 = **418 通过，1 忽略**；新增读取生命周期序列 4 项，既有 Dioxus 延迟加载、只读刷新与离页写入测试继续通过 |
| `python scripts/check_reader_rule_boundary.py --target-dir target/reader-rule-boundary-20261030/rules-build` | **23/23**；不链接 UI 框架，不创建 VirtualDom；与上行的 23 项是同一测试源码，不累加为不同规则覆盖 |
| fmt / Clippy / Wasm check | `cargo fmt --all --check`、`cargo clippy -p rssr-app --all-targets --locked -- -D warnings`、`cargo check -p rssr-app --target wasm32-unknown-unknown --locked` 全通过 |
| Web / Android 构建 | 最终均使用已校验的 dx 0.7.9；Web release 带 `--debug-symbols false`，Android 使用 JDK 21 / NDK 28.2、独立源码与 target、仅测试包身份 overlay。最终日志无 ERROR |
| Windows 实际 WebView2 `156.0.4314.50` | 基线 `windows-baseline-02` / 候选 `windows-candidate-01` 各 **4/4**，1280/390 两视口；每视口含三次阅读返回、位置恢复、图片 Esc 关闭/回焦/滚动恢复、M/F 保留正文、输入排除及 J/方向键导航 |
| Android 15 真机 `caa0c283`，WebView `156.0.8078.47` | 基线 `android-baseline-01` / 最终候选 `android-reader-rules-candidate-01` 各 **2/2**；独立 `backpolicytest` 包，真实系统返回关闭图片后仍停留 Reader，其他输入由 CDP 执行 |
| 正式 Web / 当前 rssr-web / SQLite | 复用 `readerWritesSurviveNavigation`，已读与收藏两种延迟写入在离页后成功；HTTP 延迟来自受控路由，不称作真实网络故障 |
| 页面异常与截图 | 所列最终宿主/Web 回执的 page/console errors 为空；Windows 8 张、Android 4 张对应 PNG **逐字节相同**。另查看候选 Windows 阅读与 Android 图片窗口截图；不推广为所有页面/设备视觉验收 |
| 文档、输入与脚本检查 | 329 份 Markdown / 1542 个本地链接，无新增或变更文档错误；回执 JSON 可解析，393 个产品输入与构建快照匹配，`git diff --check` 与脚本语法通过 |
| PR #50 的 `050efdd7` / CI | [run 41127100378](https://github.com/Develata/RSS-Reader/actions/runs/41127100378) 首轮 34 成功、偏好事件等待及汇总门禁失败；不改代码仅重跑失败作业后，最终 **36/36**。新增无 UI 框架编译步骤成功；Pages 构建通过。此结果属于该提交，后续仅文档提交的 Checks 仍按 PR 当前 head 核对 |

### 失败记录与验证边界

- Windows 第一轮在窄窗口的新即时滚动断言处失败，焦点检查已通过；当时未记录实际激活位置，不能据此认定产品缺陷或宣称已修复根因。运行器补充实际点击时刻的位置事实，基线和候选均通过并保留比较值。旧失败未覆盖，没有修改产品滚动规则。
- 最初调用全局 dx 0.7.10，Web 命令遗漏 `--debug-symbols false`，复现既有 wasm-opt DWARF 崩溃；dx 仍退出 0。按仓库记录使用已验证 SHA256 的 dx 0.7.9 并加参数后重新构建 Web/Android，最终产物与日志另存。没有覆盖全局工具或降低优化级别。
- Android 首个产物定位脚本用了错误目录；备份已完成，未进入资料库切换。安装 helper 首次因实际 debug 标签为 `RssrApp` 而拒绝；仅在本批副本中匹配已核验标签与固定测试包 ID 后安装。一次设备端目录名与历史证据冲突，保护检查在移动前拒绝，改用本批专用名字继续，未删除旧目录。
- Pillow 的本机二进制导入失败；未改全局安装。12 对 PNG 文件直接逐字节比较相同，足以证明这些图片一致。
- PR #50 的 `050efdd7` 首轮 CI（run `41127100378`）在既有 `preferenceOrdering` 等待最终 PUT 的 `requestfinished` 事件时超时；响应成功断言已通过，其余 34 个作业通过，汇总门禁随该失败而失败。列表偏好和这份测试均未被本批修改。保存了原始日志与失败页面；本地 Chrome `155.0.8059.40` 按原顺序运行前置 9 场景，再连续 5 轮原偏好测试，均通过，未记录到 workspace 请求失败或页面异常。随后同一提交仅重跑失败作业，完整 reading-workspace 与汇总门禁均通过，最终 36/36。尚未复现，也未确认根因；没有扩大超时、放宽断言或修改产品逻辑。后续若再出现，应增加该请求的结束/失败事件诊断，不把本次重跑通过当作根因修复。
- 重复按键/composing 是受控事件事实；不声称真实硬件键盘、系统输入法、辅助技术、macOS/Linux GUI、正式签名升级或性能验收。当前宿主通过也不证明任何新渲染器已可用；完整 session/runtime 仍有 Dioxus 装配职责。

## 产物、证据与数据保全

紧凑结果见[结构化回执](../testing/evidence/2026-10-30-reader-rule-boundary/receipt.json)。完整证据在 `target/reader-rule-boundary-20261030/`，包括命令日志、源补丁/输入清单、测试报告、APK、隔离资料库、安装回执及失败记录。

- 产物在提交前构建；`product-files.json` 的 393 项输入逐一匹配产品提交，清单 SHA256 为 `94ac16a8a93ff60d1bc5d11489de8df3d86ceed77545712c63fd6f7ad783fa7e`。后续文档提交不冒充重新构建；实际构建标识留在日志和 process receipt。
- 最终 Android APK SHA256 `2c8e245d519dc8d8644daa3c36b711c83440cad54ed4947c051862196c4c5856`，库 SHA256 `63ed0b7f1ea5ec40383d1f9f5b64c52890e84ab88aefe6e3507b3a5044864160`。APK/生成 JNI 库/Cargo 输出一致，`.d` 指向本批源码快照，生成 Activity 与测试包 overlay 一致。初次 dx 0.7.10 包另存，不作为最终候选。
- 基线 EXE / APK 复用前批已验产物；其产品输入与 PR #49 合并结果一致。不是重新构建的基线。各宿主最终实际运行的包/EXE 散列见结构化回执和 process.json。
- 测试包旧 APK 和 files 全部备份校验；每次验收后恢复原资料库，6 个原文件散列相同。日常包 APK 散列不变，原 3 条 forward 和 1 条 reverse 保留，仅移除本批映射；最终独立测试包保留新 APK 和测试证据。
- 主 checkout `feat/reading-workspace-20261024` 的 HEAD、index、状态及本批开始时 41 个文件/缺失路径一致。未切换、清理或提交它的内容，另一 session 的审计文档保留原处。

## 结果与后续

本批完成 Reader 规则独立性与现有行为保持的有界样本；没有改变七个 crate、Backend、数据库、渲染器、JS 产品脚本或业务语义。后续先收敛 Reader 的宿主入口，快捷键定义去重另作小提交并保留同步输入/捕获时序，再明确正文渲染兼容边界。保持现有长期契约为唯一入口，见[原生前置说明](../design/native-ui-preparation.md)。

下一位 Agent 先核对分支、最新 main 与对应 PR 当前 head；本记录的测试不能沿用为之后代码通过。保留主 checkout 和历史证据，本批没有删除分支、打 tag 或发布。
