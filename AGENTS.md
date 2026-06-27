# Token Anxiety Dashboard 專案指引

## 語言

- 回覆使用繁體中文（臺灣用語）。
- 程式碼命名維持英文；UI 文字目前以繁體中文為主，保留未來 i18n 擴充。

## 架構

- 這是 Tauri 2 桌面 App，不是 Vercel-only 純前端。
- 前端使用 React + TypeScript + Vite。
- 本地狀態由 Rust command 寫入 SQLite，避免只依賴瀏覽器 storage。
- Provider adapter 必須明確標示資料可信度：`official`、`local`、`estimated`、`manual`。
- **手動同步**：右上角「同步」觸發，無背景自動輪詢。
- **F12 DevTools**：同步後 Console 輸出 `[Backend Diagnostics]` 後端日誌。
- 四個 Provider 皆已接上真實本機額度資料：Claude Code（Anthropic OAuth API）、Codex（ChatGPT API）、Antigravity（Windows 憑證管理員 → Google cloudcode-pa API）、OpenCode（本地 SQLite db）。
- 設定頁「顯示項目」可勾選要顯示哪些 provider，存於 `AppSettings.visibleProviders`（`undefined`/`null` = 全部顯示）；前端用 `== null` 判斷（Rust `None` 序列化為 JSON `null`）。

## Provider 邊界

- Claude Code：Anthropic OAuth API。API 失敗時 `merge_account_state` 保留舊視窗最多 6 小時，避免閃爍。
- Codex：ChatGPT `/backend-api/wham/usage` API。
- Antigravity（取代 Gemini CLI）：Windows 憑證管理員 `gemini:antigravity` 讀 token；過期時以建置時注入的 OAuth client refresh（不寫回憑證管理員）。`loadCodeAssist`（`ideType=ANTIGRAVITY`）取 project + 方案名 → `retrieveUserQuotaSummary` 取 Claude/Gemini 的 **5h + weekly** 四條視窗（best-effort）→ `fetchAvailableModels` 作 5h 後備。額度用盡時缺 `remainingFraction` 視為 100% used；`merge_antigravity_windows` 補回消失的 bar。憑證讀取僅 Windows。
- OpenCode：本機 `opencode.db`；`$.cost` 為增量值，`SUM(cost)` 為窗口總用量；5h reset = `max(time_created) + 5h`；weekly/monthly reset 由設定頁手動指定。

## 發佈與建置

- 目前版本 **v1.2.1**；安裝檔檔名與 GitHub 一致：`Token.Anxiety.Dashboard_<version>_x64-setup.exe`（`productName` = `Token.Anxiety.Dashboard`）。
- `npm run release:assets` 印出 NSIS/MSI 路徑；發佈請**新 tag 新 release**，勿 `gh release upload --clobber` 覆蓋舊版。
- 建置前設 `ANTIGRAVITY_CLIENT_ID`/`ANTIGRAVITY_CLIENT_SECRET` 環境變數（見 README）。

## 品質要求

- 優先不可變資料更新，不直接 mutate React state。
- 邊界輸入要驗證，provider 掃描失敗不可靜默吞掉。
- 新增功能需補 Vitest 或 Rust 測試。
- UI 新增互動時需檢查小視窗排版不重疊。
- 不顯示「新增多帳號」入口，除非後續已能可靠辨識本機多帳號資料來源。

## 常用驗證

```powershell
npm test
npm run build
cd src-tauri
cargo test
cargo check
npm run release:assets   # 可選：印出安裝檔路徑
```