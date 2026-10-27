# 目录模块维护入口

目录只组织和定位已有文章，不查询文章、不保存已读／收藏、不处理订阅。时间路径是“月份 → 日期 → 文章”，来源路径是“来源 → 月份 → 文章”。两者共用文章分支和叶节点。

## 先按职责找到入口

| 修改内容 | 入口 | 保持的边界 |
| --- | --- | --- |
| 展示快照、文章身份、全局页码 | [../directory.rs](../directory.rs) 的 `DirectoryModel` | 借用既有 presenter / entries 的 Arc；不复制正文或建立第二份列表 |
| 分组树与控件结构 | [view.rs](view.rs) | `article_branch` 共用标签→展开按钮→文章的结构；读取状态，不写控制器 |
| 展开、跟随／手动、上下文失效 | [controller.rs](controller.rs) | 纯 Rust 状态转移；不读取 DOM 或执行 I/O |
| Dioxus 生命周期与页面命令 | [../directory.rs](../directory.rs) 的 `DirectoryBridge` | `set_page` 仍经 session / intent；保留 epoch / revision 提交顺序 |
| 点击／键盘归因、测量、实际滚动 | [adapter.js](adapter.js) | 只提供宿主事实和执行能力；不另存业务状态 |
| 消息解码与通道结束 | [transport.rs](transport.rs) | 区分单条坏消息和传输关闭，不把解码失败当作正常状态 |
| 可变行高与窗口范围 | [window.rs](window.rs) | 独立 Rust 模型；56px 是未测量估计，不是固定行高 |
| 虚拟文章挂载与窗口桥接 | [virtual_articles.rs](virtual_articles.rs)、[window_adapter.js](window_adapter.js) | 复用 `DirectoryModel::resolve_article` 和 `DirectoryArticle`，不复制身份／分页规则 |
| 箭头、缩进和响应式外观 | [reading-workspace.css](../../../../../../assets/styles/reading-workspace.css) 的 Directory 块 | 保持稳定 `data-layout` / `data-slot`；共享按钮基础样式，目录几何变量仅作用于 rail |

## 修改时必须守住的关系

- `DirectoryModel` 的 `presenter` 与 `entries` 是不同身份：渲染 props 用 Arc 身份比较，交互重置使用较窄的 `DirectoryResetIdentity`，不要把任意 presenter 变化都变成目录重置。
- `resolve_article` 同时校验全局 `(index, id)`，再计算目标页。过期或越界引用返回 `None`；普通和虚拟分支都跳过它，不能按下标误展示另一篇文章。不要使用窗口内下标计算页码。
- `ArticleBranch` 只是借用两种投影中已有字段的显示描述，不持有状态，不克隆树。分支类型替代语义不明的 `true / false` 参数。
- 月份／日期标签负责定位；右侧 `entry-directory-branch-toggle` 负责展开。DOM 顺序与视觉、Tab 顺序一致。父级 group toggle 与文章 branch toggle 的行为不同，不合并为一个含糊的动作。
- `data-directory-kind`、`data-directory-anchor`、`data-directory-group-anchor`、`data-directory-page` 连接视图与 adapter；`aria-expanded` / `aria-controls` 连接控件与文章容器。调整内部类名时保留这些语义接口。
- `article_branch` 返回键控根节点；分支集合直接迭代这些节点。虚拟文章也必须保持键控行在迭代器根部，不包裹无 key 的逐行片段，否则移动窗口可能丢失焦点。
- `VIRTUAL_ARTICLE_THRESHOLD` 仅决定小分支直接渲染或超过 80 篇后窗口化；不得改变完整查询投影、选择范围或跨页身份。
- 样式中的 `--directory-hit-size` 同时决定分支点击区域、父级箭头居中位置和计数预留；父级／分支箭头分别为 6px / 5px，文章容器缩进为 8px。修改几何后需要运行实际浏览器回归。

## 验证入口

1. `cargo fmt --all --check`
2. `cargo test -p rssr-app --locked directory::`
3. `cargo clippy -p rssr-app --all-targets --locked -- -D warnings`
4. 使用与锁文件匹配的 dx 0.7.9：`dx build --platform web --package rssr-app --release --locked --debug-symbols false`。release 保留完整优化，省去会触发本机 Binaryen DWARF 写出异常的调试信息；不要仅凭 dx 退出 0 判断优化成功，见[构建验收说明](../../../../../../docs/testing/release-ui-runner.md#web-release-构建约定)。
5. 用 `PUBLIC_DIR=target/dx/rssr-app/release/web/public` 指向刚构建的产物，依次执行已有脚本 `scripts/browser/rssr_directory_offline_regression.cjs` 和 `scripts/browser/rssr_directory_window_acceptance.cjs`。脚本使用隔离无头 Chrome 和离线样例；依赖解析按本机已有 Playwright 安装设置。

身份／分页规则修改需覆盖过期 ID、越界引用、页边界；视图／样式修改需覆盖时间与来源分组、手机浮层、鼠标／键盘展开。窗口路径修改还需检查焦点、跨页和收起后恢复；专项入口见下列契约。

现行产品语义见[目录跟随契约](../../../../../../docs/design/entries-directory-follow.md)，宿主与控制器约束见[原生界面前置边界](../../../../../../docs/design/native-ui-preparation.md)，主题稳定接口见[主题作者参考](../../../../../../docs/design/theme-author-selector-reference.md)。历史验收不代替本次修改后的验证。
