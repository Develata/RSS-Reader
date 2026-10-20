# 阅读页正文解析预算：超出预算整篇降级为纯文本

- 日期：2026-10-20
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：claude/wonderful-engelbart-fdac91（基于 feat/reader-mathml 2ca9bd8，未推送）
- 当前 HEAD：见本文件所在提交
- 相关 commit：本文件所在提交
- 相关 tag / release：N/A
- 状态：`validated`（Develata 选择方向 A：按纵深防御合入，写明剩余风险）

## 工作摘要

按 [调查记录](./2026-10-20-reader-deep-nesting-investigation.md) 中 Develata 批准的方案实现：
`sanitize_reader_html` 在交给 ammonia 之前，用 html5ever 自己按小块预解析一遍正文并计量树构建与
ammonia DOM 的成本；超出预算的正文整篇转成转义后的纯文本段落，仍以 `Some(html)` 返回，公开签名不变，
从不截取一部分按 HTML 显示。50k 层嵌套 `<div>` 从 7.2 s 降到约 0.25 s。三轮独立评审之后确认：从 sink 外部
计量 html5ever 0.39 给不出可证明的上界，按 Develata 的决定以纵深防御合入（v5），剩余风险写在下文与模块文档里，
根因提交上游。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/html/parse_budget.rs`（新）：计量 sink 与预算。
  - `crates/rssr-infra/src/html/plain_text.rs`（新）：只用分词器的纯文本降级。
  - `crates/rssr-infra/src/html/reader.rs`：`sanitize_reader_html` 先过预算。
  - `crates/rssr-infra/tests/test_reader_html_stress.rs`：降级契约与计时用例。
  - `docs/user-guide.md`、`docs/design/functional-design-philosophy.md`：行为说明。
- 平台：全部（纯字符串处理，wasm 同样编译）。只在显示时生效，不改存储内容。

## 关键变更

### 计量（`parse_budget.rs`）

- 与 ammonia 相同的片段上下文（`<div>`）；每块不超过 1 KiB 且不超过 8 个 `<`，块间检查预算，超限立即停止。
- sink 按 ammonia `rcdom` 的结构维护只有骨架的镜像树，计量：
  - `elem_name` + `same_node` 调用（开放元素栈与格式化元素表上的逐个比较）；外来元素名另按每 8 字节 1 计
    （外来内容结束标签按字节比较名字）；
  - `rcdom` 的线性操作：插到兄弟之前/从父节点摘下按父节点孩子数计（覆盖 foster parenting），
    `reparent_children` 按搬动数计，`add_attrs_if_missing` 按已有属性数计；
  - Noah's Ark：每新建一个格式化元素，按表中项数 + 同名项 ×（1 + 属性数）× 64 + 同名项属性字节 × 排序轮数计；
  - 块级包络（预付不经过 sink 的逐项扫描：adoption agency 按名字查格式化元素表、表格内文字缓存到下一个标签
    才逐项重建）：每块按 token 数上界 2 ×（`<`、`&`、LF、CR、NUL 个数 + 1）乘 8 ×（格式化元素表上界 + 1），
    表非空时再加开放元素栈上界；
  - 表与栈的大小用公开的 `TreeBuilder::trace_handles` 在块间精确清点（摊还到总工作量之内），两次清点之间
    按新建元素数放宽；
  - `selectedcontent`：ammonia 清洗时逐级向上找 `<select>`，按已建元素数计。
- 预算：工作量 ≤ 2^20 + 64 × 字节；物化量（元素名 + 属性字节，每个元素至少 3）≤ 64 KiB + 2 × 字节；
  开放元素栈与格式化元素表合计 ≤ 131072（容得下公式预算处理的 2 万层 KaTeX 包装；每块至多 8 个标签，
  超限前一块之内的扫描量不超过它的常数倍）。
- 镜像树逐层析构（含 `<template>` 内容），深层嵌套不会递归爆栈。
- 不按深度本身计费：外来内容（MathML）的深层嵌套对 html5ever 是线性的，仍交给公式预算单独降级。

### 降级（`plain_text.rs`）

- html5ever 分词器提取字符 token（实体已解码），丢弃 script/style 内容，块级标签处分段，转义后包成 `<p>`。

## 验证与验收

### 标定（release，本机，临时 probe 未提交）

- 418 篇真实正文（安装版内容库只读拷贝，9 个订阅）：0 篇被降级；工作量最高 6.4 / 字节（预算 64，余量 10 倍），
  物化量最高 1.0 倍（预算 2）；计量总耗时 0.57 s（同批正文消毒总耗时约 2.1 s）。
- 被拦下的形状（规模放大后均降级）：嵌套/不闭合 `<div>`、`<ul><li>`、`<blockquote>`、`<span><div></span>` 错嵌、
  不同属性的嵌套 `<b>`（Noah's Ark）、`<div><b id=k></div>` 与 `<p><b id=k>x</p>` 重建放大、大属性重建、
  `<table>` 下的 foster parenting、`selectedcontent`、`<html a=…>` 属性合并、`<span>`×400 后接大量 `</zz>`、
  表格内缓存文字逐项重建、adoption agency 按名字查表、超长属性名的同名比较、外来内容里长元素名的结束标签。
- 预算内放行且保持线性：嵌套 `<span>`/MathML、嵌套表格、表格内普通文字、平铺高亮 span（4.5 MB）、平铺段落
  （约 11 / 字节）、带缩进换行的表格与正文（约 12 / 8 每字节）、忘闭合 `<b>` 后每段重建（27 字节段落约 18 / 字节，
  60 字节段落约 8 / 字节）。

### 自动化验证

v5（本文件所在提交）的结果见下方“v5 验证”；以下为 v4（f035b66）：

- `cargo fmt --all --check`：通过
- `cargo clippy -j 1 --workspace --all-targets --locked -- -D warnings`：通过
- `cargo test -j 1 --workspace --locked`：426 通过，0 失败（含 `parse_budget` 9 个单元测试、
  `test_reader_html_stress` 16 个契约测试）。
- `cargo check -j 1 -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- `reader_html_stress_timings`（release）：本轮在机器高负载下测得，绝对值偏大（`plain 4 MiB` 354 ms，空闲时 61 ms），
  只看相对关系：nested div 50k 0.24 s（改前空闲 7.2 s）、nested ul>li 50k 0.37 s、distinct b 50k 0.19 s、
  `<p><b id=k>x</p>` 50k 0.31 s，均降级为纯文本；nested span 50k、nested katex 20k、各公式用例保持 HTML。
  计量对大正文的额外开销见标定数据（平铺高亮 span 4.5 MB：计量 0.30 s，ammonia 0.37 s）。

### v5 验证（本文件所在提交）

- `cargo fmt --all --check`、`cargo clippy -j 1 --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test -j 1 --workspace --locked`：427 通过，0 失败（`parse_budget` 10 个单元测试，含外来长元素名与按行切分的表格文字）。
- `cargo check -j 1 -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- `reader_html_stress_timings`（release，机器较空闲，`plain 4 MiB` 71 ms）：nested div 50k 0.18 s（改前 7.2 s）、
  nested ul>li 50k 0.30 s、distinct b 50k 0.13 s、`<p><b id=k>x</p>` 50k 0.19 s，均降级；nested span 50k 0.17 s、
  nested katex 20k 0.46 s 保持 HTML；平铺高亮 span 4.5 MB 1.08 s。

### 独立评审（Codex，gpt-6-astra，effort high）

- 第 1 轮（树构建之外的整条链路）：6 个 BLOCKER + 1 个 SHOULD_FIX，全部实测复现。属于树构建/DOM 阶段的
  foster parenting、Noah's Ark 属性乘数、`selectedcontent`、格式化深度过期四项在计量 v3 中修复；
  其余三项（见下方后续事项）不在本次范围。
- 第 2 轮（v3，commit 96c5e03）：3 个 BLOCKER，均为不经过 sink 的 html5ever 内部循环（表格文字跨块缓存、
  adoption agency 按名字查表、按字节比较属性名），由块级包络与按字节计的 Noah's Ark 修复（v4），
  三个反例的缩小版加入单元测试。
- 第 3 轮（v4，commit f035b66）：2 个 BLOCKER + 2 个 SHOULD_FIX，本机 release 缩小复现结果：
  - 活动格式化元素表里的 Marker 不经过 `trace_handles`，`<template><object></template>` 重复可留下任意多个
    Marker，之后每个 `</b>` 都跨过它们：**已复现，放行且平方**（1.14 MB 2.6 s，规模减半 0.64 s）。
  - 外来内容结束标签按字节比较长元素名，`elem_name` 只计 1：**已复现超线性**（2 MB 1.75 s，1 MB 0.48 s；
    是否放行未区分）。
  - 表格文字按 LF 切分 token、包络未计：缩小规模下**未复现**（约 25 ms 即降级）。
  - 长属性标签在长格式化表之后才比较：缩小规模下**未复现超线性**；按推算最坏约 1 s 后拒绝。
- 结论：从 sink 外部计量 html5ever 0.39 无法给出可证明的上界（Marker 对 sink 与 `trace_handles` 都不可见），
  继续逐项打补丁不收敛。Develata 选择方向 A：v5 只补外来元素名按长度计费、LF/CR 计入 token 上界两项并重新标定，
  第 3 轮的 Marker 问题作为已知剩余风险。

## 结果

- 按纵深防御合入：所有自然出现的超线性形状与三轮评审中可修的反例都被拦下，418 篇真实正文零误降级。
  “对已知形状有界，不是证明”已写入模块文档与 `docs/design/functional-design-philosophy.md`。
- 上游：[servo/html5ever#801](https://github.com/servo/html5ever/issues/801)（Marker 残留让格式化结束标签
  按表长扫描，附只依赖 html5ever + rcdom 的复现）；在 [servo/html5ever#289](https://github.com/servo/html5ever/issues/289)
  请求树构建工作量预算或中止钩子。分词器属性去重已有 servo/html5ever#796。

## 风险与后续事项

- 已知剩余风险（可绕过或拒绝前耗时较长）：
  - Marker 残留（`<template><object></template>`、`<div><object></div>` 重复后接大量 `</b>`）：放行且平方，
    1.8 MB 约 6 s 计量 + 5 s ammonia。根因在 html5ever（#801）。
  - 包络与 Noah's Ark 按块事后结算：一个块内或一个很长的标签内的超量要等它处理完才发现（推算最坏约 1 s 级）。
- 已知保守偏差：同一格式化标签大量不闭合（约 125 个以上的 `<font size=2>`）会被降级，尽管 html5ever 处理它是线性的。
- 计量额外开销：大正文约增加 15%–60% 的消毒时间（性能优先级最低，换取有界）。
- 独立评审（Codex）指出的、不在树构建阶段的超线性路径，本次不处理，另开任务：
  1. `normalize_html_for_live_display`：`<base href>` 很长时，每个相对图片地址都展开成完整地址，输出按平方放大
     （48 KB → 16 MB，已复现）；该函数也用于原生端刷新时的图片本地化。
  2. html5ever 分词器对同一标签内的属性去重是平方级（160 KB、2 万属性的单个标签约 1.6 s，已复现；
     上游 servo/html5ever#796），计量与纯文本降级都要经过分词器，所以本预算挡不住。
  3. `live_display.rs` 的属性实体解码对每个 `&` 向后找 `;`，无效时只前进一字节，平方级（已复现，40 KB 约 55 ms，按平方增长）。

## 给下一位 Agent 的备注

- 入口：`parse_budget.rs` 模块文档列出了每一项计量对应的 html5ever / ammonia 代码路径。
- html5ever 升级（尤其 servo/html5ever#800 那类修复）后计量值只会下降，阈值自然变宽；`TreeSink` 接口变化需同步。
- 标定用的真实正文：安装版内容库只读拷贝后用 `sqlite3 … writefile()` 导出，脚本不入库。
