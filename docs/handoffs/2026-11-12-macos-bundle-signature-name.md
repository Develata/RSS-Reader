# macOS 包签名不完整与菜单栏名称

- 日期：2026-11-12
- 作者 / Agent：Claude Code（Opus）
- 分支：claude/macos-adhoc-sign
- 当前 HEAD：4286a5db（分支基点）
- 相关 commit：见本分支提交
- 相关 tag / release：v0.2.2（问题所在版本）；修正从下一次发布生效
- 状态：`validated`（本机模拟；Release 工作流未实际运行）

## 工作摘要

Develata 报告无法安装 v0.2.2 的 macOS 版本。原因是应用包签名不完整：浏览器下载后带有隔离标记，Gatekeeper 因此拒绝打开。发布流水线现在会对整个应用包做 ad-hoc 签名，并在打包后校验签名；同时把 `CFBundleName` / `CFBundleDisplayName` 从 `RssrApp` 改为 `RSS-Reader`。

## 影响范围

- 模块：
  - `.github/workflows/release.yml`（build-macos）
  - `scripts/prepare_macos_bundle.py`、`scripts/test_prepare_macos_bundle.py`
- 平台：
  - macOS（Intel / Apple Silicon）
- 额外影响：
  - README 安装说明、`docs/releases/v0.2.2.md` 已知问题

## 关键变更

### 根因（已核实）

- v0.2.2 的 `RSS-Reader-macos-aarch64.tar.gz` 中，`Contents/MacOS/rssr-app` 只带链接器自动加的 ad-hoc 签名（`flags=0x20002(adhoc,linker-signed)`）。Bundle 没有 `_CodeSignature/`，`Sealed Resources=none`，`Info.plist=not bound`。
- `codesign --verify --deep --strict` 报错：`code has no resources but signature indicates they must be present`。`spctl -a -t exec` 判定 rejected，`syspolicy_check distribution` 把这一项标为 Fatal。
- Develata 本机的 `/Applications/RSS-Reader.app` 带 `com.apple.quarantine`，校验报同一个错误。
- x86_64 包的可执行文件完全没有签名，成因相同（dx 0.7.9 不签 bundle），只是 Gatekeeper 给出的提示可能不同（未实测）。
- 菜单栏名称：dx 按二进制名生成 `CFBundleName = CFBundleDisplayName = RssrApp`，`Dioxus.toml` 的 `application.name` 不会写进去（与 Linux desktop entry 是同一个问题）。

### 发布流水线

- 先跑 `prepare_macos_bundle.py`：写版本号，并写入名称 `RSS-Reader`。脚本遇到已签名的 bundle 会拒绝处理，所以必须排在签名之前。然后执行 `codesign --force --sign - RSS-Reader.app`。这一步只是 ad-hoc 签名，不是 Developer ID 签名或公证。
- 冒烟检查对解压后的成品运行 `codesign --verify --deep --strict`。放在 tar 打包、解包之后检查，是为了确认签名覆盖的就是实际发出去的文件。冒烟检查还断言两个名称键都是 `RSS-Reader`。

## 验证与验收

### 自动化验证

- `python3 scripts/test_prepare_macos_bundle.py`：通过（8 项）。变异检查：删掉写名称的那一行后 4 项失败，恢复后全部通过。
- `ruby -ryaml -e 'YAML.load_file(".github/workflows/release.yml")'`：通过。
- 用 v0.2.2 的 aarch64 / x86_64 正式附件模拟新流程：prepare（`--release-tag v0.2.3`）→ `codesign --force --sign -` → tar 打包再解包 → `codesign --verify --deep --strict`。两种架构都通过（`valid on disk`、`satisfies its Designated Requirement`，`Sealed Resources version=2`）。名称与版本字段符合预期。
- 原包未经新流程时，同一条校验命令失败（aarch64 报上面的 resources 错误；x86_64 未签名）。所以新加的门禁能拦住这次的问题。

### 手工验收

- 隔离 `HOME` 后直接启动模拟出的 aarch64 新包：程序正常打开资料库，数据只写进临时 HOME。LaunchServices 登记的应用名是 `RSS-Reader`。用同样方法启动 v0.2.2 原包，登记名是 `RssrApp`。
- 带隔离标记的新包仍会被 `spctl` 拒绝，这是预期的：没有 Developer ID。“系统设置 → 隐私与安全性 → 仍要打开”这条用户路径没有在界面上实测。
- 菜单栏文字没有截图核对。“菜单栏使用 LaunchServices 登记名”是推断。
- Release 工作流没有运行：GitHub 上的 `macos-latest` runner 尚未验证。

## 结果

- 合并后，下一次发布的 macOS 包会是完整的 ad-hoc 签名包，名称为 `RSS-Reader`。
- v0.2.2 用户的绕过方法是 `xattr -dr com.apple.quarantine <RSS-Reader.app 路径>`：未带隔离标记的原包已实测可以启动。这个方法已写进 README 和 v0.2.2 发布说明。

## 风险与后续事项

- 要完全去掉 Gatekeeper 弹窗，需要 Apple Developer ID 证书和公证（年费），是否采用由 Develata 决定。
- 若以后加入 Developer ID 签名，把 `codesign --sign -` 换成证书身份并加上 hardened runtime 与公证步骤。`prepare_macos_bundle.py` 仍须排在签名之前。

## 给下一位 Agent 的备注

- 入口：`.github/workflows/release.yml` 的 build-macos job 中 “Archive macOS artifacts” 与 “Smoke-check macOS artifacts” 两步。
- Linux 的同类名称问题见 [Linux 桌面入口名称](./2026-11-02-linux-desktop-entry-name.md)。
