# 主题模式选择的宿主评估

核对日期：2026-10-30。产品源码为 PR #47 head `f38c3ba8`，已合入 main `663ef4ee`，两者产品代码相同。实际包、命令与失败诊断见[本批交接](../handoffs/2026-10-30-native-theme-selector-assessment.md)。

## 结论

本轮保留现有主题模式选择。Android 的 HTML `select` 已通过 WebView 提供系统原生选择弹窗；Windows 使用 Chromium 提供的选择控件。另写三项原生菜单没有得到可验证的功能收益，而把 OS 控件嵌入页面会引入窗口、几何、焦点和样式职责。因此，这个候选完成宿主评估，**不作为新增原生控件的实现批次**。这不表示原生界面已经接入，也不表示所有控件都应继续采用 HTML。

App 设置读取规则已经通过 PR #47 合并。前置规则提取在此收口，继续复用现有 Rust 模块，不再为这一候选扩大 Shell 或创建通用页面框架。

## 已验证的宿主边界

| 平台 | 实际观察 | 行为与证据边界 |
| --- | --- | --- |
| Windows 11 / WebView2 156.0.4314.50 | UI Automation 将主题选择暴露为 `ComboBox`，framework 为 `Chrome`、原生句柄为 0；弹出三个 `ListItem` | 实际 EXE、独立 WebView2 profile 与合成 SQLite。CDP 指针和按键进入 WebView2；Esc 取消且焦点留在选择框，选择仅预览，显式保存后重开。不是 Win32 ComboBox，也不是物理键盘/IME 验收。 |
| Android 15 / WebView 156.0.8078.47 | 真机 UIAutomator 观察到 `android.widget.ListView` 和 `android.widget.CheckedTextView`，实际截图是系统选择弹窗 | 独立测试包中实际 adb 点击选择及系统返回键。返回只关闭弹窗，留在 Settings 且不产生输入；选择产生 trusted input/change，预览不写库，显式保存后重开。未测试返回手势、TalkBack 或其他厂商/版本。 |

Windows 的外部存储变更和 focus/visibility/online 通知是受控注入，用于验证 pending 字段保留、未编辑字段回填以及回填不产生用户输入。Android 本轮没有注入外部存储重读；两种平台的通过项不能互相替代。

Chromium 的公开 [SelectPopupDialog 实现](https://chromium.googlesource.com/chromium/src/+/master/content/public/android/java/src/org/chromium/content/browser/input/SelectPopupDialog.java)使用 Android `AlertDialog` 和 `ListView`，与真机观察一致。上游当前源码不是设备 WebView 二进制的精确源码证明。

## 保持的规则与所有权

- `pages/settings_page/preferences.rs` 的 `select#settings-theme-mode` 只向现有 facade 提交枚举值；没有新宿主状态副本。
- 主题预览与 `stored → submitted → pending` 继续由 Settings session 管理。选择、取消、程序回填、显式保存是不同事件；程序回填不能回送为用户输入。
- Android WebView 选择弹窗先消费返回，不把取消升级成页面后退或首页退出。已有 Rust 返回策略继续负责页面和首页返回。
- 自定义 CSS、主题选择器、Settings 保存门禁、Backend/LocalBackend、七 crate 和单库保持现状。

## 方案比较

| 方案 | 能得到什么 | 必须承担的成本 | 本轮决定 |
| --- | --- | --- | --- |
| 保留 HTML select 和宿主弹窗 | 已验证的选择、键盘/返回取消与现有预览保存 | 持续核对 WebView 升级、真实辅助技术与平台行为 | 采用 |
| 新增 Windows 宿主菜单 / Android JNI 选择器 | 宿主显式拥有弹出和回执 | 重复选择器语义；新增请求身份、取消、窗口销毁和线程回传；菜单不等于页面内原生控件 | 无明确收益，不实施 |
| 将 Win32 ComboBox / Android View 覆盖到 WebView 的字段上 | 页面中的该区域使用 OS 控件 | DOM 与 OS 的 DPI、缩放、滚动、裁剪、z-order、焦点、键盘和可访问性同步；自定义 CSS 无法直接作用于 OS 控件 | 超出单字段等价适配，未批准也未实现 |
| 独立的原生设置面板 | 可以验证真正由 OS 控件组成的表单边界 | 明确原生窗口与 WebView 的所有权、设置 session 生命周期、预览撤销、样式兼容及平台实现成本 | 作为后续设计候选，不能直接视为已批准的整页替换 |

本地锁定的 Dioxus desktop 0.7.9 暴露窗口/WebView，且重导出 muda 0.17.2；Wry 0.53.5 有[子 WebView 与 bounds API](https://docs.rs/wry/0.53.5/wry/struct.WebViewBuilder.html)。这些 API 不会自动解决“把 OS 控件放进 DOM 布局”的同步问题。Android 现有 MainActivity/JNI 桥只提供已定义的宿主能力，不应因存在桥就新增一套设置状态。

## 下一步设计的完成标准

若继续推进真正的原生控件，建议先为 **Windows 独立原生设置面板的小样**写出具体设计，只包含主题选择、取消和显式保存。需要明确如何调用现有规则、谁拥有窗口和草稿、离页/销毁如何撤销预览，以及 CSS 能力如何解释；先评估现有依赖可否支撑，再决定技术选型。

通过标准是实际宿主完成加载、编辑、取消、保存、迟到回执和关闭后的归属检查，并给出与现有路径的差异和回退方式。未完成这份设计前，不以字段简单为理由新增框架、crate、整页替换或渲染器迁移。Android 当前已具备的系统选择弹窗继续复用。
