# Token Anxiety Dashboard

Token Anxiety Dashboard 是一個 Tauri 桌面版額度儀錶板，用來集中追蹤 Claude Code、Codex、Gemini CLI 與 OpenCode 的訂閱額度、刷新時間與本機連線狀態。

## 功能狀態

- 左上角 Logo、右上角設定。
- 中央 provider 面板，每張面板顯示 provider、方案、可信度、額度進度條與刷新倒數。
- 面板支援上下拖曳排序，排序會存回本地 SQLite。
- 設定面板支援繁體中文 / English、淺色 / 深色 / 極光玻璃 / 石墨夜色主題。
- 啟動後會自動同步本機資料，並每分鐘刷新一次。
- 本機掃描 Claude Code、Codex、Gemini CLI、OpenCode 常見設定路徑。
- Codex 會從本機 session log 讀取真實 `5-hour rolling` 與 `weekly` rate limits。
- OpenCode 會從本機 `opencode.db` 彙總真實 cost usage，提供 5 小時、7 天、30 天視窗。
- Claude Code 與 Gemini CLI 目前先提供真實登入 / 方案 / 本機設定狀態；若本機沒有可安全讀取的 quota 視窗，就會明確顯示為無法讀取。

## 技術架構

- Desktop shell：Tauri 2
- Frontend：React 19、TypeScript、Vite
- Drag and drop：@dnd-kit
- Icons：lucide-react
- Local storage：Rust command + SQLite
- Tests：Vitest、Testing Library

## 開發指令

```powershell
npm install
npm test
npm run build
npm run tauri dev
```

Rust 端驗證：

```powershell
cd src-tauri
cargo check
```

## 資料來源策略

這個版本不再是純展示 UI，但也不假裝所有 provider 都有穩定公開 usage API。每張面板會以可信度標示資料來源：

- `official`：官方 usage API 或正式欄位。
- `local`：本機 CLI、設定檔或 session 狀態。
- `estimated`：依官方規則與本機校準推估。
- `manual`：使用者手動建立或校準。

目前已接上的真實資料來源：

- `Claude Code`：`claude auth status --json`
- `Codex`：`codex login status` + `~/.codex/sessions/**/*.jsonl`
- `Gemini CLI`：`~/.gemini/google_accounts.json`、`~/.gemini/settings.json`
- `OpenCode`：`~/.local/share/opencode/opencode.db`

OpenCode 的本機偵測也會檢查：

- `~/.local/share/opencode/auth.json`
- `~/.config/opencode/opencode.json`

## 專案備註

目前版本已完成可操作的桌面 UI、本地儲存與本機 adapter。Codex 與 OpenCode 已能讀取真實本機額度資料；Claude Code 與 Gemini CLI 目前以真實登入 / 設定狀態為主，後續若找到穩定且不額外耗額度的 quota 來源，再往下補。
