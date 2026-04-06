# 架构审查报告（2026-04）

## 1. 审查范围

### 覆盖范围

本次审查以 Rust workspace 的当前实现为对象，重点覆盖以下目录与 crate：

- `Cargo.toml`
- `crates/rssr-app`
- `crates/rssr-cli`
- `crates/rssr-web`
- `crates/rssr-application`
- `crates/rssr-domain`
- `crates/rssr-infra`
- `migrations/`
- `docs/`
- `specs/`

本次审查重点读取了 workspace 清单、各 crate 的 `Cargo.toml`、关键入口文件、核心模块与迁移文件，包括但不限于：

- `src/main.rs`
- `src/lib.rs`
- `bootstrap/*`
- `pages/*`
- `db/*`
- `parser/*`
- `config_sync/*`

### 审查目标

本次审查的目标不是评估单个功能是否可用，而是回答以下问题：

- 当前 workspace 分层是否与实际代码结构一致
- 哪些 crate 边界是有效的，哪些只是名义分层
- 当前维护成本主要来自哪里
- 如果要面向长期维护做重构，最值得优先处理的切入点是什么

### 本次审查不解决的问题

本次审查不直接处理以下事项：

- 不新增功能
- 不修复具体业务 bug
- 不调整 UI 视觉设计
- 不修改数据库 schema
- 不重新设计产品范围
- 不直接给出“推倒重写”方案

本次输出仅服务于后续架构收敛与低风险重构。

## 2. 当前架构概览

### Workspace / crate 总览

当前 workspace 包含以下 crate：

- `rssr-app`
- `rssr-cli`
- `rssr-web`
- `rssr-application`
- `rssr-domain`
- `rssr-infra`

整体依赖方向大体保持无环：

- `rssr-domain` 作为共享模型与 trait 基础层
- `rssr-application` 依赖 `rssr-domain`
- `rssr-infra` 依赖 `rssr-domain`
- `rssr-app` 依赖 `rssr-application`、`rssr-domain`，在 native / android 目标下直接依赖 `rssr-infra`
- `rssr-cli` 同时依赖 `rssr-application`、`rssr-domain`、`rssr-infra`
- `rssr-web` 独立存在，不复用当前核心分层

### 各 crate 的名义职责与实际职责

#### `rssr-domain`

名义职责：

- 承载核心领域模型与规则
- 定义仓储 trait

实际职责：

- 主要承载共享数据结构
- 定义 `FeedRepository`、`EntryRepository`、`SettingsRepository`
- 只包含少量真实规则，例如：
  - `normalize_feed_url`
  - `is_entry_archived`

判断：

- 更接近“共享类型层 + trait 层”
- 还不是强业务规则中心

#### `rssr-application`

名义职责：

- 承载共享 use case / 应用服务

实际职责：

- `FeedService`、`EntryService` 主要是对 repository 的透传
- `SettingsService` 提供少量校验
- `ImportExportService` 是当前最接近真实应用服务的模块

判断：

- 目前偏薄
- 尚未成为真正的共享 use case 层

#### `rssr-infra`

名义职责：

- 承载数据库、HTTP、解析、导入导出、远端配置等基础设施实现

实际职责：

- 实际上确实承担了主要 adapter 角色
- 包含：
  - SQLite 连接与迁移
  - feed / entry / settings repository
  - HTTP 抓取
  - feed 解析
  - OPML 编解码
  - WebDAV 配置同步

判断：

- 这是当前最名副其实的一层

#### `rssr-app`

名义职责：

- 作为 Dioxus UI 入口与页面层

实际职责：

- 远不止 UI
- native 端在 `bootstrap/native.rs` 中承担了：
  - 依赖装配
  - 数据库初始化与迁移
  - 自动刷新调度
  - feed 抓取编排
  - 解析与入库流程编排
  - 图片本地化后台任务
  - OPML / 配置交换 / WebDAV 调用
- wasm 端在 `bootstrap/web/*` 中自带一整套：
  - 浏览器状态持久化
  - feed 抓取与解析
  - 配置校验
  - OPML 编解码
  - 导入导出与远端配置逻辑

判断：

- 当前 `rssr-app` 已成为事实上的主业务层之一
- 与“UI 层”定位明显不一致

#### `rssr-cli`

名义职责：

- 命令行入口

实际职责：

- 并非薄入口
- 内部有独立的 `CliServices`
- 重复实现了：
  - 初始化数据库
  - feed 刷新流程
  - OPML 导入导出
  - WebDAV 配置同步

判断：

- 是第二套应用编排实现
- 与 native 端有明显重复

#### `rssr-web`

名义职责：

- Web 相关 crate

实际职责：

- 并不是浏览器端阅读器核心
- 当前主要承担部署包装层职责：
  - 静态文件服务
  - 登录门禁
  - feed proxy

判断：

- 更接近“部署壳”而不是“核心 Web 平台层”

### 当前最核心的边界失真

当前最核心的边界失真是：

- `rssr-app` 名义上是 UI crate，实际上承载了大量应用编排与基础设施调用
- `rssr-application` 名义上是应用层，实际上没有吸收大部分核心 use case
- browser / native / cli 三个入口没有共享同一套用例实现，而是在各自重复实现业务流程

这使得“理论分层”与“实际分层”存在明显偏差。

## 3. 核心问题清单

### P0

#### P0-1 三端核心用例重复实现

现象：

- native `rssr-app`
- wasm `rssr-app`
- `rssr-cli`

三处分别实现了刷新、导入导出、远端配置、部分订阅管理逻辑。

为什么影响长期维护：

- 同一业务规则需要多点修改
- 行为一致性难以长期保证
- 测试只能覆盖局部实现，无法自然约束跨入口一致性
- 每新增一个边界条件，都会放大维护成本

受影响的 crate / 模块：

- `crates/rssr-app/src/bootstrap/native.rs`
- `crates/rssr-app/src/bootstrap/web/*`
- `crates/rssr-cli/src/main.rs`

#### P0-2 UI crate 承担了事实上的主业务职责

现象：

- `rssr-app` 不只是页面和状态绑定
- native 端直接编排 repo、fetch、parser、WebDAV、OPML
- wasm 端甚至在 UI crate 内实现了自己的 browser-state backend

为什么影响长期维护：

- UI 层变成了业务演进的主入口
- 业务逻辑无法被其他入口自然复用
- 页面修改、平台适配、用例演进耦合在一起
- 后续想抽离共享 use case 时成本更高

受影响的 crate / 模块：

- `crates/rssr-app/src/bootstrap/native.rs`
- `crates/rssr-app/src/bootstrap/web.rs`
- `crates/rssr-app/src/bootstrap/web/*`

### P1

#### P1-1 `rssr-application` 过薄，未成为真正共享的 use case 层

现象：

- `FeedService`、`EntryService` 主要是透传 repository
- 真正复杂的行为仍散落在 native / cli / wasm 入口中

为什么影响长期维护：

- 多层结构存在，但核心价值没有落在中间层
- 维护者需要直接穿透到入口层看业务流程
- crate 虽然分了层，但层级并未有效吸收复杂度

受影响的 crate / 模块：

- `crates/rssr-application/src/feed_service.rs`
- `crates/rssr-application/src/entry_service.rs`
- `crates/rssr-application/src/settings_service.rs`
- `crates/rssr-application/src/import_export_service.rs`

#### P1-2 `rssr-domain` 规则承载不足，更像共享 DTO 层

现象：

- domain 中有核心模型与 trait
- 但除 URL 规范化、归档判定外，缺少更集中、更可复用的领域规则
- 设置校验、配置校验、导入导出规则散落在其他 crate

为什么影响长期维护：

- 业务规则位置不稳定
- 后续重构时难以判断哪些规则应当收敛到哪里
- 共享模型与业务约束没有形成同一演进重心

受影响的 crate / 模块：

- `crates/rssr-domain/src/*`
- `crates/rssr-application/src/settings_service.rs`
- `crates/rssr-app/src/bootstrap/web/config.rs`

#### P1-3 设置、应用状态、配置交换边界不够统一

现象：

- `UserSettings` 有 repository trait 和持久化实现
- `last_opened_feed_id` 却走 infra 私有仓储
- `RemoteConfigStore` trait 存在，但生产路径没有真正围绕它装配

为什么影响长期维护：

- 相近职责没有统一抽象
- 会出现“有抽象但未使用”和“有实现但未纳入边界”的并存状态
- 让后续边界压实更绕

受影响的 crate / 模块：

- `crates/rssr-infra/src/db/app_state_repository.rs`
- `crates/rssr-application/src/import_export_service.rs`
- `crates/rssr-app/src/bootstrap/native.rs`
- `crates/rssr-cli/src/main.rs`

### P2

#### P2-1 browser 端复制了一套与 infra 高度相似的逻辑

现象：

- `rssr-app/src/bootstrap/web/*` 中包含 feed 解析、配置校验、OPML 编解码、状态后端
- 这些能力与 `rssr-infra` 的职责高度相近，但未形成正式 adapter

为什么影响长期维护：

- 复制逻辑增加行为漂移风险
- UI crate 粒度继续膨胀
- 后续若想统一测试与规则，会遇到重复代码阻力

受影响的 crate / 模块：

- `crates/rssr-app/src/bootstrap/web/feed.rs`
- `crates/rssr-app/src/bootstrap/web/config.rs`
- `crates/rssr-app/src/bootstrap/web/exchange.rs`
- `crates/rssr-app/src/bootstrap/web/state.rs`

#### P2-2 存在半成品边界或遗留结构

现象：

- `HealthRepository` 暂未进入主要生产流程
- `sync_sources` 表已存在，但当前运行时路径未实际使用

为什么影响长期维护：

- 容易让维护者误判系统已有的能力边界
- 后续继续演进时，会增加“保留还是回收”的判断成本

受影响的 crate / 模块：

- `crates/rssr-domain/src/repository.rs`
- `migrations/0001_initial.sql`

## 4. 核心数据流观察

### 添加订阅

#### Native

调用路径大致为：

1. 页面事件直接调用 `AppServices::add_subscription`
2. `AppServices` 解析并规范化 URL
3. 调用 `FeedService::add_subscription`
4. `FeedService` 再调用 `FeedRepository::upsert_subscription`
5. 保存成功后立即执行 `refresh_feed`

特点：

- UI 并不是调用共享 use case，而是直接调用 `rssr-app` 内的 façade
- façade 内部同时承担“添加订阅”和“首次刷新”的编排职责

#### Wasm

调用路径大致为：

1. 页面直接调用 `AppServices::add_subscription`
2. `AppServices` 调用 browser-state mutation
3. mutation 直接在持久化状态中 upsert feed
4. 保存后再调用 `refresh_feed`

特点：

- 与 native 目标相同的用户行为，对应完全不同的实现路径

### 刷新订阅

#### Native

当前 refresh 流程主要集中在 `bootstrap/native.rs`：

1. 读取 feed
2. 构造条件请求
3. 通过 `FetchClient` 发起 HTTP 请求
4. 通过 `FeedParser` 解析 XML
5. 更新 feed 元数据
6. upsert entries
7. 更新抓取状态
8. 启动后台图片本地化任务

特点：

- 这是完整用例流
- 但它位于 `rssr-app` 中，而不是共享应用层中

#### Wasm

当前 refresh 流程主要集中在 `bootstrap/web/refresh.rs` 与 `bootstrap/web/feed.rs`：

1. 从 browser-state 读取 feed
2. 优先尝试 proxy URL，再回退 direct URL
3. 解析 feed
4. 更新浏览器本地持久化状态

特点：

- 逻辑上和 native 相似
- 实现上是独立重写

#### CLI

当前 refresh 流程主要集中在 `CliServices::refresh_feed`：

1. 读取 feed
2. 发起条件请求
3. 解析 feed
4. 更新 feed 元数据
5. 写入 entries
6. 更新抓取状态

特点：

- 与 native 端高度相似
- 但没有共用同一套实现

### 持久化 source / article / settings

#### Source / Feed

- native / cli 使用 SQLite `feeds` 表
- 由 `SqliteFeedRepository` 管理

#### Article / Entry

- native / cli 使用 SQLite `entries` 表
- 由 `SqliteEntryRepository` 管理

#### Settings

- native / cli 将 `UserSettings` 序列化为 JSON，保存到 `app_settings` 表中
- wasm 则将整包状态序列化后写入浏览器 `localStorage`

特点：

- domain 模型在三端共享
- 但持久化 adapter 未统一成正式抽象边界

### Native / Wasm / CLI 三端实现差异

#### Native

- 通过 `rssr-infra` 提供的 SQLite、HTTP、parser、OPML、WebDAV adapter 运行
- 当前 `rssr-app` 中承担大量编排逻辑

#### Wasm

- 不使用 SQLite
- 在 `rssr-app` 内自带 browser-state backend 与对应逻辑
- 不通过 `rssr-infra` 形成正式 adapter 层

#### CLI

- 使用 SQLite 与 `rssr-infra`
- 但独立维护一套 `CliServices`

总体观察：

- 三端共享了一套模型
- 但没有共享一套核心用例实现
- 当前最大问题不是平台差异本身，而是平台差异被直接映射成业务实现分叉

### Migration / SQLite 进入运行时的方式

SQLite 迁移由 `rssr-infra` 提供：

- `sqlx::migrate!("../../migrations")`
- `create_sqlite_pool`
- `migrate`

运行时接入方式：

- native app 在启动装配阶段连接数据库并执行迁移
- cli 在初始化服务时连接数据库并执行迁移

当前迁移目录只有一个初始迁移文件：

- `migrations/0001_initial.sql`

总体观察：

- 迁移入口清晰
- SQLite 运行时接入路径本身问题不大
- 但数据库初始化逻辑仍分散在入口层装配代码里

## 5. 目标架构建议

推荐方向不是推倒重来，而是在当前仓库现实上压实现实边界。

### 建议方向

#### 保留多 crate 结构，但压实现实边界

当前问题主要不在 crate 数量，而在于：

- 核心 use case 未被统一承载
- 入口层承担了过多业务职责
- 多平台实现直接演化成多套业务流程

因此更适合保留现有 workspace 结构，在此基础上收束责任。

#### `rssr-application` 应成为真正共享的 use case 层

应把以下能力逐步收束到 `rssr-application`：

- 刷新订阅
- 添加 / 删除订阅
- 配置导入导出
- OPML 导入导出
- 远端配置交换
- 启动状态与偏好驱动的运行时行为

目标不是让 `application` 变重，而是让复杂度真正集中到共享层。

#### `rssr-infra` 继续承载 adapter

`rssr-infra` 当前已经比较接近目标状态，应继续承载：

- SQLite adapter
- browser-state adapter
- HTTP fetch adapter
- feed parser
- OPML codec
- WebDAV adapter

重点不在改变 `infra` 的方向，而在于把 wasm 侧目前散落在 UI crate 中的逻辑也纳入正式 adapter 边界。

#### `rssr-app` / `rssr-cli` 只做入口与装配

`rssr-app` 应回到：

- 页面
- 交互绑定
- 依赖注入
- 平台入口

`rssr-cli` 应回到：

- 参数解析
- 命令入口
- 依赖装配

它们不应继续长期维护独立业务流程。

#### Wasm 相关逻辑应从 UI crate 中外移为正式 adapter

当前 `bootstrap/web/*` 中的逻辑并不适合作为 UI 私有实现长期保留。

更适合的方向是：

- 把 browser-state 持久化视为正式后端
- 把 wasm feed 刷新、配置校验、导入导出、状态更新能力外移
- 让 `rssr-app` wasm 端只负责装配与调用共享 use case

## 6. 分步重构路线图

### 第 1 步：收束核心 use case

目标：

- 把“刷新 + 订阅管理 + 配置交换”收束到真正共享的应用层

范围：

- `rssr-application`
- `rssr-app/src/bootstrap/native.rs`
- `crates/rssr-cli/src/main.rs`

风险：

- 现有入口层代码耦合较深
- 需要重新定义 application 层 API

完成标志：

- native 与 cli 不再各自维护刷新和配置交换主流程

### 第 2 步：把 wasm 逻辑外移为正式 adapter

目标：

- 让浏览器端持久化与刷新逻辑从 UI crate 私有实现转为正式 adapter

范围：

- `crates/rssr-app/src/bootstrap/web/*`
- 目标承载层应为共享 adapter 层

风险：

- wasm target 的依赖和 feature 需要重新整理
- 需要保持浏览器行为不回退

完成标志：

- `rssr-app` 的 wasm 装配代码只负责调用共享 use case，不再自行实现 feed / config / exchange 逻辑

### 第 3 步：统一状态与配置交换边界

目标：

- 清理相近职责的边界分裂

范围：

- `last_opened_feed_id`
- `RemoteConfigStore`
- WebDAV 生产装配路径
- 与 `sync_sources` 相关的残留结构

风险：

- 小范围抽象调整容易引入额外中间层

完成标志：

- 相关状态与远端配置能力都能通过统一边界进入应用层

### 第 4 步：收缩入口 crate 职责

目标：

- 让 `rssr-app` 和 `rssr-cli` 回到入口与装配角色

范围：

- `rssr-app`
- `rssr-cli`

风险：

- 需要同步调整 façade 与页面调用方式

完成标志：

- 入口 crate 中不再直接持有主要业务流程细节

### 第 5 步：补共享行为测试与一致性验证

目标：

- 用测试巩固新的共享边界

范围：

- 共享 use case
- SQLite adapter
- browser-state adapter
- 配置交换与导入导出行为

风险：

- 如果边界未先收束，测试会固化错误结构

完成标志：

- 核心行为可以通过共享测试验证，而不是主要依赖手工对比三端实现

## 7. 下一步行动建议

当前最推荐的下一步不是加新功能，而是先把“刷新 + 订阅管理 + 配置交换”收束为真正共享的 application use case。

这是当前最有价值的降复杂度动作，原因在于：

- 它直接命中当前最大的重复实现来源
- 它能为 native / wasm / cli 三端后续收敛提供统一支点
- 它能让现有多 crate 结构真正开始发挥作用

在这一步完成之前，继续增加新功能只会把现有边界失真进一步固化。
