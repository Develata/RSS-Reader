# 独立复审、历史整合与公网 Pages 验收

- 日期：2026-10-23
- 作者 / Agent：Codex
- 分支：`codex/integration-final-20261023`
- 当前基线：`b749514209d10397287792ada4e35cbbb858f856`
- 已推远端 main：`699dcf6b07ef05e5fa0cc967a0cf0020551097ed`
- 相关 commit：`780a9f8`（审查修复）、`bd59d2f`（main 历史整合）、`699dcf6`（MathML 历史整合）；最终验收记录 commit: pending
- 相关 tag / release：无；用户要求打 tag 前等待点头
- 状态：`draft`

## 工作摘要

用户已明确授权启动只读子 agent，修复问题并复审，通过后合并及完成公网 Pages 验收；本次授权不包含打 tag、Release 或镜像发布。

## 影响范围

- 构建支持、Pages 验收驱动、CI 与部署说明。
- Windows / Linux 构建、正式 Web 的构建一致性、Pages 公式图片检查。
- 所有本机编译、临时文件和浏览器 profile 都在 E 盘。

## 关键变更

- 独立审查发现两个 P2、一个 P3，见 [审查记录](../reviews/2026-10-23-integration-review.md)。已补 vendor 构建标识、公式图片实际加载断言，以及 systemd 凭据保留说明。
- 新增真实 Cargo 构建回归并接入 CI，使用两个零依赖临时 crate，分别调用实际共享构建脚本，避免只比较输入名单。
- 主 agent 在核对合并文档时另发现三处当前入口仍称 Pages 使用 BrowserStore：历史 spec 的当前状态提示、quickstart 与 application consolidation plan。已统一为阶段 D 的共享 SQL / 内存 SQLite / 重载重建；历史 handoff 与归档设计仍保留原日期语义。
- 原 `target/audit-20261009/worktree` 在审查期间失去源码与 `.git`，大量 ignored 缓存/日志也被移除；用户随后确认清理已结束、允许恢复工作树。已有提交与提交内的验证 JSON/截图完整；没有将丢失的临时日志写成仍可访问。根检出的任务外修改未改动。
- 为避开原 target 清理范围，从确切 b749514 新建 E 盘恢复工作树 `E:\gitclone\RSS-Reader-integration-20261023`。未移除旧工作树登记，也未触碰其他 agent 的检出。

## 验证与验收

- `python scripts/test_build_id.py`：通过。原 b749514 构建脚本的隔离副本运行同一测试：按预期失败，vendor-only 改动前后仍为同一 dev 标识。
- Chrome 155 的真实图片反例：200 图新旧断言均通过；404 图 complete=true/naturalWidth=0，原断言错误通过，新断言拒绝。这里只验证真实图片状态与实际验收断言，不冒充完整 Pages 验收。
- `rustfmt --edition 2024 --check build-support/build_id.rs`、`node --check scripts/browser/pages_smoke.mjs`、`actionlint -shellcheck=''`、`git diff --check`：通过。最终工作流还会执行其正式 shellcheck 检查。
- 临时证据：新工作树 `target/handoff/review-fixes/` 及 `target/handoff/readonly-review-20261023/`。
- 同一子 agent 的定向复审通过，三项发现关闭；独立重跑 Cargo 标识回归 23.202 秒通过。
- [来源路径去向清单](../testing/evidence/2026-10-23-integration/merge-dispositions.json)覆盖 main 的 98 个净变更路径和 MathML 的 58 个路径，使用 `--no-renames` 分别记录重命名两端；[修复证据](../testing/evidence/2026-10-23-integration/review-fixes.json)保留本轮实际结果。
- 两次显式 ancestry merge 已完成；三个提交的 tree 同为 `bb187bda01cca1c12b60d2258b86b7f6c917a3ee`，源码树没有因合并改变。同一子 agent 已独立核对第二父、完整路径集合、最终树与新增文档，PASS 后才推 main。
- 根检出的 main 分支仍留在原 d9ebe35，任务外 dirty 保留；远端 main 已快进到 699dcf6。没有在脏检出上 reset、checkout 或覆盖文件。
- 合并后长 CI：[39923589928](https://github.com/Develata/RSS-Reader/actions/runs/39923589928) 已完成 35/35 success；四平台包/容器：[39923596043](https://github.com/Develata/RSS-Reader/actions/runs/39923596043) 已完成 7/7 success；Pages 构建/实际部署：[39923520596](https://github.com/Develata/RSS-Reader/actions/runs/39923520596) 已完成 2/2 success。三者均为确切 699dcf6；实际验收 artifact 待归档。
- 实际 HTTPS 的 index / 404 / JS / WASM 均与上传的 github-pages artifact 逐字节 SHA-256 一致。公网浏览器首次完整 smoke 在 30 秒加载期限失败，不能用 CI 的本地通过覆盖。
- 排除残留诊断下载后，独立 Chrome profile 单次禁缓存冷启动成功：WASM 完成于 71.570 秒，76.040 秒采样已出现阅读正文，无运行时异常。无测量 hook 的请求也持续收数据，说明本次失败发生在传输尚未完成时；具体 CDN/代理/链路瓶颈未定位。记录位于 `target/handoff/public-pages/solitary-probe.json`。
- 为公网验收增加显式 `PAGES_LOAD_TIMEOUT_MS`，仅扩展文档加载等待，默认 30 秒及交互期限、图片断言均不变；结果记录真实计时与期限，失败保留传输记录，退出关闭本次标签页。此增量只读复审 PASS。
- Windows Chrome 155 的第二轮完整公网检查使用 180 秒加载期限：六篇正文、570 个 MathML、重载重置、旧存储和禁网操作通过，最后的公式图断言失败；5 秒时 144 张图有 104 已加载、40 pending、0 确认坏图。该轮整体仍记 fail，证据 `target/handoff/public-pages/solitary-profile/failure.json`。
- 图片另增 `PAGES_IMAGE_TIMEOUT_MS`（默认仍 5000），仅在公网显式设 60000；用短 CDP 查询轮询，记录逐篇 `imageWaitMs`，实际加载/自然宽/布局宽断言不变。该增量也已只读复审 PASS，无新增 P0–P3。现成 Docker Chrome 156 镜像用于另一轮完整公网验收，源码只读挂载，profile/证据均在 E 盘；不会编译或新建镜像。完整结果待收集，驱动增量 commit: pending（本次提交）。
- 两次 Windows 失败、单下载诊断、合并提交的真实长验收与各平台包结果已纳入 `docs/testing/evidence/2026-10-23-integration/`，仍明确区分失败、诊断成功、CI 静态站与公网完整验收。
- 同一提交的默认 push CI / Docker 由完整手动检查接替，重复运行取消不作为产品失败，也不作为通过证据；没有触发 Release 或镜像推送。

## 风险与后续事项

- GitHub Pages 配置返回自定义站点 `develata.me/RSS-Reader/`；以实际部署输出与 HTTPS 访问结果作为最终验收入口，不能假定 github.io 地址。
- 合并须保留已逐项适配的 v0.2 实现，避免恢复 host/sqlx/BrowserStore；根 main 检出有用户改动，不能在其上切分支、重置或覆盖。
- 完成验收后停在打 tag 之前，等用户点头。

## 给下一位 Agent 的备注

从本交接、上述独立审查记录和 `2026-10-22-branch-integration.md` 的逐提交去向表开始。候选、合并提交与各工作流产物必须按确切 SHA 区分；历史通过证据不能改称本次执行。
