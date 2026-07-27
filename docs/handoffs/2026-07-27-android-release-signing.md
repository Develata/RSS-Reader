# Android 正式签名发布链打通与发布断言修正

- 日期：2026-07-27
- 作者 / Agent：Claude Code
- 分支：main
- 当前 HEAD：36626b0
- 相关 commit：36626b0
- 相关 tag / release：暂无（下一个 tag 生效）
- 状态：`draft`

## 工作摘要

Android 发布包此前一直由 CI 每次现生成的 debug keystore 签名，每个版本证书都不同，导致用户无法覆盖升级（设备侧报「校验失败」）。本次配置签名密钥后暴露出发布流水线里三处从未执行过的缺陷，一并修掉，并把 versionCode / versionName 接到发布 tag 上。

## 影响范围

- 模块：
  - `.github/workflows/release.yml`
  - `.github/workflows/ci.yml`
  - `scripts/prepare_android_bundle.py`
- 平台：
  - Android
- 额外影响：
  - workflow / docs / release

## 关键变更

### 事故定位（已用实物证据确认）

- 下载 v0.1.10 / v0.1.11 / v0.1.12 的 APK，`apksigner verify --print-certs` 得到三个**互不相同**的证书，subject 均为 `CN=Android Debug`：
  - v0.1.10 `b037d707f73e18c2…`、v0.1.11 `f99d5dba35537fcc…`、v0.1.12 `2c84213ed90b6d98…`
- 根因：四个签名 secret 从未配置，`ANDROID_RELEASE_READY` 恒为 false，只跑 `assembleDebug`；AGP 在每台全新 runner 上自动生成一次性 debug keystore。
- 三个版本的 `versionCode` 恒为 1、`versionName` 恒为 0.1.0。

### release.yml：发布断言

- **图标断言改为端到端**。原写法 `grep 'application-icon-.*rssr_launcher'` 匹配的是**文件路径**；release 会跑 `optimizeReleaseResources`，aapt2 把 `res/mipmap-*/rssr_launcher.png` 压成 `res/xY.png`，debug 不跑这一步——所以该断言只在 debug 上成立，一开签名必然失败。现改为从 badging 取解析后的图标路径，再回 `aapt2 dump resources` 查它属于哪个资源名，要求是 `mipmap/rssr_launcher`。
  - 中途曾退化成「有任意图标 + 资源表里存在 rssr_launcher」两个弱断言，其合取**不蕴含**「应用图标就是 rssr_launcher」（图标属性丢失时脚手架默认的 `ic_launcher` 会顶上，而拷进去的 png 仍在资源表里）。评审指出后改回端到端。
- **新增 debug 密钥守卫**，并修正其失败方向。初版 `if apksigner … | grep -q 'CN=Android Debug'` 在 apksigner 缺失或 APK 未签名时会**静默放行**（管道退化成 grep 读空输入，返 1，`if` 为假）。实测四种情形，现在均正确：

  | 情形 | 结果 |
  |---|---|
  | apksigner 路径不存在 | exit 127，步骤失败 |
  | APK 由 debug 密钥签名 | exit 1，步骤失败 |
  | APK 未签名 | exit 1，步骤失败 |
  | 正常签名 | exit 0，通过 |

- **新增 tag 发布硬门禁**。secret 缺失时整段签名逻辑连同上面的守卫都会被 `if: env.ANDROID_RELEASE_READY == 'true'` 跳过，然后安静地发一个 debug APK——用户侧症状与本次事故完全一致且 CI 全绿。现在 tag push 缺 secret 直接失败；`workflow_dispatch` 仍允许降级。
- 工具输出统一先落盘再断言。`工具 | grep` 在工具失败时会退化成「grep 读空输入」：正向断言响亮失败没问题，**反向**断言会静默通过。
- 字面量断言一律 `grep -F`（`versionName='0.1.13'` 里的 `.` 在 BRE 下是通配符）。

### release.yml / ci.yml：版本号

- `RELEASE_TAG` 走 workflow 级 env，tag push 取 `github.ref_name`、dispatch 取 `inputs.release_tag`。
- `prepare_android_bundle.py` 新增可选 `--release-tag`，把 tag 解析成 `versionCode = major*10000 + minor*100 + patch`、`versionName = 去掉前导 v`。不传时行为完全不变（`ci.yml` 原调用与 README 的本地流程都不受影响）。
- **拒绝预发布 / 构建后缀**，两个独立理由：`v0.1.13-rc1` 与 `v0.1.13` 会算出同一个 versionCode；且 versionName 原样插进 Gradle Kotlin DSL 字符串字面量，后缀不限制就能用引号逃逸出来在配置阶段执行任意代码（实测 `v1.2.3-"; println("pwned"); //` 可注入，现已拒绝）。
- 替换后**验证替换确实发生**，写不进去就抛错——正则静默落空会让产物照旧带 versionCode=1 发出去而 CI 全绿。
- workflow 里的 versionCode 复用脚本同一份实现（`python3 -c … parse_release_tag`），不在 shell 里重写公式。
- `ci.yml` 的 Android bundle smoke 现在带假 tag `v9.87.65` 跑，并断言 APK 里 `versionCode='98765'` / `versionName='9.87.65'`——否则这条链只在真正打 tag 时才第一次执行，正是本次事故的形态。

### release.yml：产物

- 配置签名后只发布 release APK + AAB，不再同时发 debug APK（两者签名不同，并排挂着会让用户装错并再次撞上「装不上」）；未配置签名时才退回发布 debug APK。

## 验证与验收

### 自动化验证

- `python -c` 解析用例（正常 tag / 单调性 / 非法输入 / 注入串）：通过
- `patch_gradle_version` 对 Kotlin DSL 与 Groovy 两种写法：通过；缺 versionCode / 改名两种模板变体均按预期抛错
- 合成 bundle 端到端跑 `prepare_android_bundle.py`（带 tag、不带 tag 两条路径）：通过，图标 / app_name / SDK 等既有行为不变
- 两个 workflow 共 25 个 `run` 块 `bash -n`：0 语法错误
- 签名守卫四情形脚本化实测：通过（见上表）
- 图标路径 → 资源名映射在真实 APK 上实测：`res/mipmap-mdpi-v4/rssr_launcher.png` → `mipmap/rssr_launcher`
- `cargo` 全家桶：未执行（本次不涉及 Rust 改动）

### 手工验收

- 真机安装签名包并确认可用：**未执行**，见下方风险
- 用 `workflow_dispatch` 做一次干跑：**未执行**

## 结果

- 可合并。但**发布链未经一次真实成功运行**，合入后应先 `workflow_dispatch` 干跑再打正式 tag。
- 用户影响：换用正式签名包需要先卸载（证书与 debug 包不同），卸载会清空 Android 本地订阅与已下载正文（`$HOME/RSS-Reader/rss-reader.db`），**务必先导出 OPML**。此后所有升级都能直接覆盖。

## 风险与后续事项

- **release 包是 R8 minify 过的**（构建日志可见 `minifyReleaseWithR8`），debug 包不是。Dioxus / wry 经 JNI 反射用到的类有被 R8 剥掉的风险，可能构建成功但启动即崩。**真机验收前不要认为它等价于「签名版 debug 包」**；若崩溃，先回退安装 debug APK 并补 ProGuard keep 规则。
- keystore 一旦丢失，后续所有升级都会再次断掉，且只能靠再一次卸载恢复。
- 本次新增的 release 侧断言（`aapt2 dump resources` 输出格式、AAB `base/resources.pb` 明文命中、图标路径回查）**都从未在真 runner 上跑过**，只在本地用已发布的 APK 验证过等价逻辑。
- `patch_gradle` 若同时遇到 `build.gradle` 与 `build.gradle.kts`，两个都会被要求含 versionCode，缺的那个会硬失败。dx 目前只生成 `.kts`，暂不构成问题；属于响亮失败，不是静默。
- build-tools 版本 `34.0.0` 在 release.yml 里硬编码多处，升级时需一并改。

## 给下一位 Agent 的备注

- 入口：`.github/workflows/release.yml` 的 `build-android` job，与 `scripts/prepare_android_bundle.py`。
- 本次事故的核心教训是「断言只在发布路径上跑 = 等于没跑」，以及「反向断言必须先让工具自身退出码炸出来，否则失败方向是敞开的」。改这两个文件时优先检查这两点。
- 密钥配置步骤（keytool / gh secret set，PowerShell 与 Git Bash 两套）见 README「发布与交付」与本文件上游会话记录。
