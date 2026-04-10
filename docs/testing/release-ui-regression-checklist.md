# 发布前 UI 回归清单

这份清单服务于发布前最后一轮 UI 验收。

它和 [手工回归测试清单](./manual-regression.md) 的区别是：

- 手工回归清单偏通用
- 这份清单偏发布门禁
- 它明确要求同时覆盖：
  - 路由与基本交互
  - 主题与 CSS 契约
  - Web 两种入口
  - 关键持久化与配置交换

## 使用时机

在以下场景执行：

- 准备打版本 tag 前
- UI shell / bus / facade 有结构变更后
- 主题系统或语义接口有收口后
- Web 入口、登录门禁、静态构建回归路径有变更后

## 发布前最小自动化门禁

至少先通过：

- `cargo check -p rssr-app`
- `cargo check -p rssr-app --target wasm32-unknown-unknown`
- `cargo test -p rssr-app`
- `cargo test -p rssr-app --test test_builtin_theme_contracts`
- `cargo test -p rssr-infra --test test_subscription_contract_harness`
- `cargo test -p rssr-infra --test test_config_exchange_contract_harness`
- `cargo test -p rssr-web`

如果这组自动化没过，不进入后续手工回归。

## 发布前 Web 路径

至少覆盖两条入口：

### 1. 纯静态 Web + SPA fallback

用途：

- 验证 `rssr-app` 的默认 web 形态
- 验证本地浏览器门禁
- 验证主题、语义 DOM、页面结构

入口：

```bash
dx build --platform web --package rssr-app
bash scripts/run_web_spa_regression_server.sh --debug --skip-build --port 8091
```

### 2. `rssr-web` 部署壳

用途：

- 验证服务端登录门禁
- 验证代理态 feed 导入
- 验证真实部署入口下的路由与页面

建议覆盖：

- `/`
- 登录 / 登出
- 至少 1 个需要代理才能导入的 feed

## 页面与主题矩阵

发布前至少检查以下页面：

- `/entries`
- `/feeds`
- `/settings`
- `/reader/{entry_id}` 或等价真实阅读页路径

主题至少覆盖：

- 默认主题
- `Atlas Sidebar`
- `Newsprint`
- `Amethyst Glass`
- `Midnight Ledger`

## 核心检查项

### 1. 启动与路由

- 打开 `/`
- 确认默认能进入预期首页
- `订阅 / 文章 / 设置` 切换正常
- 浏览器刷新后仍能回到当前页
- 无白屏、无死循环、无不可恢复卡死

### 2. 本地 Web 门禁 / 服务端门禁

静态 Web：

- 首次进入需初始化本地用户名和密码
- 初始化后可进入应用
- 后续重新打开时，登录表单和状态提示正常

`rssr-web`：

- 未登录访问会进入登录页
- 错误凭证有明确提示
- 正确凭证可进入应用
- 登出后会话被清除

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

### 5. 阅读页

- 正文正常显示
- `标已读 / 标未读`
- `收藏 / 取消收藏`
- `上一篇未读 / 下一篇未读`
- `上一篇同订阅文章 / 下一篇同订阅文章`
- 返回上一页后列表仍稳定

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

- `/reader`
- 正文宽度、元信息、底部栏是否仍可用

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
- 静态 Web 结果
- `rssr-web` 结果
- 主题矩阵结果
- env-limited 项
- 是否允许发布

## 相关文档

- [主线验证矩阵](./mainline-validation-matrix.md)
- [手工回归测试清单](./manual-regression.md)
- [Headless 重构视觉等价验收](./headless-refactor-equivalence.md)
- [环境限制索引](./environment-limitations.md)
- [Web SPA 回归服务脚本](/home/develata/gitclone/RSS-Reader/docs/design/web-spa-regression-server.md)
