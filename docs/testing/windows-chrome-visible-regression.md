# Windows Chrome 可见窗口检查

v0.2 C4c 已退役旧可见窗口脚本和静态 SPA helper；保留本文件名以兼容链接。当前自动化入口是 [服务器浏览器回归](./server-web-regression.md)，使用 headless Chrome；不能把自动化结果写成用户看过页面。

需要观察真实 Windows Chrome 窗口时：

1. 用 Git Bash 运行 `bash scripts/run_rssr_web_browser_smoke.sh`，创建隔离资料库并取得登录地址和测试凭据。
2. 在独立的 Windows Chrome profile 中打开该地址，真实登录。
3. 按 [手工回归清单](./manual-regression.md) 检查页面、主题与小视口，保存本次截图、环境、commit 与结论。
4. 只关闭本次启动的窗口和服务；不要修改日常浏览器资料或借用他人的进程作为通过证据。

Chrome MCP 或其他 CDP 工具只是操作方式，连接失败不等于产品失败，也不能由此宣称已手工验收。需要固定的自动断言时，运行服务器回归的 regression、theme-matrix、feed-smoke 与 acceptance 套件；不再使用旧静态数据注入流程。

Windows 原生 WebView2 与 Windows Chrome 是不同平台证据。WebView2 的显式 target 连接、应用复制与数据目录隔离见 [原生隔离说明](./manual-regression.md#验收准备与-windows-原生隔离)。Android / macOS 的真实设备交互另行验收。
