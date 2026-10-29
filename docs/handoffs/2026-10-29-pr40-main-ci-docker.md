# PR #40 合并后 main CI 与 Docker 核验

- 日期：2026-10-29
- 作者 / Agent：Codex
- 分支：`codex/pr40-main-integration`；核验及文档推送目标为 `main`
- 核验 HEAD：`aa60c661e9a7b28380d4b9a623487492f2312244`
- 相关 commit：`ad5e8f31`（PR #40 合并）、`4bf4fd5d`（前一交接）、`aa60c661`（PR #45 文档合并）；本次交接归档提交见本文件 git log
- 相关 tag / release：无
- 状态：`validated`（本节所列 main HEAD 的 CI 和 Docker 均成功）

## 工作摘要

按用户要求继续核对 PR #40 合并后 main 的 CI 与 Docker。期间 main 又合入 PR #45 的三份文档，旧 CI 被新运行取代；最终核验 `aa60c661` 的 CI 36/36 成功、Docker 4 项成功 / 1 项预期跳过，原始验收报告及散列一致。没有发现需要修复的产品代码或测试断言问题。

## 影响范围

- 本次修改：交接记录、维护状态、索引及[结构化主线回执](../testing/evidence/2026-10-29-pr40-final-review/main-ci-docker.json)。产品代码、依赖与工作流无改动。
- 复核入口：`.github/workflows/ci.yml`、`docker.yml`、`web-packages.yml`，以及既有真实服务器、浏览器、Linux 包和容器验收。
- 平台：GitHub runner 上的 Rust / Web / Android 编译与契约检查、Linux 实际容器。没有新增本机 GUI 或其他原生平台的手工验收。
- 仅使用当前工作树；保留其他工作树、用户资料库和旧运行证据。

## 运行变化与诊断

- `4bf4fd5d` 的 [Docker 40924582920](https://github.com/Develata/RSS-Reader/actions/runs/40924582920) 成功，下载报告确认 Linux 包 6 项、容器 9 项通过，构建标识精确为 `git-4bf4fd5d21fc1e631b872915854659d7bf17f71a`。
- 它的 [CI 40924582755](https://github.com/Develata/RSS-Reader/actions/runs/40924582755) 最终为 cancelled：34 项成功、默认主题作业取消，`lint-and-test` 因依赖结果不是 success 而失败。原始日志明确记录 `The operation was canceled.`，门禁输入为 `web-ui.result=cancelled`，没有已完成测试的断言失败。
- PR #45 于此期间合入 main，新增三份交接文档；新 CI 开始时间与现有 `cancel-in-progress: true` 规则解释了旧运行的取消。未修改并发规则或放宽汇总门禁。
- 当前工作树 fast-forward 到 `aa60c661`；比较 `4bf4fd5d..aa60c661` 的产品、脚本、构建输入与工作流路径均无差异。转而验证新 main，未重跑已被取代的旧提交。
- PR #40 合并提交 `ad5e8f31` 的 Pages 和 Docker 已成功；其 CI 也曾被后续 main 推送取代。文档推送不触发 Pages 的既有 paths-ignore 规则没有改变。

## 验证与验收

### 自动化及原始报告核对

- [CI 40925661679](https://github.com/Develata/RSS-Reader/actions/runs/40925661679)：`push` / `main` / `aa60c661` / attempt 1，完整作业列表 36/36 success，包含最终 `lint-and-test`；没有待执行、失败或跳过项。
- [Docker 40925661750](https://github.com/Develata/RSS-Reader/actions/runs/40925661750)：同一 HEAD / attempt 1，source、page、Linux 包及 image 4 项成功。额外 Windows/macOS 包的 native 作业按默认 `all_platforms=false` 跳过。
- Docker image 作业确实运行了新卷、旧凭证只读迁移、owner CLI、备份、正常重启与会话/数据保留检查。GHCR 登录和镜像推送按默认非发布路径跳过；本次没有发布镜像。
- `gh api .../actions/runs/<id>` 核对事件、分支、SHA、attempt 和结论；CI jobs 使用 `per_page=100` 并检查 `total_count == jobs.length`，没有把首批作业当作全部。
- 已下载三个 artifact ZIP 并比较 SHA-256 与 GitHub digest，随后解析实际 JSON / 日志。所有构建标识匹配 `aa60c661`，报告中的成功项与工作流结论一致。

| Artifact | ID | SHA-256 | 验收结果 |
| --- | --- | --- | --- |
| ci-reading-workspace | 12920417907 | `4e6ee8290702aef1d72c40db69d63d030fc6a5fa45886a4eaffdc67c33f816e1` | workspace 18、Entries 8、Settings 8、目录顺序 26、目录窗口 26、窗口边界 11、目录复核 8，transport 通过，页面异常为空 |
| c6-container-check | 12918754864 | `65b188baa9f08f1274bfc83a1288769cb4c931e8ee6fc500af8313ea2134201a` | 9 项通过，镜像中的服务器与已验包的二进制一致 |
| web-package-check-linux-x86_64 | 12918664961 | `627b93a95e317a3eea5b31358b42141c824bd39758fe4b8549ba1983c6e3f0a6` | 6 项通过：包散列与 WASM 标识、建库、登录、静态页、同版本写入、旧版本写入拒绝 |

- `node target/verify-main-ci-evidence.cjs aa60c661e9a7b28380d4b9a623487492f2312244`：通过，逐项断言上述场景及构建身份。
- `git diff --exit-code 4bf4fd5d21fc1e631b872915854659d7bf17f71a aa60c661e9a7b28380d4b9a623487492f2312244 -- crates assets scripts .github build-support Cargo.toml Cargo.lock Dioxus.toml`：通过。
- 文档提交前执行相对链接、JSON 回执、`git diff --check` 和产品路径无变化检查；本轮仅文档，无需再次本地重编译同一产品代码。

### 手工验收与边界

- 未执行 Computer Use、系统文件选择器、真实 IME、Android 真机、macOS/Linux GUI 或正式安装包验收。本次下载的是远端实际运行报告，不宣称在本机重新启动了容器或原生应用。
- 常规 CI 的服务器验收采用短场景；没有将其记为手动开启的全部长场景。

## 结果与后续

- 本节记录的 main `aa60c661` 已通过 CI 与 Docker，无产品或工作流修复项。旧提交取消不计为新提交通过，也不当作产品断言失败。
- 本次文档收口提交会再次触发 main CI/Docker；提交后的核验按该提交自己的 SHA 和运行结果进行，最终回执另固定在 [PR #40 描述](https://github.com/Develata/RSS-Reader/pull/40)，与本节的历史基线分开。
- 原先偶发 CI 等待问题的触发条件仍沿用[已有边界记录](./2026-10-29-pr40-ci-wait-boundaries.md)，本轮连续通过不新增根因结论。不打 tag，不发布新版本。

## 给下一位 Agent 的备注

- 优先读本记录、[最终合并记录](./2026-10-29-pr40-final-review-merge.md)及结构化主线回执；后续 main 前进时重新核对 HEAD。
- 本地原始报告：`target/main-aa60c661-reading-workspace/`、`target/main-aa60c661-c6-container-check/`、`target/main-aa60c661-web-package-check-linux-x86_64/`；元数据和摘要为同前缀 JSON 文件。
- 旧取消运行的原始 jobs、默认主题日志和门禁日志保留在 `target/main-4bf4fd5d-*`，不要将其中的未完成状态当作最新 main 结果。
