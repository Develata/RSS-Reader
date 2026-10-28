# Reader 异步结果归属规则提取

- 日期：2026-10-28
- 作者 / Agent：Codex
- 分支：`feat/reading-workspace-20261024`
- 基线 HEAD：`f239620dc51552e7a701f9315995eb4de1c6aa5c`
- 相关 commit：实现、验收脚本与本文同批本地提交，以 `git log -1 -- docs/handoffs/2026-10-28-reader-result-lifecycle.md` 定位；记录生成时 `commit: pending`
- 相关 tag / release：N/A
- 状态：`validated`（下述源码、运行与编译范围）

## 工作摘要

承接 Entries 两批规则提取及验收复核，将 Reader session 中的异步结果归属判断提取为页面内普通 Rust 模块。复用现有文章 ID、加载代次和 reducer，保持加载、标记、图片本地化的任务作用域与发布行为，未引入通用页面运行时。

## 影响范围

- 生产：`crates/rssr-app/src/pages/reader_page/{mod,session,result_lifecycle}.rs`。
- 验收：新增 `scripts/browser/rssr_reader_lifecycle_acceptance.cjs`，复用离线 fixture、storage helper 和工作区 seed；同时支持显式选择的独立 Windows WebView2。
- 文档：`docs/design/native-ui-preparation.md`、`docs/design/ui-shell-bus-page-facade.md`、本记录。
- 共用逻辑覆盖 Web / desktop / Android；实际运行 Web 和 Windows，Android 仅编译检查。
- 未修改 application/domain/infra/host、页面状态字段、reducer、位置控制器、图片查看器、DOM 桥接、CSS、依赖或公开命令。

## 关键变更

### 归属规则与执行适配分离

- `ReaderResultScope` 仅捕获命令所属 `entry_id` 和既有 `load_generation` 两个标量；不以当前状态的文章 ID 替换命令自身身份。
- 加载仍先同步派发 `BeginLoading`，由原 reducer 增加代次和设置加载态，再捕获结果归属。其他命令共享当前访问的代次，成功标记不会作废同一访问中另一条命令。
- 任务完成后同时核对文章身份与代次，整批接纳或拒绝，包括没有自身文章 ID 的状态提示。切文、A→B→A 和同篇重载后的原结果均被拒绝；成功与失败使用同一规则。
- 拒绝时不遍历结果；接纳时按原顺序移动 intent 和正文，不额外收集、克隆正文或分配命令队列。返回迭代器不持有状态借用，发布前释放读取。
- session 保留 Dioxus spawn、命令执行与 intent 发布；原 reducer 按文章 ID 再检查正文/错误/标记的防线不变，页面销毁仍取消未完成 UI 任务。
- 拒绝迟到的 UI 结果不代表回滚已提交写入。图片本地化继续仅供后续打开读取缓存，完成后不替换当前 HTML；本批未改变该策略。

### 测试与真实页面验收

- 8 个普通 Rust 测试：接纳顺序及正文移动、跨文章的整批拒绝、A→B→A、同篇重载的旧成功/失败、ID 与代次必须同时匹配、同次访问命令互不失效且正文 Arc 保持、拒绝载荷不遍历、当前失败正常发布。
- 3 个 Dioxus 集成测试：加载身份/代次在首次轮询前同步可见、同篇重载隔离旧失败、页面销毁取消加载与命令任务。原有 3 个 Reader session 测试保留。
- 新脚本对桌面/窄屏执行按钮与大小写 M/F、当前文章标记持久化、正文节点与内容保持、未读导航、往返和返回列表；每次标记操作比对全体文章的 ID/已读/收藏/归档字段，Web 为 72 篇，Windows 为 5000 篇。
- Web 还延迟真实存储操作的返回，分别覆盖迟到成功/失败、A→B 与 A→B→A。先确认被延迟的确为目标命令（页面尚未更新，目标持久化值符合成功/失败），再验证旧状态提示和标记不能污染新访问；成功写入不会因 UI 结果丢弃而被撤销。
- 不存在文章的错误及返回恢复亦在 Web 运行；Windows 只运行正常阅读流程。真实图片下载、本地化失败和加载请求的宿主乱序未另做故障注入；加载乱序由普通 Rust/Dioxus 测试覆盖。

## 验证与验收

证据目录：`target/reader-lifecycle-20261028/`（ignored）。下列为本轮实际执行结果，不以旧 CI 或先前测试代替。

### 自动化验证

- `cargo test -p rssr-app --locked`：基线 191 单元 + 4 集成通过，候选 202 单元 + 4 集成通过；各有 1 个原有性能测试 ignored。新增共 11 项。
- `cargo test --workspace --locked`：500 通过、0 失败、2 ignored（原有 presenter 与 refresh adapter 性能测量）。
- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`：通过。
- `cargo check -p rssr-app --target wasm32-unknown-unknown --locked`：通过。
- `cargo check -p rssr-app --target aarch64-linux-android --locked`：通过。仅为子进程设置 NDK `28.2.13676358` 的 `aarch64-linux-android24-clang.cmd`、clang++、llvm-ar 和 linker，未安装到设备。
- `target/release-ui-tools/dioxus-0.7.9/dx.exe build --platform web --package rssr-app --locked --release --debug-symbols false`：候选优化构建通过，无 wasm-opt failure。dx SHA256 `7395c0bbbe17e6aa86215634cf511387fd1ff8afba58661d0ac81fab28eb9fed`，沿用既有 release 构建修复。
- `cargo build -p rssr-app --release --locked --target-dir target/reading-baseline-20261025/build-b`：Windows 候选 release 构建通过，已复制 EXE 固定身份。
- `node --check scripts/browser/rssr_reader_lifecycle_acceptance.cjs`：通过。
- `NODE_PATH=C:/Users/QQ/node_modules`，分别设置 `PUBLIC_DIR` 为基线 `target/entries-writes-20261028/candidate/public`、候选 `target/reader-lifecycle-20261028/candidate/public`，并设置独立 `ARTIFACT_DIR`，运行 `node scripts/browser/rssr_reader_lifecycle_acceptance.cjs`：两侧各 7 组通过，无 page/console error。最终回执为 `baseline/browser-final` 与 `candidate/browser`。
- 候选同样运行 `node scripts/browser/rssr_entries_lifecycle_acceptance.cjs`：12 组通过，无 page/console error，包含宽/窄屏搜索、刷新与正文返回列表页码；回执 `candidate/entries`。
- `target/reader-lifecycle-20261028/run-native.ps1` 分别使用 `-Variant baseline-final` / `candidate`，显式传入各自 EXE 和 SHA256：两侧各 2 组（1536×844、390×844）正常阅读流程通过，无未捕获页面/console error。脚本连接自己的 CDP 端口并传只读 SQLite 路径；实际回执在 `native-baseline-final/acceptance`、`native-candidate/acceptance`。
- 原生 helper 使用隐藏的独立进程、独立 WebView profile 和 5000 篇测试库副本；仅关闭可执行路径匹配的本轮进程。个人数据目录和用户桌面输入未使用；完成后核对端口 9276 已释放。
- 本批精确文件路径的 `git diff --check`：通过。

### 基线与候选身份

- 本轮基线 `f239620d` 相较 `f76b15c4` 只修改验收脚本和交接；生产源码一致。基线复用上一批固定的 Web/EXE，先验证生产源码哈希与产物哈希，再在修改 Reader 前运行本轮脚本。`baseline/identity.json` 明确记录这项复用，没有声称重新构建了基线。
- 候选为该基线加本批三份生产文件；`candidate/source.json` 记录其 SHA256，构建/验收后复核未变。
- 基线 WASM SHA256：`2b8e5416b386c8b1b78db888e29d57fe945d541c69da1726c40483e026010474`。
- 候选实际引用 `assets/rssr-app_bg-dxhd1737caafb525284.wasm`，SHA256：`6c391d1d08116bfebf1a4561af932eba1a0b2466c12eb50870caf63e89de973f`。`candidate/web-identity.json` 沿 index → JS → WASM 定位，不将残留旧文件作为候选。
- Windows 基线 EXE SHA256：`ced555f575e192c0e7eb3d7211ead1351e55f8b1cb5741047f2414e4376d1b8c`；候选：`99b45fa069ad91b387e6e4796e76ba49cc7f914e2f4562be1df51f54464cc889`。

### 视觉与未覆盖范围

- 已检查 Web 基线的两个视口截图；候选同位置 PNG 与最终基线逐字节一致。1536×844 SHA256 `9a00eeeb144b41e652587277b07e8c3ccb67761fbfc015ff55a394db1ddd1bbb`；390×844 SHA256 `9eef93e73339b97f51ad6612befac56417f3262568919a8e9325e05115ffd50c`。视觉结论限于这两个捕获位置。
- 最初隐藏的 Windows WebView 截图出现未绘制空白，未将其当作视觉通过。最终脚本只保存 Web 视觉回执，原生以实际 DOM 交互、正文节点和 SQLite 状态作为证据；原生窗口的前台视觉仍未验。早期记录保留于 `native-baseline`，最终相同步骤使用 `native-baseline-final`。
- 为确保迟到回复注入命中目标写入，增加了延迟期间的页面不变及持久化检查，并用最终脚本重新运行两侧基线/候选；早期 `baseline/browser` 保留，最终使用 `baseline/browser-final`。
- Android 设备、Linux/macOS、真实系统 IME、服务端 UI、外网订阅/图片下载、性能/内存和签名安装本轮未验。
- 未重跑目录全套、长文位置性能脚本或 Entries 的进程重启专项。上述局部重构不修改这些模块，不能把此前回执写成本轮新验收。

## 结果

Reader 结果归属规则已独立、全部入口接入，既有行为在列出的纯规则、Dioxus、Web 和 Windows 范围内保持一致。交接与实现一并本地提交，不包含 push、tag 或发布。

## 风险与后续事项

- `ReaderResultScope` 是既有访问身份的快照，不是新请求代次源。后续改动不能让普通标记操作递增加载代次，或让结果拒绝变成隐式重试/回滚。
- Reader 与 Entries 的状态、结果和取消语义不同；不要仅因都使用代次就引入通用 session 或共享业务状态。
- 状态借用必须在发布前释放，拒绝载荷仍不得遍历。任何后续生产源码变化应重新生成候选验收，不能直接沿用本次产物。
- 原有 Speckit 删除、旧交接修改及 Penpot/Figma 等未跟踪内容全部保留并排除在本批提交外。

## 给下一位 Agent 的备注

- 从 `result_lifecycle.rs` 的 8 个普通 Rust 测试和 session 的 3 个新增适配测试进入；代次分配及加载状态转移仍在原 state/reducer。
- 复测 Web 时使用本轮 final 基线，Windows 需新建隔离目录并使用明确 EXE 哈希。全体文章标记比对只覆盖 ID/已读/收藏/归档，不等于全部业务表或正文数据完整性审计。
