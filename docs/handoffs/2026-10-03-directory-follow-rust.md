# 文章列表目录 Rust 状态统一与新契约验收

- 日期：2026-10-03
- 作者 / Agent：Codex
- 分支：`test/directory-regression-baseline`
- 当前 HEAD（启动基线）：`9815d4e8c04e124279cdabbc55de78c07a064371`
- 相关 commit：实现 `dbe51a38d9d88dba7ad5b3769dba238b2b03e19e`；本次后续文档提交只绑定证据与交接，不改变受测生产源码
- 相关 tag / release：N/A；PR19 保持 draft，不 merge
- 状态：`validated`（本地）；PR 保持 draft，远端 CI 单列

## 工作摘要

按用户澄清的目录规则统一 Rust 状态，修订 PR19 旧的“活动组不可折叠”契约。在目录内可自由折叠当前组；用户滚动主列表即恢复跟随，即使仍处于同组，也清除其他手动展开组。

## 影响范围

- 生产：`rssr-app/src/pages/entries_page/directory.rs`、`directory/{view.rs,adapter.js}` 及原 controls / facade / 页面接入点。
- 测试：既有目录 CDP fixture / 小视口 CI 入口；没有增加 workflow。
- 文档：目录设计、主题状态约束、历史基线说明、[行为与性能验证](../testing/directory-follow-validation.md)、1200 个原始样本、本交接及上轮已授权的设计入口。
- 平台：Web / Windows WebView2 为本轮实测目标；Android 原生和辅助技术实测不在本机结果中冒充通过。

## 关键变更

- Rust 区分初始化路径和实时观测，统一跟随/手动模式及展开集合；顶部和右侧目录独立订阅，复用 presenter `Arc`。不把目录状态加入 `EntriesPresenterInput`，不复制分组树。
- JS 仅承接输入与实际滚动来源、锚点测量、平台滚动。目录滚动不恢复跟随，主列表用户实际滚动可在同组恢复；程序化导航与对齐不伪装用户输入。
- 明确开/关意图来自已显示按钮；bridge 一条在途，其余观测与各按钮显式意图按键合并，保留最新顺序。代际拒绝旧上下文，卸载显式取消监听和 rAF。
- 每个目录消费者输出提交版本，adapter 等实际 DOM 版本一致后测量对齐；不以一次 rAF 假定 Rust 已提交。
- 保留折叠 DOM，没有引入 lazy/卸载。性能和内存结论以实测为准。
- 旧 PR19 四个键盘失败 JSON、原始基线 CSV 和交接保留为历史资料。新 oracle 独立要求鼠标 / Enter / Space 真正折叠，再以真实主列表滚动验证恢复和清空其他组。

## 验证与验收

### 最终本地结果

- `cargo test --locked -p rssr-app directory -j 1`：首轮 6 项通过，包括 4 个新状态测试和 2 个既有 presenter 测试。
- `dx build --platform web --package rssr-app --release --locked --debug-symbols false`：通过。沿用本机 dx 0.7.10 / Dioxus 0.7.9，工具报告版本差异但构建成功。
- `node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:18124 --static-base http://127.0.0.1:8124 --artifact-dir target/directory-follow/web-final-verified`：242 pass，0 fail，console error 0；包括 90 项目录断言和既有阅读位置、图片、刷新等流程。最终源码 release Wasm SHA-256 `d6ed464954778cb89b9d7881435b547e1ff2f161ffd17bf43ae6ec13ba145aa9`。
- 原生 release：`cargo build --release --locked -p rssr-app -p rssr-cli -j 1`，通过；exe SHA-256 `749dd3510827d1bedd7adf5a0d0319afa8703c53c777a7b26ef72d71ef12aa11`。本任务隔离 Windows 窗口经明确 CDP target 执行 `--directory-only true`：79 pass、0 fail、console error 0，产物 `target/directory-follow/native-contract-final-2/`。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -j 1 -- -D warnings`：最终源码通过，日志 `clippy-exact-source.log`。
- `cargo test --workspace --locked -j 1`：334 pass、0 fail、2 个既有 ignored，40 个 test/doc-test 结果；日志 `workspace-exact-source.log`。
- Git Bash 内 `export PATH="/usr/bin:/bin:$PATH"; bash scripts/run_release_ui_regression.sh --release --skip-build --no-serve --log-dir target/directory-follow/aggregate-exact-source`：自动门禁通过（wasm check、app tests、3 个原生 contract harness、rssr-web tests）。该模式跳过 browser harness、服务 smoke、固定 smoke 和静态服务；不把 skipped 写成通过。Web UI 单独执行；wasm browser、主题矩阵与 Android 构建等待最终 SHA 的 CI。
- JS 语法、`git diff --check`：通过。
- 同一新契约反测旧 Web 产物：48 个断言 pass、3 个 fail 报告（其中聚合 19 个场景失败），符合旧行为不满足新规则的预期。四个活动组键盘场景均在“实际折叠”失败；原始旧四个键盘失败文件未修改。
- 同宿主 release 每端 A1/A2 → A3/B1 → B2/A4，各 600 个原始样本。Web 高亮/展开合并中位数减少约 16.6–17.1ms；Windows 展开减少约 6.3–6.7ms；其余路径接近基线。未发现这些路径的代理时延退化，不能据此声称物理显示/掉帧改善或总内存下降。精确数据、A/A 噪声、内存与构建差异见验证报告和 `baselines/2026-10-03-directory-follow-*`。

### 测试自审与中间失败

- 第一轮 Web 目录回归 81 项通过、3 个场景失败：两项键盘场景未把焦点明确移出目录；一项局部折叠场景仍假设旧实现默认展开多个组，所以目录实际没有溢出。补上明确焦点和真实溢出的前置断言。
- 第二轮新嵌套滚动 fixture 受 app shell 的 containing block 影响，固定坐标没有命中测试元素，滚轮实际滚了主列表；增加真实几何和命中检查后修复测试，再运行完整入口通过。未修改产品预期或跳过场景。
- 新测试保留固定 fixture 字面量 ID；实际滚动位移、鼠标/键盘输入有效性、只展开当前组均独立断言。快速连续激活检查显式关闭意图不被错误反转；返回页面后调用旧 tracker 不能复活它。
- 首次 Windows 加入畸形 hash 后，测试等待条件自身 `decodeURIComponent` 抛错；改用预期编码 hash 比较后，完整 Web 和 Windows 契约复验通过。产品也使用编码比较，避免该边界异常。
- Windows 默认 `bash` 指向 WSL；聚合入口最初因 PATH/WSL 权限失败。显式使用已安装 Git Bash 并在其内部设置本次 PATH 后成功，没有修改系统配置。

### 远端状态

- 实现和本地证据完成，测量源码的规范化 LF SHA-256 已逐文件核对实现 commit `dbe51a38d9d88dba7ad5b3769dba238b2b03e19e`。证据提交只改 docs，无需重新构建相同生产源码。
- 本记录提交时最终 CI 尚未启动，不记为通过；推送及最终 SHA 的 run/result 维护在 [PR19](https://github.com/Develata/RSS-Reader/pull/19) 说明，避免为回填结果再改变受检 SHA。保持 draft、不 merge，不创建新 PR。

## 工作区与同步

- 实际工作目录一直是 `E:/gitclone/RSS-Reader`。先刷新 origin；main 为 `e8c6ee165761f2e89baa6612b96324310def187a`，PR19 为 `9815d4e8c04e124279cdabbc55de78c07a064371`，均与远端一致，无同期新提交。
- 核对 C 盘 `C:/Users/QQ/AppData/Local/Temp/rssr-directory-baseline-20261002` 干净、无关联活动进程后，仅在原 SHA detach，释放分支，再在 E 盘安全 switch。未删除 C 盘任何内容，也未新建 worktree。
- 原有 22 项 `.agents/` / `.specify/` 删除及 `.workbuddy/` 保留；后者 SHA-256 仍为 `8b10390d734483195439c4c5c413849ea67e0f845747eb049ca8c115aa69df87`。上轮 4 份文档纳入本次授权范围，原文档交接保留历史时态。
- 原生基线 exe 与历史记录一致：SHA-256 `e16c55bcb2c5bc1383828da8d907af064766f8eea7261744f952707a8c193265`。按任务范围复制到 E 盘隔离验收目录；未迁移或清理无关环境。

## 结果

实现、Web/Windows 交互与同宿主测量完成；原有 22 项删除和 `.workbuddy/` 不纳入任务提交。等待远端最终 SHA 验证，不构成合并或发布许可。

## 风险与后续事项

- DOM + 两帧指标只作为代理，不代表真实显示、掉帧或 layout 成本，也未计手动→跟随切换时延。Web A 来自 CI dx 0.7.9，B 为本机 dx 0.7.10；同宿主差异不能完全归因于代码。内存快照非总进程树、未强制 GC，不能证明下降。
- 主滚动归属采用最新可信输入与实际滚动目标关联，保持惯性连续性；平台实际惯性/触摸及辅助技术仍需分别验证。
- 同页目录导航保留手动状态；新分页、分组或查询上下文重建状态。导航目标受查询上下文约束，避免旧导航迟到。

## 给下一位 Agent 的备注

先读[目录设计](../design/entries-directory-follow.md)。本轮日志、截图、隔离数据库、构建及样本保存在忽略目录 `target/directory-follow/`；原 PR19 C 盘基线产物仍保留。不得提交原有 22 项删除或 `.workbuddy/`，不得把旧契约失败恢复成禁用活动组按钮。
