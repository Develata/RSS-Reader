# 2026-07-21 Dioxus 工具链升级 0.7.3 → 0.7.9

## 工作摘要与背景

工作区存在一份未提交的 `Cargo.lock` 漂移（本地 dx CLI 0.7.9 触发的重新解析）。
经用户确认，选择「正式采纳 0.7.9」而非回退：将 lock、清单下限、CI/Docker
工具链 pin 与文档一并对齐，避免 CLI 与库版本长期错位。

## 漂移内容（本次提交的 Cargo.lock 变更）

- **Dioxus 全家族 0.7.3 → 0.7.9**（`dioxus` / `dioxus-router` / 内部 crate）。
  清单声明 `^0.7.3` 属语义化兼容范围，dx 0.7.9 运行时对齐了库版本。
- **`wasm-bindgen` 0.2.114 → 0.2.126**、`wasm-bindgen-test` 0.3.64 → 0.3.76
  （硬约束：`wasm-bindgen-cli` 必须与 crate 版本完全一致，CI pin 随之同步）。
- **净增约 74 个传递依赖**：新版 Dioxus 栈以完整默认特性拉入 `image`
  （rav1e / ravif / exr / tiff / gif / image-webp / zune-jpeg / rayon 等编解码器）。
  本仓库自身对 `image` 的直接依赖仍是 `default-features = false, features = ["png"]`，
  未扩大。
- 常规补丁位升级：`anyhow` 1.0.102 → 1.0.103、`ammonia` 4.1.2 → 4.1.3 等。

## 同步修改的 pin 与文档

| 位置 | 变更 |
| --- | --- |
| `Cargo.toml` | `dioxus` / `dioxus-router` 下限 0.7.3 → 0.7.9 |
| `.github/workflows/ci.yml` | `DIOXUS_CLI_VERSION` ×2 → 0.7.9；`WASM_BINDGEN_CLI_VERSION` → 0.2.126 |
| `.github/workflows/release.yml` | `DIOXUS_CLI_VERSION` → 0.7.9 |
| `Dockerfile` | `cargo install dioxus-cli --version 0.7.9` |
| `Dockerfile.ci-local` | `cargo install wasm-bindgen-cli --version 0.2.126` |
| `CLAUDE.md` / `README.md` / `docs/README.en.md` | 版本引用同步 |

`AGENTS.md`「Active Technologies」与 `specs/*/plan.md` 中的 0.7.3 为
speckit 生成的历史规格记录，按惯例不改写。

## 验证

- `cargo update --workspace --dry-run`：**Locking 0 packages** ——
  清单下限提升后 lock 无需任何再解析，漂移内容即 0.7.9 所需的完整闭包。
- `cargo fmt --all --check` ✓
- `cargo test --workspace`：全绿（exit 0；rssr-app 50、rssr-infra 各集成套件、
  rssr-web 15 等全部 `0 failed`）。
- `cargo check -p rssr-app --target wasm32-unknown-unknown`：通过（exit 0，
  确认 dioxus 0.7.9 / wasm-bindgen 0.2.126 栈在 wasm32 上编译干净）。
- 注：本机 workspace 构建两次出现瞬态 rlib 读取失败（「crate required to be
  available in rlib format」/ prelude 解析级联错误，rlib 实际存在于磁盘）——
  属外部干扰（疑似杀软扫描新写入产物），与版本变更无关；
  `cargo clean --profile dev` 后以 `--jobs 4` 复跑全绿，受影响测试单独运行亦通过。

## 当前状态、风险、待跟进

- 风险：
  - CI 的 dioxus-cli / wasm-bindgen-cli 冷缓存首轮需要重新 `cargo install`
    （composite action 的 cache key 含版本号，自动失效重建）。
  - 三个 wasm 浏览器契约 harness 依赖 `wasm-bindgen-test-runner` 0.2.126，
    本地 `Dockerfile.ci-local` 镜像需重建后才能复现 CI。
  - 传递依赖面扩大（image 编解码器族）会增加 CI 冷编译时长；如后续确认
    web/desktop 产物体积异常增长，可再评估是否向上游反馈特性裁剪。
- 待跟进：
  1. 推送后观察 CI 四个 job（lint-and-test / web-smoke /
     wasm-browser-contract ×3 / android-smoke）首轮全绿。
  2. 上一批次遗留：CI / Linux 跑一轮真实浏览器主题矩阵回归。
