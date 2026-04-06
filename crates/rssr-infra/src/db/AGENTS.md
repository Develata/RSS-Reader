# 数据库仓储模块说明

本目录负责原生端本地 SQLite 的仓储实现。

## 职责边界

- `sqlite_native.rs`
  - 原生端 SQLite backend
- `entry_repository.rs`
  - 文章读写、搜索、阅读导航相关查询
- `feed_repository.rs`
  - 订阅读写
- `settings_repository.rs`
  - 用户设置
- `app_state_repository.rs`
  - 应用运行态，例如最近打开订阅
- `storage_backend.rs`
  - 仓储统一抽象

## 关键原则

- 仓储层只处理存储与查询，不处理 UI 语义
- 查询顺序要与页面、阅读导航的预期保持一致
- 性能敏感路径优先：
  - 索引友好
  - 避免全表扫模拟导航
  - 避免无意义 clone

## 修改约束

- 任何文章排序或筛选变更，都要同步考虑：
  - 文章页列表
  - 阅读页上一未读 / 下一未读
  - 搜索结果一致性
- 不把 OPML / JSON 配置交换逻辑塞进仓储层
- 不把 Web `localStorage` 实现语义误混进 SQLite 路径

## 测试要求

- 改 `entry_repository.rs` 时，优先补或更新相关集成测试
- 搜索、导航、排序改动后，至少检查：
  - `test_entry_state_and_search`

