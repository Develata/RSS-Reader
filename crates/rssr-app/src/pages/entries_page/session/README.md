# 文章页会话维护入口

`EntriesPageSession` 是当前页面的命令接口。页面组件通过 `use_entries_page_session` 装配会话，
每个页面实例只创建一次状态、查询协调器和保存队列。业务调用仍走 `UiCommand → runtime → Backend`。

## 按职责定位

| 要修改的行为 | 入口 | 不应混入的职责 |
| --- | --- | --- |
| 会话装配、状态发布、写入后使旧读取失效 | `../session.rs` | 文案、具体保存循环 |
| 初始化、重新读取、列表请求与返回页码 | `queries.rs`、`../request_lifecycle.rs` | 保存队列、文章写入回执 |
| 保存偏好、记住打开的订阅、合并最后一次选择、等待来源编辑 | `workspace.rs` | 查询代次、文章勾选范围 |
| 单篇已读／收藏与批量已读／归档 | `articles.rs` | 偏好字段拆装、全局通用任务框架 |
| 文章写入的提示文案与错误类别投影 | `../write_feedback.rs` | Dioxus signal、任务生命周期、Shell 状态 |
| 保持上述行为的延迟／卸载回归 | `tests.rs` | 以新预期替换已有行为契约 |

## 数据与生命周期

- 偏好以 `EntriesWorkspaceState` 整体经过页面 effect、会话、`SaveBrowsingPreferences` 和 Backend。
  不在中间层重新拆装字段。`preferences_loaded` 仍独立传入，用于拒绝偏好加载前捕获的默认值。
- `EntriesRequestLifecycle` 只保存初始化和列表查询的代次；`WorkspaceSaveQueue` 只保存最后一次等待写入的选择。
  页面状态中的 `workspace_wanted` 用于展示与重新读取时保留用户选择；队列的 `Rc` 副本用于原页面销毁后继续完成写入，
  二者生命周期不同。它们由保存模块同步更新，其他模块不接触队列内部。
- 初始化／重新读取先等待 App 的工作区写入边界，再发出读取；读取期间不持有锁，仍允许新代次替换旧读取。
- 文章写入后的补充读取必须走 `reread()`，由恢复规则保留读取失败期间用户选择的筛选；
  替换在途读取时也要读取完整设置与偏好，不能用只读订阅的初始化请求顶替。
- 偏好保存连续处理最后一次选择，在这一轮保存结束前持有同一写入边界；来源 URL 编辑不能插队。
- 单篇、批量、偏好和记住打开订阅的写入属于 App。回执只投递给发起操作的页面，且先回到该页面作用域，保证补充查询仍随页面销毁。
  原页面已卸载时沿用全局提示及重新读取；关闭 App 结束客户端等待。结果未知不自动重试。
- `entries_page/mod.rs` 继续声明查询和保存的响应式依赖；会话内部不新增 effect、定时轮询或另一套文章状态。

## 验证

- `cargo test -p rssr-app --locked`：保留原有 session 测试路径与断言，含旧请求、连续选择、未知结果、离页和关闭 App。
- 构建当前 Web 包后运行 `scripts/browser/rssr_workspace_acceptance.cjs`：真实服务器上的来源编辑、偏好保存、批量操作和切页。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`；Web release 构建覆盖 wasm 目标与优化器。

目录渲染、平台测量和页面展示继续使用各自现有模块；不要为了会话内部整理而改变它们的交互契约。
