# Token Anxiety Dashboard 專案指引

## 語言

- 回覆使用繁體中文（臺灣用語）。
- 程式碼命名維持英文；UI 文字目前以繁體中文為主，保留未來 i18n 擴充。

## 架構

- 這是 Tauri 2 桌面 App，不是 Vercel-only 純前端。
- 前端使用 React + TypeScript + Vite。
- 本地狀態由 Rust command 寫入 SQLite，避免只依賴瀏覽器 storage。
- Provider adapter 必須明確標示資料可信度：`official`、`local`、`estimated`、`manual`。
- 四個 Provider 皆已接上真實本機額度資料：Claude Code（Anthropic OAuth API）、Codex（ChatGPT API）、Antigravity（Windows 憑證管理員 → Google cloudcode-pa API）、OpenCode（本地 SQLite db）。
- 設定頁的「顯示項目」可勾選要顯示哪些 provider，儲存於 `AppSettings.visibleProviders`（`undefined` = 全部顯示）。

## Provider 邊界

- Claude Code：已支援 `claude auth status --json` 與 Anthropic OAuth API。API 失敗時 `merge_account_state` 會保留舊視窗長達 6 小時，避免閃爍。
- Codex：已支援 `codex login status` 與 ChatGPT `/backend-api/wham/usage` API。
- Antigravity（取代已淘汰的 Gemini CLI）：從 Windows 憑證管理員 `gemini:antigravity` 讀取登入 token，access_token 過期時用內建 OAuth client refresh。兩步調用 Google `cloudcode-pa`（`loadCodeAssist` 帶 `ideType=ANTIGRAVITY` 取 project ID → `fetchAvailableModels` 取各模型額度）。`claude-*` 聚合為 Claude 一條、`gemini-*` 聚合為 Gemini 一條，各取最低 `remainingFraction`，於同一張卡片顯示兩條 bar。憑證讀取目前僅 Windows。
- OpenCode：UI 名稱使用 OpenCode；官方方案名稱為 OpenCode Go。從本機 `opencode.db` 彙總真實 cost 視窗。`$.cost` 為增量值，`SUM(cost)` 即為正確總用量。5h rolling reset = `max(time_created) + 5h`；weekly/monthly reset 由用戶在設定頁面手動指定。

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
cargo check
```



<claude-mem-context>
# Memory Context

# [Token-Anxiety-Dashboard] recent context, 2026-05-15 2:29am GMT+8

Legend: 🎯session 🔴bugfix 🟣feature 🔄refactor ✅change 🔵discovery ⚖️decision 🚨security_alert 🔐security_note
Format: ID TIME TYPE TITLE
Fetch details: get_observations([IDs]) | Search: mem-search skill

Stats: 2 obs (962t read) | 19,964t work | 95% savings

### May 15, 2026
1016 2:27a 🔵 Token-Anxiety-Dashboard API Call Patterns — On-Demand, Not Polled
1017 2:28a 🔵 Dashboard API Sync Triggered Only at Startup and Manual Button Click

Access 20k tokens of past work via get_observations([IDs]) or mem-search skill.
</claude-mem-context>
