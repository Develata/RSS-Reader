# 全局风险审查：Linux 包、CLI 刷新与 Web 代理

- 日期：2026-09-23
- 作者 / Agent：Codex
- 分支：main
- 当前 HEAD（工作开始时）：d069e2f
- 相关 commit：pending（本记录与实现一同做本地提交）
- 相关 tag / release：已发布 v0.1.16 不包含本次改动
- 状态：validated（本机可执行的模块检查完成；新 `.deb` 安装与 GUI 验收待 release runner）

## 工作摘要

在干净工作树上审查当前发布链、CI 矩阵、原生路径、CLI 命令和部署态 Web 代理。确认并修复三个实际风险：Linux `.deb` 缺运行时依赖元数据；CLI 无目标刷新被误判为刷新全部；代理对 IPv6 字面量和 IPv4 映射 IPv6 的地址分类不完整，DNS 解析没有上限。

## 影响范围

- 模块：Linux `.deb` 打包脚本 / release workflow、`rssr-cli`、`rssr-web::proxy`。
- 平台：Linux 安装包、CLI、Web/Docker 部署态；没有修改 application/domain、桌面 UI 或 Android 业务语义。
- 文档：中英文 README、使用指南、Web 部署、CLI 命令参考和主线验收矩阵。

## 关键变更

### Linux `.deb` 依赖

- 已发布 v0.1.16 包的 `DEBIAN/control` 没有 `Depends`；同一包的 `usr/bin/rssr-app` ELF 直接链接 WebKitGTK、GTK、libsoup、libxdo 等共享库。原 release job 预装开发库后再安装包，启动 smoke 不能发现用户机器缺依赖的情况。
- `prepare_linux_deb.sh` 现在调用 Debian 官方 `dpkg-shlibdeps`，从实际 ELF 和构建 runner 的库元数据生成 `Depends`。缺库、无依赖结果、上游 bundler 新增 `Depends` 时显式失败，避免静默覆盖。合成 ELF 包测试核对字段、版本、内容和拒绝路径；release job 断言 WebKitGTK 依赖存在，再安装启动。
- 此适配仅处理 Linux 平台打包事实，不让 Debian 包名进入 Rust application/domain。生成字段的包级兼容性仍需下一次 release runner 实测。

### CLI 参数

- `refresh` 现在必须且只能传 `--all` 或 `--feed-id`。原条件 `args.all || args.feed_id.is_none()` 位于 `feed_id == None` 分支，恒为真。
- 使用 Clap 在参数解析阶段拒绝无目标和双目标，实际 CLI 进程测试核对退出码 2、空 stdout、数据库未创建，并确认合法 `--all` 仍可建库执行。

### Web 代理

- 改用 `url::Host` 的类型化 IPv4/IPv6/域名分流。URL 的 IPv6 `host_str()` 带方括号，旧 `parse::<IpAddr>()` 路径无法识别；字面量现在先分类，公开字面量不做 DNS，域名仍解析后钉住校验地址。
- IPv4 映射/兼容 IPv6 地址按内嵌 IPv4 复查；补拒绝 0/8 和 240/4。DNS 解析加入 10 秒上限，重定向逐跳复用同一校验。测试覆盖本地、映射、内网和公开字面量。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`、`git diff --check`：通过。
- `cargo test --locked --workspace --exclude rssr-app`：通过。
- `cargo clippy --locked --workspace --exclude rssr-app --all-targets -- -D warnings`：通过。
- `bash scripts/test_prepare_linux_deb.sh`、`shellcheck`、`actionlint`：通过。
- `cargo test --locked -p rssr-cli --test test_stdout_contract` 与 `cargo test --locked -p rssr-web proxy::tests`：通过，已包含新增实际失效路径。
- `dpkg-deb -I` / `readelf -d` 对已发布 v0.1.16 包：确认无 `Depends`，但 ELF 有 WebKitGTK、GTK、libsoup、libxdo 等 NEEDED。该包不是新代码的安装验收材料。

### 未执行或环境受限

- 本机缺 GTK/WebKit 开发库和 Xvfb，`rssr-app` 原生构建与新 `.deb` 普通用户 GUI 启动未执行；本机对旧包运行 `dpkg-shlibdeps` 因缺 libxdo/WebKitGTK/libsoup/JSC 失败，符合新打包适配的失败关闭语义。
- 没有运行依赖外部真实 feed 的代理 smoke，也没有触发远端 CI、tag 或 release。浏览器 UI、Windows / Android / macOS 实机状态沿用此前独立记录，本次不冒充新验收。

## 结果

- 本轮修复限于已确认的问题，不宣称整个仓库无其它缺陷，也没有未经测量的性能收益。
- 新 `.deb` 仍须观察首次 release runner 的 `dpkg-shlibdeps` 字段和普通用户双启动结果；v0.1.16 附件未改变。

## 风险与后续事项

- `dpkg-shlibdeps` 生成的最低库版本来自构建用 Ubuntu runner；其它发行版兼容性不能据此推定。
- Web 代理仍需在真实远端 feed 与重定向链上做部署态 smoke；单元测试只证明地址分类和无 DNS 的字面量路径。
- Linux 单独解压的 CLI 仍按便携规则寻找自身数据；操作 `.deb` 桌面库须显式传 `--database-url`。

## 给下一位 Agent 的备注

- 先检查 `.github/workflows/release.yml` 的 Linux job 和 `scripts/prepare_linux_deb.sh` 的真实输出，再考虑发布；不要复用旧 v0.1.16 包证明修复生效。
- 代理入口在 `crates/rssr-web/src/proxy.rs`；若要扩展保留地址覆盖，优先补可复现输入与 IANA 地址依据，避免维护任意的私有地址清单。
