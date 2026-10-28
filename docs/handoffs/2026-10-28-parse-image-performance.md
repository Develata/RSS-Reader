# 正文解析性能与后台图片队列

- 日期：2026-10-28
- 作者 / Agent：Codex；Claude 只读复核
- 分支：`codex/parse-image-performance`
- 当前 HEAD / 基线：`6b2596d205b475c776a76cafb678233a9537b9f2`
- 相关 commit：pending
- 相关 tag / release：N/A；不改变已发布 v0.2.1
- 状态：`validated`（本地提交待回填）

## 工作摘要

按用户授权推进 M01/M02：先复现 HTML 解析慢点，再给当前 parser 添加小范围补丁；后台图片使用有界元数据队列和轮转，避免排队保留整批正文、失败前几篇长期占满每轮尝试。测试额外发现并修复了无引号图片地址被 `/` 截断的问题。

## 影响范围

- 模块：共享 HTML 处理、html5ever 本地补丁、application 刷新候选及源身份、infra 原生图片宿主及 SQL 读取。
- 平台：HTML 逻辑覆盖原生与 wasm；后台队列用于 Windows/macOS/Linux/Android/CLI/服务器原生宿主，浏览器不执行该队列。
- 额外影响：新增性能 probe、回归与维护文档；无迁移、无 UI/CSS 改动、无部署工作流变更。

## 关键变更

1. 0.39.0 的属性去重在大标签上切换到临时 HashSet；提供包括 Marker 的活动格式化表长。补丁仅更改上游两个源文件，许可证、源版本与移除条件保留。
2. 原预算纳入 Marker 和清点费用，格式化长属性在转交树构建之前预检；既有 MathML、raw text、文本转义和超预算整篇降级语义保留。开发中发现并修正过一次普通深层 MathML/span 的过度计费，相关旧回归恢复通过。
3. 刷新候选改为去重键、generation、源 URL 和两种指纹；持久 `rssr-content-v2` 哈希移至 application 的唯一纯函数实现，字节格式不变。
4. 图片模块从 native_host 提取。最多 2 个订阅执行、5 篇网络尝试/轮、180 秒保持；排队最多 64 个订阅/8 MiB 计费载荷，单任务最多 256 候选/256 KiB。仅执行时按需读取单篇正文，读取/写回均保留身份保护。
5. 失败、跳过和取消都能推进轮转位置；无图片请求不占尝试额度，失败不写伪派生正文。详细边界见[长期契约](../design/reader-processing-budgets.md)。
6. `live_display` 修正无引号 URL 的斜杠和自闭合判断，补绝对/相对 URL 与尾斜杠回归。

## 验证与验收

### 自动化验证

- Windows `cargo clippy --workspace --all-targets --locked -- -D warnings`：通过（首次指出旧候选测试字段失效，已修复）。
- Windows `cargo test --workspace --locked`：998 项通过，0 失败，4 项手工测量工具按声明跳过；覆盖新增的源 URL 变化、删除后恢复与持久哈希格式 canary。
- `html::parse_budget`：12 项通过；`test_reader_tokenizer`：3 项通过；`test_reader_html_stress`：24 项通过，1 项计时工具按声明跳过。
- vendored html5ever `--lib --test driver`：16 + 1 项通过。
- release `reader_html_probe`：基线/改后各 39 个独立进程样本通过，输入哈希一致；原始 JSON 与中位数见[证据](../testing/evidence/2026-10-28-parse-image/README.md)。40k 属性约 3520 → 64 ms；20k Marker 约 2352 → 364 ms。快照 6 篇输出哈希一致、597 个 MathML 保留。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：正式 Web 编译通过。
- `cargo test -p rssr-infra --target wasm32-unknown-unknown --test wasm_reader_html_contract_harness --locked`（使用项目 wasm runner）：真实 Chrome 的 9 项测试通过，覆盖公式布局、命名空间、Marker/长属性及无引号图片地址。
- `cargo fmt --all --check`：通过。暂存 diff 的空白检查通过，唯独排除按原样保留的上游 README 和 benchmark HTML（其原始行尾空白会被全量检查报出）；上游源文件补丁与本仓改动均纳入检查。227 条相关相对链接存在。准确命令（含 target-dir 和 wasm runner）、日志摘录、日志及源码哈希见[验证回执](../testing/evidence/2026-10-28-parse-image/verification.json)。
- 浏览器环境：Chrome 155.0.8059.40、隔离下载的同构建 ChromeDriver 155.0.8059.84、wasm-bindgen-test-runner 0.2.128。首次测试因既有 ChromeDriver 156 与浏览器不匹配失败；使用官方同构建驱动后通过，未更改用户浏览器安装。失败运行遗留的两个 Chrome 子进程按唯一临时 profile 定位后清理。

### 独立复核

- Claude session `ced3a501-ce51-49ce-b78c-aa83e0c8c3b9`：只使用 Read/Grep/Glob，未执行代码或修改文件。发现 1 个 P1：旧 application 测试仍引用被移除的 title/content_html 字段；与本机 clippy 发现一致，已修正为新身份断言。五个生产实现重点未发现可确认 P0/P1/P2。其未验证性能、语料、构建或上游来源，这些由上面的实际检查负责。
- Claude 复核 session `e4d4b4bc-eee2-464e-9cc3-3e579b09fe29`：确认原 P1 已关闭，本次范围未发现新的 P0/P1/P2；逐项核对日志汇总为 998 通过/4 跳过和浏览器 9 通过，检查最终 wasm cfg 补丁。它没有自行执行测试；指出日志缺少执行命令及运行前源码身份，因此收尾另存命令与执行后回执，不能把它当作独立重新运行或运行前签名。
- 本机逐文件 SHA256 比对 registry 0.39.0：除新增补丁说明外，仅 tokenizer/mod.rs、tree_builder/mod.rs 内容不同。

### 手工验收

- 无本轮桌面/Android 手工交互验收；未使用用户资料库。不得把浏览器契约或 Windows Rust 测试写成真机覆盖。

## 结果

- 实现与相称验证完成，本地提交待回填。未 push、未创建 PR、未触发远端工作流。
- 具体耗时改善已验证；不宣称任意 HTML 全链路线性或整个应用内存下降。

## 风险与后续事项

- 单 token 内的树构建仍不可取消；预算是纵深防御。上游 0.40.1 的两处热点仍在，未来升级按补丁说明重核。
- 后台队列是进程内尽力调度；饱和/同订阅去重会推迟工作，游标有淘汰且不持久，304 不触发新的候选轮次。若要持久续排、304 自主重试或跨进程公平性，需要单独设计，不在本轮暗中加入。
- 仍为每个含 HTML 的刷新文章计算紧凑候选，保留未成功图片的重试机会；不等同于“仅新增/变更候选”。
- 本轮真实内容仅仓库的 6 篇快照；历史 418 篇语料没有重跑。真实订阅规模、其他原生平台交互与长时间调度仍待观察。

## 给下一位 Agent 的备注

- 先看长期契约中的职责地图、字节预算与身份检查；不要把缓存调度游标改成第二套持久业务状态。
- 性能数据为固定夹具的三次本机采样，参见 JSON 的输入/输出/probe/二进制身份；runner 没有自动超时终止，不用它直接跑未知攻击语料。
- 任务只获本地提交授权。工作树开始干净，主仓库 `E:\gitclone\RSS-Reader` 的任务外改动未触碰；其 ignored target 仅用于构建缓存。
