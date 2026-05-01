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
  "gemini-cli": {
    label: "Gemini CLI",
    shortLabel: "Gemini",
    accent: "#59c889",
    description: "依 Gemini CLI quota tier 顯示每日與模型用量。",
  },
  "opencode-go": {
    label: "OpenCode",
    shortLabel: "OpenCode",
    accent: "#f0bd4f",
    description: "偵測 OpenCode provider auth，依官方 dollar limits 追蹤。",
  },
};

export const providerOptions = Object.entries(providerMeta).map(([id, meta]) => ({
  id: id as ProviderId,
  ...meta,
}));
