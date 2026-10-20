# 阅读页正文深层嵌套的超线性消毒成本：调查与方案

- 日期：2026-10-20
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：claude/wonderful-engelbart-fdac91（基于 feat/reader-mathml 2ca9bd8）
- 当前 HEAD：见本文件所在提交
- 相关 commit：本文件（只含文档，无代码改动）
- 相关 tag / release：N/A
- 状态：`draft`（方案待 Develata 决定，未实现）

## 工作摘要

复现并定位了 `sanitize_reader_html` 在深层嵌套非公式 HTML 上的超线性耗时，确认它不止
“深 div 慢”，还包括一类**输出放大**（几十 KB 输入产生 MB 级输出、按平方增长），
并给出有界化方案对比。这是数据边界变更（会改变部分正文的显示方式），按任务要求先提方案、不实现。

## 影响范围

- 模块：`crates/rssr-infra/src/html/reader.rs`（`sanitize_reader_html`）；调用方
  `crates/rssr-app/src/pages/reader_page/support.rs`（`select_reader_body`）。
- 平台：全部。Web 端在 UI 线程同步执行，卡顿最明显；桌面/Android 同样阻塞打开文章。
- 额外影响：无（本次只有文档）。

## 关键发现

### 已验证（release，Windows，本机实测）

- 既有基准 `reader_html_stress_timings`：nested div 10k（110 KB）228 ms，50k（550 KB）7.2 s；
  plain 4 MiB 只要 61 ms。与父会话的 0.2–0.3 s / 8.5 s 一致。
- 时间几乎全在 ammonia 内部的 html5ever 树构建；本项目自己的 `normalize_html_for_live_display`
  在所有形状上都是线性的（20k 层 div 仅 9 ms）；公式后处理不受影响。
- 根因（读源码确认，html5ever 0.39.0 `tree_builder/mod.rs`）：`in_scope` 从栈顶逐个比较到作用域边界；
  `<div>`/`<ul>`/`<blockquote>` 等开始标签都会做 “p 是否在 button scope 内”，普通块元素不是边界，
  于是每个标签扫全栈。另有 `is_marker_or_open`、`remove_from_stack` 等线性扫描，以及“活动格式化元素表”
  上的 Noah's Ark 扫描。
- 形状扫描（每档规模翻倍，看耗时倍数；sanitize 全流程）：

  | 形状 | n=5k / 10k / 20k | 增长 |
  |---|---|---|
  | 嵌套 `<div>` | 46 ms / 244 ms / 1.01 s | 平方 |
  | 不闭合 `<div>` | 44 ms / 189 ms / 1.10 s | 平方 |
  | 嵌套 `<ul><li>` | 316 ms / 1.74 s / 4.71 s | 平方 |
  | 嵌套 `<blockquote>` | 57 ms / 263 ms / 1.39 s | 平方 |
  | `<span><div></span>` 错嵌 | 146 ms / 974 ms / 2.31 s | 平方 |
  | 嵌套 `<b id=k>`（属性各不相同） | 1.28 s / 5.38 s / 25.9 s | 平方（Noah's Ark） |
  | 嵌套 `<span>` / 相同 `<b>` / 嵌套 `<table><td>` | ≤ 150 ms | 线性 |
  | 平铺高亮 span 2.8 MB / 平铺段落 0.8 MB | 425 ms / 171 ms | 线性 |

- **输出放大**（格式化元素重建）：`<div><b id=k></div>` 或 `<p><b id=k>x</p>` 重复 n 次，
  n=250/500/1000 时输入 5.6/11/23 KB，输出 0.22/0.89/3.5 MB，耗时 28/118/457 ms，输出按 n² 增长。
  这是 HTML 规范行为（每个新块都要重建之前所有未闭合的格式化元素），按平方外推 100 KB 输入即可产生约
  70 MB 输出，8 MiB 上限内足以耗尽内存。**这把问题从“性能”提升到“可用性/正确性”。**
- 栈溢出风险：无。ammonia 4.1.3 的清洗、Drop、序列化都是显式栈迭代；`math.rs` 是平铺扫描。
- 真实数据（本机安装版内容库只读拷贝，9 个订阅、418 篇 HTML 正文、共 56 MB）：
  html5ever DOM 最大深度 p50=4、p99=10、max=16；最慢一篇 6 MB 正文 189 ms（线性部分）；
  全部 418 篇合计 2.1 s。58 篇（14%）超过 100 KB。
- 上游现状：servo/html5ever#788（2026-09-21，open）即本问题；修复 PR servo/html5ever#800
  （2026-10-14，open，REVIEW_REQUIRED，+872/−197）消除栈扫描的平方项，但作者明确说明
  **活动格式化元素表的搜索与 adoption agency 中间删除仍是平方**；且 ammonia 4.1.3 依赖
  `html5ever = "0.39"`，即便合并也要等 html5ever 发版 + ammonia 跟进。#289（2017，open）是同类总议题。
- 浏览器参照：Blink `kMaximumHTMLParserDOMTreeDepth = 512`、WebKit `defaultMaximumHTMLParserDOMTreeDepth = 512`，
  超过后挂到祖先下（展平），所以深于 512 的正文在所有目标 WebView 里本来就不会按原样渲染；
  但这个上限只决定挂载位置，不限制开放元素栈，浏览器自身对深层 innerHTML 仍是平方。

### 推断（未单独实测）

- wasm 上同一输入的耗时应是原生的数倍（未测量倍数），UI 线程卡顿更长。
- 真实数据只覆盖 9 个订阅，深度结论的代表性有限；但 512 处浏览器展平是一条独立的上界证据。

## 方案对比

所有方案都只在**渲染时**生效，不改存储内容；降级与否不影响 XSS 安全（降级产物是纯转义文本，
通过的内容仍由 ammonia 消毒）。

1. **按字节限制每篇正文**：否决。成本取决于形状不是字节数（110 KB 深 div 228 ms vs 4 MiB 平文 61 ms；
   23 KB 放大到 3.5 MB）。能挡住平方项的上限（约 20 KB）会误伤大量正常长文。
2. **等上游 / `[patch]` 指向 PR #800**：不作为当前方案。未评审的 +872 行进入消毒器关键路径、
   打破依赖锁定惯例，而且不解决格式化元素那一类（含输出放大）。建议跟踪 #788/#800，落地后可放宽本地阈值。
3. **自写线性预扫描（数标签算嵌套深度/累计深度）后整体降级**：实现小（约 60 行），真实数据零误报，
   probe 里的所有形状都能拦下（只在结束标签与栈顶匹配时出栈，可同时覆盖格式化元素那类）。
   但它是对 html5ever 的**模型**：raw text（script/style/textarea…）、外来内容（svg/math 里的 style 不是 raw text）、
   注释与转义状态上的任何分歧都可能被构造成绕过（例如让扫描器在 html5ever 看不到的结束标签上出栈）；
   不闭合的同类兄弟（`<p>`/`<li>`/`<td>` 不写结束标签）会被高估成深层嵌套，造成误报。
4. **用 html5ever 自己做一遍“计量预解析”后整体降级（推荐）**：实现一个只计数的 `TreeSink`
   （句柄是下标，不建真实 DOM），按小块喂给 html5ever 树构建器；计量
   “`elem_name`/`same_node` 调用次数（即栈扫描步数）+ 每个新建元素的 DOM 深度之和（覆盖 Noah's Ark 与重建放大）”，
   超过 `α·字节数 + c` 就停止并整体降级。优点：与 ammonia 用的是同一份解析器，分词/raw text/外来内容零分歧，
   计量的就是真实成本；上游修好栈扫描后计量值自然下降、阈值自动变宽。代价：约 150–200 行，
   正常正文多一次线性解析（估计为现有耗时的三到四成，6 MB 正文约 +60 ms，未实测）；`TreeSink` 接口会随 html5ever 升级变动，
   但 rssr-infra 已直接依赖 html5ever 0.39（`subscription_probe`），升级本来就要一起走。
   阈值用 418 篇真实正文的计量值标定，留至少一个数量级余量。

### 降级成什么（需要决定）

- (a) 返回 `None`：零新代码，但 `select_reader_body` 会退到 `content_text`/`summary`，而 `content_text`
  在本项目里就是 summary（`feed_normalization.rs:110`，真实数据平均 192 字符），用户只看到摘要；
  若摘要本身也是被拦下的 HTML，最终会把原始标签当字面文本显示。
- (b) 推荐：把**同一字段**整体转成纯文本（html5ever 分词器线性提取文本、解码实体、跳过 script/style），
  转义后包成段落仍以 `Some(html)` 返回，公开签名不变，调用方无需改动。不做部分截断。

## 验证与验收

### 自动化验证

- `cargo test -j 1 --release -p rssr-infra --test test_reader_html_stress --locked -- --ignored --nocapture reader_html_stress_timings`：通过（数据见上）。
- 形状扫描与真实正文统计用的是临时 probe（`tests/scratch_deep_nesting_probe.rs`），未提交，已删除。
- 无代码改动，未跑 fmt/clippy/wasm check。

### 手工验收

- 未执行（没有行为变更）。

## 结果

- 结论与方案待 Develata 决定；本分支只有本文件，可直接丢弃或合入文档。

## 风险与后续事项

- 未修复前：恶意或损坏的 feed 可以让打开某篇文章卡住数秒到数分钟，放大形状可能导致内存耗尽。
- 线性部分（数 MB 正文数百 ms）是既有成本，不在本次范围。
- 实现时应把放大形状、Noah's Ark 形状、`<ul><li>` 形状加入 `test_reader_html_stress.rs`，断言“降级为纯文本、输出有界”。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-infra/src/html/reader.rs::sanitize_reader_html`；html5ever 源码
  `tree_builder/mod.rs` 的 `in_scope`、`reconstruct_active_formatting_elements`、`create_formatting_element_for`。
- 真实正文标定：安装版内容库在 `E:\apps\rss-reader\...\RSS-Reader\rss-reader-content.db`，务必只读拷贝后使用。
- 本机内存小：release 构建用 `-j 1`；首次在本 worktree 构建时 rustc 编译 `h2` 曾崩溃（STATUS_STACK_BUFFER_OVERRUN），重跑即过。
