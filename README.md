# Token Anxiety Dashboard

Token Anxiety Dashboard 是一個 Tauri 2 桌面版額度儀表板，集中追蹤 **Claude Code**、**Codex**、**Antigravity**、**OpenCode** 與 **Grok** 的訂閱額度、刷新時間與本機連線狀態。

## 功能狀態

- 左上角 Logo、右上角「同步」與「設定」按鈕。
- 中央 provider 面板：方案、可信度、額度進度條與刷新倒數。
- 面板支援拖曳排序（存入本機 SQLite）。
- 設定：繁中 / English；極光／深色／石墨／淺色主題。
- 設定可勾選「顯示項目」（Claude Code / Codex / Antigravity / OpenCode / Grok）。
- **Antigravity**：Claude + Gemini 各 **5h + 每週** 共 4 條進度條。
- **Grok**：單條 **每週 credit**（讀 `~/.grok/auth.json`，呼叫 Grok CLI billing API）。
- **手動同步**：僅按「同步」才刷新，無背景輪詢。
- **F12 DevTools**：同步後 Console 有 `[Backend Diagnostics]`。

## 技術架構

- Desktop：Tauri 2
- Frontend：React 19、TypeScript、Vite
- DnD：@dnd-kit
- Icons：lucide-react
- 狀態：Rust command + SQLite
- Tests：Vitest、Testing Library
- HTTP：ureq（Rust）

## 建置說明

### 前置需求

- **Node.js** 22+（見 `package.json` `engines`）
- **Rust**（[rustup](https://rustup.rs/)）
- **Windows**：[Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)（Desktop development with C++）

### 第一次建置

```powershell
npm install
npm run tauri dev

npm test
cd src-tauri; cargo test
```

### 發佈建置（產出安裝檔）

Antigravity token refresh 需要建置時注入 Google OAuth client（公開 native-app client，可從 Antigravity 或 [Antigravity-Manager](https://github.com/lbjlaq/Antigravity-Manager) 取得）。未設定時仍可用未過期的 access_token，但無法自動 refresh。

```powershell
$env:ANTIGRAVITY_CLIENT_ID = "<antigravity_oauth_client_id>"
$env:ANTIGRAVITY_CLIENT_SECRET = "<antigravity_oauth_client_secret>"

npx tauri build
```

產物（`productName` = `Token.Anxiety.Dashboard`，版本取自 `package.json`）：

- NSIS：`src-tauri/target/release/bundle/nsis/Token.Anxiety.Dashboard_<version>_x64-setup.exe`
- MSI：`src-tauri/target/release/bundle/msi/Token.Anxiety.Dashboard_<version>_x64_en-US.msi`

例（v1.2.1）：

- `Token.Anxiety.Dashboard_1.2.1_x64-setup.exe`
- `Token.Anxiety.Dashboard_1.2.1_x64_en-US.msi`

上傳前：`npm run release:assets` 印出路徑。發佈請**新 tag／新 release**，勿 `--clobber` 覆蓋舊附件。

視窗標題為「Token Anxiety Dashboard」（與安裝檔檔名分開）。`target/` 不進 git，需自行 `npx tauri build`。

**Grok** 不需額外建置環境變數；本機需已執行過 `grok login`（或等價登入）以產生 `~/.grok/auth.json`。

## 下載（GitHub Releases）

預編譯安裝檔見 [Releases](https://github.com/RX5950XT/Token-Anxiety-Dashboard/releases)：

| 版本 | 說明 | 建議下載 |
|------|------|----------|
| **v1.2.1**（目前發佈最新） | Antigravity 四視窗（5h + 每週）、額度滿 bar 不消失 | `Token.Anxiety.Dashboard_1.2.1_x64-setup.exe` |
| v1.2.0 | 初版 Antigravity（Claude/Gemini 各一條 bar） | `Token.Anxiety.Dashboard_1.2.0_x64-setup.exe` |

> **Grok 每週額度**已合入原始碼（main），尚未以獨立 tag 發佈。需要 Grok 卡片請自行從 main 建置，或等下一版 release（建議 v1.2.2）。

## 下載與安裝（瀏覽器／SmartScreen 警示）

安裝檔**未做程式碼簽章**，瀏覽器與 Windows 可能顯示安全警示；檔案本身安全，僅因發行者未受信任簽章。

1. **瀏覽器**：下載列 **⋯ → 保留**。
2. **SmartScreen**：**更多資訊 → 仍要執行**。

付費簽章選項（Azure Trusted Signing / OV / EV）見先前說明；自簽無法消除 SmartScreen。

## 資料來源策略

可信度標示：

- `official`：官方 usage / billing API
- `local`：本機 CLI、設定檔或資料庫
- `estimated`：推估或 API 失敗時沿用快取
- `manual`：使用者手動建立或校準

### 已接上的真實資料來源

| Provider | 來源 | 視窗 | 計算方式 |
|----------|------|------|----------|
| Claude Code | `~/.claude/.credentials.json` → Anthropic OAuth | 5h、7d weekly | `utilization` 已用% |
| Codex | `~/.codex/auth.json` → ChatGPT wham usage | 5h、weekly | `used_percent` |
| Antigravity | Windows 憑證 `gemini:antigravity` → cloudcode-pa（summary + models 後備） | Claude/Gemini 各 5h+weekly | `used = (1 - remainingFraction) * 100`；額度滿 bar 持久 |
| OpenCode | `~/.local/share/opencode/opencode.db` | 5h、7d、30d cost | `SUM(cost)/limit`；5h reset = `max(time_created)+5h`；週／月 reset 設定頁 |
| Grok | `~/.grok/auth.json`（或 `$GROK_HOME`）→ `cli-chat-proxy.grok.com/v1/billing?format=credits` | weekly credit | `creditUsagePercent` 已用%；reset = `currentPeriod.end` |

### 輔助偵測路徑

- OpenCode：`~/.local/share/opencode/auth.json`、`~/.config/opencode/opencode.json`
- Grok：`~/.grok/auth.json`（需先 `grok login`）

## Windows 部署注意

- 外部 CLI 呼叫帶 `CREATE_NO_WINDOW`，不彈終端機。
- F12 需 Tauri feature `devtools`。

## 故障排查（Debug Log）

同步後 **F12 → Console → `[Backend Diagnostics]`**：

| 前綴 | 內容 |
|------|------|
| `claude:` | OAuth 視窗與 reset |
| `codex:` | `used_percent`、`reset_at` |
| `antigravity:` | 憑證、refresh、`quotaSummary`、tier、5h/weekly used |
| `opencode:` | `SUM(cost)`、`max(time_created)` |
| `grok:` | token 長度、`weekly used`、billing 失敗 |

與官方不符時，優先比對日誌中的**原始 API 數值**。

### OpenCode 常見問題

- **Used% 過低**：`$.cost` 是增量，應用 `SUM(cost)`，勿當累計做 delta。
- **5h reset 過長**：reset = **最新**使用時間 + 5h（`max` 不是 `min`）。
- **Weekly/Monthly 不對**：須在設定頁手動指定重置時間。

### Antigravity 常見問題

- **未連線**：憑證管理員無 `gemini:antigravity` → 先在 Antigravity 登入（`cmdkey /list`）。
- **全 0% 或 API 失敗**：token 過期且 refresh 失敗 → 看 F12；建置需 `ANTIGRAVITY_CLIENT_*`。
- **只有 2 條 bar**：舊版 v1.2.0 或 summary 失敗 → 用 v1.2.1+，確認 `quotaSummary OK`。
- **額度用盡 bar 消失**：v1.2.1 已修。
- **% 與官方 App 不同**：本儀表板顯示**已用%**，App 可能顯示剩餘%。

### Grok 常見問題

- **未連線／找不到憑證**：執行 `grok login`，確認 `~/.grok/auth.json`（或 `$GROK_HOME`）存在且 entry 有 `key`。
- **API 401/403**：session 失效 → 重新 `grok login`。請求須帶 `X-XAI-Token-Auth: xai-grok-cli`。
- **沒有 5h 條**：Grok 訂閱目前為**每週 credit**，儀表板只顯示 weekly（與 Orca 一致）。
- **發佈版沒有 Grok 卡片**：v1.2.1 安裝檔尚無 Grok；請從 main 建置或等下一版 release。

## 開發與提交注意

請勿把下列內容提交進 git（已列於 `.gitignore`）：

- `mcps/`（本機 MCP tool schema 快取）
- `release-assets/`、`release-assets-*`、`*.exe`、`*.msi`（安裝產物）
- `.env`、`auth.json`、credentials、本機 SQLite 傾印
