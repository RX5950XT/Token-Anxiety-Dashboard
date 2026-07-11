# CONTEXT — 開發紀錄交接（給下一位 AI Agent）

> 精簡交接。完整規範：`CLAUDE.md`／`AGENTS.md`；使用者說明：`README.md`。

## 專案一句話

Tauri 2 桌面額度儀表板（**v1.3.0**：Grok weekly + OpenCode 本機估算語意）。手動同步顯示 Claude Code / Codex / Antigravity / OpenCode / Grok。

## 目前功能狀態

| Provider | 視窗 | 資料來源 |
|----------|------|----------|
| Claude Code | 5h、7d weekly | Anthropic OAuth API |
| Codex | 5h、weekly | ChatGPT wham API |
| Antigravity | Claude/Gemini 各 5h + weekly（4 bar） | `retrieveUserQuotaSummary` + models 後備 |
| OpenCode | 5h、7d、30d cost | 本機 `opencode.db`（**local 估算**，非 Go 官方 API） |
| Grok | weekly credit（1 bar） | `~/.grok/auth.json` → `cli-chat-proxy.grok.com/v1/billing?format=credits` |

- 手動同步、F12 `[Backend Diagnostics]`、`visibleProviders`（`null` = 全顯示）。
- Antigravity 憑證僅 Windows；OAuth client 建置時環境變數注入。

## 最近一次任務

### v1.3.0 發佈（本 session）

- **Grok** weekly credit（官方 billing API）
- **OpenCode**：本機估算語意、只讀 DB、失敗保留 6h、設定頁說明
- tag：`v1.3.0`；安裝檔建置後上傳 Release，本機 bundle **不進 git**

### 倉庫清理

- 已 ignore：`mcps/`、`release-assets*`、`.env*`、安裝檔、auth／credentials。

## Antigravity 技術要點（已發佈 v1.2.1）

- Window ID：`antigravity-claude-5h`、`…-claude-weekly`、`…-gemini-5h`、`…-gemini-weekly`。
- 額度滿：缺 `remainingFraction` → 100% used；`merge_antigravity_windows` 補 bar。

## 已修 Bug（摘要）

- 設定頁黑屏：`visibleProviders` JSON `null` → 前端改 `== null`。
- Antigravity bar 用盡消失：缺欄位當 exhausted + merge 補回。

## 決策紀錄

- 安裝檔不簽章（README 有 SmartScreen 步驟）。
- 本機 `npx tauri build`，不設 CI；Release 一版一 tag，不覆蓋舊附件。
- 本機安裝產物與 MCP schema 不快取進 git。

## 待辦／未驗證

- 發佈 **v1.2.2**（或下一版）：含 Grok；更新 GitHub Release 說明與 README 下載表。
- 「顯示項目」重啟持久化實機再確認。
- 非 Windows Antigravity 憑證未實作。
- Antigravity 四視窗與官方 App 實機對照。
- Grok token 過期後是否需自動 refresh（目前靠 CLI 登入更新 `auth.json`）。

## 驗證指令

```powershell
npm test            # 11 passed
npm run build
cd src-tauri; cargo test   # 16 passed
cargo check
npx tauri build     # 需 ANTIGRAVITY_CLIENT_ID/SECRET
npm run release:assets
```
