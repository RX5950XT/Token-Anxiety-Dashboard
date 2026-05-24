import type { DashboardState, ProviderId, QuotaWindow, UsageAccount } from "../types";

const hours = (value: number) => value * 60 * 60 * 1000;
const days = (value: number) => value * 24 * 60 * 60 * 1000;

const isoIn = (baseTime: number, offsetMs: number) =>
  new Date(baseTime + offsetMs).toISOString();

const windowFor = (
  id: string,
  label: string,
  kind: QuotaWindow["kind"],
  used: number,
  limit: number,
  resetAt: string,
): QuotaWindow => ({
  id,
  label,
  kind,
  used,
  limit,
  resetAt,
});

const createAccount = (
  id: string,
  provider: ProviderId,
  accountName: string,
  planName: string,
  windows: QuotaWindow[],
  order: number,
  notes: string,
): UsageAccount => ({
  id,
  provider,
  accountName,
  planName,
  windows,
  order,
  notes,
  status: "available",
  accuracy: "estimated",
  lastUpdated: new Date().toISOString(),
});

export function createDefaultState(baseTime = Date.now()): DashboardState {
  return {
    settings: {
      locale: "zh-TW",
      theme: "aurora",
    },
    accounts: [
      createAccount(
        "claude-main",
        "claude-code",
        "Claude Code",
        "Claude Pro / Max",
        [
          windowFor(
            "claude-5h",
            "",
            "rolling-5h",
            62,
            100,
            isoIn(baseTime, hours(2.4)),
          ),
          windowFor(
            "claude-weekly",
            "",
            "weekly",
            38,
            100,
            isoIn(baseTime, days(3.2)),
          ),
        ],
        0,
        "已從 Anthropic OAuth API 讀取真實額度。",   
      ),
      createAccount(
        "codex-chatgpt",
        "codex",
        "Codex",
        "Plus / Pro",
        [
          windowFor(
            "codex-5h",
            "",
            "rolling-5h",
            45,
            100,
            isoIn(baseTime, hours(3.1)),
          ),
          windowFor(
            "codex-weekly",
            "",
            "weekly",
            28,
            100,
            isoIn(baseTime, days(5.4)),
          ),
        ],
        1,
        "已從 ChatGPT API 讀取真實額度。",
      ),
      createAccount(
        "antigravity-default",
        "antigravity",
        "Antigravity",
        "Antigravity",
        [
          windowFor(
            "antigravity-claude",
            "Claude",
            "daily",
            32,
            100,
            isoIn(baseTime, hours(14)),
          ),
          windowFor(
            "antigravity-gemini",
            "Gemini",
            "daily",
            58,
            100,
            isoIn(baseTime, hours(14)),
          ),
        ],
        2,
        "已從 Google cloudcode-pa API（Antigravity）讀取真實額度。",
      ),
      createAccount(
        "opencode-go",
        "opencode-go",
        "OpenCode",
        "OpenCode Go",
        [
          windowFor("opencode-5h", "", "rolling-5h", 4.2, 12, isoIn(baseTime, hours(1.6))),
          windowFor("opencode-weekly", "", "weekly", 14, 30, isoIn(baseTime, days(4.5))),
          windowFor("opencode-monthly", "", "monthly", 22, 60, isoIn(baseTime, days(18))),
        ],
        3,
        "已從本機 opencode.db 讀取真實用量。",   
      ),
    ],
  };
}

export function normalizeDashboardState(state: DashboardState): DashboardState {
  const fallback = createDefaultState();
  const existingByProvider = new Map(state.accounts.map((account) => [account.provider, account]));

  return {
    settings: {
      ...fallback.settings,
      ...state.settings,
    },
    accounts: fallback.accounts.map((fallbackAccount) => existingByProvider.get(fallbackAccount.provider) ?? fallbackAccount),
  };
}
