# 2026-04-08 Headless Phase 1 Inventory

## Summary

- 订阅页已经完成 phase 1 的命令/查询分层，是当前最成熟的 headless 命令面。
- 文章页已经完成局部 workspace/session 骨架，是当前最接近 `headless active interface` 目标的页面。
- 阅读页已经完成局部 session 化，动作、快捷键和运行时结果都走统一链路。
- 设置页不适合整页状态机，但已经把最重的两块能力收成局部 session：配置交换和保存设置。

## Scope Reviewed

- `crates/rssr-app/src/pages/feeds_page.rs`
- `crates/rssr-app/src/pages/feeds_page/{bindings,commands,dispatch,queries}.rs`
- `crates/rssr-app/src/pages/entries_page*.rs`
- `crates/rssr-app/src/pages/reader_page*.rs`
- `crates/rssr-app/src/pages/settings_page*.rs`
- `docs/design/headless-active-interface.md`

## Per-Page Assessment

### Feeds Page

Status: phase 1 complete

Current shape:
- 命令层已独立：`FeedsPageCommand`
- 分发层已独立：`execute_feeds_page_command`
- 查询层已独立：`load_feeds_page_snapshot`
- 绑定层已独立：`FeedsPageBindings`

Why it is considered phase 1 complete:
- 添加订阅、删除订阅、配置交换都不再直接写在页面按钮闭包里。
- 页面主壳主要只负责 signal 组装、触发初始查询和渲染 section。
- 查询与命令已经明显先于 DOM 结构存在。

Remaining gap to full headless:
- 还没有显式 local session/reducer。
- 页面主壳仍然自己持有较多 signal。

Recommendation:
- 订阅页暂时不必继续重构，保持现状即可。

### Entries Page

Status: phase 1.5 complete

Current shape:
- 局部状态：`EntriesPageState`
- 意图层：`EntriesPageIntent`
- 规约器：`entries_page_reducer`
- 查询层：`entries_page_queries`
- 副作用层：`EntriesPageEffect`
- runtime：`execute_entries_page_effect`
- 绑定层：`EntriesPageBindings`
- presenter：`EntriesPagePresenter`

Why it is ahead of the other pages:
- 加载、筛选、分组偏好保存、卡片动作已经统一进入局部 workspace 流。
- 页面主文件已经明显退化成 view shell。
- 派生视图模型不再散落在多个闭包里，而是集中在 presenter/state。

Remaining gap to full headless:
- 仍未显式引入单独的 `EntriesPageSession` 壳。
- `use_resource/use_effect` 仍然散布在页面壳内，尚未完全统一为 session 驱动。
- 查询结果目前仍然由页面壳协调触发，而不是完全交由单一 session 管理。

Recommendation:
- 如果继续推进 headless 主线，文章页下一步最值的是补一个显式 `EntriesPageSession`，把加载和副作用触发统一收口。

### Reader Page

Status: phase 1 complete

Current shape:
- 状态：`ReaderPageState`
- 意图：`ReaderPageIntent`
- 规约器：`reader_page_reducer`
- 副作用：`ReaderPageEffect`
- runtime：`execute_reader_page_effect`
- 绑定层：`ReaderPageBindings`
- session：`ReaderPageSession`

Why it is considered phase 1 complete:
- 正文加载、已读/收藏动作都走 `effect -> runtime -> intent -> reducer -> state`。
- 快捷键不再直接碰页面实现，而是通过 `ReaderPageSession` 进入动作链。
- 页面主壳基本只做路由、布局和命令入口渲染。

Remaining gap to full headless:
- 导航动作仍然较多直接写在按钮闭包里，尚未提升为统一导航命令族。
- 还没有显式 presenter 层。

Recommendation:
- 暂时不继续细拆阅读页，收益已经开始下降。

### Settings Page

Status: selective phase 1

Current shape:
- 主页面壳保持较薄。
- `settings_page_sync_*` 已形成局部 sync session。
- `settings_page_save_*` 已形成局部 save session。
- 主题实验室 support 已拆成 `theme_apply/theme_io/theme_preset/theme_validation`。

Why it should stay selective:
- 设置页更像工具卡片集合，不像文章页那样是持续交互工作台。
- 强行做整页状态机会为了统一而统一，收益偏低。

Remaining gap to full headless:
- 还没有跨卡片统一的设置命令面。
- 不同卡片的 `data-action` / 命令命名还未进行全页一致性整理。

Recommendation:
- 保持“局部 session + 薄主壳”的方向，不要急着做总 `SettingsPageSession`。

## Inventory Conclusion

当前项目对照 `headless-active-interface.md` 的实现状态可以总结为：

- 订阅页：phase 1 已完成
- 阅读页：phase 1 已完成
- 设置页：局部能力完成 phase 1，整页不建议继续统一
- 文章页：最接近目标状态，但还差显式 session 收口

## Recommended Next Step

最值得继续推进的是：

1. 为文章页补 `EntriesPageSession`
2. 把文章页现有的加载与副作用触发统一迁入 session
3. 之后再做一次跨页命令面命名与 `data-action` 一致性整理

不建议的下一步：

- 把设置页强行推成整页状态机
- 继续细拆阅读页内部文件
- 回到大规模 CSS 剥离而暂时搁置命令面收口

## Verification

- 本轮主要为实现盘点与收口建议，没有引入新的行为改动。
- 工作区仅清理了 `theme_io.rs` 中无意义的导入顺序 diff，并补充本 inventory 文档。
