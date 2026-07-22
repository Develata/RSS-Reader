# CI/Release 工作流 action 升级：消除 Node 20 弃用警告

- 日期：2026-07-22
- 作者 / Agent：Claude Code (math-architect)
- 分支：main
- 相关 commit：本次 ci commit（升级 action 大版本）
- 相关 tag / release：N/A（仅影响后续 workflow 运行）
- 状态：`validated`（CI 侧由 main push 触发验证；Release 侧待下次打 tag 验证）

## 工作摘要

v0.1.9 Release 运行日志出现 GitHub「Node.js 20 已弃用、被强制在 Node.js 24 上运行」
警告。按警告清单把五个 action 升到当前最新大版本（均已原生 `runs.using: node24`）：

| Action | 旧 | 新 | 涉及文件 |
|---|---|---|---|
| `actions/upload-artifact` | v4 | v7 | release.yml ×5 |
| `actions/download-artifact` | v4 | v8 | release.yml ×1 |
| `actions/setup-java` | v4 | v5 | release.yml、ci.yml |
| `android-actions/setup-android` | v3 | v4 | release.yml、ci.yml |
| `softprops/action-gh-release` | v2 | v3 | release.yml ×1 |

## 破坏性变更核对（对本仓库均无影响）

- upload-artifact v7：新增可选 `archive: false` 直传单文件；内部迁 ESM。
  本仓库只用 `name` + `path`，不受影响。
- download-artifact v5 的路径语义变更只影响「按 artifact-id 下载单个」；v8 起
  哈希不匹配默认报错（更安全）。本仓库是无 `name` 的全量下载（每个 artifact
  落入同名子目录），行为不变，`release-assets/**/*` 通配继续命中。
- setup-java v5 / gh-release v3：仅 Node 24 运行时迁移。
- setup-android v4：默认 cmdline-tools 升到 20.0；本仓库随后显式
  `sdkmanager` 安装 platform-tools / android-34 / build-tools;34.0.0 / NDK，
  不依赖默认组件版本。
- 新版本要求 runner ≥ 2.327.1：GitHub 托管 runner 满足。

## 保留项与未动项

- `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true`（三个 workflow 的 env）保留：
  升级后成为无操作，但可兜底未来引入的 node20 action，成本一行。
- `actions/cache@v5`（setup-dioxus-cli 复合 action）未升 v6：未被弃用警告点名
  （已原生 node24），超出本次范围。
- docker.yml 无被点名 action，未改。

## 验证与验收

- CI（push main 触发）：android-check job 实际执行 setup-java@v5 与
  setup-android@v4，绿即验证通过。
- Release 侧（upload/download/gh-release）需等下次打 tag 才真正执行；
  语义核对见上，风险为低。
