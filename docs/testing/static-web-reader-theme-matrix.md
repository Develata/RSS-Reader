# Static Web `/reader` 主题矩阵 Smoke

这份说明服务于静态 `rssr-app` Web 入口下，真实阅读页的多主题回归。

它解决的是：

- 不再手工进入设置页逐个切主题
- 让 `/reader` 的多主题检查变成固定入口
- 直接复用同源 local auth helper 与 `reader-demo` seed

## 脚本

- [run_static_web_reader_theme_matrix.sh](/home/develata/gitclone/RSS-Reader/scripts/run_static_web_reader_theme_matrix.sh)

## 最短用法

```bash
bash scripts/run_static_web_reader_theme_matrix.sh
```

默认行为：

- 启动带 SPA fallback 的静态 Web 服务
- 用 `reader-demo` seed 固定进入 `/entries/2`
- 依次覆盖：
  - 默认主题
  - `Atlas Sidebar`
  - `Newsprint`
  - `Amethyst Glass`
  - `Midnight Ledger`
- 对每个主题产出：
  - `*.html`
  - `*.png`

## 常用参数

```bash
bash scripts/run_static_web_reader_theme_matrix.sh --skip-build
bash scripts/run_static_web_reader_theme_matrix.sh --release
bash scripts/run_static_web_reader_theme_matrix.sh --port 8103
```

## 验收重点

- `/entries/2` 始终进入真实阅读页
- DOM 中仍有：
  - `data-page="reader"`
  - `data-layout="reader-page"`
  - 真实文章标题
- 非默认主题下，`user-custom-css` 已真实注入
- 不同主题下：
  - 标题、元信息、正文区、底部栏仍可读
  - 布局没有塌陷

## 结果记录

脚本会生成：

- `target/static-web-reader-theme-matrix/<timestamp>/summary.md`

建议直接在模板里补：

- 各主题结果
- 是否出现布局回退
- 是否允许进入发布前总回归
