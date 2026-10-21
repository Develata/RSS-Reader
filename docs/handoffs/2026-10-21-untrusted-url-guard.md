# 不受信 URL 的解析上限：`url` 根上修复、feed 结构预检与自有入口预检

- 日期：2026-10-21
- 作者 / Agent：Claude Code (Opus 5.5)
- 分支：claude/wonderful-engelbart-fdac91（未推送）
- 当前 HEAD：见本文件所在提交
- 相关 commit：本文件所在提交
- 相关 tag / release：N/A
- 状态：`validated`（独立评审结果见下）

## 工作摘要

`url` 2.5.8 处理每个 `..` 段时从序列化结果末尾反向找 `/`，在根路径上要扫过整个 authority；带长用户名、大量
`../` 的地址解析是平方级（本机：规模翻倍耗时约 4–10 倍）。feed-rs 解析文章链接、重定向、本项目自己的各个入口都会
走到这里。另外 feed-rs 把 `xml:base` 克隆进每个元素（1 MB 基地址 × 2000 条目约 9 s），quick-xml 的命名空间解析
与重复属性检查也随结构放大。

最初的方案是在 feed-rs 之前复刻它的读法、扫描会被当作地址的字符串，三轮独立评审每轮都找到新的读法分叉（未绑定前缀的
回退、吞错后继续解析、解码器时机、原始与反转义值），不收敛。Develata 选定方案 B：

1. **在根上修 `url`**：`vendor/url` 是 crates.io 的 `url` 2.5.8 原样副本，只改 `Parser::last_slash_can_be_removed`
   一个函数（反向查找限制在路径内，结果不变），经 `[patch.crates-io]` 接入，工作区 `exclude` 掉。说明与移除条件见
   `vendor/url/PATCH.md`。
2. **feed 结构预检**（`feed_structure_guard.rs`）：只在字节层面数结构上限，不解码文本、不判断哪些字符串是地址。
3. **自有入口预检**（`rssr-domain::url_guard`）：本项目自己把远端字符串交给 `url` 的地方统一限长与限点数，作为纵深防御。
4. 上游议题草稿（`docs/upstream-drafts/`），提交前给 Develata 看。

## 影响范围

- `Cargo.toml`：`[patch.crates-io] url = { path = "vendor/url" }`，`workspace.exclude = ["vendor/url"]`；
  `Cargo.lock` 里 `url` 不再指向 registry。
- `vendor/url/`（新）：`url` 2.5.8 副本 + `PATCH.md`。
- `crates/rssr-domain/src/url_guard.rs`（新）：`url_path_dots`、`untrusted_url_is_parseable`、`parse_untrusted_url`、
  `join_untrusted_url`（上限 64 个点、8 KiB；`join` 时基地址也计入长度），以及确认补丁生效的 canary 测试。
- `crates/rssr-domain/src/validation.rs`：`parse_feed_url`（配置导入、OPML、WebDAV 拉取、添加订阅）先预检。
- `crates/rssr-infra/src/feed_structure_guard.rs`（新）+ `feed_normalization.rs`：feed-rs 解析前的结构预检；
  条目与站点链接改用 `parse_untrusted_url`。
- `crates/rssr-infra/src/subscription_probe/html.rs`：`<base href>` 与 `<link rel=alternate>` 走 `join_untrusted_url`，
  页面声明的订阅地址最多采纳 64 个。
- `crates/rssr-application/src/subscription_discovery.rs`：用户输入的订阅地址先预检。
- `crates/rssr-web/src/proxy.rs` + `Cargo.toml`：代理的 `?url=` 与重定向 `Location` 先预检（新增对纯领域 crate
  `rssr-domain` 的依赖）。
- `crates/rssr-infra/src/html/live_display.rs`：改用共享的 `url_path_dots`。
- `crates/rssr-infra/Cargo.toml`：新增 `feed-quick-xml`（即 feed-rs 用的 quick-xml 0.41.0，同版本同特性，合并为同一份）。

## 关键变更

- **结构预检**：与 feed-rs 相同的 XML/JSON 判定（前 8 KiB 里第一个 `<` 或 `{`，JSON 直接放行）与相同的分词配置；
  不做命名空间解析（那正是要防的路径），对 quick-xml 的读取错误能继续就继续（与 feed-rs 吞错后继续读一致）。上限：
  - 嵌套 ≤ 1024 层；
  - 作用域内命名空间声明 ≤ 256 个（像 NsReader 一样连重复声明一起数）、原始值 ≤ 256 字节；
  - 属性（第 4 轮评审后）：不带重复检查遍历时没有错误、也没有重复键的标签，feed-rs 带检查的遍历每一步都通过检查，
    得到的属性逐个相同；一旦有错误或重复（“不规整”），两者出错后的恢复位置可能不同，不去对齐。每个标签的属性代价
    计入独立预算（≤ 16 × 输入长度 + 1 MiB）：规整标签按属性个数的平方（第 5 轮：quick-xml 判重哈希表种子固定，
    哈希低位相同的属性名让插入退化成平方级，代价不超过个数的平方，与种子无关），不规整标签按整个标签长度的平方
    （feed-rs 在该标签切出的属性名与出错次数都不超过标签长度）；不规整且含 `xml:base` 字节的标签再按整个标签长度
    计入基地址计费；
  - 基地址克隆总量（第 3 轮评审后改为单调上界）：每个元素计费 = 自身及所有打开祖先上全部 `xml:base` 原始值的字节
    数之和（不规整、又含 `xml:base` 字节的标签按整个标签长度计），不因绝对地址、`/` 开头、解码失败而重置；总和 ≤
    64 × 输入长度 + 1 MiB（第 4 轮从 16 调到 64）。它只依赖“feed-rs 的基地址由这些值逐层得出”：绝对地址不长于自身，相对
    地址拼接后不长于上级加自身，被丢弃时沿用上级，都被这个和覆盖（父元素的和也不超过子元素的，RSS `image` 下的
    子元素用父基地址也被覆盖）。解码成 UTF-8、百分号编码与 IDNA 只让真实长度变长常数倍，真实复制量仍是输入长度的
    常数倍。代价是会多算：几层写同一个绝对地址按几倍计，Atom XHTML 正文里的元素 feed-rs 并不逐个克隆基地址也照样
    计；XHTML 正文里层层带长相对基地址时更明显（12 层 × 92 字节的构造会被拒）。倍数 64 让“feed、entry、content 三层
    各一个约 200 字节绝对地址 + 4000 格表格”这类内容通过（16 时会误拒）；更极端的形状仍可能误拒，真实 feed 未见；
  - 解码器一旦换成与 ASCII 不兼容的编码（UTF-16 等，无论来自编码声明还是 quick-xml 按开头字节自动识别）就拒绝：
    预检按原始字节识别属性，feed-rs 却会按那种编码解码属性名；正文已经是 UTF-8 字符串，这种编码本来就不对。
    每读一个事件都检查一次。
  计数取“可能发生”的最大值，只会多拒绝、不会漏；越界就整个 feed 拒绝（“拒绝解析 feed：…”）。

## 验证与验收

- `vendor/url` 与 registry 原版只差一个 hunk；在隔离副本上跑 `url` 自带的测试：66 个单元测试 + WPT URL 测试数据全部通过。
- 点段复现（release，n = 65536，262 KB 输入）：原版 252 ms，补丁后 2.4 ms，与短 authority 的对照组同级，线性。
- canary：`rssr-domain` 的 `dot_segments_parse_in_linear_time`（4 MB、1 Mi 个 `../`，另起线程、60 s 上限）debug 下约
  0.5 s；变异检查：恢复原版 `parser.rs` 后该测试在 60 s 时明确失败，再恢复补丁。
- 真实 feed（26 个：本机订阅 16 个 + 10 个常见公开 feed，含 Atom `xml:base`、JSON Feed、1.2 MB 大 feed）：feed-rs 能解析的
  23 个全部照常解析，0 误拒；3 个 feed-rs 本身就解析失败。
- 压力契约 `tests/test_feed_structure_guard.rs`：4 MB 点段链接 feed 照常解析（地址超 8 KiB 不进入条目）、1 MB `xml:base`
  × 1000 条目、32000 层命名空间、1 MiB 命名空间值 × 10 万元素、3.2 万重复属性都在解析前被拒，debug 下合计 0.91 s。
- `cargo fmt --all --check`、`cargo clippy -j 1 --workspace --all-targets --locked -- -D warnings`：通过（`vendor/url` 不是
  工作区成员，不参与 fmt/clippy/test，内容与 registry 原版逐字一致，只差补丁那一处）。
- `cargo test -j 1 --workspace --locked`：456 通过，0 失败（第 5 轮修复后重跑；真实 feed 复测仍是 23 个照常、0 误拒）。
- `cargo check -j 1 -p rssr-app --target wasm32-unknown-unknown --locked`：通过（wasm 构建同样使用补丁后的 `url`）。

## 独立评审（Codex，gpt-6-astra，effort high）

- 地址扫描方案（1fd6421 → 539884a）共 3 轮，未通过；每轮都是预检与 feed-rs 读法的新分叉，记录在 git 历史的本文件旧版。
- 方案 B 第 1 轮（fbf2c50）：`url` 补丁判定对所有可达调用等价、修复有效（点段处理按摊还线性）；结构预检 3 个 BLOCKER
  （文档中途 UTF-16 声明让字节层面看不见 `xml:base`、重复 `xml:base` 重置计数、256 字节原始值不是序列化长度上界）+
  2 个 SHOULD_FIX（相对路径替换链按原始字节累加误拒、canary 不能在补丁丢失时明确失败）。已修复：拒绝与 ASCII
  不兼容的编码声明、拒绝重复 `xml:base`、去掉单元素的原始字节上限只保留复制总量（真实长度与原始值只差常数倍）、
  canary 改为线程 + 60 s 超时并做了变异检查。另一个 NICE_TO_HAVE（`path_segments_mut().extend` 在 `file:` 上的平方级）
  本项目没有调用，未处理。
- 方案 B 第 2 轮（c78d00b）：2 个 BLOCKER（无声明、按开头字节 `00 3C 00 3F` 自动识别的 UTF-16BE 绕过声明检查；
  RSS `image` 的子元素自带 `xml:base` 把计费降下来，而 feed-rs 用父元素的基地址）+ 1 个 SHOULD_FIX（以 `/` 开头的
  相对基地址链被累加，正常 feed 可能误拒）。已修复：每次读取后检查解码器编码；每个元素按父子较大者计费；记录最近
  的绝对基地址长度，`/` 开头的值按它加自身计。新增对应单元测试（共 9 个）；变异检查：分别去掉三处修复，各自恰好让对应测试失败。
- 方案 B 第 3 轮（17f6182）：REJECT，3 个 BLOCKER，实测确认（预检放行，550 KB 输入解析约 1 s，对照 2 ms）：`//` 改变
  authority 后 `/` 开头的值用了过期的绝对长度；没加引号的 `xml:base` 出错后，第二个被预检当作有效值重置计费，而
  feed-rs 把它当重复丢弃、沿用上级；Shift_JIS 下解码失败的值被预检用来重置、feed-rs 丢弃。三者都是“重置计费”要复刻
  feed-rs 语义造成的分叉。经 Develata 决定改为上面的单调上界：删掉重置、`absolute_bytes` 与父子取大，三个反例
  合成一个参数化测试（带对照组）。复现：三个反例在解析前被拒（各 < 0.5 ms）。变异检查：恢复“绝对地址重置”让该测试
  失败；去掉“属性错误时按整个标签计”测试仍全部通过（当时据此认为预检看到的有效属性是 feed-rs 的超集，这个结论
  第 4 轮被推翻）。
- 方案 B 第 4 轮（02e6ce9）：REJECT。单调计费本身未发现漏计；1 个 BLOCKER + 2 个 SHOULD_FIX，前两项实测确认：
  - BLOCKER：重复属性出错后，feed-rs 带检查的遍历从 `=` 处恢复、落进引号里面继续切属性，预检不带检查的遍历只看到
    一次错误；K 个不同属性后跟在值里重复 R 次的属性，feed-rs 做 Ω(K·R) 次比较（309 KB 输入放行、解析 308 ms）。
    “有效属性是超集”的说法因此不成立。
  - SHOULD_FIX：同一路径能切出预检看不到的有效 `xml:base`，整标签计费有独立作用，缺回归测试。
  - SHOULD_FIX：三层各一个 201 字节绝对地址 + 4000 格表格（43 KB）被误拒，“正常 feed 不受影响”的说法过满。
  经 Develata 决定：去掉 `MAX_ATTRIBUTE_ERRORS`，改为上面的“不规整标签按长度平方计入独立预算”，整标签计费只用于
  不规整标签；基地址倍数 16 → 64。新增测试：属性恢复反例、隐藏 `xml:base` 反例（只靠整标签计费拒绝）、Atom 表格
  兼容用例、少量不规整属性仍容忍。复现：属性反例在解析前被拒（3 ms），Atom 表格照常解析。变异检查：属性平方计费
  置零、去掉整标签计费、倍数改回 16，各自恰好让对应测试失败。
- 方案 B 第 5 轮（19fb7f3）：REJECT，1 个 BLOCKER + 1 个 NICE_TO_HAVE。确认“规整标签与 feed-rs 属性逐个相同”和
  不规整标签的长度平方包络都成立（每个键至少消耗一个字节、恢复不回退，回查总字节 O(L) × O(L) 次）。
  - BLOCKER：规整标签不计属性代价，而 quick-xml 判重用 `DefaultHasher::new()`（SipHash13，固定零种子）+ hashbrown，
    哈希低位相同的属性名让每次插入扫过同一串满桶，Θ(n²)。实测：8192 个低 14 位为零的属性名，feed-rs 解析是随机名的
    约 4.6 倍，规模翻倍耗时约 3 倍（构造名字需离线算力，约 n × 2^bits 次哈希）。已修复：规整标签按属性个数的平方计
    入同一预算；新增测试（2000 个不同属性被拒），变异检查：该项置零让测试失败。
  - NICE_TO_HAVE：兼容边界。三层同一绝对基地址从 201 增到 321 字节、4000 格表格就会被拒；一个重复属性加 2 KB 属性值
    的标签按长度平方计、在小 feed 里也会被拒。没有证据表明这些形状常见，记为已知边界（见“结果”），不再扩大预算。
  - 另记：真实复制量的上界按 Codex 推导约为 2880 × 输入长度 + 45 MiB 量级（64 倍预算 × 解码 3 倍 × 百分号编码 3 倍
    × 约 5 份复制），仍是线性；不含分配器搬迁与 IDNA。

## 结果

- `url` 补丁已经评审确认；结构预检是纵深防御，第 5 轮问题已修复、未再送 Codex 复审。真实 feed 语料 0 误拒，但计费
  是保守上界，已知会误拒的合法形状：多层重复写长绝对基地址（如三层 321 字节）配大量微小 XHTML 元素；带重复或写错
  属性、又有数 KB 长属性值的标签；单个标签上数百个以上属性。超长/点段过多的地址按设计拒绝。
- 预检仍依赖的依赖行为（升级 feed-rs/quick-xml 时要重新核对）：前 8 KiB 的 XML/JSON 判定、Reader 分词配置、成功
  Start/End 的作用域推进、ASCII 兼容编码下按原始字节识别属性名、带检查与不带检查遍历在无错误无重复时一致。

## 风险与后续事项

- `vendor/url` 是一个需要跟进的本地补丁：`url` 有安全更新时要同步；上游修复发布后按 `PATCH.md` 移除。
- `url`/IDNA 在任意 Unicode 主机名上的最坏复杂度未证明。
- JSON Feed 不做结构预检：feed-rs 会把顶层作者克隆进每个缺作者的条目、并线性去重作者，巨大的作者名或大量不同作者仍有
  放大（feed-rs 的既有行为，待上游）。
- 仍直接解析、输入可信度较高的位置：WebDAV 端点与远程路径（用户设置）、Web 端 `x-rssr-final-url`（本项目代理生成）、
  数据库/BrowserStore 里已序列化的地址（写入时已过预检）。
- 上游：经 Develata 确认已提交 servo/rust-url#1163（点段反扫）与 feed-rs/feed-rs#301（`xml:base` 克隆放大），草稿保留在
  `docs/upstream-drafts/`；#1163 修复发布后按 `vendor/url/PATCH.md` 移除补丁。
