# Reader 三批整合与正文渲染边界

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`main`；复用 `E:\gitclone\RSS-Reader-worktrees\native-back-policy`
- 整合前 main：`4cef074c54d7df54808e7892f950f56a7a750232`
- 产品整合 HEAD：`dbbaf39008afd7d318538161d8e314374d58fb24`
- 相关 commit：三次合并见下表；本记录所在提交只补正文契约、维护状态和证据
- 相关 tag / release：无新 tag 或版本发布；Pages 按 main 工作流自动部署，Docker 未推送镜像
- 状态：`validated`（产品整合验证通过；后续文档 head 的远端检查按精确 SHA 单独留存）

## 工作摘要

按已确认的下一步顺序，先整合 Reader 规则、宿主入口、快捷键定义及模块归属，再按实际源码明确正文内容、资源和阅读锚点与 HTML/DOM 的边界。三批实现已进入 main，没有待合并的叠加实现。本批没有新增运行代码，也没有把未来渲染器能力写成已完成。

## 影响范围

- 合并范围：PR #50 的普通规则与请求归属、#51 的图片/dialog/快捷键宿主入口、#52 的快捷键单一定义及模块归属。
- 本轮文档：正文渲染契约、设计索引、原生控件兼容与验收边界、维护状态及待办。
- 平台：现有 Windows、Android、正式 Web 代码进入 main；本轮新执行 Rust、本地无框架规则、主线 CI、Docker/Linux 包及 Pages 流程。
- 七 crate、Backend/LocalBackend、rusqlite 单库、正式 Web 同源服务和现有 UI 保持不变。

## 关键变更与 review

### 按依赖顺序整合

| PR | 已核验的 head | 合并到 main | head CI |
| --- | --- | --- | --- |
| [#50](https://github.com/Develata/RSS-Reader/pull/50) | `ff628ed03a96e57392b94ad3655cc4cb62be736a` | `7edb118d2ec30a3427842d3e0302f021dfb5f48e` | [41129157604](https://github.com/Develata/RSS-Reader/actions/runs/41129157604)：36/36 |
| [#51](https://github.com/Develata/RSS-Reader/pull/51) | `6a5b91b460a112543bd5154e9b86705b83421748` | `5d51025ae8084d53dc30b4287463224621ecdddd` | [41135027211](https://github.com/Develata/RSS-Reader/actions/runs/41135027211)：36/36 |
| [#52](https://github.com/Develata/RSS-Reader/pull/52) | `4d0aab02271fbc12bd424da91dd779d1d4394f88` | `dbbaf39008afd7d318538161d8e314374d58fb24` | [41147314689](https://github.com/Develata/RSS-Reader/actions/runs/41147314689)：36/36 |

#51、#52 在前置批次合入后改以 main 为 base，先用 `git merge-tree --write-tree` 核对预期树，再以精确 head SHA 合并；没有冲突或手动改写实现。每次合并后均核对整个文件树与该 PR head 相同。最终树为 `eb9ada0e1cc288526b6f1b574730a13c007f0165`，等于 `4d0aab02` 的树。

已检查各 PR 的 reviews、inline threads 和 comments，均为空；本轮是源码自审与自动化核验，不称作外部 review 批准。未发现新的阻塞问题。此前偏好响应等待的 CI 偶发失败尚无确定根因，本轮通过不能证明其已被修复，见[既有诊断](./2026-10-30-reader-shortcut-definition.md#ci-失败与独立诊断)。

### 正文、资源与锚点契约

新增[Reader 正文渲染边界](../design/reader-rendering-boundary.md)，集中记录：

- 当前 HTML → HTML 形态文本/摘要 → 普通文本的实际候选顺序、空字符串与缺失的区别，以及唯一 infra 清洗/预算链路。
- 正文在标记、重读和图片缓存完成时保持不变；访问归属、资源缓存、呈现与宿主焦点分别由现有模块负责。
- 当前图片状态接收 `currentSrc || src`，不是稳定资源 ID；DOM 锚点依赖块序号、标签和文本，仅属于本次运行，不是跨渲染器书签。
- HTML/CSS、主题、选择复制、IME、无障碍与系统操作需要具体候选宿主证明。现有 24 项无框架测试不包含正文选择/清洗链路。

未引入通用文档 IR、全局 Host trait、新 crate、资源数据库或第二份界面状态。下一实现只建议处理 `ui/runtime/reader.rs → pages/reader_page/support.rs` 的正文选择依赖；此函数目前仍在原处，本批没有提前实施。

## 验证与验收

原始证据：`target/reader-integration-20261030/`；提交内精简回执：[receipt.json](../testing/evidence/2026-10-30-reader-integration/receipt.json)。证据分别标明来源 head，历史 CI 不替代主线检查。

| 命令或流程 | 实际结果 |
| --- | --- |
| `cargo fmt --all --check`、`git diff --check` | 通过 |
| `cargo test -p rssr-app --locked` | 合并后重新运行：420 通过、1 忽略；391 单元、24 独立规则、2 主题、3 样式 |
| `python scripts/check_reader_rule_boundary.py --target-dir target/reader-integration-20261030/rules-build` | 合并后重新编译同一生产模块：24/24；不链接 Dioxus/WebView |
| main `dbbaf390` [CI 41148876182](https://github.com/Develata/RSS-Reader/actions/runs/41148876182) | 36/36 成功；读取完整 jobs 列表核对 |
| CI 的正式 Web Reader 宿主脚本 | 下载 `ci-reading-workspace` 核对：4/4，两视口各 70 项过滤边界、10 个有效键值；无页面或控制台错误；`serverBuild=dev-dbbaf39008af` |
| main [Docker 41148876298](https://github.com/Develata/RSS-Reader/actions/runs/41148876298) | 构建与容器验收成功；下载回执为容器 9 项、Linux 包 6 项通过，均绑定 `git-dbbaf39008afd7d318538161d8e314374d58fb24` |
| main [Pages 41148876280](https://github.com/Develata/RSS-Reader/actions/runs/41148876280) | build/deploy 成功；下载 smoke 回执 `pass`，演示为内存 SQLite、刷新重置 |
| `python target/reader-integration-20261030/check_docs.py` | 检查本批 Markdown 链接/锚点和 JSON；具体统计见 `documentation-check.json` |
| `python target/reader-integration-20261030/verify-preservation.py` | 主 checkout 保全、产品源码与分支/工作树数量核对；见下文 |

Docker 的默认主线流程没有执行 all-platforms 原生包 job；GHCR 登录、tag、push 步骤跳过，非发布检查通过。Linux 包检查不能代替 Linux GUI 验收；Pages 的演示烟测不能代替正式 Web 持久化验收。

本轮未重新安装或运行 Windows/Android 应用。实际 Windows 8/8、Android 15 真机 4/4、正式 Web 4/4 的基线/候选回执属于[上一批同产品树验证](./2026-10-30-reader-shortcut-cohesion.md)，包括物料、哈希、隔离资料库及视觉差异调查。它们与本轮新执行的主线 CI 分开列示，不写成合并后重新做过真机验收。

后续文档提交只改变 `docs/`。推送后的精确 head CI/Docker 状态、下载回执与哈希追加到证据根 `final-main-receipt.json`，并在本次最终交付中给出；不在提交前预报成功，也不为把提交 SHA 写进自身而循环提交。

## 产物身份与工作区保全

- `4d0aab02` 与三批整合 main 的整个 Git 文件树一致；后续差异限定为文档。
- 原 464 项产品输入清单 SHA-256 为 `cfa3a2d7d7554c0efef020f7e3ac458cea22b9e1fed3d2e83a31e7833316769c`。当前工作副本 454 项逐字节相同，10 项因 `core.autocrlf=true` 在切分支后由 LF 变为 CRLF；这 10 项去除 CR 后逐字节等于原清单及原 Git blob。原始差异与当前清单保留在 `product-eol-check.json`、`product-files.json`，不把换行规范化写成原始哈希相同。
- 主 checkout 仍在 `feat/reading-workspace-20261024` / `020f9d4031febc709cf1fdca2e7360698ba19f26`。HEAD、分支、status、index 和 41 个文件/缺失路径均与开始时相同；另一任务的 JavaScript 审计未纳入提交。
- 仅当前 E 盘工作树切回并快进 main。本批保留全部 8 个本地分支、2 个工作树、7 个真实远端分支；不清理旧 target、配置、资料库或其他目录。

## 结果、风险与后续

Reader 已完成的三批实现已进入 main，长期目标仍是让当前 WebView 可替换、让交互规则可复用。本轮补齐当前兼容事实与证据边界，没有证明新渲染器可行。

下一小批可整理正文选择模块的归属，并直接运行同一生产函数的输入序列，保留候选顺序、HTML 输出和唯一清洗政策。macOS/Linux 原生交互、实际系统 IME/辅助技术、候选正文渲染器均未在本轮覆盖；不扩大为完整宿主抽象。

## 给下一位 Agent 的备注

从[当前维护状态](../current-status.md)和[正文渲染边界](../design/reader-rendering-boundary.md)继续。开始前重新核对 main、dirty 和远端状态，不把旧工作树名称当成当前分支。正文选择还在 `pages/reader_page/support.rs`；`ui/runtime/reader.rs` 是现有调用方。复用清洗实现和隔离验收脚本，继续保留主 checkout 的其他任务资料。
