# Contributing

感谢你愿意给 RSS-Reader 贡献代码、文档或测试。

这个项目欢迎改进，但有一个前提：**任何贡献都不能偏离 `docs/` 里已经明确写下的核心设计哲学。**

开始之前，建议先读：

- [根 README](./README.md)
- [文档索引](./docs/README.md)
- [功能设计哲学](./docs/design/functional-design-philosophy.md)
- [前端命令与界面接口清单](./docs/design/frontend-command-reference.md)
- [主题作者选择器参考](./docs/design/theme-author-selector-reference.md)

## 最重要的规则

RSS-Reader 的核心边界长期只围绕四类能力：

- 订阅
- 阅读
- 基本设置
- 基础配置交换

这意味着以下方向默认不接受：

- AI 总结、AI 分析、文本改写
- 推荐流、社交、评论、内容平台化能力
- 复杂标签树、规则引擎、重型管理系统
- 任何会明显冲淡“快速进入阅读”目标的功能膨胀

如果一个改动不能直接改善“订阅、阅读、基本设置、基础配置交换”中的至少一类，就应该先停下来重新判断。

## 贡献时的判断标准

提交前请先自查：

1. 这个改动是否直接改善订阅管理？
2. 这个改动是否直接改善进入阅读和持续阅读的体验？
3. 这个改动是否保持设置页克制？
4. 这个改动是否尽量不破坏多平台兼容性？

如果上面的问题答不清楚，最好不要直接实现。

## 开发约定

- UI、应用服务、基础设施保持分层
- 不要在 UI 组件里直接写 SQL、HTTP 或 feed 解析逻辑
- 保持本地优先
- 兼容性字段可以保留，但不要把它们扩成主线产品能力
- 样式可以自由优化，但需要继续保持“高度支持自定义 CSS”

## 提交前建议执行

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

如果改动影响 Web 交互，建议再补做一轮：

- `dx bundle --platform web --package rssr-app --release --debug-symbols false --out-dir target/web-e2e`
- 使用 `rssr-web` 做浏览器回归
- 更新 [全局浏览器回归报告](./docs/testing/global-browser-regression.md)

## 文档同步

如果你的改动改变了行为边界、用户流程、配置方式或测试结论，请同时更新：

- `README.md`
- `docs/`
- `specs/001-minimal-rss-reader/`

特别是：

- 新功能如果已经实现，`spec / plan / tasks / quickstart` 要同步
- 新的贡献约束或产品边界变化，要优先落回 [功能设计哲学](./docs/design/functional-design-philosophy.md)

## 风格建议

- 优先做小而完整、能直接改善阅读体验的改动
- 不为了“像别的产品”而复制超出边界的功能
- 借鉴成熟 RSS 产品时，只学习它们对订阅和阅读真正有帮助的部分

如果你不确定某个方向是否越界，先以“不要偏离设计哲学”为原则收窄方案。
