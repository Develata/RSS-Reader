# 全局工程质量、并发 CI 与阅读体验优化

- 日期：2026-09-22
- 作者 / Agent：Codex
- 分支：`main`
- 实现基线 HEAD：`1b5f8ebf8f7b50d576df2dc1e2ae4eec0ede4d23`
- 相关 commit：`61e42d6`（CLI）、`98ff3f9`（设置）、`d0d98da`（共享 UI）、`9d03652`（浏览器）、`2a1687a`（CI）；原实现阶段为 pending，现已分批本地提交
- 相关 tag / release：基线 v0.1.14；后续已获授权分批本地提交，未打 tag、push、触发远端工作流或发布
- 状态：`validated`（本地 Rust / Web；远端新 CI 和实机未验）

分批提交清单与后续本地提交授权见[提交交接](2026-09-22-batched-local-commits.md)；以下保留各实施阶段的验证证据与平台缺口。

## 工作摘要

在上一轮 Home / Reader 实现和复审的未提交工作上继续全局检查，保留原有 dirty / untracked 内容。复读根 AGENTS / CLAUDE、就近页面契约、功能设计与交接记录；仓库无 CodeGraph 索引，使用源码与 rg。按 CI、性能、UI、异步与 CLI 四个方向并行检查，选择可复现、可测量且不需要架构重做的问题实施。

本轮没有新增 crate、service、trait、依赖、平台业务分支或 SQL 分页；domain / application / infra 和 Cargo.lock 未修改。页面 session 管理异步归属和草稿，runtime 继续调用现有 application 用例，CSS 只管理呈现。

## 影响范围

- `.github/workflows/ci.yml`、`scripts/run_wasm_contract_harness.sh`：模块并发、真实 UI 门禁、锁定依赖和失败守卫。
- `.github/workflows/docker.yml` / `release.yml`：补跑 actionlint 后等价整理 Shell 写法，不改变触发、权限或发布条件。
- Entries `groups.rs` / presenter 性能探针：减少重复树查询和临时集合。
- Reader session / state / runtime：访问代际隔离，缓存更新不重载当前正文。
- Settings save session：提交快照与可编辑草稿分离，保存失败 / 并发行为。
- CLI main / 进程集成测试：结构化 stdout 与诊断 stderr 分离。
- 默认样式、Newsprint / Amethyst 主题、Feeds 原生 form、既有 browser assertions / fixture。
- README、spec、command reference、模块边界文档、主线及发布验收矩阵。

## 关键变更

### CI 按模块并发且只有真实成功才通过

- Cargo metadata 的 `workspace_members` 直接生成 6 个 crate 的 matrix，避免重复维护清单和路径选测漏项。每个 crate 独立执行 locked test（含 doc tests）和 Clippy；最多 4 个并行，只有 app 安装 GUI 开发库。
- format、Web bundle、3 个 wasm browser harness、Android 均为独立验收路径。Web 只构建一次；5 个 UI theme jobs 下载同一 public 包，复用原 360×800 / 1280×800 smoke，最多 3 个主题并行。
- 同 ref / PR 的旧 CI 继续取消，不同 PR 不互相取消；matrix `fail-fast: false`。全部 jobs 有显式超时、内容只读权限；native / wasm 缓存分别按包 / harness 区分。
- 保留 `lint-and-test` 检查名作为全路径汇总；failure、cancelled、skipped 均返回失败。该逻辑已用真实 inline Python 分别注入四种结果执行验证。
- Web artifact 保留 3 天，UI 断言、截图、日志保留 7 天，不上传 Chrome profile。
- Android 使用精确安装的 NDK 版本；SDK license 用 process substitution，避免 `yes` 的 SIGPIPE 被误判。原 `! unzip | grep` 在 errexit 下不会中止 step，已改显式 `if ...; then exit 1`，且先保存成功解包的文件清单。隔离 fake APK 验证 ARM64 通过，混合 / 错误架构拒绝。
- 没有修改 branch protection、发布授权或 release / Docker 的触发、权限与发布条件；后续 actionlint 补验仅整理两者的 Shell 写法，详见下文。真实远端调度和 artifact 服务尚未验证；读取的最新 main run 成功仅代表旧配置，不作为本轮通过证据。

### actionlint 安装后的补验与等价修正

- 用户安装后定位到 `/home/deve/go/bin/actionlint`，版本 v1.7.12；当前非交互 shell 的 PATH 不含该目录，直接用绝对路径运行，未修改用户 shell 配置。
- 首次全仓检查：`ci.yml` 通过；Docker 报未使用循环变量及 trap 回调可达性诊断，Release 报重复重定向，共 5 条。没有禁用 ShellCheck 规则。
- Docker 的 EXIT 清理改为内联 trap，重试改为 Bash 算术循环，仍最多 20 次；Release 将相同的六条环境变量写入合并为一次重定向，值和顺序不变。
- 用修改前后实际提取的 step 脚本执行隔离对比：Docker 立即成功、第三次成功、20 次失败、启动失败和清理失败均保持相同退出码、调用顺序和一次清理；Release 在中文空格 SDK 路径下，`GITHUB_ENV` / `GITHUB_PATH` 输出逐字节相同。Docker 调用使用 stub；不是容器或远端发布验收。

### 性能：保持相同语义，降低常数成本

- 原来源分组为条目和最新时间维护两个 BTreeMap；合并到同一个 bucket，减少每条重复树查找和最终回查。
- 来源 / 月等叶子直接构建最终 `Vec<EntryCardRef>` 并移交，不再先复制完整 `EntryGroupKey` 向量后立即转换丢弃。
- 保留原树分桶、来源按最新时间和名称排序、UTC 日期、绝对索引、目录 count / 页映射。时间复杂度仍是原有的分桶 / 排序量级；没有增加缓存失效机制或复杂索引。
- 两项回归测试覆盖最新时间相同、无日期、输入顺序、UTC 跨日和目标页。

相同主机 / native release、800 / 2000 条、40 来源、每页 50 条；保留前后二进制，交替执行各 7 次，每次 presenter 300 轮。以下为每轮耗时中位数：

| 输入 | 投影 | 前（μs） | 后（μs） | 降低 |
| --- | --- | ---: | ---: | ---: |
| 800 | Time | 666.214 | 605.207 | 9.2% |
| 800 | Source | 460.615 | 429.535 | 6.7% |
| 2000 | Time | 1519.460 | 1428.029 | 6.0% |
| 2000 | Source | 1065.922 | 899.626 | 15.6% |

完整样本和基线位于 `target/home-reader-review/perf-source-buckets-*`，汇总为 `perf-source-buckets-summary.json`。这是 CPU 微基准，有调度波动；本轮未测内存峰值、帧率、移动端耗时或远端 CI 总耗时，不外推这些收益。snapshot / input+eq 本轮未改，其纳秒级波动不作为新优化结论。

### 异步正确性和可用性

- Reader 的旧图片本地化原会发无身份 `BumpReload`；切换文章时可能清空并重载新正文。现本地化只更新缓存，下次打开使用。所有 Reader 异步结果按 `entry_id + load_generation` 验证，即使 A→B→A 也拒绝第一次 A 的迟到正文、错误和标记。
- Settings 原先只禁用底部保存按钮，主题入口仍可并发保存；旧完成结果无条件覆盖草稿。现在创建 future 前同步去重；成功只回填仍未编辑的提交快照，保留后续编辑 / 新选择的 preset；失败保留草稿重试，提示区分保存版本和未保存修改。
- CLI `export-config` 修复前 exit 0 却把初始化 INFO 写入 stdout，JSON 解析失败。现在日志写 stderr；新增实际子进程测试覆盖 JSON / OPML 导出再导入、show-settings、失败非零且 stdout 空、中文空格临时路径。
- 新增 9 项实际执行链路测试：Reader 3、Settings 3 使用 VirtualDom / oneshot 控制完成顺序，CLI 3 使用真正二进制。没有把异步正确性只验证为独立计数函数。

### UI 与具体使用体验

- 手机断点及 Newsprint / Amethyst 原来把正文字号写死，覆盖用户偏好；现在统一乘 `reader-font-scale`，保持缩放为 1 时的原外观。真实设置保存 1.25 后，默认手机 15.52→19.4px、桌面 17.28→21.6px，其他主题亦为 1.25 倍。
- 默认正文链接有下划线和主题色，长代码块局部横向滚动；实际 4484px 内容限制在 318px 代码块，根页面仍 360px。
- 显式键盘 focus ring、S / 设置当前页提示、筛选展开按钮至少 44px；保持原生选择和用户 CSS 覆盖能力。
- 添加订阅改原生 form：Enter 和按钮复用原命令，URL 输入采用合适键盘提示；刷新按钮明确不是 submit。
- 截图发现订阅页两个辅助统计卡在手机堆叠、挤占主操作位置；仅该区域改两列紧凑布局，360px 时地址输入底部为 373px，两个数值和长标签仍可读取。
- 沿用默认视觉体系，未以未经测量的心理学结论宣称用户效率提升；以上均有具体交互、几何或字号验证。

## 验证与验收

本轮新证据位于 `target/global-quality-review/`；性能证据路径另见上文。GUI 全部串行执行、使用隔离 profile 与 fixture，不触碰用户数据。

- `cargo fmt --all --check`：通过。
- 六个包各自的 `cargo clippy --locked -p <crate> --all-targets -- -D warnings` 和 `cargo test --locked -p <crate>`：全部通过，与新 native matrix 命令一致；`ci-rssr-*-{clippy,tests}.log`。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- `cargo test --locked --workspace`：263 passed、0 failed、1 ignored（显式性能探针），34 组含 doc tests。
- `cargo run --locked -p rssr-cli -- --help`：通过。
- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown`：通过。
- 三个 `wasm_*_contract_harness` 的 `cargo test --locked ... --target wasm32-unknown-unknown --no-run`：编译通过。未运行浏览器 harness：没有 chromedriver；不可把编译算作契约运行通过。
- `/tmp/rssr-dx-0.7.9/dx bundle --platform web --package rssr-app --release --locked --debug-symbols false --out-dir target/global-quality-review/web-dist`：通过，`web-build-final.log`；核对实际 public 目录与 CI artifact 路径一致。
- 默认 + 4 个内置主题的现有 small viewport smoke：最终目录 `ui-final-{default,atlas-sidebar,newsprint,amethyst-glass,midnight-ledger}`；每主题 100 项全部通过，360×800 / DPR 3 与 1280×800；console errors / ignored errors 均为 0。
- 新浏览器断言涵盖字体实际保存、链接可识别、代码不撑页、统计布局、Enter 添加一次和刷新不误提交；原刷新 / 图片 / 复制与全选断言继续执行。
- YAML 解析、全部 CI inline Bash 的 `bash -n` / ShellCheck、动态 crate matrix 输出、汇总 success/failure/cancelled/skipped、fake APK ABI 守卫：通过。日志 `ci-local-validation-final.log` / `android-abi-guard.log`。
- `/home/deve/go/bin/actionlint -verbose -color`（v1.7.12，ShellCheck 0.10.0）：全仓 3 个工作流通过，0 errors，退出码 0；日志 `actionlint-version.log` / `actionlint-before.log` / `actionlint-final.log`。可选 pyflakes 不在 PATH，Python lint 未执行；不把该部分记为通过。
- Docker 5 种前后行为对比、Release 环境文件字节对比、提取脚本 `bash -n`：通过，见 `actionlint-behavior-validation.log`；修改前后脚本保留于 `actionlint-fixtures/`。
- 修改 JS 的 `node --check`、shell `bash -n`、`git diff --check`：通过。

## 结果

现有结构上完成了有测量支撑的性能优化、异步正确性修复和具体 UI 改善；CI 的分模块并发配置可供 review，仍未提交到远端。没有重新设计核心、平台分叉、扩展产品功能或发布动作。

## 风险与后续事项

- Android target / NDK 和对应设备不可用，未重新运行 Android 构建或实机；Windows / macOS 原生 UI、Android 长按 / 系统返回 / pinch 仍待实机。
- 未触发新 GitHub CI，实际 runner 并发、缓存命中、artifact 上传下载、Android step 和 CI 总时间仍需提交后的远端运行确认。没有重跑整个 Release / Docker / 公网 proxy campaign。
- 初次下载 actionlint 曾被自动审批拒绝；用户随后自行安装，已完成上述全仓补验，该缺口已关闭。可选 pyflakes 仍未安装；新 GitHub 工作流、真实 Docker / Release 执行的缺口不因静态检查通过而关闭。
- Reader 新本地化缓存下次打开生效，当前正文维持原快照；这项取舍优先保证阅读位置、文本选区和 lightbox 状态。
- Settings 保存失败保留草稿，但仍是页面内草稿；没有增加离开页面的草稿持久化或新的确认流程。

## 给下一位 Agent 的备注

- 先看主线矩阵的 CI DAG、Reader / Settings session 和 source grouping；不要用新通用 framework 替代这些不同职责的小型策略。
- 性能前后是本轮当前 dirty 基线之间的比较，不与上一轮 Arc 改造数字混算。
- 旧记录中的 86 / 93 项及本轮中间 99 项不是最终统计布局版本的验收数量；以最终 `ui-final-*` 目录为准。
