# 主线推送前审查、空态修复与 Pages 合并

- 日期：2026-09-30
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD：`3236bc2`（Pages 合并提交；本记录随其后的修复提交）
- 相关 commit：`3236bc2` 及本记录所在的修复提交；审查起点为 `b771fcc`
- 相关 tag / release：N/A；本轮不创建 tag
- 状态：`validated`（本地验证完成；远端 CI 待复核）

## 工作摘要

用户要求 review 并修复，完成后推送触发 CI；CI 完成后由用户提醒复核，再决定新 tag。审查本地尚未推送的七笔提交，并合并远端三笔 Pages 演示提交。发现并修复一项 P2：首次查询尚未完成或读取失败时，文章页错误显示“还没有订阅”。

## 影响范围

- 修复模块：`rssr-app` 的 Entries facade / 页面渲染。
- 平台：共享 UI 影响 Web、desktop、Android；本轮交互验收使用真实 Chromium，另完成原生与 Android ARM64 编译检查。
- 合并范围：保留 `b771fcc` 之前的七笔本地提交，以及远端 `a275b0e`、`3e0d199`、`da3d104`；使用合并提交，不重写双方提交。
- 文档与验收：前端命令契约、发布回归清单、既有 small viewport 脚本和本记录。

## 关键变更

### P2：查询未完成时误报空库

- 根因：页面直接以初始的空 `entries` / `feeds` 渲染空态，未区分 `entries_loaded == false`。
- 真实复现：用另一标签页短暂持有实际 Web Lock，已有文章的页面同时显示“正在加载文章列表…”与“还没有订阅”；模拟初次存储读取失败时，同样在错误信息下显示空库提示。
- 修复：只有首次文章查询成功后才渲染空结果说明；加载状态与错误继续由既有 live region 呈现。
- 回归：新增加载等待、锁释放后恢复、初次读取失败、恢复读取后保留原有文章四条断言；沿用原有真实浏览器与真实 WASM 入口。
- 修复前产物运行新断言明确失败：`pending entries do not claim the library is empty`，证据中 `ready=false`、`empty=true`。

### 远端兼容性

- 推送前发现本地与远端分叉，获取并合并 `origin/main` 的 Pages 演示能力；自动合并无冲突，逐项核对 App shell 与静态提示的合并结果。
- 审查覆盖未读导航与快捷键、成功 / 错误反馈、筛选清除与持久化、主题旧 CSS 识别、Pages 初始化与网络能力限制。
- Pages 的策略保持在现有 demo / Web host 层，BrowserStore 使用既有通用初始化接口；本轮修复没有增加平台业务分支。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test --workspace --locked`：312 项通过、0 失败、2 项既有忽略。
- `cargo clippy -p rssr-app --target wasm32-unknown-unknown --locked -- -D warnings`：通过。
- `cargo clippy -p rssr-app --target wasm32-unknown-unknown --features pages-demo --locked -- -D warnings`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：指定现有 NDK 27.3.13750724 / API 34 的 compiler、ar 与 linker 后通过。
- `dx build --package rssr-app --platform web --locked`：通过，使用本次产物运行五主题回归。
- `node --check scripts/browser/rssr_small_viewport_assertions.mjs`、`node --check scripts/browser/pages_smoke.mjs`、`bash -n scripts/test_pages.sh`：通过。
- `/home/deve/go/bin/actionlint -verbose`：四个 workflow 均通过；本机缺少可选 pyflakes，不计该规则为已验收。

### 实际浏览器验收

- `bash scripts/run_static_web_small_viewport_smoke.sh --skip-build --port <port> --preset <key> --chrome-bin /home/deve/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome --log-dir target/review-fix-validation/smoke-<theme>`：默认、Amethyst、Atlas、Midnight、Newsprint 各 147 条断言通过，合计 735 条，0 console error。默认主题省略 `--preset`；端口依次为 8141–8145。
- `bash scripts/run_wasm_contract_harness.sh wasm_subscription_contract_harness wasm_refresh_contract_harness wasm_config_exchange_contract_harness`：真实浏览器通过 subscription 19、refresh 27、config exchange 3，合计 49 项，包含合并后的初始数据写入保护。
- wasm 工具使用本机已有的 0.2.126 runner 与匹配的 Chrome / ChromeDriver 151.0.7922.34。首次自动启动驱动在测试加载前 connection reset；与[已有环境记录](./2026-09-22-ci-matrix-actions-modernization.md)一致。先启动同一 driver，等待 `/status` ready，再设置 `CHROMEDRIVER_REMOTE` 后通过；没有用 stub 替代浏览器，也没有修改正式执行器。
- `dx build --platform web --package rssr-app --release --locked --features pages-demo --base-path /RSS-Reader --debug-symbols false`、`python3 scripts/prepare_pages.py target/dx/rssr-app/release/web/public`、`PAGES_BASE_PATH=/RSS-Reader bash scripts/test_pages.sh`：全部通过。使用本次 release bundle 验证真实 Reader、404 深链接、子路径导航、重载后收藏状态保留、添加 / 刷新被阻断且没有 feed 网络请求。

## 结果

- 本地修复、五主题、浏览器存储契约与 Pages 验收均已通过，未发现其他阻断本次推送的问题。
- 远端合并独立提交为 `3236bc2`；本记录与空态修复同批提交，按用户授权推送 main 触发 CI。工作流结果需按本记录所在提交的 SHA 匹配，不能以旧 run 的成功状态代替。
- CI 结果待远端运行后复核；没有创建新 tag 或执行版本发布。

## 风险与后续事项

- 本机 dx 为 0.7.10，项目 / CI 固定 Dioxus 与 dx 0.7.9；本地构建仍显示既有版本差异提示。CI 需要在其固定工具版本上再次验证。
- 未执行 desktop / Android 原生 UI、Windows / macOS 实机、真实屏幕阅读器或安装包验收；不能以编译与 Web 结果替代这些环境。
- 本地 wasm 契约的驱动就绪适配只验证了实际浏览器契约，自动驱动启动仍由远端 CI 验证。

## 给下一位 Agent 的备注

- 本地证据在 `target/review-fix-validation/`：原始复现 JSON / 截图、修复前预期失败、Rust 日志、五主题断言、两次 wasm 驱动日志与 Pages 验收产物。
- 用户约定是本轮 push CI，待其提醒后检查对应提交的 CI / Pages 结果，再讨论和准备 tag；不要提前打 tag。
