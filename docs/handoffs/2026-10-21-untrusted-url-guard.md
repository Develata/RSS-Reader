# 不受信 URL 的解析上限：共享预检与 feed 解析前的地址扫描

- 日期：2026-10-21
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：claude/wonderful-engelbart-fdac91（未推送）
- 当前 HEAD：见本文件所在提交
- 相关 commit：本文件所在提交
- 相关 tag / release：N/A
- 状态：`validated`（独立评审结果见下）

## 工作摘要

`url` 2.5.8 处理每个 `..` 段时从序列化结果末尾反向找 `/`，在根路径上要扫过整个 authority；带长用户名、大量
`../` 的地址解析是平方级（本机：规模翻倍耗时约 10 倍）。另外 feed-rs 会把 `xml:base` 克隆进每个元素，长基地址乘
条目数也是平方级（1 MB 基地址 × 1000 条目约 9 s）。两者都发生在刷新时、本项目代码之前，阅读页的图片归一化已有点数
上限，其余入口没有。按 Develata 选定的方案：共享预检 + feed-rs 之前的地址扫描 + 上游议题草稿（提交前给 Develata 看）。

## 影响范围

- `crates/rssr-domain/src/url_guard.rs`（新）：`url_path_dots`、`untrusted_url_is_parseable`、
  `parse_untrusted_url`、`join_untrusted_url`；上限 64 个点（`?`/`#` 之前，含 `%2e`，忽略 TAB/LF/CR）、8 KiB。
- `crates/rssr-domain/src/validation.rs`：`parse_feed_url`（配置导入、OPML、WebDAV 拉取、添加订阅）先预检，
  错误信息不再回显超长原文。
- `crates/rssr-infra/src/feed_url_guard.rs`（新）+ `feed_normalization.rs`：feed-rs 解析前的扫描。
- `crates/rssr-infra/src/feed_normalization.rs`：条目与站点链接改用 `parse_untrusted_url`。
- `crates/rssr-infra/src/subscription_probe/html.rs`：`<base href>` 与 `<link rel=alternate>` 改用
  `join_untrusted_url`，页面声明的订阅地址最多采纳 64 个。
- `crates/rssr-application/src/subscription_discovery.rs`：用户输入的订阅地址先预检。
- `crates/rssr-web/src/proxy.rs` + `Cargo.toml`：代理的 `?url=` 与重定向 `Location` 先预检
  （`rssr-web` 新增对纯领域 crate `rssr-domain` 的依赖）。
- `crates/rssr-infra/src/html/live_display.rs`：改用共享的 `url_path_dots`。
- `crates/rssr-infra/Cargo.toml`：新增 `feed-quick-xml`（即 feed-rs 用的 quick-xml 0.41.0，同版本同特性，
  Cargo 合并为同一份，`Cargo.lock` 只多两行依赖引用）。

## 关键变更

- **feed 扫描**（`check_feed_urls`）：与 feed-rs 相同的判定（前 8 KiB 里第一个 `<` 或 `{`）、相同的 quick-xml
  分词配置、相同的文本/CDATA/实体合并与属性反转义；不做命名空间解析（那本身就是要防的平方级路径），自己数绑定。
  - 结构：嵌套 ≤ 1024 层（quick-xml 的 `u16` 深度计数远未溢出）、同时在作用域内的命名空间声明 ≤ 256、
    单个元素的属性错误（重复等）≤ 16——这三项同时挡住 feed-rs 自身的命名空间解析与重复属性回查的平方级。
  - 地址：只检查 feed-rs 会当作地址解析的位置：`link`/`icon`/`logo`/`url` 元素的文本，`href`/`url`/`src`/`uri`
    属性，`xml:base`（基地址长度按百分号编码 3 倍估算，绝对地址重新计、相对地址与上级累加，不超过 512 字节）；
    JSON Feed 只检查 feed-rs 读取的 `home_page_url`、`feed_url`、条目 `url`/`external_url`、附件 `url`，
    无法整体解析的 JSON 直接拒绝（否则深层扩展字段可以让检查失效而 feed-rs 照常解析）。
  - 越界就整个 feed 拒绝（错误“拒绝解析 feed：…”，与解析失败同样处理），不做部分解析；正文省略号等不受影响。
    XML 格式本身不对时不在这里报错，交给 feed-rs（同一个分词器，会在同一处失败）。
- **共享预检**：上述各入口统一走 `parse_untrusted_url`/`join_untrusted_url`；`join` 时基地址也计入 8 KiB 上限。

## 验证与验收

- 真实 feed（26 个：本机订阅 16 个 + 10 个常见公开 feed，含 Atom `xml:base`、JSON Feed、1.2 MB 大 feed）：feed-rs 能解析的
  23 个全部照常解析，0 个被误拒；另 3 个 feed-rs 本身就解析失败。扫描开销每个 feed 毫秒级。
- 新增测试：`rssr-domain` 预检单元测试；`feed_url_guard` 单元测试（普通 RSS/Atom/JSON 通过；链接文本、属性、字符引用、
  CDATA 拆分、`%2\ne` 拆分、长/叠加 `xml:base`、JSON url 字段被拒）；`tests/test_feed_url_guard.rs` 压力契约
  （4 MB 点段链接、1 MB `xml:base` × 1000 条目、JSON 点段在解析前被拒，debug 下合计 0.24 s；普通 feed 解析结果不变）；
  订阅发现的数量上限与 `<base href>` 预检。
- `cargo fmt --all --check`、`cargo clippy -j 1 --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test -j 1 --workspace --locked`：452 通过，0 失败（第 1 轮评审修复后重跑；真实 feed 复测仍是 23 个照常、0 误拒）。
- 独立评审（Codex）第 1 轮：4 个 BLOCKER（JSON 深度绕过、命名空间解析平方级、重复属性平方级、深度计数溢出）+
  2 个 SHOULD_FIX（`xml:base` 长度模型、JSON 键后缀过宽），已按上文修复并加单元测试。XML 侧仍只按本地名判断元素与属性，
  比 feed-rs 按命名空间区分宽，属于多检查的一侧。
- `cargo check -j 1 -p rssr-app --target wasm32-unknown-unknown --locked`：通过。

## 结果

- 可合并。只拒绝含异常地址的 feed 与输入；正常 feed、订阅与导入不受影响。

## 风险与后续事项

- 重定向 `Location` 在 reqwest/tower-http 内部解析，本项目只能管代理自己的一次 `join`；单个头的大小受 HTTP 头上限约束。
- `url`/IDNA 在任意 Unicode 主机名上的最坏复杂度未证明。
- 上游：rust-url（点段反扫）与 feed-rs（`xml:base` 克隆放大）的议题草稿已写好，等 Develata 确认后提交。
