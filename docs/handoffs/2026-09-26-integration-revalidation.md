# 任务 2–5：开发依赖补齐、主线集成与最终复验

- 日期：2026-09-26
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：fc717c0（产品集成完成时；随后仅提交交接记录收尾）
- 相关 commit：4d0532e、a1aae97、3930289、fc717c0
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

补齐此前阻塞的 Linux / Android 工具链，修复全量测试发现的颜色 token 问题；按任务 2→3→4→5 顺序集成、复验、各做一次本地提交。最后在同一 Web bundle 上交叉回归全部四项能力。没有 push、tag 或 release。

## 影响范围

- 模块：app 页面/会话/反馈、application 订阅与刷新用例、domain 最小契约扩展、SQLite 与 browser adapters、CLI、Web proxy 和文档。
- 平台：Linux 原生、Web、Android ARM64 编译；Android 本轮不做运行验收。
- 额外影响：用户授权的开发工具安装；未修改全局 shell 配置、数据库 schema 或依赖版本。

## 关键变更

### 开发依赖

- 用户安装的 GTK 3.24.49、WebKitGTK 2.52.6、OpenJDK 21.0.12.1 已实际确认，pkg-config / java 命令退出 0。
- sdkmanager --licenses 和 SDK 安装均退出 0。安装 platform-tools、platforms;android-34、build-tools;34.0.0、ndk;27.3.13750724。
- SDK 位于 ~/Android/Sdk；sdkmanager 22.0，NDK ARM64 API 34 Clang 18.0.4 可执行。工具虽输出 sdkmanager 弃用提示，安装成功。
- 进程环境脚本保留于忽略目录 `target/integration-validation/android-env.sh`，可 source 后运行 Android 检查；未写入 ~/.bashrc。

### 按职责提交与逐文件记录

| 任务 | 本地提交 | 逐文件说明与完整公开签名 |
|---|---|---|
| 2 位置恢复 | 4d0532e | [任务 2](./2026-09-25-task2-reading-position.md) |
| 3 筛选批量已读 | a1aae97 | [任务 3](./2026-09-25-task3-filtered-mark-read.md) |
| 4 订阅自动发现 | 3930289 | [任务 4](./2026-09-25-task4-subscription-discovery.md) |
| 5 刷新真实新增数 | fc717c0 | [任务 5](./2026-09-25-task5-refresh-inserted-count.md) |

- 任务 2：高亮从硬编码颜色回退改为 `var(--accent-soft)`，修复现有主题契约测试失败。
- 任务 3：三方合并保留分页恢复、搜索首次挂载规则、批量确认与高亮；未覆盖前一任务。
- 任务 4：浏览器 harness 同时保留批量与发现案例；为任务 3 的 NewFeedSubscription 构造补 site_url=None。
- 任务 5：保留候选选择和反馈清除 revision；首次入库沿用统一 apply_source_output 并取得实际 inserted_count。文档原有“成功约1秒”同步修正，避免与新增章节矛盾。
- 本记录与四份任务 handoff 的收尾只更新验证状态、提交号与文件清单，不改变产品行为。

### 契约摘要

- 任务 2：新增 data-return-highlight；data-position-* 为内部测量，不新增持久化/仓储/CLI 契约。
- 任务 3：MarkReadPreview / MarkReadOutcome；EntryIndexRepository、EntriesListService、EntriesPort 的 preview_mark_read / mark_read_if_unchanged；CLI mark-read 及三项批量 data-action。
- 任务 4：SubscriptionProbePort、PreparedSubscription / PrepareSubscriptionOutcome；SubscriptionWorkflow::new 增加 probe，新增 prepare_subscription / add_prepared_subscription；NewFeedSubscription.site_url；host/UI 最小透传 fallback_site_url 与候选结果；proxy 增加 x-rssr-final-url。
- 任务 5：RefreshStorePort::commit 返回 RefreshCommitOutcome；Updated、summary 和 host outcome 增加真实 inserted_count；新增 upsert_entries_with_outcome，旧 upsert 方法签名保留。
- 不变：数据库 schema、原有 CLI add-feed / refresh 参数、任务 1 reader 快照/构造契约、RefreshFlight 合并与浏览器批次收尾规则。平台判断仍在 adapter / host 能力边界。

## 验证与验收

全部真实日志位于 `target/integration-validation/`。下表为各任务合并后的最终退出码。

| 命令 | 任务2 | 任务3 | 任务4 | 任务5 |
|---|---:|---:|---:|---:|
| cargo fmt --all --check | 0 | 0 | 0 | 0 |
| cargo clippy --workspace --all-targets -- -D warnings | 0 | 0 | 0 | 0 |
| cargo test --workspace | 0 | 0 | 0 | 0 |
| cargo check -p rssr-app --target wasm32-unknown-unknown | 0 | 0 | 0 | 0 |
| cargo clippy -p rssr-app --target wasm32-unknown-unknown -- -D warnings | 0 | 0 | 0 | 0 |
| cargo check -p rssr-app --target aarch64-linux-android | 0 | 0 | 0 | 0 |
| git diff --check | 0 | 0 | 0 | 0 |

- workspace 测试分别为 297 / 300 / 302 / 303 passed，各有 2 ignored。任务 2 初次退出 101：现有主题契约拦截 CSS 硬编码颜色；修复后重新完整运行退出 0。
- 最终 `bash scripts/run_wasm_contract_harness.sh wasm_refresh_contract_harness`：0，20 passed。
- 最终 `bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness`：0，6 passed。
- 最终 `bash scripts/run_wasm_contract_harness.sh wasm_config_exchange_contract_harness`：0，3 passed。
- harness 使用既有 ChromeDriver 151 / wasm-bindgen 0.2.126，CHROMEDRIVER_REMOTE 指向本轮 localhost 9518，子进程取消代理；完整环境脚本在忽略目录。
- 任务 3 单独 test_entry_state_and_search / test_bulk_read --nocapture：均 0。5 万条本次预览 182ms、应用 598ms，仅代表本机本次。
- 原生构建与基线源码归档构建最终退出 0。基线首个外层进程退出 143，未当作通过；重跑取到 0 后才复制验收产物。
- 各阶段 dx Web build 均 0。保留既有 dx 0.7.10 / Dioxus 0.7.9 不匹配提示，未自行升级工具或依赖。

### 实际界面与 CLI

- Linux / WSLg、WebKitGTK 2.52.6、1280×900：隔离数据库和原生可执行文件。通过该原生实例的 WebKit Inspector 执行 DOM click / scrollTo，X11 截图核对。基线列表 2500→157、正文重开 3200→0；任务 2 列表 2500→2500、标题 top 171→171、正文 3200→3200。两套脚本退出 0。二进制 SHA-256 留在 native-binaries.sha256。
- 原生首次截图尚未加载为黑屏，后续正常显示；不能用首次截图判定产品加载失败。X11 来源点击有效，但合成滚轮 / PageDown 未观察到滚动，上述坐标验收明确通过 Inspector，不宣称物理滚轮验证通过。
- Web：真实 Chromium 151 / Playwright（bun），360×800 与 1280×800。最终同一 bundle 串行运行 task5、task4、task3 browser.cjs、task4 direct-browser.cjs、task2 positions.cjs，全部退出 0。
- 覆盖位置/分页恢复、目标消失回退、主动输入取消、高亮期限；批量 60 条跨三页、59 条重确认、一次写入、失败回滚及零结果；单/多 feed 发现、四路径上限、去重不抓取、来源地址回退、候选换行；新增 1/0/2、正文更新0、部分/全失败、成功3秒/错误6秒、自动刷新静默、Reader 只显示图标。
- 任务 2 最终延迟图片专项退出 0，两视口真实延迟加载下锚点与位置恢复正确；后续位置逻辑未再修改。
- CLI 发现：真实 HTTP 8101，skip-refresh 退出0、多候选预期退出1、直接 feed 添加退出0；SQLite 断言0，skip订阅无文章，另一订阅1篇。
- CLI 新增计数：真实 HTTP 8102，四次刷新均0，数量1/0/2/0；SQLite 断言0，最终3篇。

## 结果

已验证：任务 2–5 按本轮确认范围完成集成、复验和本地提交。构建通过与实际界面验收分别记录，没有把 Android 编译当作 Android 运行。

## 风险与后续事项

- 未运行：Android 模拟器/实机、Windows/macOS 原生 GUI；本轮 Android 明确仅编译检查。
- 未验证：原生物理滚轮 / PageDown 的实际设备输入；Inspector 对照只证明记录、路由返回与恢复逻辑。
- 沿用风险：SQLite 索引与正文分库已有部分提交边界；若正文写入失败，失败订阅不计入成功新增数。未把该边界扩大为跨库事务迁移。
- 多个猜测候选在用户选定后重新验证，不能保证远端在两次请求间不变；请求大小和超时有界。
- 旧任务 3/4/5 worktree 仍保留原 dirty / untracked 副本及 target 符号链接；不是待合并的新成果。不要删除或重复应用。

## 给下一位 Agent 的备注

- 任务 2–5 最终实现已在 main；原独立 worktree 是保留副本。
- 本轮启动的两个原生测试实例、SPA 8099、ChromeDriver 9518、HTTP fixtures 8101/8102 已停止。SDK 安装保留，临时日志/截图/测试库保留在 target。
- .handoff/ 的 git/info/exclude 规则保持原状。未 push、tag、release；交接记录收尾提交后再次确认主工作树状态。
- 未运行的额外检查均已说明原因；文档收尾不重复运行已通过且源码未变的产品测试，只检查最终差异。
