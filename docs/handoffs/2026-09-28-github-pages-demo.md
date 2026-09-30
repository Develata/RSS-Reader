# GitHub Pages 接入补丁

- 日期：2026-09-28
- 作者 / Agent：Codex
- 分支：main
- 基础 HEAD：dbe68e11fdbf93769bfa876831113a17928108ab
- 相关 commit：功能 `a275b0e`；smoke 修复 / 部署 `3e0d199`；本记录最后一次更新为纯文档提交
- 相关 tag / release：v0.1.20 发布记录
- 状态：已提交、部署成功；主 CI、Pages 与 Docker smoke 均通过

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

功能和 smoke 修复已进入 main；Pages 已部署真实 Dioxus bundle，主 CI、Wasm 浏览器契约与 Docker smoke 均成功。最终运行证据见文末。

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

## 首次远端验收与修复

- 功能提交：`a275b0ed7c738ead5e6743d5eb38aa832ab1e588`；CLI 无推送凭据，使用已连接 GitHub Git Data API 创建等价 tree / commit 并以非 force 更新 main。未改变仓库保护规则。
- [首次 Pages 运行](https://github.com/Develata/RSS-Reader/actions/runs/36451614213)：真实 release 构建成功；CDP 依次完成 reader、子路径、收藏持久化、添加和刷新阻断断言，最后错误汇总失败，部署被跳过。
- 原因：Dioxus 规范化路由去掉 `?pages-smoke=1`，reload 的同一文档返回预期 HTTP 404；测试只按完整 URL 豁免首次直达，误将 reload 归为资源错误。
- 修复：仅对 `Document` 类型、HTTP 404、同 origin + pathname 的目标路由豁免；任何非文档资源 404、其它路由、500、运行时错误与外部请求仍失败。
- [主 CI](https://github.com/Develata/RSS-Reader/actions/runs/36451614234) 的三组 Wasm 浏览器契约已通过；subscription harness 19/19，包含新增原子初始化与已有数据保护用例；五主题 Web UI 已通过。完整运行最终状态待回读。
- 修复验证：`node --check scripts/browser/pages_smoke.mjs`、`git diff --check` 通过；修复的真实浏览器回归由后续 Pages run 验证。

## 最终远端验收（2026-09-28）

- 部署源码：`3e0d199d9ddd4494e8c24e1156f47a8018805a84`。
- [Pages 36452167159](https://github.com/Develata/RSS-Reader/actions/runs/36452167159)：build、CDP smoke、产物上传与 deploy 全部成功；部署日志于 16:38:20 UTC 报告 success。
- [CI 36452166565](https://github.com/Develata/RSS-Reader/actions/runs/36452166565)：21/21 jobs 成功，包括聚合 lint-and-test、Android bundle、六个 native module、三组 Wasm 浏览器契约、五主题 Web UI。首次功能提交的 CI 36451614234 同样成功。
- [Docker 36452166434](https://github.com/Develata/RSS-Reader/actions/runs/36452166434)：镜像构建与容器运行验收成功；main 按既有规则不推送 tag 镜像。
- Pages 日志确认 dx / wasm-bindgen / Chrome 命中版本缓存；真实 release bundle 构建约 109.74 秒，Web asset tools 缓存成功保存。
- CDP 成功日志：`Pages smoke passed: actual reader, 404 deep link, prefix, persistence, blocked add/refresh`；没有放宽外部网络请求或资源加载错误断言。
- 公开地址：[https://develata.github.io/RSS-Reader/](https://develata.github.io/RSS-Reader/)。独立 HTTP 回读首页与 hash JS 均 200；hash WASM 200、Content-Type 为 application/wasm、magic 为 0061736d；`/entries/2` 返回预期 404 并包含同一应用入口，`/404.html` 为 200。
- 浏览器功能验收在 GitHub runner 对同一发布 bundle 执行；公开域名补充 HTTP / 资源检查，没有将 HTTP 检查冒充线上浏览器交互验收。
- 结束后仅提交本交接文档，使用 `[skip ci]` 避免为相同源码重复执行全套构建；不修改 workflow、测试或分支保护。已部署版本保持 `3e0d199`。
- 已收敛本轮 Pages 失败，无待修复 CI 错误。Windows / macOS 安装器与设备交互仍未在本轮执行；未打新 release tag。
