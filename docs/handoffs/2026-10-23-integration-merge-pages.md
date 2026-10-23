# 独立复审、历史整合与公网 Pages 验收

- 日期：2026-10-23
- 作者 / Agent：Codex
- 分支：`codex/integration-final-20261023`
- 当前基线：`b749514209d10397287792ada4e35cbbb858f856`
- 相关 commit：pending
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
- 原 `target/audit-20261009/worktree` 在审查期间失去源码与 `.git`，大量 ignored 缓存/日志也被移除，原因尚未确认。已有提交与提交内的验证 JSON/截图完整；没有将丢失的临时日志写成仍可访问。根检出的任务外修改未改动。
- 为避开原 target 清理范围，从确切 b749514 新建 E 盘恢复工作树 `E:\gitclone\RSS-Reader-integration-20261023`。未移除旧工作树登记，也未触碰其他 agent 的检出。

## 验证与验收

- `python scripts/test_build_id.py`：通过。原 b749514 构建脚本的隔离副本运行同一测试：按预期失败，vendor-only 改动前后仍为同一 dev 标识。
- Chrome 155 的真实图片反例：200 图新旧断言均通过；404 图 complete=true/naturalWidth=0，原断言错误通过，新断言拒绝。这里只验证真实图片状态与实际验收断言，不冒充完整 Pages 验收。
- `rustfmt --edition 2024 --check build-support/build_id.rs`、`node --check scripts/browser/pages_smoke.mjs`、`actionlint -shellcheck=''`、`git diff --check`：通过。最终工作流还会执行其正式 shellcheck 检查。
- 临时证据：新工作树 `target/handoff/review-fixes/` 及 `target/handoff/readonly-review-20261023/`。
- 同一子 agent 的定向复审通过，三项发现关闭；独立重跑 Cargo 标识回归 23.202 秒通过。
- [来源路径去向清单](../testing/evidence/2026-10-23-integration/merge-dispositions.json)覆盖 main 的 98 个净变更路径和 MathML 的 58 个路径，使用 `--no-renames` 分别记录重命名两端；[修复证据](../testing/evidence/2026-10-23-integration/review-fixes.json)保留本轮实际结果。
- 历史整合、合并后远端检查、公网 Pages：待完成。

## 风险与后续事项

- GitHub Pages 配置返回自定义站点 `develata.me/RSS-Reader/`；以实际部署输出与 HTTPS 访问结果作为最终验收入口，不能假定 github.io 地址。
- 合并须保留已逐项适配的 v0.2 实现，避免恢复 host/sqlx/BrowserStore；根 main 检出有用户改动，不能在其上切分支、重置或覆盖。
- 完成验收后停在打 tag 之前，等用户点头。

## 给下一位 Agent 的备注

从本交接、上述独立审查记录和 `2026-10-22-branch-integration.md` 的逐提交去向表开始。候选、合并提交与各工作流产物必须按确切 SHA 区分；历史通过证据不能改称本次执行。
