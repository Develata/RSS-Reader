# macOS 发布包版本元数据同步

- 日期：2026-10-05
- 作者 / Agent：dot
- 分支：`fix/macos-bundle-release-version`
- 当前 HEAD：以本记录所在提交的 Git 历史为准；基线 `b745a846346bfdaea54728820dbb339725c2a864`
- 相关 commit：本记录随修复提交，确切 SHA 见 PR 提交列表
- 相关 tag / release：既有 `v0.1.22` 保持不变；修复需后续包含本提交的新发布才会生效
- 状态：`draft`

## 工作摘要

修复 macOS App 的 `CFBundleShortVersionString` / `CFBundleVersion` 沿用 workspace `0.1.0` 的问题。沿用 Release workflow 的 `RELEASE_TAG` 作为版本来源，不修改 Cargo workspace 版本、CLI 或 Windows 实现。

## 影响范围

- 模块：`scripts/prepare_macos_bundle.py`、`scripts/test_prepare_macos_bundle.py`
- 平台：macOS 的 x86_64 / aarch64 App 归档；其他平台程序行为不变
- workflow / 文档：`.github/workflows/release.yml`、`.github/workflows/ci.yml`、本交接记录
- 无数据库、迁移、UI 或业务层改动

## 关键变更

### 归档前同步元数据

- 接受规范稳定 tag `vX.Y.Z`，拒绝缺少前缀、位数错误、前导零、预发布 / build 后缀、空白和非 ASCII 数字。
- 在归档 staging App 中将两个 plist 字符串字段同时设置为去掉 `v` 的版本，保留其余元数据与 XML / binary 格式，不改可执行文件。
- 拒绝缺少版本字段、非字符串字段、缺少 plist、symlink plist，以及带 `Contents/_CodeSignature` 或 `Contents/CodeResources` 的已签名 bundle。不会删除签名或发布因修改 plist 而失效的已签名包。
- 当前 Dioxus.toml 和 Release macOS job 均未配置 App 签名。未来增加签名 / 公证时必须让本适配器运行在其前面；若直接让 dx 签名，守卫会中止归档，须先调整流程。
- App 查找使用 `-prune`，并要求结果为一个有效目录，避免从多个候选或嵌套 App 中静默取第一个。

### 最终产物与回归检查

- 两个 macOS matrix job 都解压最终 `RSS-Reader-<suffix>.tar.gz`，用原生 `plutil` 校验 plist，并独立用 `PlistBuddy` 逐项精确断言两个版本等于 tag 版本。失败即停止上传 / 发布。
- 在既有日常 CI 的 `test-tools` job 加入 7 项 Python fixture 测试，覆盖两种 plist 格式、其他元数据与可执行文件保留、无效 tag、版本字段异常、已有签名、symlink、缺失 / 损坏 plist、CLI 路径含空格以及 tar.gz round trip。
- fixture 测试不声称实际 macOS App 编译、启动、签名或公证已通过。

## 验证与验收

### 自动化验证

- 本次通过 GitHub connector 读取源码、审查并提交变更；未在本地执行任何构建或测试命令。
- `python3 scripts/test_prepare_macos_bundle.py`：已接入 PR / main CI，初始提交时尚未执行，结果以对应提交的 `CI / test-tools` 为准。
- `actionlint`：沿用 `CI / test-tools` 的既有全 workflow lint，初始提交时尚未执行。
- `cargo fmt`、Clippy、workspace tests 与其他模块：未在本地执行，沿用现有 PR CI；本修复未修改 Rust 源码。
- 双架构实际 `dx bundle` 与最终归档的 `plutil` / `PlistBuddy` 检查：尚未执行；Release 仅由 tag push 或带 release tag 的 workflow_dispatch 触发，普通 PR CI 不执行该发布路径。
- 没有手动触发 Release、打 tag、修改 main 或替换现有 Release 附件。

### 手工验收

- macOS 实机启动 / Finder 版本显示：未执行。
- macOS 签名 / 公证：未执行；签名守卫测试只使用 fixture seal，不等同于实际 codesign 验证。

## 结果

代码与回归门禁已准备为独立 Draft PR。是否通过日常 CI 以 PR 对应提交的检查为准；尚未宣称可以发布。既有 v0.1.22 下载包和历史发布说明中的 macOS 元数据限制继续有效。

## 风险与后续事项

- 合并本修复并在获授权的新发布中运行两个 macOS Release job 后，才有真实产物的版本核验证据。
- 若 bundler 将来改变版本字段、签名流程或输出结构，适配器会失败而非猜测继续发布。
- 目前仅支持稳定数字 tag；预发布 tag 需要另行明确 macOS 版本映射，不静默截断。

## 给下一位 Agent 的备注

- Dioxus 0.7.9 的 [version_string](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/cli/src/bundler/mod.rs) 直接取 Cargo package version；[macOS bundler](https://github.com/DioxusLabs/dioxus/blob/v0.7.9/packages/cli/src/bundler/macos.rs) 用它生成短版本，并默认同时生成 bundle version。不能仅设置 `bundle.version` 后假定 macOS 两项都改变。
- 既有 v0.1.22 产物与验收历史保留在 [2026-10-04 发布记录](./2026-10-04-v0.1.22-release.md)，本次不改写历史验证结论。
