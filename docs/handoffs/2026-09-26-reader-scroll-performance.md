# 阅读页上下滚动性能诊断

- 日期：2026-09-26
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：3b411b5
- 相关 commit：本记录所在的文档提交（父提交 3b411b5）
- 相关 tag / release：N/A
- 状态：`draft`

## 工作摘要

用户在上一轮原生界面验收中观察到上下翻动不流畅。本次定位可优化路径，未修改产品行为；以下是诊断采样，不是优化完成或跨平台流畅度验收。

## 影响范围

- 模块：`crates/rssr-app/src/ui/reading_position.js`、阅读页呈现。
- 平台：Chromium Web、Linux WebKitGTK / WSLg；Android 未测。
- 变更：仅本交接文档；实验脚本、日志和采样位于忽略目录 `target/reader-performance/`。
- 公开签名、trait、存储、data-* 与主题接口均未变化。

## 关键发现

### 位置采集

`onScroll` 和滚轮等 `onInput` 每次都会查询正文块，从开头逐块读取布局，发送完整位置事实。长文深处的工作量随前置正文块数增长。Rust 位置状态本身不是每次滚动驱动页面重新渲染的响应式状态。

Chromium 151 / Playwright，1280×800，CDP 4 倍 CPU 降速；替换渲染正文为 200 / 2000 段，滚到 80% 深度，90 个 rAF 中交替上下移动。通过 CDP 临时移除位置监听做对照，随后恢复；不是产品修改。

| 段落 | 位置监听 | 布局读取次数 | 帧间隔 p95 | 最大帧间隔 |
| --- | --- | --- | --- | --- |
| 200 | 开 | 14768 | 20.9ms | 59.7ms |
| 200 | 关 | 0 | 17.5ms | 17.9ms |
| 2000 | 开 | 144368 | 23.9ms | 93.3ms |
| 2000 | 关 | 0 | 17.5ms | 21.2ms |

这是单轮合成长文诊断，证明重复测量成本，不能直接推成用户设备的 FPS 或最终优化收益。

### 原生呈现

复用上轮 Task 2 验收二进制及隔离数据库（80 段测试正文），不是当前整合 HEAD 新二进制的运行验收。WebKit Inspector 在 1280×900 下执行 180 个 rAF 的交替 scrollBy：

- 未强制软件渲染：中位数 48ms、p95 64ms；静止 rAF 中位数 16ms、p95 17ms。
- 临时禁用 backdrop-filter：中位数 53ms、p95 74ms，未观察到改善。
- 强制软件渲染并禁用合成：中位数 23ms、p95 30ms。

这些样本运行顺序不同，部分期间存在后台编译，且没有验证实际 GPU renderer；不能据此归因到硬件加速或背景模糊。上轮坐标验收还使用 instant scrollTo，不是在测动画。原生输入不是物理滚轮，rAF 间隔也不等同于最终屏幕呈现帧率。

## 建议的实现顺序

1. 优先合并滚动位置测量，复用正文节点集合，减少重复扫描、文本读取和桥接消息；必须保留导航前即时捕获、用户输入取消恢复与图片重排校正。
2. 若长文仍慢，再考虑可失效的锚点索引；不能假定嵌套 li / blockquote 的底边坐标单调，不宜直接对当前节点数组二分。
3. 在没有后台构建的相同条件下重复原生、Web 实际滚轮和触摸测试，再决定是否调整视觉效果；当前不建议全局添加平滑滚动或随意添加 will-change。

## 验证与验收

- `bun target/reader-performance/profile.cjs`：退出 0；真实 Chromium 受控实验，原始结果见 `baseline.json`。
- `bun target/integration-validation/native-eval.cjs <expression>`：退出 0；原生 Inspector 实验见 `native-default.json`、`native-default-idle.json`、`native-no-blur.json`、`native-software.json`。
- `CARGO_BUILD_JOBS=2 cargo build -p rssr-app`：初次及直接重试退出 101，旧依赖缓存缺少当前公开符号。fingerprint 的 dep-info 明确引用 `target/integration-validation/baseline-src/`。单独重新编译 domain/application 未刷新 app 使用的全部构建变体。
- 将两个引用基线源码的 fingerprint 目录移至 `target/reader-performance/stale-fingerprints/` 保留后重新构建：退出 0；没有删除工作区或修改产品源码。日志为 `native-rebuild-restored.log`。
- `git diff --check`：退出 0。
- 未运行：本轮 workspace test/clippy、wasm/Android 编译及完整 UI 回归，因为没有产品代码变更；不能以先前通过代替本轮性能验收。
- 未运行：Android 实机、物理滚轮、360×800 性能采样及多轮统计，尚不能宣称解决卡顿。

## 结果与风险

完成诊断并提出优化顺序，尚未实现性能修改。重复布局读取是源码与对照实验支持的优化点；用户所见卡顿的主要成因仍需原生受控复验。本轮启动的测试服务器及原生窗口已停止。保留所有任务外 worktree 内容，不 push / tag / release。

## 给下一位 Agent 的备注

入口为 `ui/reading_position.js` 和 `ui/reading_position.rs`。优化后必须重跑 Task 2 返回位置、输入取消恢复、延迟图片布局以及列表分页恢复回归；不要以禁用位置记忆作为修复。跨源码副本共享 target 后需留意错误复用的 dep-info。
