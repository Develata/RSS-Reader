# 正文图片地址归一化的超线性路径：URL 展开预算与实体解码窗口

- 日期：2026-10-20
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：claude/wonderful-engelbart-fdac91（基于 feat/reader-mathml 9c23114，未推送）
- 当前 HEAD：见本文件所在提交
- 相关 commit：本文件所在提交
- 相关 tag / release：N/A
- 状态：`validated`

## 工作摘要

修复 [阅读页正文解析预算](./2026-10-20-reader-parse-budget.md) 的独立评审第 1 轮里发现、但不属于树构建阶段的两条
超线性路径。两者都在 `normalize_html_for_live_display`（本项目自己的代码）里，在解析预算之前执行，而且原生端刷新时的
图片本地化也调用它，归一化结果会写入正文库。

## 影响范围

- 模块：`crates/rssr-infra/src/html/live_display.rs`；调用方 `html/reader.rs`（阅读页）与
  `fetch/client/body_asset_localizer.rs`（原生端刷新时本地化正文图片）。
- 平台：全部。

## 关键变更

- **URL 展开预算**：相对图片地址按 `<base href>` 或 feed 地址展开，很长的基地址乘以大量很短的相对地址会让输出
  按平方放大（48 KB → 16 MB）。新增 `UrlBudget`：解析出的地址总字节数 ≤ 2 × 输入长度 + 1 MiB，按每个解析结果扣减，
  `srcset` 里的每个候选也计入，预算用完立即停止。超出时 `normalize_html_for_live_display` **整篇原样返回**，
  不做任何改写（不做部分改写）。后果是这类正文里的相对图片不会被展开，可能显示为裂图；文字不受影响，
  之后照常经过解析预算与 ammonia。正常正文每个图片地址最多展开一个 feed 地址长度，远在上限之内。
- **实体解码窗口**：`decode_html_attribute_entities_once` 原先对每个 `&` 向后找 `;`，实体无效时只前进一字节，
  一长串 `&` 共用末尾一个 `;` 时平方级。现在只在实体名可能的长度（32 字节）之内找 `;`；可解码的实体
  （`amp`/`quot`/`lt`/`gt`/`apos`/`nbsp`、十进制与十六进制数字实体，包括带前导零的写法）结果不变。
- `normalize_html_for_live_display` 按文档顺序收集改写，用 `Vec` 代替 `BTreeMap`。

## 验证与验收

- 新增单元测试：实体在窗口内解码（含多字节字符紧跟 `&`、超长名字原样保留）；URL 展开超预算时整篇不变、
  同一基地址少量图片仍正常展开；`<source srcset>` 的候选共享预算。
- 新增压力契约（`test_reader_html_stress.rs`）：长 `<base href>` × 16384 个相对图片、10 万候选的 `srcset`、
  100 万个 `&` 的属性值。
- 评审反例复测（release）：base-href × img 48 KB 输入的归一化输出由 16 MB 变为原样 48 KB（12 KB 输入在 1 MiB 余量内
  仍展开为约 1 MB，属常数上界）；`&` 串归一化 10 KB → 40 KB 输入为 0.4 → 1.7 ms（原 6 → 87 ms，按平方增长）。
- `cargo fmt --all --check`、`cargo clippy -j 1 --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo test -j 1 --workspace --locked`：432 通过，0 失败。
- `cargo check -j 1 -p rssr-app --target wasm32-unknown-unknown --locked`：通过。

## 结果

- 可合并。

## 风险与后续事项

- 超预算时相对图片不展开，Web/桌面会显示裂图；这是为有界付出的、只影响异常正文的代价。
- html5ever 分词器的属性去重（servo/html5ever#796）与格式化元素表 Marker 残留（servo/html5ever#801）仍在上游。

## 给下一位 Agent 的备注

- 入口：`live_display.rs` 里的 `UrlBudget` 与 `decode_html_attribute_entities_once`。
- `LocalizableImageDocument::parse` 只为前 `max_images` 个图片解析地址，本身有界，未改。
