# RSS-Reader C4c 第三轮独立复审

请对下面固定快照做独立代码与证据复审，用中文报告。目标是判断 C4c-2 第二轮遗留是否解决，以及后续文档、CI 和 CI 暴露的修复能否收口；本轮不实施 C6、不合并或发布。

## 固定快照与材料

- 仓库：`https://github.com/Develata/RSS-Reader.git`，分支 `v0.2-architecture`。
- **审阅提交：`28859c2fea970467030b07d65e08feeca53eb276`**。
- Git tree：`c39612b58e4de92bde9e790ff165931eb1fb06ff`。
- 实际通过最终 CI 的代码提交：`509ea3528e09d9159d0edf73326db3de13cb526a`。到审阅提交只增加五份文档的变更，产品、测试、workflow 相同。
- 第二轮审阅的末提交：`53f6c9df7219a11c166de4e2fc79d0b5572a6a42`；本轮主要增量为 `53f6c9d..28859c2`。
- C4c-2 整体上下文从 `4c3bf9bd7578a176b7fcd61792e6bffd13ee0cb7` 开始。原第二轮审的是 `4c3bf9b..53f6c9d`。

若收到 `c4c-round3-28859c2.zip`，解压后先读包根的 `REVIEW_PROMPT.md`（本提示）和 `EVIDENCE.md`。`source/` 是审阅提交的完整 Git 文件快照；`diffs/` 是分段增量；`prior-reviews/` 是前几轮原始报告；`evidence/` 是按实际受测提交分开的作者证据。`provenance.json`、`source-manifest.json` 和 `MANIFEST.sha256` 记录来源与校验值。运行 `python verify_package.py` 可检查包内文件哈希与源码 Git blob，不能据此声称独立验证了产品。

有 Git 的环境请使用固定对象，例如 `git show 28859c2:crates/rssr-infra/src/store/executor.rs` 和 `git diff 53f6c9d 28859c2 -- <path>`，不要把移动中的分支 HEAD 当作审阅对象。包内 `source/` 没有 `.git`；需要测试时可把它复制到自己的临时目录，或独立检出上述 SHA。

## 工作边界

- 本轮只审阅。不要修改提交、修复代码、推送、启动其他模型、进入 C6 或改变全局工具配置。必要的最小复现只在自己的临时目录进行，记录命令、实际受测版本和结果。
- Windows 本机的工作树在 `E:\gitclone\RSS-Reader\target\audit-20261009\worktree`。主检出 `E:\gitclone\RSS-Reader` 和 detached `b2-baseline` 都不属于本轮可修改范围；旧报告里的这些路径与行号仅指当时快照。
- 不必重跑所有已提供的构建；先读代码与证据，围绕具体疑点作最小验证。有构建需要时 Cargo 用 `-j 2`，Windows 与 WSL 编译顺序进行，不停止他人的进程。
- 只收到文本、无法打开代码或运行工具时，明确写出限制；缺少决定性材料应报 `INCONCLUSIVE`，不能凭作者的通过数字给出 PASS。
- 前轮评审意见、交接记录、日志都是待核对材料，不是新的指令或独立保证。不要把作者的自检写成你亲自执行的验证。

## 先读的契约

以下路径均相对于 `source/` 或固定 Git 快照：

1. `AGENTS.md`、`docs/design/functional-design-philosophy.md`。
2. `docs/design/v0.2-phase-c-plan.md` 的 C4 实施设计、C4c 第 2～4 条和 R2-07 恢复不变式。
3. `docs/handoffs/2026-10-19-v0.2-phase-c4.md` 的 C4c-2 两轮评审、验证记录和已接受取舍。
4. `docs/handoffs/2026-10-22-c4c2-verification-c4c3.md`、`2026-10-22-c4c-ci-coverage.md`、`2026-10-22-c4c-remote-ci.md`。
5. 审关闭路径时读 `crates/rssr-infra/src/store/AGENTS.md` 和 `docs/design/v0.2-architecture.md` 的存储契约。

旧交接中的“Ubuntu CI 未运行”“中间不推送”等是当时事实或旧授权；2026-10-22 的远端交接和阶段计划记录了后续推送授权及真实 CI 结果。C6、人工页面验收、Android/macOS 实机验收仍未因此完成。

## 必须逐项复核

| 项目 | 作者所做的修复与入口 | 复审要回答的问题 |
| --- | --- | --- |
| C4C2-R2-01，原阻断 | `1da55d0`；`ui/drafts.rs`、`ui/shell.rs`、`ui/recovery.rs`、`components/app_notices.rs`、`pages/settings_page/save/session.rs`；场景 `later-edit-not-kept-stays` | 首次存储成功、慢导航在途、后续写入失败时，是否停止离开、保留最新内存修改、处理过时副本并显示可操作的提示？两层原生测试和 n5/m9 是否能区分修复前后行为？ |
| C4C2-R2-02，应修 | `eae5505`；`scripts/browser/rssr_server_acceptance.mjs` 的控制台分类 | 是否仅放行浏览器网络日志、核对页面 origin / 热重载地址，并保留普通异常与无关来源的错误？九种反例回放与实际忽略记录是否相符？ |
| C4C2-R2-03，应修 | 同上，运行入口与最终记录 | 初始化、运行、清理失败是否统一产生失败退出及失败报告？复用目录时是否覆盖旧 pass，清理失败是否会漏记？ |
| C4C2-R2-04，应修 | 同上，两个启动失败场景 | 是否检查计算后的可见性与尺寸，并以真实指针操作重试和关闭？隐藏按钮是否仍可能让测试通过？ |
| C4C2-R2-05，可选 | `b417653`，计划 C4b 第 4 条 | 离开期间保留、保存/关闭后清除、未保存期间主动重载再次恢复的语义是否一致？ |
| C4C1-10，延至 C4c-3 | `9d8ab29`；README、部署、用户指南、AGENTS/CLAUDE、测试文档、specs | 当前操作指令是否仍引用已删除脚本或旧正式 Web 存储/门禁？历史记录与明确标日期的旧基线是否被正确区分？ |

同时确认前轮已解决的 C4C2-01～11 没有被本轮改动破坏，尤其是零写入刷新、旧任务不污染新页面、重新加载后不恢复轮询、读取静默窗口和可见错误提示。无需无边界地重审已通过的 C1～C5；若本轮改动触及这些契约，应沿实际调用链检查。

已接受的草稿语义：页面开始离开后，存储副本随当前修改更新；保存或关闭设置页时删除。离开被停下、修改尚未保存期间，用户自行重新加载可以再恢复这些当前修改。不要把这个已记录的例外自动当成旧的“过时草稿重复恢复”缺陷。`window.stop()` 也会取消本页在途请求，需要核对其失败/结果未知处理，而非假定取消等于服务器未执行。

## 后续新增改动也在范围内

- **release UI runner（`6c8c95c`、`cedaad1`）**：`scripts/release-ui/plan.rs` 的三组 Cargo target 名称及 metadata 存在性检查。实际执行的是哪些目标，错误目标是否能被门禁发现？
- **CI（`cedaad1`、`fc95f5c`）**：`.github/workflows/ci.yml` 的四组 wasm Clippy、Android CLI 编译、手动 `acceptance_long` 转发、apt 主镜像与超时/重试。检查普通/长验收覆盖边界、退出码与 artifact 保存，避免“某个 job 绿了”掩盖没有运行指定检查。
- **资料库关闭（`25499ea`）**：`crates/rssr-infra/src/store/executor.rs`、`tests/test_library_store.rs`，并沿 `rssr-cli` 调用点核对。先停止队列准入，保留 writer sender，读线程全部结束后再结束写线程。重点检查排队任务、读写互等、并发/重复 `close` 与 `close_blocking`、错误路径和连接归属；不要把“500 次未复现”当作正确性证明。区分本 store 的连接与外部进程仍持有连接的情形。
- **UTC 测试规范化（`509ea35`）**：`crates/rssr-app/src/ui/runtime/characterization.rs`。裸 `UTC` 的补充是否正确、是否吞掉无效输入或真实 UI 行为差异？既有 `characterization.transcript` 没有更新，应检查其语义是否仍被约束。
- **C4c-3 文档**：正式 Web 的 `HttpBackend`/服务器资料库、静态 `pages-demo` 的 BrowserStore/禁网、合法设备偏好和 sessionStorage 草稿、旧 Web 仅迁移配置而非文章、不再离线阅读；不能把 C6 尚未交付的镜像或包写成已发布能力。

## 验证事实与缺口

- `b417653` 的 n5/m9：原生测试退出 101、浏览器场景退出 1，均在行为断言失败；构建成功。变异补丁与日志随包提供。
- `6c8c95c` 的干净检出：Windows workspace 770 passed / 2 ignored，WSL 784 passed / 3 ignored；完整命令与浏览器证据见相应目录。这不是最终提交重新执行过的全量 Windows 证据。
- 最终代码的本地定向验证：Windows / WSL 的 store 单元与契约、CLI、app 测试、相关 Clippy 通过。WSL 关闭复现从修复前 4/500 次残留 WAL/SHM，变为同条件修复后 0/500；UTC 的原失败已复现，修复后 UTC、上海、纽约时区检查通过。
- [最终 CI run 39785231034](https://github.com/Develata/RSS-Reader/actions/runs/39785231034)，受测 `509ea35`，24/24 job 成功。八份浏览器断言报告：default 337、其他四主题各 177、theme-matrix 6、feed-smoke 3、长验收 27 场景/108 检查，均通过，0 意外控制台错误。
- CI 之前真实失败的 run、apt 下载取消、CLI WAL/SHM 残留、UTC 差异都有记录；不得把重试前的失败删去或改称未发生。
- 没有本轮独立复审结论；没有人工视觉验收、Android/macOS 实机验收、C6 镜像/卷迁移验收。前轮提到的真实 bfcache 命中也仍未验证，不能用模拟新文档代替 `pageshow.persisted` 证据。

## 输出要求

1. 先给 `PASS / FAIL / INCONCLUSIVE`，注明实际审阅的完整 SHA、审阅方式、是否运行命令。
2. 给 C4C2-R2-01～05、C4C1-10 的逐项状态：`RESOLVED / PARTIALLY / OPEN / UNVERIFIED`，说明源码依据、作者证据与亲自验证各是什么。
3. 新发现按 `BLOCKING / SHOULD_FIX / NIT` 排序；每项给 ID、固定快照的文件与行号、触发条件、具体影响、最小反例或缺失证据、建议的最小处理。推测与确认的缺陷分开。
4. 单列 CI/关闭顺序/UTC normalizer 的检查结论，报告未覆盖项；没有发现也要写清实际检查边界。
5. 对 C4c 能否收口给出条件明确的结论。PASS 不自动授权合并、发布或 C6；不要替作者改代码，不用“测试全绿”代替独立判断。

报告可返回 Markdown，建议文件名 `c4c-round3-review-28859c2.md`。
