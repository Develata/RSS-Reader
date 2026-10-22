# 发布前 UI 回归清单

这份清单服务于发布前最后一轮 UI 验收。

它和 [手工回归测试清单](./manual-regression.md) 的区别是：

- 手工回归清单偏通用
- 这份清单偏发布门禁
- 它明确要求同时覆盖：
  - 路由与基本交互
  - 主题与 CSS 契约
  - 正式 Web 与 Pages 演示两种构建
  - 关键持久化与配置交换

如果想先看“哪些已经被固定覆盖、哪些仍要手工补”，先读：

- [发布前 UI 覆盖矩阵](./release-ui-coverage-matrix.md)

如果这轮要直接用 Chrome MCP 做真实浏览器态补查，先起：

```bash
bash scripts/run_chrome_mcp_target.sh --restart
```

## 使用时机

在以下场景执行：

- 准备打版本 tag 前
- UI shell / bus / facade 有结构变更后
- 主题系统或语义接口有收口后
- Web 入口、服务器登录与浏览器回归路径有变更后

## 发布前最小自动化门禁

建议优先走统一脚本：

该入口现在只构建并启动 `scripts/release-ui/` 的独立 Rust CLI；参数、阶段状态、
build-once、summary 和退出码由 Rust 持有。执行 `--plan` 可只查看 JSON 计划。
Windows 仍依赖 Git Bash 和既有阶段工具，不代表所有 smoke 已原生移植。
行为基线、直接调用方式、信号边界与实测限制见 [release UI runner](release-ui-runner.md)。

```bash
bash scripts/run_release_ui_regression.sh --debug --port 8091
```

默认仅运行自动化门禁并退出；不再常驻静态服务：

```bash
bash scripts/run_release_ui_regression.sh
```

如果还想把 `rssr-web` 部署壳的最小 smoke 一起串进来：

```bash
bash scripts/run_release_ui_regression.sh --debug --port 8091 --with-rssr-web
```

如果要一次性把 browser / wasm contract harness 和 3 条固定 smoke 也串进来：

```bash
bash scripts/run_release_ui_regression.sh --debug --port 8091 --with-rssr-web --with-browser-contracts --with-fixed-smokes
```

等价快捷入口：

```bash
bash scripts/run_release_ui_regression.sh --debug --port 8091 --full
```

脚本内部会串行执行下面这组自动化检查。`--full` 包含五主题 Reader 截图和默认主题小视口断言，不等于 CI 的五主题全页面矩阵。与 CI 对齐时还需分别对四套内置主题运行 small viewport smoke（`--preset <key>`），以及本次变更涉及的手工/实机项。

至少先通过：

- `cargo check --locked -p rssr-app --target wasm32-unknown-unknown`
- `cargo test --locked -p rssr-app`（包含 builtin theme 契约，不重复单跑）
- `cargo test --locked -p rssr-infra --test test_library_refresh_contract_harness --test test_library_subscription_contract_harness --test test_library_config_exchange_contract_harness`
- `cargo test --locked -p rssr-web`

如果启用 `--with-browser-contracts`，还会补：

- `bash scripts/run_wasm_contract_harness.sh test_library_refresh_contract_harness test_library_subscription_contract_harness test_library_config_exchange_contract_harness`

三个原有单模块脚本入口继续保留。统一入口通过 std-only Rust runner 与 Cargo 的 target runner 机制执行精确产物，每个浏览器运行使用独立临时配置和 profile；不再按 mtime 寻找 wasm 或覆盖 crate 中的 `webdriver.json`。本地一个 Cargo 构建多个 harness 后串行运行浏览器；CI 先通过 `--prepare` 构建一次，再在独立 runner 通过 `--prebuilt` 按模块并发，准备产物成功不等于契约通过。详见 [主线矩阵](mainline-validation-matrix.md)。

同一次发布预检最多构建一次相同 profile 的 Web 包，后续固定 smoke 复用它；该标记不跨进程持久化。`--skip-build` 仍由调用者明确选择。部署壳 smoke 拒绝已占用端口，并确认本次启动的进程存活；HTTP 请求有超时，探测或断言失败也会清理本次服务。不再启动独立静态 SPA 服务。

主题矩阵、小视口与 feed smoke 共用真实服务器和 SQLite 夹具工具；驱动经 CDP 等页面就绪，保存同次加载的 DOM 与截图。Node.js 22+ 与 Chrome 的路径可用 `NODE_BIN`、`CHROME_BIN` 指定；隔离、构建标识与控制台规则见 [统一入口](./server-web-regression.md)。

如果启用 `--with-fixed-smokes`，还会补：

- `bash scripts/run_server_web_regression.sh --suite theme-matrix --skip-build`
- `bash scripts/run_server_web_regression.sh --suite regression --skip-build`
- `bash scripts/run_server_web_regression.sh --suite feed-smoke --skip-build`

如果这组自动化没过，不进入后续手工回归。

## 发布前 Web 路径

### 1. 正式 Web（服务器资料库）

```bash
bash scripts/run_server_web_regression.sh --suite regression
bash scripts/run_server_web_regression.sh --suite acceptance --long
```

页面、服务器与夹具工具必须来自同一版本。后者覆盖认证/CSRF、只读刷新、结果未知、任务、草稿与启动错误；聚合入口 `--full` 不自动包含长验收，需要单独运行。固定套件与 CI 五主题矩阵也不替代人工视觉检查。

认证层至少核对：未登录页面跳转 `/login`，未登录 API 返回 JSON 401；真实登录后 `/feeds`、`/settings`、`/api/v1/session` 可读；跨来源提交拒绝；`/logout` 先确认，退出后重放 cookie 拒绝。正式 Web 没有本地浏览器口令或旧会话探测接口。

手工检查用 `bash scripts/run_rssr_web_browser_smoke.sh`。在 `/feeds` 输入 `data-field="feed-url-input"`，点击 `add-feed`、`refresh-feed`，再经 `data-nav="feed-entries"` 打开文章。真实远端 feed 单独记录服务器网络与出站规则；本地 feed smoke 不能证明它可达。

### 2. Pages 静态演示

按 [Pages 构建与验收](../deployment/github-pages.md) 使用 `pages-demo` 与 `scripts/test_pages.sh`。该构建使用内存 SQLite，资料库操作重载即重置，禁用在线订阅、WebDAV 和自动刷新；不能复用正式 Web 的产物或把演示测试当作服务器 API 验收。

## 页面与主题矩阵

发布前至少检查以下页面：

- `/entries`
- `/feeds`
- `/settings`
- `/entries/:entry_id`（使用真实存在的文章 ID）

主题至少覆盖：

- 默认主题
- `Atlas Sidebar`
- `Newsprint`
- `Amethyst Glass`
- `Midnight Ledger`

如果要把多主题阅读页 回归固定化，直接用：

```bash
bash scripts/run_server_web_regression.sh --suite theme-matrix
```

## 核心检查项

### 1. 启动与路由

- 打开 `/`
- 确认默认能进入预期首页
- `订阅 / 文章 / 设置` 切换正常
- 浏览器刷新后仍能回到当前页
- 无白屏、无死循环、无不可恢复卡死

### 2. 服务器登录与恢复

- 未登录页面进入 `/login`；错误凭据有提示，正确凭据进入应用。
- 退出后会话失效；已有页面操作需重新登录时保留可恢复的设置草稿。
- 版本不匹配提示重新加载；CSS 与设置草稿恢复后仍需手动保存。
- 写请求结果未知时看得见，不显示成功、不自动重发；重新读取只更新显示。
- 浏览器不能保留草稿时停留并提示；强制离开的按钮明确说明草稿会丢失。
- 服务器不可用时显示读取失败，不把旧浏览器文章库作为离线后备。
- Pages 演示直接进入样例，不显示本地口令设置流程。

### 3. 订阅页

- 添加一个有效 feed
- `刷新全部`
- `刷新此订阅`
- 两步确认删除
- `导出配置`
- `导出 OPML`
- 如环境允许，再做一次 `导入配置` 或 `导入 OPML`

### 4. 文章页

- 搜索标题
- `按时间 / 按来源` 切换
- `仅未读 / 仅已读 / 仅收藏 / 仅未收藏`
- 来源多选筛选
- 进入阅读页

- 来源未读数（含 0）与订阅页一致，不随当前搜索范围变化
- 收起筛选后可见生效条件；无匹配结果时可清除条件，分组与归档偏好保留
- 首次加载中或读取失败时不误报没有订阅；恢复读取后显示原有文章
- 批量已读的预览、跨分页范围、集合变化后的重新确认和失败恢复

### 5. 阅读页

- 正文正常显示
- `标已读 / 标未读`
- `收藏 / 取消收藏`
- `上一篇未读 / 下一篇未读`
- 无对应方向未读时按钮禁用，方向快捷键不回退到已读文章
- `上一篇同订阅文章 / 下一篇同订阅文章`
- 返回上一页后列表仍稳定

- 元信息、设备本地时间和独立原文入口
- 返回列表与再次开文的位置恢复；重载清除运行内记忆
- Tab 焦点、阅读区内快捷键和搜索输入不误触
- 收藏 / 已读成功反馈不移动当前正文，失败仍显示可见错误
- 窄屏底栏四个按钮的点击区域不重叠，主题元信息不逐行重复增加块级留白

### 6. 设置页

- 切换主题模式
- 修改刷新间隔或字号
- `保存设置`
- 应用当前 CSS
- 导出当前 CSS
- 如环境允许，导入一份 CSS 文件
- `上传配置 / 下载配置`
- GitHub 仓库入口

### 7. 主题切换

对每个内置主题至少检查：

- `/entries`
- `/feeds`
- `/settings`

关注：

- 页头是否错位
- 导航壳是否仍可用
- 卡片是否仍可读
- 按钮是否仍可点击
- 输入框和状态提示是否仍可见

如果有真实阅读页数据，再加：

- `/entries/:entry_id`
- 正文宽度、元信息、底部栏是否仍可用

如果要把这条回归固定化，优先用：

```bash
bash scripts/run_server_web_regression.sh --suite theme-matrix
```

## 语义接口检查

发布前至少确认：

- 新增样式不再依赖旧高密度 selector
- 内置主题继续通过：
  - `cargo test -p rssr-app --test test_builtin_theme_contracts`
- 若本轮改了页面结构，至少 spot-check：
  - `data-page`
  - `data-layout`
  - `data-slot`
  - `data-nav`
  - `data-action`
  - `data-field`
  - `data-state`

## 视口

至少两档：

- 桌面宽度
- 小视口，例如 `390 x 844`

如果要把小视口检查固定化，优先用：

```bash
bash scripts/run_server_web_regression.sh --suite regression
```

移动端重点看：

- 顶部导航折叠/展开
- 筛选区可达性
- 阅读页返回路径
- 设置页表单滚动与提交

## Console 门禁

以下情况一律视为未通过：

- panic
- `unreachable`
- 未处理 promise / future 错误
- 新的 WASM 初始化错误
- 新的表单结构或可访问性严重告警

## 结果记录模板

建议每次发布前至少记录：

- 日期
- commit
- 执行环境
- 自动化门禁结果
- 正式 Web 与 Pages 演示的各自结果
- `rssr-web` 结果
- 主题矩阵结果
- env-limited 项
- 是否允许发布

统一脚本会自动在日志目录生成一份初始模板：

- `target/release-ui-regression/<timestamp>/summary.md`

可以直接在这份模板上补完人工结论。

## 相关文档

- [主线验证矩阵](./mainline-validation-matrix.md)
- [手工回归测试清单](./manual-regression.md)
- [`rssr-web` 浏览器手工 Smoke](./rssr-web-browser-smoke.md)
- [Static Web 浏览器手工 Smoke](./static-web-browser-smoke.md)
- [Headless 重构视觉等价验收](./headless-refactor-equivalence.md)
- [环境限制索引](./environment-limitations.md)
- [Web SPA 回归服务脚本](../design/web-spa-regression-server.md)
