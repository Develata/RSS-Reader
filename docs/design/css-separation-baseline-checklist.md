# CSS 完全分离基线清单

## 目标

这份清单用于维护当前 UI 的 CSS 分离基线，避免后续主题、页面和组件继续回退到以下旧模式：

- 依赖深 DOM 层级和匿名子节点
- 依赖位置选择器驱动业务语义
- 把页面结构 class 误当成对外稳定接口
- 把主题覆写写死在具体组件实现细节上

当前判断口径是：

- 页面和主题优先依赖稳定的 `data-*` 语义接口
- class 只保留给设计系统 primitive、utility 或组件内部实现
- `infra` / 页面代码负责产出真实语义，不让 CSS 反向猜 DOM

## 当前基线

### 已稳定的公开接口

- 状态接口：
  - `data-state`
  - `data-variant`
  - `data-density`
- 结构接口：
  - `data-layout`
  - `data-slot`
  - `data-page`
  - `data-nav`
  - `data-action`
  - `data-field`
  - `data-context`
  - `data-list-edge`

### 已完成的主要迁移

- 状态语义已从旧 modifier class 迁到数据属性：
  - `status-banner.*` -> `data-state`
  - `button.*` -> `data-variant`
  - `app-shell.density-*` -> `data-density`
  - 主题卡、来源筛选 chip、阅读底栏按钮 -> `data-state`
- 页面结构和阅读壳已补齐稳定布局语义：
  - 导航壳、阅读页、顶部目录、侧边目录、筛选区、Web 门禁壳均已迁到 `data-layout` / `data-slot`
- 阅读列表边界语义已收口：
  - 原先依赖 `:first-child` / `:last-child` / 邻接选择器的边界判断改为 `data-list-edge`
- 内置主题首轮旧 selector 已清掉：
  - `.app-nav*`
  - `.reader-page*`
  - `.entry-filters*`
  - `.button.secondary/.danger/.danger-outline`
  - `.theme-card.is-active`

## 允许保留的 class 边界

### 设计系统 / 全局 primitive

- `app-shell`
  - 作为应用壳根类保留。
  - 密度等状态语义已经由 `data-density` 承担。
- `theme-light` / `theme-dark` / `theme-system`
  - 当前仍可作为主题根状态类保留。
  - 如果后续统一成 `data-theme`，应单独立项，不混在页面槽位收口里推进。
- `button`
- `text-input`
- `text-area`
- `select-input`
- `field-label`
- `icon-link-button`
- `inline-actions`
- `inline-actions__item`
  - 这些属于 design-system primitive，不需要为了“完全分离”强行公开成页面语义接口。

### Utility / a11y

- `sr-only`
- `sr-only-file-input`

这类 class 必须保留为 utility，不做语义化改造。

### 组件内部实现

- `reader-bottom-bar__button`
  - 当前状态语义已通过 `data-state` 暴露。
  - 按钮本体 class 可以继续作为阅读底栏内部实现。

## 允许存在的例外

### `reader-body-html`

- 这是正文内容岛，不属于稳定页面壳语义。
- 其中标签级样式允许继续使用元素选择器和内容容器限定。
- 后续如需收口，只能按“内容渲染策略”处理，不能机械替换成大批 `data-slot`。

### 主题内的直接子布局规则

- 仅当某主题确实需要控制壳层直系布局时，允许保留有限的直接子选择器。
- 前提是页面入口已经先暴露 `data-layout` / `data-slot`，主题只在这些稳定入口上做补充，而不是反过来猜结构。

## 审查命令

### 审查 CSS 中剩余 class selector

```bash
rg --pcre2 -o '(^|[,\\s])\\.[A-Za-z][A-Za-z0-9_-]*(?=[\\s:{.#\\[,>+~)]|$)' assets/styles assets/themes -S
```

### 审查 Rust DOM 中直接输出的 class

```bash
rg -o 'class: "[^"]+"' crates/rssr-app/src -S
```

### 审查深选择器

```bash
rg -n "(^|[,{])[^{}]*(>|\\+|~)[^{}]*\\{" assets/styles assets/themes -S
```

### 审查标签选择器是否越过稳定语义边界

```bash
rg -n "\\.[A-Za-z][A-Za-z0-9_-]+\\s+(h[1-6]|p|ul|ol|li|img|figure|button|span|div|input|textarea|select)\\b|\\]\\s+(h[1-6]|p|ul|ol|li|img|figure|button|span|div|input|textarea|select)\\b" assets/styles assets/themes -S
```

## 下一轮判断规则

遇到一个 class 或 selector 时，按下面顺序判断：

1. 它是否表达页面公开语义？
2. 它是否已经有现成的 `data-layout` / `data-slot` / `data-state` 可替代？
3. 它是否只是 design-system primitive 或 utility？
4. 它是否只是组件内部实现，不应暴露给主题作者？

只有第 1 类缺失语义接口时，才继续迁移到 `data-*`。

不要再做这些事：

- 为了“看起来更纯”把所有 class 一次性改成 `data-*`
- 在没有页面语义需求时额外制造新公开接口
- 让主题继续依赖匿名节点顺序或深层嵌套

## 剩余重点

### P1

- 继续清理“页面语义缺失”的个别区域，而不是清空所有 class。
- 新增页面或组件时，先补稳定 `data-*` 入口，再写主题覆写。
- 新主题和新 smoke 断言统一优先依赖 `data-layout` / `data-slot` / `data-state`。

### P2

- 持续复查内置主题是否重新引入页面私有 class selector。
- 对仍保留的实现 class，确认它们没有被跨主题或跨页面当成公开契约使用。

## 完成标准

满足以下条件即可认为当前 CSS 分离基线稳定：

- 页面和主题不再依赖深 DOM 层级表达主语义
- 主题覆写优先使用稳定 `data-*` 接口
- 保留 class 的用途能清楚归类到 primitive、utility 或内部实现
- 新增页面改动可以通过 selector 审查，不再反复回退旧模式
