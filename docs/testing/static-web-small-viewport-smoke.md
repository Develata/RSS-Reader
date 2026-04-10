# Static Web 小视口 Smoke

这份说明服务于静态 `rssr-app` Web 入口下，小视口关键路径回归。

它解决的是：

- 把响应式断点回归固定成可重复入口
- 直接覆盖 `/entries`、`/feeds`、`/settings`、`/entries/2`
- 避免每次手工改浏览器窗口尺寸再重测

## 脚本

- [run_static_web_small_viewport_smoke.sh](/home/develata/gitclone/RSS-Reader/scripts/run_static_web_small_viewport_smoke.sh)

## 最短用法

```bash
bash scripts/run_static_web_small_viewport_smoke.sh
```

默认行为：

- 启动带 SPA fallback 的静态 Web 服务
- 用 `reader-demo` seed 保证关键路径都有真实内容
- 用 `390x844` 视口生成：
  - `/entries`
  - `/feeds`
  - `/settings`
  - `/entries/2`
  的 DOM dump 与截图

## 常用参数

```bash
bash scripts/run_static_web_small_viewport_smoke.sh --skip-build
bash scripts/run_static_web_small_viewport_smoke.sh --viewport 430,932
bash scripts/run_static_web_small_viewport_smoke.sh --preset newsprint
```

## 验收重点

- 四条路径都能进入真实页面，不回退到门禁壳
- 关键布局语义仍在：
  - `entries-layout`
  - `settings-grid`
  - `reader-page`
- 截图可人工确认：
  - 导航未消失
  - 表单和列表未挤爆
  - 阅读页正文和底部栏仍可用

## 结果记录

脚本会生成：

- `target/static-web-small-viewport-smoke/<timestamp>/summary.md`

建议补：

- 每条路径结果
- 视口尺寸
- 是否存在小视口布局回退

## 当前基线

- 2026-04-10 已完成一轮人工视觉验收：
  - 产物目录：`target/static-web-small-viewport-smoke/20260410-213206/`
  - 视口：`390x844`
  - 结论：`/entries`、`/feeds`、`/settings`、`/entries/2` 均可接受
- 本轮通过条件：
  - 导航未消失
  - 表单与列表未挤爆
  - 阅读页正文和底部栏仍可用
