import type { AccountStatus, DashboardState, QuotaWindow, UsageAccount } from "../types";
import { t } from "../i18n";
import type { LocaleId } from "../types";

export interface WindowProgress {
  percentage: number;
  resetLabel: string;
  status: AccountStatus;
}

export function clampPercentage(value: number): number {
  if (Number.isNaN(value) || !Number.isFinite(value)) {
    return 0;
  }

  return Math.min(100, Math.max(0, Math.round(value)));
}

export function getWindowProgress(window: QuotaWindow, now = Date.now(), locale: LocaleId = "zh-TW"): WindowProgress {
  const percentage = clampPercentage((window.used / window.limit) * 100);
  return {
    percentage,
    resetLabel: formatDurationUntil(window.resetAt, now, locale),
    status: statusFromPercentage(percentage),
  };
}

export function statusFromPercentage(percentage: number): AccountStatus {
  if (percentage >= 100) {
    return "limited";
  }

  if (percentage >= 80) {
    return "warning";
  }

  return "available";
}

export function deriveAccountStatus(account: UsageAccount, now = Date.now()): AccountStatus {
  if (account.status === "disconnected") {
    return "disconnected";
  }

  // When a provider is logged in but has no quota windows, show "connected"
  // instead of "available" to avoid implying unlimited quota.
  if (account.windows.length === 0) {
    return "connected";
  }

  const statuses = account.windows.map((window) => getWindowProgress(window, now).status);
  if (statuses.includes("limited")) {
    return "limited";
  }

  if (statuses.includes("warning")) {
    return "warning";
  }

  return "available";
}

export function formatDurationUntil(isoDate: string, now = Date.now(), locale: LocaleId = "zh-TW"): string {
  if (!isoDate || isoDate.trim() === "") {
    return "";
  }
  const target = new Date(isoDate).getTime();
  if (Number.isNaN(target)) {
    return t(locale, "needsCalibration");
  }

  const diff = target - now;
  if (diff <= 0) {
    return t(locale, "readyToRefresh");
  }

  const totalSeconds = Math.floor(diff / 1000);
  const days = Math.floor(totalSeconds / 86400);
  const hours = Math.floor((totalSeconds % 86400) / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  // Compact CC-Switch style for both locales
  if (days > 0) {
    return `${days}${t(locale, "day")} ${hours}${t(locale, "hour")}`;
  }

  if (hours > 0) {
    return `${hours}${t(locale, "hour")} ${minutes}${t(locale, "minute")}`;
  }

  if (minutes > 0) {
    return `${minutes}${t(locale, "minute")}`;
  }

  // Show seconds when less than a minute remains
  return `${seconds}${t(locale, "secondsShort")}`;
}

export function summarizeDashboard(state: DashboardState, now = Date.now(), locale: LocaleId = state.settings.locale) {
  const statuses = state.accounts.map((account) => deriveAccountStatus(account, now));
  const limitedCount = statuses.filter((status) => status === "limited").length;
  const warningCount = statuses.filter((status) => status === "warning").length;
  const nextReset = state.accounts
    .flatMap((account) => account.windows)
    .map((window) => new Date(window.resetAt).getTime())
    .filter((time) => !Number.isNaN(time) && time > now)
    .sort((a, b) => a - b)[0];

  return {
    accountCount: state.accounts.length,
    availableCount: statuses.filter((status) => status === "available").length,
    attentionCount: limitedCount + warningCount,
    nextResetLabel: nextReset ? formatDurationUntil(new Date(nextReset).toISOString(), now, locale) : t(locale, "noData"),
  };
}

export function formatWindowLabel(window: QuotaWindow, locale: LocaleId): string {
  if (window.kind === "daily") {
    return t(locale, "dailyUsage");
  }

  if (window.kind === "monthly") {
    return t(locale, "monthlyWindow");
  }

  if (window.kind === "weekly") {
    return t(locale, "weeklyWindow");
  }

  return t(locale, "fiveHourWindow");
}
