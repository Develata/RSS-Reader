# Rust 目录状态：交互与性能验证

日期：2026-10-03。对应 draft PR19 的首次实施记录，源码为 `dbe51a3` / `9d1a4bf`。现行产品规则见[目录设计](../design/entries-directory-follow.md)，首次工程状态见[交接](../handoffs/2026-10-03-directory-follow-rust.md)。本记录与[旧基线](./directory-regression-baseline.md)分开保存。

**校正说明：** 后续复核确认已读操作的来源未读数刷新会误重置手动目录状态；首次用例只检查活动位置，没有覆盖该状态回归。另外，旧性能工具的监听阶段、监听顺序及 rAF 条件轮询会造成计时偏差，Web 两版构建来源也不同。因此撤回原先“代理时延未见退化”和“一帧收益”的比较结论；下文原始数据仅作历史测量记录。修复与校正后的证据见[本轮验证](./directory-review-validation.md)与[本轮交接](../handoffs/2026-10-03-directory-review-correction.md)。

## 范围与结论

目录内任意折叠当前组、主列表同组滚动恢复跟随并清除其他组，已在 Web 和 Windows WebView2 专项验证。Rust 持有交互状态，两个目录消费者分别订阅；文章 presenter 输入未加入目录状态。保留折叠 DOM，没有 lazy/unmount。

旧工具记录过 Web 高亮/展开约 16.6–17.1ms、Windows 展开约 6.3–6.7ms 的代理时延差。这些数字混入测量方法偏差，即使正反顺序趋势一致，也不能据此归因于 Rust pipeline 或证明性能不退化。这里的“代理”是预期 DOM 条件加两次 rAF；它不代表物理显示时延、掉帧、长列表上限或 Android 实机表现。

内存快照不足以证明降低总内存；CSS 折叠也不是内存收益证据。

## 行为契约与反测

- 最终 Web 完整套件：242 pass、0 fail、0 console error，包含 90 项目录断言和既有阅读/图片/刷新等场景。
- Windows 可见 WebView2 目录专项：79 pass、0 fail、0 console error；未用 Chrome 结果替代原生窗口验收。
- 固定数据：6 来源、72 文章、每页 50，时间与来源两种分组；目标 ID 是 fixture 字面量，不从运行时当前组计算预期。
- 鼠标、Enter、Space 均要求当前组实际折叠；真实同组/跨组主列表 wheel 必须产生位移，再验证仅目标组展开。键盘主列表滚动先移出目录焦点。
- 验证真实目录内部位移、局部折叠不重定位、嵌套控件真实位移及冒泡隔离、程序化滚动不夺取手动状态、快速明确关闭意图、迟到滚动、实际标记更新、畸形 hash、同页/跨页跳转、96px 锚点边界、视口切换和退出/返回清理。
- Web 消费者宽度 360 / 720 / 721 / 1280；这是响应式模拟，不是 Android 原生或触摸设备验收。

同一新契约反测旧 Web 产物：48 个断言通过、3 个失败报告，其中聚合报告包含 **19 个场景失败**，二者不是相同计数单位。旧版在时间/来源的 Enter、Space 场景均无法实际折叠当前组；鼠标活动组被禁用而不可命中。其余失败还包含旧默认展开策略、缺少新状态标记和生命周期差异。这是新契约识别旧行为的证据，不是新增生产回归。

PR19 原始四个键盘失败 JSON、历史 CSV、交接保持原样；没有跳过旧失败对应的输入场景或仅为 CI 绿色放宽预期。新的四个 Rust 状态测试验证同组恢复/清空、被动观测、按呈现状态提交明确意图和陈旧消息拒绝，另有两个既有 presenter 目录测试。

中间失败没有隐藏：首次 Web 的焦点/不可滚动 fixture 前提、嵌套 fixture 实际命中位置、首次 Windows 的测试 hash 解码错误均已修正并完整复验。产物保留于 `target/directory-follow/`，具体见交接。

## 产物与可复核数据

- A 生产源码：`e8c6ee165761f2e89baa6612b96324310def187a`；初始 PR19 HEAD：`9815d4e8c04e124279cdabbc55de78c07a064371`。
- A Web Wasm SHA-256：`4c4cf1e694d6ed62347b99b611eab490fbe114ad75366704591f1114c9e6b4d0`，沿用该 SHA 的 CI release 产物，dx 0.7.9。
- B Web Wasm SHA-256：`d6ed464954778cb89b9d7881435b547e1ff2f161ffd17bf43ae6ec13ba145aa9`，本机 Windows release，dx 0.7.10 / Dioxus 0.7.9，`--debug-symbols false`。工具报告版本差异，构建成功。
- A Windows exe SHA-256：`e16c55bcb2c5bc1383828da8d907af064766f8eea7261744f952707a8c193265`。
- B Windows exe SHA-256：`749dd3510827d1bedd7adf5a0d0319afa8703c53c777a7b26ef72d71ef12aa11`。
- [完整元数据、源码指纹、逐轮统计、首个测量试次与内存快照](./baselines/2026-10-03-directory-follow-comparison.json)。`candidateCommit` 已绑定实现提交 `dbe51a38d9d88dba7ad5b3769dba238b2b03e19e`；工作区字节与规范化 LF 指纹均保留，后者已逐文件核对 Git 源码。后续证据提交只改 docs。
- [Web 600 个原始样本](./baselines/2026-10-03-directory-follow-web.csv)、[Windows 600 个原始样本](./baselines/2026-10-03-directory-follow-native.csv)。原始 DOM 与 proxy 时间均保留，未删异常值。

同一 Windows 11 10.0.26200 / i7-12700H / 约 16GiB 主机，Node 24.21.0。Web 使用隔离 Chrome 154.0.8037.93，headless、1280×800、DPR 1；Windows 使用可见 WebView2 154.0.4258.53，1280×900、DPR 1.5，采样开始确认 visible/focused。物理显示器报告 165Hz；headless 的 rAF 节拍独立，不能当作物理刷新率。

固定默认主题和同一数据量。每宿主按 **A1、A2、A3、B1、B2、A4** 串行启动：先 A/A，随后 A/B 和 B/A。每轮每路径 3 次预热、20 次保留样本；合并比较只取 A3+A4 与 B1+B2，各路径各版本 40 样本。无并行编译、测试或 GUI 自动化；普通 OS 后台活动未控制。每个原生窗口使用本任务独立 SQLite/profile，用后关闭。没有重建、删除或迁移原 C 盘基线。

测量沿用原 benchmark 的动作与计时方法：可信 scroll/click/keydown/keyup 到预期 DOM，再两次 rAF。scroll-highlight 的位置由脚本设置、事件由浏览器产生，**没有测量从手动模式恢复跟随的时延**。记录的 firstSample 是准备完成后的首个预热测量，不是冷启动、真正首次挂载或卸载后首次展开。

## 历史代理时延记录（比较结论已撤回）

全部单位 ms。差值统一为 B−A；A/A 列为两轮中位数的绝对差，属于当时的噪声观测，不能抵消监听顺序等系统偏差，不是统计置信区间或长期门槛。不得跨宿主比较表中绝对值。1200 个总样本只相当于比较中每路径每版本 40 个样本，不能靠总量为 p95 尾部结论背书。

### Web / Chrome

| 路径 | A/A 差 | A→B 差 | B→A 顺序 B−A 差 | A 合并中位数 | B 合并中位数 | B−A |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| scroll-highlight | 0.00 | -16.20 | -17.40 | 50.10 | 33.20 | -16.90 |
| expand-mouse | 1.20 | -16.55 | -17.10 | 62.55 | 45.50 | -17.05 |
| expand-Enter | 0.05 | -16.80 | -17.05 | 64.35 | 47.45 | -16.90 |
| expand-Space | 0.40 | -17.20 | -16.25 | 63.90 | 47.35 | -16.55 |
| navigate-align | 0.60 | -0.15 | -0.80 | 579.95 | 579.00 | -0.95 |

### Windows / WebView2

| 路径 | A/A 差 | A→B 差 | B→A 顺序 B−A 差 | A 合并中位数 | B 合并中位数 | B−A |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| scroll-highlight | 0.05 | -0.10 | -0.10 | 18.20 | 18.10 | -0.10 |
| expand-mouse | 0.10 | -6.85 | -6.55 | 27.50 | 20.80 | -6.70 |
| expand-Enter | 0.15 | -6.40 | -6.10 | 28.55 | 22.30 | -6.25 |
| expand-Space | 0.35 | -6.25 | -6.45 | 28.10 | 21.80 | -6.30 |
| navigate-align | 0.00 | -0.40 | -0.05 | 651.95 | 651.75 | -0.20 |

## 内存与未测量项

Web 各轮结束的 JS used heap：A 约 2.29–2.31MiB，B 约 2.04MiB；DOM counter 的 node：A 1677、B 1680。Wasm 线性内存容量：A 3,538,944B，B 3,473,408B，差一个 64KiB page。没有强制 GC，容量不是存活 Rust 对象大小，也未覆盖峰值、长时间使用或完整进程树。

原生结束 JS used heap：A 约 2.11–2.17MiB、B 2.12–2.15MiB，区间重叠。主 app private bytes 约 9.3–10.0MiB，仅包括 app 进程，不包括 WebView2 子进程；第一轮 A 的 DOM 快照还有未回收节点干扰。不能由这些快照声称总内存下降。

仍未测量：物理显示时延、掉帧与 layout 成本、手动→跟随切换时延、长列表/长时内存、真实触摸/触控板惯性及滚动条拖动的全平台交互、Android 原生 GUI、辅助技术。CI Android 构建即使通过也不能替代这些验收。后续若考虑卸载节点，必须另验首次展开、恢复跟随、焦点与滚动稳定性。

## 复验入口

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -j 1 -- -D warnings
cargo test --workspace --locked -j 1
dx build --platform web --package rssr-app --release --locked --debug-symbols false
node scripts/browser/rssr_small_viewport_assertions.mjs --cdp-base http://127.0.0.1:18124 --static-base http://127.0.0.1:8124 --artifact-dir target/directory-follow/web-final-verified
```

现有 Bash 聚合入口在 Git Bash 内执行 `export PATH="/usr/bin:/bin:$PATH"; bash scripts/run_release_ui_regression.sh --release --skip-build --no-serve --log-dir target/directory-follow/aggregate-exact-source`；该次只运行自动门禁，browser harness、服务 smoke 与固定 smoke 被该模式跳过。完整 Web UI另行执行，wasm browser、主题矩阵、Android 构建以最终 SHA 的 CI 为准，不能把聚合入口中 skipped 写成通过。

性能沿用同一 Node 入口，加 `--directory-perf <metadata.json>`；原生另加 `--native-target <本任务窗口的明确 target id>`。元数据支持 `batchCount: 1`，须核对实际产物 hash、隔离 fixture、窗口可见/聚焦、构建参数与浏览器版本。局部脚本、日志、截图和 profile 留在 `target/directory-follow/`，远端 CI 结果见 PR19。
