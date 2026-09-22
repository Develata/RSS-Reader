# Home / Reader 交互复审修复

- 日期：2026-09-22
- 作者 / Agent：Codex
- 分支：`main`
- 实现基线 HEAD：`1b5f8ebf8f7b50d576df2dc1e2ae4eec0ede4d23`
- 相关 commit：`d0d98da`（共享 UI）、`9d03652`（浏览器验收）；原实现阶段为 pending，现已分批本地提交
- 相关 tag / release：基线 v0.1.14；后续已获授权分批本地提交，未打 tag、push、触发远端工作流或发布
- 状态：`validated`（Linux Rust 检查和 Chromium Web 验证；实机缺口见下文）

分批提交清单与后续本地提交授权见[提交交接](2026-09-22-batched-local-commits.md)；以下保留各实施阶段的验证证据与平台缺口。

## 工作摘要

针对“review and fix”重新审查上一轮未提交的 Home / Reader 改动，保留所有已有工作。复读根 AGENTS / CLAUDE 和就近约束，按刷新并发、DOM bridge 生命周期、UI / 验证三部分独立复核，再以源码、真实任务测试和浏览器结果判断。确认并修复两项 P2 问题，没有因未经证实的图片清理猜测改写桥接。

## 影响范围

- `rssr-app/src/pages/entries_page/{session,mod}.rs`：共享页面查询结果顺序。
- `assets/styles/shell.css`：共享搜索展开动画。
- 既有 `rssr_small_viewport_assertions.mjs`：加强真实浏览器覆盖。
- command reference、小视口说明、发布覆盖矩阵及本交接记录。
- Desktop / Web / Android 共用 Rust / CSS；domain / application / infra、平台 adapter、依赖和 Cargo.lock 本轮复审均未改动。

## 关键变更

### P2：较早的列表查询可能覆盖新筛选结果

刷新 revision 重取列表与用户切换搜索 / 来源会产生并行 `LoadEntries`。原 helper 的 resource 只完成 spawn，并不持有查询 future；较早查询 A 可以晚于 B 完成并无条件发布列表或错误状态。

- 页面 session 新增查询 generation，查询开始前登记，完成后只接纳最新请求的全部 intents。
- 校验后到发布期间没有 await；错误和成功遵守同一规则。只读查询继续随页面卸载取消，全局手动刷新仍属于 App scope，写命令维持原生命周期。
- generation 的写入和 `peek` 不建立响应式订阅，不会触发查询自身循环。
- 实际 Dioxus `VirtualDom` / spawn 与 oneshot 控制 B→A 完成，覆盖旧成功和旧失败晚到。没有用纯计数器测试代替真实发布链路。
- 修正 snapshot 注释，明确 entries / feeds 已共享 Arc，小字段仍按 state clone。

### P2：搜索展开动画声明无效

`--transition-quick` 自带 `120ms ease`，原 animation 后面又接 `ease-out`，展开后含两个 timing-function，被浏览器忽略。删除重复项。新增 computed-style 检查在修改前真实失败（`animationName: none`），修复构建后为 `search-reveal`；保留 reduced-motion 规则。

### 验证盲区与图片生命周期

- 自动 / 手动刷新合并现在检查整轮完成后两个 fixture feed 各请求一次，避免“先等待再排第二轮”的错误实现假通过。
- Home、固定分页和图片增加 CDP 实际 touch tap / hit testing，覆盖 DOM `.click()` 无法识别的遮挡问题。
- 增加图片打开时 history back、forward 后连续三次开关，检查 body lock、目标页滚动、正文滚动及焦点。
- 此路径实际通过，未确认 viewer cleanup 缺陷，因此没有修改图片桥接。最初回退测试从后续分页记录 846px，但重新挂载的首页更短、只能滚到 402px；使用稳定且小于目标页高度的 120px 后，回退恢复为 120px。此前失败不能证明桥接覆盖了目标页滚动。
- 复用既有 smoke / fixture，没有新增孤立 QA 入口。

## 验证与验收

本轮完整新日志和截图在 `target/home-reader-review-fixes/`；以最终目录为准，中间失败保留供追溯。

| 命令 / 路径 | 实际结果 |
| --- | --- |
| `cargo fmt --all --check` | 通过，`fmt.log` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | 通过，`clippy.log` |
| `cargo test --locked --workspace` | 252 passed、0 failed、1 ignored（原显式性能测量），33 组；`workspace-tests.log` 包含两项新增查询乱序测试 |
| `cargo check --locked -p rssr-app --target wasm32-unknown-unknown` | 通过，`wasm-check.log` |
| `/tmp/rssr-dx-0.7.9/dx build --platform web --package rssr-app --release --locked --debug-symbols=false` | 通过，最终 `web-build-final.log` 无构建警告 / 错误 |
| `bash scripts/run_static_web_small_viewport_smoke.sh --release --skip-build --port 8144 --chrome-bin /home/deve/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome --log-dir target/home-reader-review-fixes/lifecycle-offset` | 默认主题 93 项通过；360×800 / DPR 3、1280×800；console errors 和 ignored errors 均为 0 |
| 同上，`--preset atlas-sidebar --port 8145 --log-dir target/home-reader-review-fixes/atlas-final` | Atlas Sidebar 93 项通过；console errors 和 ignored errors 均为 0；与默认主题串行执行 |
| `node --check scripts/browser/rssr_small_viewport_assertions.mjs`、`git diff --check` | 通过 |

- 新截图复核：默认主题 image viewer、Atlas Reader 搜索；实际触摸、鼠标拖选和 Ctrl+C / Ctrl+A 由既有脚本执行。
- Native 检查沿用已存在 `/tmp/rssr-native-deps/env.sh` 的临时 sysroot，未改系统包或依赖。
- 没有再次测量性能：本轮仅查询结果采纳、CSS 和测试修复，不宣称新增性能收益；此前微基准及其局限仍见[实现交接](2026-09-22-home-reader-shared-interactions.md)。

## 结果

两项已确认问题修复，新增确定性测试和既有浏览器门禁通过；架构边界、共享刷新语义和平台职责保持原状。原复审结束时所有改动未提交；现已按用户后续授权分批本地提交。

## 风险与后续事项

- 未运行 Android 构建 / 实机验收：重新核实仅安装 native 和 wasm targets；Android target / NDK 缺口沿用上一轮。Android 选择手柄、系统返回、pinch 和原生桌面 WebView 的行为仍待对应环境确认。
- 未重跑全主题矩阵、rssr-web 本地 feed smoke 和 release 聚合：本轮未修改这些实现，已重跑受影响的默认 / Atlas、Rust workspace 和 Web 构建。上一轮聚合的公网 proxy 项被环境 DNS / SSRF 拒绝，不能声称全 release campaign 已通过。
- 列表从 Reader 返回后会按现有页面状态重新挂载；本轮没有扩展为跨路由分页状态持久化。
- generation 防止旧查询覆盖结果，不是取消底层已开始读取、数据库事务或跨页面写入互斥机制。

## 给下一位 Agent 的备注

- 查询顺序检查入口是 `EntriesPageSession::spawn_entries_query`，不要把该 gate 扩大为取消全局刷新或写命令。
- 最新浏览器通过目录是 `lifecycle-offset/` 和 `atlas-final/`；`baseline/` 的动画失败是修复前证据，`default/` / `lifecycle/` 是完善测试输入过程中的失败。
- 上一轮整体设计、性能基准和已有平台缺口见[实现交接](2026-09-22-home-reader-shared-interactions.md)。
