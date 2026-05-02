# Token Anxiety Dashboard

Token Anxiety Dashboard 是一個 Tauri 2 桌面版額度儀表板，集中追蹤 Claude Code、Codex、Gemini CLI 與 OpenCode 的訂閱額度、刷新時間與本機連線狀態。

## 功能狀態

- 左上角 Logo、右上角「同步」與「設定」按鈕。
- 中央 provider 面板，每張面板顯示 provider、方案、可信度、額度進度條與刷新倒數。
- 面板支援拖曳排序，排序會存回本地 SQLite。
- 設定面板支援繁體中文 / English、淺色 / 深色 / 極光玻璃 / 石墨夜色主題。
- **手動同步**：按「同步」按鈕才會刷新本機資料，無背景自動輪詢。
- **F12 開啟 DevTools**：同步後會在 Console 輸出 `[Backend Diagnostics]` 後端除錯日誌。

## 技術架構

- Desktop shell：Tauri 2
- Frontend：React 19、TypeScript、Vite
- Drag and drop：@dnd-kit
- Icons：lucide-react
- Local storage：Rust command + SQLite
- Tests：Vitest、Testing Library
- HTTP client：ureq (Rust)

## 建置說明

### 前置需求

- **Node.js** 22+（本專案使用 `package.json` 中的 `engines` 欄位鎖定）
- **Rust**（Tauri 2 需要 `rustc` + `cargo`，建議透過 [rustup](https://rustup.rs/) 安裝）
- **Windows**：需安裝 [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)（Workload: "Desktop development with C++"）

### 第一次建置

```powershell
# 1. 安裝前端依賴
npm install

# 2. 開發模式（自動熱重載）
npm run tauri dev

# 3. 執行測試
npm test              # 前端 Vitest
cd src-tauri && cargo test   # Rust 端測試
```

### 發佈建置（產出安裝檔）

```powershell
npx tauri build
```

建置完成後，**無論是誰建置，路徑結構都相同**（Tauri 預設行為）：

- **MSI 安裝程式**：`src-tauri/target/release/bundle/msi/Token Anxiety Dashboard_0.1.1_x64_en-US.msi`
- **NSIS 安裝程式**：`src-tauri/target/release/bundle/nsis/Token Anxiety Dashboard_0.1.1_x64-setup.exe`

直接雙擊 `.msi` 或 `-setup.exe` 即可安裝到 Windows 系統。

> **注意**：`target/` 目錄在 `.gitignore` 中，不會提交到 git。每個接手者都必須自行執行 `npx tauri build` 產出安裝檔。

## 資料來源策略

每張面板以可信度標示資料來源：

- `official`：官方 usage API。
- `local`：本機 CLI、設定檔或資料庫。
- `estimated`：依官方規則與本機校準推估。
- `manual`：使用者手動建立或校準。

### 已接上的真實資料來源

| Provider    | 來源                                                          | 視窗                        | 計算方式                                                                                                     |
| ----------- | ------------------------------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Claude Code | `~/.claude/.credentials.json` → Anthropic OAuth API        | 5h rolling、7d weekly       | `utilization` 為已用%直接使用                                                                              |
| Codex       | `~/.codex/auth.json` → ChatGPT `/backend-api/wham/usage` | 5h rolling、weekly          | `used_percent` 直接使用                                                                                    |
| Gemini CLI  | `~/.gemini/oauth_creds.json` → Google cloudcode-pa API     | Pro、Flash、Flash Lite 每日 | `used = (1 - min_remainingFraction) * 100`，按類別（Pro/Flash/Flash Lite）聚合取最低 remainingFraction     |
| OpenCode    | `~/.local/share/opencode/opencode.db`                       | 5h rolling、7d、30d cost    | `used = SUM(cost) / limit`；5h reset = `max(time_created) + 5h`；weekly/monthly reset 由用戶在設定中指定 |

### 輔助偵測路徑

- OpenCode：`~/.local/share/opencode/auth.json`、`~/.config/opencode/opencode.json`

## Windows 部署注意

- 外部 CLI 呼叫（Rust `std::process::Command`）會帶 `CREATE_NO_WINDOW`，避免彈出終端機視窗。
- F12 可開啟 Chromium DevTools（需 `tauri` feature `devtools`）。

## 故障排查（Debug Log）

同步後按 **F12** 打開 Console，找到 `[Backend Diagnostics]` 群組：

- `claude:` → Claude OAuth API 回傳的視窗與 reset 時間
- `codex:` → ChatGPT API 回傳的 `used_percent` 與 `reset_at`
- `gemini:` → 聚合後各類別的 `used`、`remainingFraction`、`resetTime`（Pro/Flash/Flash Lite）
- `opencode:` → `SUM(cost)` 結果、`max(time_created)`（reset 計算來源）

如果某個 provider 的顯示與官方不符，優先比對上述日誌中的**原始 API 數據**。

### OpenCode 常見問題

**Used% 過低（遠低於官方）**：`part` 表的 `$.cost` 是**單筆增量值**，不是累計值。正確做法為直接用 `SUM(cost)` 計算窗口總用量。若誤當成累計值做 delta 計算，會導致 used% 偏低。

**5h Reset 時間過長（>4h）**：5h rolling 的 reset 應為**最新使用時間**往後推 5 小時（`max(time_created) + 5h`）。若誤用 `min(time_created) + 5h`，reset 會固定在窗口最舊記錄 + 5h，導致幾乎不歸零。

**Weekly/Monthly Reset 不對**：Weekly 與 Monthly 的重置時間無法從本地 DB 自動推斷，必須在「設定」頁面手動指定（星期幾/日期 + 時間）。

### Gemini 常見問題

**三個視窗 reset 都相同（~24h）**：API 回傳的 `resetTime` 對所有 model 都是約 +24h，這是正常現象。CC-Switch 等工具對 Flash Lite 顯示較短 reset（如 7h）可能是基於本機使用記錄的額外計算。

**Used% 與官網差異大**：請確認 API 調用是否成功（先 `loadCodeAssist` 取 project ID，再帶入 `retrieveUserQuota`），以及是否按類別（Pro/Flash/Flash Lite）正確聚合取最低 `remainingFraction`。
