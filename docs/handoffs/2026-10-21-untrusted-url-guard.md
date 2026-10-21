# 不受信 URL 的解析上限：`url` 根上修复、feed 结构预检与自有入口预检

- 日期：2026-10-21
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：claude/wonderful-engelbart-fdac91（未推送）
- 当前 HEAD：见本文件所在提交
- 相关 commit：本文件所在提交
- 相关 tag / release：N/A
- 状态：`validated`（独立评审结果见下）

## 工作摘要

`url` 2.5.8 处理每个 `..` 段时从序列化结果末尾反向找 `/`，在根路径上要扫过整个 authority；带长用户名、大量
`../` 的地址解析是平方级（本机：规模翻倍耗时约 4–10 倍）。feed-rs 解析文章链接、重定向、本项目自己的各个入口都会
走到这里。另外 feed-rs 把 `xml:base` 克隆进每个元素（1 MB 基地址 × 2000 条目约 9 s），quick-xml 的命名空间解析
与重复属性检查也随结构放大。

最初的方案是在 feed-rs 之前复刻它的读法、扫描会被当作地址的字符串，三轮独立评审每轮都找到新的读法分叉（未绑定前缀的
回退、吞错后继续解析、解码器时机、原始与反转义值），不收敛。Develata 选定方案 B：

1. **在根上修 `url`**：`vendor/url` 是 crates.io 的 `url` 2.5.8 原样副本，只改 `Parser::last_slash_can_be_removed`
   一个函数（反向查找限制在路径内，结果不变），经 `[patch.crates-io]` 接入，工作区 `exclude` 掉。说明与移除条件见
   `vendor/url/PATCH.md`。
2. **feed 结构预检**（`feed_structure_guard.rs`）：只在字节层面数结构上限，不解码文本、不判断哪些字符串是地址。
3. **自有入口预检**（`rssr-domain::url_guard`）：本项目自己把远端字符串交给 `url` 的地方统一限长与限点数，作为纵深防御。
4. 上游议题草稿（`docs/upstream-drafts/`），提交前给 Develata 看。

## 影响范围

- `Cargo.toml`：`[patch.crates-io] url = { path = "vendor/url" }`，`workspace.exclude = ["vendor/url"]`；
  `Cargo.lock` 里 `url` 不再指向 registry。
- `vendor/url/`（新）：`url` 2.5.8 副本 + `PATCH.md`。
- `crates/rssr-domain/src/url_guard.rs`（新）：`url_path_dots`、`untrusted_url_is_parseable`、`parse_untrusted_url`、
  `join_untrusted_url`（上限 64 个点、8 KiB；`join` 时基地址也计入长度），以及确认补丁生效的 canary 测试。
- `crates/rssr-domain/src/validation.rs`：`parse_feed_url`（配置导入、OPML、WebDAV 拉取、添加订阅）先预检。
- `crates/rssr-infra/src/feed_structure_guard.rs`（新）+ `feed_normalization.rs`：feed-rs 解析前的结构预检；
  条目与站点链接改用 `parse_untrusted_url`。
- `crates/rssr-infra/src/subscription_probe/html.rs`：`<base href>` 与 `<link rel=alternate>` 走 `join_untrusted_url`，
  页面声明的订阅地址最多采纳 64 个。
- `crates/rssr-application/src/subscription_discovery.rs`：用户输入的订阅地址先预检。
- `crates/rssr-web/src/proxy.rs` + `Cargo.toml`：代理的 `?url=` 与重定向 `Location` 先预检（新增对纯领域 crate
  `rssr-domain` 的依赖）。
- `crates/rssr-infra/src/html/live_display.rs`：改用共享的 `url_path_dots`。
- `crates/rssr-infra/Cargo.toml`：新增 `feed-quick-xml`（即 feed-rs 用的 quick-xml 0.41.0，同版本同特性，合并为同一份）。

## 关键变更

- **结构预检**：与 feed-rs 相同的 XML/JSON 判定（前 8 KiB 里第一个 `<` 或 `{`，JSON 直接放行）与相同的分词配置；
  不做命名空间解析（那正是要防的路径），对 quick-xml 的读取错误能继续就继续（与 feed-rs 吞错后继续读一致）。上限：
  嵌套 ≤ 1024 层；作用域内命名空间声明 ≤ 256 个（像 NsReader 一样连重复声明一起数）、原始值 ≤ 256 字节；单个元素的
  属性错误（自己用哈希集合判重）≤ 16；作用域内 `xml:base` 原始字节和 ≤ 256（ASCII scheme 开头的绝对地址重新计），
  并且每个元素按当前基地址字节数累计的克隆总量 ≤ 16 × 输入长度 + 1 MiB。所有计数都取“可能发生”的最大值（重复的、
  正文子树里的 `xml:base` 也计），只会多拒绝、不会漏；越界就整个 feed 拒绝（“拒绝解析 feed：…”）。

## 验证与验收

- `vendor/url` 与 registry 原版只差一个 hunk；在隔离副本上跑 `url` 自带的测试：66 个单元测试 + WPT URL 测试数据全部通过。
- 点段复现（release，n = 65536，262 KB 输入）：原版 252 ms，补丁后 2.4 ms，与短 authority 的对照组同级，线性。
- canary：`rssr-domain` 的 `dot_segments_parse_in_linear_time`（256 Ki 个 `../`）debug 下 0.13 s；补丁丢失时要几十秒。
- 真实 feed（26 个：本机订阅 16 个 + 10 个常见公开 feed，含 Atom `xml:base`、JSON Feed、1.2 MB 大 feed）：feed-rs 能解析的
  23 个全部照常解析，0 误拒；3 个 feed-rs 本身就解析失败。
- 压力契约 `tests/test_feed_structure_guard.rs`：4 MB 点段链接 feed 照常解析（地址超 8 KiB 不进入条目）、1 MB `xml:base`
  × 1000 条目、32000 层命名空间、1 MiB 命名空间值 × 10 万元素、3.2 万重复属性都在解析前被拒，debug 下合计 0.91 s。
- `cargo fmt --all --check`、`cargo clippy -j 1 --workspace --all-targets --locked -- -D warnings`：通过（`vendor/url` 不是
  工作区成员，不参与 fmt/clippy/test，内容与 registry 原版逐字一致，只差补丁那一处）。
- `cargo test -j 1 --workspace --locked`：452 通过，0 失败。
- `cargo check -j 1 -p rssr-app --target wasm32-unknown-unknown --locked`：通过（wasm 构建同样使用补丁后的 `url`）。

## 独立评审（Codex，gpt-6-astra，effort high）

- 地址扫描方案（1fd6421 → 539884a）共 3 轮，未通过；每轮都是预检与 feed-rs 读法的新分叉，记录在 git 历史的本文件旧版。
- 方案 B（本提交）：见下方记录。

## 结果

- 可合并（随方案 B 的独立评审结论）。正常 feed、订阅与导入不受影响；只拒绝结构异常的 feed 与超长/点段过多的输入。

## 风险与后续事项

- `vendor/url` 是一个需要跟进的本地补丁：`url` 有安全更新时要同步；上游修复发布后按 `PATCH.md` 移除。
- `url`/IDNA 在任意 Unicode 主机名上的最坏复杂度未证明。
- JSON Feed 不做结构预检：feed-rs 会把顶层作者克隆进每个缺作者的条目、并线性去重作者，巨大的作者名或大量不同作者仍有
  放大（feed-rs 的既有行为，待上游）。
- 仍直接解析、输入可信度较高的位置：WebDAV 端点与远程路径（用户设置）、Web 端 `x-rssr-final-url`（本项目代理生成）、
  数据库/BrowserStore 里已序列化的地址（写入时已过预检）。
- 上游：rust-url（点段反扫）与 feed-rs（`xml:base` 克隆放大）议题草稿在 `docs/upstream-drafts/`，等 Develata 确认后提交。
