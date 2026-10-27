# v0.2 与主干、公式防护分支的整合

- 日期：2026-10-22
- 作者 / Agent：Codex
- 分支：`v0.2-architecture`
- 最新验收候选：`94cb58f`；最终证据随本文提交，未改变验收候选的产品实现
- 当前产品实现 HEAD：`2c4f472`（整合前为 `5de1068`）
- 相关 commit：`bd4a667`（Feed/URL 实现）、`45917d5`（systemd 模板与历史记录）、`f1897a0`（CI 字体）、`2c4f472`（编号公式布局）、`5d89e9e`（补充截图与 Docker 排除项）、`7ab5eb4`（共用 APT 配置）、`94cb58f`（截图坐标稳定化）
- 相关 tag / release：无
- 状态：`validated`（代码移植与分支验收通过；独立复审、main 合并及公网 Pages 验收未完成）

## 工作摘要

用户要求先处理未合入工作、复核与验收，再合并；打 tag 前仍等待确认。以已确认的 v0.2 架构为基准，核对主干 `d9ebe35` 的 22 个非 merge 提交及 `feat/reader-mathml` 的 `73f8d6e`，逐项保留产品能力与安全修复。

## 影响范围

- infra：RSS HTTP 超时、Feed 预扫描与现有模型预算、正文和发现页 URL 边界。
- domain/application：共享的不受信 URL 输入边界。
- 依赖：`url` 的点段解析修复及其可复现来源。
- 平台：原生、正式 Web、Pages；所有本机编译输出位于 E 盘。

## 关键变更

- 适配主干 `c64b899` 的 10 秒连接 / 30 秒读空闲 / 120 秒总时限；移除请求级固定 30 秒总超时，避免覆盖共享 client 的设置。抓取与订阅探测共用 builder，保留出站策略、User-Agent 与错误 URL 脱敏。
- `feat/reader-mathml` 的公式和正文预算已在 `c2e2278` 适配。本轮引入共享 URL 输入边界、`url` 依赖补丁与 Feed 字节预扫描，保留 JSON 作者、媒体评级、模型体积与 UTF-8 固定等 v0.2 防护。
- Feed 预扫描在 `NsReader` 之前限制命名空间、属性判重工作量与 `xml:base` 累计复制量；随后执行已有模型预算。使用同一 quick-xml 版本与 `Reader::from_str`，不恢复旧分支的编码探测或独立 8 KiB 类型嗅探。已经转成 UTF-8 的正文不能被遗留的 XML 编码声明重新解释。
- URL 通用输入最多 8 KiB、查询/片段之前最多 64 个点（包括 `%2e`，忽略 TAB/LF/CR）；各用例仍保留原有更小限制。发现页仍用当前流式解析器、50 候选和 2 KiB 输出边界；正文的 opaque `data:image/` 豁免保留。
- `vendor/url` 是 crates.io `url` 2.5.8，加 `Parser::last_slash_can_be_removed` 一处改动，将根路径点段的反向搜索限制在路径内。逐文件对照本地 registry，除 `PATCH.md` 外仅 `src/parser.rs` 不同；不改版本，不增加第二个 XML 依赖。保留许可证与上游问题草稿。
- 补入真实 Feed 校准工具；默认忽略，需要显式提供 RSS 文件。不把工具存在当作真实语料验收完成。
- 逐文件复核补入主干的 systemd 部署模板，改用 v0.2 的 `RSSR_DATA_DIR`、共享认证/资料目录和明确 HTTPS public origin；保留 StateDirectory、私有权限与服务隔离，不恢复旧数据目录变量或活动 auth.json。
- 同时保留主干 `.dockerignore` 对 `.claude`、`.workbuddy` 的排除，避免本机 agent worktree / 工具状态进入源码镜像构建上下文。
- 保留两条来源分支的 13 份历史 handoff，顶部标出确切来源与历史属性。同名超时记录分别保留；旧服务器设计归档，原路径仅指向当前契约，避免引导维护者恢复旧架构。
- 尚未开始合并。根检出的任务外修改及其他 worktree 保留。

### 主干独有提交去向

主干是另一路服务器实现；整合以用户确认的 v0.2 架构为准，不重新引入 `rssr-host`、sqlx 双库、BrowserStore 或 feed proxy。按精确提交核对产品语义与安全修复，不能用文件自动合并成功代替这张去向表。

| 主干提交 | 内容与 v0.2 去向 |
|---|---|
| `c611578`、`633f2d2`、`1ae7dad` | 服务器模式设计；由 `v0.2-architecture.md`、阶段 C 计划和当前部署契约取代。历史 handoff 保留，旧设计若归档须明确已取代。 |
| `b0b3099`、`50f61bb`、`f0648ae` | UI 无关 host、Send/Sync、宿主生命周期；对应 `rssr-api::Backend`、infra `LocalBackend` / shared composition 和显式 close，不恢复旧 host crate。 |
| `c64b899` | 慢速 Feed 超时；本轮补齐，共用 builder 的空闲 / 总时限均有回归。 |
| `f7233c4` | Feed、发现、图片、WebDAV 的 PublicOnly；当前 `outbound` 已覆盖 URL、DNS 全部答案与每跳重定向，并增加可配置 allow/deny 和错误脱敏。 |
| `cc4ebda` | host facade / wire protocol；由薄 `rssr-api`、application 公开输入结果、`HttpBackend` 接管，保持单一用例语义。 |
| `d339066`、`c4d7dd2`、`dbc8e73` | 认证 API、解析器成功路径、Origin、32 并发、读 body 前鉴权、大小和时间边界、IPv6、私有资料目录、端点矩阵；当前 auth / api / outbound / store 对应实现与测试已覆盖。写请求要求明确的可信 Origin + CSRF；当前公共 IPv6 允许列表比旧特殊地址排除表更严格。 |
| `9594238`、`a067b54`、`1c903ec` | 远程 UI、每页固定后端、401、响应边界；对应正式 Web bootstrap / `HttpBackend` 及 client_tests，包含结果未知与构建标识失配，不恢复浏览器库后备路径。 |
| `91d5f4e`、`d2615f7` | 匹配的 Linux 服务＋页面包、容器；由 C6 四平台包、同源构建标识、复用受测 Linux artifact 的镜像和卷验收接管。本轮另补 systemd 示例，适配当前环境变量与共享数据目录。 |
| `783f0e0`、`176d8b3`、`183a99c` | feed proxy 退役；当前实现已删除，正式 HTTP 测试断言不再提供此接口。旧 proxy 隔离脚本不恢复。 |
| `16a8d8c` | 测试独立目录；当前真实 HTTP/API 测试分别使用独立临时库，工作区并行测试通过。 |
| `a57ca5b` | 小视口 smoke 历史证据；保留记录，当前服务器验收与 Pages 验收分别提供候选证据。 |

### 公式分支去向

来源固定为 `73f8d6ee94f2926ac49d8e2f4f6b2e3a30f43c3b`，不读取其正在工作的 checkout。

| 来源内容 | 整合结果 |
|---|---|
| MathML / KaTeX 去重、单次树遍历、公式降级、reader CSS | 已在 `c2e2278` 适配，原测试与浏览器正文契约保留。 |
| HTML tree-builder 预算、纯文本降级、归一化 URL 工作 / 输出预算、重复属性、本地化图片复制计费 | 已适配，保留 v0.2 图片 URL 安全规则与 opaque 地址豁免。 |
| `5070d71` 空闲超时 | 与主干 `c64b899` 同项，本轮共用实现。 |
| `1fd6421` 起的领域 URL API、Feed 链接与发现页预检 | 本轮适配；旧 proxy 中同类调用随接口退役，不恢复。 |
| `fbf2c50` 起的 vendor URL 补丁、预扫描及后续第 2–7 轮修复 | 补丁原样保留；结构预算取最终版本，编码 / 类型选择 / 256 层深度与 v0.2 契约对齐。 |
| 压力契约、语料校准工具、上游草稿 | 测试与工具补齐；默认跳过外部语料工具，旧性能数字标记为来源历史，不转作当前测量。 |
| 旧 README / user guide / composition / proxy wiring | 当前文档及 v0.2 接线为准；正文处理相关能力已适配，不引入旧浏览器存储或认证接口。 |

来源分支历史评审不等于本轮适配已经独立复审。

## 验证与验收

- 远端主干与候选已刷新并核对；GitHub 未发现 open issue/PR。
- 修复前：新增 HTTP 测试 2 pass / 2 fail，复现请求级固定超时覆盖 client 总时限；有效 RSS 的超长 namespace 回归失败，复现进入昂贵命名空间处理前没有上限。日志 `timeout-baseline.log` / `prescan-baseline.log`。
- 修复后 `cargo test --workspace --locked`：**886 passed / 0 failed / 4 ignored**。包括现有 C3 模型安全契约、4 个真实 TCP 超时用例、新 Feed 预扫描 / URL 合约和既有正文压力测试。四个 ignored 项没有被算作通过。
- `cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo fmt --all --check`：通过。
- wasm Clippy：正式 `rssr-app`、`rssr-app --features pages-demo`、默认 `rssr-infra`、`rssr-infra --tests --features memory-store,http-backend`：通过；新增测试的 feature gate 单独再编译通过。
- 在隔离的 E 盘目录运行 vendored URL 自带测试：1 个 lib、66 个单元集成测试、WPT URL 数据集、67 个 doc tests 均通过；未把它误算入 workspace 测试。
- 工作区 URL canary 已通过；另用同一测试文件连接未打补丁的 registry 2.5.8 做隔离 mutation，60.01 秒后按预期失败（exit 101，提示补丁缺失）。未修改正在工作的 vendor 文件。日志 `url-unpatched-canary.log`。
- `url` 2.5.8 的点段问题 [#1163](https://github.com/servo/rust-url/issues/1163) 与 feed-rs 的 xml:base 复制问题 [#301](https://github.com/feed-rs/feed-rs/issues/301) 仍 open；这不是对全部上游版本的扫描。

- systemd 模板通过 WSL Debian 的 `systemd-analyze verify --man=no`；校验副本只将不存在的安装路径 `ExecStart` 临时替换成 `/bin/true`，没有安装/启停服务。因此这是单元配置静态校验，不是实际 systemd 部署验收。日志 `systemd-verify.log`。

本机日志：`target/handoff/integration-20261022/`。所有编译、临时文件和 mutation 的 target 均位于 E 盘。

`bd4a667` 已推送并启动非发布验收：

| 工作流 | 运行 | 状态 |
|---|---|---|
| CI，`acceptance_long=true` | [39880628092](https://github.com/Develata/RSS-Reader/actions/runs/39880628092) | 35 / 35 jobs 成功 |
| Pages fresh RSS / release 构建 / 浏览器验收 | [39880632180](https://github.com/Develata/RSS-Reader/actions/runs/39880632180) | 通过，分支部署跳过 |
| 四平台 Web 包 / Docker，`all_platforms=true` | [39880639908](https://github.com/Develata/RSS-Reader/actions/runs/39880639908) | 7 / 7 jobs 成功；不推送镜像 |

Pages artifact `12470067858` 已下载并复算 ZIP SHA-256：`6d3c6823abe2589ebae6bd07619a23a65694d7ff86f5cdb2e4df5b7fff6f436a`，与 GitHub digest 相同。快照新抓取于 `2026-10-23 00:53:13 UTC`，每日数学内容已经滚动更新；本轮是 570 个 MathML、144 张公式图片（全部加载）。自动规则通过，但目视发现 CI 截图中文缺字，现有浏览器安装 action 没有安装 CJK 字体；不能把这组截图记成完整目视验收。随后为截图 job 补充 CJK / OpenType math 字体并重新验收，结果见后续记录。

字体修复仅影响 CI 环境：`setup-browser-tests` 增加显式 `with-fonts` 输入，Pages 与真实 UI 截图 job 安装 Noto CJK 和 Latin Modern Math，通过 fontconfig 检查字体存在；不向产品包添加字体，不给 14 个无截图 WASM 契约 job 增加安装步骤。已用 Debian 包清单确认 `fonts-lmodern` 包含 `latinmodern-math.otf`，actionlint 通过。修复后的远端截图已确认中文可读，详见后续完整候选记录。

### 新鲜 RSS 触发的编号公式布局问题

- `f1897a0` 的 [Pages 运行 39882215340](https://github.com/Develata/RSS-Reader/actions/runs/39882215340)通过，确认 fontconfig 使用 `Noto Sans CJK SC` 和 `Latin Modern Math`；这解决 CI 缺字环境，但不解决下面的公式结构问题。
- 新抓取的 `2026-10-23 · 平方之后为何更简单` 带一个 MathJax `mlabeledtr`。它不在 [MathML Core 的元素列表](https://www.w3.org/TR/mathml-core/#mathml-elements-and-attributes)中；Chromium 156 将它排成 `block math`，其中 `mtd` 变成 block，公式操作数竖排。原始 MathML 在带 Cambria Math 的独立 Chrome 对照里也复现，排除清洗或缺少字体导致的可能。
- 同一真实公式只将行设为 `display: table-row` 后，布局与原生 `mtr` 对照相同：公式表高从 184.66 px 降至 74.38 px，符号与编号在正常行内。`reader.css` 仅为 reader 内的 `mlabeledtr` 加此规则；不修改公式 DOM、操作数或消毒策略，也不引入数学渲染脚本。这是保留编号文本的普通表格行兼容布局，不声称实现 MathML 旧版完整的标签定位规则。
- 新浏览器契约比较带编号行与相同内容的原生行几何尺寸，并检查消毒/重解析/命名空间；修复前 **6 pass / 1 fail**，修复后 **7 / 7 pass**。Pages 对无跨行单元格的编号公式增加行高一致断言，避免只数 `<math>` 而放过排版损坏。
- 日志与截图在 `formula-probe/`、`numbered-baseline-browser.log`、`numbered-fixed-browser.log`；两个 WASM 测试二进制按 Cargo JSON 的确切 executable 路径保存，未用宽泛通配符猜测产物。Rust fmt 和 JS 语法检查通过；完整候选远端验收见下。

### 完整产品候选 `2c4f472`

| 工作流 | 运行 | 状态 |
|---|---|---|
| CI，`acceptance_long=true` | [39883092648](https://github.com/Develata/RSS-Reader/actions/runs/39883092648) | 取消：32 jobs 成功，两个字体安装 job 超时；汇总 gate 失败，未记为通过 |
| Pages fresh RSS / release 构建 / 浏览器验收 | [39883097508](https://github.com/Develata/RSS-Reader/actions/runs/39883097508) | 通过，分支部署跳过 |
| 四平台 Web 包 / Docker，`all_platforms=true` | [39883102202](https://github.com/Develata/RSS-Reader/actions/runs/39883102202) | 7 / 7 jobs 成功；不推送镜像 |

- Pages snapshot 抓取时间 `2026-10-23 01:17:54 UTC`；2 个真实源、6 篇文章、570 个 MathML、144 张公式图片，144 张均加载成功。编号公式的两个单元格实际高度同为 60.4375 px，行布局断言通过。
- Pages smoke artifact `12471467214` 的 ZIP SHA-256 为 `8e69c56f6e98e99f7d3febe80e50c95eae188a286603b932b1a1e9c0d94ac6f1`；RSS artifact `12471531982` 为 `20530e4507f28f59ecaa36612b609ba9c0e15c7d9e75f601fa977ab537dd0969`。下载后均复算并与 GitHub digest 对上。
- 已目视每日数学与陶哲轩文章的标题、正文截图：中文不再缺字，普通 MathML 行内与展示公式可读。原截图只滚到正文开头，编号公式可能在浮动工具栏下方；另一个正文截图还可能早于滚动完成。因此补充即时滚动并等待两帧、将编号公式或图片公式滚到视口中央再截图，补齐可复核的目视证据。这只改截图脚本，不改产品。
- 启动 `readyMs=480.3`，WASM memory `4,587,520` bytes，最大已观测 long task `75 ms`；本地静态 HTTP 服务传输未压缩 WASM body `4,992,463` bytes。不是公网网络、压缩传输或真实手机性能数据。

### 截图补齐与 APT 配置复用

- `5d89e9e` 的 [Pages 运行 39883947947](https://github.com/Develata/RSS-Reader/actions/runs/39883947947)通过，部署跳过；相对 `2c4f472` 只改截图脚本、Docker 排除项和本记录，产品源码与资源未变。新快照时间 `2026-10-23 01:27:08 UTC`，仍为 570 个 MathML、144 / 144 图片加载成功。已目视确认每日数学的 `(1)` 与公式在同一行，陶哲轩正文的图片公式可读，截图中中文完整。
- 代表截图链接现已更新为最终 `94cb58f` 的原样截图：[每日数学编号公式](../testing/evidence/2026-10-22-branch-integration/daily-math-formula.png)、[陶哲轩图片公式](../testing/evidence/2026-10-22-branch-integration/tao-formula.png)。未编辑图像，来源和每张图片的哈希在最终证据 JSON 中。
- smoke artifact `12471044533` 的 ZIP SHA-256：`77936b137f5881d83710cf3b870e4cddc642732677f7b5c9b3e3173927b2c70c`；snapshot artifact `12470919756`：`652ab988a2cee369e1de92f9c59080eae0a7a61d7021daa46e8866b07a1ca56c`。均已下载复算并匹配 GitHub digest。
- 完整产品候选的默认主题验收 job `120185756815` 已通过：`--long` **27 / 27 场景、108 checks、0 unexpected console**。核对了 `ci-ui-default` artifact `12472235827` 中的 `assertions.json`；其 ZIP SHA-256 `4ae589140a3527f8f4ca56297a8eed10c634a7fa2e371ab22f5c3d762cc12e8f` 已复算匹配。
- 新字体安装没有复用此前 native CI 的 APT mirrorlist 修复；同时 Linux Release 的依赖安装仍缺此配置。将同一改写抽成 `scripts/configure_ubuntu_apt.sh`，在三处共用；仅替换 Ubuntu archive 的 Azure 主机，保留 suite、签名与第三方源。字体与原生安装继续使用连接超时 / 重试设置，Linux Release 同步增加这些设置与 10 分钟步骤时限。隔离 Linux fixture 通过（四种配置位置及 suite / Microsoft 来源保留），actionlint、diff check 通过；远端结果见下。未触发 Release 工作流。
- 旧候选 CI 的 Atlas job `120185756733` 日志确认字体安装仍走 Azure 镜像：`01:21:23 UTC` 开始下载 61.2 MB Noto CJK，`01:35:11` 因 15 分钟 job 时限取消，UI 未开始。`7ab5eb4` 的 [Pages 运行 39884707827](https://github.com/Develata/RSS-Reader/actions/runs/39884707827)通过：日志明确改写 `apt-mirrors.txt` 并从 `https://archive.ubuntu.com` 下载 66.0 MB 字体，耗时 3 秒。
- 对 `7ab5eb4` 的截图复核发现，等待两帧后截视口仍可能回到文章页首，自动内容/布局断言通过不能证明图片截到目标。改用 CDP 的文档绝对坐标与 `captureBeyondViewport` 直接截正文/公式区域，并把区域坐标写入结果；不再依赖截图瞬间的窗口滚动位置。最终区域截图已目视复核，不把错误视口图片记为公式验收。

### 最终验收候选 `94cb58f`

| 验收 | 运行 / 证据 | 结果 |
|---|---|---|
| CI，含 `--long` | [39885292548](https://github.com/Develata/RSS-Reader/actions/runs/39885292548) | **35 / 35 jobs 成功；27 / 27 场景、108 checks、0 unexpected console** |
| Pages，真实 RSS、release 构建及截图 | [39885298592](https://github.com/Develata/RSS-Reader/actions/runs/39885298592) | 通过，部署跳过 |
| 四平台 Web 包 / Docker | [39883102202](https://github.com/Develata/RSS-Reader/actions/runs/39883102202)，源码 `2c4f472` | 7 / 7 成功；后续只改文档、截图、上下文排除与安装依赖配置。未将此旧构建标识当成 `94cb58f` 产物 |

- [结构化证据](../testing/evidence/2026-10-22-branch-integration/verification.json)保留确切源 SHA、各 job 状态、真实包/容器检查、artifact ID / digest、来源 RSS 哈希、公式布局与性能数据。已下载的 ZIP 均复算 SHA-256，截图原样保留；未下载的包没有标记为本机复算通过。
- Pages 最新抓取于 `2026-10-23 01:41:25 UTC`，2 源 / 6 篇，570 个 MathML，144 / 144 张公式图片加载。编号行两格高度同为 60.4375 px；已目视检查编号公式、中文和陶哲轩正文可见图片公式。不是对全部 570 个公式逐一目视检查。
- `readyMs=530.7`，WASM memory `4,587,520` bytes，最大已观测 long task `99 ms`，未压缩 WASM body `4,992,463` bytes；这些仍是 CI 本地静态服务数据，不是实际公网 Pages 或手机数据。
- 新 Pages 安装 66 MB 字体仅 2 秒，Atlas 主题也在 2 秒内下载字体并完成实际 UI 回归。旧 Atlas / midnight 的失败日志确认二者均停在 Azure Noto 字体下载，分别在约 905 / 906 秒取消，UI 没有开始。
- 最终默认主题 artifact `12473025706` 已下载并复算 ZIP SHA-256：`f87a5d9815f4d242da56df3df1bd721a748e77e7cf02b5d6ced45d8093ca89ae`；其中 `default-acceptance/assertions.json` 明确 `long=true`、27 场景、108 检查全部通过且无意外控制台记录。midnight 字体下载 3 秒，实际 UI 回归也通过；没有只重跑后把失败忽略。

## 风险与后续事项

- 字节预扫描必须先于 NsReader 的命名空间处理，且与 v0.2 固定 UTF-8 输入保持一致；不能覆盖现有模型预算。
- 既有 `576cba4` 的 CI 结果不代替本次改动后的验收。
- 主干与公式分支的提交去向表已补齐。最终独立复审仍待完成；通过后在本任务 worktree 解决 main 合并冲突，推送并完成实际公网 Pages 子路径验收。公网部署依赖合入 main，不能用本轮分支静态站验收替代。
- 新复审包：`target/handoff/2026-10-22-integration-final-review.md`（ignored），包含确切候选、当前 C6 / D / Feed 防护范围与已接受边界；已请求用户确定独立只读审查者，尚未将主审自查记成独立评审。
- 用户后续授权先完成这些工作再合并；不再将 main 合并列为完全未获授权，但不得越过其“处理完成后”的条件。打 tag / Release 仍需用户最终点头。

## 给下一位 Agent 的备注

- 历史比较只读 Git 对象；工作树 `target/audit-20261009/worktree`。
- 没有 tag、Release 或 GHCR 发布；禁止在 C 盘编译。
