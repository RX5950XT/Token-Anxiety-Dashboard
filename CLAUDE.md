# Token Anxiety Dashboard 專案指引

## 語言

- 回覆使用繁體中文（臺灣用語）。
- 程式碼命名維持英文；UI 文字目前以繁體中文為主，保留未來 i18n 擴充。

## 架構

- 這是 Tauri 2 桌面 App，不是 Vercel-only 純前端。
- 前端使用 React + TypeScript + Vite。
- 本地狀態由 Rust command 寫入 SQLite，避免只依賴瀏覽器 storage。
- Provider adapter 必須明確標示資料可信度：`official`、`local`、`estimated`、`manual`。
- **手動同步**：右上角「同步」按鈕觸發，無自動定時刷新，避免背景耗額。
- **F12 DevTools**：同步後 Console 輸出 `[Backend Diagnostics]` 後端日誌。
- **顯示項目**：`AppSettings.visibleProviders`（`undefined`/`null` = 全部顯示）；前端用 `== null` 判斷。
- **五個 Provider**：Claude Code、Codex、Antigravity、OpenCode、Grok。

## Provider 邊界與資料來源

| Provider | 資料來源 | 視窗內容 | 備註 |
|----------|---------|---------|------|
| Claude Code | `~/.claude/.credentials.json` → Anthropic OAuth API | 5h rolling、7d weekly | 失敗時保留舊視窗最多 6 小時 |
| Codex | `~/.codex/auth.json` → ChatGPT `/backend-api/wham/usage` | 5h rolling、weekly | ChatGPT access_token |
| Antigravity | Windows 憑證管理員 `gemini:antigravity` → Google cloudcode-pa | Claude/Gemini 各 **5h + weekly**（4 bar） | summary 主路徑 + models 5h 後備；額度滿 bar 不消失；僅 Windows |
| OpenCode | `~/.local/share/opencode/opencode.db` | 5h、7d、30d cost | 5h reset = `max(time_created)+width`；weekly/monthly 手動設定 |
| Grok | `~/.grok/auth.json`（或 `$GROK_HOME`）→ billing credits | **weekly** 一條 bar | `creditUsagePercent`；header 需 `X-XAI-Token-Auth: xai-grok-cli`；不自動 refresh |

- Windows 外部 CLI 呼叫帶 `CREATE_NO_WINDOW`。
- Rust `log_debug!` → 記憶體環形緩衝；前端同步後 `get_debug_logs` 輸出 F12。

## 品質要求

- 優先不可變資料更新；邊界輸入要驗證；provider 掃描失敗不可靜默吞掉。
- 新增功能需補 Vitest 或 Rust 測試；UI 注意小視窗排版。
- 不顯示「新增多帳號」入口，除非能可靠辨識本機多帳號資料來源。
- **勿提交**：`mcps/`、`release-assets*`、`.env*`、安裝檔 `*.exe`/`*.msi`、本機 auth／credentials（見 `.gitignore`）。

## 常用驗證

```powershell
npm test
npm run build
cd src-tauri
cargo test   # 14 passed（含 Grok）
cargo check
npx tauri build
npm run release:assets
```

## 發佈與安裝檔檔名

- `tauri.conf.json`：`productName` = `Token.Anxiety.Dashboard`，`app.windows.title` = `Token Anxiety Dashboard`。
- 產物：`Token.Anxiety.Dashboard_<version>_x64-setup.exe`、`…_x64_en-US.msi`。
- GitHub：**新 tag 新 release**；勿 `gh release upload --clobber`。
- **已發佈最新 tag：v1.2.1**（Antigravity 四視窗）。**Grok 已在 main 程式碼**，發佈時建議 **v1.2.2**（或下一版號）。v1.2.0 = 初版 Antigravity（兩條 bar）。

---

## 實踐紀錄與故障排查（給後續 AI 接手用）

### 一、OpenCode 的 Trailing Window 計算（最容易出錯）

**檔案**：`src-tauri/src/lib.rs` → `query_opencode_cost`、`query_trailing_reset_at`、`calculate_next_weekly_reset`、`calculate_next_monthly_reset`

**核心**：`part` 表 `step-finish` 的 `$.cost` 是**單筆增量**，`SUM(cost)` 即窗口總用量。

**5h Rolling**：used = `SUM(cost)` / $12；reset = `max(time_created) + 5h`；`now > reset_at` 時 used → 0%。

**Weekly/Monthly**：used = `SUM(cost)` / limit；reset 由設定頁指定，存 `app_settings`。

**診斷**：F12 `[Backend Diagnostics]` → `opencode:` 的 `query cost`、`latest record`、`calculated reset_at`。

**官方 limit**：5h = $12、7d = $30、30d = $60。

---

### 二、Antigravity 的 Quota API

**檔案**：`read_antigravity_credentials`、`refresh_antigravity_token`、`fetch_antigravity_*`、`apply_antigravity_quota`、`merge_antigravity_windows`、`refresh_antigravity_account`

**參考**：[lbjlaq/Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) `retrieveUserQuotaSummary`。

**憑證**：Windows 憑證管理員 target `gemini:antigravity`（GENERIC），blob UTF-8 JSON。`CredReadW` 僅 Windows。

**Token**：依 `expiry` 主動 refresh（skew 15 分）；不寫回憑證；401 強制 refresh。client 由 `ANTIGRAVITY_CLIENT_ID`/`SECRET` + `option_env!` 注入。

**流程**：`loadCodeAssist` → `retrieveUserQuotaSummary` → `fetchAvailableModels`（5h 後備）。四條 bar：Claude 5h/weekly、Gemini 5h/weekly。`used = (1 - remainingFraction) * 100`。缺欄位 = 100% used + merge 補 bar。

**診斷**：F12 `quotaSummary OK`、`antigravity: Claude/Gemini 5h/weekly used=..`。

---

### 三、Claude Code OAuth API

**端點**：`GET https://api.anthropic.com/api/oauth/usage`  
Header：`anthropic-beta: oauth-2025-04-20`；Bearer 來自 `claudeAiOauth.accessToken`。  
`five_hour` / `seven_day` 的 `utilization` 為已用%。常 429 → `merge_account_state` 保留舊視窗 360 分鐘。

---

### 四、Codex ChatGPT API

**端點**：`GET https://chatgpt.com/backend-api/wham/usage`  
Bearer 來自 `~/.codex/auth.json` 的 `tokens.access_token`。  
`primary_window` / `secondary_window`：`used_percent`、`reset_at`（unix 秒）。

---

### 五、Grok CLI 每週額度

**檔案**：`parse_grok_auth_json`、`fetch_grok_billing`、`apply_grok_billing`、`refresh_grok_account`

**參考**：[stablyai/orca](https://github.com/stablyai/orca) `grok-fetcher.ts`、`grok-auth.ts`。

**憑證**：`$GROK_HOME/auth.json` 或 `~/.grok/auth.json`（map entry：`key`、`user_id`、`email`、`expires_at`…）。

**API**：`GET https://cli-chat-proxy.grok.com/v1/billing?format=credits`  
Headers：`Authorization: Bearer {key}`、`X-XAI-Token-Auth: xai-grok-cli`、`Accept: application/json`、可選 `x-userid`。  
回傳：`config.creditUsagePercent`、`config.currentPeriod.end`、可選 `subscriptionTier`。週期型別多為 `USAGE_PERIOD_TYPE_WEEKLY`。

**視窗**：`grok-weekly`（kind=`weekly`，limit=100）。**不**自動 refresh token。

**診斷**：`grok: weekly used=..` 或 `grok: billing API FAILED`。

---

### 六、Debug Log 系統

Rust：`log_debug!` → `DEBUG_LOGS`（上限 500 行）+ `eprintln!`。  
前端：`handleSync` → `getDebugLogs()` → F12 `[Backend Diagnostics]`。

```rust
log_debug!("[DIAG] provider: key={} value={}", key, value);
```

---

### 七、成功經驗

1. OpenCode `$.cost` 是增量 → 直接 `SUM`，勿做 delta。
2. OpenCode 5h reset 用 `max(time_created)+5h`，不是 `min`。
3. OpenCode weekly/monthly reset 必須使用者設定。
4. Antigravity 憑證在 Windows 憑證管理員，不是檔案。
5. Antigravity access_token 常過期 → 務必依 expiry refresh。
6. Antigravity 四視窗：summary 主、models 後備；不要逐模型 bar。
7. Antigravity 額度滿常省略欄位 → 100% used + merge。
8. Claude 429 → 360 分鐘快取避免閃爍。
9. Release 勿 `--clobber`；新版新 tag。
10. Grok 只要 weekly；headers 必含 `X-XAI-Token-Auth: xai-grok-cli`。
11. 本機 `mcps/`、`release-assets*` 不進 git。

### 八、常見問題速查

| 問題現象 | 可能原因 | 排查 |
|---------|---------|------|
| OpenCode 5h reset 過長 | 用了 `min` 非 `max` | F12 newest + 5h |
| OpenCode used% 過低 | 對 cost 做了 delta | 確認 `SUM(cost)` |
| OpenCode weekly/monthly 不對 | 未讀用戶設定 | `opencode_*_reset` |
| Antigravity 未連線 | 無 `gemini:antigravity` | `cmdkey /list`、先登入 App |
| Antigravity 全 0% / API 失敗 | token 過期且 refresh 失敗 | F12 refresh；重登入 |
| Antigravity bar 用盡消失 | 未 merge | exhausted + merge |
| Antigravity 只有 2 條 bar | 舊版或 summary 失敗 | ≥v1.2.1；`quotaSummary OK` |
| Claude 視窗短暫消失又出現 | 360 分快取 | feature |
| Grok 未連線 | 無 auth 或無 `key` | `grok login`；`$GROK_HOME` |
| Grok 401/403 | token 失效 | 重登入；檢查 header |

### 九、外部參考

- OpenCode（archive）→ charmbracelet/crush
- [Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager)
- [Orca Grok rate-limits](https://github.com/stablyai/orca)
- [Tauri](https://tauri.app/)
- Anthropic OAuth：內部 beta（`oauth-2025-04-20`）
