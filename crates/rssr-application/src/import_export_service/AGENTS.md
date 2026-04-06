# 配置交换服务模块说明

本目录负责配置导入导出与配置交换规则。

## 职责边界

- `import_export_service.rs`
  - 应用层主流程
  - 配置导入 / 导出 / 远端同步入口
- `rules.rs`
  - 配置包校验规则
  - 字段导入约束
- `tests.rs`
  - 配套测试

## 模块目标

- 保持配置交换“直白、可恢复、可验证”
- 重点服务于：
  - 用户设置
  - 订阅列表
  - JSON / OPML 基础交换
- 不把它扩张成复杂同步系统

## 修改约束

- 导入坏数据时优先返回清晰错误，不静默吞掉
- 配置包结构变化要同时考虑：
  - README
  - specs
  - schema/codec 测试
- `folder` 当前只作为互操作保真字段，不重新扩展成产品级文件夹能力
- 不把页面交互细节写进服务层

## 变更后建议检查

- `cargo check -p rssr-application`
- `cargo test -p rssr-application`
- 涉及配置包结构时，再补：
  - `cargo test -p rssr-infra --test test_config_package_codec`
  - `cargo test -p rssr-infra --test test_config_package_schema_consistency`
