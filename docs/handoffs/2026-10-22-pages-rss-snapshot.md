# Pages 预抓取真实 RSS 文章

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 当前 HEAD（实现前）：`ddf4837`
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`（本地通过，分支 CI 待运行；未部署）

## 工作摘要

用户明确要求 Pages 提前准备真实 RSS 内容，由 CI 构建时抓取，并指定个人主页的每日数学和陶哲轩博客。核实原实现只打包合成测试夹具后，改为预抓取真实文章快照。此决定替换设计中的“合成样例”；阶段 D 的内存 SQLite、删除旧存储及刷新重置仍待完成。

## 影响范围

- 模块：`rssr-app::demo`、共享快照格式、infra 的原生构建工具 example。
- 平台：Pages 演示；正式 Web 的后端与用户资料库未改。
- workflow / 文档：Pages 构建前抓取、来源存档、真实内容 smoke、中英文入口、部署与架构文档。
- worktree：继续使用 `target/audit-20261009/worktree`，其他检出未修改。

## 关键变更

- `assets/pages-demo/sources.json` 指定 [Develata 每日数学](https://develata.me/rss/daily-math.xml) 5 篇、[Terence Tao 博客](https://terrytao.wordpress.com/feed/) 1 篇。Tao 的[转载说明](https://terrytao.wordpress.com/about/)允许附来源转载合理篇幅，例如单篇文章；快照保留原文链接与 RSS 提供的作者，不将内容改授为项目 MIT 许可。
- `cargo run --locked -p rssr-infra --example pages_snapshot` 复用产品 RSS/Atom 解析器，抓取 HTTPS 源，按时间选取并去重；保留正文、发布时间和原文地址，记录抓取时间、最终 URL 与原始 RSS SHA-256。
- 请求总超时 40 秒、最多 3 次跳转；每个响应及完整快照分别限 2 MiB。源失败则失败，所有源完成前不替换现有快照。没有后台或定时抓取。
- 已提交的快照供离线构建；Pages CI 在构建 WASM 之前重新抓取并保留该次快照 artifact 7 天。分支手动运行使用 `/RSS-Reader` 子路径并跳过部署。
- 演示通过原有 `upsert_entries` 与 `open_with_initial_state` 载入真实数据，已有浏览器数据继续保留。测试合成夹具不改。正式 Web 不编入这些快照。
- 浏览器验收检查真实标题、正文、两个来源，以及全部条目和原文链接进入真实资料库。正文外部图片单独记录，其他跨源请求仍不允许。

## 验证与验收

- Windows 实际抓取：2 个源、6 篇正文，`snapshot.json` 356385 bytes；抓取时间 `2026-10-22 20:19:41.8132114 +00:00:00`，SHA-256 `50c1339647cfcf79d78c3b04b8c72c7207989f9073a7d4e013725ed478493d29`。
- `cargo test --locked -p rssr-infra --example pages_snapshot`：2/2 通过（真实解析路径上的排序/去重/元数据与空源/无效上限）。
- `cargo clippy --locked -p rssr-infra --example pages_snapshot -- -D warnings`：通过。
- `cargo clippy --locked -p rssr-app --target wasm32-unknown-unknown --features pages-demo -- -D warnings`：通过。
- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown`：正式 Web 通过；演示 check 也通过。
- 真实 HTTPS 404 反例：使用另一个输出路径，工具返回 1，原输出的 SHA-256 完全不变。
- 固定 dx 0.7.9：`dx build --platform web --package rssr-app --release --locked --features pages-demo --base-path /RSS-Reader --debug-symbols false` 通过，构建标识 `pages-rss-20261022`。
- Docker Ubuntu 24.04 / Chrome 156.0.8078.47 / Node 22：`PAGES_BASE_PATH=/RSS-Reader bash scripts/test_pages.sh` 通过。真实阅读器、404 深链接、子路径导航、收藏重载、两个来源与 6 篇条目/原文、添加及刷新被拒全部通过；未发出跨源请求，`externalImages` 为空。
- `cargo fmt --all --check`、`actionlint .github/workflows/pages.yml`、`node --check scripts/browser/pages_smoke.mjs`、`git diff --check`：通过。
- 本地构建与浏览器原始证据：`target/handoff/pages-feeds/`（ignored）；可追溯快照本身入库。

## 结果

预抓取、打包与本地真实浏览器路径完成。公开 Pages 尚未部署；没有创建版本 tag 或 Release。分支 CI 结果在完成后补入。

## 风险与后续事项

- 初次载入才使用当前快照；已有 BrowserStore 内容不会因部署更新而覆盖。可用新浏览器配置查看新内容；阶段 D 才改为内存库、重载重置。
- 外部图片仍可能联网；不声称完整离线显示或已验收所有数学公式的视觉排版。Develata 的 RSS 未提供作者字段时保留缺失，不虚构作者。
- 上游内容变化会改变快照；固定回归仍用合成夹具，公开 Pages smoke 对本次实际抓取结果断言。
- 此次不把阶段 D 或整个 v0.2 发布门禁标为通过。

## 给下一位 Agent 的备注

- 先看 `docs/deployment/github-pages.md` 和 `assets/pages-demo/README.md`。快照格式不依赖 BrowserStore，阶段 D 可继续复用同一份内容。
- 后续修改源清单后重新运行抓取工具、构建和 Pages smoke；不要覆盖其他检出或用户浏览器数据。
