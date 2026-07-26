# 文章页 presenter memo 依赖收窄 + 服务端会话过期恢复路径

- 日期：2026-07-26
- 作者 / Agent：Claude (math-architect)
- 分支：main
- 当前 HEAD：pending（本记录随同一批改动提交）
- 相关 commit：`6ffab09`、`ecb8ad2`，以及本次复核修正（pending）
- 相关 tag / release：`v0.1.10` 之后，尚未发布
- 状态：`validated`

## 工作摘要

接上一轮列出的待办，按当时给的顺序做前两项：把文章页 presenter 的 memo 依赖收窄
（上一轮复核指出这是比「降低单次重建成本」更值钱的杠杆），以及修掉服务端会话过期时
把用户引去创建本地浏览器凭据的缺陷。两项都经过一次独立正确性复核，复核意见已逐条落地。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/entries_page/{presenter,mod,session}.rs`
  - `crates/rssr-app/src/{web_auth.rs,web_auth_browser.rs}`、`ui/shell.rs`
  - `crates/rssr-app/Cargo.toml`（wasm 侧新增 `urlencoding`）
- 平台：
  - 文章页改动影响全平台；认证恢复路径只影响 **Web 部署态**（`rssr-web` 前面挂门禁 cookie 的场景）
- 额外影响：无迁移、无 workflow 变更

## 关键变更

### 1. presenter memo 依赖收窄

Dioxus 的信号订阅粒度是**整个信号**：读 `EntriesPageState` 的任何一个字段都会订阅整份状态，
于是 `SetStatus`（每次切换已读/收藏都会附带发一条，与 `PatchEntryFlags` 是两次独立写入）、
`SetControlsHidden` 这类与分组毫无关系的 intent 也会让 presenter 失效并重建两棵分组树。

新增 `EntriesPresenterInput`，只含 presenter 真正读的字段，改用 memo 链：
投影 memo 每次状态变化都重算，但它的值没变时 presenter memo 不再重算。
实测效果：一次读/收藏切换从「建树 2 次」降到「建树 1 次」。

**memo 链成立的依据（已核对 dioxus-signals 0.7.9 源码，不是推测）**：

- `memo.rs` 的 `recompute()` 只在 `new_value != *peak` 时才 `set`，值不变则不写上游信号、
  不通知订阅者。
- `Memo` 的 `Readable::try_read_unchecked` 订阅的是 **memo 自己那份 `SignalData`** 的
  `subscribers`，源码注释明确写了「读 inner generational box 而不是 signal，以便更精细地
  控制订阅时机」——订阅不会透传到上游信号。
- 顺序也安全：投影 memo 的 recompute 任务与文章页 scope 的 `ScopeOrder` 相等，
  而调度器在 `Ordering::Equal` 时优先 `Work::PollTask`，因此投影一定先于组件重跑完成，
  不会读到滞后一帧的值。

### 2. 服务端会话过期不再回落到本地浏览器凭据

`auth_state()` 在回落到 `local_auth_state()` 前有一道回环主机检查，但
`use_authenticated_shell_bus` 的探测失败分支**直接**调用了 `local_auth_state()`，绕过了它。
`/session-probe` 挂在 `require_auth` 之后，服务端会话一过期探测就不会回 204，于是正式部署上
用户会看到「初始化 Web 登录」，被引导去创建一组与服务端登录毫无关系的本地凭据
（还会在 `localStorage` 留下永远用不上的凭据）。

`verify_server_gate() -> bool` 换成 `probe_server_gate() -> ServerGateProbe`，四种结果分别处理：

| 结果 | 处理 | 理由 |
|---|---|---|
| `Authenticated` | 置为已认证 | — |
| `SessionExpired`（收到非 204 且非 5xx 应答） | 整页跳服务端 `/login`，带 `next` | 服务端登录是这类部署唯一的恢复路径 |
| `Unreachable`（传输失败或 5xx） | **放行渲染**，打 warn | 见下 |
| `Absent` | 走完整 `auth_state()`（含回环主机检查） | 只有没有服务端门禁时才轮到本地判定 |

`Unreachable` 之所以放行而不是跳转：Web 端数据在 `localStorage`，断网时页面本来还能渲染，
跳 `/login` 只会得到一个加载失败的页面，把一个还能用的会话直接毁掉。放行是安全的——
客户端这道门禁**不是安全边界**，服务端 `require_auth` 对每个请求都会重新校验。

## 复核意见与落地情况

一次独立正确性复核（子代理）确认：memo 链成立、投影字段完整、`feeds` 处理等价、
`Arc::make_mut` 不会改到 memo 缓存里那份值、原生路径无回归。以下为它提出并已修的问题：

- **`Unreachable` 一并跳 `/login` 是真错**，而且是这条路径**最常见**的触发原因：
  会话 cookie 与门禁 cookie 的 `Max-Age` 都等于 `session_ttl`、同时签发同时到期，
  「门禁还在而会话已失效」的窗口实践中几乎为零，真正会走到失败分支的是网络错误。
  已在 `ecb8ad2` 改正。另外若 `redirect_to_server_login()` 因拿不到 window/origin 提前返回，
  用户会永久卡在没有出口的 spinner 上——这也是不该无条件跳转的原因之一。
- **5xx 被当成会话过期**：应用自身 5xx 或反代 502/504 都会被判为过期而跳 `/login`，
  那个地址同样打不开。已改为归类到 `Unreachable`。
- **跳转丢掉当前路由**：服务端自己的 302 会保留 `next`，客户端探测却把用户扔回首页。
  已带上 URL-encoded 的 `pathname + search`（服务端 `sanitize_next` 仍会拒掉 `//` 与外域）。
- **`browser_origin()` 为 `None` 被归类成 `Unreachable`**：那是拿不到 window，不是网络故障，
  已改为 `Absent`。
- **注释里「很便宜」的说法不成立**：`session.snapshot()` 每次状态变化都深拷贝整份状态，
  `presenter_input()` 又克隆整份投影。已改为 `session.with_state(..)` 与
  `presenter_input.with(..)` 两处借用；`from_input` 里对 `entries` 的死克隆也去掉了。
  文档注释改为如实说明代价（N 次 `Arc` 指针拷贝 + 一次 `feeds` 深拷贝）。
- **不变量搬离了使用点**：`feed_id` 已放回投影，「按订阅浏览 ⇒ 无来源筛选项」重新由
  `from_input` 强制执行，而不是只剩一句注释。
- **测试名断言了假命题**：`archived_count` 并不影响分组（presenter 只是原样透传），
  测试已改名为「投影包含/排除哪些字段」，并补上 `feeds` 与按订阅路由两处此前无覆盖的分支。
- **缺交接记录**（仓库硬规则）：即本文件。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过（0 输出）
- `cargo test --workspace`：通过（33 个测试二进制，0 failed）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-infra --target wasm32-unknown-unknown --all-targets`：通过
- 新增/调整测试（`rssr-app`，presenter 共 5 条）：
  - `presenter_input_excludes_state_that_does_not_change_rendering`
  - `presenter_input_includes_every_field_that_changes_rendering`（含 `feeds`）
  - `browsing_a_single_feed_hides_the_source_filter`

### 手工验收

- 未执行。建议回归：
  - 文章页大量文章时连续切换已读/收藏的流畅度
  - **Web 部署态**：服务端会话过期后应跳到 `/login` 且回到原页面；断网时页面应仍能渲染，
    不应出现「初始化 Web 登录」这类本地凭据引导

## 风险与后续事项

1. **分组树仍会在读/收藏切换时重建一次**：`PatchEntryFlags` 改的是条目自身的标记，而分组树
   持有这些条目。彻底消除需要让树不再携带带标记的条目对象（payload 改成索引或 id，
   卡片另行解析标记），涉及 `groups.rs` / `presenter.rs` / `cards.rs` 与渲染循环，
   是一次独立重构，本轮**未做**。
2. **列表 SQL 分页**仍未做，依旧需要先设计分组聚合接口。
3. 全量分组树的叶子只为 `find_active_*_anchors` 扫 id 用，可改成按索引区间推导；
   Source 模式还会构建随即丢弃的 date 层。
4. 页面层其余已确认未改的问题见 `2026-07-26-audit-remediation-round-2.md`。

## 给下一位 Agent 的备注

- 往 `EntriesPresenterInput` 加字段前先想清楚：加进去意味着该字段变化会重建两棵分组树。
  两条投影测试就是这条边界的守卫。
- 认证恢复路径的判定集中在 `probe_server_gate()`；改动前先确认新分支在**断网**与
  **5xx** 两种情况下都不会把用户送去一个打不开的地址。
