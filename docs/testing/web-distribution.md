# Web 发布包与容器验收（C6）

构建流程见 [web-packages.yml](../../.github/workflows/web-packages.yml)、[docker.yml](../../.github/workflows/docker.yml) 与 [release.yml](../../.github/workflows/release.yml)。实际运行结果另记在 [C6 交接](../handoffs/archive/2026-10/2026-10-22-c6-web-distribution.md)，本页描述检查能力，不代表本次已通过。

## 包与构建标识

`package_web.py create` 只复制明确传入的服务程序与 `public/`，生成文件清单和 SHA-256。未知标识、页面不含预期标识、缺少 index、符号链接、已有输出均拒绝。`unpack` 拒绝目录穿越、链接、重复文件和校验不符，保留 Unix 可执行权限。

```bash
python scripts/test_package_web.py
python scripts/package_web.py create --binary target/release/rssr-web --public target/web-dist/public --build-id "$RSSR_BUILD_ID" --output target/rssr-web-linux-x86_64.tar.gz
python scripts/package_web.py unpack target/rssr-web-linux-x86_64.tar.gz target/web-verify
python scripts/smoke_web_package.py target/web-verify --logs target/web-package-check
```

这会在临时新库启动实际解压出的程序，经登录后获取页面、执行匹配版本的设置写入，并确认旧标识写入返回 `build_mismatch` 且没有修改资料库。`build.json` 自身不是可信签名；文件清单验证不能替代实际程序启动与浏览器检查。

`smoke_web_package.py` 可加 `--chrome <Chrome 可执行文件> --old-package <另一构建标识的解压包>`：真实页面从自己的 WASM 发出构建标识，匹配页面可以保存；旧包的真实页面被当前服务拒绝，显示可见的重新加载提示并保留服务器设置。这里不使用请求头篡改模拟旧页面。旧包必须经过清单校验且标识不同。

## 容器与卷

镜像的 `packaged` target 消费 Linux 包与同次构建的 `rssr-cli`；runtime 检查比较容器内服务二进制与包内 SHA-256。源码构建使用 Dockerfile 默认的 `source-built` target，并要求显式 `RSSR_BUILD_ID`。

```bash
python scripts/smoke_web_container.py --image rss-reader-smoke:<sha> --build-id git-<sha> --package target/c6-image/package --logs target/c6-container-check
```

检查用随机名称创建自己的容器和卷，不使用部署中的卷；结束后只清理本次创建并记录的名称。覆盖：

- 新卷的 `library.sqlite3`、`auth.sqlite3`、`outbound.json`、登录、匹配构建写入与旧构建拒绝。
- 默认 `rssr` 用户的 CLI 修改可由服务器读取；root 与无关 UID 的 CLI 被现有属主/策略检查拒绝。
- 服务器运行期间 CLI 备份，正常重启后会话与数据保留。
- 按 v0.1.22 的格式生成测试凭据和有效 HMAC 旧会话；旧认证卷以只读方式挂载，凭据迁入新数据卷，旧会话被拒绝，旧文件字节不变。
- 停止旧实例、继续挂载同一新数据卷但移除旧认证卷后，迁移凭据仍有效；不同初始密码不覆盖已有认证库。

GitHub 的 Docker 手动运行可勾选 `all_platforms`，同时构建并启动验证 Linux、Windows、macOS Intel / ARM 包；该入口不发布 GitHub Release 或 GHCR 镜像。正式 Release 工作流把同一 Linux artifact 交给 Docker 验收，镜像不会另行构建服务或页面。

## 边界

自动化不代替人工视觉、真实 HTTPS/公网出口、跨机网络卷、Android/macOS GUI 或独立评审。用户部署卷的迁移应先备份；本脚本只验收合成旧卷。Windows 没有 Unix 属主/ACL 检查，容器属主证据来自 Linux。
