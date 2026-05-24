# Token Anxiety Dashboard 專案指引

## 語言

- 回覆使用繁體中文（臺灣用語）。
- 程式碼命名維持英文；UI 文字目前以繁體中文為主，保留未來 i18n 擴充。

## 架構

- 這是 Tauri 2 桌面 App，不是 Vercel-only 純前端。
- 前端使用 React + TypeScript + Vite。
- 本地狀態由 Rust command 寫入 SQLite，避免只依賴瀏覽器 storage。
- Provider adapter 必須明確標示資料可信度：`official`、`local`、`estimated`、`manual`。
- **同步改為手動**：右上角「同步」按鈕觸發，無自動定時刷新，避免背景耗額。
- **F12 開啟 DevTools**：便於查看 `[Backend Diagnostics]` 後端除錯日誌群組。
- **顯示項目設定**：設定頁可勾選顯示哪些 provider，存於 `AppSettings.visibleProviders`（`undefined` = 全部顯示）；前端 `App.tsx` 以此過濾卡片。

## Provider 邊界與資料來源

| Provider | 資料來源 | 視窗內容 | 備註 |
|----------|---------|---------|------|
| Claude Code | `~/.claude/.credentials.json` → Anthropic OAuth API | 5h rolling、7d weekly | 失敗時保留舊視窗最多 6 小時 |
| Codex | `~/.codex/auth.json` → ChatGPT `/backend-api/wham/usage` | 5h rolling、weekly | 使用 ChatGPT access_token |
| Antigravity | Windows 憑證管理員 `gemini:antigravity` → Google cloudcode-pa API | Claude、Gemini 各一條共用額度 | 兩步調用（loadCodeAssist `ideType=ANTIGRAVITY` → fetchAvailableModels）；`claude-*`/`gemini-*` 各聚合取最低 remainingFraction；token 過期以內建 client refresh |
| OpenCode | `~/.local/share/opencode/opencode.db` | 5h rolling、7d、30d cost | 5h: `reset` = `max(time_created) + width`；weekly/monthly: **用戶手動設定重置時間** |

- Windows 外部 CLI 呼叫帶 `CREATE_NO_WINDOW`，不彈出終端機視窗。
- Rust 端 `log_debug!` 巨集會寫入記憶體環形緩衝區；前端同步後會呼叫 `get_debug_logs` 並在 F12 console 輸出 `[Backend Diagnostics]`。

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
npx tauri build
```

---

## 實踐紀錄與故障排查（給後續 AI 接手用）

### 一、OpenCode 的 Trailing Window 計算（最容易出錯）

**檔案位置**：`src-tauri/src/lib.rs` → `query_opencode_cost`、`query_trailing_reset_at`、`calculate_next_weekly_reset`、`calculate_next_monthly_reset`

**核心發現**：OpenCode 的 `part` 表中的 `step-finish` 事件，其 `$.cost` 欄位存的是**單筆增量值**，不是累計值。因此直接用 `SUM(cost)` 即可得到窗口期間的總使用量。

**5h Rolling Window**：
- **used%**：`SUM(cost)` in 5h window / $12 limit
- **reset**：`max(time_created) + 5h`（最後使用後 5 小時自動歸零）
- 當 `now > reset_at` 時，窗口內無記錄，used% 自動變為 0%

**Weekly/Monthly Fixed Window**：
- **used%**：`SUM(cost)` in window / limit
- **reset**：由用戶在「設定」頁面手動設定
  - weekly：星期幾 + 時間（如：週一 07:00）
  - monthly：日期 + 時間（如：每月 29 號 00:00）
  - 設定儲存在 `app_state` SQLite table（key = `app_settings`）

**設定 UI**：`src/components/SettingsDialog.tsx` → OpenCode 額度重置設定區段
- 5h Rolling：唯讀顯示「自動計算（最後使用後 5 小時歸零）」
- Weekly：星期幾下拉選單 + 時間選擇器
- Monthly：日期下拉選單（1-31）+ 時間選擇器

**診斷方式**：按 F12 → Console → 找 `[Backend Diagnostics]` 群組 → 看 `opencode:` 開頭的日誌：
- `query cost since XX = Ok(YY)` → 確認 SUM(cost) 結果
- `latest record since XX = Some(YY)` → 確認 `max(time_created)` 的值
- `calculated reset_at = Some(ZZ)` → 確認 reset 時間

**參考程式碼**：
- `query_opencode_cost`：用 `SUM(json_extract(data, '$.cost'))` 計算窗口總成本
- `query_trailing_reset_at`：用 `select max(time_created)` 抓最近使用時間（+ 5h = reset）
- `calculate_next_weekly_reset` / `calculate_next_monthly_reset`：根據用戶設定計算下一次 reset 時間

**官方文件對照**：
- OpenCode Go 官網顯示的用量即為上述正確計算結果
- 官方 Limit：5h = $12、7d = $30、30d = $60

---

### 二、Antigravity 的 Quota API（取代已淘汰的 Gemini CLI）

**檔案位置**：`src-tauri/src/lib.rs` → `read_antigravity_credentials`、`refresh_antigravity_token`、`fetch_antigravity_project_id`、`fetch_antigravity_models`、`apply_antigravity_quota`、`refresh_antigravity_account`

**本機憑證來源**：Antigravity 4.x 把登入 token 寫進 **Windows 憑證管理員**，target `gemini:antigravity`（type GENERIC），blob 為 UTF-8 JSON：`{ "token": { "access_token", "refresh_token", "expiry"(RFC3339) }, "auth_method" }`。用 Win32 FFI `CredReadW`/`CredFree` 讀取（僅 Windows）。

**Token 更新**：解析 `expiry`，過期/接近過期（skew 15 分）/無法解析時，用 Antigravity 內建 OAuth client（`oauth2.googleapis.com/token`，grant_type=refresh_token）refresh，只在記憶體用新 token，**不寫回憑證管理員**。models 回 401 時再強制 refresh 重試一次。client_id/secret **不硬編碼**，改由建置時環境變數 `ANTIGRAVITY_CLIENT_ID`/`ANTIGRAVITY_CLIENT_SECRET` 經 `option_env!` 注入（見 README「發佈建置」）；未設定則略過 refresh。

**API 調用流程**：
1. `POST {base}/v1internal:loadCodeAssist`，body `{"metadata":{"ideType":"ANTIGRAVITY"}}` → 取 `cloudaicompanionProject`
2. `POST {base}/v1internal:fetchAvailableModels`，body `{"project": pid}`（403 則去掉 project 重試）
- base fallback：`daily-cloudcode-pa.sandbox` → `daily-cloudcode-pa` → `cloudcode-pa`
- Header：Bearer token + `User-Agent: vscode/1.X.X (Antigravity/4.2.1)`

**回傳格式**：`models` 物件，每個 model 的 `quotaInfo.remainingFraction`(0~1) 與 `quotaInfo.resetTime`(RFC3339)。

**數據處理（兩條共用額度）**：`claude-*` 聚合為一條 "Claude"、`gemini-*` 聚合為一條 "Gemini"（忽略 gpt/image/chat/tab），各取**最低 remainingFraction**；同一張卡片顯示兩條 bar，固定順序 Claude → Gemini。

**已用%計算**：`used = (1.0 - min_remainingFraction) * 100.0`

**診斷方式**：F12 Console 看 `[DIAG] antigravity: aggregated Claude/Gemini used=.. remaining=.. reset=..`。

---

### 三、Claude Code OAuth API

**檔案位置**：`src-tauri/src/lib.rs` → `fetch_claude_usage`

**API 端點**：`GET https://api.anthropic.com/api/oauth/usage`
- 需要 Bearer token（來自 `~/.claude/.credentials.json` 的 `claudeAiOauth.accessToken`）
- Header：`anthropic-beta: oauth-2025-04-20`

**回傳格式**：
- `five_hour.utilization`：0~100 的已用%（直接使用，不需計算）
- `five_hour.resets_at`：ISO 8601 字串
- `seven_day.utilization`：同上

**Rate limit 處理**：API 經常回傳 HTTP 429。為避免視窗閃爍消失，`merge_account_state` 會在 API 失敗時保留舊視窗長達 **360 分鐘**（6 小時）。

---

### 四、Codex ChatGPT API

**檔案位置**：`src-tauri/src/lib.rs` → `fetch_codex_usage_from_api`

**API 端點**：`GET https://chatgpt.com/backend-api/wham/usage`
- 需要 Bearer token（來自 `~/.codex/auth.json` 的 `tokens.access_token`）

**回傳格式**：`rate_limit.primary_window` / `secondary_window`
- `used_percent`：直接使用
- `limit_window_seconds`：視窗寬度（秒）
- `reset_at`：Unix timestamp（秒）

---

### 五、Debug Log 系統

**Rust 端**：`src-tauri/src/lib.rs` → `log_debug!` 巨集
- 寫入記憶體環形緩衝區 `DEBUG_LOGS`（上限 500 行）
- 同時輸出到 `eprintln!`（release build 中不可見）

**前端讀取**：`src/App.tsx` → `handleSync` → `getDebugLogs()`
- 同步完成後自動在 F12 Console 輸出 `[Backend Diagnostics]` 群組
- **這是排查 provider 資料問題的首要工具**

**添加新日誌**：在 Rust 端任何需要觀察的地方使用：
```rust
log_debug!("[DIAG] provider: key={} value={}", key, value);
```

---

### 六、成功經驗（給後續 AI 接手時避免重蹈覆轍）

1. **OpenCode `$.cost` 是增量值**：`part` 表的 `step-finish` 事件，`$.cost` 存的是**單次 step 的成本**，不是累計值。正確做法：`SUM(cost)` 即為窗口總用量。曾經誤以為是累計值而寫了 delta 邏輯，導致 used% 偏低。
2. **OpenCode 5h reset = `max(time_created) + 5h`**：曾誤用 `min(time_created) + 5h`，導致 reset 時間固定為窗口最舊記錄 + 5h，永遠不歸零。正確應該用**最新使用時間** + 5h。
3. **OpenCode weekly/monthly 必須手動設定**：從本地 DB 無法推斷用戶的實際重置時間（數據不足）。UI 提供 day-of-week + time picker（weekly）和 day-of-month + time picker（monthly），儲存在 `app_settings`。
4. **Antigravity 憑證在 Windows 憑證管理員**：不是檔案。target `gemini:antigravity`，需 `CredReadW` 讀取。憑證裡的 `expiry`/API 的 `resetTime` 皆為 RFC3339（注意：PowerShell `ConvertFrom-Json` 會把它顯示成 `MM/DD/YYYY`，那是顯示假象，原始值是 RFC3339）。
5. **Antigravity access_token 常已過期**：App 不在前景時 token 不會被刷新，讀到多半過期，故務必依 `expiry` 主動 refresh（並保留 401 重試）。refresh_token 不變、不要寫回憑證管理員。
6. **Antigravity 兩條共用額度**：`claude-*` 共用一池、`gemini-*` 共用另一池，各取最低 `remainingFraction` 聚合成一條，不要逐模型顯示。
7. **Claude API 429 處理**：Anthropic OAuth API 經常 429。`merge_account_state` 的 360 分鐘快取是必要設計，避免視窗閃爍消失。

### 七、常見問題速查

| 問題現象 | 可能原因 | 排查步驟 |
|---------|---------|---------|
| OpenCode 5h reset 時間過長（>4h） | `query_trailing_reset_at` 用了 `min` 而非 `max` | 檢查 F12 日誌中 `oldest record` vs `newest record`，reset 應為 newest + 5h |
| OpenCode used% 遠低於官方 | 把 `$.cost` 當成累計值，做了不必要的 delta 計算 | 檢查 `query_opencode_cost` 是否直接用 `SUM(cost)` |
| OpenCode weekly/monthly reset 不對 | 使用了硬編碼時間而非用戶設定 | 檢查 `get_settings` 是否讀到正確的 `opencode_weekly/monthly_reset` |
| Antigravity 顯示未連線 | Windows 憑證管理員無 `gemini:antigravity` | `cmdkey /list` 確認；請先在 Antigravity 登入 |
| Antigravity Claude/Gemini 皆 0% 或 API 失敗 | access_token 過期且 refresh 失敗 | 看 F12 `antigravity:` 日誌的 refresh 結果；必要時重新登入 Antigravity |
| Claude 視窗消失後又出現 | `merge_account_state` 360 分鐘快取生效 | 這是 feature，不是 bug |
| 同步後 Console 無日誌 | `get_debug_logs` 呼叫失敗或無日誌產生 | 檢查 Rust 端是否有 `log_debug!` 呼叫 |

---

### 八、外部參考連結

- **OpenCode 官方（Crush 前身）**：https://github.com/opencode-ai/opencode（已 archive，轉移至 charmbracelet/crush）
- **Antigravity-Manager（憑證/quota API 參考）**：https://github.com/lbjlaq/Antigravity-Manager
- **CC-Switch 作者**：GitHub @jonz94（原始碼未公開，功能類似本專案）
- **Tauri 文件**：https://tauri.app/
- **Anthropic OAuth API 文件**：內部 beta，無公開文件（參考 `oauth-2025-04-20` header）
