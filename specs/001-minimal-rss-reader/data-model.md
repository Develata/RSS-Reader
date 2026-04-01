# 数据模型：极简个人 RSS 阅读器 MVP

## 实体 1：订阅源（Feed）

**用途**：表示用户保存的 RSS/Atom 信息源，是文章抓取与分组展示的根对象。

### 字段

| 字段 | 类型 | 说明 |
|------|------|------|
| id | 本地整数主键 | SQLite 主键 |
| url | 文本 | 订阅源规范 URL，唯一 |
| title | 文本，可空 | 源站标题 |
| site_url | 文本，可空 | 源站主页 |
| description | 文本，可空 | 源站说明 |
| icon_url | 文本，可空 | 图标地址 |
| folder | 文本，可空 | 导入导出保真字段，用于保留 OPML / 配置中的原始分组信息 |
| etag | 文本，可空 | 条件请求标记 |
| last_modified | 文本，可空 | 条件请求时间戳 |
| last_fetched_at | 时间，可空 | 最近尝试刷新时间 |
| last_success_at | 时间，可空 | 最近成功刷新时间 |
| fetch_error | 文本，可空 | 最近一次错误摘要 |
| is_deleted | 布尔 | 软删除标志 |
| created_at | 时间 | 创建时间 |
| updated_at | 时间 | 最后更新时间 |

### 校验规则

- `url` 必须是合法 URL。
- 同一 `url` 只能存在一个有效订阅源。
- `folder` 为可选字段，缺失时不影响阅读能力。
- `folder` 仅用于导入导出保真与互操作，不参与当前 GUI 的阅读组织能力。

### 状态变化

- 新建：用户添加订阅源。
- 更新：刷新后同步标题、描述、条件请求信息。
- 软删除：用户移除订阅源时标记删除，文章保留策略由实现决定。

## 实体 2：文章（Entry）

**用途**：表示抓取并保存在本地的文章条目，是文章列表、阅读页、搜索与状态管理的核
心对象。

### 字段

| 字段 | 类型 | 说明 |
|------|------|------|
| id | 本地整数主键 | SQLite 主键 |
| feed_id | 外键 | 关联订阅源 |
| external_id | 文本，可空 | 源内稳定标识 |
| dedup_key | 文本 | 持久化去重键 |
| url | 文本，可空 | 原文链接 |
| title | 文本 | 标题 |
| author | 文本，可空 | 作者 |
| summary | 文本，可空 | 摘要 |
| content_html | 文本，可空 | 阅读页使用的正文缓存 |
| content_text | 文本，可空 | 搜索和降级展示文本 |
| published_at | 时间，可空 | 发布时间 |
| updated_at_source | 时间，可空 | 源端更新时间 |
| first_seen_at | 时间 | 首次抓取时间 |
| content_hash | 文本，可空 | 内容摘要，用于更新检测 |
| is_read | 布尔 | 已读状态 |
| is_starred | 布尔 | 收藏状态 |
| read_at | 时间，可空 | 已读时间 |
| starred_at | 时间，可空 | 收藏时间 |
| created_at | 时间 | 创建时间 |
| updated_at | 时间 | 最后更新时间 |

### 校验规则

- `(feed_id, dedup_key)` 必须唯一。
- `dedup_key` 必须按以下优先级持久化生成：`external_id` → `url` → 归一化标题 + 发布时间。
- `title` 不能为空；若源站缺失标题，需在应用层生成可用占位标题。
- `content_html` 与 `content_text` 至少应有一种可用于阅读或搜索。

### 状态变化

- 新建：首次抓取到该文章。
- 更新：源站内容变化时更新摘要、正文、源端更新时间和内容摘要；同一 `dedup_key` 命中时不得新建重复记录。
- 已读切换：`is_read` 与 `read_at` 联动。
- 收藏切换：`is_starred` 与 `starred_at` 联动。
- 归档判定：归档属于基于发布时间和用户阈值实时计算的阅读组织状态，不作为单独持久化字段写入数据库。

## 实体 3：用户偏好设置（UserSettings）

**用途**：表示本地阅读与界面偏好，并作为配置包的一部分被导入导出。

### 字段

| 字段 | 类型 | 说明 |
|------|------|------|
| theme | 枚举 | `light` / `dark` / `system` |
| list_density | 枚举 | `comfortable` / `compact` |
| startup_view | 枚举 | `all` / `last_feed` |
| refresh_interval_minutes | 整数 | 刷新间隔 |
| archive_after_months | 整数 | 自动归档阈值，默认 `3` |
| reader_font_scale | 浮点数 | 阅读字号缩放 |
| custom_css | 文本 | 用户自定义主题 CSS，可为空 |

### 校验规则

- `refresh_interval_minutes` 必须大于 0。
- `archive_after_months` 必须大于 0。
- `reader_font_scale` 必须处于可接受范围，例如 `0.8` 到 `1.5`。

## 实体 4：配置包（ConfigPackage）

**用途**：用于导入导出和远端配置交换的可迁移载体。

### 字段

| 字段 | 类型 | 说明 |
|------|------|------|
| version | 整数 | 配置格式版本 |
| exported_at | 时间 | 导出时间 |
| feeds | 数组 | 订阅源列表 |
| settings | 对象 | 用户偏好设置 |

### 校验规则

- `version` 必须是已支持版本。
- `feeds` 中相同规范 URL 不得重复，导入器在 schema 通过后仍必须执行语义去重校验。
- 不允许包含文章正文、已读状态、收藏状态或搜索索引。

## 关系

- 一个订阅源可以拥有多篇文章。
- 一篇文章只能属于一个订阅源。
- 用户偏好设置属于单用户单安装环境。
- 配置包包含多个订阅源和一个偏好设置对象。

## 索引与约束建议

- `feeds.url` 唯一索引
- `entries(feed_id, dedup_key)` 唯一索引
- `entries(feed_id, published_at DESC)` 索引
- `entries(published_at DESC)` 索引
- `entries(is_read, published_at DESC)` 索引
- `entries(is_starred, published_at DESC)` 索引
- `entries(title)` 普通索引

## 业务规则

- 移除订阅源不等于启动完整同步删除。
- 配置交换永远是“覆盖式配置更新”，不是文章级合并。
- 文章列表默认按发布时间倒序。
- 文章列表默认隐藏超过自动归档阈值的旧文章，但这些文章仍保留在本地库中，可重新显示和阅读。
- 标题搜索只作用于本地文章标题。
- Web 端与原生端共享同一份 SQLite schema，只是持久化适配层不同。
