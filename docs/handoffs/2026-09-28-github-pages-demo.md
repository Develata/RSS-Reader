# GitHub Pages 接入补丁

- 日期：2026-09-28
- 作者 / Agent：Codex
- 分支：main
- 基础 HEAD：dbe68e11fdbf93769bfa876831113a17928108ab
- 相关 commit：本记录所属功能提交；远端验收结果另行记录
- 相关 tag / release：v0.1.20 发布记录
- 状态：用户已授权提交、推送；远端 CI / 部署待验收

## 工作摘要

为现有 Dioxus Web bundle 增加 Pages 发布路径，复用原 UI 与 reader-demo 样例，
明确限制纯静态环境网络能力。先交付 unified diff；2026-09-28 用户授权将补丁提交到仓库并触发 CI / Pages。Pages environment 的 main 分支限制由用户预先配置。

## 影响范围

- 模块：rssr-app Web 宿主、BrowserStore 初始化、workflow、共享 CLI 安装 actions、验收脚本、部署文档。
- 平台：GitHub Pages / Web；默认关闭 pages-demo，原生与普通 Web 保持既有行为。

## 关键变更

- pages.yml 固定 dx 0.7.9，main 部署、PR 验收；最小化权限、OIDC 与 github-pages environment。
- base path 由 configure-pages 提供；准备脚本复制真实入口为 404.html。
- pages-demo 编入已有四份 JSON，BrowserStore 在 Web Lock 内判定是否全新并原子发布。
- 网络能力在 Web Host Port 入口阻断；UI 使用既有 StatusBanner 显示公开演示说明。
- 增加真实 WASM bundle 的子路径 / 404 / 收藏持久化 / 刷新降级 CDP 验收。
- 增加 BrowserStore 初始状态的并发与已有数据保护契约。

## 验证与验收

- `cargo fmt --all --check`：通过。
- `git diff --check`：通过。
- `actionlint -shellcheck='' .github/workflows/pages.yml`：通过；未执行 shellcheck。
- `bash -n scripts/test_pages.sh`、`node --check scripts/browser/pages_smoke.mjs`：通过。
- `python3 -m py_compile scripts/prepare_pages.py`：通过。
- 临时打包 fixture 检查：404 与 index 字节一致、子路径资源 200、直达路由 404 且入口内容一致，通过；这不是 UI 验收。
- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown --features pages-demo`：通过。
- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown`：通过。
- `cargo check --locked -p rssr-infra --target wasm32-unknown-unknown --test wasm_subscription_contract_harness`：通过（仅编译测试，未在浏览器执行）。
- 编译使用 Rust stable 1.98.1、`CARGO_TARGET_DIR=/tmp/rssr-pages-target` 和 `CARGO_BUILD_JOBS=2`；首次默认目录编译因依赖 archive 的文件映射错误失败，切换临时目录重试通过。
- `dx build --platform web --package rssr-app --release --locked --features pages-demo --base-path /RSS-Reader --debug-symbols false`：真实 release bundle 构建通过（dx 0.7.9，约 432 秒）；入口使用 `/RSS-Reader/assets/`，打包后的 404.html 与 index.html 字节一致。
- 本地 dx 下载工具时遇到环境证书 UnknownIssuer；使用 curl 下载同版本 esbuild 0.27.3 / binaryen 129（核对官方 checksum），将它们与 binstall 安装的 wasm-bindgen 0.2.126 预置于 DX_HOME 后完成构建。未关闭 TLS 校验，未将本地环境处理写入工作流。
- 并发启动两份 `scripts/test_pages.sh`：Chrome 均在进入应用前因 `socket() failed: Operation not permitted` 退出；CDP 功能与并发验收未通过，须由 GitHub runner 执行。失败日志得到保留。
- `git apply --cached --check`（独立 index，基于原始 HEAD）：通过。
- Pages 实际部署：未执行。

## 结果

补丁可供审查与应用；最终发布前必须让 Pages build 和原有 wasm 契约 CI 通过。

## 风险与后续事项

- 深链接首次返回 HTTP 404；GitHub Pages 不提供任意路径的 200 rewrite。
- localStorage 仍按 origin 共享，路径不隔离；不覆盖该 origin 已有 RSSR 数据。
- 初始化持久化失败可保留已提交空库，不以重新 seed 掩盖错误。
- 公开演示无服务端认证、feed 代理；不能当作完整线上 RSS 服务。

## 给下一位 Agent 的备注

先读 docs/deployment/github-pages.md。不要复制另一套 HTML UI，不要发布回归 server 的
认证/seed helper；不要在全局 Dioxus.toml 写死项目 base path。

## 第二轮 review（并发、性能与模块边界）

- build 按 ref 取消旧任务；deploy 单独串行且不取消进行中的发布，获得部署槽后检查 main HEAD，避免队列非 FIFO 导致旧产物回滚。HEAD 检查与部署不是原子事务，后续 main 构建仍负责收敛最新版本。
- PR 不上传 Pages 发布产物；Cargo 缓存仅 main 写入、Pages key 独立；删除重复 fmt，文档变更跳过 Pages 构建。原有 CI 与 Pages 各自验收，不声称存在跨 workflow 发布门禁。
- 共享 CLI 安装 actions 优先 cargo binstall，仅允许 crate-meta-data 策略；固定 installer 版本与 action SHA；source fallback 需显式开启；缓存命中仍核对工具版本。
- Linux 冷安装已实际验证 dx 0.7.9 与 wasm-bindgen / test-runner 0.2.126，分别约 31 / 36 秒。共享安装器也影响现有 native release / browser CI；Windows、macOS runner 未在本地验证。
- dx 0.7.9 不读取 PATH 中的 wasm-bindgen，添加局部版本适配并缓存 Web 工具；关闭 release debug symbols。
- seed 判定从通用 transaction 移至专用初始化操作；普通 transaction 函数与原始 main 保持一致。Web Lock / cache 调度复用，infra 不依赖 Pages 或 fixture；业务与 UI 不分叉。
- smoke 使用随机端口、独立 profile / 日志目录，并在失败时保留截图与 CDP 错误；补充添加订阅、单源刷新阻断断言及 reload 导航等待。
- review 后 wasm 契约 harness 再次 cargo check 通过；格式、脚本语法、三个 workflow 的 actionlint（未启用 shellcheck）与完整补丁可应用检查通过。
- review 阶段 commit: pending；该阶段无远端 push、Pages 设置变更或部署。

## 提交授权与验收跟进

- 用户确认「可以，那就提交」，授权将已审查补丁提交到仓库、触发 CI / Pages 并跟进失败。
- 推送前通过 GitHub API 确认 main 仍为基础 HEAD，无并发源码更新；再次 `git diff --check` 通过。
- 本次提交包含 19 个已审查文件，复用前述构建验证；完整浏览器验收交由 GitHub runner 执行。
- 远端运行链接及最终部署结果待 CI 完成后补充。
