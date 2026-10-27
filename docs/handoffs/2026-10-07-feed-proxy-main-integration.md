# PR #25 合入当前 main 的组合验收

- 日期：2026-10-07
- 作者 / Agent：Codex，Windows LAPTOP-H6JEOCF0
- 分支：`fix/feed-proxy-content-isolation`；[Draft PR #25](https://github.com/Develata/RSS-Reader/pull/25)
- 合并第一父提交：`e17a03e6d4a07c8f2fa3cd2998447e926c32c9ef`（原 PR #25 head）
- 合并第二父提交：`adcd39e92ce1b416c71df3542d64dae706accdb6`（fetch 核对后的 main，PR #26 squash commit）
- 组合 commit：本记录所在 merge commit；提交前 `commit: pending`
- 状态：`validated`（本地组合检查）；最终精确 head CI 回执写入 PR 说明
- tag / release：N/A；本轮未授权 v0.1.23，不 merge PR、不打 tag、不发布

## 工作摘要

按本轮明确授权，将当前 main 合入 PR #25 原分支，验证 `/feed-proxy` 隔离修复与原生出站安全修复的组合。使用既有独立 worktree，正常 merge/push，不 rebase 或 force push。旧 CI 37594770614 使用旧 main 的组合，不作为本次新组合通过证据。

## 影响范围

- 原生 infra：原样引入 PR #26 的图片 Referer、WebDAV 请求边界及其测试和证据。
- Web / Docker：保留 PR #25 的响应隔离、确定性浏览器验收与 CI job。
- 本轮额外修改仅本交接记录；未修改生产功能、测试断言、依赖、网络设置或权限。

## 关键变更

- 两批文件集无重叠，`git merge --no-ff --no-commit origin/main` 无冲突；验证完成后以一个双父 merge commit 连同本记录提交。
- `git diff --cached origin/main -- crates/rssr-infra` 为空；`git diff --cached e17a03e -- .github/workflows/ci.yml crates/rssr-web scripts/browser/feed_proxy_isolation.mjs` 为空，确认两批实现均原样保留。
- 未恢复主目录已删除的 `.agents/` / `.specify/`。独立 worktree 已有 spec-kit 技能说明仅针对 spec/tasks 流程，本次普通集成不启动该流程。

## 验证与验收

本轮构建与临时文件继续位于 E 盘独立 worktree 的 `target/`，日志在 `target/integration-main/evidence/`。本地 Cargo 使用锁定离线缓存，`TEMP`/`TMP`、`CARGO_TARGET_DIR` 指向本任务 target，dev/test debug info 为 0。

- `cargo fmt --all --check`：通过。
- `cargo test --locked --offline -p rssr-web`：20 passed、0 failed、1 ignored（专用浏览器 fixture server，由浏览器驱动单独执行）。
- `cargo test --locked --offline -p rssr-infra --test test_native_outbound_requests --test test_webdav_local_roundtrip`：14 + 1 passed、0 failed。
- `cargo clippy --locked --offline -p rssr-web -p rssr-infra --all-targets -- -D warnings`：通过。
- `cargo check --locked --offline -p rssr-app --target wasm32-unknown-unknown`：通过。
- `node scripts/browser/feed_proxy_isolation.mjs target/integration-main/evidence/browser`：通过；Windows Chrome 的无隔离对照、6 类顶层文档隔离、iframe/存储隔离及 RSS/Atom/Latin-1/HTML 发现兼容性均通过，报告为 `browser/run-NUE5zO/result.json`。沿用之前核对哈希的既有 Web bundle；PR #26 仅修改非 wasm 生产模块。新组合的 Web bundle 重建与浏览器验收由本次新 head CI 执行，不把旧 bundle 当作新构建。
- 本地 workspace 全量、Android check / APK / 实机、整套发布 UI：本轮未重跑；最终精确 head 的完整 CI 必须重新运行，包含六 crate、五主题、三个 wasm 契约、Android、双平台 runner、新 `feed-proxy-isolation` 和最终 `lint-and-test`。结果以 PR 中的新 run/head 回执为准，不重新运行旧 run 充数。
- `git diff --cached --check` 对引入 main 的历史证据 `docs/testing/evidence/2026-10-07-native-outbound-request-safety/targeted-tests.txt:84` 报告末尾多空行；这是 main 原有内容，保留原样，不扩大本批范围。新增记录单独检查空白。

## 结果

合入无冲突、无新增生产修改。提交/push 后跟踪该双父组合 commit 的完整 CI 至终态，并在 PR 描述记录精确 SHA、父提交、run URL 和结果。本记录不预先宣称尚未执行的远端检查通过。

## 风险与后续事项

- 304 边界保持：合成 fixture 只验证 production response builder/测试服务输出，未覆盖 resolve/fetch 全链；真实上游 304 无 `Location` 仍会错误映射为 502。本轮不修复、不声称这条真实路径通过。
- 之前公开 Atom smoke 被本机 fake-IP DNS 校验拒绝；本轮没有重新跑外网 smoke，也不将其写成通过。
- 本轮不新增安全修复或新功能，若组合暴露生产问题需先报告；不修改 GitHub 保护规则或发布设置。

## 给下一位 Agent 的备注

- worktree：`E:/gitclone/RSS-Reader/target/feed-proxy-isolation-20261007/worktree`。
- 主目录当前为 `fix/native-outbound-request-safety@6479ffba71d9118d9f31b8c840a8d7f303098665`；开始时记录 25 项无关状态（22 删除、2 修改、`.workbuddy/`），包含内容 SHA-256 与 staged diff 快照，见 `target/integration-main/evidence/unrelated-before.json`。提交前逐项比对通过：HEAD、25 项状态、staged diff、3 个现存文件 SHA-256 均不变；`unrelated-after.json` 为 `preserved: true`。
- PR24 worktree 未操作；无 reset/stash/checkout 覆盖主目录、无真实 RSS 数据库操作。
