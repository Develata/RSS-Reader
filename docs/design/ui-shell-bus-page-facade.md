# UI Shell / Bus / Page Facade 边界

## 目的

这份文档描述 `rssr-app` 当前已经落地的前端边界：

- `ui/shell`
- `ui bus`
- `page facade`
- `infra / application`

它回答的是：

- 现在 repo 里的前端真实结构是什么
- 哪些职责应该留在 `ui/shell`
- 哪些职责应该留在 `UiCommand / UiRuntime / UiIntent`
- 哪些职责应该留在 `page facade`
- 哪些东西不应再回流到页面组件

这份文档是 [Headless Active Interface 设计目标](./headless-active-interface.md) 的当前实现版，不是愿景版。

---

## 当前分层

### 1. `rssr-domain`

负责：

- 实体
- 枚举和值对象
- query object
- repository trait

不负责：

- UI
- 平台 API
- 持久化实现

### 2. `rssr-application`

负责：

- 共享 use case
- 跨平台业务行为

当前主要包括：

- feed service
- entry service
- settings service
- refresh service
- subscription workflow
- import/export service

### 3. `rssr-infra`

负责：

- SQLite / native 实现
- browser persisted-state 实现
- parser / fetch / WebDAV / OPML / JSON codec

这层是“真实实现层”，不是 UI 的附属工具层。

### 4. `rssr-app/src/ui`

负责：

- shell state
- shell facade
- global UI command bus
- UI intent projection
- bus helper

当前目录：

- [commands.rs](../../crates/rssr-app/src/ui/commands.rs)
- [runtime.rs](../../crates/rssr-app/src/ui/runtime.rs)
- [snapshot.rs](../../crates/rssr-app/src/ui/snapshot.rs)
- [helpers.rs](../../crates/rssr-app/src/ui/helpers.rs)
- [shell.rs](../../crates/rssr-app/src/ui/shell.rs)

### 5. `rssr-app/src/pages/*`

负责：

- page-local state
- reducer
- session
- facade
- semantic DOM

不再负责：

- 直接拉 `AppServices::shared()`
- 自己拼业务副作用
- 自己长期持有 runtime 层

---

## `ui/shell` 的职责

`ui/shell` 是应用壳层，不是业务层。

它负责：

- 认证壳状态
- startup route 壳
- 顶部导航壳
- 全局搜索输入壳
- Web auth gate 壳

典型接口见：

- [shell.rs](../../crates/rssr-app/src/ui/shell.rs)

它可以持有：

- `Signal<String>`
- `Signal<bool>`
- `Navigator`
- 与壳渲染直接相关的状态文案

它不应负责：

- 订阅刷新
- 文章读取
- 配置导入导出
- 设置持久化

也就是说，`shell` 负责“应用壳如何运转”，不负责“阅读器业务如何执行”。

---

## `UiCommand / UiRuntime / UiIntent`

### `UiCommand`

`UiCommand` 是统一动作入口。

它表达的是：

- 需要执行什么行为
- 行为所需输入是什么

当前定义见：

- [commands.rs](../../crates/rssr-app/src/ui/commands.rs)

命令命名应满足：

- 和页面无关
- 和按钮文本无关
- 一个命令只表达一个清晰行为

### `UiRuntime`

`UiRuntime` 负责：

- 接收 `UiCommand`
- 调用 `AppServices`
- 访问 application / infra 能力
- 产出 `Vec<UiIntent>`

当前实现见：

- [runtime.rs](../../crates/rssr-app/src/ui/runtime.rs)

它是“前端行为承接面”，不是页面私有 helper。

### `UiIntent`

`UiIntent` 是 bus 返回给页面层的投影结果。

它不是业务实体，也不是数据库模型。

它负责：

- 页面状态所需的最小投影
- 页面 reducer 可消费的结果
- 启动壳 / 认证壳 / 页面状态切换

---

## `page facade` 的职责

每个主页面现在都已有 facade：

- [entries_page/facade.rs](../../crates/rssr-app/src/pages/entries_page/facade.rs)
- [reader_page/facade.rs](../../crates/rssr-app/src/pages/reader_page/facade.rs)
- [feeds_page/facade.rs](../../crates/rssr-app/src/pages/feeds_page/facade.rs)
- [settings_page/facade.rs](../../crates/rssr-app/src/pages/settings_page/facade.rs)

facade 是页面边界对象，不是简单 DTO。

它负责三类事：

### 1. snapshot accessors

例如：

- `status_message()`
- `status_tone()`
- `total_feed_count()`
- `visible_entries_len()`
- `published_at()`

### 2. action slots

例如：

- `toggle_read(...)`
- `toggle_starred(...)`
- `set_grouping_mode(...)`
- `save()`
- `push()`
- `pull()`
- `remove_feed(...)`

### 3. 默认展示策略

例如：

- `has_status_message()`
- `save_button_label()`
- `config_import_button_label()`
- `remove_feed_button_label(...)`
- `theme_card_class(...)`
- `theme_apply_button_label(...)`

也就是说，页面和 section/card/control 不应再自己散落地决定：

- 某个按钮当前显示什么文案
- 某个危险态按钮现在用什么 class
- 某段默认状态文案是什么

这些默认策略应尽量先收进 facade。

---

## 页面组件现在应该做什么

页面组件现在应只做：

- 挂 facade/workspace
- 输出语义 DOM
- 使用稳定的 `data-*`
- 渲染 facade 暴露的值
- 调用 facade 暴露的动作

也就是说，页面组件应该更像：

- semantic shell
- default page skin

而不是：

- 行为实现中心
- 展示策略散落地
- 直接依赖 service 的 controller

---

## CSS 与页面结构的关系

当前目标不是“让页面完全没有结构”，而是：

- 页面只保留语义结构
- CSS 决定视觉和布局
- facade 决定默认行为语义与默认展示策略

因此：

- DOM 可以继续存在 card / rail / toolbar / section
- 但这些结构不应绑死真实行为
- 只要 `data-page` / `data-nav` / `data-action` / `data-field` 稳定，CSS 就应能自由重排

---

## 不应该再回流的反模式

后续改动中，应避免重新出现这些模式：

### 1. 页面组件里直接 `AppServices::shared()`

这会绕过 bus，重新把页面变回行为中心。

### 2. 页面组件里新建 page-local runtime

当前已经完成从 `page-local runtime + UiRuntime` 向“薄 session + facade + UiRuntime”迁移。

除非有非常强的本地浏览器能力理由，否则不要再回到每页一套 runtime。

### 3. 组件内部自己长期决定默认展示策略

例如：

- 删除确认文案
- 危险态按钮 class
- 默认状态 banner 是否显示

这些应优先进 facade。

### 4. 让 CSS 反推业务状态

CSS 可以控制显示，不应承担业务判断。

---

## 继续演进时的优先顺序

如果后续继续沿这条线推进，优先顺序应是：

1. 保持 `UiCommand` 命名与边界稳定
2. 保持 facade 的 `snapshot + action + default strategy` 结构稳定
3. 只在 facade 无法表达时，才考虑新增壳层 helper
4. 最后才考虑是否继续抽更高层的共享 page boundary trait

不建议现在做：

- 巨型全局 reducer/store
- 让所有页面强制同构
- 重新把 application 语义搬回 UI 状态层

---

## 当前状态判断

到目前为止，`rssr-app` 的页面层已经从：

- page-local runtime
- 页面自己调 service
- 页面自己决定大量默认展示策略

演进到：

- `ui/shell`
- `UiCommand / UiRuntime / UiIntent`
- `page facade`
- semantic page shell

这已经是比较明确的：

- `headless active interface`
- `CSS 完全分离`
- `infra` 承担真实行为

方向实现版。
