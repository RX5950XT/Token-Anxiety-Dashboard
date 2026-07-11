import type { ProviderId } from "../types";

export const providerMeta: Record<
  ProviderId,
  {
    label: string;
    shortLabel: string;
    accent: string;
    description: string;
  }
> = {
  "claude-code": {
    label: "Claude Code",
    shortLabel: "Claude",
    accent: "#c87955",
    description: "讀取 statusline rate_limits 或 CLI session 狀態。",
  },
  codex: {
    label: "Codex",
    shortLabel: "Codex",
    accent: "#46a5ff",
    description: "追蹤 Codex / ChatGPT 訂閱額度與 reset 狀態。",
  },
  antigravity: {
    label: "Antigravity",
    shortLabel: "Antigravity",
    accent: "#59c889",
    description: "讀取 Antigravity 登入憑證，分別顯示 Claude 與 Gemini 的 5 小時與每週用量。",
  },
  "opencode-go": {
    label: "OpenCode",
    shortLabel: "OpenCode",
    accent: "#f0bd4f",
    description: "本機 opencode.db 成本估算；上限對照 Go 常見 $12/$30/$60（非官方 API）。",
  },
  grok: {
    label: "Grok",
    shortLabel: "Grok",
    accent: "#e8e8e8",
    description: "讀取 Grok CLI 登入憑證，顯示每週 credit 用量。",
  },
};

export const providerOptions = Object.entries(providerMeta).map(([id, meta]) => ({
  id: id as ProviderId,
  ...meta,
}));
