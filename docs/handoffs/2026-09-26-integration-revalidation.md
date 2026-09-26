# 任务 2–5：开发依赖补齐与集成复验

- 日期：2026-09-26
- 作者 / Agent：Codex
- 分支：main；任务 3–5 保留原隔离 worktree
- 当前 HEAD：2ba826a
- 相关 commit：pending
- 相关 tag / release：N/A
- 状态：draft

## 工作摘要

用户安装 Debian GTK/WebKit 开发库、Java 21 与 Android command-line tools 后，继续安装与 CI 一致的 SDK/NDK，恢复之前因环境阻塞的检查。本记录为复验进度，不能替代各任务的最终验收。

## 影响范围

- 模块：任务 2 的返回高亮 CSS；本地工具链与验证记录。
- 平台：Linux 原生、Web、Android ARM64 编译。
- 额外影响：不修改全局 shell 配置、数据库 schema 或依赖版本；不触碰日常 RSS 数据。

## 关键变更

- 用户安装的 GTK 3.24.49、WebKitGTK 2.52.6、OpenJDK 21.0.12.1 已通过实际命令确认。
- 安装 Android platform-tools、platforms;android-34、build-tools;34.0.0、ndk;27.3.13750724；sdkmanager 安装退出 0。
- SDK 在 ~/Android/Sdk；NDK ARM64 API 34 Clang 可执行，版本 18.0.4。
- 进程环境脚本保留于忽略目录 target/integration-validation/android-env.sh；source 后可执行 Android 检查。未写入 ~/.bashrc。
- workspace 测试发现返回高亮硬编码颜色回退违反现有主题契约；entries.css 改用既有 --accent-soft。

## 验证与验收

日志位于 target/integration-validation/。

- pkg-config --modversion gtk+-3.0 webkit2gtk-4.1：0。
- java -version：0。
- sdkmanager --licenses：0。
- sdkmanager 安装上述四个 SDK 包：0；工具输出弃用提示，但安装成功。
- sdkmanager --version：0，22.0；NDK Clang --version：0。
- cargo fmt --all --check：0。
- cargo clippy --workspace --all-targets -- -D warnings：0。
- cargo test --workspace：初次 101，test_default_style_token_contract 拦截 entries.css 的硬编码颜色；修复后重跑 0，共 297 passed、2 ignored。
- git diff --check：0（高亮修复前检查，交付前需复查最终差异）。
- 桌面构建、Android/wasm 编译复验：进行中，尚无最终结果。
- 桌面实际交互验收：未运行；已准备独立测试数据库与基线源码归档。
- Android 运行验收：未运行，按本轮确认范围仅做编译检查。

## 结果

此前系统开发库和 Android NDK 缺失已解决；集成与提交尚未完成。commit: pending，未 push / tag / release。

## 风险与后续事项

- 必须取得剩余命令真实退出码，并完成桌面位置恢复对照；不能把构建结果替代界面验收。
- 任务 3–5 尚未集成，原工作树保留；本次 workspace 测试仅对应主工作树的任务 2。
- 临时工具、测试数据库、构建日志均位于 target，不纳入提交。

## 给下一位 Agent 的备注

先读取各任务 2026-09-25 handoff。最终选择为 4.3 B，其余 A；安装与相称验证后本地提交已授权，不重复询问。保留所有既有改动及 .handoff 排除规则，不 push。

## 任务 2 最终复验

- 原生构建、Android ARM64 check、wasm check / clippy、最终 fmt / workspace clippy / diff check 全部退出 0。
- 基线与实现的原生 1280×900 Inspector 专项均退出 0：基线正文重开 0，实现恢复 3200；实现列表返回仍为 2500。
- Web 重建、positions.cjs、delayed-image.cjs 全部退出 0，覆盖 360×800 / 1280×800。
- 任务 2 达到提交条件；首次提交本记录的 commit 包含任务 2 与本次颜色 token 修复。Android 运行及物理滚轮仍不在已验证结论中。

## 本地提交与任务 3 集成

- 任务 2 已本地提交：`4d0532e`，未 push。主工作树提交后干净。
- 任务 3 从原隔离 worktree 以独立临时 Git index 生成补丁，在主工作树三方应用；原 worktree 的 dirty/index 保留。
- 解决 entries.css、entries_page/facade.rs、entries_page/mod.rs 三处重叠：同时保留返回高亮、分页恢复和批量确认；搜索首次挂载不重置页码，筛选变化仍取消批量预览。
- 集成后的任务 3 正在复验，commit: pending。
- 任务 3 合并后所有通用检查退出 0；subscription harness 5 项通过，Web 两视口 bulk 验收和任务 2 位置交叉回归退出 0。5 万条本次预览 182ms / 应用 598ms。

## 任务 4 集成

- 任务 3 已本地提交 `a1aae97`，提交后主工作树干净，未 push。
- 任务 4 在该提交上三方合并；保留批量已读与阅读位置的文档及浏览器契约，同时补充 test_bulk_read 的 NewFeedSubscription.site_url 构造。
- 原任务 4 worktree / index 保留；本次主线集成 commit: pending。
- 任务 4 全部通用检查退出 0；三个 wasm harness 为 6 / 19 / 3 项通过，Web 两视口及真实 HTTP 直连通过，CLI 0 / 预期1 / 0 和独立数据库断言符合预期。

## 任务 5 集成

- 任务 4 已本地提交 `3930289`，提交后主工作树干净，未 push。
- 任务 5 已合入：保留订阅候选状态与刷新反馈 revision 清除逻辑；任务 4 的 apply_prepared_update 仍经统一 apply_source_output 取得真实 inserted_count。
- 为新增插入计数测试补齐 site_url=None；SQLite 批量已读事务方法与刷新插入计数事务方法均保留。任务 5 原隔离工作树 / index 保留。
- 任务 5 全部通用检查退出 0，最终 workspace 为 303 passed / 2 ignored；三个 wasm harness 为 20 / 6 / 3 项通过。
- 最终同一 Web bundle 的任务 2–5 全部交叉回归退出 0，CLI 真实 HTTP 与 SQLite 断言确认新增 1 / 0 / 2 / 0。
