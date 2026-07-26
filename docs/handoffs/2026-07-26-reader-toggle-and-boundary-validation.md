# 阅读页标记切换不再整页重载；自定义 CSS 校验补上导入侧；设置数值输入不再静默吞输入

- 日期：2026-07-26
- 作者 / Agent：Claude (math-architect)
- 分支：main
- 当前 HEAD：`d70b2b5`
- 相关 commit：`d70b2b5`（代码与本记录同一个提交）
- 相关 tag / release：`v0.1.10` 之后，尚未发布
- 状态：`validated`

## 工作摘要

用户明确提出的约束：**不希望当前的阅读体验与视觉设计发生可察觉的变化**。本轮据此挑了三项，
逐项确认过「改完之后用户会看到什么」：

1. 阅读页切换已读/收藏不再整页重载——唯一可见的变化是「文章整篇闪一下」这个现象消失。
2. 自定义 CSS 校验补上配置导入侧的缺口——除非导入的配置里 CSS 没闭合，否则完全看不见。
3. 设置页四个数值输入框不再静默吞掉无效输入——除非输入非数字，否则完全看不见。

三项都不动样式表、不动布局、不动主题、不动数据库与配置格式，无迁移。

## 影响范围

- 模块：
  - `crates/rssr-app/src/pages/reader_page/{intent,reducer}.rs`、`crates/rssr-app/src/ui/runtime/reader.rs`
  - `crates/rssr-domain/src/validation.rs`；删除 `crates/rssr-app/src/pages/settings_page/themes/theme_validation.rs`，
    `themes/{mod,theme_apply}.rs` 改为引用 domain
  - `crates/rssr-app/src/pages/settings_page/preferences.rs`
- 平台：全平台（页面层与 domain）
- 额外影响：无迁移、无 workflow 变更、无配置格式变更

## 关键变更

### 1. 阅读页：标记切换只回写标记（`PatchEntryFlags`）

此前 `ToggleRead` / `ToggleStarred` 成功后返回 `[SetStatus, BumpReload]`，
`BumpReload` 让 `reload_tick += 1`，`reader_page/mod.rs:159` 的 `use_reactive_task`
因此重跑 `session.load()`，而 `begin_loading`（`state.rs:58-75`）会清空
`title` / `body_text` / `body_html` / `source` / `published_at` / `navigation_state`
并重置 `asset_localization_requested`。

结果就是：**每点一次「标已读」或「收藏」，正在读的这篇文章会整篇清空再重绘**——
标题变回「正在加载…」，正文消失，底部上一篇/下一篇栏重置，按钮文案先翻成错的再翻回来。
标记本身只是两个布尔字段，用不着这套。

新增 `ReaderPageIntent::PatchEntryFlags { entry_id, is_read: Option<bool>, is_starred: Option<bool> }`，
只写这两个字段，并带与 `ApplyLoadedContent` / `SetError` 相同的归属校验
（`entry_id != current_entry_id` 直接丢弃，防止翻页后迟到的切换结果改到新文章上）。

`BumpReload` **保留**，但只剩一个使用者：`LocalizeEntryAssets` 返回 `Ok(true)`
（`ui/runtime/reader.rs:67`）——正文图片被改写过，这时候确实需要重新读一遍正文。

顺带消掉的连带开销：一次切换原本是「一次正文库读 + 一次图片本地化请求」，
若本地化确实做了事还会再 `BumpReload` 一次导致第二次正文读。

### 2. 自定义 CSS 校验：搬进 domain，并补上导入侧

`validate_custom_css` 原先只在页面层（`themes/theme_validation.rs`），
调用点只有设置页保存与主题应用两处；**通过配置包导入或 WebDAV 拉取进来的 `custom_css`
不经任何校验直接进 `<style>`**，一段没闭合的 CSS 会把后面的样式规则整段吃掉。

函数原样搬进 `rssr_domain::validation`，并在 `validate_config_package` 里调用。

两个刻意的决定：

- **返回 `Result<(), &'static str>` 而不是 `DomainError`**。两个调用方包装方式不同：
  设置页要把原因直接拼进 `"自定义 CSS 格式无效：{err}"`，配置包校验要包成 `DomainError`。
  保持原始 reason 字符串，设置页的提示文案因此**逐字不变**。
- **不放进 `validate_user_settings`**。那个函数在每条写设置的路径上都会跑，
  一旦把 CSS 校验塞进去，用户早已存好、浏览器也能正常解析的 CSS 可能在某次保存时突然被拒。
  用户明确要求不改变现有体验，这里按最保守的方式落。测试
  `user_settings_validation_ignores_custom_css` 就是这条边界的守卫。

### 3. 设置页数值输入：无效输入不再静默丢弃

四个数值框此前是 `if let Ok(..)` 没有 `else`：清空或输入非数字时草稿悄悄保留旧值，
界面显示的却是用户刚输入的内容，保存写回旧值且全程无提示——所见与所存不一致，且没人告诉用户。

改为 `match`，解析失败时置位组件内的 `InvalidNumericFields` 标记并渲染一行提示，
解析成功时清除。**「解析失败就不写入草稿」这一既有行为没有改动**，只是把它说出来。

提示复用既有的 `data-slot="page-intro"` 槽位（该 section 本来就有一条同样形态的说明文字），
不新增任何样式，只在输入无效时出现。

标记存的是**置位时的草稿值**而不是一个布尔：草稿被整体替换后（拉取远端配置、恢复设置），
存的值与新草稿不再相等，提示自动失效，不会残留在一个已经被换成合法值的输入框下面。
判定发生在渲染期的比较里，不写信号，因此不需要 `use_effect`，也不给按键路径多加渲染。

## 复核意见与落地情况

一次独立正确性复核（子代理）逐条核对了三项改动，无 CRITICAL / HIGH。以下为其提出的问题及处理：

- **（已修，MEDIUM）无效输入提示会残留在一个已经合法的值下面。**
  复核指出我在风险清单里「不用 `use_effect`，因为会给每次按键多加一次渲染」的理由**用错了地方**：
  真正需要重置的两个点是 `SettingsPageSession::apply_loaded_settings` 与 `restore_settings`
  （对应「从 WebDAV 拉取配置」「恢复设置」），它们不是按键路径，那条理由对它们不成立。
  可复现场景：在「刷新间隔」里输入 `abc`（提示出现），不修正就去拉取远端配置，
  输入框被换成拉取到的合法数字，而提示还在说「当前输入不是有效数字」。

  修法没有采纳复核建议的「把状态提到 `SettingsPageSession`」——那会把一个纯 UI 细节
  塞进会话层。改为让标记**记录置位时的草稿值**（`Option<u32>` / `Option<f32>` 而不是 `bool`），
  渲染期比较：草稿被整体替换后值不再相等，提示自动失效。判定发生在比较里、不往信号里写，
  因此既不需要 effect，也不会给按键路径多加渲染，状态仍然留在组件内。

- **（接受并记录，MEDIUM）阅读页开着时，底部导航栏不再随标记切换刷新。**
  复核先确认了本轮的关键假设成立：`entry_repository.rs:618-654` 与 `find_adjacent_entry_id`
  表明**当前文章自身的 `is_read`/`is_starred` 不参与邻居计算**，所以切换标记本身不会让
  导航栏过期。真正的差异在别处：以前每次切换都顺带整体重取一次导航，因此能捎带上
  「后台自动刷新期间新插入的同订阅文章」；现在要等换文章才会重取。
  暴露面：后台刷新恰好在阅读期间落地 **且** 用户切换了标记 **且** 随后点了「下一篇同订阅文章」，
  此时可能跳过新到的那篇，直到离开该文章再回来。

  按取舍接受：为这点新鲜度保留整页重载，等于把「文章整篇闪一下」这个用户能直接看见的
  问题留着。而且用户**不**切换标记时导航本来就同样不刷新，改动后行为反而更一致。
  已记入下方风险清单。

- **（记录，LOW）导入是原子的：CSS 不闭合会让整份配置包导入失败，订阅列表也一并不导入。**
  这与 `validate_config_package` 既有的其他检查（重复 URL、设置越界）行为一致，不是新引入的模式。
  复核同时确认**导出/推送路径不调用** `validate_config_package`
  （`import_export_service.rs:171-286`、`file_format.rs:5-24`），
  因此存量 CSS 有问题的用户仍然能正常导出与推送，没有新的拒绝点。

- **（确认，LOW）`data-state="invalid"` 在 `assets/` 里没有任何匹配规则**，
  提示沿用 `[data-slot="page-intro"]` 的既有样式，与该 section 里本来就有的说明文字一致——
  即用户要求的「不产生视觉变化」成立。该属性目前是惰性的，留作将来可能的样式钩子。

- **（记录，LOW，非本轮引入）`SetStatus` 不带 `entry_id`**，reducer 无条件应用。
  用户在异步切换返回前离开文章时，提示文案可能落在新文章上（标记写入本身已被
  `PatchEntryFlags` 的归属校验挡住）。改动前是同样的形状，本轮未处理。

复核另外明确确认无误的点：首屏图片本地化仍会正常触发；`reload_tick` 没有其他消费者；
`PatchEntryFlags` 的归属校验与既有两处结构一致且 `current_entry_id` 必然先于异步结果落地；
`validate_custom_css` 函数体与被删文件逐字节相同、两处提示文案未变；
`invalid.write()` 的临时借用不跨 `update_draft`，无 borrow panic 风险；
`parse::<f32>()` 对 `"1e10"`/`"inf"`/`"NaN"` 的接受行为改动前后一致，仍由保存时的范围校验拦下。

## 验证与验收

### 自动化验证

- `cargo fmt --all --check`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过（0 输出）
- `cargo test --workspace`：通过（33 个测试二进制全部 ok，0 failed）
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过
- `cargo check -p rssr-infra --target wasm32-unknown-unknown --all-targets`：通过
- `cargo check -p rssr-domain --target wasm32-unknown-unknown --all-targets`：通过
- `cargo check -p rssr-app --target aarch64-linux-android`：**未能执行**，本机缺 NDK 的
  `clang.exe`，与本次改动无关。

### 新增测试

`rssr-app`（`reader_page::reducer`）：

- `patch_entry_flags_keeps_the_article_on_screen`：切换标记后，标题、正文、来源、发布时间、
  导航状态、`reload_tick`、`asset_localization_requested` 全部保持不变，且只改被指定的那个标记。
  这条就是「文章不再闪」的守卫。
- `patch_entry_flags_from_a_previous_entry_is_discarded`：翻页后迟到的切换结果不改到当前文章。

`rssr-domain`（`validation`）：

- `accepts_css_with_quotes_comments_and_nesting` / `rejects_css_that_would_swallow_following_rules`：
  搬迁后的行为守卫（引号内的花括号、注释内的花括号、转义引号、嵌套 `@media`、`url(...)`）。
- `rejects_config_package_with_unbalanced_custom_css`：导入侧缺口已闭合。
- `user_settings_validation_ignores_custom_css`：设置保存路径**不受影响**。

### 手工验收

- 未执行。建议回归：
  - 阅读页连续点「标已读」/「收藏」，文章不应再闪；底部上一篇/下一篇仍然正确
  - 导入一份 `custom_css` 没闭合的配置包，应被拒绝且给出原因
  - 设置页清空「刷新间隔」，应出现提示；填回数字后提示消失

## 风险与后续事项

1. **阅读页开着时导航栏不再随标记切换刷新**（复核提出，按取舍接受，理由见上）。
   若后续要补，正确做法是加一条只重取 `reader_navigation` 的轻量命令，
   **不要**退回 `BumpReload`——那会把「文章整篇闪一下」原样带回来。
2. `session.snapshot()` 每次渲染仍深拷贝整份状态（既有开销，见上一轮记录）。
3. **列表 SQL 分页**仍未做，依旧需要先设计分组聚合接口。
4. **桌面图片代理 `rssr-img://` 仍是「注册了但走不通」**，待决定接通还是删除。
5. 页面层其余已确认未改的问题见 `2026-07-26-audit-remediation-round-2.md`。

## 给下一位 Agent 的备注

- 阅读页的标记切换走 `PatchEntryFlags`，**不要**为了「顺便刷新一下」把它改回 `BumpReload`：
  那条路径会清空正文，用户能直接看见。`BumpReload` 现在的唯一合法用途是正文被改写过
  （图片本地化）之后重新读正文。
- 自定义 CSS 的校验规则只改 `crates/rssr-domain/src/validation.rs`。
  加规则前先想清楚它会不会作用到设置保存路径上——`validate_user_settings` 里**没有**
  CSS 校验是刻意的，有测试守着。
