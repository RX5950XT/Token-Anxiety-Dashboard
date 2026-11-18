# Token-Anxiety-Dashboard todo
最後更新：2026-11-19

## 現況（交接）
- Tauri 2 桌面額度儀表板，最新發佈 **v1.3.0**（tag `v1.3.0`）；功能、建置、下載見 `README.md`，規範與故障排查見 `CLAUDE.md`／`AGENTS.md`。
- Provider 與資料來源：

| Provider | 視窗 | 資料來源 |
|----------|------|----------|
| Claude Code | 5h、7d weekly | Anthropic OAuth API |
| Codex | 5h、weekly | ChatGPT wham API |
| Antigravity | Claude/Gemini 各 5h + weekly（4 bar） | `retrieveUserQuotaSummary` + models 後備 |
| OpenCode | 5h、7d、30d cost | 本機 `opencode.db`（**本機估算**，非 Go 官方 API；只讀 DB，失敗保留 6h） |
| Grok | weekly credit（1 bar） | `~/.grok/auth.json` → `cli-chat-proxy.grok.com/v1/billing?format=credits` |

- 只有按「同步」才刷新；F12 Console 看 `[Backend Diagnostics]`；`visibleProviders` 為 `null` 代表全顯示。
- Antigravity 憑證只支援 Windows；OAuth client 在建置時用環境變數注入。

## 技術要點與踩雷
- Antigravity Window ID：`antigravity-claude-5h`、`…-claude-weekly`、`…-gemini-5h`、`…-gemini-weekly`。
- 額度用滿時 API 會缺 `remainingFraction` → 當成 100% used，`merge_antigravity_windows` 補回 bar（否則 bar 會消失）。
- 設定頁黑屏：`visibleProviders` JSON 是 `null`，前端要用 `== null` 判斷。

## 決策
- 安裝檔不簽章（README 有 SmartScreen 步驟）。
- 本機 `npx tauri build`，不設 CI；Release 一版一個新 tag，不 `--clobber` 覆蓋舊附件。
- 安裝產物、`mcps/`、`release-assets*`、`.env*`、auth／credentials 都不進 git。

## 待辦／未驗證
- [ ] 「顯示項目」重啟後是否保留，實機再確認。
- [ ] Antigravity 四視窗與官方 App 實機對照。
- [ ] Grok token 過期是否要自動 refresh（目前靠 CLI 登入更新 `auth.json`）。

## 已知問題／刻意不處理
- 非 Windows 的 Antigravity 憑證未實作。

## 驗證指令
```powershell
npm test; npm run build
cd src-tauri; cargo test; cargo check
npx tauri build          # 需 ANTIGRAVITY_CLIENT_ID/SECRET
npm run release:assets
```
