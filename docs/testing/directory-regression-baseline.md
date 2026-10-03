# 目录行为回归与性能基线

本次只增加测试、fixture 和可选测量。生产基线为 `e8c6ee165761f2e89baa6612b96324310def187a`。
没有 JS → Rust controller、IPC 协议或迁移 candidate；不能由这里的结果宣称迁移性能通过。

## 实际执行入口

```bash
bash scripts/run_static_web_small_viewport_smoke.sh --release --skip-build
# 仅运行新的目录契约（同一 CDP/fixture/报告工具链）
bash scripts/run_static_web_small_viewport_smoke.sh --release --skip-build --directory-only
```

CI `web-ui (default)` 无需新增 job，直接从现有入口执行完整目录契约。其余四个主题继续执行原有 UI 断言；本次不增加主题 × 设备 × 数据量的笛卡尔积。
Rust 八行状态真值表由 `rssr-app` 测试执行。性能采样不在 CI 中自动运行，也不设百分比门槛。
现有 10k 内存 SQLite 性能探针、历史手工脚本和 Android 构建均不等于本次 GUI 验收。

## Fixture 与判定

`tests/fixtures/browser_state/directory_contract_*.json` 是固定输入：Alpha–Foxtrot 六个来源，2026 年 4–9 月，每月 1/2 日各六篇，共 72 篇，每页 50 篇，禁用归档。启动总会刷新，因此 fixture URL 被映射到既有 loopback 服务，固定返回 304；准备阶段等待刷新结束，避免网络请求或数据变化污染交互与采样。
四份 slice 通过既有 setup helper 在隔离 profile 内发布。每个 Web 场景重新 seed；Enter 和 Space 各用独立场景，只激活一次。活动组测试在**同一挂载页面内滚到另一组**后检查基础偏好，期间不离页或重置。

`directory_contract.mjs` 中的预期 ID 是 fixture 的字面量事实，没有调用生产选择器或按 DOM 顺序算 expected。
时间和来源模式均检查：首项、同组下一项、跨组前后、97px/95px 分列 96px 阈值两侧（定位误差 ≤0.75px），活动组自动展开与不可切换、非活动组手动偏好、鼠标局部折叠不回跳、真实 flag 更新的 DOM/VDOM 交错、同页与跨页/hash 目标对齐（≤2px）。
721px/720px（实际 CSS 断点两侧）、1280px 与 360px 切换时检查 CSS 显示的目录消费者及其高亮字重；移动横向对齐只许改变目录 scrollLeft，正文应保持明确请求的 scrollY（≤1px）。顶部目录本身可能已滚出屏幕，`visible` 在这里指有布局盒而非物理视口内可见。局部折叠前明确把 rail 滚离活动项；只允许因内容缩短产生的原生 scrollTop 上限夹取，不能回跳到正文活动组。

离开/返回通过正常路由执行；使用 CDP 只读列举 tracker 的 scroll/resize listener，检查删除与恰好一次重装。另直接调用现有 lifecycle API 检查有待执行 rAF 时 cleanup 不复活 tracker。没有覆写全局事件、rAF 或生产计数器。
新的等待以连续三帧满足条件加有界超时实现；沿用既有准备函数中的等待。阅读位置恢复通过已有真实 wheel 输入取消，不用任意加长 sleep 掩盖恢复竞争。

## 当前阻塞：活动组键盘会修改潜在偏好

在基线 Web release 上，以下四个场景准确失败：时间/来源 × Enter/Space。

1. 打开 fixture 首页，滚到第一组，确认 `data-active=true`、`data-can-toggle=false`、`aria-disabled=true`、`data-open-base=true`。
2. 聚焦该组按钮，只按一次 Enter 或 Space。
3. 组仍展开，但 `data-open-base` 已变为 `false`。
4. **不离开页面**，滚到下一组：原组变为折叠。

`controls.rs` 的两个 onclick 无条件翻转偏好；CSS 仅用 `pointer-events:none` 拦鼠标，`aria-disabled` 不禁止键盘默认激活。
最小修复方向是让真实按钮激活资格与现有 viewport active 状态一致（例如同步原生 disabled 状态），继续保留 Rust 的基础偏好。仅用 Rust 初始 `month.is_active`/`source.is_active` 作 guard 可能与 JS 后续滚动状态不一致，需要一起核对。
本 PR **不实施生产修复，不 skip，不放宽预期**。因此 default Web UI CI 预期失败，draft 不能合并；生产修复交回父任务决定。

`assertions.json` 保存逐条状态、几何和失败详情；每个失败场景独立截图/DOM，最后聚合失败返回非零。CI 的现有 always-upload 保存这些证据。清理与截图失败不覆盖原始断言失败。
本机最终完整 Web 入口为 203 条通过、4 个行为失败（另 1 条聚合失败），真实 Windows 原生为 40 条通过、相同 4 个行为失败（另 1 条聚合失败）；两者 console error 都为 0。[已提交的失败状态摘录](./baselines/2026-10-03-directory-known-failures.json) 包含每次单键前后及同页滚走后的状态、原始报告位置和 hash，未将失败改成预期通过。

## 可选性能测量

对已有 CDP 实例执行；metadata JSON 必须明确提供完整 baseline SHA、`build: release`、平台、产物 SHA-256 和刷新率（未知填 null）：

```bash
bun scripts/browser/rssr_small_viewport_assertions.mjs \
  --cdp-base http://127.0.0.1:18114 --static-base http://127.0.0.1:8114 \
  --directory-perf /path/to/metadata.json --artifact-dir target/directory-perf

# 先复制本次 release app/CLI 到新的可写临时目录；由该 CLI 创建空库
./isolated/rssr-cli.exe list-feeds
python scripts/seed_directory_native.py ./isolated/RSS-Reader --fixture-base http://127.0.0.1:8114
# 仅给隔离应用进程设置 WEBVIEW2_USER_DATA_FOLDER 和 CDP 调试端口
bun scripts/browser/rssr_small_viewport_assertions.mjs \
  --cdp-base http://127.0.0.1:18115 --native-target EXPLICIT_TARGET_ID \
  --directory-perf /path/to/native-metadata.json --artifact-dir target/native-directory-perf
```

原生模式须使用上述相同 fixture、默认主题、50 篇/页；不注入 localStorage 或模拟视口，也不关闭宿主窗口。seeder 拒绝非空 feeds/entries/content，先由同版 CLI 完成正式迁移，不自行重建 schema。

仅三条路径：滚动→目录高亮；非活动组展开（鼠标/Enter/Space 分列）；目录点击→目标/hash 并对齐。
每批每项预热三次，保留 20 次样本，重复三批。起点为浏览器 trusted scroll/click/key 事件；滚动位置由脚本请求，起点不包含请求到 scroll 事件的耗时。终点是目标 DOM 条件成立后两个 rAF 回调；DOM 轮询按帧量化。
此指标排除 CDP 传输、操作前定位、事件前输入分派和实际显示，不等于光子响应时间；也不能推断 layout 耗时或掉帧。
计时前的 80px 定位允许 2px 偏差（Windows DPR 1.5 的首轮重复导航准备曾实测 79.0625px）；它仍远离 96px 阈值，且不在计时区间。行为回归的 97/95px 定位仍要求 ≤0.75px，未放宽键盘或导航失败预期。原生采样要求真实窗口 `visibilityState=visible`；隐藏窗口超时不能作为性能样本。
原始 `domMs`、`proxyMs` 和环境写入 `directory-performance.json`，只汇总中位数/最小/最大及三批中位数跨度，不伪造 p95。
提交的 CSV hash 对应 UTF-8 / LF 的 Git blob；Windows checkout 若转换为 CRLF，校验前需还原 LF，或直接读取 `git show HEAD:docs/testing/baselines/<file>.csv` 的字节。

## 后续 candidate 比较方法

先在同一机器/宿主/发布模式/数据/视口/DPR/刷新率上重跑 A/A，检查当日噪声是否与本次三个批次相容。再串行交替 A/B、B/A，保留逐次样本和失败，不把 GUI 测试与编译并行。
候选比较的初始分辨预算采用对应路径实测的 **max(三批中位数跨度, 全部样本 IQR)**（毫秒）；IQR 使用 inclusive 线性插值。这个描述性预算同时保留批间漂移与批内抖动，避免三个四舍五入后相同的中位数被误解为零噪声。它不是可接受产品退化的政策。若差异仍在该范围内，结论应为无法区分；若超出，继续增加成对批次并报告中位数差的不确定性，再由产品要求确定可接受预算。
不能把稳定的两帧等待当性能改善，也不能跨 Chrome headless / Windows WebView2 比百分比。Android 真实 WebView、屏幕延迟、掉帧、layout cost 和迁移 candidate 全部需独立补验。

## 2026-10-03 正式基线

共同机器：Windows 11 `10.0.26200`、i7-12700H、约 16 GiB RAM、RTX 3060 Laptop；物理显示器报告 `2560×1600 / 165Hz`。默认主题，相同 72 篇 fixture，无并行编译/测试参与计时。普通宿主后台进程未受控。

- Web：Chrome `154.0.8037.93`，headless、disable-gpu，`1280×800 / DPR 1`；采用基线 SHA 的成功 CI release artifact，来源/wasm hash 见 [Web 环境与汇总](./baselines/2026-10-03-directory-windows-chrome.json)，[300 个原始样本](./baselines/2026-10-03-directory-windows-chrome.csv)。headless rAF 不是物理屏幕刷新率。
- Native：WebView2 `154.0.4258.53`，实际可见 Windows Dioxus 窗口 `1280×900 / DPR 1.5`，无视口模拟；本机 MSVC release 构建及 exe hash 见 [原生环境与汇总](./baselines/2026-10-03-directory-windows-webview2.json)，[300 个原始样本](./baselines/2026-10-03-directory-windows-webview2.csv)。初次附着隐藏窗口的失败和后续准备定位诊断不纳入正式样本。

| 路径 | Web 三批中位数 ms | Native 三批中位数 ms | Web / Native 试测分辨预算 ms |
| --- | --- | --- | --- |
| 滚动→高亮 | 53.00 / 50.80 / 50.60 | 18.10 / 18.10 / 18.10 | 3.70 / 0.20 |
| 鼠标展开 | 63.30 / 63.25 / 62.80 | 27.80 / 28.15 / 27.70 | 1.13 / 0.70 |
| Enter 展开 | 64.65 / 64.75 / 65.00 | 29.00 / 28.70 / 28.80 | 1.13 / 0.60 |
| Space 展开 | 63.70 / 64.30 / 64.40 | 28.10 / 28.50 / 27.90 | 1.33 / 0.65 |
| 导航→目标对齐 | 582.65 / 581.20 / 580.95 | 652.30 / 652.35 / 652.15 | 11.55 / 0.50 |

上述展开均针对可操作的非活动组；已知活动组键盘 bug 仍存在。导航包括实际平滑滚动，不能从总时间反推 controller 或 layout 成本。两个宿主视口、DPR、刷新节奏不同，表格只列各自基线，不作跨宿主性能排名。
