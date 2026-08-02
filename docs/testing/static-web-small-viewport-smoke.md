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
- 依次载入 `mobile-ui-overflow` 与 `mobile-ui-short` fixture；
- 在 `/entries`、`/feeds`、`/settings`、`/entries/2` 和 `1280×800` 桌面回归上执行断言；
- 任一断言、浏览器 console error 或错误 overlay 出现时返回非零。

## 常用参数

```bash
bash scripts/run_static_web_small_viewport_smoke.sh --skip-build
bash scripts/run_static_web_small_viewport_smoke.sh --viewport 430,932
bash scripts/run_static_web_small_viewport_smoke.sh --preset newsprint
bash scripts/run_static_web_small_viewport_smoke.sh --release
```

发布聚合入口 `bash scripts/run_release_ui_regression.sh --with-fixed-smokes --no-serve` 会自动调用此门禁，并继承聚合入口的 debug/release profile。

## 自动断言

- 视口精确为 `360×800`，根文档无横向溢出；
- 长来源 chip 保持单行、未撑高、发生省略，且 `title` 与 `aria-label` 都保存完整名称；
- 11 个月目录在 0%、50%、99% 保留右缘渐隐，100% 才移除 mask；单月目录不显示渐隐；
- 超长 feed、entry、reader 标题不越界，移动按钮不碰撞；触控目标与键盘提示规则不回归；
- 主题入口只使用“应用”语义，不依赖按钮总数；
- console error 与应用错误 overlay 均为零；
- `1280×800` 下桌面 rail、页面宽度和来源 chip 不回归。

## 结果记录

脚本会在 `target/static-web-small-viewport-smoke/<timestamp>/` 生成：

- `assertions.json`：每条断言、实测几何、console/error-overlay 汇总；
- 各场景 DOM dump；
- 各场景 PNG 截图；
- Chrome、静态服务与 runner 日志；
- `summary.md`。

## 当前基线

- 2026-08-02：`360×800` / DPR 3 的 30 项自动断言全部通过；长目录 mask 的 0%/50%/99%/100% 状态、短目录、长标题、来源 chip、主题文案、移动触控目标和桌面回归均已固化。
- 截图仍保留作视觉复核证据，但脚本通过不再依赖人工填写结果。
