# 独立复审、历史整合与公网 Pages 验收

- 日期：2026-10-23
- 作者 / Agent：Codex
- 分支：`codex/integration-final-20261023`
- 当前基线：`b749514209d10397287792ada4e35cbbb858f856`
- 已推远端 main：`7cb83b2e8dd5903bc60f2d88d2301c2a0cca49e3`
- 当前受测产品 / Pages HEAD：`7cb83b2e8dd5903bc60f2d88d2301c2a0cca49e3`
- 相关 commit：`780a9f8`（审查修复）、`bd59d2f`（main 历史整合）、`699dcf6`（MathML 历史整合）、`7cb83b2`（公网验收的显式资源等待）；最终记录与证据随本次 docs 提交，不再改变产品、驱动或部署输入
- 相关 tag / release：无；用户要求打 tag 前等待点头
- 状态：`validated`；合并与公网功能验收完成，停在 tag 之前

## 工作摘要

用户已明确授权启动只读子 agent，修复问题并复审，通过后合并及完成公网 Pages 验收；本次授权不包含打 tag、Release 或镜像发布。

## 影响范围

- 构建支持、Pages 验收驱动、CI 与部署说明。
- Windows / Linux 构建、正式 Web 的构建一致性、Pages 公式图片检查。
- 本任务的编译输出、测试证据与浏览器专用 profile 均在 E 盘；没有在 C 盘编译。

## 关键变更

- 独立审查发现两个 P2、一个 P3，见 [审查记录](../../../reviews/2026-10-23-integration-review.md)。已补 vendor 构建标识、公式图片实际加载断言，以及 systemd 凭据保留说明。
- 新增真实 Cargo 构建回归并接入 CI，使用两个零依赖临时 crate，分别调用实际共享构建脚本，避免只比较输入名单。
- 主 agent 在核对合并文档时另发现三处当前入口仍称 Pages 使用 BrowserStore：历史 spec 的当前状态提示、quickstart 与 application consolidation plan。已统一为阶段 D 的共享 SQL / 内存 SQLite / 重载重建；历史 handoff 与归档设计仍保留原日期语义。
- 原 `target/audit-20261009/worktree` 在审查期间失去源码与 `.git`，大量 ignored 缓存/日志也被移除；用户随后确认清理已结束、允许恢复工作树。已有提交与提交内的验证 JSON/截图完整；没有将丢失的临时日志写成仍可访问。根检出的任务外修改未改动。
- 为避开原 target 清理范围，从确切 b749514 新建 E 盘恢复工作树 `E:\gitclone\RSS-Reader-integration-20261023`。未移除旧工作树登记，也未触碰其他 agent 的检出。

## 验证与验收

- `python scripts/test_build_id.py`：通过。原 b749514 构建脚本的隔离副本运行同一测试：按预期失败，vendor-only 改动前后仍为同一 dev 标识。
- Chrome 155 的真实图片反例：200 图新旧断言均通过；404 图 complete=true/naturalWidth=0，原断言错误通过，新断言拒绝。这里只验证真实图片状态与实际验收断言，不冒充完整 Pages 验收。
- `rustfmt --edition 2024 --check build-support/build_id.rs`、`node --check scripts/browser/pages_smoke.mjs`、`actionlint -shellcheck=''`、`git diff --check`：通过。最终 CI 的 test-tools 也已通过，包括正式 shellcheck 检查。
- 临时证据：新工作树 `target/handoff/review-fixes/` 及 `target/handoff/readonly-review-20261023/`。
- 同一子 agent 的定向复审通过，三项发现关闭；独立重跑 Cargo 标识回归 23.202 秒通过。
- [来源路径去向清单](../../../testing/evidence/2026-10-23-integration/merge-dispositions.json)覆盖 main 的 98 个净变更路径和 MathML 的 58 个路径，使用 `--no-renames` 分别记录重命名两端；[修复证据](../../../testing/evidence/2026-10-23-integration/review-fixes.json)保留本轮实际结果。
- 两次显式 ancestry merge 已完成；三个提交的 tree 同为 `bb187bda01cca1c12b60d2258b86b7f6c917a3ee`，源码树没有因合并改变。同一子 agent 已独立核对第二父、完整路径集合、最终树与新增文档，PASS 后才推 main。
- 根检出的 main 分支仍留在原 d9ebe35，任务外 dirty 保留；远端 main 先快进到 699dcf6，之后更新为 7cb83b2。没有在脏检出上 reset、checkout 或覆盖文件。
- 合并后长 CI：[39923589928](https://github.com/Develata/RSS-Reader/actions/runs/39923589928) 已完成 35/35 success；四平台包/容器：[39923596043](https://github.com/Develata/RSS-Reader/actions/runs/39923596043) 已完成 7/7 success；Pages 构建/实际部署：[39923520596](https://github.com/Develata/RSS-Reader/actions/runs/39923520596) 已完成 2/2 success。三者均为确切 699dcf6；实际下载的长验收、四平台包各 6 项与容器 9 项结果已归档于 [acceptance-and-packages.json](../../../testing/evidence/2026-10-23-integration/acceptance-and-packages.json)。
- 实际 HTTPS 的 index / 404 / JS / WASM 均与上传的 github-pages artifact 逐字节 SHA-256 一致。公网浏览器首次完整 smoke 在 30 秒加载期限失败，不能用 CI 的本地通过覆盖。
- 排除残留诊断下载后，独立 Chrome profile 单次禁缓存冷启动成功：WASM 完成于 71.570 秒，76.040 秒采样已出现阅读正文，无运行时异常。无测量 hook 的请求也持续收数据，说明本次失败发生在传输尚未完成时；具体 CDN/代理/链路瓶颈未定位。记录位于 `target/handoff/public-pages/solitary-probe.json`。
- 为公网验收增加显式 `PAGES_LOAD_TIMEOUT_MS`，仅扩展文档加载等待，默认 30 秒及交互期限、图片断言均不变；结果记录真实计时与期限，失败保留传输记录，退出关闭本次标签页。此增量只读复审 PASS。
- Windows Chrome 155 的第二轮完整公网检查使用 180 秒加载期限：六篇正文、570 个 MathML、重载重置、旧存储和禁网操作通过，最后的公式图断言失败；5 秒时 144 张图有 104 已加载、40 pending、0 确认坏图。该轮整体仍记 fail，证据 `target/handoff/public-pages/solitary-profile/failure.json`。
- 图片另增 `PAGES_IMAGE_TIMEOUT_MS`（默认仍 5000），仅在公网显式设 60000；用短 CDP 查询轮询，记录逐篇 `imageWaitMs`，实际加载/自然宽/布局宽断言不变。该增量也已只读复审 PASS，无新增 P0–P3，提交为 `7cb83b2`。
- 现成 Docker Chrome 156 的公网尝试失败：WASM 返回 503，引发 bundle 加载异常，继而等到 180 秒期限。它不是纯慢速超时，保留 [失败记录](../../../testing/evidence/2026-10-23-integration/public-linux-503.json)。没有编译或新建镜像，源码只读挂载，profile/证据均在 E。
- Windows Chrome 155、原系统代理路径、禁缓存的完整公网复验 **PASS**：2 个真实源、6 篇文章、570 个 MathML、144/144 公式图，重载重置、旧存储保留、原文与来源、子路径/404 直达、禁网操作全部通过。部署为 699dcf6，驱动为 7cb83b2（SHA-256 `7479212a632d520d4a9f91390dfac046ca5da1a0bb803eb8f380511f5810991c`）；[完整结果](../../../testing/evidence/2026-10-23-integration/public-pages-699dcf6.json)绑定两者。
- 该次首屏 9642.9 ms，WASM 传输 8007.3 ms、gzip 1,894,216 bytes、解压 4,992,463 bytes；线性内存 4,587,520 bytes，三个主线程长任务 79/88/274 ms。陶哲轩图片实际等待 7528 ms；5 秒期限确实不足，延长等待没有豁免坏图。正文及公式截图已目视检查并归档。
- 7cb83b2 的 [分支长 CI](https://github.com/Develata/RSS-Reader/actions/runs/39930743719) 35/35 success，下载真实 assertions 再核对 27/27 场景、108 checks、0 unexpected console；[默认期限 Pages 检查](https://github.com/Develata/RSS-Reader/actions/runs/39931551509)成功，仍为加载 30000 / 图片 5000 / 交互 30000 ms。该分支 Pages 不部署。
- 7cb83b2 已快进至 main：[main CI](https://github.com/Develata/RSS-Reader/actions/runs/39932187742) 35/35 success，[Docker](https://github.com/Develata/RSS-Reader/actions/runs/39932187988) 4 success / 1 native matrix skipped，[Pages build/deploy](https://github.com/Develata/RSS-Reader/actions/runs/39932187880) 2/2 success。实际下载的 main 默认验收为 24 场景通过 / 3 长场景按配置跳过、104 checks、0 unexpected console；同 SHA 的分支长验收仍为 27/27、108 checks。Linux 包 6 项、容器 9 项均通过；不将本轮写成四平台重跑。
- 最终 main 的工作流与产物回执分别归档为 [final-main-workflows.json](../../../testing/evidence/2026-10-23-integration/final-main-workflows.json) 和 [final-artifact-checks.json](../../../testing/evidence/2026-10-23-integration/final-artifact-checks.json)，8 个下载 ZIP 的 SHA-256 再次核对一致。新部署 CI 本地 Pages smoke 仍在默认 30000 / 5000 / 30000 ms 期限下通过。
- 7cb83b2 新部署的 index / 404 / JS / WASM 四个公网文件 SHA-256 与实际发布 artifact 全部一致，见 [完整性记录](../../../testing/evidence/2026-10-23-integration/public-integrity-7cb83b2.json)。其中 WASM 完整下载耗时 100077 ms，仍保留为链路波动证据。
- 最终部署再做定向公网浏览器确认：陶哲轩正文与 144/144 公式图、每日数学正文与 116 个 MathML、标题/原文链接/布局，以及确切的新 JS/WASM URL 全部通过，无控制台异常和禁用网络请求；两张截图已目视检查。见 [最终浏览器结果](../../../testing/evidence/2026-10-23-integration/public-pages-7cb83b2.json) 与包含原始字节内容及 SHA-256 的 [检查脚本记录](../../../testing/evidence/2026-10-23-integration/public-pages-7cb83b2-probe.json)。两次禁缓存正文就绪分别为 26389.6 / 63323.2 ms，使用加载 180 秒、图片 60 秒期限。
- 上述最后一步是定向确认，不是第二次完整交互套件。699dcf6 → 7cb83b2 仅改驱动与文档，产品源码、资源、依赖和构建支持完全一致；三份构建快照除抓取时刻外，两个源及六篇文章数据逐一相同，其中分支运行 39931551509 没有部署。因此完整交互覆盖绑定 699dcf6 的实际公网通过记录，最终部署覆盖绑定 7cb83b2 的新文件及两种公式确认，两者不混写。
- 两次 Windows 失败、单下载诊断、合并提交的真实长验收与各平台包结果已纳入 `docs/testing/evidence/2026-10-23-integration/`，仍明确区分失败、诊断成功、CI 静态站与公网完整验收。
- 同一提交的默认 push CI / Docker 由完整手动检查接替，重复运行取消不作为产品失败，也不作为通过证据；没有触发 Release 或镜像推送。
- 最终只读证据核对通过，唯一 P3 文案已将“三个部署快照”更正为“三份构建快照”，明确分支运行没有部署；审查者确认修正后可提交 docs，无需重跑网络或编译。JSON 解析、归档脚本内容 SHA、8 个 ZIP SHA 和 staged diff 检查均通过。本任务的两个专用 Chrome 进程已关闭。

## 风险与后续事项

- GitHub Pages 配置返回自定义站点 `develata.me/RSS-Reader/`；以实际部署输出与 HTTPS 访问结果作为最终验收入口，不能假定 github.io 地址。
- 公网资源加载存在明显波动：单下载诊断曾耗时 71.570 秒，完整通过那轮的 WASM 传输为 8.007 秒，最终产物完整性下载为 100.077 秒，Docker 尝试曾收到 503。具体 CDN、代理或链路瓶颈未定位；功能通过不代表启动性能或公网可用性达标。主 agent 的一次后置 Node 完整性请求在 60 秒截止时中止，没有记为通过或哈希不符。
- 合并须保留已逐项适配的 v0.2 实现，避免恢复 host/sqlx/BrowserStore；根 main 检出有用户改动，不能在其上切分支、重置或覆盖。
- 只读复审属于同模型的独立 agent 审查，不冒充跨模型复审。未补做真实 systemd 安装、移动端目视或另一操作系统的公网通过验收；Linux Docker 公网 503 失败如实保留。
- 最终 docs 提交仅归档记录、结果和截图；Pages 的 docs 路径排除规则不会因该提交重新部署。受测产品和公网部署绑定 7cb83b2，四平台包绑定产品源码相同的 699dcf6。
- 已完成授权内的修复、复审、合并、远端验证与公网功能验收；接下来等用户点头再打 tag。未创建 tag、Release 或发布 GHCR 镜像。

## 给下一位 Agent 的备注

从本交接、上述独立审查记录和 `2026-10-22-branch-integration.md` 的逐提交去向表开始。候选、合并提交与各工作流产物必须按确切 SHA 区分；历史通过证据不能改称本次执行。
