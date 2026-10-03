# PR19 目录状态回归与性能测量校正

- 日期：2026-10-03
- 作者 / Agent：Codex
- 分支：`test/directory-regression-baseline`
- 启动 HEAD：`9d1a4bf4ef56b426dc5f9ca48261f639e62b7af7`
- 当前 HEAD（文档提交前）：`282592f816c87746c8354ee7981b98be3db16dea`
- 相关 commit：`282592f816c87746c8354ee7981b98be3db16dea`（源码与工具）；本记录、报告及原始数据随其后的独立 docs 提交入库
- 相关 tag / release：N/A；同一 Draft PR19，不 merge
- 状态：`validated`（本地功能修复与测量流程；不代表性能目标全部达成或最终 CI 已通过）

## 工作摘要

复现并修复 feeds-only 元数据刷新误重置目录手动状态，补真实输入、提交等待及跨组观察测试，校正 PR19 性能测量的计时起点、工作负载和构建条件。目录独立浏览及主列表跟随属于既有产品意图；旧四项旧契约键盘失败继续保留，不能称为原需求下的四个真实 bug。

## 影响范围

- 模块：rssr-app 文章列表目录、既有 CDP 目录回归及显式性能工具。
- 平台：Web 和可见 Windows WebView2；其他平台限实际执行的构建/CI，不冒充真机验收。
- 文档：本交接及本轮校正证据；不修改 application/domain/infra 主语义。

## 关键变更与现有发现

- Web 时间/来源及 Windows WebView2 已复现：`ReadFilter::All` 下收起当前组、打开另一组，真实已读点击及来源未读数刷新完成后，manual/open 被重置。Web 有效失败证据：`target/directory-review-20261003/before-web-2/assertions.json`；Windows：`target/directory-follow/review-before-native/assertions.json`。最初 Web 探针的数字读取转义错误不计为产品证据。
- 目录 reset 身份与整个 presenter 的 Arc 身份分离，排除来源未读数等无关 metadata，保留页面/分组/目录结构变化。增加 bridge/DOM 等待入口；其范围只是已观测消息，不代表未来 I/O 或平滑滚动已完成。
- 连续跨页点击丢失的假设未复现，没有修改导航队列。初始鼠标探针只有首个目标收到点击，不构成产品失败；后续真实键盘双输入记录目标、事件时间及呈现 epoch。修复前 Windows 的零额外等待探针收到同一页/epoch 的两次输入并到达最后目标；修复后 Web / Windows 的四组有效双输入也都到达最后目标。正式契约等待第二控件实际可聚焦，所以 0/4/12/32ms 是最小附加等待，不是真实输入间隔或所有竞态已排除的证明。
- 旧性能工具的事件监听阶段与 rAF 排队次序不一致，因此撤回旧“一帧收益”和“未退化”比较结论。旧原始数据保留；新工具统一从可信 `event.timeStamp` 开始，由 DOM 变更/事件微任务观察完成，另记两帧代理，并单列新版 manual→follow。

## 验证与验收

### 自动化验证

- 上述修复前 Web / Windows 有效失败已保存，不重复复现。
- `cargo test --locked -p rssr-app directory -j 1`：7 pass，0 fail；新增重置身份测试覆盖元数据变化、真实分页与目录结构变化。最初测试草稿使用不存在的 `EntriesPageState::default()` 导致编译失败，改为已有的 `new(false)` 后通过。
- 修复后 Web 完整回归：278 pass，0 fail，console / ignored console error 均为 0，证据 `target/directory-review-20261003/after-web-1/`。
- 修复后可见 Windows WebView2 目录专项：115 pass，0 fail，console / ignored console error 均为 0，证据 `target/directory-review-20261003/after-native-1/`。窗口由隔离 fixture 启动，用后关闭。
- 两种分组均先确认来源未读数完成真实刷新，再等待 bridge 与已挂载消费者提交，断言完整 manual/open 集合及主列表、右侧、顶部目录位置不变；另外确认分组切换仍重置状态。跨组程序化滚动、真实双击、迟到滚动也使用明确提交等待。
- release 构建成功。B Web Wasm SHA-256 `9c70408925716d6a79c22f3747fcfcb9524bcc5cccff77e49502cdb26faa427a`；B Windows exe SHA-256 `fa5eb6d6fa2ffeea560adc055be7d16fdcafbd2da1ba74ea0dbbd5f05b6284c4`。Web dx 0.7.10 / Dioxus 0.7.9 仍报告版本差异，产物生成成功。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -j 1 -- -D warnings`：通过。
- `cargo test --workspace --locked -j 1`：335 pass、0 fail、2 个既有 ignored（文章投影与刷新映射的显式手动性能探针），不将 ignored 算成通过。
- 同工具链 A 构建完成：E 盘只读 `git archive 9815d4e8`，相对 `e8c6ee1` 的 crate 差异仅限 `cfg(test)`。A 本机 Web Wasm SHA-256 `976365428eafe6f5e44b851656e90d7737cae0704584d8cfc691a1f6222deac9`；Windows exe SHA-256 `9c8b490d03c30b9cd061ca5e0663391b92d7cc1fc84c4809b22478e3df0ee307`。
- 校正工具四组试采成功；正式 Web / Windows 各 6 批，共 6800 个样本，每路径每批 100 个，按 A1/A2/A3/B1/B2/A4 串行完成，无并行编译、测试或 GUI 自动化。所有正式批次及 console 检查通过。[本轮验证报告](../testing/directory-review-validation.md)与四个 `2026-10-03-directory-review-*` 数据文件保存方法、原始样本、逐批 median/p95、噪声及修复前后状态摘录。
- 新比较观察到小幅代价：Web 滚动高亮 DOM 中位数 +0.95–1.15ms；Windows +1.30–1.50ms，且 p95 +1.50–1.60ms。Windows Space 展开中位数 +0.20–0.80ms；导航中位数约 −4.50–5.15ms。不把代理值接近或个别路径改善写成整体性能不退化。
- 新版恢复跟随 DOM 中位数：Web 同组/跨组约 50.20–50.50ms；Windows 同组约 19.70–19.80ms、跨组 20.10ms。旧版缺少同一契约，仅报告 B 绝对值；不代表 photon latency。
- 最终含文档 SHA 的 CI 在推送后核对；结果维护于 [PR19](https://github.com/Develata/RSS-Reader/pull/19)，避免为回填结果不断改变受检 SHA。本记录不提前声称通过。

### 手工与平台边界

- Web 与可见 Windows 均由 CDP 自动驱动，未宣称人工或 Android 真机验收。响应式模拟、触摸/触控板惯性、辅助技术与 Android GUI 的限制见验证报告。

## 工作区与恢复

- 电脑 `LAPTOP-H6JEOCF0`，主目录 `E:/gitclone/RSS-Reader`，未新建 worktree。
- 断线后只读核验确认最后一次键盘导航补丁未写入；恢复后已补入。其余三个代码/测试草稿完整保留。
- 原 22 项 `.agents/`、`.specify/` 删除与 `.workbuddy` 保留。后者 SHA-256 `8b10390d734483195439c4c5c413849ea67e0f845747eb049ca8c115aa69df87`。
- 原有未提交 `2026-10-03-directory-follow-rust.md` 补记不纳入本轮改写，SHA-256 `ff51104f93327b65c8ded8951cb06febff32b17dd15db5303d92ed7328ce185e`；备份在 `target/directory-review-20261003/preserved-handoff.md`。
- 临时进程及产物集中于 `target/directory-review-20261003/`，仅可核实身份后清理本任务进程；不删除用户数据。

## 结果

本地状态回归修复、交互验收与测量校正完成。源码及工具已提交 `282592f816c87746c8354ee7981b98be3db16dea`；受测生产源码和测量工具的 LF 指纹逐文件核对该提交通过，证据提交仅改 docs。推送后的最终完整 SHA 与 CI 结果维护在 PR19。PR 保持 Draft，不 merge；性能不退化和内存下降没有被证实。

## 风险与后续事项

- 校正后已保留独立批次和尾部数据。小幅 DOM 代价及 Web 导航尾部波动不能隐去；没有预先约定性能差异预算，后续验收须与功能通过分开。
- 没有完整可比的内存证据则不作内存收益结论；旧 1200 样本总量不等于每路径每版本充足尾部样本。
- 未证实的导航假设不能写成事实。正式跨页契约的附加等待不是实测间隔；修复前原生早期探针未记录实际时间戳，只可证明两个可信目标在同一 epoch 被激活并到达最后目标。

## 给下一位 Agent 的备注

从本轮验证报告与已提交的回归摘录进入，不必重跑已经保存的失败复现。保留原 dirty 文件及旧四项历史键盘失败；最终完整 SHA 与 CI 链接以 PR19 收尾说明为准，不 merge。
