# Rust 验收执行、失败恢复与订阅交互优化

- 日期：2026-09-22
- 作者 / Agent：Codex
- 分支：`main`
- 实现基线 HEAD：`1b5f8ebf8f7b50d576df2dc1e2ae4eec0ede4d23`
- 相关 commit：`17d9536`（刷新）、`d0d98da`（订阅）、`9d03652`（浏览器）、`2a1687a`（Rust 验收 / CI）；原实现阶段为 pending，现已分批本地提交
- 相关 tag / release：基线 v0.1.14；后续已获授权分批本地提交，未打 tag、push、触发远端工作流或发布
- 状态：`validated`（本地范围；wasm 实际浏览器、新远端 CI、Android / 原生实机仍未验）

分批提交清单与后续本地提交授权见[提交交接](2026-09-22-batched-local-commits.md)；以下保留各实施阶段的验证证据与平台缺口。

## 工作摘要

在已有 Home / Reader 与全局优化的 dirty 工作树上继续检查，没有覆盖或清理已有改动。重点处理验收产物身份和并发隔离、重复构建、失败资源清理、订阅提交状态及刷新写失败恢复。读取根 AGENTS / CLAUDE、相关目录说明和前轮交接；仓库没有 CodeGraph 索引，按当前源码核对。

保持原有分层和产品边界；没有新增 crate、依赖、通用 service 或平台业务状态。独立验收执行工具直接用 std-only Rust 编译，避免产品 CLI 承担开发流程、也避免为运行 wasm 契约先编译 native 产品依赖。浏览器 DOM 断言仍在现有 CDP 脚本，环境下载、启动和平台适配保留薄 Shell。

## 影响范围

- `scripts/wasm_contract_runner.rs`、`run_wasm_contract_harness.sh`：Cargo artifact 执行与浏览器隔离；三个原有单 harness 入口保留。
- `.github/workflows/ci.yml`、`setup-browser-tests` / `setup-wasm-bindgen` composite actions、`setup_chrome_for_testing.sh`：模块验收与工具缓存。
- `run_release_ui_regression.sh`、`run_web_spa_regression_server.sh`：去重、锁定依赖、超时和进程所有权。
- `rssr-infra` native refresh adapter 及其真实 SQLite 集成测试：字符串所有权转换、失败后的条件请求资格。
- `rssr-app` Feeds state / reducer / session / runtime / compose：重复提交、输入保留、读取结果次序。
- README、spec、command reference、主线与发布验证矩阵、现有 small viewport assertions。
- Desktop / Android 复用现有 native adapter 和共享 Rust UI；Web 复用相同 UI。没有将平台判断加入 application / domain。

## 关键变更

### Rust 验收与模块并发

- 原 wasm 脚本用 `find + mtime` 选择 `.wasm`，还会覆盖并删除 crate 内的 `webdriver.json`。现在由 Cargo target runner 提供实际编译产物，支持 `CARGO_TARGET_DIR`；Rust 为每个执行实例建立独立临时 webdriver JSON 和 Chrome profile，正常完成、测试失败、启动失败均清理。
- 支持一次 Cargo 调用选择多个 harness；原阶段入口仍接受原参数。artifact 层转发 runner 参数和进程状态，外层返回 Cargo 状态，失败不会误判成功。默认 test / driver 超时 60 / 15 秒，已有环境变量优先。
- 配置路径用 JSON / TOML 字符串正确转义，验证中文、空格、引号及反斜杠；不改用户已有 webdriver 文件。薄入口保留 PATH 优先级，避免系统另一个 wasm runner 版本覆盖 CI 选定版本。
- 新独立 `test-tools` CI job 运行 standalone Rust fmt、Clippy 与 5 项测试；纳入原 `lint-and-test` 全量汇总。native 六模块、wasm 三模块、UI 五主题的已有并发上限保持。
- wasm-bindgen 按 OS / 架构 / 版本缓存到独立工具目录，缓存后校验真实版本；不覆盖全局 Cargo 注册文件。Chrome 按精确版本缓存，UI 只装 Chrome，wasm 才装 ChromeDriver。版本解析失败保持非零，不输出伪成功的空版本。
- Chrome 元数据读取与下载增加有限超时；没有实际触发新 GitHub CI，缓存命中和 runner 调度效率仍待远端测量。

### 去除确定性重复工作，修复进程泄漏

- 发布预检同一进程同一 profile 只构建一次 Web 包，后续 smoke / server 复用；不持久化构建成功标记。
- 删除已被 app tests 覆盖的重复 native check / theme test；三个 infra harness 合并为一次 Cargo 调用。所有这些构建 / 检查补 `--locked`。
- 对真实脚本做 before/after 隔离调用对比：`--full` 路径中 Web 构建调用由 4 次降为 1 次，基础 gates 的 Cargo 调用由 8 次降为 4 次；`--full --no-serve` 构建由 2 次降为 1 次。失败退出、skip 和缺失产物拒绝均保留。
- 原部署壳 smoke 的 RETURN trap 在 `set -e` 失败时不会清理后台服务；实测失败退出 22 后子进程仍存活。改为函数 subshell 持有 EXIT trap，复测同一失败后子进程消失。HTTP 探测有 connect / total timeout。
- 独立复审另发现旧实例误验收：真实旧服务占用端口时，新进程绑定失败，原 smoke 却能借旧服务的登录 / 路由通过。已复现 exit 0，再加入启动前端口占用拒绝、ready / 最终成功的子进程存活检查；复测旧服务不被终止，新检查拒绝，释放端口后的新实例正常通过。证据 `host-instance-validation/summary.log`。
- 原 SPA 脚本退出时 Python 子进程仍监听；真实启动 / 终止前后复现。最终 Python 使用 `exec`，上层所持 PID 就是服务进程；终止后端口关闭。未触碰其它既有开发进程。

### 刷新正确性与低复杂度性能优化

- SQLite refresh adapter 已拥有 `ParsedEntryData` 批次，却再次深 clone 标题、URL、摘要、双正文；现在消费 Vec 并移动字段。没有新增索引或缓存失效机制，仍是一轮线性转换。
- 扩展真实 SQLite 落盘字段断言，覆盖中文空格、身份、URL、作者、摘要、HTML / text 与时间。
- 另复现写失败恢复缺陷：用真实 SQL trigger 分别让索引库、正文库写入失败，原实现随后发送失败响应的新 ETag，收到 304，正文一直停留在旧值。修改前 2 项恢复测试失败。
- 最小修复仅在已有缓存且 `fetch_error` 为空时使用条件请求。失败响应 metadata 和旧 `last_success_at` 继续保留；重试完整抓取，成功清错误后自然恢复 ETag / Last-Modified。两项测试都覆盖这个完整闭环，未改变双库事务边界。

### 订阅操作体验

- 添加订阅及首刷以 `Option<submitted_url>` 表示 pending，在创建 future 前同步去重；按钮显示“正在添加…”、disabled / aria-busy，地址仍可编辑。
- 成功只清空与提交快照相同的地址；新输入保留，失败释放 gate 供重试。部分保存 / 首刷失败也正确更新列表。
- Feeds 读取以 query generation 拒绝迟到的旧快照与旧错误，写操作完成不被该 gate 丢弃。
- 标量读取不再复制整个 feeds state；没有对这个细节单独宣称性能百分比。
- 5 项新增 UI Rust 测试；既有 browser assertions 暂停实际首刷请求，验证重复提交、忙碌反馈和下一地址保留。默认视觉体系保持，未进行未经用户验证的视觉重设计或心理学效果宣称。

## 性能证据

adapter 字段转换：相同 native release 二进制前后各保留一份；800 / 2000 条同一 fixture，预热 5 次，每样本 40 次，交替前后各 7 轮。下表为样本中位数：

| 条目 | 前（μs） | 后（μs） |
| --- | ---: | ---: |
| 800 | 4838.463 | 16.883 |
| 2000 | 11887.147 | 48.253 |

该探针只量字段转换，排除 fixture 构建 / 销毁、网络和 SQLite I/O；不外推整轮刷新、内存峰值、帧率或移动端收益。证据 `target/global-quality-round2/refresh-mapping/summary.json`、前后二进制及原始样本。

本地发布基础 gates 另做一次预热、前后交替各 3 轮：前 20.770 / 18.512 / 14.703 秒，后 14.636 / 25.541 / 6.289 秒；中位数 18.512→14.636 秒，但波动很大，不据此承诺稳定加速比例。确定收益是上述构建 / Cargo 调用次数减少。记录 `gate-timing/summary.json`；这不含完整 browser Campaign 或真实远端 CI。

## 验证与验收

主要日志在 `target/global-quality-round2/`，runner 专项证据另在 `target/quality-round2-runner/`。GUI 本地串行，fixture / profile / auth state 隔离。

- `cargo fmt --all --check`：通过。
- `cargo clippy --locked --workspace --all-targets -- -D warnings`：通过。
- `cargo test --locked --workspace`：270 passed、0 failed、2 ignored（显式性能探针）；不是把 stub 计入业务测试数量。
- Feeds targeted 16 项、infra targeted 4 项与完整 infra 87 项：通过。
- std-only runner rustfmt / 普通与 test 模式 Clippy、5 项 Rust 单测：通过；并发真实 stub 子进程、失败退出和清理均覆盖。
- 实际 Cargo 临时零依赖 fixture 构建两个 wasm targets、自定义 target 目录、中文引号 runner 路径，精确 artifact 均交给 stub runner：通过。该测试不等于真实浏览器 harness。
- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown`：通过；三个 wasm harness 的 locked `--no-run` 编译通过。
- Web release bundle（Dioxus 0.7.9、locked、debug symbols false）：通过，`web-build.log`。
- 现有 small viewport smoke：默认及四主题各 103 项通过，360×800 / DPR 3 + 1280×800；console errors / ignored errors 均为 0。查看默认 feeds 和 reader desktop 截图，未见新增布局溢出或位置回归。
- 真实 `rssr-web` 二进制：同一个 smoke 函数执行登录、重定向、鉴权页面、session probe、登出，结束后端口关闭；仅将 cargo 启动换成已构建二进制以避免干扰 gates 计时，未用假 host。
- 聚合脚本调用计数、失败 / skip / 缺产物、后台服务失败清理、真实 SPA listener 生命周期、缓存预热 stub 无下载和版本失败传播：通过。
- 真实旧 host 占用端口误验收的 before/after 复现、修复后拒绝旧实例且保留它、全新 host 成功与退出清理：通过。
- actionlint 1.7.12：3 个工作流 0 errors；composite action 的 inline Bash 另经 bash -n / ShellCheck。可选 pyflakes 未安装，不计为通过。
- JS `node --check`、修改脚本 bash -n / ShellCheck、CI 汇总 success / failure / cancelled / skipped、`git diff --check`：通过。

## 结果与风险

- 工作树已按用户后续授权分批本地提交，可按上方 commit review；未推送、发 tag、发布或触发远端 workflow。
- 新 GitHub CI 的调度、实际缓存和 artifact 服务尚未运行，不将本地检查等同于远端部署成功。
- 本机 wasm-bindgen-test-runner 是 0.2.128，而 Cargo.lock 要求 0.2.126，且缺 Linux ChromeDriver。尝试从官方源下载匹配工具被自动审批拒绝（需要审批，但当前策略禁止请求审批），未绕过重试；真实 wasm browser harness 未运行。
- Android target / NDK 未安装；Android 长按 / 系统返回、Windows / macOS 原生 UI 未实机确认。Web 仿真不替代这些结论。
- 未重跑完整公网 proxy / Release / Docker Campaign；本轮没有发布授权变化。
- 本轮恢复修复不承诺两个 SQLite 数据库提交之间进程崩溃的原子性；添加订阅仍属页面任务，未改变离页取消语义。全局手动刷新仍保留 App 生命周期。

## 给下一位 Agent 的备注

- 先看 Rust runner、两个 setup actions、Feeds session 和 refresh adapter；不要把工具执行职责塞进产品 CLI 或把 Python fixture auth 塞入部署服务。
- 本轮性能基线是开始本轮时的 dirty 状态，不与前轮 Entries groups / Arc 数字混合。
- 首次新增浏览器断言因漏匹配已有 cache-busting 查询后缀而超时，修正匹配范围后五主题均通过；最终证据以 `ui-final-*` 为准。
- 原有脚本 / 产品源码仍包含前几轮未提交修改，不能仅按 git diff 的全量行数当作本轮增量。
