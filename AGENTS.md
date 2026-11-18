# Token Anxiety Dashboard 專案指引

## 語言

- 回覆使用繁體中文（臺灣用語）。
- 程式碼命名維持英文；UI 文字目前以繁體中文為主，保留未來 i18n 擴充。

## 架構

- 這是 Tauri 2 桌面 App，不是 Vercel-only 純前端。
- 前端：React + TypeScript + Vite；本地狀態由 Rust command 寫入 SQLite。
- Provider adapter 必須標示可信度：`official`、`local`、`estimated`、`manual`。
- **手動同步**：右上角「同步」觸發，無背景自動輪詢。
- **F12 DevTools**：同步後 Console 輸出 `[Backend Diagnostics]`。
- **五個 Provider**：Claude Code、Codex、Antigravity、OpenCode（本機估算）、Grok。
- 設定頁「顯示項目」→ `AppSettings.visibleProviders`（`undefined`/`null` = 全顯示；前端用 `== null`）。

## Provider 邊界

| Provider | 來源 | 視窗 |
|----------|------|------|
| Claude Code | `~/.claude/.credentials.json` → Anthropic OAuth | 5h、7d weekly；API 失敗保留舊視窗最多 6h |
| Codex | `~/.codex/auth.json` → ChatGPT wham usage | 5h、weekly |
| Antigravity | Windows 憑證 `gemini:antigravity` → cloudcode-pa | Claude/Gemini 各 5h+weekly（4 bar）；額度滿補 bar；僅 Windows |
| OpenCode | 本機 `opencode.db`（只讀；**local 估算**） | 5h / 7d / 30d cost；上限對照 Go $12/$30/$60；週/月 reset 手動；DB 失敗保留 6h |
| Grok | `~/.grok/auth.json`（或 `$GROK_HOME`）→ billing credits | 單條 **weekly**；不自動 refresh token |

- Antigravity：`loadCodeAssist` → `retrieveUserQuotaSummary`（主）+ `fetchAvailableModels`（5h 後備）；缺 `remainingFraction` = 100% used；`merge_antigravity_windows` 補 bar。OAuth client 建置時 `ANTIGRAVITY_CLIENT_ID`/`SECRET` 注入。
- OpenCode：`$.cost` 為增量，`SUM(cost)` 為窗口總量（**無法依 provider 過濾**）；5h reset = `max(time_created) + 5h`；非 Go 官方 usage API。
- Grok：`GET …/billing?format=credits`，header 必含 `X-XAI-Token-Auth: xai-grok-cli`；`creditUsagePercent` + `currentPeriod.end`；失敗請 `grok login`。

## 發佈與建置

- **package 版本**：**1.3.0**（Grok weekly + OpenCode 本機估算改良）。發佈請 **新 tag 新 release**，勿 `--clobber` 覆蓋舊附件。
- 安裝檔：`Token.Anxiety.Dashboard_<version>_x64-setup.exe`（`productName` = `Token.Anxiety.Dashboard`）。
- `npm run release:assets` 印出 NSIS/MSI 路徑。
- 建置前設 `ANTIGRAVITY_CLIENT_ID`/`ANTIGRAVITY_CLIENT_SECRET`（見 README）。

## 品質要求

- 不可變更新 state；邊界輸入要驗證；provider 失敗不可靜默吞掉。
- 新功能補 Vitest 或 Rust 測試；UI 注意小視窗排版。
- 不顯示「新增多帳號」入口，除非能可靠辨識本機多帳號來源。
- **勿提交**：`mcps/`、`release-assets*`、`.env*`、`*.exe`/`*.msi`、本機 `auth.json`／credentials（見根目錄 `.gitignore`）。

## 常用驗證

```powershell
npm test
npm run build
cd src-tauri
cargo test   # 14 passed（含 Grok）
cargo check
npm run release:assets   # 可選
```

## 文件分工

| 檔案 | 用途 |
|------|------|
| `AGENTS.md` | Agent 通用規範（本檔，與 CLAUDE 對齊） |
| `CLAUDE.md` | 同上 + 實踐紀錄／故障排查 |
| `tasks/todo.md` | 精簡交接：現況、決策、待辦（≤100 行） |
| `README.md` | 使用者建置、下載、資料來源、FAQ |
