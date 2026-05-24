# CONTEXT — 開發紀錄交接（給下一位 AI Agent）

> 精簡交接文件。完整規範看 `CLAUDE.md`／`AGENTS.md`，使用者說明看 `README.md`。

## 專案一句話

Tauri 2 桌面額度儀表板，手動同步顯示 Claude Code / Codex / Antigravity / OpenCode 的本機額度。

## 最近一次任務（Gemini CLI → Antigravity）

**背景**：Gemini CLI 已被 Google Antigravity 取代，故移除 Gemini CLI provider、改成 Antigravity，並新增「顯示項目」設定與未簽章下載說明。

**已完成**：

1. **後端 `src-tauri/src/lib.rs`**
   - 移除所有 `*_gemini_*` 函式與 `Gemini*` structs。
   - 新增 Antigravity：`read_antigravity_credentials`（Win32 `CredReadW` 讀 `gemini:antigravity`）、`refresh_antigravity_token`、`fetch_antigravity_project_id`、`fetch_antigravity_models`、`apply_antigravity_quota`、`refresh_antigravity_account`、`inspect_antigravity`。
   - `AppSettings` 新增 `visible_providers: Option<Vec<String>>`。
   - 6 個 Rust 測試通過（含 `parse_antigravity_creds`、`antigravity_token_expired`、`apply_antigravity_quota`）。
2. **前端**
   - `types.ts`（ProviderId `antigravity`＋`visibleProviders`）、`providers.ts`、`defaultState.ts`（兩條 window：Claude、Gemini）、`services/storage.ts`、`i18n.ts`。
   - `SettingsDialog.tsx` 新增「顯示項目」四個 checkbox；`App.tsx` 以 `visibleProviders` 過濾卡片。
   - `App.css` 加 `.provider-toggle-checkbox`。
3. **文件**：README 新增「下載與安裝（SmartScreen 警示）」與 Antigravity 章節；CLAUDE.md／AGENTS.md 同步。

**關鍵技術點（實測確認，勿再踩雷）**

- Antigravity 憑證在 **Windows 憑證管理員**（非檔案）：target `gemini:antigravity`，blob JSON `{token:{access_token,refresh_token,expiry}}`。
- `expiry` / `resetTime` 都是 **RFC3339**（PowerShell 顯示成 `MM/DD/YYYY` 是 ConvertFrom-Json 的假象）。
- 讀到的 access_token **常已過期** → 依 `expiry` 主動 refresh（內建 client id/secret 在 lib.rs），401 再重試。
- 額度 API：`loadCodeAssist`(ideType=ANTIGRAVITY) → `fetchAvailableModels`；`claude-*`/`gemini-*` 各聚合最低 `remainingFraction` 成兩條 bar。
- 憑證讀取僅 Windows；非 Windows 回 `disconnected`（未來可補 keychain/secret-tool）。

## 決策紀錄

- **未簽章 EXE 警示**：本次**不簽章**（使用者決定），只在 README 寫繞過步驟。未來若簽章：Azure Trusted Signing（~$10/月，帳號制可簽多 App）為首選，於 `tauri.conf.json` `bundle.windows` 設定。
- **建置**：維持本機手動 `npx tauri build`，不設 CI。

## 已驗證（實機）

- Antigravity 後端在實機 EXE **正常運作**（使用者 F12 log 確認）：`creds read` → `loadCodeAssist OK project=hip-case-5tjfd` → `API OK 19 models` → `aggregated Claude/Gemini`。憑證 `expiry` 過期會自動 refresh。

## 已修 Bug

- **設定頁黑屏**：Rust `Option<Vec<String>>` 的 `None` 序列化成 JSON **`null`**（非 `undefined`）。`SettingsDialog` 的 `isProviderVisible` 原本用 `=== undefined` 判斷，遇到 null 仍呼叫 `null.includes()` → React 整棵崩潰 → 黑屏。改用 `== null`（涵蓋 null/undefined）。教訓：**Rust None → JSON null，前端要用 `== null` 或 `!value`，不要只判 `=== undefined`**。jsdom 測試走 fallback（undefined）抓不到，需專門用 null 測（見 `SettingsDialog.test.tsx`）。
- 順手修：`.modal` 加 `max-height` + `overflow-y:auto`，避免設定內容過長時溢出/裁切。

## 建置注意

- **Antigravity OAuth client 不硬編碼**：`ANTIGRAVITY_CLIENT_ID`/`ANTIGRAVITY_CLIENT_SECRET` 改由建置時環境變數經 `option_env!` 注入（GitHub push protection 會擋字面值、且符合「不可硬編碼 secrets」規則）。發佈前務必先 `$env:ANTIGRAVITY_CLIENT_ID=...; $env:ANTIGRAVITY_CLIENT_SECRET=...` 再 `npx tauri build`，否則 release 的 EXE 無法自動 refresh token。值為 Antigravity 公開 native-app client（可從 App 或 `lbjlaq/Antigravity-Manager` 取得）。

## 待辦／未驗證

- 「顯示項目」勾選的即時顯示/隱藏與重啟持久化，建議實機再點一次確認（後端已回傳 `visibleProviders`）。
- 非 Windows 平台的 Antigravity 憑證讀取尚未實作。

## 驗證指令

```powershell
npm test            # 前端 9 passed
npm run build       # tsc + vite OK
cd src-tauri; cargo test   # 6 passed
cargo check
npx tauri build     # 產出 EXE（尚未實機跑過）
```
