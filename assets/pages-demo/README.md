# Pages RSS 文章快照

- `sources.json`：构建前抓取的源与文章数量上限。
- `snapshot.json`：已抓取、按时间选取的文章及来源记录，供 `pages-demo` 编入 WASM。
- 刷新：`cargo run --locked -p rssr-infra --example pages_snapshot`；修改快照后重新构建 Pages。

当前来源为 [Develata 每日数学](https://develata.me/rss/daily-math.xml)（5 篇）和
[Terence Tao / What's new](https://terrytao.wordpress.com/feed/)（1 篇）。
文章作者、时间、原文地址和 RSS 正文均保留。站点源信息见
[Develata 的订阅页](https://develata.me/about/rss)；Tao 的
[转载说明](https://terrytao.wordpress.com/about/)允许附原文引用转载合理篇幅（例如单篇文章），
因此该源仅预载一篇。文章内容不改变原作者的权利，也不随本仓库代码的 MIT 许可重新授权。

快照数据格式与浏览器存储无关，阶段 D 可以将同一份文章载入内存 SQLite。
这一步只替换演示内容来源；当前 BrowserStore 的首次初始化仍保留已有用户数据。
