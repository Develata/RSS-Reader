# Reader 读取恢复提示修复与故障回归

- 日期：2026-10-31
- 作者 / Agent：Codex
- 分支：`codex/main-regression-20261031`，复用 E 盘接续工作树
- 开工 HEAD：`1acef61a94a007f9c7a8fc9cd934778200723642`
- main 基线：`60b4c6f175fc3a0cc166818307dd213c9c67c852`，开工时与真实远端一致
- 相关 commit：实现、回归与候选交接为 `6a39570e`，父提交 `1acef61a`；后续 PR #57 合入 `df531fcc`，主线核验单列记录
- 相关 tag / release：无
- 状态：`validated`

## 工作摘要与审查结论

继续按用户要求查找并修复实际缺陷。本轮在真实正式 Web / SQLite 上复现 **P2：Reader 只读重读失败后，后续读取已成功应用新标记，旧错误提示却一直保留**。故障注入先返回一次 503，再恢复正常请求；通过服务器 API 改变真实已读标记，界面显示新值证明读取成功，原错误仍在。新增 Rust 集成测试和永久浏览器回归均在修复前失败。

修复后，成功读取只清除当前由只读重读产生的失败或缺失提示；当前显示的写入失败、结果未知和其他操作提示继续保留。额外检查写入拒绝、已提交但响应丢失、迟到读取等场景，所测范围内没有再发现产品缺陷。这是本轮代码自审和实际执行结果，不是第三方独立 review，也不代表所有平台与输入组合均无缺陷。

## 影响范围

- `rssr-app` 的 Reader intent、纯状态/reducer、runtime 回执和相关归属/session 测试；正式 Web、Windows、Android 共用规则。
- 新增 `scripts/browser/reader_recovery_scenarios.cjs`，接入现有正式 Web Reader 验收。
- 七 crate、Backend、唯一资料库、正文选择与渲染器不变；没有新增公共接口、请求、写入重试或全局状态。
- 沿用 `E:/gitclone/RSS-Reader-worktrees/native-back-policy`。主 checkout 的 HEAD、分支、索引、状态及 41 个改动/缺失路径均保持不变。

## 关键变更

### 读取反馈有明确来源

此前 `RereadEntry` 失败发出通用 `UiIntent::Failed`，与写入反馈共用状态；成功 `ApplyReread` 只更新标记与邻居，无法判断哪条提示已经过期。

现在 runtime 发出带文章身份的 `RereadFailed`，保留 `failure()` 的登录/版本恢复传递。页面现有状态增加一个私有来源标记，表示当前提示是否来自重读；没有复制业务状态或创建提示队列。有效的 `ApplyReread` / `ApplyLoadedContent` 清除这类提示，`SetStatus` / 通用 `Failed` 重置来源，避免后续读取误清当前写入结果。缺失文章提示也随成功读取退役。

访问代次与重读代次仍由现有 `ReaderResultScope` 约束。旧读取回执不能清除新访问的反馈，已读/收藏写入仍使更早的读取失效；成功重读不替换正文 `Arc` 或挂载节点。仍沿用当前提示的覆盖顺序，不承诺保存被后来提示覆盖的全部历史结果。

### 回归要求读取真正落地

永久浏览器用例使用隔离服务器资料库，通过真实 API 更改标记，再触发既有 `online` 路径，等待界面显示新值后断言错误提示为空、正文节点未替换。最后执行显式写入，验证门禁已释放并恢复夹具标记；不直接修改产品页面状态。

每种宽度恰好产生一次预期 503 控制台错误，脚本精确核对后单独记录。没有忽略其他页面/控制台异常，也没有以“请求发出”代替应用回执完成。

## 验证与验收

原始日志、失败截图、候选包、源码/产物清单、启动与故障探针保存在 `target/reader-recovery-review-20261031/`；[精简回执](../testing/evidence/2026-10-31-reader-reread-recovery/receipt.json)记录报告散列、463 个产品输入的清单身份与测试范围。

| 命令 / 场景 | 本轮结果 |
| --- | --- |
| `cargo test -p rssr-app --locked a_recovered_reread_clears_its_obsolete_failure`，修复前 | 预期失败：成功回读后旧状态未清除；`rust-before.log` |
| 新浏览器回归运行旧 Web 包 | 预期失败；`baseline-status-02/` 保存新标记已落地但旧错误仍在的事实，`regression-before/` 验证永久回归能捕获该问题 |
| `cargo test -p rssr-app --locked` | 445 项通过、0 失败、1 项既有性能测量忽略；403 单元 + 37 Reader 规则 + 2 主题 + 3 样式 |
| `python scripts/check_reader_rule_boundary.py` | 37/37，实际本机依赖图无 Dioxus/web-sys/Wry/Tao；与 app 规则测试重叠，不重复相加 |
| `cargo clippy -p rssr-app --all-targets --locked -- -D warnings` | 通过 |
| `cargo build -p rssr-app -p rssr-web --locked` | 通过；复制产物后用于隔离验收，避免与 Cargo 后续构建争用 |
| `dx build --platform web --package rssr-app --release --locked --debug-symbols false` | 匹配项目的 dx 0.7.9；最终优化构建成功，详细日志确认执行 wasm-opt 且无错误 |
| `node scripts/browser/rssr_reader_host_acceptance.cjs` | 最终优化 Web 1280 / 390，共 8/8；页面异常 0，非预期控制台异常 0，故意注入 503 共 2 次 |
| `node target/reader-recovery-review-20261031/probe.cjs` | 最终优化 Web 7/7；加载/重读恢复、两种标记写入拒绝与显式重试、两种写入已提交但响应丢失、旧读取不能回退新写入；页面异常 0，4 次预期 503、2 次预期连接重置 |
| `run-windows.ps1 -RunName windows-candidate-01` | 新构建 EXE、实际 Windows WebView2 `156.0.4314.50`，1280 / 390 共 8/8；返回、位置、图片优先关闭、快捷键与宿主卸载，页面/控制台异常 0 |
| `cargo check -p rssr-app --target aarch64-linux-android --locked` | NDK 28.2.13676358 / API 24 编译检查通过；保留既有 `LOCAL_DATA_DIR_NAME` 未使用警告。本轮没有 Android 实际宿主验收 |
| `cargo fmt --all --check`、两个改动脚本的 `node --check`、`git diff --check` | 通过 |
| `python target/reader-recovery-review-20261031/collect-receipt.py`、`check_docs.py` | 核对源码、产物、结果计数、主 checkout 保全及文档链接/JSON |

候选 build ID 为 `reader-recovery-1acef61a-dad9c82fa837`，产品清单 SHA256 为 `dad9c82fa8377beb1f05c622abdb1037b094357dc8b391af7400b8cd7d89de82`，绑定开工 HEAD 加本批产品补丁。Web 使用 Chrome `155.0.8059.40` 与独立服务器库；Windows 使用 100 篇合成文章、6 个 loopback 来源和独立 WebView2 profile，已退出本轮拥有的应用进程。旧 Web 包仅用于修复前复现：其产品输入与 main 基线等价，不冒充本轮新构建。

### 测试工具问题与处理

- 初次探针错误地期望加载失败页存在禁用的标记按钮；实际 UI 不渲染这些按钮。修正测试断言后继续，未改产品行为。
- 一次探针误用 Node fetch 的 `response.ok()`，改为其布尔属性 `response.ok`；失败日志保留。
- 初步探针只检查读取返回，未检查旧提示消失。补上“真实服务器标记改变并在 UI 落地”的证据及提示断言后，复现本批产品问题；未把此前通过结果当作缺陷不存在的证明。
- 初次临时 Web 构建遗漏仓库现有的 `--debug-symbols false`，触发已知 Windows wasm-opt `0xc0000409`。dx 虽然退出 0 并产生包，不能记为优化成功。按[已有构建说明](./archive/2026-10/2026-10-27-wasm-opt-release-build.md)补齐参数，重新构建并复制到 `candidate-optimized/public/`，最终 8+7 场景均在此产物重跑通过；未修改工具安装、workflow 或产品构建规则。

## 当前状态、风险与后续

- 本次实现交付时仅完成本地提交 `6a39570e`。后续按用户授权通过 PR #57 合入 main `df531fcc`；PR 与新 main 的检查单列于[合并交接](./2026-10-31-reader-recovery-merge.md)，不将下列本地验证改写为合并后重新验收。
- 本轮 Windows 是实际宿主一般回归，错误注入发生在正式 Web 与 Rust 测试；没有声称 Windows 原生后端也执行了同一网络故障注入。
- Android 本轮仅编译；系统返回、设备交互、macOS/Linux GUI、物理 IME、辅助技术、正式签名升级和真实资料库迁移未验收。
- 本批只改 Reader，本轮未重跑整个 workspace、远端 CI、Docker 或 Pages；上一轮 1149 项测试及主线 CI 属于其原提交，不能替代这次候选验收。

## 给下一位 Agent 的备注

先看本记录的复现与回归，再看 [Reader 渲染边界](../design/reader-rendering-boundary.md)。后续原生界面继续复用纯状态和归属规则；不要将通用写入结果也改成“读取成功就清除”，更不要通过重载正文来消除提示。证据及既有工作树全部保留。
