# Token Anxiety Dashboard 專案指引

## 語言

- 回覆使用繁體中文（臺灣用語）。
- 程式碼命名維持英文；UI 文字目前以繁體中文為主，保留未來 i18n 擴充。

## 架構

- 這是 Tauri 2 桌面 App，不是 Vercel-only 純前端。
- 前端使用 React + TypeScript + Vite。
- 本地狀態由 Rust command 寫入 SQLite，避免只依賴瀏覽器 storage。
- Provider adapter 必須明確標示資料可信度：`official`、`local`、`estimated`、`manual`。
- 目前 `Codex` 與 `OpenCode` 已接上真實本機額度資料；`Claude Code` 與 `Gemini CLI` 先提供真實登入 / 設定狀態。

## Provider 邊界

- Claude Code：已支援 `claude auth status --json`；若沒有可安全讀取的本機額度視窗，不要偽造 quota。
- Codex：已支援 `codex login status` 與本機 session `rate_limits` 解析。
- Gemini CLI：已支援本機登入與設定檔偵測；若沒有穩定 quota 來源，維持無視窗狀態。
- OpenCode：UI 名稱使用 OpenCode；官方方案名稱為 OpenCode Go。目前從本機 `opencode.db` 彙總真實 cost 視窗。

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

# [Token-Anxiety-Dashboard] recent context, 2026-05-01 5:50am GMT+8

No previous sessions found.
</claude-mem-context>
