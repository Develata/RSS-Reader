# B6 与 D5 指定提交范围的 Astra 评审

> 历史交接：本记录仅说明当时的实现与验收，旧路径和命令可能已退役。当前状态见[文档索引](../../../README.md)，归档范围见[历史索引](../README.md)。

- 日期：2026-10-12
- 作者 / Agent：Astra（gpt-6-astra，独立源码评审）；Codex 主代理（验收证据复核与记录）
- 分支：`audit/2026-10-09`
- worktree：`E:/gitclone/RSS-Reader/target/audit-20261009/worktree`
- 当前 HEAD：`0ebea13261d620ae6ae94e0075fe5b64f2561070`（未移动）
- 评审范围：`bdeeae4..0ebea13`
- 重点提交：`f7f28b4`、`d872fdc`、`8a64904`、`f48ef80`；`0ebea13` 仅改文档
- 相关 commit：无代码提交；本记录由 Claude 在 `0ebea13` 之后单独提交入库
- 相关 tag / release：未创建；验收材料包含 v0.1.21 Windows CLI
- 状态：`validated`（源码评审与留存证据复核，不代表完成发布验收）

## 工作摘要

**PASS / no blocker。在指定范围的最终源码中，未确认新引入、可行动的缺陷。**

依据最终版本的 `docs/handoffs/2026-10-09-audit-fixes.md` 中“订阅名称（B6）”与“风险与后续事项”的 B6 决议评审。核对了两轮设计原文和三轮实现评审；已接受的政策与兼容性后果未重复列为新缺陷。

代码通过 `git show <sha>:<path>` 读取指定提交；没有读取主工作区源码，也没有 checkout 或修改生产代码、测试和原有验收数据。主工作区位于另一条分支并有任务外改动，保持原样。按仓库交接规范，仅在既有审计 worktree 新增此记录。

## 影响范围

- 模块：B6 涉及的 domain 标题契约、application 添加与配置交换、infra 原生和浏览器存储、OPML codec、迁移 0009，以及标题显示消费点。
- 平台：原生与 Web 的共享语义和适配实现做了静态审查；本次留存二进制证据仅涉及 Windows CLI。
- 文档：仅新增本交接记录；无 workflow、依赖、tag 或 release 变更。

## 关键变更

### 源码评审结论

没有修改源码。Astra 核对以下链路，未发现新缺陷：

- 添加与刷新：用户名称和探测标题在同次激活操作中分别保存；原生与 Web 刷新只更新源标题，generation 条件仍有效。
- 显示名称：订阅摘要、文章列表、阅读页和 CLI 消费显示名称；JSON 导出不把 URL 兜底写成名称。检索各 `src` 中的标题消费点，未发现漏改。
- 配置交换：OPML 与 JSON 导入均明确设置或清除指定名称，覆盖软删除后重新激活；两端配置替换保留源标题。OPML 导出通过 `OpmlFeed` 独立携带显示名称和源标题。
- OPML：最终实现已删除名称与 URL 相等时的推断；空白名称回退、`<head/>`、属性转义和 `<body>` 外 outline 拒绝符合已接受契约。完整解码和 URL 校验先于写入。
- 升级：迁移 0009 仅新增 nullable 列，符合不回填政策。浏览器在同名锁内读取提交头，验证数据后发布 v2；基线旧代码在 mutation 与 cleanup 之前拒绝 v2。未发现静态协议反例。

### 决策边界

下列行为已在原 handoff 接受，本轮没有重新要求更改：旧名称不回填为指定名称；本应用导出后再导入会固化名称；同名订阅按既有 UI 语义合组；无名称 OPML 写空 `text`；外部阅读器可能优先使用源标题；OPML 不做完整 XML 良构性校验。

## 验证与验收

### 本次实际执行

- `git show <sha>:<path>`、指定提交 diff 与标题消费点检索：完成源码审查。原生配置替换、浏览器提交协议及基线旧写入者路径均已核对。
- `git diff --check bdeeae4 0ebea13`：通过。
- `git diff de62375 0ebea13 --name-only`：仅两个文档文件，未改变候选构建所对应提交之后的源码。
- PowerShell `Get-FileHash -Algorithm SHA256`：留存旧版与候选 CLI、下载 ZIP、两个 `legacy-original` 数据库的哈希，与原 handoff / `summary.json` 的记录相符。
- Python `zipfile` 与 `hashlib`：下载 ZIP 中的 `rssr-cli.exe` 和留存旧版可执行文件字节哈希一致。
- `git show 0ebea13:Cargo.lock` 配合 Python `hashlib`：CRLF 形式的 SHA-256 与候选 `provenance.txt` 精确一致。该记录注明构建提交 `de62375`、rustc 1.98.1；本次没有重新构建来验证源码与可执行文件之间的对应关系。
- Python `sqlite3`，URI 参数 `mode=ro&immutable=1`，打开前确认没有 WAL 文件：复核 `run-1` 下 9 组留存数据库。全部 integrity / foreign-key 检查通过；忽略三个新增列后，所有旧字段与正文逐值匹配 `legacy-snapshot.json`。旧库和恢复库为迁移 1–5，其余重试完成库为 1–9；升级库 `custom_title` 全为空、`generation` 全为 0、搜索标题回填与预期一致。
- `run-1/commands.json`：读取 22 条留存命令记录，其中 4 次迁移注入失败和 1 次拒绝降级均有对应错误输出。没有将失败后的最终重试库当作故障中间状态的独立复现。
- `b6-acceptance/results.json` 与 `commands.json`：结果和命令输出相符。旧版添加时即用源标题覆盖指定名称；候选版本在添加和刷新后保留“我的名称”，未指定名称的订阅随上游改名；旧库升级后仍按已接受政策跟随源标题。

### 留存证据位置

以下路径相对于主仓库 `E:/gitclone/RSS-Reader`：

- `target/audit-20261009/upgrade/run-1/summary.json`、`commands.json`、`legacy-snapshot.json` 和各场景数据库。
- `target/audit-20261009/upgrade/b6-acceptance/results.json`、`commands.json`。
- `target/audit-20261009/upgrade/candidate/provenance.txt` 与候选 CLI。
- `target/audit-20261009/upgrade/download/rssr-cli-windows-x86_64.zip` 与 `legacy/rssr-cli.exe`。

原始设计与实现评审位于：

`C:/Users/QQ/AppData/Local/Temp/claude/E--gitclone-RSS-Reader/6aff2ded-0a5e-4408-bb90-f7726407dfab/scratchpad/review/`

本次读取的文件为 `codex-b6-design.md`、`codex-b6-design-round2.md`、`b6u1-review.md`、`b6u2-review.md`、`b6r3-review.md`。这些是临时文件，不保证长期保留。

### 未执行

- 未重新运行 Cargo 构建、测试、Rust codec、浏览器 harness 或升级脚本；原 handoff 中的历史测试数量不算本次实测结果。
- 未重新访问 Release API；官方包来源核验沿用原记录，本次仅核对留存文件之间的哈希一致性。
- 未运行 GUI、Android 实机、其他平台包、签名或外部阅读器互操作验收。

## 结果

指定范围没有新增的评审阻塞项。现有 Windows CLI 升级与 B6 行为记录得到留存数据及命令输出支持；本次结论限于源码审查和证据复核，不扩展为全平台发布批准。

## 风险与后续事项

- 浏览器混合版本的两个交错场景仍未实测：旧版持锁提交后新版接续；升级完成后旧版在途刷新再次提交。目前只有静态时序分析与已有单版本测试，不新增“已验收”声明。
- 原 handoff 记录的临时替换 `Storage.prototype.setItem` 未以 `finally` 恢复，仍是既有测试隔离限制。
- 已接受政策及发布说明要点继续以 `0ebea13:docs/handoffs/2026-10-09-audit-fixes.md` 为准；本轮没有重启设计决策，也没有提出架构迁移。

## 给下一位 Agent 的备注

- 复核源码请继续固定 `bdeeae4..0ebea13`，不要把主工作区另一分支的内容混入本轮结论。
- 该评审没有生产代码补丁。新增交接记录 `commit: pending`；没有 commit、push、tag、发布或远端工作流操作。
- 若继续补证，优先补上述浏览器混合版本交错；如果进入发布阶段，再执行原验收指南要求的 GUI、Android 和最终包门禁。
