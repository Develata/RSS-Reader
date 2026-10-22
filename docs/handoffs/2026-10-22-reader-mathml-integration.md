# 发布候选：真实 RSS 数学公式保真

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 当前 HEAD：`9b70455`
- 相关 commit：`c2e2278`
- 相关 tag / release：未创建，等待最终验收与用户确认
- 状态：`draft`

## 工作摘要

阶段 D 使用真实 RSS 后，目视验收发现每日数学的 MathML 被现有 ammonia 白名单整体删除。接入 `feat/reader-mathml` 的展示与预算实现，在当前分支上保留较新的图片 URL 安全策略，并补充真实快照的公式保真断言。

## 影响范围

- 模块：infra 的正文归一化、消毒、MathML 与 HTML 解析预算；reader CSS。
- 平台：所有阅读页；原生压力测试、真实 Chrome/WASM 重解析及 Pages 验收。
- workflow / 文档：浏览器契约矩阵增加正文 target（共 14 个），用户指南与验收驱动。
- worktree：`target/audit-20261009/worktree`；未修改其他 worktree。

## 关键变更

- 来源为确切提交 `73f8d6ee94f2926ac49d8e2f4f6b2e3a30f43c3b` 的 MathML、HTML 预算、纯文本降级和相关测试。历史四轮评审仅是来源证据，不等于本次适配经过独立评审。
- ammonia 只允许安装一次属性回调。本次将 MathML 属性检查和当前分支的 `data:` 图片限制组合，保留当前 `srcset` 状态机、描述符校验及全部原有安全测试，未整分支覆盖或合并。
- MathML 展示元素及有限属性白名单、公式规模上限、KaTeX 双份去重、超限公式转为转义文本；超宽公式在正文内滚动。
- HTML 树构建采用已批准的计量预解析和整篇纯文本降级方案。它是纵深防御，不能证明任意输入的全局耗时上界；已知 Marker、超长单标签和分词器属性去重边界继续记录在 `parse_budget.rs`。
- 旧分支压力测试在当前归一化器上复现四项输出/解析预算失败；接入 URL 解析工作量、输出增长、重复属性、本地化图片复制与实体扫描的预算。点段预检留在 infra，HTML 计量 sink 同时检查解码后的 URL 属性，以覆盖 ammonia 在自定义回调之前的再次解析；不把 opaque 的本地化 `data:image/` 内容当作点段路径。未引入旧分支的领域 URL 接口或 vendor/url 补丁。
- Pages 验收逐篇核对原始 MathML 数量、浏览器命名空间、非零布局和 LaTeX 图片数量，并等待图片加载后截图。陶哲轩的 144 张公式图片本身正常，旧截图为空是抓取时机问题。

## 验证与验收

### 自动化验证

- `cargo test --locked -p rssr-infra --lib html::`：86 passed，0 failed。
- `cargo test --locked -p rssr-infra --test test_reader_html_stress`：24 passed，0 failed，1 ignored（耗时基准）。包含仓库真实 RSS 快照的公式/图片保真。
- `wasm_reader_html_contract_harness`：Chrome 156、确切 WASM 产物，6 passed，0 failed；验证命名空间、重复解析、攻击载荷、公式降级、KaTeX 和 390px 容器布局。
- 日志：`target/handoff/mathml-html-unit-final.log`、`mathml-stress-final.log`、`mathml-wasm-logs-final/`。编译产物均在 E 盘。
- `cargo test --workspace --locked`：861 passed / 0 failed / 3 ignored；这是补充 URL 属性预算测试之前的完整基线。补充后定向压力测试 24 / 24 通过，最终完整回归由远端 CI 再跑。日志 `mathml-workspace-tests.log`。
- workspace 全 targets Clippy、正式 Web wasm Clippy、infra memory-store/http-backend wasm tests Clippy 均通过；release-ui runner 3 项测试通过。
- Dioxus release Pages 包在 `/RSS-Reader` 完成真实 Chrome 验收：597 个 MathML 全部保留且具有 MathML 布局，144 张公式图片全部加载；重载重置和禁用在线操作通过。
- 最终远端长验收、Pages 和四平台 Web 包 / Docker：待推送后执行。

### 手工验收

- 修复前每日数学首篇原始 120 个 MathML 在浏览器中变成 0；原始 RSS 快照和 SQL 转换均保留内容。
- 图片等待后的修复前基线：`target/handoff/phase-d-formula-baseline/run.Lo8ONQ/result.json`，陶哲轩 144 / 144 张图片加载。
- 修复后 Pages 目视验收通过：`target/handoff/mathml-pages-final/run.57gFWm/`，首篇和陶哲轩正文截图均可见公式。容器只读挂载本机微软雅黑与 Cambria Math 字体用于验收，字体未打包或入库。该包在后续 opaque-image URL 预算豁免之前构建；后者已由压力测试覆盖，远端重新构建最终源码。
- 本地未压缩传输：启动约 1003.9 ms，WASM 线性内存 4,718,592 bytes；不是公网/CDN性能结果。

## 结果

已提交并推送 `c2e2278`。本地验收通过；远端最终候选、实际公网 Pages 与独立评审仍未齐备，不能宣布可打 tag。

## 风险与后续事项

- 完成同一候选的远端长验收、各平台打包与 Docker 实际包运行；实际公网 Pages 子路径和最终独立评审仍需完成。
- Chrome 覆盖不能替代 WebKit 或 Android 实机阅读页；原站图片可能不可用。
- HTML 计量防护保留已接受的剩余风险，不宣称消除了全部恶意输入性能问题。

## 给下一位 Agent 的备注

- 入口：`html/reader.rs` 的组合属性回调、`math.rs` 的公式约束、`parse_budget.rs` 的已知边界；勿用旧分支 reader 文件覆盖当前 URL 安全修复。
- 当前 RSS fixture 保真测试依赖仓库快照；CI 预抓取的新快照由 Pages 真实浏览器再次核对。
