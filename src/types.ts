export type ProviderId = "claude-code" | "codex" | "gemini-cli" | "opencode-go";

export type AccuracyLevel = "official" | "local" | "estimated" | "manual";

export type AccountStatus = "available" | "warning" | "limited" | "disconnected" | "connected";

export type RefreshWindowKind = "rolling-5h" | "daily" | "weekly" | "monthly";

export type ThemeId = "dark" | "light" | "aurora" | "graphite";

export type LocaleId = "zh-TW" | "en";

export interface QuotaWindow {
  id: string;
  label: string;
  kind: RefreshWindowKind;
  used: number;
  limit: number;
  resetAt: string;
}

export interface UsageAccount {
  id: string;
  provider: ProviderId;
  accountName: string;
  planName: string;
  status: AccountStatus;
  accuracy: AccuracyLevel;
  lastUpdated: string;
  windows: QuotaWindow[];
  notes: string;
  order: number;
}

export interface OpenCodeResetConfig {
  day: number;
  hour: number;
  minute: number;
}

export interface AppSettings {
  locale: LocaleId;
  theme: ThemeId;
  opencodeWeeklyReset?: OpenCodeResetConfig;
  opencodeMonthlyReset?: OpenCodeResetConfig;
}

export interface DashboardState {
  accounts: UsageAccount[];
  settings: AppSettings;
}

export interface ProviderEnvironment {
  provider: ProviderId;
  label: string;
  detected: boolean;
  source: string;
  detail: string;
}
