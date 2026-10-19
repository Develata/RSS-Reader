# 阅读页 MathML 公式保留、预算与 KaTeX 去重

- 日期：2026-10-19
- 作者 / Agent：Claude Code（Opus 5.5）；设计经 Codex（gpt-6-astra，high）两轮对抗评审
- 分支：`feat/reader-mathml`（基于 `origin/main` `e4c4a82`）
- 当前 HEAD：pending
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：`draft`

## 工作摘要

feed 正文里的 MathML 公式在阅读页被整段删除。原因是 ammonia 默认白名单没有 MathML，
而它的命名空间检查会把失去 `<math>` 父节点的 MathML 子树连文字一起删掉
（`<p>x = <math><mfrac>…</mfrac></math></p>` → `<p>x = </p>`）。本次保留公式并交给 WebView
原生排版，同时加上数值属性约束、渲染预算、KaTeX 双份输出去重和文本字段里的 `<math` 识别。
存储层保存的是消毒前正文，修复后重新打开即生效，无需迁移或重新订阅。

## 影响范围

- 模块：
  - `crates/rssr-infra/src/html/math.rs`（新增）：MathML 白名单、`attribute_filter`、公式后处理
  - `crates/rssr-infra/src/html/reader.rs`：静态 `LazyLock<Builder>`、流水线组合、保守退路、
    `looks_like_html_fragment` 的 `<math` 识别
  - `assets/styles/reader.css`：`[data-slot="reader-body-html"] math` 自身横向滚动
  - `crates/rssr-infra/tests/test_reader_html_stress.rs`（新增）：压力契约
  - `crates/rssr-infra/tests/wasm_reader_html_contract_harness.rs`（新增）：浏览器契约，CI 矩阵按
    `wasm_*_contract_harness` 命名自动收录，无需改 workflow
- 平台：Windows / macOS / Linux / Android / Web（消毒代码三端共用）
- 额外影响：docs（README、README.en、user-guide、functional-design-philosophy）

## 关键变更

### 消毒策略（`math.rs` + `reader.rs`）

- 消毒策略只构建一次（`static READER_SANITIZER: LazyLock<ammonia::Builder<'static>>`），图片策略原样继承。
- 白名单 = ammonia 认识的 MathML 展示元素（42 个，含 `menclose`、`none`、`mfenced` 等旧版展示元素）
  + `semantics`/`annotation`；只排除 `annotation-xml` 与 `mglyph`。原因：ammonia 会拆掉不在白名单里的
  已知 MathML 元素并提升子节点，拆 `menclose` 会让 `mfrac` 从 2 个子节点变成 4 个。被排除的两个拆掉后
  不改变可见结构（`mglyph` 无子节点；`annotation-xml` 的 HTML 子节点被命名空间检查删除，MathML 子节点
  落进 `semantics` 后不显示）。内容 MathML 不放行。不属于 MathML 的未知元素会被 ammonia 整棵删除，
  后处理无法察觉，这是已知局限。
- 属性：`dir`/`displaystyle`/`scriptlevel` 作用于所有 MathML 元素；其余按元素精确列出。不放行
  `style`、`class`、`href`、`mathcolor`、`mathbackground`、`mathsize`、事件属性。
- 数值约束（ammonia `attribute_filter`，同一次遍历，按 (元素, 属性) 分派，不影响 `img`/`td` 同名属性）：
  长度按 CSS 数字语法 + 每单位名义 em 折算，|值| ≤ 100em、百分比 ≤ 1000%；无单位只接受 0
  （`minsize`/`maxsize` 另接受 ≤ 10 倍）；`scriptlevel` 只接受绝对值 0–7 与 `+n`（`-n` 会逐层累积放大字号）；
  `mtd` 跨度 1–64；枚举属性按关键字校验。
- KaTeX：`span` 只保留 `katex`/`katex-mathml`/`katex-html` 三个 class；其它 class 被 ammonia 清成空
  `class=""`（无害）。

### 公式后处理（`math::constrain_formulas`）

- 只在 ammonia 输出含 `<math` 时运行；扫描的是 html5ever 0.39 规范序列化（注释已删、非 void 元素有显式
  结束标签、属性值和正文里的 `<` `>` 已转义），形状不符时返回错误，调用方退回不含 MathML 的保守策略。
- 预算以最外层 `<math>` 为原子单位：单公式元素 ≤ 2048、深度 ≤ 64、文本 ≤ 16 KiB；整篇按 MathML 保留的
  公式 ≤ 4096 个、元素合计 ≤ 100,000。超限公式整体替换为 `<code>` + TeX annotation（序列化后的转义片段，
  不解码）或 `alttext`，再不然显示占位文字，长度 ≤ 2 KiB。**阈值是实验参数，尚未用问题订阅源校准。**
- KaTeX 去重：`span.katex` 恰好两个元素子节点（`katex-mathml` 在前且其中 `<math>` 有元素子节点，
  `katex-html` 在后），其间只有空白时，删掉 HTML 副本并去掉三个 class；其它形状原样保留；与预算替换
  重叠时让位。

### 样式

- `math { max-width: 100%; overflow-x: auto; overflow-y: hidden; }`，不覆盖 UA 的 `display`。

## 验证与验收

### 自动化验证

本机内存紧张（可用提交约 3 GB），全部以 `-j 1` 执行。

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets --locked -j 1 -- -D warnings`：通过（首次报测试里一处无用 `format!`，已改，重跑通过）
- `cargo test --workspace --locked -j 1`：通过，43 个测试套件合计 390 passed / 0 failed / 4 ignored
- `cargo test -p rssr-infra --locked -j 1`（clippy 修复后重跑）：通过，184 passed / 0 failed
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked -j 1`：通过
- `bash scripts/run_wasm_contract_harness.sh wasm_reader_html_contract_harness`（headless Chrome，
  wasm-bindgen-test-runner 0.2.126，真实 chromedriver）：通过，4 passed
- 变异检查：临时停用 `reader.css` 的 `math` 规则后，`long_formulas_do_not_widen_a_narrow_reader` 失败
  （390px 容器被撑到 1693px），恢复后通过
- `cargo test -p rssr-infra --release --test test_reader_html_stress -- --ignored --nocapture`（计时，不作断言）：
  嵌套 mrow 1 万层 17 ms；单公式 20 万个兄弟 `mi`（2 MB）388 ms；6 万个公式 254 ms；纯文本 4 MiB 50 ms；
  嵌套 div 1 万层 209 ms、5 万层 9.59 s
- 未执行：其余三个 wasm harness（未改动它们覆盖的代码，交给 CI）；Android target check（未改移动端交互）

### 手工验收

- 真实问题订阅源：未执行（等待 Develata 提供订阅源）
- WebKit 系（macOS WKWebView、Linux WebKitGTK）与 Android WebView 实机：未执行

## 结果

- 自动化验证通过；待 Codex 对实际 diff 做正式评审，待用真实订阅源校准阈值。
- 用户影响：带 MathML 的正文重新打开即显示公式；KaTeX 公式不再重复显示。

## 风险与后续事项

- 阈值待用问题订阅源、KaTeX 输出与 LaTeXML 测试样例校准。
- 既有问题（与本次无关）：html5ever 对纯 `div` 深层嵌套有二次方作用域扫描，release 下 50,000 层
  （550 KB）消毒约 9.6 s、10,000 层约 0.21 s，远低于 8 MiB feed 上限即可触发；需单独决策。
- WebKit 主干的 MathML UA 样式仍是 `display: block`，溢出与 `semantics` 表现未在 WebKit 上验证。
- `overflow-y: hidden` 对特别高的公式是否会裁切，只在 Chromium 上目测了上标。
- `mathvariant` 只有 `normal` 在 MathML Core 里有跨端保证。

## 给下一位 Agent 的备注

- 入口：`crates/rssr-infra/src/html/math.rs` 模块注释，`reader.rs::sanitize_reader_html`。
- 本地跑 wasm harness：`main` 锁定 wasm-bindgen 0.2.126，本机全局 runner 若是 0.2.128 会因 schema 不符失败；
  用 `RSSR_WASM_BINDGEN_TEST_RUNNER` 指向 0.2.126 的 runner，并把真实 chromedriver 目录放在 PATH 前面
  （不要走 scoop shim）。
- Codex 两轮评审报告不入库，结论已并入本记录。
