# CONTEXT — 開發紀錄交接（給下一位 AI Agent）

> 精簡交接文件。完整規範看 `CLAUDE.md`／`AGENTS.md`，使用者說明看 `README.md`。

## 專案一句話

Tauri 2 桌面額度儀表板（**v1.2.1**），手動同步顯示 Claude Code / Codex / Antigravity / OpenCode 的本機額度。

## 目前功能狀態

| Provider | 視窗 | 資料來源 |
|----------|------|----------|
| Claude Code | 5h、7d weekly | Anthropic OAuth API |
| Codex | 5h、weekly | ChatGPT wham API |
| Antigravity | Claude/Gemini 各 5h + weekly（**4 條 bar**） | `retrieveUserQuotaSummary` + `fetchAvailableModels` 後備 |
| OpenCode | 5h、7d、30d cost | 本機 `opencode.db` |

- 手動同步、F12 `[Backend Diagnostics]`、設定頁「顯示項目」勾選（`visibleProviders`，`null` = 全顯示）。
- Antigravity 憑證僅 Windows（`gemini:antigravity`）；OAuth client 建置時環境變數注入。

## 最近一次任務（文件同步 + 安裝檔檔名對齊）

- `productName` = `Token.Anxiety.Dashboard` → 建置產物與 GitHub 同名，無需手動 rename。
- `npm run release:assets` 印出路徑；發佈用**新 tag**，禁止 `--clobber` 覆蓋舊 release 附件。
- GitHub：**v1.2.1** = 四視窗版；**v1.2.0** = 初版 Antigravity（兩條 bar）。

## Antigravity 技術要點（v1.2.1）

- 參考 [Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) `retrieveUserQuotaSummary`。
- Window ID：`antigravity-claude-5h`、`antigravity-claude-weekly`、`antigravity-gemini-5h`、`antigravity-gemini-weekly`。
- 額度滿：缺 `remainingFraction` → 100% used；`merge_antigravity_windows` 補缺失 bar。
- `loadCodeAssist` 解析 `paidTier`/`currentTier` → 方案名（如 Antigravity Pro）。

## 已修 Bug（摘要）

- **設定頁黑屏**：`visibleProviders` 為 JSON `null` 時，`=== undefined` 會崩潰 → 改用 `== null`。
- **Antigravity bar 用盡消失**：`apply_antigravity_quota` 跳過缺欄位模型 → 改視為 exhausted + merge 補回。

## 決策紀錄

- 安裝檔**不簽章**（README 有 SmartScreen 繞過步驟）。
- 本機手動 `npx tauri build`，不設 CI。
- Release 一版一 tag，不覆蓋舊附件。

## 待辦／未驗證

- 「顯示項目」重啟持久化建議實機再確認。
- 非 Windows Antigravity 憑證讀取未實作。
- Antigravity 四視窗數值與官方 App 實機對照（summary API）。

## 驗證指令

```powershell
npm test            # 前端 11 passed
npm run build
cd src-tauri; cargo test   # Rust 10 passed
cargo check
npx tauri build     # 需 ANTIGRAVITY_CLIENT_ID/SECRET
npm run release:assets
```