# Agent 交接总览

最后更新：2026-04-06  
适用对象：接手本仓库的下一个 Codex / Agent / 多 Agent 协作成员

## 文档分工

这份文档是**稳定总览**，不是每次改动都追加的开发日志。

从 2026-04-06 起，滚动式交接记录统一追加到：

- [docs/handoffs/README.md](/home/develata/gitclone/RSS-Reader/docs/handoffs/README.md)

阅读顺序建议是：

1. 先看这份总览，建立整体认知
2. 再看 `docs/handoffs/` 最近几条记录，了解最新实现与风险

## 结论

**只靠当前 `specify` 不够。**

`spec.md / plan.md / tasks.md` 已经能说明产品目标、范围和主要设计约束，但还不足以覆盖以下关键信息：

- 最近一轮跨端行为修复
- `rssr-web` 部署态的真实边界
- GitHub Release / Docker / Android 打包链路的修复历史
- Android / desktop / Web 的特殊交互约束
- 哪些约定已经成为“实现真相”，但没有完整落在 spec 中

这份文档就是用来补这些“只看规格会漏掉”的上下文。

---

## 当前项目状态

RSS-Reader 现在已经不是 MVP 脚手架，而是一个较完整的多端个人 RSS 阅读器实现，核心能力包括：

- 订阅添加、刷新、删除
- 按时间 / 按来源浏览文章
- 阅读页连续阅读
- 已读 / 收藏 / 标题搜索 / 来源筛选
- 自动刷新
- 自动归档
- JSON / OPML / WebDAV 配置交换
- Web 部署态登录保护与 feed 代理
- Android Debug APK 构建链路
- Windows / macOS / Linux / Web / Docker 发布 workflow

当前代码组织已经达到“中高程度模块化”，不是大泥球。

---

## 当前架构概览

### Workspace 分层

- `crates/rssr-app`
  - Dioxus UI
  - 页面、组件、hooks、平台装配
- `crates/rssr-application`
  - 应用服务编排
- `crates/rssr-domain`
  - 领域模型与 trait
- `crates/rssr-infra`
  - SQLite、抓取、解析、OPML、配置交换基础设施
- `crates/rssr-web`
  - Web 部署态服务端包装层
  - 登录、会话、feed 代理、静态资源托管
- `crates/rssr-cli`
  - CLI 入口

### 关键模块

- `crates/rssr-app/src/bootstrap/web/`
  - Web 运行时已拆成：
    - `state.rs`
    - `query.rs`
    - `mutations.rs`
    - `refresh.rs`
    - `exchange.rs`
    - `config.rs`
    - `feed.rs`
- `crates/rssr-app/src/pages/`
  - 页面层已拆分，不再是单文件巨型 UI
- `crates/rssr-web/src/auth/`
  - 认证已拆分为：
    - `config.rs`
    - `rate_limit.rs`
    - `session.rs`
- `crates/rssr-application/src/import_export_service/`
  - 配置交换规则与测试已拆开

---

## 已添加的模块级 AGENTS 文档

为了后续过渡到多 Agent 协作，当前已经新增了 5 个模块级 `AGENTS.md`：

- [crates/rssr-app/src/bootstrap/web/AGENTS.md](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/bootstrap/web/AGENTS.md)
- [crates/rssr-app/src/pages/AGENTS.md](/home/develata/gitclone/RSS-Reader/crates/rssr-app/src/pages/AGENTS.md)
- [crates/rssr-web/src/auth/AGENTS.md](/home/develata/gitclone/RSS-Reader/crates/rssr-web/src/auth/AGENTS.md)
- [crates/rssr-infra/src/db/AGENTS.md](/home/develata/gitclone/RSS-Reader/crates/rssr-infra/src/db/AGENTS.md)
- [crates/rssr-application/src/import_export_service/AGENTS.md](/home/develata/gitclone/RSS-Reader/crates/rssr-application/src/import_export_service/AGENTS.md)

它们的定位是：

- 根级 [AGENTS.md](/home/develata/gitclone/RSS-Reader/AGENTS.md) 负责全局规则
- 模块级 `AGENTS.md` 负责局部边界和注意事项

**不要继续把 `AGENTS.md` 铺满所有小目录。**  
当前策略是“少而稳”，只在职责稳定、容易多人并行修改的模块里放。

---

## 产品与设计哲学

真正的产品边界以这几份文档为准：

- [spec.md](/home/develata/gitclone/RSS-Reader/specs/001-minimal-rss-reader/spec.md)
- [plan.md](/home/develata/gitclone/RSS-Reader/specs/001-minimal-rss-reader/plan.md)
- [docs/design/functional-design-philosophy.md](/home/develata/gitclone/RSS-Reader/docs/design/functional-design-philosophy.md)

需要特别记住的几条：

- 这是 RSS 阅读器，不是文档站、CMS、AI 平台
- 设计优先级是：
  - 阅读流畅
  - 性能敏感
  - 本地优先
  - 配置交换而非完整同步
- 应用内解释性文案已被刻意压缩  
  原则是：说明尽量放 README / docs，软件界面保持克制
- `folder` 当前只保留为 OPML / 配置互操作保真字段  
  不是产品主线功能

---

## 重要的“实现真相”

这些点如果只看 spec，容易漏掉。

### 1. Web 当前真实存储不是 wasm SQLite

当前 Web 的真实实现是：

- 浏览器本地持久化状态
- 当前实现为 `localStorage` 序列化状态

不要再按旧认知把 Web 当成 SQLite / IndexedDB 实现。

### 2. `rssr-web` 是部署包装层，不是核心产品层

`rssr-web` 负责：

- 登录页
- 会话 cookie
- 登录限速
- feed 代理
- 静态资源托管

它不是阅读器核心领域逻辑的归宿。

### 3. Web 与纯静态 Web 有边界差异

- `dx serve --platform web --package rssr-app`
  - 受浏览器 CORS 限制
- `rssr-web` / Docker 部署态
  - 有服务端 `/feed-proxy`
  - 能支持更多受限 feed 源

不要把“纯浏览器 Web 能否读某些 feed”当成解析器格式问题。

### 4. 当前不追求旧版本兼容负担

在 `v1.0.0` 前，项目明确选择：

- 不为旧版本兼容性增加过多代码负担
- 不为了兼容旧实现保留大量冗余路径

所以做架构收敛时，可以更果断，但要同步更新文档和 workflow。

---

## 最近关键修复与隐性坑点

下面这些都是已经踩过、而且容易回归的点。

### 发布 / workflow

- **tag 重新跑不会自动包含新 workflow 修复**
  - 旧 tag 会跑旧版本 workflow
  - 修复发布链后，必须发新 tag 才会生效
- **macOS bundling 的图标路径曾经有过 canonicalize 失败**
  - 现在 workflow 会提前 stage icons
- **Web bundle 的 wasm 路径不是稳定文件名**
  - release check 已改成匹配 hashed 输出
- **Linux 现在发布为 `.deb`**
  - 不是裸二进制 tar.gz
- **Docker 现在发布前会跑 runtime smoke**
  - `/healthz`

### Android

- 之前出现过：
  - 安装后提示“针对旧版安卓开发”
  - 原因是生成工程的 `targetSdk/compileSdk` 没真正改到
  - 现在 [prepare_android_bundle.py](/home/develata/gitclone/RSS-Reader/scripts/prepare_android_bundle.py) 会 patch 生成的 Gradle 文件到 `34`
- 之前出现过：
  - 返回键直接退出 app
  - 现在已经接成优先走应用内导航
- 之前出现过：
  - 按 Home 回桌面，再回 app 后点击无响应
  - 当前已加 `Resumed / Focused(true)` 时恢复窗口可见与焦点
  - 这块仍然建议真机持续留意

### Docker / rssr-web

- 曾经存在真实 bug：
  - 登录后 `GET /entries` 返回 `404 + index.html`
  - 这会让浏览器工具和部分环境把前端路由当失败页
- 现在已修成：
  - 已知前端路由显式返回 `200 + index.html`

### 页面交互

- desktop 曾出现目录点击把 `#hash` 当外部链接打开 `file:///...`
  - 现在目录点击改成应用内滚动
- 移动端目录已改成：
  - 保留目录条
  - 去掉单独的“目录/收起目录”按钮
- Android 目录跳转已加固：
  - 更新 hash
  - `scrollIntoView`
  - 下一帧再次滚动

### Web 本地状态

- 之前 `localStorage` 损坏会被静默当成空状态
  - 现在会备份损坏内容并 warning
- 导入配置时曾有损坏 URL 会 panic
  - 现在改成降级处理

---

## 文章页 / 阅读体验的重要约定

### 文章组织

- 按时间：
  - `月份 -> 日期 -> 来源 -> 文章`
  - 越新的文章越靠前
- 按来源：
  - `来源 -> 月份 -> 文章`

### 偏好持久化

以下偏好现在都会持久化：

- 组织方式
- 是否显示已归档
- 已读筛选
- 收藏筛选
- 来源筛选

不要再把这些恢复成“每次启动重置”。

### 快捷键

阅读页当前快捷键：

- `M`：切换已读
- `F`：切换收藏
- `←`：上一未读
- `→`：下一未读

---

## 跨端一致性现状

### 当前已经较一致

- Web
- Windows
- macOS
- Android
- Docker / `rssr-web`

它们共享：

- 同一产品边界
- 大部分页面结构
- 同一阅读组织与筛选模型

### 仍需继续警惕的平台差异

- Android 生命周期
- `rssr-web` 部署态与纯浏览器 Web 的 feed 能力差异
- 原生端与 Web 在正文资源本地化方面的能力差异

---

## Release / CI 交接重点

下一个 agent 接手时，优先关注：

- [.github/workflows/ci.yml](/home/develata/gitclone/RSS-Reader/.github/workflows/ci.yml)
- [.github/workflows/release.yml](/home/develata/gitclone/RSS-Reader/.github/workflows/release.yml)
- [.github/workflows/docker.yml](/home/develata/gitclone/RSS-Reader/.github/workflows/docker.yml)
- [.github/actions/setup-dioxus-cli/action.yml](/home/develata/gitclone/RSS-Reader/.github/actions/setup-dioxus-cli/action.yml)
- [.github/actions/cleanup-github-runner/action.yml](/home/develata/gitclone/RSS-Reader/.github/actions/cleanup-github-runner/action.yml)

当前这些 workflow 已经包含：

- `checkout/cache` 升级到 Node 24 兼容版本
- `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true`
- CI 拆分成并行 job
- Dioxus CLI 共享 setup action
- runner 预清理
- Android / Web / Linux / macOS / Windows 的 release smoke checks
- Docker runtime smoke

### release 时要记住

- **旧 tag 重新跑不会吃到新 workflow**
- 修 release pipeline 后，应该发新 tag，例如 `v0.1.7`
- 不要默认“重跑 `v0.1.6` 就会带上新修复”

---

## 下一个 agent 的推荐工作顺序

如果今天换人接手，建议按这个顺序理解项目：

1. 先读：
   - 根级 [AGENTS.md](/home/develata/gitclone/RSS-Reader/AGENTS.md)
   - 本文档
   - [docs/design/functional-design-philosophy.md](/home/develata/gitclone/RSS-Reader/docs/design/functional-design-philosophy.md)
2. 再看：
   - [spec.md](/home/develata/gitclone/RSS-Reader/specs/001-minimal-rss-reader/spec.md)
   - [plan.md](/home/develata/gitclone/RSS-Reader/specs/001-minimal-rss-reader/plan.md)
   - [tasks.md](/home/develata/gitclone/RSS-Reader/specs/001-minimal-rss-reader/tasks.md)
3. 然后按所改模块，进入对应模块级 `AGENTS.md`
4. 如果是发布问题，先看 workflow，再看 tag 指向
5. 如果是 Android / Docker / Web 部署态问题，优先按“平台边界”排查，不要先怀疑领域层

---

## 当前 HEAD 附近的重要提交

接手时优先理解这些提交的意图：

- `9a44ce4` `fix: return 200 for protected web app routes`
- `5287393` `fix: restore android window interactivity on resume`
- `e45dba4` `ci: upgrade workflow actions for node 24`
- `51c3b4b` `ci: fix linux release smoke check paths`
- `1c80a50` `ci: strengthen android and docker release smoke checks`
- `b7818fd` `ci: package linux desktop release as deb`
- `f7c98d7` `feat: persist entry browsing preferences and fix android sdk patch`
- `e5c5799` `fix: streamline mobile entry directory navigation`
- `203b301` `fix: keep entry directory navigation inside app`

---

## 一句话交接结论

**下一个 agent 不应该只看 `specify` 就开工。**

正确做法是：

- 用 `specify` 理解产品目标
- 用根级与模块级 `AGENTS.md` 理解代码边界
- 用本文档理解“已经发生过什么、哪些地方最容易回归、哪些修复必须带着记忆继续维护”
