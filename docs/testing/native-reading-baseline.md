# 原生宿主阅读性能基线

本流程比较两份明确的 release 构建，在真实 Windows WebView2 和 Android WebView 中记录阅读操作与内存。它不验证原生渲染器，也不把结构重构直接当作性能收益。

## 前置条件

- 分别记录源码提交、二进制 SHA-256、编译工具链、设备、显示尺寸和刷新率。
- 两个工作区必须使用独立构建输出目录，避免 Cargo 指纹与同名输出文件交叉复用。最终二进制需再次校验。
- SQLx 迁移校验包含 SQL 文件的换行字节。除了 Git 提交，还应记录实际迁移文件哈希；比较构建使用相同字节的迁移输入。不要为基准测试改写已有数据库的迁移记录。
- Windows 使用全新、独立且已迁移的测试库；用 `seed_reading_native.py` 生成 5000 篇文章、24 篇长正文、每篇 120 段和 4 张本地图片。启动前复制同一份种子双库。
- Android 仅使用已确认的测试手机和测试数据，先备份；核对两个 APK 的包名、签名、ABI 和 SHA-256。覆盖安装使用 `adb install -r`，禁止清数据或卸载日常应用。
- 等待所有构建结束，关闭本任务先前启动的测试实例。保留用户其他程序，记录环境负载；不能把机器受其他任务干扰的样本写成空闲基线。

## 采样

`measure_reading_windows.ps1` 和 `measure_reading_android.ps1` 使用 A、B、B、A、A、B 顺序，每版三个冷进程轮次；操作前先统一来源分组、全部来源、搜索为空。

每轮运行 `browser/reading_native_baseline.mjs`：

- 搜索单篇、清除搜索、展开来源，默认各重复 5 次。阅读往返默认也为 5 次，可通过 Android `-ReaderSamples` 明确配置；同平台 A/B 必须一致，汇总保留实际次数。
- 以真实输入事件的时间戳为起点，记录首个满足结果条件的 DOM 更新；另记录两次 requestAnimationFrame 后的代理时间。
- 以 CDP 输入在正文滚动 1800 CSS px，保存回调间隔。Android 同时重置并读取该次滚动的 gfxinfo 原始数据。
- 列表、展开目录、正文、返回后，分别静置 4 秒后采 3 次内存。
- Windows 只纳入与指定可执行文件对应的进程及创建时间有效的子进程，累加 private working set；出现计数缺失时最多尝试 3 次采样（2 次重试），保留重试证据。
- Android 通过 activity process 的 packageList 确定应用及隔离渲染进程，分别读取 PSS 后求和。无法确认完整归属时测试失败。
- 保存状态截图、尺寸、错误、进程身份和原始测量。失败轮次不能混入正式汇总。

参数见脚本的 `param` 块；Windows/Android 均要求传入已核对的二进制哈希。输出目录必须不存在；不要复用已有轮次目录掩盖失败记录。Android 脚本会在提前结束且停留于 A 时恢复 B 的安装，但用户偏好和最终页面需在验收结束后另行检查、恢复。

## 解读限制

- `summarize_reading_baseline.py` 要求每个平台每版三个成功轮次，样本数与显式参数一致且 A/B 相等。报告三轮中位数的中位数，并保留每轮数值和范围。
- 冷进程不等于冷磁盘缓存；Windows 每轮使用新的 WebView 用户目录，Android 保留已有 WebView 数据。不能横向直接比较两个平台的启动数字。
- `startupObservedUpperBoundMs` 包括调试连接和轮询，是观测到就绪的上界。Android `activityFirstDrawMs` 来自 `am start -W`，是 Activity 首帧，不能写成正文加载完成时间。
- 两次 rAF 是呈现代理，回调间隔不是显示器帧率。Android gfxinfo 是 View 帧证据，不覆盖所有 WebView 合成器工作。
- Windows private working set 与 Android PSS 的含义不同；两者只能分别做同平台对照。普通 working set 相加会重复计算共享页。
- 三轮是描述性基线，不是统计显著性证明。目录功能增加、查询策略、持久化位置等若同时改变，应报告累计变更，不能把全部差异归因于 Rust 化。
- 测量不是完整发布验收。CI、真实宿主交互、升级兼容、签名和多设备验收应分别记录。

## 2026-10-25 执行约束

- Windows 每轮阅读往返 5 次；Android 旧版第二次返回存在真实功能缺陷，因此两版正式对照均使用 `-ReaderSamples 1`，每版共 3 次独立冷进程阅读往返。旧版失败记录保留，不纳入正式汇总，也没有修改旧版来跑通测试。
- Android 当前包额外完成连续 5 次工具栏返回和 3 次系统返回检查；该修复比 Windows 被测源码多一个 Android 专用提交，两平台源码身份分别记录。
- 该次基线读的是匹配 SQL 字节的测试库，不能覆盖迁移文件换行不同的历史升级兼容性。完整结果与风险见 [交接记录](../handoffs/2026-10-25-reading-native-baseline.md)。
