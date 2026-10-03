# Static Web 小视口 Smoke

这份 smoke 是静态 `rssr-app` Web 入口的确定性移动端 UI 门禁。它不读取开发机已有的 localStorage，而是用仓库内 fixture 分别覆盖长内容和短目录场景，并在浏览器内执行 computed-style、几何与可访问性断言。

## 脚本

- [`scripts/run_static_web_small_viewport_smoke.sh`](../../scripts/run_static_web_small_viewport_smoke.sh)

## 最短用法

```bash
bash scripts/run_static_web_small_viewport_smoke.sh
```

默认行为：

- 构建并启动带 SPA fallback 的静态 Web 服务；
- 启动隔离 profile 的 Chrome，通过 CDP 固定为 `360×800`、DPR 3、mobile/touch emulation；
- 依次载入 `mobile-ui-overflow`、`mobile-ui-short` 与 `home-reader` fixture；
- 在 `/entries`、`/feeds`、`/settings`、`/entries/2` 和 `1280×800` 桌面回归上执行断言；
- 任一断言、浏览器 console error 或错误 overlay 出现时返回非零。
- 在启动浏览器前检查实际构建的初始 HTML 标题恰好是 `RSS-Reader`，防止模板占位标题与 Dioxus `document::Title` 拼接。

## 常用参数

```bash
bash scripts/run_static_web_small_viewport_smoke.sh --skip-build
bash scripts/run_static_web_small_viewport_smoke.sh --viewport 430,932
bash scripts/run_static_web_small_viewport_smoke.sh --preset newsprint
bash scripts/run_static_web_small_viewport_smoke.sh --release
```

发布聚合入口 `bash scripts/run_release_ui_regression.sh --with-fixed-smokes --no-serve` 会自动调用此门禁，并继承聚合入口的 debug/release profile。

HTTP 端口由 `--port` 指定（默认 8091，允许 1..55535），CDP 使用该端口加 10000。脚本启动前拒绝已占用的任一端口，就绪检查和断言完成时都确认本次子进程仍在运行；不能借用开发机旧服务取得通过结果。HTTP 探测具有连接 / 请求超时，失败或中断时只清理本次启动的服务和浏览器，终止等待有上限。本地 GUI 验收继续串行执行；GitHub 矩阵使用独立 runner。

## 自动断言

- 视口精确为 `360×800`，根文档无横向溢出；
- 长来源在可多选的完整换行行中显示，极端无空格名称无水平裁切，选择区有独立纵向滚动；
- 11 个月目录在 0%、50%、99% 保留右缘渐隐，100% 才移除 mask；单月目录不显示渐隐；
- 超长 feed、entry、reader 标题不越界，移动按钮不碰撞；触控目标与键盘提示规则不回归；
- 主题入口只使用“应用”语义，不依赖按钮总数；
- console error 与应用错误 overlay 均为零；
- 展开导航时 R / Reader 返回可见，收起时仅保留展开箭头，所有顶栏图标有可访问名称、title 和 44px 点击区域；旧导航折叠偏好不再隐藏 R；
- 搜索开关、Esc、Enter 与订阅 / 设置导航；检查非 reduced-motion 下搜索展开动画的实际 computed declaration；输入法 composing 的 Esc 不收起搜索；只有首页再次点击 R 才发起刷新；
- CDP 暂停真实同源 RSS 请求，验证 R 与自动刷新合并，整轮完成后每个 fixture feed 恰好请求一次；连续 R / Feeds 刷新去重、页面卸载后整轮继续、Reader DOM 与滚动不受刷新影响、部分失败后可重试；
- Reader 中刷新进行、成功和失败反馈均不覆盖标题；保留 live region、完整 title 以及 R 的结果小标记（DOM 断言不等同于辅助技术实机验收）；
- 单份分页在页面中部仍位于视口，翻页后回到起点；真实 CDP touch 验证短拖动、非顶部不触发，以及顶部阈值和 R 共用刷新状态；
- 图片采用原渲染 data URI，原 sanitizer 仍剔除 inline handler；原生 modal、Esc / Enter / 关闭按钮 / 背景、焦点和滚动恢复；`2000×12000` 竖图与 `12000×2000` 横图限制在查看器视口内；
- Home、分页与图片使用实际 CDP tap 检查命中区域；图片打开时 history back 释放 body lock 并恢复目标页滚动，forward 后连续开关图片恢复正文滚动及焦点；
- `1280×800` 下真实鼠标拖选、Ctrl+C 原生复制、Ctrl+A 全选；桌面 rail 的名称、计数以及移动端来源目录不裁切。
- 正文链接下划线、长单行代码局部滚动；通过设置页实际保存字号 1.25，再打开 Reader，手机和桌面 computed font-size 均按比例变化；
- 新增订阅输入的可见键盘焦点、Enter 提交恰好添加一份订阅、刷新按钮不误提交地址。
- 暂停新增订阅的实际首刷请求，验证重复 submit 只产生一次刷新、disabled / `aria-busy` 反馈，以及期间输入的下一个地址在完成后保留；匹配请求时包含既有 cache-busting 参数。
- 小屏订阅页的两个辅助统计同排显示，地址输入仍位于视口内。
- 订阅页在操作前已有空的 polite / atomic 状态区域，导出结果更新同一个 DOM 节点且不抢焦点；静态说明不自动播报。
- Reader 的具名快捷键区域、关联说明、收藏 `aria-pressed` 与状态一致，四个装饰图标不进入可访问名称。
- 首页启动刷新后，订阅页“刷新全部”同步禁用、显示忙态和较低透明度；任务结束后恢复可用文案和 `aria-busy="false"`。

Android 实机长按选择手柄、系统返回、pinch zoom，以及 macOS Cmd+C/Cmd+A 不由 Chrome touch emulation 代替验收。未接实机时必须保持未验证。

同一浏览器断言模块还提供 Windows 原生 WebView2 的显式 target 连接模式，见[原生隔离与执行说明](./manual-regression.md#复用现有断言检查-windows-原生窗口)。原生模式不使用此处的浏览器 seed 或视口模拟，结果与 Web 分开记录。

## 结果记录

脚本会在 `target/static-web-small-viewport-smoke/<timestamp>/` 生成：

- `assertions.json`：每条断言、实测几何、console/error-overlay 汇总；
- 各场景 DOM dump；
- 各场景 PNG 截图；
- Chrome、静态服务与 runner 日志；
- `summary.md`。

## 基线与扩展验收

目录行为回归现由默认主题 CI 入口实际执行，包含独立的时间/来源分组 fixture、96px 边界、Enter/Space 潜在偏好、导航及生命周期；详见[目录契约与性能基线](./directory-regression-baseline.md)。`--directory-only` 可单独复现。基线活动组键盘 bug 的失败预期保持开启，因此相关 draft 的 default UI job 会失败，不能由其它主题通过推定整体通过。可选性能采样必须显式传 `--directory-perf`，不属于每次 CI 自动运行内容。

断言数量以本次 `assertions.json` 为准。2026-09-22 的初版及后续扩展保留在对应 handoff；2026-09-26 五主题复验见 [集成记录](../handoffs/2026-09-26-integration-revalidation.md)。历史通过不证明新构建通过。

导航收起、操作边界位置采集、来源计数、批量已读等近期能力还应按 [手工回归](./manual-regression.md) 和相关 handoff 补验；不要从脚本名称推定所有功能已自动覆盖。截图是视觉复核证据，结构断言不能替代色彩、信息层次和真实辅助技术体验检查。
