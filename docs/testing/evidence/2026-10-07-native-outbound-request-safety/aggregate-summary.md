# 发布前 UI 预检结果

- started (Unix ms)：1791375915168
- commit：88b6abb32b992c7b19bb81ff5718ebb4cd8a6be1
- profile：debug
- 静态 Web 端口：8091
- rssr-web 端口：18081
- 日志目录：\\?\E:\gitclone\RSS-Reader\target\outbound-security\aggregate
- outcome：completed
- exit code：Some(0)

## 状态

- 自动化门禁：passed
- browser / wasm contract harness：skipped
- Web bundle：skipped
- rssr-web smoke：skipped
- 固定 smoke 套件：skipped
- 静态 Web + SPA fallback：skipped

## 阶段与日志

| stage | status | exit | ms | log | detail |
| --- | --- | --- | --- | --- | --- |
| wasm-check | passed | 0 | 5253 | automated-gates.log |  |
| app-tests | passed | 0 | 73228 | automated-gates.log |  |
| host-contracts | passed | 0 | 27404 | automated-gates.log |  |
| web-tests | passed | 0 | 33480 | automated-gates.log |  |
| browser-contracts | skipped |  | 0 | browser-contracts.log |  |
| web-bundle | skipped |  | 0 | web-build.log |  |
| web-auth | skipped |  | 0 | rssr-web-auth-smoke.log |  |
| web-browser-feed | skipped |  | 0 | rssr-web-browser-feed-smoke.log |  |
| reader-theme-matrix | skipped |  | 0 | fixed-smokes.log |  |
| small-viewport | skipped |  | 0 | fixed-smokes.log |  |
| proxy-feed | skipped |  | 0 | fixed-smokes.log |  |
| fixed-browser-feed | skipped |  | 0 | fixed-smokes.log |  |
| spa | skipped |  | 0 | spa-server.log |  |

## 日志与产物

- rssr-web 服务日志：rssr-web.log
- 固定 smoke 目录：static-web-reader-theme-matrix / static-web-small-viewport-smoke / rssr-web-proxy-feed-smoke / rssr-web-browser-feed-smoke

## 结果记录补充

- 执行环境：
- env-limited 项：
- host / sqlite contract harness：
- wasm / browser contract harness：
- /entries：
- /feeds：
- /settings：
- /reader/{entry_id}：
- 静态 reader seed smoke：
- 默认主题：
- Atlas Sidebar：
- Newsprint：
- Amethyst Glass：
- Midnight Ledger：
- 是否允许发布（需手工结论）：
