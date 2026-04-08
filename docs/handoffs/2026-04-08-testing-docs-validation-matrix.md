# testing docs validation matrix and environment limitations

- 日期：2026-04-08
- 作者 / Agent：Codex
- 分支：refactor/wasm-config-exchange-extraction-step2b
- 当前 HEAD：d2dc005
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

将当前主线分散在测试文件、手工回归记录和阶段性 handoff 里的验证结论整理为两份长期维护文档：主线验证矩阵与环境限制索引。

## 影响范围

- 模块：
  - `docs/testing/mainline-validation-matrix.md`
  - `docs/testing/environment-limitations.md`
  - `docs/handoffs/2026-04-08-testing-docs-validation-matrix.md`
- 平台：
  - Desktop
  - Web
  - CLI
- 额外影响：
  - docs
  - workflow

## 关键变更

### 主线验证矩阵

- 新增 `docs/testing/mainline-validation-matrix.md`
- 将 add/remove、refresh、config/exchange、reader rendering、web startup、paste/input、settings validation、remote pull cleanup 统一整理到同一张长期矩阵中
- 为每项补齐：
  - 自动化入口
  - 是否需要手工 smoke
  - 是否受环境限制
  - 通过标准
  - 最近一次验证来源占位

### 环境限制索引

- 新增 `docs/testing/environment-limitations.md`
- 将当前已知的非代码回归型环境限制整理为长期索引：
  - `test_webdav_local_roundtrip` loopback 端口权限限制
  - 纯静态 Web 路径下的 CORS 限制
  - WSLg 宿主窗口行为问题
- 为每条限制补齐：
  - 受影响入口
  - 触发环境
  - 现象
  - 不算代码回归的判断依据
  - 规避方式 / 复验方式

## 验证与验收

### 自动化验证

- `sed -n '1,220p' docs/testing/README.md`：通过
- `sed -n '1,240p' docs/handoffs/README.md`：通过
- `sed -n '1,260p' docs/handoffs/TEMPLATE.md`：通过

### 手工验收

- 文档内容与现有测试盘点结论一致性检查：通过
- UI / 浏览器 / Desktop 运行时手工回归：未执行

## 结果

- 当前主线验证流程已经有可长期维护的文档入口，不再只依赖阶段性 handoff 追溯
- 本轮仅新增文档，不涉及代码、测试逻辑或运行时行为变更

## 风险与后续事项

- `docs/testing/README.md` 目前尚未把这两份新文档列入“当前文档”，后续如要进一步收口导航，可补一轮文档索引更新
- `mainline-validation-matrix.md` 中“最近一次验证来源”目前仍是占位格式，下一轮实际验证后应及时回填

## 给下一位 Agent 的备注

- 如果继续做主线测试收尾，优先从 `docs/testing/mainline-validation-matrix.md` 开始执行
- 如果后续再遇到“本地失败但不像代码回归”的情况，先更新 `docs/testing/environment-limitations.md`，再写 handoff
