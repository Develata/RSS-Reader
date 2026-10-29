# PR #40 接入 PR #43：目录阅读顺序与最新整合验收

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：本地 `codex/pr40-main-integration`；推送目标 `codex/native-ui-preparation-20261029`
- 合并父提交：PR `7345d98a116a0d4248203cd7b3fe075431c9c7c4` + main `0692c92465b25a059d083a977b513d9f8e088355`
- 验收源树：`d9c7c698cf02d69eca7cce55ec4dcdf84f5718ab`（合并解决后、补充本次文档前的 staged tree）
- 相关 commit：本次普通合并与本记录同一提交，可由本文件 git log 查询
- 相关 tag / release：无
- 状态：`validated`（本地范围；推送后的新 head CI 另在 PR 描述核验）

## 工作摘要

`7345d98a` 自动 CI 与 Pages 已通过 37 项、部署 1 项预期跳过。收尾时 main 合入 PR #43，新增目录阅读顺序修复并产生两处文本冲突。本次接入 main `0692c924`，保留主线的共享分组规则、PR #40 的私有生命周期模块和 Settings 逐字段保存/预览规则，再以同次源码构建并验收。

## 影响范围

- 产品模块：main 的 Entries `groups.rs` / presenter 及目录契约；无手工产品冲突改写。
- 集成文件：CI 场景清单、交接索引。
- 平台：Windows 原生 WebView2、正式 Web / 真实 SQLite 服务器；其他平台由推送后的 CI 编译或契约检查。
- 保留其他工作树、分支、用户资料库及安装产物。

## 关键变更

- 两处冲突分别为 `.github/workflows/ci.yml` 和 `docs/handoffs/README.md`，均保留双方内容。
- 目录阅读顺序继续由 main 的共享分组模型负责，以全局页号和来源组织日期内的来源段；完整目录顺序与逐页主列表顺序一致。主线产品文件、夹具、新增回归与契约均原样接入。
- CI 同时运行 `directory_page_order` 和 `native_preparation`，既有 workspace / Entries / 目录窗口 / 传输验收保留。
- Settings 的 Edits / Requests 封装、逐字段回填、主题预览与显式保存，以及验收脚本的等待边界修正保留。
- 新增[紧凑验收回执](../testing/evidence/2026-10-29-pr40-directory-order/result.json)。源码树、合并父提交、实际 EXE 哈希共同标识本地验收；后续只补交接文档，提交前再比较产品路径与验收树一致。

## 验证与验收

### 自动化验证

- `CARGO_BUILD_JOBS=2 cargo test --workspace --locked`：1070 通过、0 失败、4 项既有忽略。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo build -p rssr-app -p rssr-web --locked`：通过。
- Dioxus CLI 0.7.9 `dx build --platform web --package rssr-app --locked --release --debug-symbols false`：通过。使用 `E:/gitclone/RSS-Reader/target/release-ui-tools/dioxus-0.7.9/dx.exe`，不是 PATH 中的 0.7.10。
- 同次 Web/server 构建：新增目录阅读顺序 26/26、workspace 18/18、Entries 8/8、Settings 8/8；目录与页面异常为空，Entries / Settings 无意外控制台错误。
- 使用临时 `target/pr40-isolated-suite.cjs` 将 fixture 端口在进程内改到 9325，分别执行 `directory_page_order`、`workspace`、`entries_lifecycle`、`native_preparation`。仓库 fixture 未增加本地专用端口逻辑。报告和日志位于 `target/pr43-integration-<suite>/` 与同名 `.log`。
- Windows 原生：`node scripts/browser/rssr_native_transport_acceptance.cjs --process-file target/native-pr43-integration/process.json --artifact-dir target/pr43-integration-native --baseline false`，13/13 通过，WebView2 `156.0.4314.50`，页面/控制台错误为 0。
- 原生 EXE SHA-256：`b67c4a016fb05ed531c011f36d7c7dea877451981a8a084f5b94ffccb3f39f72`；合成资料库 5000 篇、6 个回环订阅。
- 正常关闭后：完整性为 ok、外键检查无结果；只复制主数据库独立读取，设置、5000 篇全部标志和 6 个订阅地址与最终报告逐项一致。原库 WAL 为 0 字节。独立主库 SHA-256：`a713f6499c166cfccf9d0da71c7d4c5bc900b05d26e0f12ee786ff58db512ec3`。
- 原生进程正常退出、回环服务停止。原生报告及关闭后回执在 `target/pr43-integration-native/`；EXE / 独立 profile / 数据库在 `target/native-pr43-integration/`。
- `git diff --cached --check`、未解决冲突检查、相对验收树的产品路径一致性检查：通过（提交前）。

### 手工验收

- 本轮使用 CLI、Rust 测试和浏览器 / CDP 自动化，没有 Computer Use。
- OS 文件选择器、真实 IME、Android 真机、macOS/Linux GUI、正式安装包未验收；目录的 Web 逐帧回归不等于各原生平台的触控/惯性验收。

## 结果与 CI 回执

- 本地整合验证通过，按原授权普通合并、commit、push；PR 保持草稿，不合并或发布。
- 整合前 `7345d98a` 的 [CI 40912313353](https://github.com/Develata/RSS-Reader/actions/runs/40912313353) 和 [Pages 40912313412](https://github.com/Develata/RSS-Reader/actions/runs/40912313412) 已成功，37 成功 / 1 预期跳过。阅读 artifact `12915650094` SHA-256 为 `26b5bcfe7e2b099347a596c5e747849d284312cc9d3259e2c129fa6d56f5687a`；本地回执 `target/pr40-7345d98a-ci-receipt.json`。
- 上述旧 head 结果不冒充本次接入 main 后的 CI。推送后按新 head 核对自动检查，最终 head / run / artifact 在 [PR #40](https://github.com/Develata/RSS-Reader/pull/40) 描述固定。

## 风险与后续事项

- 先前 CI 超时的具体触发条件尚未复现，等待边界已收紧且后续整套 CI 通过，详见[等待边界记录](./2026-10-29-pr40-ci-wait-boundaries.md)。若再次出现，保留阶段错误和原始报告定位。
- 不将普通合并无冲突、原生编译或单次通过推导为全平台发布就绪。
- 主线继续前进时，先比较新增内容和职责，不以旧 head 的检查结果代替新整合。

## 给下一位 Agent 的备注

- 先读 `docs/design/entries-directory-follow.md`、`docs/design/native-control-adapter-contracts.md`，再看此记录和 PR 当前 head 的 CI。
- 构建日志前缀 `target/pr43-integration-`；本轮 build 产物来自合并前的等价产品源码树，实际原生身份以回执中的父提交、tree 和 EXE 哈希为准。
