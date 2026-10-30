# 原生 CSS 开头 BOM 的独立呈现修复

- 日期：2026-10-30
- 作者 / Agent：Codex
- 分支：`codex/android-theme-import`
- 基线：main `ef04ded9955699e1175b3904d470c2e9046edf9f`
- 相关 commit：pending；与 Android 导入适配单独提交
- 相关 tag / release：无
- 状态：`validated`

## 工作摘要与影响范围

Android 导入真机验收发现带 BOM 的 CSS 虽然被原样交给 `<style>`，第一条规则却不生效。按用户要求先复现、再单列修复，没有通过丢掉文件原文来绕过断言。影响仅在 `rssr-app/src/app.rs` 自定义样式呈现入口，适用于 WebView/Web；不改变 Settings 草稿、校验、导出、Backend 或数据库契约。

## 复现与根因

- 最初在新导入包观察到：textarea 内容正确，`#user-custom-css.textContent` 保留 BOM / CRLF，等待 20 秒后 `--rssr-import-test` 仍为空。
- 旧密度退役测试 APK SHA256 `0044b89d149ba9a4e103bd2c96d85cb7b07eb6a698b9a5d85351f8dae55c7db5` 复现相同结果。该 APK 的产品已由 PR #48 合入 main，身份见[密度回执](./2026-10-30-density-retirement.md)。未使用新导入路径，通过原有 CSS 编辑和预览操作复现。
- 实际 Android 15 / WebView CSSOM：普通 `:root { --rssr-import-test: 37; }` 生效；在注释前加 U+FEFF 后，选择器成为 `U+FEFF :root`，值为空。不是读取失败，也不是 Dioxus 字符串表错位。原 SQLite 设置始终不变。
- 证据：`target/android-theme-import-20261030/android-bom-baseline/acceptance/acceptance.json`；初次失败保留于 `android-03` / `android-04`。旧包源码快照与 patch 身份沿用其原回执，不将旧包标为本批构建。

## 关键变更

渲染 `<style>` 时只借用去掉一个开头 U+FEFF 后的切片，无额外分配；草稿、存储和导出继续保留原文。内部 U+FEFF、第二个连续 U+FEFF 与换行均不被这一步归一化。原生传输回归仍验证字段和原文一致，样式节点断言更新为仅略去一个文件签名。

## 验证与验收

- 旧 APK 原生复现：普通 CSS 与 BOM CSS 对照通过，确认缺陷存在于合并基线。
- 修正后 `cargo test -p rssr-app --locked`：391 通过、1 忽略。
- 新 Android 实际系统导入：10/10 中明确验证首规则生效，BOM / CRLF 原文明确保存后仍原样入库；Windows 实际 WebView2：13/13，覆盖字段、CSS 规则生效、连续/内部 BOM 传输和保存重开。验证基于本工作树最终产品快照，含另行提交的 Android 适配，产物身份见[接续交接](./2026-10-30-android-theme-import.md)。
- `cargo fmt --all --check`、Clippy、Wasm check 均通过。没有新增仅镜像两行切片逻辑的单元测试，以真实 CSSOM / computed style、草稿与数据库对照回归。

## 当前状态、风险与交接

commit: pending。这是有意改变样式呈现的缺陷修复，与文件导入功能分开记录和提交。已完成所列真实设备和桌面回归；系统 IME、辅助技术、更广设备和 Web GUI 不在本地验收覆盖内。完整接续见[Android 导入交接](./2026-10-30-android-theme-import.md)。
